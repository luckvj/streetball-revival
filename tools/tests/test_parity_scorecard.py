"""Guard against misleading counts and stale evidence in the parity ledger."""
import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import parity_scorecard as scorecard


class ScorecardTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.inventory = self.root / "inventory"
        self.inventory.mkdir()
        (self.root / "evidence.txt").write_text("reviewed original/rewrite fixture", encoding="utf-8")

    def case(self, identity="fixture.match", **overrides):
        return {"id": identity, "scenario": "A bounded fixture output",
                "status": "matched", "match_type": "exact", "evidence_kind": "static",
                "evidence": [{"path": "evidence.txt", "locator": "fixture",
                              "note": "Evidence for scorecard logic only"}],
                "tolerance": "Exact fixture equality", "limitations": "Not game evidence",
                "next_probe": "Review when source changes", **overrides}

    def write(self, cases, measurements=None):
        (self.inventory / "fixture.json").write_text(json.dumps(
            {"area": "Fixture", "cases": cases, "measurements": measurements or []}), encoding="utf-8")

    def test_unknown_and_deferred_are_never_active_matches(self):
        self.write([self.case(), self.case("fixture.unknown", status="untested", match_type="unknown",
                                         evidence_kind="none", evidence=[]),
                    self.case("fixture.deferred", disposition="deferred")])
        report = scorecard.collect(self.inventory, self.root)
        self.assertEqual(report["active_counts"], {"matched": 1, "mismatched": 0, "untested": 1})
        self.assertEqual(report["deferred"], 1)
        self.assertIsNone(report["whole_game_accuracy_percent"])

    def test_missing_or_changed_evidence_requires_review(self):
        self.write([self.case()])
        scorecard.pin_evidence(self.inventory, self.root)
        (self.root / "evidence.txt").write_text("changed output", encoding="utf-8")
        changed = scorecard.collect(self.inventory, self.root)
        self.assertEqual(changed["active_counts"]["untested"], 1)
        self.assertEqual(changed["areas"][0]["cases"][0]["changed_evidence"], ["evidence.txt"])
        (self.root / "evidence.txt").unlink()
        missing = scorecard.collect(self.inventory, self.root)
        self.assertEqual(missing["active_counts"]["matched"], 0)
        self.assertEqual(missing["areas"][0]["cases"][0]["missing_evidence"], ["evidence.txt"])

    def test_duplicate_obligation_is_rejected(self):
        self.write([self.case(), self.case()])
        with self.assertRaisesRegex(ValueError, "duplicate case id"):
            scorecard.collect(self.inventory, self.root)

    def test_inventory_is_not_its_own_original_evidence(self):
        self.write([self.case(evidence=[{"path": "inventory/fixture.json",
                                        "locator": "self", "note": "Unsupported self-reference"}])])
        with self.assertRaisesRegex(ValueError, "own evidence"):
            scorecard.collect(self.inventory, self.root)

    def test_impossible_measurement_cannot_be_reported_as_accuracy(self):
        metric = {"id": "fixture.metric", "label": "Fixture", "matched": 2, "total": 1,
                  "unit": "outputs", "tolerance": "exact", "evidence_path": "evidence.txt",
                  "limitations": "No game-wide denominator", "production_comparison": False}
        self.write([self.case()], [metric])
        with self.assertRaisesRegex(ValueError, "positive total"):
            scorecard.collect(self.inventory, self.root)

    def test_unchanged_repins_replace_one_line_rather_than_append(self):
        line = "Last re-pinned 2026-10-18 after the fixture pass (re-pins that leave the cited region unchanged replace this line)."
        changed = "Re-pinned 2026-10-17 after the shot pass: the cited test now holds the gate; the rest of the cited region is unchanged."
        appended = "Re-pinned 2026-10-18 after the fixture pass: TRACE.md gained a section; the cited region is unchanged."
        self.write([self.case(limitations=f"Not game evidence. {changed} {line}"),
                    self.case("fixture.appended", limitations=f"Not game evidence. {appended} {line}"),
                    self.case("fixture.twice", limitations=f"Not game evidence. {line} {line}")])
        warnings = scorecard.repin_warnings(scorecard.collect(self.inventory, self.root))
        self.assertEqual(len(warnings), 2)
        self.assertTrue(warnings[0].startswith("fixture.appended: 1 appended"))
        self.assertTrue(warnings[1].startswith("fixture.twice: more than one"))

    def test_outputs_near_the_publish_limit_are_reported(self):
        output = self.root / "results.json"
        output.write_bytes(b"x" * 10)
        self.assertEqual(scorecard.size_warnings([output, None], limit=10), [])
        self.assertEqual(len(scorecard.size_warnings([output], limit=9)), 1)
        self.assertLess(scorecard.SIZE_WARNING, 2 * 1024 * 1024)


if __name__ == "__main__":
    unittest.main()
