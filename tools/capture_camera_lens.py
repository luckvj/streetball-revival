"""Opt-in, read-only US-game lens producer/publication bookends.

Resolve the active helper and renderer chain afresh for every attempt. One
PINE request reads three complete ordered word sets. Equal sets do not prove
atomic execution, a completed generation, or scalar-to-render correspondence.
No guest calls, writes, patches, pauses, restoration or input are performed.
"""
import argparse
import collections
import hashlib
import json
import math
import os
from pathlib import Path
import subprocess
import tempfile
import time

from pine_probe import Pine, float_word, pointer


ROOT = Path(__file__).resolve().parents[1]
RAM_END = 0x02000000
SERIAL = "SLUS-20651"
CLASSES = {"manager": 0x498d10, "camera": 0x498950, "wrapper": 0x49b350}
CALLBACKS = {"manager": (0x38, 0x3046c0),
             "camera": (0x18, 0x301fa8), "wrapper": (0x30, 0x323450)}
LIMITATION = ("Live ordered reads are not atomic or paused. Triple equality "
              "and the late projection+0x154 word do not prove generation identity; "
              "that word is a publication witness, not a counter.")


def address(value):
    return f"0x{value:08x}"


def valid_span(value, size):
    return pointer(value) and value + size <= RAM_END


def typed_word(addr, word, kind):
    value = float_word(word) if kind == "f32" else word
    finite = kind != "f32" or math.isfinite(value)
    return {"address": address(addr), "word": word, "bits": f"{word:08x}",
            "kind": kind, "value": value if finite else None, "finite": finite}


def capture(pine):
    packet = {"capture_started_ns": time.time_ns(), "discovery": [],
              "limitation": LIMITATION}

    def read(stage, addresses):
        addresses = list(addresses)
        entry = {"stage": stage, "addresses": [address(a) for a in addresses]}
        packet["discovery"].append(entry)
        words = pine.words(addresses)
        entry["words"] = words
        if len(words) != len(addresses):
            raise RuntimeError(f"short {stage} word response")
        return words

    def require(value, size, name):
        if not valid_span(value, size):
            packet.update(status="invalid_pointer", invalid_field=name,
                          invalid_word=value, required_span=size)
            return False
        return True

    try:
        context, active = read("roots", [0x400058, 0x402020])
        if not require(context, 0x14, "context") or not require(active, 0x1c8, "active"):
            return packet
        manager, = read("manager", [context + 0x10])
        if not require(manager, 0x68, "manager"):
            return packet
        wrapper, = read("wrapper", [manager + 0x34])
        if not require(wrapper, 0x18, "wrapper"):
            return packet
        projection, wclass, mclass, cclass = read(
            "objects", [wrapper, wrapper + 8, manager + 0x20, active + 0x1b8])
        if not require(projection, 0x194, "projection"):
            return packet
        classes = {"manager": mclass, "camera": cclass, "wrapper": wclass}
        for name, value in classes.items():
            if not require(value, 0x40, name + "_class"):
                return packet
        packet["identities"] = {name: address(value) for name, value in
                                {"context": context, "active": active,
                                 "manager": manager, "wrapper": wrapper,
                                 "projection": projection, **{
                                     name + "_class": value for name, value in classes.items()
                                 }}.items()}
        if classes != CLASSES:
            packet.update(status="outside_class_domain", expected_classes={
                name: address(value) for name, value in CLASSES.items()})
            return packet

        layout = []
        expected = {}

        def add(name, addr, kind="u32", identity=None):
            layout.append((name, addr, kind))
            if identity is not None:
                expected[name] = identity

        for name, addr, value in [
            ("chain.context", 0x400058, context), ("chain.active", 0x402020, active),
            ("chain.manager", context + 0x10, manager),
            ("chain.manager_class", manager + 0x20, mclass),
            ("chain.owner", manager + 0x2c, active),
            ("chain.wrapper", manager + 0x34, wrapper),
            ("chain.camera_class", active + 0x1b8, cclass),
            ("chain.camera_wrapper", active + 0x170, wrapper),
            ("chain.projection", wrapper, projection),
            ("chain.wrapper_class", wrapper + 8, wclass),
            ("chain.projection_self", projection, projection),
        ]:
            add(name, addr, identity=value)
        for name, (offset, _) in CALLBACKS.items():
            add(name + ".callback_adjustment_word", classes[name] + offset)
            add(name + ".callback_target", classes[name] + offset + 4)
        for name, offset in [("scalar", 0x98), ("horizontal_tangent", 0x80),
                             ("distance_parameter", 0x1c0), ("lens_target_ratio", 0x1c4)]:
            add("camera." + name, active + offset, "f32")
        add("manager.state", manager + 0x64)
        add("camera.update_count", active + 0x124)
        add("camera.state140", active + 0x140)
        add("wrapper.widescreen", wrapper + 0xc)
        add("wrapper.preserve_vertical_fov", wrapper + 0x10)
        add("wrapper.incoming_horizontal_degrees", wrapper + 0x14, "f32")
        for name, offset, kind in [
            ("m00", 0x20, "f32"), ("m11", 0x34, "f32"),
            ("matrix_offset_x", 0x40, "f32"), ("matrix_offset_y", 0x44, "f32"),
            ("viewport_x", 0xe0, "u32"), ("viewport_y", 0xe4, "u32"),
            ("viewport_width", 0xe8, "u32"), ("viewport_height", 0xec, "u32"),
            ("requested_x", 0xf8, "u32"), ("requested_y", 0xfc, "u32"),
            ("requested_width", 0x100, "u32"), ("requested_height", 0x104, "u32"),
            ("horizontal_degrees", 0x110, "f32"), ("aspect", 0x114, "f32"),
            ("near", 0x118, "f32"), ("far", 0x11c, "f32"),
            ("viewport_scale_x", 0x120, "f32"), ("viewport_scale_y", 0x124, "f32"),
            ("viewport_offset_x", 0x128, "f32"), ("viewport_offset_y", 0x12c, "f32"),
            ("zoom", 0x190, "f32"),
        ]:
            add("projection." + name, projection + offset, kind)
        for offset in range(0x138, 0x158, 4):
            add(f"projection.frustum_cache_{offset:03x}", projection + offset, "f32")
        for offset in range(0x158, 0x168, 4):
            add(f"projection.frustum_input_{offset:03x}", projection + offset, "f32")
        for name, addr, kind in [
            ("damping_enabled_word", 0x400b40, "u32"),
            ("damping", 0x400b44, "f32"), ("config_initialized", 0x4035bc, "u32"),
            ("fps", 0x4da23c, "u32"),
        ]:
            add("globals." + name, addr, kind)

        addresses = [addr for _, addr, _ in layout]
        packet["layout"] = [{"name": name, "address": address(addr), "kind": kind}
                            for name, addr, kind in layout]
        packet["bracket_started_ns"] = time.time_ns()
        words = pine.words(addresses * 3)
        packet["bracket_finished_ns"] = time.time_ns()
        if len(words) != 3 * len(layout):
            packet["partial_bracket_words"] = words
            raise RuntimeError("short triple bracket word response")
        size = len(layout)
        stages = {name: words[i * size:(i + 1) * size]
                  for i, name in enumerate(("before", "main", "after"))}
        packet["raw_stages"] = stages
        values = {stage: dict(zip((name for name, _, _ in layout), data))
                  for stage, data in stages.items()}
        packet["typed_main"] = {
            name: typed_word(addr, word, kind)
            for (name, addr, kind), word in zip(layout, stages["main"])}
        packet["raw_stable"] = stages["before"] == stages["main"] == stages["after"]
        packet["bookends_equal"] = stages["before"] == stages["after"]
        packet["changed_fields"] = [name for name, _, _ in layout
                                    if len({data[name] for data in values.values()}) != 1]
        packet["identity_matches_discovery"] = {
            stage: all(data[name] == value for name, value in expected.items())
            for stage, data in values.items()}
        descriptors = {}
        for stage, data in values.items():
            descriptors[stage] = {}
            for name, (_, target) in CALLBACKS.items():
                low = data[name + ".callback_adjustment_word"] & 0xffff
                adjustment = low - 0x10000 if low & 0x8000 else low
                descriptors[stage][name] = {
                    "adjustment": adjustment, "target": data[name + ".callback_target"],
                    "matches_original": adjustment == 0 and data[name + ".callback_target"] == target}
        packet["callback_descriptors"] = descriptors
        callbacks_match = all(item["matches_original"] for stage in descriptors.values()
                              for item in stage.values())
        packet["callbacks_match_original"] = callbacks_match
        packet["finite_positive_conversion_inputs"] = all(
            packet["typed_main"][name]["value"] is not None
            and packet["typed_main"][name]["value"] > 0
            for name in ("camera.scalar", "projection.aspect", "projection.zoom"))
        packet["nonfinite_main_fields"] = [
            name for name, item in packet["typed_main"].items() if not item["finite"]]
        if not all(packet["identity_matches_discovery"].values()):
            packet["status"] = "identity_mismatch"
        elif not callbacks_match:
            packet["status"] = "outside_callback_domain"
        elif not packet["raw_stable"]:
            packet["status"] = "unstable"
        elif not packet["finite_positive_conversion_inputs"]:
            packet["status"] = "outside_numeric_domain"
        else:
            packet["status"] = "stable"
        return packet
    except (OSError, RuntimeError, ValueError) as error:
        packet.update(status="read_error", error=str(error))
        return packet
    finally:
        packet["capture_finished_ns"] = time.time_ns()


def source_metadata():
    sources = [{"path": str(path), "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}
               for path in (Path(__file__).resolve(), ROOT / "tools/pine_probe.py")]
    try:
        revision = subprocess.run(["git", "rev-parse", "HEAD"], cwd=ROOT,
                                  capture_output=True, text=True, timeout=5, check=True).stdout.strip()
    except (OSError, subprocess.SubprocessError):
        revision = None
    return {"git_revision": revision, "source_files": sources}


def atomic_json(path, document):
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = None
    try:
        with tempfile.NamedTemporaryFile(mode="w", encoding="utf8", newline="\n",
                                         dir=path.parent, prefix="." + path.name + ".",
                                         suffix=".tmp", delete=False) as stream:
            temporary = Path(stream.name)
            json.dump(document, stream, indent=2, allow_nan=False)
            stream.write("\n")
        os.replace(temporary, path)
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)


def sample_count(value):
    count = int(value)
    if not 1 <= count <= 1000:
        raise argparse.ArgumentTypeError("samples must be 1..1000")
    return count


def interval_seconds(value):
    interval = float(value)
    if not math.isfinite(interval) or not 0 <= interval <= 60:
        raise argparse.ArgumentTypeError("interval must be finite and 0..60 seconds")
    return interval


def parser():
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("--port", type=int, default=28011)
    result.add_argument("--samples", type=sample_count, default=1)
    result.add_argument("--interval", type=interval_seconds, default=0.0)
    result.add_argument("--output", type=Path, required=True)
    result.add_argument("--provenance", default="Active original runtime; this probe issues no restore or input.")
    return result


def main(argv=None):
    cli = parser()
    args = cli.parse_args(argv)
    output = args.output.resolve()
    if output.is_relative_to(ROOT):
        cli.error("retain original captures outside the Git repository")
    if not 1 <= args.port <= 65535:
        cli.error("port must be 1..65535")
    pine = Pine(args.port)
    attempts = []
    interrupted = False
    try:
        metadata = {"emulator": pine.string(8), "title": pine.string(11),
                    "serial": pine.string(12), "uuid": pine.string(13)}
        if metadata["serial"] != SERIAL:
            cli.error(f"exact US serial {SERIAL} required before EE memory reads")
        metadata.update(source_metadata())
        metadata.update(scope="Native lens producer/publication raw brackets only; no conversion or generation parity claim.",
                        read_only=True, limitation=LIMITATION)
        for index in range(args.samples):
            try:
                if index:
                    time.sleep(args.interval)
                attempts.append({"index": index, "packet": capture(pine)})
            except KeyboardInterrupt:
                attempts.append({"index": index, "packet": {"status": "interrupted"}})
                interrupted = True
                break
    finally:
        pine.socket.close()
    result = {"schema": 1, "metadata": metadata, "provenance": args.provenance,
              "requested_samples": args.samples, "interval_seconds": args.interval,
              "interrupted": interrupted, "attempts": attempts,
              "status_counts": dict(collections.Counter(a["packet"]["status"] for a in attempts))}
    atomic_json(output, result)
    print(json.dumps({"output": str(output), "attempts": len(attempts),
                      "status_counts": result["status_counts"]}, allow_nan=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
