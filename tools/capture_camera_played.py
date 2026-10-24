"""Per-update, read-only PINE capture of the original's camera in a match.

Reads the US game (SLUS-20651) in PCSX2 with PINE on localhost and writes one
JSON line per game update it sees (game+0x128). Each sample holds:

- the game: phase +0xf4, possession +0x110, the side bytes +0x98, +0x19c,
  +0x1a0, each actor's controller (+0x24 + id*4, 4 the computer's) and
  distance to the attacked basket (+0x374), both gamebreaker meters (+0xc);
- the ball: +0x30 position, +0x40 motion, +0xf4 holder, +0xf8 shooter,
  +0x100 pass target, +0x154 distance to the basket, +0x178, +0x180 state,
  +0x184 kind;
- the six actors' +0x30 position, +0x40 physical vector, +0xa4 bits, +0x260
  height (the camera's focus margin is 1.2 times it) and channel 0's clip
  record (+0x50, which FUN_00300b98 tests);
- the camera manager (context 0x400058 +0x10): words +0x00..+0x6c, which
  include the helpers +0x24..+0x30, +0x40 (gamebreaker), +0x4c (manager
  attract flag: 0 in a played match) and +0x50..+0x60;
- the active helper `DAT_00402020` and the camera globals (`0x40200c`,
  `0x402010`, `0x402014`, the Street offsets `0x400b58`/`0x400b5c`, their
  pointer gate `0x400b1c`, `0x400b54`, the source entity `0x401098` and
  the script flag `0x40108c`, the shot depth `0x401080` and the scales
  `0x4d8d04..0x4d8d0c`), and the front end's mode `0x4bbf88`;
- the row helper (manager +0x28) words +0x00..+0x210, and the active
  helper's when it is another one (positioned +0x2c, blend +0x30).

No memory writes, pad input or patches are issued. Reads are not atomic: one
sample is a single PINE batch bracketed by the update counter and is kept as
`torn` when the counter moved inside it. Updates the poller misses appear as
gaps in `frame`. A read can still land between the game update and the
camera's (or inside a selection's settle loop): the same update is then read
again when the camera's pose has moved, flagged `frozen`, as are the camera's
moves while a replay holds the game update. Readers keep the last sample of
each update (`tools/compare_camera_played.py`).

PCSX2's PINE serves one client: run nothing else on the port meanwhile.
"""
import argparse
import json
import time
from pathlib import Path

from pine_probe import Pine, pointer

GAME_ROOT = 0x3d30bc
CONTEXT_ROOT = 0x400058
ACTIVE_CAMERA = 0x402020
GLOBALS = {
    "toss_pending_40200c": 0x40200C,
    "cut_clock_402010": 0x402010,
    "cut_clock_402014": 0x402014,
    "street_height_400b58": 0x400B58,
    "street_lens_400b5c": 0x400B5C,
    "offset_gate_400b1c": 0x400B1C,
    "threshold_400b54": 0x400B54,
    "source_entity_401098": 0x401098,
    "script_40108c": 0x40108C,
    "front_end_mode_4bbf88": 0x4BBF88,
    "depth_401080": 0x401080,
    "pass_depth_scale_4d8d04": 0x4D8D04,
    "shot_depth_scale_4d8d08": 0x4D8D08,
    "pass_yaw_half_4d8d0c": 0x4D8D0C,
}
# Helper words: +0x00..+0x210 (the basis, the pose +0x84..+0x98, the rates,
# the focus +0xc0/+0xd0/+0xe0, motion +0xf0, the gates, the descriptor
# +0x174/+0x180/+0x1a0, the row +0x1f0/+0x1f4 and the shake +0x1fc..+0x208).
HELPER_WORDS = 0x214 // 4
MANAGER_WORDS = 0x70 // 4


def discover(pine):
    """The pointers that stay fixed through a match; None outside one."""
    game = pine.word(GAME_ROOT)
    if not pointer(game):
        return None
    ball, team0, team1 = pine.words([game + 0x130, game + 0x184, game + 0x188])
    if not (pointer(ball) and pointer(team0) and pointer(team1)):
        return None
    actors = []
    for team in (team0, team1):
        array = pine.word(team + 0x44)
        if not pointer(array):
            return None
        actors.extend(pine.words([array, array + 4, array + 8]))
    if not all(pointer(a) for a in actors):
        return None
    context = pine.word(CONTEXT_ROOT)
    if not pointer(context):
        return None
    manager = pine.word(context + 0x10)
    if not pointer(manager):
        return None
    helpers = pine.words([manager + 0x24, manager + 0x28, manager + 0x2C, manager + 0x30])
    if not all(pointer(h) for h in helpers):
        return None
    # Each actor's channel 0 (FUN_0019c2e0(actor + 0xc4, 0)), whose clip
    # record +0x50 FUN_00300b98 tests.
    tables = pine.words([a + 0xC8 for a in actors])
    channels = pine.words(tables) if all(pointer(t) for t in tables) else [0] * 6
    # The two gamebreaker meters (game+0xc8/+0xcc; +0xc the fill, 50000 full).
    meters = pine.words([game + 0xC8, game + 0xCC])
    return dict(game=game, ball=ball, actors=actors, manager=manager, helpers=helpers,
                channels=channels, meters=meters)


def layout(ptr, active):
    """The addresses of one sample, in order, and the names that split them."""
    game, ball = ptr["game"], ptr["ball"]
    parts = []

    def add(name, address, count=1):
        parts.append((name, [address + 4 * i for i in range(count)]))

    add("frame", game + 0x128)
    add("game_root", GAME_ROOT)
    add("phase", game + 0xF4)
    add("possession", game + 0x110)
    add("sides", game + 0x98)
    add("ball_position", ball + 0x30, 4)
    add("ball_motion", ball + 0x40, 4)
    add("ball_holder", ball + 0xF4)
    add("ball_state", ball + 0x180)
    add("ball_kind", ball + 0x184)
    add("ball_gamebreaker", ball + 0x178)
    add("ball_shooter", ball + 0xF8)
    add("ball_target", ball + 0x100)
    add("ball_distance", ball + 0x154)
    # Each actor's planar distance to the attacked basket (FUN_0013f748).
    add("distances", game + 0x374, 6)
    # Each actor's controller (game+0x24 + id*4; 4 is the computer's, as
    # FUN_00156d98 reads it for FUN_00300b08).
    add("controllers", game + 0x24, 6)
    add("game_19c", game + 0x19C)
    add("game_1a0", game + 0x1A0)
    for k, meter in enumerate(ptr.get("meters", [])):
        if pointer(meter):
            add(f"meter{k}", meter + 0xC)
    for i, actor in enumerate(ptr["actors"]):
        add(f"a{i}_position", actor + 0x30, 4)
        add(f"a{i}_motion", actor + 0x40, 4)
        add(f"a{i}_bits", actor + 0xA4)
        add(f"a{i}_height", actor + 0x260)
        if pointer(ptr.get("channels", [0] * 6)[i]):
            add(f"a{i}_clip", ptr["channels"][i] + 0x50)
    add("manager", ptr["manager"], MANAGER_WORDS)
    add("active", ACTIVE_CAMERA)
    for name, address in GLOBALS.items():
        add(name, address)
    add("row", ptr["helpers"][1], HELPER_WORDS)
    if active != ptr["helpers"][1] and pointer(active):
        add("active_helper", active, HELPER_WORDS)
    add("frame_end", game + 0x128)
    return parts


def pose_words(sample):
    """The helper on screen's pose words +0x84..+0x98 (with its identity)."""
    words = sample.get("active_helper") or sample["row"]
    return (sample["active"], tuple(words[0x84 // 4:0x9C // 4]))


def decode(parts, words):
    out = {}
    at = 0
    for name, addresses in parts:
        raw = words[at:at + len(addresses)]
        at += len(addresses)
        out[name] = raw[0] if len(raw) == 1 else raw
    return out


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--port", type=int, default=28011)
    parser.add_argument("--seconds", type=float, default=60.0, help="at most 1800")
    parser.add_argument("--wait", type=float, default=0.0, help="seconds to wait for a match")
    parser.add_argument("--out", type=Path, required=True, help="JSON lines output")
    parser.add_argument("--note", default="", help="what is being played, kept in the metadata")
    args = parser.parse_args()
    if not 0 < args.seconds <= 1800 or not 0 <= args.wait <= 3600:
        parser.error("seconds must be in (0, 1800], wait in [0, 3600]")
    if args.out.resolve().is_relative_to(Path(__file__).resolve().parents[1]):
        parser.error("keep native captures outside the repository")
    deadline = time.monotonic() + args.wait
    while True:
        # PINE answers before the VM has booted the state: retry until then.
        try:
            pine = Pine(args.port)
            pine.string(12)
            break
        except (OSError, RuntimeError):
            if time.monotonic() > deadline:
                raise
            time.sleep(0.5)
    kept = torn = repeats = 0
    try:
        metadata = {"emulator": pine.string(8), "title": pine.string(11),
                    "serial": pine.string(12), "uuid": pine.string(13),
                    "tool": "capture_camera_played", "schema": 1, "note": args.note,
                    "words": "raw u32 words; floats are their bit patterns",
                    "limitation": "Live reads are not atomic or paused; a sample is one "
                                  "batch bracketed by game+0x128. Missed updates are gaps."}
        if metadata["serial"] != "SLUS-20651":
            raise RuntimeError(f"Wrong executable for this field map: {metadata['serial']}")
        ptr = discover(pine)
        while ptr is None:
            if time.monotonic() > deadline:
                raise RuntimeError("No match running")
            time.sleep(0.25)
            ptr = discover(pine)
        with args.out.open("w", encoding="utf-8") as out:
            out.write(json.dumps({"metadata": metadata}) + "\n")
            out.write(json.dumps({"event": "pointers", **ptr}) + "\n")
            end = time.monotonic() + args.seconds
            last_frame = last_pose = None
            active = pine.word(ACTIVE_CAMERA)
            parts = layout(ptr, active)
            addresses = [a for _, part in parts for a in part]
            while time.monotonic() < end:
                sample = decode(parts, pine.words(addresses))
                if sample["game_root"] != ptr["game"]:
                    again = discover(pine)
                    if again is None:
                        time.sleep(0.1)
                        continue
                    ptr = again
                    out.write(json.dumps({"event": "pointers", **ptr}) + "\n")
                    parts = layout(ptr, sample["active"])
                    addresses = [a for _, part in parts for a in part]
                    last_frame = None
                    continue
                if sample["active"] != active:
                    # Another helper went on screen: read its words from now on.
                    active = sample["active"]
                    parts = layout(ptr, active)
                    addresses = [a for _, part in parts for a in part]
                    sample = decode(parts, pine.words(addresses))
                pose = pose_words(sample)
                if sample["frame"] == last_frame:
                    # The game update stops while a replay plays (the
                    # manager suspends AIGame); keep the camera's moves.
                    if pose == last_pose:
                        repeats += 1
                        continue
                    sample["frozen"] = True
                sample["torn"] = sample["frame"] != sample.pop("frame_end")
                sample.pop("game_root")
                sample["wall_time_ns"] = time.time_ns()
                torn += sample["torn"]
                kept += 1
                last_frame = sample["frame"]
                last_pose = pose
                out.write(json.dumps(sample) + "\n")
            out.write(json.dumps({"coverage": {"samples": kept, "torn": torn,
                                               "repeat_polls": repeats}}) + "\n")
        print(json.dumps({"samples": kept, "torn": torn, "repeat_polls": repeats}))
    finally:
        pine.socket.close()


if __name__ == "__main__":
    main()
