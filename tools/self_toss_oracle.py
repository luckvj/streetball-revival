"""Offline branch-0 attachment oracle for read-only PINE capture reports.

Uses captured world bone transforms, not the rewrite's animation sampler.
Checks held-ball positions to validate the ordered native offset transform.
Catch points (param7=1) are derived outputs, not observed catches. No script-69
execution or frame-21 target parity is implied by a held-ball match.
"""

import argparse
import json
import math
from collections import Counter
from pathlib import Path


def multiply(a, b):
    x, y, z, w = a
    xx, yy, zz, ww = b
    return [w * xx + x * ww + y * zz - z * yy,
            w * yy - x * zz + y * ww + z * xx,
            w * zz + x * yy - y * xx + z * ww,
            w * ww - x * xx - y * yy - z * zz]


def attachment(sample, mode, skip_radius):
    """FUN_0014dc78 -> 14ddb4 -> 14de48 -> 14dfac, branch 0.

    Offset(.4,0,0) is componentwise scaled first (14de58). For normal
    attachment, radius is added to local Z (14dee0..14df04). Mode 2
    negates local Z (14df40..14df4c). Rotate by evaluated bone quaternion
    with q*v*conj(q), then add evaluated translation (14dfac..14e01c).
    Catch uses param7=1, skipping radius addition, but retaining reflection.
    """
    bone = sample["hand_bones"][str(19 if mode == 1 else 15)]
    vector = [0.4 * sample["skeleton"]["body_scale"][0], 0.0,
              0.0 if skip_radius else sample["ball"]["radius"], 0.0]
    if mode == 2:
        vector[2] = -vector[2]
    q = bone["quaternion"]
    conjugate = [-q[0], -q[1], -q[2], q[3]]
    rotated = multiply(multiply(q, vector), conjugate)
    return [rotated[k] + bone["translation"][k] for k in range(3)]


def ordinary_branch(animation):
    # Conservative exclusion of ids whose frame-dependent dispatcher can
    # select a nonzero branch (14da20..14dc58). Rows296/297 stay branch0.
    special = [(0x84, 0x86), (0xc8, 0xca), (0xcc, 0xd0), (0xd6, 0xd8),
               (0xde, 0xe0), (0x10e, 0x110), (0x122, 0x126), (0x12e, 0x130),
               (0x207, 0x20d), (0x2a6, 0x2a8)]
    return not any(start <= animation < end for start, end in special)


def inspect(report, tolerance):
    rows = []
    omitted = {}
    for index, sample in enumerate(report["samples"]):
        reason = None
        if not all(key in sample for key in ("ball", "player", "skeleton", "hand_bones", "channel0")):
            reason = "missing captured rig"
        elif "radius" not in sample["ball"] or "body_scale" not in sample["skeleton"]:
            reason = "capture predates radius/body scale fields"
        elif sample["ball"]["state"] != 3:
            reason = "ball not held"
        elif sample["player"]["selected_hand"] not in (1, 2):
            reason = "hand mode not 1 or 2"
        elif not ordinary_branch(sample["channel0"]["animation"]):
            reason = "frame-dependent nonzero branch possible"
        if reason:
            omitted[reason] = omitted.get(reason, 0) + 1
            continue
        mode = sample["player"]["selected_hand"]
        predicted = attachment(sample, mode, False)
        observed = sample["ball"]["position"][:3]
        error = math.dist(predicted, observed)
        flags = sample["channel0"]["state"] & 0xc
        direct_hand_flag = flags == (8 if mode == 1 else 4)
        rows.append({"sample": index, "animation": sample["channel0"]["animation"],
                     "frame": sample["channel0"]["frame_word"], "hand_mode": mode,
                     "state": sample["channel0"]["state"],
                     "single_hand_flag_matches_mode": direct_hand_flag,
                     "channel_blending": bool(sample["channel0"]["flags"] & 0x200000),
                     "root_pointers_stable": sample["root_pointers_stable"],
                     "predicted_attachment": predicted, "observed_ball": observed,
                     "error": error, "within_tolerance": error <= tolerance,
                     "derived_catch_point": attachment(sample, mode, True)})
    strict = [row for row in rows if row["single_hand_flag_matches_mode"]
              and not row["channel_blending"] and row["root_pointers_stable"]]
    def summary(items):
        return {"compared": len(items), "within_tolerance": sum(r["within_tolerance"] for r in items),
                "maximum_error": max((r["error"] for r in items), default=None)}
    return {"capture_metadata": report.get("metadata"), "tolerance": tolerance,
            "arithmetic": "Python double precision q*v*conj(q); target executes float32 VU arithmetic",
            "limitations": "Capture reads are not atomic. Non-hand dribble/dual-hand states need other logic. Catch points are derived, not observed. This does not prove script 69 frame21 target parity.",
            "all_ordinary_held": summary(rows), "single_hand_unblended_stable": summary(strict),
            "compared_by_mode": dict(Counter(r["hand_mode"] for r in rows)),
            "matched_by_mode": dict(Counter(r["hand_mode"] for r in rows if r["within_tolerance"])),
            "omitted": omitted, "rows": rows}


def inspect_hierarchy(report, tolerance):
    """Main player bones0..20; head21 needs uncaptured head-scale fields.

    FUN_0019d4c8 uses uniform bodyScale.x for local translations except
    player-shape bones13/17 X and1/5 Z use bodyScale.y when it exceeds X.
    Qworld=Qparent*Qlocal; Pworld=Pparent+Qparent*scaled(P_local)*conj(Qparent).
    Root copies its local translation/quaternion without applying body scale.
    """
    rows = []
    captures = 0
    for index, sample in enumerate(report["samples"]):
        if "rig" not in sample:
            continue
        captures += 1
        rig = sample["rig"]
        sx, sy = sample["skeleton"]["body_scale"][:2]
        positions, quaternions = [], []
        for bone in range(21):
            local = rig[f"{bone}:local_translation"]
            q = rig[f"{bone}:local_quaternion"]
            parent = rig[f"{bone}:parent"]
            if parent == 0xffffffff:
                position, quaternion = local[:3], q
            else:
                scales = [sx, sx, sx]
                if sy > sx:
                    if bone in (13, 17):
                        scales[0] = sy
                    if bone in (1, 5):
                        scales[2] = sy
                pq = quaternions[parent]
                scaled = [local[k] * scales[k] for k in range(3)] + [0]
                rotated = multiply(multiply(pq, scaled), [-pq[0], -pq[1], -pq[2], pq[3]])
                position = [rotated[k] + positions[parent][k] for k in range(3)]
                quaternion = multiply(pq, q)
            positions.append(position)
            quaternions.append(quaternion)
            actual_position = rig[f"{bone}:translation"][:3]
            actual_quaternion = rig[f"{bone}:quaternion"]
            position_error = math.dist(position, actual_position)
            quaternion_error = min(math.dist(quaternion, actual_quaternion),
                                   math.dist([-v for v in quaternion], actual_quaternion))
            rows.append({"sample": index, "bone": bone, "parent": parent,
                         "predicted_translation": position, "observed_translation": actual_position,
                         "position_error": position_error,
                         "predicted_quaternion": quaternion, "observed_quaternion": actual_quaternion,
                         "quaternion_sign_equivalent_error": quaternion_error})
    return {"capture_metadata": report.get("metadata"), "rig_samples": captures,
            "compared_bones": len(rows), "bone_range": "0..20 inclusive",
            "position_matches": sum(r["position_error"] <= tolerance for r in rows),
            "quaternion_matches": sum(r["quaternion_sign_equivalent_error"] <= tolerance for r in rows),
            "maximum_position_error": max((r["position_error"] for r in rows), default=None),
            "maximum_quaternion_error": max((r["quaternion_sign_equivalent_error"] for r in rows), default=None),
            "tolerance": tolerance,
            "limitations": "Reads are not atomic. Player-shape branch is assumed, its virtual-method result was not captured. Head21 requires uncaptured head-scale multipliers; auxiliary22..42 are outside this main-bone evaluator. All nonmatches retained. No script69 frame21 execution is asserted.",
            "rows": rows}


def inspect_script69(report):
    """Compare observed callback target using captured release/catch rig inputs.

    The frame21 catch's local quaternion array includes actor orientation
    in bone0, matching callback53's temporary sampled root quaternion.
    Other hierarchy inputs come from the last prelaunch sample. Frame21's
    captured root-compensation is the callback's scale/rotate root correction.
    This isolates the world bridge from the rewrite's quaternion decoder.
    """
    samples = report["samples"]
    events = []
    for index in range(1, len(samples)):
        before, released = samples[index - 1], samples[index]
        if not all("rig" in s and "player" in s and "channel0" in s for s in (before, released)):
            continue
        pc = released["channel0"]["script_pc"]
        ball = released["ball"]
        actor = int(released["player"]["address"], 16)
        if not (0x44e365 <= pc < 0x44e3da and before["ball"]["state"] == 3
                and ball["state"] == 4 and ball["target_player"] == actor
                and ball["previous_holder"] == actor):
            continue
        caught = None
        for catch_index in range(index + 1, len(samples)):
            candidate = samples[catch_index]
            if candidate.get("player", {}).get("address") != released["player"]["address"]:
                continue
            if (candidate.get("ball", {}).get("state") == 3
                    and candidate["ball"]["holder"] == actor and "rig" in candidate
                    and candidate.get("channel0", {}).get("frame_word") == 21
                    and candidate["channel0"]["animation"] in (296, 297)):
                caught = candidate
                break
        if caught is None:
            continue
        flags = caught["channel0"]["state"] & 0xc
        if flags not in (4, 8):
            continue
        mode = 1 if flags == 8 else 2
        positions, quaternions = [], []
        sx, sy = before["skeleton"]["body_scale"][:2]
        for bone in range(20):
            local = before["rig"][f"{bone}:local_translation"]
            q = caught["rig"][f"{bone}:local_quaternion"]
            parent = before["rig"][f"{bone}:parent"]
            if parent == 0xffffffff:
                position, quaternion = before["skeleton"]["root_translation"][:3], q
            else:
                scales = [sx, sx, sx]
                if sy > sx:
                    if bone in (13, 17):
                        scales[0] = sy
                    if bone in (1, 5):
                        scales[2] = sy
                pq = quaternions[parent]
                vector = [local[k] * scales[k] for k in range(3)] + [0]
                vector = multiply(multiply(pq, vector), [-pq[0], -pq[1], -pq[2], pq[3]])
                position = [positions[parent][k] + vector[k] for k in range(3)]
                quaternion = multiply(pq, q)
            positions.append(position)
            quaternions.append(quaternion)
        hand = 19 if mode == 1 else 15
        q = quaternions[hand]
        vector = [0.4 * sx, 0, released["ball"]["radius"] * (1 if mode == 1 else -1), 0]
        rotated = multiply(multiply(q, vector), [-q[0], -q[1], -q[2], q[3]])
        predicted = [positions[hand][k] + rotated[k] + caught["player"]["root_compensation"][k]
                     - before["player"]["root_compensation"][k] for k in range(3)]
        observed = released["self_toss_target"][:3]
        events.append({"prelaunch_sample": index - 1, "release_sample": index,
                       "catch_sample": catch_index, "animation": released["channel0"]["animation"],
                       "sampled_frame": 21, "mode": mode,
                       "flight_updates": ball["flight_updates"],
                       "first_observed_timer": ball["timer"],
                       "predicted_target": predicted, "observed_callback_target": observed,
                       "target_error": math.dist(predicted, observed)})
    return {"capture_metadata": report.get("metadata"), "events": events,
            "limitations": "Uses observed frame21 local quaternion/root-compensation data, not decoded disc quaternions. Nonatomic capture and head/auxiliary exclusions apply. This validates the callback world-coordinate bridge for observed events only."}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("capture", type=Path)
    parser.add_argument("--out", type=Path)
    parser.add_argument("--tolerance", type=float, default=1e-4)
    parser.add_argument("--hierarchy", action="store_true", help="compare native main-bone hierarchy instead of held attachments")
    parser.add_argument("--script69", action="store_true", help="compare observed script69 callback targets using captured frame21 poses")
    args = parser.parse_args()
    report = json.loads(args.capture.read_text(encoding="utf-8"))
    result = inspect_script69(report) if args.script69 else (inspect_hierarchy if args.hierarchy else inspect)(report, args.tolerance)
    if args.out:
        args.out.write_text(json.dumps(result, indent=2, allow_nan=False) + "\n", encoding="utf-8")
    print(json.dumps({k: v for k, v in result.items() if k != "rows"}, indent=2, allow_nan=False))


if __name__ == "__main__":
    main()
