import copy
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from compare_staged_paths import reference_position


class ReferencePositionAdmissionTests(unittest.TestCase):
    def setUp(self):
        self.position = [0x3f800000, 0, 0x40000000]
        self.actor = dict(reference_position_before=self.position.copy(),
                          reference_position_after=self.position.copy(),
                          physical_position_before=[0x40800000, 0, 0x40a00000],
                          physical_position_guards_stable=True,
                          actor_launch_before=0, actor_launch_after=0,
                          payload_words=self.position + [0] * 10)

    def test_animated_root_displacement_does_not_replace_the_reference_basis(self):
        self.assertEqual(reference_position(self.actor), (self.position, None))

    def test_changed_or_missing_bookends_are_retained_as_exclusions(self):
        for field, value in [('physical_position_guards_stable', False),
                             ('reference_position_after', [0, 0, 0]),
                             ('reference_position_before', None)]:
            with self.subTest(field=field):
                actor = copy.deepcopy(self.actor)
                actor[field] = value
                self.assertEqual(reference_position(actor)[1],
                                 'missing_or_changed_reference_position_bookends')

    def test_matching_bookends_cannot_hide_a_torn_middle_payload(self):
        self.actor['payload_words'][1] = 0x3f800000
        self.assertEqual(reference_position(self.actor)[1],
                         'reference_bookend_does_not_match_live_proxy_payload')

    def test_actor_launch_and_height_offsets_are_outside_this_comparison(self):
        self.actor['actor_launch_before'] = 1
        self.assertEqual(reference_position(self.actor)[1],
                         'native_actor_launch_outside_reference_placement')
        self.actor['actor_launch_before'] = 0
        self.actor['physical_position_before'][1] = 0x3f800000
        self.assertEqual(reference_position(self.actor)[1],
                         'native_reference_and_physical_height_do_not_correspond')

    def test_nonfinite_original_words_are_rejected_before_model_evaluation(self):
        for value in (0x7fc00000, 0x7f800000, 0xff800000):
            with self.subTest(value=value):
                self.actor['reference_position_before'][0] = value
                self.actor['reference_position_after'][0] = value
                self.actor['payload_words'][0] = value
                self.assertEqual(reference_position(self.actor)[1],
                                 'nonfinite_original_reference_position')


if __name__ == '__main__':
    unittest.main()
