"""Mocked checks of the per-update AI capture and its report; not native evidence."""
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import pine_ai_capture as capture
import pine_ai_report as report

GAME, BALL, TEAMS, REFEREE = 0x100000, 0x110000, (0x120000, 0x130000), 0x140000


class MockPine:
    """Only the read interface: no writes, no pad input."""

    def __init__(self):
        self.memory = {capture.GAME_ROOT: GAME, GAME + 0x130: BALL, GAME + 0x184: TEAMS[0],
                       GAME + 0x188: TEAMS[1], GAME + 0x170: REFEREE, GAME + 0x128: 77,
                       GAME + 0xf4: 6}
        for k, team in enumerate(TEAMS):
            array = team + 0x1000
            self.memory[team + 0x44] = array
            for slot in range(3):
                actor = 0x200000 + (3 * k + slot) * 0x10000
                ai, pad = actor + 0x8000, actor + 0x9000
                queue, node = actor + 0xa000, actor + 0xb000
                self.memory.update({array + 4 * slot: actor, actor + 0x290: ai, ai + 4: pad,
                                    ai + 0x24: queue, ai + 0x28: queue + 4,
                                    queue: node, node + 0xc: 0, node + 0x14: 0x437a48,
                                    actor + 0x284: k, actor + 0x280: slot})
        self.memory[REFEREE + 0x194] = 0

    def words(self, addresses):
        return [self.memory.get(a, 0) for a in addresses]

    def word(self, address):
        return self.memory.get(address, 0)


class CaptureTests(unittest.TestCase):
    def test_a_sample_reads_the_queue_fronts_by_vtable(self):
        pine = MockPine()
        ptr = capture.discover(pine)
        self.assertEqual(len(ptr["actors"]), 6)
        first = capture.first_layout(ptr)
        values = first.decode(pine.words(first.addresses))
        second_layout = capture.second_layout(ptr, values)
        second = second_layout.decode(pine.words(second_layout.addresses))
        third_layout = capture.third_layout(values, second)
        third = third_layout.decode(pine.words(third_layout.addresses))
        sample = dict(values, **second, **third, torn=False)
        for key in ("frame_after_pointers", "frame_end", "game_root", "context"):
            sample.pop(key, None)
        out = capture.tidy(sample)
        self.assertEqual(out["frame"], 77)
        self.assertEqual([a["queue"][0]["name"] for a in out["actors"]], ["inbound pos"] * 6)

    def test_draws_are_counted_by_stepping_the_generator(self):
        state = [0xf22d0e56, 0x96041893, 0x3df3b646, 0x40dde76d, 0x97327ae1, 0xd1a9fbe7]
        later = state
        for _ in range(5):
            later = report.rng_next(later)
        self.assertEqual(report.draws_between(state, later), 5)
        self.assertIsNone(report.draws_between(later, state, limit=10))


if __name__ == "__main__":
    unittest.main()
