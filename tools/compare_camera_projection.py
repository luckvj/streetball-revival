"""Retain an original projection capture and compare the compiled production path.

The capture is an original-runtime JSON object with one `observed` record:
scalar, aspect, zoom, horizontal_degrees, m00, m11. No row-mode observations
are synthesized. Rust invokes the same scalar conversion as mode_vertical_fov
and Bevy's actual PerspectiveProjection matrix, with native Y sign conversion.
This checks conversion of an externally supplied scalar, not its generation,
camera controller, target selection, basis, pose, or the whole renderer.

Use --prepare-only before a shared cargo test gate, then --finalize-only after.
Reports must live outside the repository because original captures are local.
"""

import argparse
import hashlib
import json
import math
from pathlib import Path
import shutil
import struct
import subprocess
import os


ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "crates/street-game/src/view.rs"
FIELDS = ("scalar", "aspect", "zoom", "horizontal_degrees", "m00", "m11")
TEST = "view::camera_pose_tests::original_capture_matches_production_scalar_projection"


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def fnv1a(data):
    result = 0xCBF29CE484222325
    for byte in data:
        result = ((result ^ byte) * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return f"{result:016x}"


def paths(report):
    return {key: report.with_suffix(f".{suffix}") for key, suffix in (
        ("bits", "bits.txt"), ("rust", "rust.json"),
        ("manifest", "manifest.json"), ("original", "original.json"))}


def prepare(capture, report):
    retained = paths(report)
    raw = json.loads(capture.read_text(encoding="utf-8-sig"))
    observed = raw["observed"]
    values = [float(observed[field]) for field in FIELDS]
    if not all(math.isfinite(value) for value in values):
        raise ValueError("original observation contains a nonfinite projection value")
    if not all(value > 0 for value in values[:3]):
        raise ValueError("finite positive scalar, aspect and zoom required")
    words = [struct.unpack("<I", struct.pack("<f", value))[0] for value in values]
    report.parent.mkdir(parents=True, exist_ok=True)
    if capture.resolve() != retained["original"].resolve():
        shutil.copyfile(capture, retained["original"])
    retained["bits"].write_text(" ".join(f"{word:08x}" for word in words) + "\n")
    # Remove only the old intermediate result for this explicitly named report.
    retained["rust"].unlink(missing_ok=True)
    manifest = {
        "schema": 1,
        "original_capture_path": str(capture),
        "retained_original_capture_path": str(retained["original"]),
        "original_capture_sha256": sha256(retained["original"]),
        "production_source_path": str(SOURCE),
        "production_source_sha256": sha256(SOURCE),
        "production_source_fnv1a64": fnv1a(SOURCE.read_bytes()),
        "cargo_lock_sha256": sha256(ROOT / "Cargo.lock"),
        "comparison_adapter_sha256": sha256(Path(__file__)),
        "adapter_sha256": sha256(retained["bits"]),
        "eligibility": {
            "input_observations": 1, "eligible_observations": 1,
            "rejected_observations": 0, "rule": "one original finite positive scalar/aspect/zoom capture; no selection by comparison error"
        },
        "expected_bits": dict(zip(FIELDS, [f"{word:08x}" for word in words])),
        "scope": "projection conversion for one supplied native lens scalar, using supplied aspect and zoom; helper identity retained in original capture",
        "limitations": [
            "This projection capture contains no runtime mode-row observation; none is synthesized.",
            "Supplied active/context identities and provenance are retained verbatim; identities are not reclassified.",
            "Capture provenance says one non-atomic PINE batch; guest execution may advance between reads.",
            "Comparison does not execute basis, pose or whole-controller output.",
            ("Original capture retains viewport scales/offsets and renderer-wrapper widescreen flags; full viewport validation is recorded there. Wrapper+0x10 is distinct from projection-object+0x10."
             if "projection.viewport_scale_x" in raw.get("fields", {}) else
             "Historical capture assumes full viewport; viewport scales and renderer-wrapper widescreen flags were not captured."),
            "Native m11 is compared after negating Bevy's positive Y projection coefficient.",
            "Declared tolerance is exact angle and at most four float32 ULPs per matrix coefficient; all errors are retained. This is a numerical acceptance bound, not an EE arithmetic explanation; Bevy adds an atan/tan roundtrip."
        ],
    }
    retained["manifest"].write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    print(f"$env:STREET_CAMERA_PARITY_CAPTURE = '{retained['bits']}'")
    print(f"$env:STREET_CAMERA_PARITY_REPORT = '{retained['rust']}'")
    return retained


def finalize(report):
    retained = paths(report)
    manifest = json.loads(retained["manifest"].read_text(encoding="utf-8"))
    result = json.loads(retained["rust"].read_text(encoding="utf-8"))
    if sha256(SOURCE) != manifest["production_source_sha256"]:
        raise ValueError("production source changed since prepare; reprepare and rerun")
    if result["compiled_view_rs_fnv1a64"] != manifest["production_source_fnv1a64"]:
        raise ValueError("compiled production source differs from prepared source")
    if sha256(retained["original"]) != manifest["original_capture_sha256"]:
        raise ValueError("retained original capture changed")
    if sha256(retained["bits"]) != manifest["adapter_sha256"]:
        raise ValueError("capture adapter changed")
    if result["input_bits"] != list(manifest["expected_bits"].values()):
        raise ValueError("compiled test consumed different capture values")
    manifest["original_capture"] = json.loads(retained["original"].read_text(encoding="utf-8-sig"))
    manifest["comparison"] = result
    manifest["comparison_report_sha256"] = sha256(retained["rust"])
    manifest["eligibility"]["compared_observations"] = result["observations"]
    manifest["eligibility"]["failed_observations"] = int(not result["within_declared_tolerance"])
    report.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"report": str(report), "bit_exact": result["bit_exact"],
                      "within_declared_tolerance": result["within_declared_tolerance"],
                      "metrics": result["metrics"]}, indent=2))
    return result["within_declared_tolerance"]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--capture", type=Path)
    parser.add_argument("--report", type=Path, required=True)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--prepare-only", action="store_true")
    mode.add_argument("--finalize-only", action="store_true")
    args = parser.parse_args()
    report = args.report.resolve()
    if report.is_relative_to(ROOT):
        parser.error("retain captures and reports outside the Git repository")
    if args.finalize_only:
        return 0 if finalize(report) else 1
    if args.capture is None:
        parser.error("--capture is required unless --finalize-only")
    retained = prepare(args.capture.resolve(), report)
    if args.prepare_only:
        return 0
    env = os.environ.copy()
    env["STREET_CAMERA_PARITY_CAPTURE"] = str(retained["bits"])
    env["STREET_CAMERA_PARITY_REPORT"] = str(retained["rust"])
    completed = subprocess.run(["cargo", "test", "-p", "street-game", TEST,
                                "--", "--exact", "--nocapture"], cwd=ROOT, env=env)
    if retained["rust"].exists():
        passed = finalize(report)
        return 0 if completed.returncode == 0 and passed else 1
    return completed.returncode or 1


if __name__ == "__main__":
    raise SystemExit(main())
