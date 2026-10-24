"""Per-update, read-only PINE capture of a match a person plays in the original.

Reads the US game (SLUS-20651) running in PCSX2 with PINE on localhost and
writes one JSON line per game update it sees, for the person's shot, pass
and catch timing (docs/TRACE.md "The person's shots, passes and catches").
Each sample holds:

- the game: the update counter game+0x128, the phase +0xf4, the possession
  +0x110, the side and people bytes +0x98..+0x9b, each actor's controller
  (+0x24 + id*4; 4 is the computer's), the per-actor distance to the
  attacked basket (+0x374, `FUN_0013f748`), the shot clock object +0x190
  (its +4 the seconds, `FUN_00131378`), the meters' clock +0x4a4, the points
  game's stage bounds +0x4b0/+0x4b4 and target +0xf8, the shared generator
  `FUN_003a16b8` (0x4309f0..0x430a04), the release callback's last roll,
  angle and aim kind (0x3d6580..0x3d6588), the options word (config
  0x4bbf10 +0x12c: bits 0..1 the difficulty, 0 Got Game, as the release
  callback reads it at 0x172b68), both meters (meter object +0xc) and the
  sides' statistics (points, baskets, shots, threes made and tried:
  `FUN_00186df8` scope 1);
- the ball: +0x30 position, +0x40 velocity, +0x60/+0x64 the flight's
  progress and length, +0x90 target, +0xf4 holder, +0xf8 shooter, +0xfc
  previous holder, +0x100 pass target, +0x154 the distance to the basket,
  +0x158 timer, +0x178 gamebreaker, +0x180 state, +0x184 kind;
- each of the six actors: +0x30 position, +0x40 motion, +0x60/+0x64 the
  jump's progress and length (the jump opcode `FUN_00194c40` stores
  `FUN_0019bed8`'s updates until landing at +0x64 and 0 at +0x60; the
  shot's release gate, callback 19 `FUN_00174c10`, reads their difference;
  captures before 2026-10-15 23:00 lack them), the pad copy
  `FUN_001587d8` writes every update (+0x78 the held word, +0x7c the pressed
  word, +0x80 the released word; a person's come from his controller record
  `FUN_0012fca0`, a computer's from its AIPlayer's pad), +0x74 the pass
  target, +0x88 the requested heading and +0x90 the intensity (the stick
  as the game reads it: 1024 units a turn, 0 toward -z, 256 toward -x;
  0..64, a diagonal of the D-pad 90), +0xa4 the state bits, +0xa8 the
  heading, +0x1a8 the catch delay, +0x1d0 the pump fake (callback 20,
  `FUN_00172348`), +0x1e4 the shot kind word (`FUN_0016b4b8` writes 9 for
  a jump shot, 8 for a fadeaway; callback 17 `FUN_001726c0` writes 8 when
  the stick fades the shot away; 5 is seen through a dunk), +0x25c the
  live hand, +0x268 the pass kind, +0x298 the AI override (1 while the
  computer walks a person's actor, as in every restart), +0xb0 the facing
  `FUN_0018d290` turns against the rim when `FUN_0016b4b8` picks a shot
  (captures before 2026-10-16 lack it); and channels 0
  and 1 of actor+0xc8's table: +0x10 the script pc, +0x48 flags, +0x4c
  the VM count, +0x50 the record, +0x54 a blend's target, +0x5c the
  frame, +0x64 the float frame;
- once, at discovery: each actor's id and team and his roster record's
  fields 0 (the persistent player id) and 16..26 (the ratings
  `FUN_001d7d00` reads: Shots, Threes, Dunks, ...); captures before
  2026-10-18 lack them for a record whose fields lie 2 past a word.

Since schema 2 (2026-10-18, for the person's passes, docs/TRACE.md "The
person's presses") each sample also holds the six turbo bars (game+0xb0's
pointers) and the controllers' bar slots (game+0xa0), the passout special's
state (game+0x140..+0x160, `FUN_00142068`), and the active camera helper
(0x402020) with its yaw (+0x94, radians: `FUN_00146278` adds
`trunc(yaw * 162.97 + 0.5)` units to the pass's direction, `0x146510`).

Since schema 3 (2026-10-19, for Street School, docs/TRACE.md "Street School
as a capture suite") each sample also holds the game's type (+0x9c: 4 a
lesson) and, in a School game (vtable 0x441680), the lesson manager's words
game+0x4c0..+0x500 (`FUN_0018ec28`, `FUN_0018eec0`: +0x4cc the lesson,
+0x4d0 the result's delay, +0x4d4 the result, +0x4d8 the prompt's state,
+0x4e0 its timer, +0x4e4 the start, +0x4e8 the selected actor, +0x4f0 the
entry count), its listener's (`FUN_0018df00`, +0x00..+0x2c) and the user
payload's lesson words (+0x54..+0xd0: the flags, the nibbles, the launch
counts); in every match both meters' whole records (+0x00..+0x4c), the
gamebreaker task (game+0x434, `FUN_00144df0`: +0x2c..+0x44 its busy flag,
state, count, kind, script and actor), the script manager's +0x30..+0x5c
(game+0x424, +0x50 its state) and +0xb8, the replay manager's +0x28..+0x6c
(game+0x420), the camera manager's +0x00..+0x6c (context 0x400058 +0x10;
+0x40 the gamebreaker camera) and the in-game pause selector 0x4b34a8.
The School menu holds the game (its counter stands still) while it shows:
a sample of a repeated update is kept, marked `repeat`, when the lesson
manager's, its listener's, the user's or the pause selector's words
changed (readers that key on the update skip them).

No memory writes, pad input or patches are issued. Reads are not atomic:
a sample is one PINE batch bracketed by the update counter and is kept as
`torn` when the counter moved inside it. Updates the poller misses appear
as gaps in `frame`. PCSX2's PINE serves one client: run nothing else on
the port meanwhile. The discovery is `tools/pine_ai_capture.py`'s.
"""
import argparse
import json
import threading
import time
from pathlib import Path

from pine_ai_capture import Layout, RNG, TIP_TIMING, TIP_WINNER, discover as discover_ai, signed
from pine_probe import Pine, float_word, pointer

GAME_ROOT = 0x3d30bc
CONFIG = 0x4bbf10
ACTIVE_CAMERA_ROOT = 0x402020
CONTEXT_ROOT = 0x400058
PAUSE_SELECTOR = 0x4b34a8
SCHOOL_VTABLE = 0x441680
SHOT_ROLL = 0x3d6580
ACTOR_FIELDS = [
    ("air_progress", 0x60), ("air_updates", 0x64),
    ("pass_target", 0x74), ("held", 0x78), ("pressed", 0x7c), ("released", 0x80),
    ("requested_heading", 0x88), ("requested_intensity", 0x90), ("bits", 0xa4),
    ("heading", 0xa8), ("facing", 0xb0), ("catch_delay", 0x1a8), ("pump_fake", 0x1d0), ("fadeaway", 0x1e4),
    ("hand", 0x25c), ("pass_kind", 0x268), ("ai_forced", 0x298),
]
CHANNEL_FIELDS = [
    ("pc", 0x10), ("flags", 0x48), ("count", 0x4c), ("record", 0x50), ("target", 0x54),
    ("frame", 0x5c), ("float_frame", 0x64),
]
# The side statistics read (`FUN_00186df8(stats, side, 1, stat)`).
SIDE_STATS = [("points", 0), ("baskets", 7), ("threes_made", 8), ("threes_tried", 9),
              ("shots_tried", 10)]


def discover(pine):
    """The pointers a sample reads; None outside a match."""
    ptr = discover_ai(pine)
    if ptr is None:
        return None
    game = ptr["game"]
    tables = pine.words([a + 0xc8 for a in ptr["actors"]])
    channels = []
    for t in tables:
        pair = pine.words([t, t + 4]) if pointer(t) else [0, 0]
        channels.append([c if pointer(c) else 0 for c in pair])
    ptr["channels"] = channels
    ptr["meters"] = [m if pointer(m) else 0 for m in pine.words([game + 0xc8, game + 0xcc])]
    # The statistics listener (game+0xd4) holds the stats object at +8;
    # scope 1's stat arrays are the pointers at +0x40 + stat*4.
    listener = pine.word(game + 0xd4)
    stats = pine.word(listener + 8) if pointer(listener) else 0
    ptr["stats"] = {}
    if pointer(stats):
        arrays = pine.words([stats + 0x3c + 4 + stat * 4 for _, stat in SIDE_STATS])
        ptr["stats"] = {name: a for (name, _), a in zip(SIDE_STATS, arrays) if pointer(a)}
    ptr["records"] = []
    for actor in ptr["actors"]:
        ident, team, record = pine.words([actor + 0x27c, actor + 0x284, actor + 0x288])
        entry = {"id": signed(ident), "team": signed(team), "record": record}
        if pointer(record):
            data = pine.word(record + 0x18)
            # The fields are u16s at +0x18 + 2k (`FUN_001da4e0`), their table
            # at any even address (save state 1's actors 0 and 4 lie 2 past a
            # word, which captures before 2026-10-18 skipped).
            if 0 < data < 0x02000000 and data % 2 == 0:
                fields = halfwords(pine, data, [0] + list(range(16, 27)))
                entry["player_id"] = fields[0]
                entry["ratings"] = [signed16(v) for v in fields[1:]]
        ptr["records"].append(entry)
    # Schema 3: the School game's lesson manager and the managers every
    # match keeps (fixed for the match; the gamebreaker task is not).
    vtable, replays, context = pine.words([game + 0x20, game + 0x420, CONTEXT_ROOT])
    ptr["school"] = vtable == SCHOOL_VTABLE
    ptr["lesson_listener"] = ptr["lesson_user"] = 0
    if ptr["school"]:
        listener, user = pine.words([game + 0x4c0, game + 0x4c4])
        ptr["lesson_listener"] = listener if pointer(listener) else 0
        ptr["lesson_user"] = user if pointer(user) else 0
    ptr["replays"] = replays if pointer(replays) else 0
    manager = pine.word(context + 0x10) if pointer(context) else 0
    ptr["camera_manager"] = manager if pointer(manager) else 0
    return ptr


def halfwords(pine, base, indices):
    """The u16 fields `indices` of the table at `base` (an even address),
    read as the words that cover them."""
    addresses = sorted({(base + 2 * k) & ~3 for k in indices})
    words = dict(zip(addresses, pine.words(addresses)))
    out = []
    for k in indices:
        at = base + 2 * k
        word = words[at & ~3]
        out.append(word >> 16 if at & 2 else word & 0xFFFF)
    return out


def signed16(h):
    return h - 0x10000 if h & 0x8000 else h


def layout(ptr, camera=0, task=0):
    """Every word of a sample, in one batch; `camera` the active camera
    helper whose yaw the batch reads and `task` the gamebreaker task whose
    words it reads (each the pointer of an earlier sample)."""
    game, ball = ptr["game"], ptr["ball"]
    lay = Layout()
    lay.add("frame", game + 0x128)
    lay.add("game_root", GAME_ROOT)
    lay.add("game_type", game + 0x9c, "i32")
    if ptr.get("school"):
        lay.add("lesson_words", game + 0x4c0, "i32", 17)
        if ptr["lesson_listener"]:
            lay.add("lesson_listener", ptr["lesson_listener"], "i32", 12)
        if ptr["lesson_user"]:
            lay.add("lesson_user", ptr["lesson_user"] + 0x54, "u32", 32)
    lay.add("gb_task", game + 0x434)
    if pointer(task):
        lay.add("gb_task_words", task + 0x2c, "i32", 7)
    if ptr.get("scripts"):
        lay.add("script_words", ptr["scripts"] + 0x30, "i32", 12)
        lay.add("script_b8", ptr["scripts"] + 0xb8, "i32")
    if ptr.get("replays"):
        lay.add("replay_words", ptr["replays"] + 0x28, "u32", 18)
    if ptr.get("camera_manager"):
        lay.add("camera_manager", ptr["camera_manager"], "u32", 28)
    lay.add("pause_selector", PAUSE_SELECTOR, "i32")
    lay.add("phase", game + 0xf4, "i32")
    lay.add("possession", game + 0x110, "i32")
    lay.add("sides_word", game + 0x98)
    lay.add("controllers", game + 0x24, "i32", 6)
    lay.add("distances", game + 0x374, "f32", 6)
    lay.add("shot_clock_words", game + 0x190, "u32", 4)
    lay.add("meter_clock", game + 0x4a4)
    lay.add("target_points", game + 0xf8, "i32")
    lay.add("stage_bounds", game + 0x4b0, "i32", 2)
    lay.add("rng", RNG, "u32", 6)
    lay.add("tip_timing", TIP_TIMING, "i32", 2)
    lay.add("tip_winner", TIP_WINNER, "i32")
    lay.add("shot_roll", SHOT_ROLL, "u32", 3)
    lay.add("options", CONFIG + 0x12c)
    lay.add("front_end_mode", CONFIG + 0x78)
    for k, bar in enumerate(ptr.get("turbo", [])):
        if bar:
            lay.add(f"turbo_bar{k}", bar, "f32")
    lay.add("turbo_slots", game + 0xa0, "i32", 4)
    lay.add("passout", game + 0x140, "u32", 9)
    lay.add("camera", ACTIVE_CAMERA_ROOT)
    if pointer(camera):
        lay.add("camera_yaw", camera + 0x94, "f32")
    for k, meter in enumerate(ptr["meters"]):
        if meter:
            lay.add(f"meter{k}", meter + 0xc, "f32")
            lay.add(f"meter{k}_words", meter, "u32", 20)
    for name, array in ptr["stats"].items():
        lay.add("stat_" + name, array, "i32", 2)
    lay.add("ball_position", ball + 0x30, "f32", 3)
    lay.add("ball_velocity", ball + 0x40, "f32", 3)
    lay.add("ball_progress", ball + 0x60, "i32")
    lay.add("ball_flight_updates", ball + 0x64, "i32")
    lay.add("ball_target", ball + 0x90, "f32", 3)
    lay.add("ball_holder", ball + 0xf4)
    lay.add("ball_shooter", ball + 0xf8)
    lay.add("ball_previous_holder", ball + 0xfc)
    lay.add("ball_target_player", ball + 0x100)
    lay.add("ball_distance", ball + 0x154, "f32")
    lay.add("ball_timer", ball + 0x158, "i32")
    lay.add("ball_gamebreaker", ball + 0x178, "i32")
    lay.add("ball_state", ball + 0x180, "i32")
    lay.add("ball_kind", ball + 0x184, "i32")
    for i, actor in enumerate(ptr["actors"]):
        p = f"a{i}_"
        lay.add(p + "position", actor + 0x30, "f32", 3)
        lay.add(p + "motion", actor + 0x40, "f32", 3)
        for name, offset in ACTOR_FIELDS:
            kind = "i32" if name in ("requested_heading", "requested_intensity", "heading",
                                     "facing", "catch_delay", "pump_fake", "fadeaway", "hand",
                                     "pass_kind") else "u32"
            lay.add(p + name, actor + offset, kind)
        for c, channel in enumerate(ptr["channels"][i]):
            if channel:
                for name, offset in CHANNEL_FIELDS:
                    kind = "f32" if name == "float_frame" else "u32"
                    lay.add(f"{p}c{c}_{name}", channel + offset, kind)
    lay.add("frame_end", game + 0x128)
    return lay


def tidy(sample):
    """Group the flat names into the ball and the actors."""
    out = {"frame": sample.pop("frame")}
    actors = []
    for i in range(6):
        p = f"a{i}_"
        actor = {k[len(p):]: sample.pop(k) for k in list(sample) if k.startswith(p)}
        channels = []
        for c in range(2):
            q = f"c{c}_"
            channel = {k[len(q):]: actor.pop(k) for k in list(actor) if k.startswith(q)}
            if channel:
                channels.append(channel)
        actor["channels"] = channels
        actors.append(actor)
    out["actors"] = actors
    ball = {k[len("ball_"):]: sample.pop(k) for k in list(sample) if k.startswith("ball_")}
    out["ball"] = ball
    stats = {k[len("stat_"):]: sample.pop(k) for k in list(sample) if k.startswith("stat_")}
    out["stats"] = stats
    words = sample["shot_clock_words"]
    sample["shot_clock"] = float_word(words[1])
    bars = {int(key[len("turbo_bar"):]): sample.pop(key) for key in list(sample)
            if key.startswith("turbo_bar")}
    out["turbo"] = [bars.get(k) for k in range(6)]
    out.update(sample)
    return out


class Session:
    """A connected capture: `discover` finds the match, `read` takes one
    sample (None while no match runs or the pointers changed)."""

    def __init__(self, pine):
        self.pine = pine
        self.ptr = None
        self.layout = None
        self.camera = 0
        self.task = 0
        self.events = []

    def discover(self):
        self.ptr = discover(self.pine)
        self.camera = self.pine.word(ACTIVE_CAMERA_ROOT) if self.ptr else 0
        self.task = self.pine.word(self.ptr["game"] + 0x434) if self.ptr else 0
        self.layout = layout(self.ptr, self.camera, self.task) if self.ptr else None
        if self.ptr:
            self.events.append({"event": "pointers", **self.ptr})
        return self.ptr is not None

    def read(self):
        if self.layout is None:
            return None
        sample = self.layout.decode(self.pine.words(self.layout.addresses))
        if sample["game_root"] != self.ptr["game"]:
            self.ptr = self.layout = None
            return None
        sample["torn"] = sample["frame"] != sample.pop("frame_end")
        sample.pop("game_root")
        moved = False
        if sample["camera"] != self.camera:
            # The yaw read was the old helper's: dropped, and the next
            # batch reads the new one's.
            sample.pop("camera_yaw", None)
            self.camera = sample["camera"]
            moved = True
        if sample["gb_task"] != self.task:
            # The same for the gamebreaker task's words.
            sample.pop("gb_task_words", None)
            self.task = sample["gb_task"]
            moved = True
        if moved:
            self.layout = layout(self.ptr, self.camera, self.task)
        sample["wall_time_ns"] = time.time_ns()
        return tidy(sample)


def metadata(pine, note):
    meta = {"emulator": pine.string(8), "title": pine.string(11), "serial": pine.string(12),
            "uuid": pine.string(13), "tool": "pine_played_capture", "schema": 3, "note": note,
            "limitation": "Live reads are not atomic or paused; a sample is one batch "
                          "bracketed by game+0x128. Missed updates are gaps."}
    if meta["serial"] != "SLUS-20651":
        raise RuntimeError(f"Wrong executable for this field map: {meta['serial']}")
    return meta


def connect(port, wait):
    """A PINE connection once the VM answers (PCSX2 accepts the socket
    before the state has booted)."""
    deadline = time.monotonic() + wait
    while True:
        try:
            pine = Pine(port)
            pine.string(12)
            return pine
        except (OSError, RuntimeError):
            if time.monotonic() > deadline:
                raise
            time.sleep(0.5)


def signature(sample):
    """The School words a repeated update's sample is kept for."""
    return json.dumps([sample.get(k) for k in ("lesson_words", "lesson_listener", "lesson_user",
                                                 "pause_selector", "gb_task", "phase")])


class Recorder:
    """Samples a session into a JSON lines file until `stop` is set,
    keeping the last sample in `latest` for a driver to read."""

    def __init__(self, session, out, note=""):
        self.session = session
        self.out = out
        self.note = note
        self.latest = None
        self.stop = threading.Event()
        self.kept = self.torn = self.repeats = 0
        self.thread = threading.Thread(target=self.run, daemon=True)

    def run(self):
        pine = self.session.pine
        temporary = self.out.with_suffix(self.out.suffix + ".tmp")
        with temporary.open("w", encoding="utf-8") as out:
            out.write(json.dumps({"metadata": metadata(pine, self.note)}) + "\n")
            last_frame = last_signature = None
            while not self.stop.is_set():
                if self.session.ptr is None:
                    if not self.session.discover():
                        time.sleep(0.25)
                        continue
                    last_frame = last_signature = None
                for event in self.session.events:
                    out.write(json.dumps(event) + "\n")
                self.session.events.clear()
                sample = self.session.read()
                if sample is None:
                    out.write(json.dumps({"event": "no_match",
                                          "wall_time_ns": time.time_ns()}) + "\n")
                    continue
                # The driver reads the newest, kept or not.
                self.latest = sample
                if sample["frame"] == last_frame:
                    # Street School holds the game (and its counter) while
                    # its menu shows; what the manager and the menu do then
                    # is kept as `repeat` samples when it changes.
                    if signature(sample) == last_signature:
                        self.repeats += 1
                        continue
                    sample["repeat"] = True
                last_frame = sample["frame"]
                last_signature = signature(sample)
                self.torn += sample["torn"]
                self.kept += 1
                out.write(json.dumps(sample, allow_nan=False) + "\n")
            out.write(json.dumps({"coverage": self.coverage()}) + "\n")
        temporary.replace(self.out)

    def coverage(self):
        return {"samples": self.kept, "torn": self.torn, "repeat_polls": self.repeats}


def main():
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--port", type=int, default=28011)
    parser.add_argument("--seconds", type=float, default=120.0, help="at most 1800")
    parser.add_argument("--wait", type=float, default=0.0, help="seconds to wait for a match")
    parser.add_argument("--out", type=Path, required=True, help="JSON lines output")
    parser.add_argument("--note", default="", help="what is being played, kept in the metadata")
    args = parser.parse_args()
    if not 0 < args.seconds <= 1800 or not 0 <= args.wait <= 3600:
        parser.error("seconds must be in (0, 1800], wait in [0, 3600]")
    if args.out.resolve().is_relative_to(Path(__file__).resolve().parents[1]):
        parser.error("keep native captures outside the repository")
    pine = connect(args.port, args.wait)
    session = Session(pine)
    deadline = time.monotonic() + args.wait
    while not session.discover():
        if time.monotonic() > deadline:
            raise RuntimeError("No match running")
        time.sleep(0.25)
    recorder = Recorder(session, args.out, args.note)
    recorder.thread.start()
    try:
        recorder.thread.join(args.seconds)
    finally:
        recorder.stop.set()
        recorder.thread.join()
        pine.socket.close()
    print(json.dumps(recorder.coverage()))


if __name__ == "__main__":
    main()
