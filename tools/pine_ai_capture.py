"""Per-update, read-only PINE capture of the original's computer players.

Reads the US game (SLUS-20651) running in PCSX2 with PINE on localhost and
writes one JSON line per game update it sees: the game's phase and clocks
(game+0xf4, +0x110, +0x128 the update counter, +0x404..+0x418 the restart
ready flags), the shared generator `FUN_003a16b8` (0x4309f0..0x430a04), the
tip-off globals (`DAT_003d6578`, `DAT_003d6570`), the ball, the referee
(game+0x170, his AIPlayer +0x194 and its front node), and for each of the six
actors his placement, his pad copy and his AIPlayer (`FUN_0013a760`): the pad
object (+4: +0 built word, +4 pressed, +8 held, +0xc stick angle, +0x10
strength, +0x24 receiver), the behaviour queue (+0xc node, children +0x24
begin, +0x28 end; each node +0xc type, +0x14 vtable, +4 age, +8 started) and
the speed level (+0x48), plus both AITeams (game+0x184/+0x188: +0x5c strategy,
its vtable +8, +0xa0 last holder, +0xa4 saving turbo, +0xb8 timer).
Since 2026-10-17 each sample also holds the scoring state: the combo
manager (game+0xd0: the six-slot ring +0x4..+0x9b, each slot class, extra,
tick, value, span and weight; +0x144 its side, +0x148 the matched row,
+0x14c, +0x150 the matched total, +0x154..+0x170 its flags), both meter
objects (game+0xc8/+0xcc, +0xc..+0x60: the meter, hold, gamebreaker,
pocket, the trick value and repeat ring, the one-shot flags and the
dunk/gamebreaker/passout/alley-oop values), each actor's +0x1e4,
+0x218, +0x248, +0x24c, +0x264, +0x26c and +0x270, the statistics (the stats
object at game+0xd4 +8: stats 0..14 for the six actors and the two sides,
and the side-only 0x10..0x27), the ball's +0x10c, +0x178, +0x184 and
+0x1ac, and the script manager's state (game+0x424 +0x50).
Since schema 3 (2026-10-17) each sample also holds the rosters and the turbo:
each actor's player record (+0x274, the database row `street_assets::db`
reads), his updates on court (+0x2a4) and his turbo owner flag (+0x2ac), the
six turbo bars (game+0xb0's pointers), the controllers' map (game+0x24 by
actor, game+0xa0 by controller), and the playbook's star and the weights it
was drawn by (strategy +0xc8..+0xd4).

No memory writes, pad input or patches are issued. Reads are not atomic: a
sample is two PINE batches bracketed by the update counter, and is kept as
`torn` when the counter moved between them. Updates the poller misses appear
as gaps in `frame`.
"""
import argparse
import json
import struct
import time
from pathlib import Path

from pine_probe import Pine, float_word, pointer

GAME_ROOT = 0x3d30bc
RNG = 0x4309f0
TIP_TIMING = 0x3d6578
TIP_WINNER = 0x3d6570
CONTEXT_ROOT = 0x400058
ACTIVE_CAMERA_ROOT = 0x402020
QUEUE_LIMIT = 6

# Behaviour vtables by constructor (each calls FUN_0013be20(node, type) and
# stores the vtable at node+0x14), with the names docs/TRACE.md 7b uses.
BEHAVIOURS = {
    0x437398: ("shoot", 0x100e38), 0x43ae88: ("playbook hold", 0x1276f8),
    0x4370f8: ("leaper", 0x100100), 0x4370a0: ("second leaper", 0x100360),
    0x437448: ("pass", 0x100a08), 0x4373f0: ("trick pass", 0x100c60),
    0x437340: ("carrier move", 0x101698), 0x4372e8: ("carrier pause", 0x101948),
    0x437290: ("pocket", 0x102420), 0x437238: ("cancel", 0x1024b8),
    0x437538: ("defense", 0x1062b8), 0x437708: ("drive", 0x1095d8),
    0x437948: ("fast break", 0x10a378), 0x437898: ("wait for hc", 0x10a430),
    0x4378f0: ("outlet", 0x10af48), 0x437aa0: ("throw inbound", 0x10b658),
    0x437a48: ("inbound pos", 0x10ba18), 0x437bf8: ("iso3pt", 0x10c0a8),
    0x437cd0: ("pause 0x10d090", 0x10d090), 0x437db8: ("keep away", 0x10d4c8),
    0x437f28: ("loose ball", 0x10dd68), 0x437ed0: ("open loose ball", 0x10ea98),
    0x438000: ("goto", 0x10f278), 0x4380d8: ("pause", 0x10f748),
    0x438368: ("cut move", 0x10fae8), 0x438310: ("cut out", 0x10fd78),
    0x4382b8: ("net cut in", 0x110868), 0x438260: ("circle cut in", 0x110c48),
    0x438208: ("cut to", 0x111858), 0x437150: ("node under construction", 0x13be20), 0x438450: ("perim pause", 0x111d50),
    0x4385b0: ("perim player", 0x1121e0), 0x4386a8: ("post cut", 0x112b80),
    0x438810: ("post pause", 0x1135a8), 0x438910: ("post player", 0x114910),
    0x438a58: ("post up", 0x115368), 0x438ce0: ("prepare rebound", 0x1161e8),
    0x438c88: ("drift up court", 0x116580), 0x438c30: ("rebound", 0x116860),
    0x438bd8: ("goal tend", 0x116fb8), 0x438b80: ("box out", 0x117290),
    0x438dc0: ("roll", 0x117e80), 0x438f90: ("set pick", 0x118508),
    0x438f38: ("run pick play", 0x118f60), 0x439098: ("pick off ball", 0x1199e0),
    0x4391f0: ("feeder", 0x11a580), 0x4392f0: ("referee wait", 0x11ad68),
    0x439348: ("wait for toss", 0x11af98),
}


def signed(word):
    return struct.unpack("<i", struct.pack("<I", word))[0]


class Layout:
    """A flat address list read in one PINE batch, decoded by name."""

    def __init__(self):
        self.addresses = []
        self.fields = []

    def add(self, name, address, kind="u32", count=1):
        self.fields.append((name, len(self.addresses), kind, count))
        self.addresses.extend(address + 4 * i for i in range(count))

    def decode(self, words):
        out = {}
        for name, at, kind, count in self.fields:
            raw = words[at:at + count]
            if kind == "f32":
                value = [float_word(w) for w in raw]
            elif kind == "i32":
                value = [signed(w) for w in raw]
            else:
                value = list(raw)
            out[name] = value[0] if count == 1 else value
        return out


def discover(pine):
    """Resolve the pointers a sample reads; None outside a match."""
    game = pine.word(GAME_ROOT)
    if not pointer(game):
        return None
    ball, team0, team1, referee = pine.words([game + 0x130, game + 0x184, game + 0x188,
                                               game + 0x170])
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
    ais = pine.words([a + 0x290 for a in actors])
    pads = [pine.word(ai + 4) if pointer(ai) else 0 for ai in ais]
    referee_ai = pine.word(referee + 0x194) if pointer(referee) else 0
    combo, meter0, meter1, listener, scripts = pine.words(
        [game + 0xd0, game + 0xc8, game + 0xcc, game + 0xd4, game + 0x424])
    stats = pine.word(listener + 8) if pointer(listener) else 0
    arrays = pine.words([stats + 4 + 4 * k for k in range(30)]) if pointer(stats) else []
    turbo = pine.words([game + 0xb0 + 4 * k for k in range(6)])
    return dict(game=game, ball=ball, teams=[team0, team1], actors=actors, ais=ais,
                turbo=[t if pointer(t) else 0 for t in turbo],
                pads=pads, referee=referee, referee_ai=referee_ai,
                combo=combo if pointer(combo) else 0,
                meters=[m if pointer(m) else 0 for m in (meter0, meter1)],
                stats=stats if pointer(stats) else 0,
                stat_arrays=[a if pointer(a) else 0 for a in arrays],
                scripts=scripts if pointer(scripts) else 0)


def first_layout(ptr):
    """Every fixed-address word of a sample (batch one)."""
    game, ball = ptr["game"], ptr["ball"]
    lay = Layout()
    lay.add("frame", game + 0x128)
    lay.add("game_root", GAME_ROOT)
    lay.add("phase", game + 0xf4, "i32")
    lay.add("possession", game + 0x110, "i32")
    lay.add("ready", game + 0x404, "u32", 6)
    lay.add("restart_4a4", game + 0x4a4)
    lay.add("held_frames", game + 0x490, "i32")
    lay.add("loose_frames", game + 0x440, "i32")
    lay.add("last_team", game + 0x43c, "i32")
    lay.add("shot_clock_words", game + 0x190, "u32", 4)
    lay.add("game_104", game + 0x104)
    lay.add("game_9c", game + 0x9c)
    # The side bytes (game+0x98+team, FUN_001404c0) and the people bytes
    # (game+0x9a+team, FUN_0018ab00), as one word.
    lay.add("sides_word", game + 0x98)
    # The restart strategies' globals: DAT_003ce9b0 (defence has spots),
    # 0x3ce9b4 (replanned), 0x3ce9b8 (row), 0x3ce9bc (look timer) and the
    # order DAT_004e7408..0x4e7410.
    lay.add("restart_globals", 0x3ce9b0, "i32", 4)
    lay.add("restart_order", 0x4e7408, "i32", 3)
    lay.add("rng", RNG, "u32", 6)
    lay.add("tip_timing", TIP_TIMING, "i32", 2)
    lay.add("tip_winner", TIP_WINNER, "i32")
    lay.add("ball_position", ball + 0x30, "f32", 3)
    lay.add("ball_velocity", ball + 0x40, "f32", 3)
    lay.add("ball_progress", ball + 0x60, "i32")
    lay.add("ball_flight_updates", ball + 0x64, "i32")
    lay.add("ball_target", ball + 0x90, "f32", 3)
    lay.add("ball_holder", ball + 0xf4)
    lay.add("ball_previous_holder", ball + 0xfc)
    lay.add("ball_target_player", ball + 0x100)
    lay.add("ball_gravity", ball + 0x118, "f32")
    lay.add("ball_timer", ball + 0x158, "i32")
    lay.add("ball_state", ball + 0x180, "i32")
    referee = ptr["referee"]
    if pointer(referee):
        lay.add("referee_position", referee + 0x30, "f32", 3)
        lay.add("referee_400", referee + 0x190)
        lay.add("referee_channels", referee + 0xc8)
    if pointer(ptr["referee_ai"]):
        lay.add("referee_queue", ptr["referee_ai"] + 0x24, "u32", 2)
    for i, actor in enumerate(ptr["actors"]):
        p = f"a{i}_"
        lay.add(p + "position", actor + 0x30, "f32", 3)
        lay.add(p + "pressed", actor + 0x7c)
        lay.add(p + "pass_target", actor + 0x74)
        lay.add(p + "requested_heading", actor + 0x88, "i32")
        lay.add(p + "requested_intensity", actor + 0x90, "i32")
        lay.add(p + "heading", actor + 0xa8, "i32")
        lay.add(p + "animation_state", actor + 0xa4)
        lay.add(p + "catch_delay", actor + 0x1a8, "i32")
        lay.add(p + "id", actor + 0x27c, "i32")
        lay.add(p + "slot", actor + 0x280, "i32")
        lay.add(p + "team", actor + 0x284, "i32")
        lay.add(p + "ai_forced", actor + 0x298)
        lay.add(p + "channels", actor + 0xc8)
        # The scoring words (2026-10-17): +0x1e4 the shot kind, +0x218 the
        # fake's qualifier, +0x248 the trick's value class, +0x24c the
        # dunk's finish, +0x264 the trick's script.
        lay.add(p + "shot_kind", actor + 0x1e4, "i32")
        lay.add(p + "faked", actor + 0x218, "i32")
        lay.add(p + "value_class", actor + 0x248, "i32", 2)
        lay.add(p + "trick_script", actor + 0x264, "i32")
        # +0x26c the lay-up kind, +0x270 the alley-oop's style bits.
        lay.add(p + "finish_kinds", actor + 0x26c, "i32", 2)
        # The roster and the turbo (schema 3): the player record, the
        # updates on court and the turbo owner flag.
        lay.add(p + "record", actor + 0x274, "i32")
        lay.add(p + "on_court", actor + 0x2a4, "i32")
        lay.add(p + "turbo_owner", actor + 0x2ac, "i32")
        ai, pad = ptr["ais"][i], ptr["pads"][i]
        if pointer(ai):
            lay.add(p + "queue", ai + 0x24, "u32", 2)
            lay.add(p + "ai_frame", ai + 0x38, "i32")
            lay.add(p + "speed_level", ai + 0x48, "i32")
            lay.add(p + "speed_changed", ai + 0x4c, "i32")
        if pointer(pad):
            lay.add(p + "pad", pad, "u32", 3)
            lay.add(p + "stick", pad + 0xc, "i32", 2)
            lay.add(p + "receiver", pad + 0x24)
    for k, team in enumerate(ptr["teams"]):
        p = f"t{k}_"
        lay.add(p + "index", team + 0x30, "i32")
        lay.add(p + "strategy", team + 0x5c)
        lay.add(p + "open", team + 0x94, "i32", 3)
        lay.add(p + "last_holder", team + 0xa0, "i32")
        lay.add(p + "saving_turbo", team + 0xa4)
        lay.add(p + "timer", team + 0xb8, "i32")
    # The scoring state (2026-10-17): the combo manager's ring and flags,
    # both meters, the statistics, the ball's scoring words and the script
    # manager's state.
    combo = ptr.get("combo", 0)
    if pointer(combo):
        lay.add("cm_ring", combo + 4, "i32", 36)
        lay.add("cm_state", combo + 0x144, "i32", 12)
    for k, meter in enumerate(ptr.get("meters", [0, 0])):
        if pointer(meter):
            lay.add(f"m{k}_value", meter + 0xc, "f32")
            lay.add(f"m{k}_words", meter + 0x10, "i32", 11)
            lay.add(f"m{k}_fill", meter + 0x40, "f32", 3)
            lay.add(f"m{k}_tail", meter + 0x4c, "i32", 6)
    arrays = ptr.get("stat_arrays", [])
    for k, array in enumerate(arrays[:15]):
        if pointer(array):
            lay.add(f"st_actor{k}", array, "i32", 6)
    for k, array in enumerate(arrays[15:30]):
        if pointer(array):
            lay.add(f"st_side{k}", array, "i32", 2)
    if pointer(ptr.get("stats", 0)):
        lay.add("st_extra", ptr["stats"] + 0xb8, "i32", 48)
    for k, bar in enumerate(ptr.get("turbo", [])):
        if pointer(bar):
            lay.add(f"turbo_bar{k}", bar, "f32")
    lay.add("controllers", game + 0x24, "i32", 6)
    lay.add("turbo_slots", game + 0xa0, "i32", 4)
    lay.add("ball_scoring", ball + 0x10c, "i32")
    lay.add("ball_shooter", ball + 0xf8)
    lay.add("ball_gamebreaker", ball + 0x178, "i32")
    lay.add("ball_kind", ball + 0x184, "i32")
    lay.add("ball_passout", ball + 0x1ac, "i32")
    if pointer(ptr.get("scripts", 0)):
        lay.add("script_state", ptr["scripts"] + 0x50, "i32")
    lay.add("active_camera", ACTIVE_CAMERA_ROOT)
    # The front end's mode (config 0x4bbf10 +0x78): 0 the attract demo.
    lay.add("front_end_mode", 0x4bbf88)
    lay.add("context", CONTEXT_ROOT)
    return lay


def second_layout(ptr, first):
    """Words behind pointers read in batch one (batch two)."""
    lay = Layout()
    queues = [(f"a{i}_", first.get(f"a{i}_queue")) for i in range(6)]
    queues.append(("referee_", first.get("referee_queue")))
    for p, queue in queues:
        if not queue or not pointer(queue[0]):
            continue
        begin, end = queue
        count = (end - begin) // 4 if end >= begin else 0
        count = min(count, QUEUE_LIMIT)
        if count:
            lay.add(p + "nodes", begin, "u32", count)
    for p in ("a0_", "a1_", "a2_", "a3_", "a4_", "a5_", "referee_"):
        channels = first.get(p + "channels")
        if channels and pointer(channels):
            lay.add(p + "channel0", channels)
    for k in range(2):
        strategy = first.get(f"t{k}_strategy")
        if strategy and pointer(strategy):
            lay.add(f"t{k}_strategy_vtable", strategy + 8)
            # The playbook's running and last play (words 0x23, 0x24).
            lay.add(f"t{k}_strategy_words", strategy + 0x8c, "u32", 2)
            # The playbook's star ([0x32]) and the weights its draw
            # stored (+0xcc..+0xd4, percent).
            lay.add(f"t{k}_star", strategy + 0xc8, "i32", 4)
    context = first.get("context")
    if context and pointer(context):
        lay.add("camera_manager", context + 0x10)
    lay.add("frame_after_pointers", ptr["game"] + 0x128)
    return lay


def third_layout(first, second):
    """Node, channel and camera manager fields (batch three)."""
    lay = Layout()
    for p in ("a0_", "a1_", "a2_", "a3_", "a4_", "a5_", "referee_"):
        nodes = second.get(p + "nodes")
        if nodes is None:
            continue
        if isinstance(nodes, int):
            nodes = [nodes]
        for n, node in enumerate(nodes):
            if pointer(node):
                lay.add(f"{p}node{n}", node + 4, "u32", 4)  # age, started, type, ball
                lay.add(f"{p}node{n}_vtable", node + 0x14)
        if p == "referee_" and nodes and pointer(nodes[0]):
            # FUN_0011aea0's node: +0x1c the count, +0x20.. six ready flags.
            lay.add("referee_wait", nodes[0] + 0x1c, "i32", 7)
        channel = second.get(p + "channel0")
        if channel and pointer(channel):
            lay.add(p + "script_pc", channel + 0x10)
            lay.add(p + "animation", channel + 0x50)
            lay.add(p + "frame_word", channel + 0x5c)
    for k in range(2):
        play = (second.get(f"t{k}_strategy_words") or [0])[0]
        if pointer(play):
            lay.add(f"t{k}_play_head", play, "u32", 4)
    manager = second.get("camera_manager")
    if manager and pointer(manager):
        lay.add("camera_helpers", manager + 0x24, "u32", 4)
        lay.add("camera_cinematic", manager + 0x40)
        lay.add("camera_tracking", manager + 0x44, "i32")
        # Manager +0x4c = (config +0x78 == 0): 1 in the attract demo, whose
        # camera choices (FUN_00303248, FUN_003037b8) differ from a played
        # match's. Camera fields from a demo are demo evidence only.
        lay.add("camera_attract", manager + 0x4c)
        lay.add("camera_delay", manager + 0x50)
        lay.add("camera_state", manager + 0x64)
    lay.add("frame_end", first["game_root"] + 0x128)
    return lay


def tidy(sample):
    """Group the flat names into actors, the referee and the teams."""
    out = {"frame": sample.pop("frame")}
    actors = []
    for i in range(6):
        p = f"a{i}_"
        actor = {k[len(p):]: sample.pop(k) for k in list(sample) if k.startswith(p)}
        queue = []
        for n in range(QUEUE_LIMIT):
            head = actor.pop(f"node{n}", None)
            vtable = actor.pop(f"node{n}_vtable", None)
            if head is None:
                continue
            age, started, kind, _ = head
            queue.append({"type": signed(kind), "vtable": vtable, "age": age & 0xffff,
                          "started": started,
                          "name": BEHAVIOURS.get(vtable, (f"vtable 0x{vtable:x}", 0))[0]})
        actor.pop("nodes", None)
        actor["queue"] = queue
        actors.append(actor)
    out["actors"] = actors
    referee = {k[len("referee_"):]: sample.pop(k) for k in list(sample) if k.startswith("referee_")}
    rq = []
    for n in range(QUEUE_LIMIT):
        head = referee.pop(f"node{n}", None)
        vtable = referee.pop(f"node{n}_vtable", None)
        if head is not None:
            rq.append({"type": signed(head[2]), "vtable": vtable, "age": head[0] & 0xffff,
                       "name": BEHAVIOURS.get(vtable, (f"vtable 0x{vtable:x}", 0))[0]})
    referee.pop("nodes", None)
    referee["queue"] = rq
    out["referee"] = referee
    for k in range(2):
        p = f"t{k}_"
        out[f"team{k}"] = {key[len(p):]: sample.pop(key) for key in list(sample) if key.startswith(p)}
    ring = sample.pop("cm_ring", None)
    state = sample.pop("cm_state", None)
    if ring is not None and state is not None:
        names = ("side", "row", "count", "total", "drain_kind", "basket", "game",
                 "pass_special", "selected", "pushed", "ready", "after")
        out["combo"] = {"slots": [ring[6 * k:6 * k + 6] for k in range(6)],
                        **dict(zip(names, state))}
    meters = []
    for k in range(2):
        p = f"m{k}_"
        meter = {key[len(p):]: sample.pop(key) for key in list(sample) if key.startswith(p)}
        if meter:
            words = meter.pop("words")
            tail = meter.pop("tail")
            meter.update(size=words[0], hold=words[2], gamebreaker=words[3], pocket=words[4],
                         trick=words[5], repeats=words[6], ring=words[7:11],
                         pass_armed=tail[0], paid=tail[1], dunk=tail[2], value_gb=tail[3],
                         passout=tail[4], alley_oop=tail[5])
            meters.append(meter)
    if meters:
        out["meters"] = meters
    stats = {key[len("st_"):]: sample.pop(key) for key in list(sample) if key.startswith("st_")}
    if stats:
        out["stats"] = stats
    bars = {int(key[len("turbo_bar"):]): sample.pop(key) for key in list(sample)
            if key.startswith("turbo_bar")}
    out["turbo"] = [bars.get(k) for k in range(6)]
    out.update(sample)
    return out


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--port", type=int, default=28011)
    parser.add_argument("--seconds", type=float, default=60.0,
                        help="wall-clock length of the capture (at most 1800)")
    parser.add_argument("--wait", type=float, default=0.0,
                        help="seconds to wait for a match before giving up (at most 3600)")
    parser.add_argument("--out", type=Path, required=True, help="JSON lines output")
    args = parser.parse_args()
    if not 0 < args.seconds <= 1800 or not 0 <= args.wait <= 3600:
        parser.error("seconds must be in (0, 1800], wait in [0, 3600]")
    pine = Pine(args.port)
    temporary = args.out.with_suffix(args.out.suffix + ".tmp")
    kept = torn = repeats = 0
    try:
        metadata = {"emulator": pine.string(8), "title": pine.string(11),
                    "serial": pine.string(12), "uuid": pine.string(13),
                    "tool": "pine_ai_capture", "schema": 3,
                    "limitation": "Live reads are not atomic or paused; a sample is three "
                                  "batches bracketed by game+0x128. Missed updates are gaps."}
        if metadata["serial"] != "SLUS-20651":
            raise RuntimeError(f"Wrong executable for this field map: {metadata['serial']}")
        deadline = time.monotonic() + args.wait
        ptr = None
        while ptr is None:
            ptr = discover(pine)
            if ptr is None:
                if time.monotonic() > deadline:
                    raise RuntimeError("No match running")
                time.sleep(0.25)
        with temporary.open("w", encoding="utf-8") as out:
            out.write(json.dumps({"metadata": metadata}) + "\n")
            out.write(json.dumps({"event": "pointers", **ptr}) + "\n")
            end = time.monotonic() + args.seconds
            last_frame = None
            layout = first_layout(ptr)
            while time.monotonic() < end:
                words = pine.words(layout.addresses)
                first = layout.decode(words)
                if first["game_root"] != ptr["game"]:
                    ptr = discover(pine)
                    if ptr is None:
                        out.write(json.dumps({"event": "no_match",
                                              "wall_time_ns": time.time_ns()}) + "\n")
                        time.sleep(0.25)
                        while ptr is None and time.monotonic() < end:
                            ptr = discover(pine)
                            time.sleep(0.25)
                        if ptr is None:
                            break
                    out.write(json.dumps({"event": "pointers", **ptr}) + "\n")
                    layout = first_layout(ptr)
                    last_frame = None
                    continue
                if first["frame"] == last_frame:
                    repeats += 1
                    continue
                if not pointer(ptr["referee_ai"]) and first["frame"] % 8 == 0:
                    # The referee is built after the match object; the
                    # pointers are read again until he has his AIPlayer.
                    again = discover(pine)
                    if again is not None and again["game"] == ptr["game"]:
                        ptr = again
                        layout = first_layout(ptr)
                        out.write(json.dumps({"event": "pointers", **ptr}) + "\n")
                lay2 = second_layout(ptr, first)
                second = lay2.decode(pine.words(lay2.addresses))
                lay3 = third_layout(first, second)
                third = lay3.decode(pine.words(lay3.addresses))
                sample = dict(first)
                sample.update(second)
                sample.update(third)
                sample["torn"] = not (first["frame"] == second["frame_after_pointers"]
                                      == third["frame_end"])
                for key in ("frame_after_pointers", "frame_end", "game_root", "context",
                            "camera_manager"):
                    sample.pop(key, None)
                sample["wall_time_ns"] = time.time_ns()
                torn += sample["torn"]
                kept += 1
                last_frame = first["frame"]
                out.write(json.dumps(tidy(sample), allow_nan=False) + "\n")
            out.write(json.dumps({"coverage": {"samples": kept, "torn": torn,
                                               "repeat_polls": repeats}}) + "\n")
        temporary.replace(args.out)
        print(json.dumps({"samples": kept, "torn": torn, "repeat_polls": repeats}))
    finally:
        pine.socket.close()


if __name__ == "__main__":
    main()
