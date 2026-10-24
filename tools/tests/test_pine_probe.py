"""Read-only mocked PINE checks; these fixtures are not native-game evidence."""
import contextlib
import io
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import pine_probe as probe


GAME = 0x100000
BALL = 0x110000
GUARD_ADDRESSES = [0x3d30bc, GAME + 0x130] + [
    BALL + offset for _, offset, _ in probe.FLIGHT_CLOCK_GUARD]
BALL_ADDRESSES = [BALL + offset + i * 4
                  for _, offset, kind in probe.BALL + probe.FLIGHT_CLOCK
                  for i in range({"vec4": 4, "vec3": 3}.get(kind, 1))]


class MockPine:
    """No transport/write methods: only the read interface used by snapshots."""
    def __init__(self, transform=None):
        self.memory = {0x3d30bc: GAME, GAME + 0x130: BALL,
                       BALL + 0x60: 12, BALL + 0x64: 28,
                       BALL + 0x158: 21, BALL + 0x180: 4, BALL + 0x190: 0}
        self.calls = []
        self.transform = transform
        self.socket = self
        self.closed = False

    def words(self, addresses):
        addresses = list(addresses)
        self.calls.append(addresses)
        values = [self.memory.get(address, 0) for address in addresses]
        return self.transform(addresses, values, self.calls) if self.transform else values

    def word(self, address):
        return self.words([address])[0]

    def string(self, opcode):
        return {8: "mock", 11: "fixture", 12: "SLUS-20651", 13: "fixture-uuid"}[opcode]

    def close(self):
        self.closed = True


class OrdinaryActorGuardTests(unittest.TestCase):
    def test_hidden_actor_lifetime_change_and_signed_zero_are_rejected(self):
        layout = [("root_state", 0x60, "u32"), ("orientation", 0x50, "vec4")]
        packet = {"root_state": 2, "orientation": [0.0, -0.0, 0.0, 1.0]}
        expected = {0x60: 1, 0x50: 0, 0x54: 0x80000000, 0x58: 0, 0x5c: 0x3f800000}
        self.assertFalse(probe.guarded_fields_match(packet, layout, expected))
        packet["root_state"] = 1
        self.assertTrue(probe.guarded_fields_match(packet, layout, expected))
        packet["orientation"][1] = 0.0
        self.assertFalse(probe.guarded_fields_match(packet, layout, expected))


    @staticmethod
    def actor_pine(transform=None):
        pine = MockPine(transform)
        pine.memory.update({
            GAME + 0x184: 0x130000,
            0x130000 + 0x44: 0x140000,
            0x140000: 0x150000,
            0x150000 + 0xc8: 0x160000,
            0x160000: 0x170000,
            0x150000 + 0x60: 1,
            0x150000 + 0x64: 5,
            0x150000 + 0x54: 0x80000000,
        })
        return pine

    def test_snapshot_actor_packet_matches_lifetime_and_channel_identity(self):
        actor = probe.snapshot(self.actor_pine(), include_gait=True)["actors"][0]
        guard = actor["ordinary_vm_guard"]
        self.assertTrue(guard["stable"])
        self.assertTrue(guard["packet_matches_guards"])
        self.assertTrue(guard["channel_identity_stable"])
        self.assertEqual(guard["channel_slot_before"], 0x170000)
        self.assertEqual(guard["channel_slot_after"], 0x170000)
        self.assertEqual(actor["channel0"]["address"], "0x00170000")
        self.assertEqual(guard["before"][guard["offsets"].index(0x54)],
                         0x80000000)

    def test_snapshot_equal_bookends_reject_hidden_main_lifetime_change(self):
        actor_address = 0x150000
        gait_addresses = [
            actor_address + offset + i * 4
            for _, offset, kind in probe.GAIT_PLAYER
            for i in range({"vec4": 4, "vec3": 3}.get(kind, 1))
        ]
        def alter_main(addresses, values, calls):
            if addresses == gait_addresses:
                values[addresses.index(actor_address + 0x60)] = 2
            return values
        actor = probe.snapshot(self.actor_pine(alter_main),
                               include_gait=True)["actors"][0]
        guard = actor["ordinary_vm_guard"]
        self.assertTrue(guard["stable"])
        self.assertFalse(guard["packet_matches_guards"])
        self.assertTrue(guard["channel_identity_stable"])
        self.assertEqual(actor["gait"]["root_state"], 2)
        offset = guard["offsets"].index(0x60)
        self.assertEqual((guard["before"][offset], guard["after"][offset]),
                         (1, 1))

    def test_snapshot_changed_channel_slot_is_retained_and_rejected(self):
        def alter_after(addresses, values, calls):
            if addresses == [0x160000] and calls.count([0x160000]) == 3:
                values[0] = 0x180000
            return values
        actor = probe.snapshot(self.actor_pine(alter_after),
                               include_gait=True)["actors"][0]
        guard = actor["ordinary_vm_guard"]
        self.assertTrue(guard["stable"])
        self.assertTrue(guard["packet_matches_guards"])
        self.assertFalse(guard["channel_identity_stable"])
        self.assertEqual(guard["channel_slot_before"], 0x170000)
        self.assertEqual(guard["channel_slot_after"], 0x180000)
        self.assertEqual(actor["channel0"]["address"], "0x00170000")

    def test_snapshot_equal_slot_bookends_reject_different_main_channel(self):
        def alter_main(addresses, values, calls):
            if addresses == [0x160000] and calls.count([0x160000]) == 2:
                values[0] = 0x180000
            return values
        actor = probe.snapshot(self.actor_pine(alter_main),
                               include_gait=True)["actors"][0]
        guard = actor["ordinary_vm_guard"]
        self.assertTrue(guard["stable"])
        self.assertTrue(guard["packet_matches_guards"])
        self.assertFalse(guard["channel_identity_stable"])
        self.assertEqual(guard["channel_slot_before"], 0x170000)
        self.assertEqual(guard["channel_slot_after"], 0x170000)
        self.assertEqual(actor["channel0"]["address"], "0x00180000")


class OrdinaryChannelGuardTests(unittest.TestCase):
    def test_all_scalar_inputs_are_bracketed_and_main_packet_matches(self):
        channel = 0x120000
        pine = MockPine()
        pine.memory.update({channel + 0x3c: 0x3f800000,
                            channel + 0x64: 0x80000000,
                            channel + 0x68: 0x3f800000})
        packet = probe.gait_channel(pine, channel)
        self.assertTrue(packet["guards_stable"])
        self.assertTrue(packet["packet_matches_guards"])
        self.assertTrue({0x28, 0x3c, 0x58, 0x5c, 0x6c}.issubset(packet["guard_offsets"]))
        self.assertEqual(pine.calls[0], pine.calls[-1])
        self.assertEqual(packet["guards_before"][packet["guard_offsets"].index(0x64)],
                         0x80000000)

    def test_equal_bookends_do_not_admit_a_different_main_frame(self):
        channel = 0x120000
        def alter_main(addresses, values, calls):
            if len(calls) == 2:
                values[addresses.index(channel + 0x5c)] = 7
            return values
        packet = probe.gait_channel(MockPine(alter_main), channel)
        self.assertTrue(packet["guards_stable"])
        self.assertFalse(packet["packet_matches_guards"])
        self.assertEqual(packet["frame_word"], 7)

    def test_changed_duration_is_retained_and_rejected(self):
        channel = 0x120000
        def alter_after(addresses, values, calls):
            if len(calls) == 3:
                values[addresses.index(channel + 0x6c)] = 31
            return values
        packet = probe.gait_channel(MockPine(alter_after), channel)
        self.assertFalse(packet["guards_stable"])
        self.assertFalse(packet["packet_matches_guards"])
        self.assertEqual(packet["guards_after"][packet["guard_offsets"].index(0x6c)],31)


class FlightClockTests(unittest.TestCase):
    def test_default_shape_and_reads_remain_unchanged(self):
        plain_pine, clock_pine = MockPine(), MockPine()
        with patch.object(probe.time, "time_ns", return_value=123):
            plain = probe.snapshot(plain_pine)
            clock = probe.snapshot(clock_pine, include_flight_clock=True)
        self.assertNotIn("flight_clock", plain)
        self.assertNotIn("progress", plain["ball"])
        self.assertNotIn("timer_bypass", plain["ball"])
        plain_addresses = {address for call in plain_pine.calls for address in call}
        self.assertNotIn(BALL + 0x60, plain_addresses)
        self.assertNotIn(BALL + 0x190, plain_addresses)
        clock.pop("flight_clock")
        clock["ball"].pop("progress")
        clock["ball"].pop("timer_bypass")
        self.assertEqual(clock, plain)

    def test_guards_immediately_bracket_one_ball_packet_and_keep_u32(self):
        pine = MockPine()
        pine.memory[BALL + 0x60] = 0xdeadbeef
        pine.memory[BALL + 0x190] = 0xffffffff
        result = probe.snapshot(pine, include_flight_clock=True)
        clock = result["flight_clock"]
        self.assertEqual(result["ball"]["progress"], 0xdeadbeef)
        self.assertEqual(result["ball"]["timer_bypass"], 0xffffffff)
        expected = {"game": GAME, "ball": BALL, "progress": 0xdeadbeef,
                    "flight_updates": 28, "timer": 21, "state": 4,
                    "timer_bypass": 0xffffffff}
        self.assertEqual(clock["guard_before"], expected)
        self.assertEqual(clock["guard_after"], expected)
        self.assertTrue(clock["guard_words_stable"])
        self.assertTrue(clock["packet_matches_guards"])
        index = pine.calls.index(BALL_ADDRESSES)
        self.assertEqual(pine.calls[index - 1:index + 2],
                         [GUARD_ADDRESSES, BALL_ADDRESSES, GUARD_ADDRESSES])

    def test_changed_clock_words_reject_bracket(self):
        def advance(addresses, values, calls):
            if addresses == GUARD_ADDRESSES and calls.count(GUARD_ADDRESSES) == 2:
                values[2] += 1
                values[4] -= 1
            return values
        clock = probe.snapshot(MockPine(advance), include_flight_clock=True)["flight_clock"]
        self.assertFalse(clock["guard_words_stable"])
        self.assertFalse(clock["packet_matches_guards"])
        self.assertEqual(clock["guard_after"]["progress"], 13)
        self.assertEqual(clock["guard_after"]["timer"], 20)

    def test_matching_guards_cannot_hide_different_main_packet(self):
        def tear(addresses, values, calls):
            if addresses == BALL_ADDRESSES:
                values[addresses.index(BALL + 0x190)] = 1
            return values
        result = probe.snapshot(MockPine(tear), include_flight_clock=True)
        self.assertEqual(result["ball"]["timer_bypass"], 1)
        self.assertTrue(result["flight_clock"]["guard_words_stable"])
        self.assertFalse(result["flight_clock"]["packet_matches_guards"])

    def test_changed_identity_or_duration_rejects_packet(self):
        for guard_index in (0, 1, 3, 5, 6):
            with self.subTest(guard_index=guard_index):
                def change(addresses, values, calls):
                    if addresses == GUARD_ADDRESSES and calls.count(GUARD_ADDRESSES) == 2:
                        values[guard_index] += 4
                    return values
                clock = probe.snapshot(MockPine(change), include_flight_clock=True)["flight_clock"]
                self.assertFalse(clock["guard_words_stable"])
                self.assertFalse(clock["packet_matches_guards"])

    def test_stable_guards_pointing_to_another_ball_are_not_accepted(self):
        def replaced(addresses, values, calls):
            if addresses == GUARD_ADDRESSES:
                values[1] = BALL + 4
            return values
        clock = probe.snapshot(MockPine(replaced), include_flight_clock=True)["flight_clock"]
        self.assertTrue(clock["guard_words_stable"])
        self.assertFalse(clock["packet_matches_guards"])

    def test_missing_ball_omits_clock_packet(self):
        pine = MockPine()
        pine.memory[GAME + 0x130] = 3
        result = probe.snapshot(pine, include_flight_clock=True)
        self.assertNotIn("ball", result)
        self.assertNotIn("flight_clock", result)
        self.assertNotIn(GUARD_ADDRESSES, pine.calls)

    def test_cli_records_opt_in_metadata_and_coverage(self):
        for enabled in (False, True):
            with self.subTest(enabled=enabled), tempfile.TemporaryDirectory() as directory:
                path = Path(directory) / "capture.json"
                pine = MockPine()
                argv = ["pine_probe.py", "--out", str(path)]
                if enabled:
                    argv.append("--flight-clock")
                with patch.object(sys, "argv", argv), patch.object(probe, "Pine", return_value=pine):
                    with contextlib.redirect_stdout(io.StringIO()):
                        probe.main()
                report = json.loads(path.read_text(encoding="utf-8"))
                self.assertTrue(pine.closed)
                self.assertEqual(report["metadata"]["serial"], "SLUS-20651")
                if enabled:
                    self.assertIs(report["metadata"]["flight_clock_capture"], True)
                    self.assertIn("do not prove atomic", report["metadata"]["flight_clock_limitation"])
                    self.assertEqual(report["coverage"]["flight_clock_packets"], 1)
                    self.assertEqual(report["coverage"]["flight_clock_guarded_packets"], 1)
                    self.assertEqual(report["coverage"]["flight_clock_unstable_packets"], 0)
                else:
                    self.assertNotIn("flight_clock_capture", report["metadata"])
                    self.assertNotIn("flight_clock_packets", report["coverage"])


class StagedPhysicalPositionGuardTests(unittest.TestCase):
    def capture(self, changed=False):
        manager, actor, proxy, channel, skeleton = [0x120000 + 0x10000 * i for i in range(5)]
        reads = 0

        def transform(addresses, values, calls):
            nonlocal reads
            if addresses[-11:-9] == [actor + 0x30, actor + 0x38]:
                reads += 1
                if changed and reads == 2:
                    values[-11] = 0x40000000
            return values

        pine = MockPine(transform)
        pine.memory.update({manager + 0x50: 3, manager + 0x58: 11,
                            actor + 0x2a0: proxy, actor + 0x108: skeleton,
                            actor + 0x30: 0x3f800000, actor + 0x34: 0x40400000,
                            actor + 0x38: 0x40800000, proxy + 0x14: 10,
                            proxy + 0x18: manager, proxy + 0xb8: actor})
        body = {"address": hex(actor), "id": 0, "channel0": {"address": hex(channel)}}
        packet = probe.staged_clock_packet(pine, manager, [body])
        return packet["actors"][0]

    def test_stable_physical_position_retains_exact_bits_and_legacy_guard_shape(self):
        item = self.capture()
        self.assertEqual(item['physical_position_before'], [0x3f800000, 0x40400000, 0x40800000])
        self.assertEqual(item['physical_position_before'], item['physical_position_after'])
        self.assertTrue(item['physical_position_guards_stable'])
        self.assertEqual(len(item['guards_before']), 10)

    def test_a_horizontal_change_is_rejected_even_when_the_clock_guard_stays_stable(self):
        item = self.capture(changed=True)
        self.assertTrue(item['guards_stable'])
        self.assertFalse(item['physical_position_guards_stable'])
        self.assertNotEqual(item['physical_position_before'], item['physical_position_after'])


if __name__ == "__main__":
    unittest.main()
