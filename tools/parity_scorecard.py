"""Validate and report bounded original-game parity obligations.

Passing rewrite tests are not a whole-game accuracy denominator. This report
keeps static matches, runtime matches, known differences and unknowns separate.
Numerical observations are reported individually, never pooled across units.

Re-pinning a case after its cited files change: a re-pin that leaves the
cited region unchanged replaces the case's one "Last re-pinned <date> after
<pass> (...)" line at the end of its limitations (or adds it); only a re-pin
whose cited region, claim or status changed adds a dated note saying what
changed, before that line. Git history keeps the old notes. The report warns
when a case appends unchanged re-pin notes or an output nears the publish
check's 2 MiB limit.
"""
import argparse
import hashlib
import json
import os
import re
import sys
import tempfile
from pathlib import Path


STATUSES = ("matched", "mismatched", "untested")
DISPOSITIONS = ("active", "deferred", "out_of_scope")
ROOT = Path(__file__).resolve().parents[1]
# tools/publish_check.py fails a file over 2 MiB; warn well before.
SIZE_WARNING = 3 * 1024 * 1024 // 2
REPIN_NOTE = re.compile(r"(?:(?<=\s)|^)(?=Re-pinned\b)")
UNCHANGED_REPIN = re.compile(
    r"(?:^|[;:.]\s*)(?:[Tt]he|[Tt]hat) cited (?:region|regions|code|section|functions|tests|test|items)"
    r" (?:is|are) unchanged\.\s*$")
LAST_REPINNED = re.compile(r"Last re-pinned .*?(?:replace this line\)\.|\.(?=\s|$))")


def required_text(item, key, context):
    value = item.get(key)
    if not isinstance(value, str) or not value.strip():
        raise ValueError(f"{context}: {key} must be nonempty text")
    return value


def local_path(path):
    """Evidence kept off the repository is named relative to `$EVIDENCE`
    (the STREET_EVIDENCE folder, default the system temp folder) or is the
    disc image `$STREET_ISO`, so no machine's own paths are committed."""
    if path == "$STREET_ISO":
        return Path(os.environ.get("STREET_ISO", "$STREET_ISO"))
    if path.startswith("$EVIDENCE/"):
        base = os.environ.get("STREET_EVIDENCE", tempfile.gettempdir())
        return Path(base) / path[len("$EVIDENCE/"):].replace("\\", "/")
    return Path(path)


def evidence_exists(path, root):
    candidate = local_path(path)
    return (candidate if candidate.is_absolute() else root / candidate).is_file()


def reject_self_reference(reference, inventory, root, identity):
    candidate = local_path(reference)
    candidate = candidate if candidate.is_absolute() else root / candidate
    if candidate.resolve() == inventory.resolve():
        raise ValueError(f"{identity}: inventory cannot serve as its own evidence")


def evidence_digest(path, root, cache):
    candidate = local_path(path)
    candidate = candidate if candidate.is_absolute() else root / candidate
    key = str(candidate.resolve())
    if key not in cache:
        if not candidate.is_file():
            cache[key] = None
        else:
            digest = hashlib.sha256()
            with candidate.open("rb") as source:
                for chunk in iter(lambda: source.read(1024 * 1024), b""):
                    digest.update(chunk)
            cache[key] = digest.hexdigest()
    return cache[key]


def pin_evidence(directory, root=ROOT):
    """Explicitly stamp a reviewed inventory, never as a side effect of reporting."""
    cache = {}
    for path in sorted(directory.glob("*.json")):
        if path.name == "results.json":
            continue
        source = json.loads(path.read_text(encoding="utf-8-sig"))
        for case in source.get("cases", []):
            for reference in case.get("evidence", []):
                digest = evidence_digest(reference["path"], root, cache)
                if digest:
                    reference["sha256"] = digest
        for metric in source.get("measurements", []):
            digest = evidence_digest(metric["evidence_path"], root, cache)
            if digest:
                metric["evidence_sha256"] = digest
        path.write_text(json.dumps(source, indent=2, ensure_ascii=False) + "\n",
                        encoding="utf-8", newline="\n")


def collect(directory, root=ROOT):
    areas, seen, measurements_seen, digests = [], set(), set(), {}
    for path in sorted(directory.glob("*.json")):
        if path.name == "results.json":
            continue
        source = json.loads(path.read_text(encoding="utf-8-sig"))
        area = required_text(source, "area", str(path))
        cases = source.get("cases")
        if not isinstance(cases, list) or not cases:
            raise ValueError(f"{path}: cases must be a nonempty list")
        reviewed = []
        for case in cases:
            identity = required_text(case, "id", area)
            if identity in seen:
                raise ValueError(f"duplicate case id: {identity}")
            seen.add(identity)
            for key in ("scenario", "tolerance", "limitations", "next_probe"):
                required_text(case, key, identity)
            status = case.get("status")
            kind = case.get("evidence_kind")
            disposition = case.get("disposition", "active")
            match_type = case.get("match_type", "unknown")
            if status not in STATUSES or kind not in ("runtime", "static", "none"):
                raise ValueError(f"{identity}: invalid status or evidence kind")
            if disposition not in DISPOSITIONS:
                raise ValueError(f"{identity}: invalid disposition")
            if match_type not in ("exact", "numerical", "structural", "unknown"):
                raise ValueError(f"{identity}: invalid match type")
            if status == "matched" and match_type == "unknown":
                raise ValueError(f"{identity}: a match needs an explicit acceptance type")
            evidence = case.get("evidence", [])
            if not isinstance(evidence, list):
                raise ValueError(f"{identity}: evidence must be a list")
            if status != "untested" and (kind == "none" or not evidence):
                raise ValueError(f"{identity}: a conclusion needs original evidence")
            missing, stale = [], []
            for reference in evidence:
                for key in ("path", "locator", "note"):
                    required_text(reference, key, identity)
                reject_self_reference(reference["path"], path, root, identity)
                if not evidence_exists(reference["path"], root):
                    missing.append(reference["path"])
                elif reference.get("sha256") and reference["sha256"] != evidence_digest(reference["path"], root, digests):
                    stale.append(reference["path"])
            # A source conclusion whose evidence is unavailable is not a
            # currently verifiable conclusion on this machine.
            effective = "untested" if missing or stale else status
            reviewed.append({**case, "disposition": disposition,
                             "match_type": match_type,
                             "effective_status": effective,
                             "missing_evidence": missing, "changed_evidence": stale})
        observed = []
        for measurement in source.get("measurements", []):
            identity = required_text(measurement, "id", area)
            if identity in measurements_seen:
                raise ValueError(f"duplicate measurement id: {identity}")
            measurements_seen.add(identity)
            for key in ("label", "unit", "tolerance", "evidence_path", "limitations"):
                required_text(measurement, key, identity)
            matched, total = measurement.get("matched"), measurement.get("total")
            if (type(matched) is not int or type(total) is not int
                    or total <= 0 or not 0 <= matched <= total):
                raise ValueError(f"{identity}: require 0 <= matched <= positive total")
            if type(measurement.get("production_comparison")) is not bool:
                raise ValueError(f"{identity}: production_comparison must be boolean")
            reject_self_reference(measurement["evidence_path"], path, root, identity)
            observed.append({**measurement,
                             "evidence_available": evidence_exists(measurement["evidence_path"], root),
                             "evidence_changed": bool(measurement.get("evidence_sha256")) and measurement["evidence_sha256"] != evidence_digest(measurement["evidence_path"], root, digests),
                             "agreement_percent": round(100 * matched / total, 4)})
        areas.append({"area": area, "source": path.name,
                      "cases": reviewed, "measurements": observed})
    if not areas:
        raise ValueError(f"no area inventories in {directory}")
    active = [case for area in areas for case in area["cases"]
              if case["disposition"] == "active"]
    counts = {status: sum(case["effective_status"] == status for case in active)
              for status in STATUSES}
    return {"whole_game_accuracy_percent": None,
            "scope": "Version 1 selected obligations; inventory completeness is not certified.",
            "active_counts": counts, "active_total": len(active),
            "matched_acceptance": {kind: sum(case["effective_status"] == "matched"
                                              and case["match_type"] == kind for case in active)
                                   for kind in ("exact", "numerical", "structural")},
            "deferred": sum(case["disposition"] == "deferred" for area in areas for case in area["cases"]),
            "out_of_scope": sum(case["disposition"] == "out_of_scope" for area in areas for case in area["cases"]),
            "areas": areas}


def cell(value):
    return str(value).replace("|", "\\|").replace("\n", " ")


def render(report):
    counts = report["active_counts"]
    acceptance = report["matched_acceptance"]
    lines = ["# Original-game parity scorecard", "",
             "Whole-game accuracy: **not yet measurable**. This is a version-1 inventory of selected",
             "behavior obligations, not a representative or complete sample of the entire game.",
             "Feature coverage and passing rewrite tests do not establish original-game parity.", "",
             f"Active inventory: **{report['active_total']} obligations**; "
             f"**{counts['matched']} matched**, **{counts['mismatched']} known differences**, "
             f"**{counts['untested']} untested/unresolved**.",
             f"Matched acceptance: {acceptance['exact']} exact, {acceptance['numerical']} numerical, "
             f"{acceptance['structural']} structural.",
             f"Separately tracked: {report['deferred']} deferred; {report['out_of_scope']} outside original-parity scope.", "",
             "A match is limited to its stated scenario, evidence and tolerance. Static source agreement",
             "is labeled separately from original-runtime agreement. Missing evidence downgrades a",
             "recorded conclusion to untested on the current machine. Changed pinned evidence also requires",
             "review before its conclusion is counted. Unknowns are never counted as matches.", "",
             "Exact matches require equality of the stated outputs; numerical matches meet a declared",
             "error bound; structural matches establish a traced control/data relationship without",
             "a paired-output equality result. None alone proves an entire subsystem accurate.", "",
             "## Obligation inventory", "",
             "| Area | Static matches | Runtime matches | Known differences | Untested | Deferred/excluded |",
             "|---|---:|---:|---:|---:|---:|"]
    for area in report["areas"]:
        active = [c for c in area["cases"] if c["disposition"] == "active"]
        matches = [c for c in active if c["effective_status"] == "matched"]
        lines.append(f"| {cell(area['area'])} | "
                     f"{sum(c['evidence_kind'] == 'static' for c in matches)} | "
                     f"{sum(c['evidence_kind'] == 'runtime' for c in matches)} | "
                     f"{sum(c['effective_status'] == 'mismatched' for c in active)} | "
                     f"{sum(c['effective_status'] == 'untested' for c in active)} | "
                     f"{len(area['cases']) - len(active)} |")
    lines += ["", "## Bounded numerical comparisons", "",
              "Each denominator below describes one component or observation set. They cannot be pooled",
              "into a game-wide percentage. An offline oracle is not a production end-to-end comparison.", "",
              "| Comparison | Matches / observations | Agreement | Unit | Production component? | Evidence available? |",
              "|---|---:|---:|---|---|---|"]
    for area in report["areas"]:
        for metric in area["measurements"]:
            lines.append(f"| {cell(metric['label'])} | {metric['matched']}/{metric['total']} | "
                         f"{metric['agreement_percent']:.4f}% | {cell(metric['unit'])} | "
                         f"{'yes' if metric['production_comparison'] else 'no (oracle/reconstruction)'} | "
                         f"{'changed; review required' if metric['evidence_changed'] else 'yes' if metric['evidence_available'] else 'no; historical result only'} |")
    lines += ["", "Exact equality and tolerance-based agreement are different acceptance criteria.",
              "A zero exact-match count can coexist with very small numerical errors; read the bounds.", ""]
    for area in report["areas"]:
        for metric in area["measurements"]:
            lines += [f"- **{metric['id']}**: {metric['label']}",
                      f"  - Acceptance: {metric['tolerance']}",
                      f"  - Limits: {metric['limitations']}",
                      f"  - Evidence: `{metric['evidence_path']}`"]
    lines += ["", "## Cases and next probes", ""]
    for area in report["areas"]:
        lines += [f"### {area['area']}", ""]
        for case in area["cases"]:
            lines += [f"- **{case['id']}** — {case['effective_status']} / {case['match_type']} / {case['evidence_kind']} / "
                      f"{case['disposition']}: {case['scenario']}",
                      f"  - Acceptance: {case['tolerance']}",
                      f"  - Limits: {case['limitations']}",
                      f"  - Next: {case['next_probe']}"]
            for evidence in case["evidence"]:
                lines.append(f"  - Evidence: `{evidence['path']}` — {evidence['locator']}; {evidence['note']}")
            if case["missing_evidence"]:
                lines.append("  - Unavailable: " + ", ".join(case["missing_evidence"]))
            if case["changed_evidence"]:
                lines.append("  - Changed since review: " + ", ".join(case["changed_evidence"]))
        lines.append("")
    lines += ["## How this becomes an accuracy assessment", "",
              "1. Complete and review the scenario inventory against the original's subsystems, modes,",
              "   input boundaries and state transitions before calling it whole-game coverage.",
              "2. Run paired original/rewrite scenarios with the same initial state and input schedule.",
              "   Record exact equality separately from numerical agreement and explain every exclusion.",
              "3. Preserve original captures, rewrite outputs, tolerances and reproducible comparison commands.",
              "   Compare production code paths, not just disconnected arithmetic kernels.",
              "4. Publish per-area agreement and untested coverage. Fix any global weighting in advance;",
              "   never let thousands of easy asset/lookup checks outweigh unresolved gameplay behavior.", ""]
    return "\n".join(lines)


def repin_warnings(report):
    """Cases that append re-pin notes instead of replacing their one line."""
    warnings = []
    for area in report["areas"]:
        for case in area["cases"]:
            text = case["limitations"]
            notes = REPIN_NOTE.split(LAST_REPINNED.sub("", text))[1:]
            unchanged = sum(bool(UNCHANGED_REPIN.search(note.strip())) for note in notes)
            if unchanged:
                warnings.append(f"{case['id']}: {unchanged} appended re-pin note(s) with the cited "
                                "region unchanged; replace the case's last re-pinned line instead")
            if len(LAST_REPINNED.findall(text)) > 1:
                warnings.append(f"{case['id']}: more than one last re-pinned line")
    return warnings


def size_warnings(paths, limit=SIZE_WARNING):
    return [f"{path}: {path.stat().st_size} bytes, over {limit}; the publish check fails a file "
            "over 2 MiB, so trim the parity notes" for path in paths
            if path and path.is_file() and path.stat().st_size > limit]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--directory", type=Path, default=ROOT / "docs" / "parity")
    parser.add_argument("--out", type=Path)
    parser.add_argument("--json-out", type=Path)
    parser.add_argument("--pin-evidence", action="store_true",
                        help="stamp evidence digests only after reviewing the source conclusions")
    args = parser.parse_args()
    try:
        # Validate before writing a reviewed stamp, and again after stamping.
        report = collect(args.directory)
        if args.pin_evidence:
            pin_evidence(args.directory)
            report = collect(args.directory)
    except (ValueError, KeyError, TypeError) as error:
        parser.error(str(error))
    if args.out:
        args.out.write_text(render(report), encoding="utf-8", newline="\n")
    if args.json_out:
        args.json_out.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8", newline="\n")
    print(json.dumps({k: v for k, v in report.items() if k != "areas"}, indent=2))
    for warning in repin_warnings(report) + size_warnings([args.out, args.json_out]):
        print(f"warning: {warning}", file=sys.stderr)


if __name__ == "__main__":
    main()
