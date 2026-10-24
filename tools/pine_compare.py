"""Compare captured EE values with recovered arithmetic, without runtime writes.

Input is a pine_probe.py report. Live samples can skip updates or mix fields
from different updates. Every eligible comparison is retained, including
nonmatches; these kernel checks do not establish whole-game parity.
"""
import argparse
import json
import math
from pathlib import Path

from camera_oracle import add, div, mul, positioned_lens, sub


def compare(report):
    flight = []
    lens = []
    targets = []
    previous = None
    for index, sample in enumerate(report["samples"]):
        ball = sample.get("ball", {})
        if (ball.get("state") == 4 and ball.get("gravity_enabled") == 1
                and ball.get("holder") == 0 and ball.get("bounce") == 0
                and ball.get("timer", 0) > 5):
            # Native direct-flight timeout includes fps/12 trailing updates.
            fps = sample["globals"]["fps"]
            remaining = ball["timer"] - fps // 12
            if remaining > 0:
                predicted = [ball["position"][axis] + ball["velocity"][axis] * remaining
                             for axis in range(3)]
                predicted[1] -= ball["gravity"] * remaining * remaining / 2
                flight.append({"sample": index, "remaining_updates": remaining,
                               "predicted": predicted, "target": ball["target_position"][:3],
                               "error": math.dist(predicted, ball["target_position"][:3])})
        camera = sample.get("active_camera", {})
        if camera.get("vtable") != 0x498950 or "target_lens" not in camera:
            previous = None
            continue
        globals_ = sample["globals"]
        damping = globals_["lens_damping"] if globals_["lens_damping_enabled"] else .25
        target = positioned_lens(camera["lens_tangent"], camera["distance_scale"],
                                 camera["eye"], camera["focus_e0"], globals_["fps"], damping)
        targets.append({"sample": index, "predicted": target["target"],
                        "observed": camera["target_lens"],
                        "error": abs(target["target"] - camera["target_lens"])})
        signature = (camera["address"], camera["lens_tangent"], camera["target_lens"])
        if previous is not None and signature != previous[0]:
            if signature[0] == previous[0][0]:
                # Use the observed new target to isolate the easing operation.
                delta = mul(sub(camera["target_lens"], previous[0][1]), .25)
                limit = div(60., globals_["fps"])
                correction = max(-limit, min(limit, mul(delta, damping)))
                predicted = add(previous[0][1], sub(delta, correction))
                lens.append({"before_sample": previous[1], "after_sample": index,
                             "predicted": predicted, "observed": camera["lens_tangent"],
                             "error": abs(predicted - camera["lens_tangent"]),
                             "exact": predicted == camera["lens_tangent"]})
        previous = (signature, index)

    def summary(rows):
        return {"compared": len(rows), "maximum_error": max((r["error"] for r in rows), default=None)}

    return {"metadata": report.get("metadata"),
            "limitations": "Unpaused reads may mix fields or skip ticks. Flight checks use double arithmetic; lens easing rounds each operation to float32. Matches validate these arithmetic kernels only; no script-69 execution is implied.",
            "flight": summary(flight), "lens_target": summary(targets),
            "lens_easing": {**summary(lens), "exact": sum(r["exact"] for r in lens)},
            "rows": {"flight": flight, "lens_target": targets, "lens_easing": lens}}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("capture", type=Path)
    parser.add_argument("--out", type=Path)
    args = parser.parse_args()
    result = compare(json.loads(args.capture.read_text(encoding="utf-8")))
    if args.out:
        temporary = args.out.with_suffix(args.out.suffix + ".tmp")
        temporary.write_text(json.dumps(result, indent=2, allow_nan=False) + "\n", encoding="utf-8")
        temporary.replace(args.out)
    print(json.dumps({key: value for key, value in result.items() if key != "rows"}, indent=2))


if __name__ == "__main__":
    main()
