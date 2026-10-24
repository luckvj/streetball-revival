from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from compare_staged_motion import manager_height_is_zero


class ManagerHeightTests(unittest.TestCase):
    def test_both_signed_zeroes_allow_the_original_producer(self):
        for word in (0, 0x80000000):
            with self.subTest(word=word):
                self.assertTrue(manager_height_is_zero({'payload_words': [0, word]}))

    def test_later_clamped_actor_height_cannot_admit_a_nonzero_manager_height(self):
        for word in (0xbe000000, 0x35000000, 0x3f800000):
            with self.subTest(word=word):
                self.assertFalse(manager_height_is_zero({'payload_words': [0, word], 'actor_height': 0.0}))


if __name__ == '__main__':
    unittest.main()
