"""Summarise a pine_ai_capture.py capture of the original's computer players.

Reads the JSON lines a capture wrote and reports, offline:
- coverage: updates seen, gaps (missed updates) and torn samples;
- the shared generator (`FUN_003a16b8`): draws between consecutive captured
  updates, found by stepping the six-word state forward (bounded search);
- the phase timeline (game+0xf4) with update numbers;
- the opening: the referee's wait (his "referee wait" node, `FUN_0011aea0`:
  +0x1c the count, +0x20.. the six ready flags), the toss launch (ball state
  7), the tip-off timings and winner (`DAT_003d6578`, `DAT_003d6570`) and the
  tip (the first pass and its receiver);
- each restart (phase 6): its length, the ready flags' rise (game+0x404..),
  the behaviours queued and the inbound pass;
- the behaviours each player runs, by phase and by whether his side has the
  ball, and the pad bits his AIPlayer presses;
- the passes: the updates from the press (ball state 4) to the release (the
  flight's progress leaving 0, the pass script's callback 38), and how long
  the receiver's catch (actor+0xa4 bit 9) holds after the catch.

Nothing here reads the running game. Draw counts between non-adjacent
updates cover every update in the gap.
"""
import argparse
import json
from collections import Counter, defaultdict
from pathlib import Path

from pine_ai_capture import BEHAVIOURS

PAD_BITS = {
    0x10: "juke", 0x80: "shoot", 0x100: "pass", 0x200: "steal", 0x800: "kick",
    0x1000: "block", 0x2000: "turbo", 0x4000: "L1", 0x8000: "L2", 0x10000: "R1",
    0x20000: "R2", 0x40000: "two turbos", 0x100000: "rebound", 0x200000: "toss",
    0x400000: "stance", 0x800000: "juke held", 0x2000000: "strategy",
    0x4000000: "step in", 0x8000000: "pocket", 0x10000000: "cancel",
}


def rng_next(state):
    """FUN_003a16b8 on the six words at 0x4309f0 (index 0 = 0x4309f0)."""
    s = list(state)
    carry = 0
    for i in range(4, -1, -1):
        total = s[i] + s[i + 1] + carry
        s[i] = total & 0xffffffff
        carry = total >> 32
    for i in range(5, -1, -1):
        s[i] = (s[i] + 1) & 0xffffffff
        if s[i]:
            break
    return s


def draws_between(before, after, limit=4096):
    """Draws taking `before` to `after`, or None past `limit`."""
    state = list(before)
    for n in range(limit + 1):
        if state == list(after):
            return n
        state = rng_next(state)
    return None


def load(path):
    samples, events, meta = [], [], None
    with open(path, encoding="utf-8") as f:
        for line in f:
            item = json.loads(line)
            if "metadata" in item:
                meta = item["metadata"]
            elif "frame" in item:
                samples.append(item)
            else:
                events.append(item)
    return meta, samples, events


def segments(samples):
    """Split at a frame counter that goes backwards (a new match)."""
    out, cur = [], []
    for s in samples:
        if cur and s["frame"] < cur[-1]["frame"]:
            out.append(cur)
            cur = []
        cur.append(s)
    if cur:
        out.append(cur)
    return out


def front(actor):
    """The front behaviour's name, from its vtable (older captures named
    fewer vtables when they were taken)."""
    q = actor.get("queue") or []
    if not q:
        return "-"
    return BEHAVIOURS.get(q[0]["vtable"], (q[0]["name"], 0))[0]


def pad_names(word):
    return [name for bit, name in PAD_BITS.items() if word & bit]


def analyse(samples):
    report = {}
    clean = [s for s in samples if not s["torn"]]
    frames = [s["frame"] for s in samples]
    gaps = Counter(b - a for a, b in zip(frames, frames[1:]))
    report["coverage"] = {
        "samples": len(samples), "torn": len(samples) - len(clean),
        "first_frame": frames[0], "last_frame": frames[-1],
        "updates_spanned": frames[-1] - frames[0] + 1,
        "step_histogram": {str(k): v for k, v in sorted(gaps.items())},
    }
    # Draws per update, only across adjacent clean updates.
    per_update = []
    for a, b in zip(clean, clean[1:]):
        if b["frame"] - a["frame"] == 1:
            n = draws_between(a["rng"], b["rng"])
            per_update.append((b["frame"], b["phase"], n))
    by_phase = defaultdict(Counter)
    for _, phase, n in per_update:
        by_phase[phase][n if n is not None else "over"] += 1
    report["draws_per_update"] = {
        "adjacent_pairs": len(per_update),
        "by_phase": {str(p): {str(k): v for k, v in sorted(c.items(), key=lambda kv: str(kv[0]))}
                     for p, c in sorted(by_phase.items())},
    }
    report["_draws"] = per_update
    # Phase timeline.
    timeline = []
    for s in samples:
        if not timeline or timeline[-1]["phase"] != s["phase"]:
            timeline.append({"phase": s["phase"], "frame": s["frame"],
                             "possession": s["possession"]})
    report["phase_timeline"] = timeline
    report["opening"] = opening(samples)
    report["restarts"] = restarts(samples)
    report["behaviours"] = behaviours(samples)
    report["passes"] = passes(samples)
    return report


def opening(samples):
    out = {}
    one = [s for s in samples if s["phase"] == 1]
    if not one:
        return {"seen": False}
    out["seen"] = True
    out["phase1_first_frame"] = one[0]["frame"]
    out["phase1_last_frame"] = one[-1]["frame"]
    waits = []
    for s in one:
        rq = s["referee"].get("queue") or []
        w = s["referee"].get("wait")
        if rq and rq[0]["vtable"] == 0x4392f0 and w:
            waits.append({"frame": s["frame"], "count": w[0], "ready": w[1:]})
    if waits:
        all_ready = next((w for w in waits if all(w["ready"])), None)
        out["referee_wait_first"] = waits[0]
        out["all_ready_first_frame"] = all_ready["frame"] if all_ready else None
        zero = next((w for w in waits if w["count"] == 0), None)
        out["referee_count_zero_frame"] = zero["frame"] if zero else None
        # Each player's ready flag rise.
        rise = [None] * 6
        for w in waits:
            for k in range(6):
                if rise[k] is None and w["ready"][k]:
                    rise[k] = w["frame"]
        out["ready_rise_frames"] = rise
    launch = next((s for s in samples if s["ball_state"] == 7), None)
    if launch:
        out["launch_frame"] = launch["frame"]
        out["launch_ball"] = {"position": launch["ball_position"],
                              "velocity": launch["ball_velocity"]}
        prev = [s for s in samples if s["frame"] < launch["frame"]]
        if prev:
            out["before_launch"] = {"frame": prev[-1]["frame"], "ball_state": prev[-1]["ball_state"],
                                    "ball_position": prev[-1]["ball_position"],
                                    "rng": prev[-1]["rng"]}
    decided = next((s for s in samples if s["tip_winner"] != -1 and s["phase"] == 1), None)
    if decided:
        prev = [s for s in samples if s["frame"] < decided["frame"]]
        out["tip"] = {"decided_frame": decided["frame"], "timing": decided["tip_timing"],
                      "winner_actor": decided["tip_winner"],
                      "ball_at_decision": decided["ball_position"],
                      "previous_frame": prev[-1]["frame"] if prev else None,
                      "rng_before": prev[-1]["rng"] if prev else None,
                      "rng_after": decided["rng"]}
        if prev:
            out["tip"]["timing_before"] = prev[-1]["tip_timing"]
    tipped = next((s for s in samples if s["phase"] == 1 and s["ball_state"] == 4), None)
    if tipped:
        out["tip_pass"] = {"frame": tipped["frame"], "ball_position": tipped["ball_position"],
                           "target_player": tipped["ball_target_player"],
                           "holder": tipped["ball_holder"]}
    out["phase1_queues"] = Counter(
        f"{i}:{front(a)}" for s in one for i, a in enumerate(s["actors"]))
    return out


def restarts(samples):
    out = []
    i = 0
    while i < len(samples):
        if samples[i]["phase"] != 6:
            i += 1
            continue
        j = i
        while j < len(samples) and samples[j]["phase"] == 6:
            j += 1
        seg = samples[i:j]
        rise = [None] * 6
        for s in seg:
            for k in range(6):
                if rise[k] is None and s["ready"][k]:
                    rise[k] = s["frame"]
        fronts = defaultdict(list)
        for s in seg:
            for k, a in enumerate(s["actors"]):
                name = front(a)
                if not fronts[k] or fronts[k][-1][0] != name:
                    fronts[k].append((name, s["frame"]))
        passes = [s for s in seg if s["ball_state"] == 4]
        out.append({
            "first_frame": seg[0]["frame"], "last_frame": seg[-1]["frame"],
            "possession": seg[0]["possession"],
            "updates": seg[-1]["frame"] - seg[0]["frame"] + 1,
            "ready_rise_frames": rise,
            "all_ready_frame": next((s["frame"] for s in seg if all(s["ready"])), None),
            "pass_frame": passes[0]["frame"] if passes else None,
            "receiver": passes[0]["ball_target_player"] if passes else None,
            "fronts": {str(k): v for k, v in fronts.items()},
            "start_positions": [a["position"] for a in seg[0]["actors"]],
            "end_positions": [a["position"] for a in seg[-1]["actors"]],
            "next_phase": samples[j]["phase"] if j < len(samples) else None,
        })
        i = j
    return out


def passes(samples):
    """Each pass's release and catch: the ball stays in the passer's hands
    (state 4, progress 0) until its animation lets go."""
    by = {s["frame"]: s for s in samples if not s["torn"]}
    rows = []
    for a, b in zip(samples, samples[1:]):
        if a["torn"] or b["torn"] or b["frame"] != a["frame"] + 1:
            continue
        if a["ball_state"] == 4 or b["ball_state"] != 4:
            continue
        f = b["frame"]
        while f in by and by[f]["ball_state"] == 4 and by[f]["ball_progress"] == 0:
            f += 1
        release = f - b["frame"] if f in by and by[f]["ball_state"] == 4 else None
        hold = None
        if release is not None:
            while f in by and by[f]["ball_state"] == 4:
                f += 1
            s = by.get(f)
            if s and s["ball_state"] == 3:
                r = next((i for i, x in enumerate(s["actors"])
                          if x["animation_state"] & 0x200), None)
                if r is not None:
                    g = f
                    while g in by and by[g]["actors"][r]["animation_state"] & 0x200:
                        g += 1
                    hold = g - f
        rows.append({"press_frame": b["frame"], "phase": a["phase"],
                     "release_after": release, "catch_hold": hold})

    def count(key, phases):
        return dict(sorted(Counter(
            r[key] for r in rows if r["phase"] in phases and r[key] is not None).items()))
    return {"passes": rows,
            "inbound_release": count("release_after", (6,)),
            "live_release": count("release_after", (4, 5)),
            "catch_hold": count("catch_hold", (4, 5, 6))}


def behaviours(samples):
    by = defaultdict(Counter)
    pads = defaultdict(Counter)
    starts = Counter()
    for s in samples:
        if s["torn"]:
            continue
        for i, a in enumerate(s["actors"]):
            side = "offence" if a.get("team") == s["possession"] else "defence"
            by[f"{s['phase']}:{side}"][front(a)] += 1
            for name in pad_names(a.get("pad", [0])[0] if a.get("pad") else 0):
                pads[f"{s['phase']}:{side}"][name] += 1
            q = a.get("queue") or []
            if q and q[0]["age"] == 1:
                starts[q[0]["name"]] += 1
    return {"front_by_phase_side": {k: dict(v.most_common()) for k, v in sorted(by.items())},
            "pad_bits_by_phase_side": {k: dict(v.most_common()) for k, v in sorted(pads.items())},
            "front_starts": dict(starts.most_common())}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("capture", type=Path)
    parser.add_argument("--out", type=Path)
    args = parser.parse_args()
    meta, samples, events = load(args.capture)
    if not samples:
        raise SystemExit("no samples")
    reports = []
    for seg in segments(samples):
        r = analyse(seg)
        r.pop("_draws")
        reports.append(r)
    text = json.dumps({"metadata": meta, "events": events, "segments": reports}, indent=1)
    if args.out:
        args.out.write_text(text + "\n", encoding="utf-8")
    else:
        print(text)


if __name__ == "__main__":
    main()
