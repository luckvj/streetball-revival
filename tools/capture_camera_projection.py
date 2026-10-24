"""Read one original US-game projection through PCSX2 PINE, without writes.

The game context (root 0x400058), renderer wrapper (manager+0x34), and
projection object (*wrapper) are distinct objects. Wrapper+0xc is widescreen;
wrapper+0x10 selects its second widescreen conversion branch. Projection
object+0x10 is a framebuffer pointer, never a widescreen flag.

No emulator pause, restore, input, code patch, or memory write is performed.
Root and field bookends detect some races; PINE is not an atomic EE snapshot.
"""

import argparse
import hashlib
import json
import math
import re
import time
from pathlib import Path

from pine_probe import Pine, float_word, pointer


ROOT = Path(__file__).resolve().parents[1]
GAME_CONTEXT_ROOT = 0x400058
ACTIVE_ROOT = 0x402020
WRAPPER_STATIC = 0x4DBC10
MANAGER_VTABLE = 0x498D10
WRAPPER_VTABLE = 0x49B350
RENDER_CALLBACK = 0x323450
ROW_VTABLE = 0x4985B0

# All offsets are byte offsets. Every value below is an actual memory read.
PROJECTION = [
    ("m00", 0x20, "f32"), ("m11", 0x34, "f32"),
    ("matrix_offset_x", 0x40, "f32"), ("matrix_offset_y", 0x44, "f32"),
    ("viewport_x", 0xE0, "u32"), ("viewport_y", 0xE4, "u32"),
    ("viewport_width", 0xE8, "u32"), ("viewport_height", 0xEC, "u32"),
    ("requested_x", 0xF8, "u32"), ("requested_y", 0xFC, "u32"),
    ("requested_width", 0x100, "u32"), ("requested_height", 0x104, "u32"),
    ("horizontal_degrees", 0x110, "f32"), ("aspect", 0x114, "f32"),
    ("near", 0x118, "f32"), ("far", 0x11C, "f32"),
    ("viewport_scale_x", 0x120, "f32"), ("viewport_scale_y", 0x124, "f32"),
    ("viewport_offset_x", 0x128, "f32"), ("viewport_offset_y", 0x12C, "f32"),
    ("zoom", 0x190, "f32"),
]


def address(value):
    return f"0x{value:08x}"


def require_pointer(value, name):
    if not pointer(value):
        raise ValueError(f"invalid EE RAM pointer for {name}: {address(value)}")
    return value


def record(addr, word, kind="u32"):
    value = float_word(word) if kind == "f32" else word
    return {"address": address(addr), "bits": f"{word:08x}",
            "kind": kind, "value": value if not isinstance(value, float) or math.isfinite(value) else None}


def capture(pine, provenance):
    # Each dependent pointer is range checked before dereferencing.
    context, active = pine.words([GAME_CONTEXT_ROOT, ACTIVE_ROOT])
    require_pointer(context, "game_context")
    require_pointer(active, "active_camera")
    manager = require_pointer(pine.word(context + 0x10), "camera_manager")
    wrapper = require_pointer(pine.word(manager + 0x34), "renderer_wrapper")
    if wrapper != WRAPPER_STATIC:
        raise ValueError(f"unexpected renderer wrapper {address(wrapper)}; expected singleton 0x004dbc10")
    projection, wrapper_vtable = pine.words([wrapper, wrapper + 8])
    require_pointer(projection, "projection_object")
    if wrapper_vtable != WRAPPER_VTABLE:
        raise ValueError(f"unexpected wrapper vtable {address(wrapper_vtable)}")
    manager_vtable, callback, projection_self = pine.words(
        [manager + 0x20, wrapper_vtable + 0x34, projection])
    if (manager_vtable, callback, projection_self) != (MANAGER_VTABLE, RENDER_CALLBACK, projection):
        raise ValueError("native manager/wrapper callback/projection self-identity validation failed")

    chain = [("game_context", GAME_CONTEXT_ROOT, context),
             ("active_camera", ACTIVE_ROOT, active),
             ("camera_manager", context + 0x10, manager),
             ("manager_vtable", manager + 0x20, manager_vtable),
             ("renderer_wrapper", manager + 0x34, wrapper),
             ("projection_object", wrapper, projection),
             ("wrapper_vtable", wrapper + 8, wrapper_vtable),
             ("projection_callback", wrapper_vtable + 0x34, callback),
             ("projection_self", projection, projection)]
    specs = [("camera.scalar", active + 0x98, "f32"),
             ("camera.horizontal_tangent", active + 0x80, "f32"),
             ("camera.vtable", active + 0x1B8, "u32"),
             ("camera.renderer_wrapper", active + 0x170, "u32"),
             ("wrapper.widescreen", wrapper + 0xC, "u32"),
             ("wrapper.preserve_vertical_fov", wrapper + 0x10, "u32"),
             ("wrapper.incoming_horizontal_degrees", wrapper + 0x14, "f32")]
    specs += [("projection." + name, projection + offset, kind)
              for name, offset, kind in PROJECTION]
    specs += [("helpers." + name, manager + offset, "u32")
              for name, offset in [("positioned", 0x24), ("row", 0x28),
                                   ("blend", 0x2C), ("staged", 0x30)]]
    addresses = [addr for _, addr, _ in chain] + [addr for _, addr, _ in specs]
    started = time.time_ns()
    # One PINE request contains two complete, ordered sets of bookend reads.
    words = pine.words(addresses + addresses)
    finished = time.time_ns()
    width = len(addresses)
    before, after = words[:width], words[width:]
    nchain = len(chain)
    fields = {name: record(addr, word, kind)
              for (name, addr, kind), word in zip(specs, before[nchain:])}
    observed = {name: fields["camera." + name]["value"]
                for name in ("scalar", "horizontal_tangent")}
    observed.update({name: fields["projection." + name]["value"]
                     for name in ("horizontal_degrees", "aspect", "zoom", "m00", "m11")})
    chain_stable = before[:nchain] == after[:nchain] == [expected for _, _, expected in chain]
    changed = [name for (name, _, _), a, b in zip(specs, before[nchain:], after[nchain:]) if a != b]
    helper_matches = [name.split(".")[1] for name, entry in fields.items()
                      if name.startswith("helpers.") and entry["value"] == active]
    full_viewport = all(fields["projection." + name]["bits"] == "3f800000"
                        for name in ("viewport_scale_x", "viewport_scale_y"))
    full_viewport &= all(fields["projection." + name]["bits"] in ("00000000", "80000000")
                         for name in ("viewport_offset_x", "viewport_offset_y"))
    validation = {
        "identity_checks_passed": True, "chain_bookends_stable": chain_stable,
        "active_renderer_wrapper_matches_manager": fields["camera.renderer_wrapper"]["value"] == wrapper,
        "fields_changed_between_bookends": changed, "fields_bookends_stable": not changed,
        "active_helper_matches": helper_matches,
        "active_is_validated_row": fields["camera.vtable"]["value"] == ROW_VTABLE and "row" in helper_matches,
        "full_viewport_scale_and_offsets_observed": bool(full_viewport),
        "component_inputs_finite_positive": all(observed[name] is not None and observed[name] > 0
                                                 for name in ("scalar", "aspect", "zoom")),
    }
    return {
        "schema": 1, "provenance": provenance,
        "capture_started_ns": started, "capture_finished_ns": finished,
        "active": address(active), "context": address(projection),
        "identities": {"game_context": address(context), "camera_manager": address(manager),
                       "renderer_wrapper": address(wrapper), "projection_object": address(projection)},
        "observed": observed, "fields": fields,
        "bookends": {"chain": {name: {"address": address(addr), "expected": address(expected),
                                      "before": address(a), "after": address(b)}
                                for (name, addr, expected), a, b in zip(chain, before, after)},
                     "field_bits_after": {name: f"{word:08x}"
                                          for (name, _, _), word in zip(specs, after[nchain:])}},
        "validation": validation,
        "static_evidence": {
            "game_context_to_manager": "FUN_002f3000 creates FUN_003038b0 at DAT_00400058[4]",
            "manager_to_wrapper": "FUN_003038b0 +0x34 = FUN_00323988 singleton 0x004dbc10",
            "wrapper_to_projection": "FUN_00322f90 stores FUN_00376da0 result at *wrapper",
            "active_to_wrapper": "FUN_002fa1e0 stores wrapper at active camera+0x170; sets +0x80 from +0x98",
            "wrapper_flags": "FUN_00323450 reads wrapper+0xc and +0x10; FUN_003232a0 initializes both zero",
            "aspect_and_zoom": "FUN_00378328 stores actual aspect +0x114 and reads zoom +0x190",
            "viewport": "FUN_00377fe8 writes scales +0x120/+0x124, offsets +0x128/+0x12c",
        },
        "limitations": [
            "Read-only PINE request; EE execution is not paused or atomic. Stable bookends cannot prove simultaneity.",
            "No savestate restore is performed; provenance describes the caller's current original-runtime state.",
            "observed.horizontal_degrees is the consumed projection-object +0x110, not a computed oracle value.",
            "Context means projection object here; its +0x10 is a framebuffer pointer. Widescreen flags belong to renderer_wrapper.",
            "Scalar generation, whole camera/controller, depth/crop behavior, and framebuffer pixels are not compared.",
            "Helper identity is recorded; non-row helpers are never converted into a per-mode observation.",
        ],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--port", type=int, default=28011)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--provenance", required=True)
    args = parser.parse_args()
    output = args.output.resolve()
    if output.is_relative_to(ROOT):
        parser.error("retain original-runtime captures outside the Git repository")
    pine = Pine(args.port)
    try:
        metadata = {"emulator": pine.string(8), "title": pine.string(11),
                    "serial": pine.string(12), "uuid": pine.string(13)}
        serial = re.sub(r"[^A-Za-z0-9]", "", metadata["serial"]).upper()
        if serial != "SLUS20651":
            raise ValueError(f"wrong game serial {metadata['serial']!r}; require SLUS20651 before fixed-address reads")
        result = capture(pine, args.provenance)
        result["metadata"] = metadata
    finally:
        pine.socket.close()
    result["capture_tool_sha256"] = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(result, indent=2, allow_nan=False) + "\n", encoding="utf-8")
    print(json.dumps({"output": str(output), "observed": result["observed"],
                      "validation": result["validation"]}, indent=2))


if __name__ == "__main__":
    main()
