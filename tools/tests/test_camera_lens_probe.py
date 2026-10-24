"""Mocked probe integrity checks; these fixtures are not native-game evidence."""
import contextlib
import io
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import capture_camera_lens as probe


CONTEXT = 0x100000
MANAGER = 0x110000
ACTIVE = 0x120000
WRAPPER = 0x130000
PROJECTION = 0x140000


class MockPine:
    """Only memory/metadata reads and close exist; there is no write/call API."""
    def __init__(self, transform=None, serial="SLUS-20651"):
        self.memory = {}
        self.calls = []
        self.metadata_calls = []
        self.transform = transform
        self.serial = serial
        self.socket = self
        self.closed = False
        self.install()

    def install(self, context=CONTEXT, manager=MANAGER, active=ACTIVE,
                wrapper=WRAPPER, projection=PROJECTION):
        self.memory.update({
            0x400058: context, 0x402020: active, context + 0x10: manager,
            manager + 0x20: 0x498d10, manager + 0x2c: active,
            manager + 0x34: wrapper, manager + 0x64: 3,
            active + 0x1b8: 0x498950, active + 0x170: wrapper,
            active + 0x98: 0x3d9e35ec, active + 0x80: 0x3dd2f290,
            active + 0x1c0: 0x3f800000, active + 0x1c4: 0x3d800000,
            active + 0x124: 1,
            wrapper: projection, wrapper + 8: 0x49b350,
            wrapper + 0x14: 0x413cbb00, projection: projection,
            projection + 0x110: 0x413cbb00, projection + 0x114: 0x3faaaaab,
            projection + 0x118: 0x3f000000, projection + 0x11c: 0x44bb8000,
            projection + 0x190: 0x3f800000,
            projection + 0xe8: 640, projection + 0xec: 448,
            projection + 0x100: 640, projection + 0x104: 448,
            0x498d10 + 0x38: 0, 0x498d10 + 0x3c: 0x3046c0,
            0x498950 + 0x18: 0, 0x498950 + 0x1c: 0x301fa8,
            0x49b350 + 0x30: 0, 0x49b350 + 0x34: 0x323450,
            0x400b44: 0x3f800000, 0x4035bc: 1, 0x4da23c: 60,
        })

    def words(self, addresses):
        addresses = list(addresses)
        self.calls.append(addresses)
        words = [self.memory.get(address, 0) for address in addresses]
        return self.transform(addresses, words) if self.transform else words

    def string(self, opcode):
        self.metadata_calls.append(opcode)
        return {8: "mock emulator", 11: "fixture", 12: self.serial,
                13: "fixture-uuid"}[opcode]

    def close(self):
        self.closed = True


def main_only_change(target, word):
    def transform(addresses, words):
        if len(addresses) > 20:
            size = len(addresses) // 3
            index = addresses[:size].index(target)
            words[size + index] = word
        return words
    return transform


class LensPacketTests(unittest.TestCase):
    def test_complete_raw_sets_use_one_request_and_main_is_independent(self):
        pine = MockPine()
        packet = probe.capture(pine)
        self.assertEqual(packet["status"], "stable")
        self.assertTrue(packet["raw_stable"])
        self.assertTrue(packet["callbacks_match_original"])
        self.assertTrue(all(packet["identity_matches_discovery"].values()))
        self.assertEqual(len(pine.calls), 5)
        request = pine.calls[-1]
        size = len(packet["layout"])
        self.assertEqual(request[:size], request[size:2 * size])
        self.assertEqual(request[:size], request[2 * size:])
        for stage in ("before", "main", "after"):
            self.assertEqual(len(packet["raw_stages"][stage]), size)
        fields = packet["typed_main"]
        self.assertEqual(fields["camera.scalar"]["word"], 0x3d9e35ec)
        self.assertEqual(fields["camera.scalar"]["address"], "0x00120098")
        self.assertEqual(fields["camera.lens_target_ratio"]["word"], 0x3d800000)
        self.assertEqual(fields["globals.fps"]["value"], 60)
        # Independently require the original late publication fields and the
        # complete legacy projection viewport/matrix offset context.
        required = {PROJECTION + offset for offset in (
            0x20, 0x34, 0x40, 0x44, 0xe0, 0xe4, 0xe8, 0xec,
            0xf8, 0xfc, 0x100, 0x104, 0x110, 0x114, 0x118, 0x11c,
            0x120, 0x124, 0x128, 0x12c, 0x138, 0x13c, 0x140, 0x144,
            0x148, 0x14c, 0x150, 0x154, 0x158, 0x15c, 0x160, 0x164, 0x190)}
        self.assertTrue(required.issubset(request[:size]))
        self.assertIn("not a counter", packet["limitation"])

    def test_equal_bookends_do_not_hide_different_main_scalar(self):
        packet = probe.capture(MockPine(main_only_change(ACTIVE + 0x98, 0x3d800000)))
        self.assertEqual(packet["status"], "unstable")
        self.assertTrue(packet["bookends_equal"])
        self.assertFalse(packet["raw_stable"])
        self.assertEqual(packet["changed_fields"], ["camera.scalar"])
        self.assertEqual(packet["typed_main"]["camera.scalar"]["word"], 0x3d800000)
        self.assertNotEqual(packet["raw_stages"]["main"], packet["raw_stages"]["before"])

    def test_raw_signed_zero_change_in_late_cache_is_not_float_equality(self):
        packet = probe.capture(MockPine(main_only_change(PROJECTION + 0x154, 0x80000000)))
        self.assertEqual(packet["status"], "unstable")
        self.assertEqual(packet["changed_fields"], ["projection.frustum_cache_154"])
        self.assertEqual(packet["typed_main"]["projection.frustum_cache_154"]["value"], 0.0)
        self.assertEqual(packet["typed_main"]["projection.frustum_cache_154"]["bits"], "80000000")

    def test_invalid_pointer_span_is_retained_before_dereference(self):
        for root, value, invalid_field in (
            (0x400058, 3, "context"), (0x402020, 0x01fffffc, "active")):
            with self.subTest(invalid_field=invalid_field):
                pine = MockPine()
                pine.memory[root] = value
                packet = probe.capture(pine)
                self.assertEqual(packet["status"], "invalid_pointer")
                self.assertEqual(packet["invalid_field"], invalid_field)
                self.assertEqual(packet["invalid_word"], value)
                self.assertEqual(pine.calls, [[0x400058, 0x402020]])
                self.assertEqual(packet["discovery"][0]["words"][
                    0 if root == 0x400058 else 1], value)

    def test_unknown_class_retains_identity_without_reading_its_callbacks(self):
        pine = MockPine()
        pine.memory[ACTIVE + 0x1b8] = 0x490000
        packet = probe.capture(pine)
        self.assertEqual(packet["status"], "outside_class_domain")
        self.assertEqual(packet["identities"]["camera_class"], "0x00490000")
        self.assertEqual(len(pine.calls), 4)
        self.assertNotIn("raw_stages", packet)

    def test_changed_callback_and_signed_adjustment_are_retained(self):
        for adjustment, target in ((0, 0x301fac), (0xffff, 0x301fa8)):
            with self.subTest(adjustment=adjustment, target=target):
                pine = MockPine()
                pine.memory[0x498950 + 0x18] = adjustment
                pine.memory[0x498950 + 0x1c] = target
                packet = probe.capture(pine)
                self.assertEqual(packet["status"], "outside_callback_domain")
                self.assertTrue(packet["raw_stable"])
                self.assertEqual(packet["callback_descriptors"]["main"]["camera"][
                    "adjustment"], -1 if adjustment else 0)
                self.assertFalse(packet["callbacks_match_original"])

    def test_main_identity_change_and_stable_owner_alias_failure_are_retained(self):
        packet = probe.capture(MockPine(main_only_change(0x402020, ACTIVE + 0x1000)))
        self.assertEqual(packet["status"], "identity_mismatch")
        self.assertTrue(packet["bookends_equal"])
        self.assertEqual(packet["identity_matches_discovery"], {
            "before": True, "main": False, "after": True})
        pine = MockPine()
        pine.memory[MANAGER + 0x2c] = ACTIVE + 0x1000
        packet = probe.capture(pine)
        self.assertEqual(packet["status"], "identity_mismatch")
        self.assertTrue(packet["raw_stable"])
        self.assertFalse(any(packet["identity_matches_discovery"].values()))

    def test_each_attempt_resolves_new_objects_without_cached_heap_addresses(self):
        pine = MockPine()
        first = probe.capture(pine)
        pine.calls.clear()
        pine.install(context=0x200000, manager=0x210000, active=0x220000,
                     wrapper=0x230000, projection=0x240000)
        second = probe.capture(pine)
        self.assertEqual(first["status"], "stable")
        self.assertEqual(second["status"], "stable")
        self.assertEqual(second["identities"]["active"], "0x00220000")
        self.assertNotIn(ACTIVE + 0x98, pine.calls[-1])
        self.assertIn(0x220098, pine.calls[-1])

    def test_nonfinite_and_nonpositive_domains_keep_raw_words_in_json(self):
        for word in (0x7fc01234, 0x7f800000, 0, 0xbf800000):
            with self.subTest(word=word), tempfile.TemporaryDirectory() as directory:
                pine = MockPine()
                pine.memory[ACTIVE + 0x98] = word
                packet = probe.capture(pine)
                self.assertEqual(packet["status"], "outside_numeric_domain")
                self.assertTrue(packet["raw_stable"])
                self.assertFalse(packet["finite_positive_conversion_inputs"])
                item = packet["typed_main"]["camera.scalar"]
                self.assertEqual(item["word"], word)
                if word in (0x7fc01234, 0x7f800000):
                    self.assertIsNone(item["value"])
                    self.assertIn("camera.scalar", packet["nonfinite_main_fields"])
                output = Path(directory) / "packet.json"
                probe.atomic_json(output, packet)
                retained = json.loads(output.read_text(encoding="utf8"))
                self.assertEqual(retained["typed_main"]["camera.scalar"]["word"], word)

    def test_failed_and_short_reads_keep_requested_or_partial_data(self):
        def disconnected(addresses, words):
            if addresses == [CONTEXT + 0x10]:
                raise OSError("mock disconnect")
            return words
        packet = probe.capture(MockPine(disconnected))
        self.assertEqual(packet["status"], "read_error")
        self.assertEqual(packet["discovery"][-1]["addresses"], ["0x00100010"])
        self.assertNotIn("words", packet["discovery"][-1])
        self.assertIn("mock disconnect", packet["error"])

        def short(addresses, words):
            return words[:-1] if len(addresses) > 20 else words
        packet = probe.capture(MockPine(short))
        self.assertEqual(packet["status"], "read_error")
        self.assertEqual(len(packet["partial_bracket_words"]), 3 * len(packet["layout"]) - 1)
        self.assertNotIn("raw_stages", packet)

    def test_nonfinite_unused_cache_does_not_gate_positive_conversion_inputs(self):
        pine = MockPine()
        pine.memory[PROJECTION + 0x154] = 0x7fc01234
        packet = probe.capture(pine)
        self.assertEqual(packet["status"], "stable")
        self.assertTrue(packet["raw_stable"])
        self.assertTrue(packet["finite_positive_conversion_inputs"])
        self.assertEqual(packet["nonfinite_main_fields"], ["projection.frustum_cache_154"])
        cached = packet["typed_main"]["projection.frustum_cache_154"]
        self.assertEqual(cached["word"], 0x7fc01234)
        self.assertIsNone(cached["value"])
        json.dumps(packet, allow_nan=False)


class LensCliTests(unittest.TestCase):
    def test_exact_serial_gate_precedes_all_ee_reads_and_publication(self):
        for serial in ("slus-20651", "SLUS_206.51", "SLUS-20651 ", "SLES-5168"):
            with self.subTest(serial=serial), tempfile.TemporaryDirectory() as directory:
                output = Path(directory) / "wrong-game.json"
                pine = MockPine(serial=serial)
                with patch.object(probe, "Pine", return_value=pine):
                    with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit) as error:
                        probe.main(["--output", str(output)])
                self.assertEqual(error.exception.code, 2)
                self.assertEqual(pine.calls, [])
                self.assertTrue(pine.closed)
                self.assertFalse(output.exists())

    def test_argument_bounds_are_rejected_before_connection(self):
        for args in (["--samples", "0"], ["--samples", "1001"], ["--samples", "1.5"],
                     ["--interval", "-1"], ["--interval", "60.1"], ["--interval", "nan"],
                     ["--interval", "inf"], ["--port", "0"], ["--port", "65536"]):
            with self.subTest(args=args), tempfile.TemporaryDirectory() as directory:
                with patch.object(probe, "Pine") as connect:
                    with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
                        probe.main(["--output", str(Path(directory) / "none.json"), *args])
                    connect.assert_not_called()
        parsed = probe.parser().parse_args(["--output", "unused", "--samples", "1000",
                                           "--interval", "60"])
        self.assertEqual((parsed.samples, parsed.interval), (1000, 60.0))
        with patch.object(probe, "Pine") as connect:
            with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
                probe.main(["--output", str(probe.ROOT / "forbidden-capture.json")])
            connect.assert_not_called()

    def test_bounded_capture_keeps_invalid_attempt_and_hash_scope_metadata(self):
        pine = MockPine()
        pine.memory[0x400058] = 3
        def next_attempt(_):
            pine.install()
        metadata = {"git_revision": "fixture-revision", "source_files": [{
            "path": "fixture-probe.py", "sha256": "a" * 64}]}
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "nested" / "retained.json"
            with patch.object(probe, "Pine", return_value=pine):
                with patch.object(probe, "source_metadata", return_value=metadata):
                    with patch.object(probe.time, "sleep", side_effect=next_attempt) as sleep:
                        with contextlib.redirect_stdout(io.StringIO()):
                            result = probe.main(["--output", str(output), "--samples", "2",
                                                 "--interval", "60", "--provenance", "mock lifecycle"])
            report = json.loads(output.read_text(encoding="utf8"))
            self.assertEqual(result, 0)
            sleep.assert_called_once_with(60.0)
            self.assertTrue(pine.closed)
            self.assertEqual(report["status_counts"], {"invalid_pointer": 1, "stable": 1})
            self.assertEqual([a["index"] for a in report["attempts"]], [0, 1])
            self.assertEqual(report["requested_samples"], 2)
            self.assertEqual(report["provenance"], "mock lifecycle")
            self.assertEqual(report["metadata"]["git_revision"], "fixture-revision")
            self.assertEqual(report["metadata"]["source_files"], metadata["source_files"])
            self.assertTrue(report["metadata"]["read_only"])
            self.assertIn("no conversion", report["metadata"]["scope"])
            self.assertEqual(pine.metadata_calls, [8, 11, 12, 13])

    def test_serialized_output_is_atomic_and_rejects_nan(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "report.json"
            output.write_text("original\n", encoding="utf8")
            with self.assertRaises(ValueError):
                probe.atomic_json(output, {"invalid": float("nan")})
            self.assertEqual(output.read_text(encoding="utf8"), "original\n")
            self.assertEqual(list(Path(directory).iterdir()), [output])
            probe.atomic_json(output, {"valid": 17})
            self.assertEqual(json.loads(output.read_text(encoding="utf8")), {"valid": 17})
            self.assertEqual(list(Path(directory).iterdir()), [output])


if __name__ == "__main__":
    unittest.main()
