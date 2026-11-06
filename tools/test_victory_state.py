import copy
import unittest
from victory_state import apply_game_result, completed_frame


def win(lane, **fields):
    return dict(kind="victory", victory_type="VICTORY_" + lane, won=True,
                local_player=3, turn=173, **fields)


class VictoryStateTest(unittest.TestCase):
    def test_diplomatic_is_a_lower_bound_and_does_not_rewrite_history(self):
        frame = dict(turn=172, dvp=16)
        final = completed_frame(frame, win("DIPLOMATIC"))
        self.assertEqual(final["dvp"], 20)
        self.assertEqual(final["turn"], 173)
        self.assertEqual(frame, dict(turn=172, dvp=16))
        self.assertEqual(completed_frame(dict(turn=173, dvp=23), win("DIPLOMATIC"))["dvp"], 23)

    def test_science_uses_host_target_and_keeps_overshoot(self):
        for target, points in ((25, 24), (50, 53)):
            final = completed_frame(dict(turn=172, science_victory_points_needed=target,
                                         science_victory_points=points), win("TECHNOLOGY"))
            self.assertEqual(final["science_victory_points"], max(target, points))

    def test_culture_exceeds_the_rivals_target(self):
        final = completed_frame(dict(turn=172, foreign_tourists=60,
                                    rivals=[dict(civ="CIVILIZATION_GREECE", domestic_tourists=89)]), win("CULTURE"))
        self.assertEqual(final["foreign_tourists"], 90)

    def test_every_lane_finishes_the_mirror_without_inventing_cities(self):
        for lane in ("TECHNOLOGY", "CULTURE", "DIPLOMATIC", "CONQUEST", "RELIGIOUS", "SCORE"):
            game = dict(turn=172, players=[dict(dvp=16)], cities=[dict(owner=1)],
                        observed_public_empire_stats={"0": dict(science_victory_points=24,
                                                               science_victory_points_needed=25)})
            apply_game_result(game, [win(lane)])
            self.assertEqual(game["winner"], 0)
            self.assertEqual(game["turn"], 173)
            self.assertEqual(game["cities"], [dict(owner=1)])
            if lane == "TECHNOLOGY":
                self.assertEqual(game["observed_public_empire_stats"]["0"]["science_victory_points"], 25)

    def test_losses_unconfirmed_and_old_results_do_not_complete_a_game(self):
        for outcome in (dict(win("TECHNOLOGY"), won=False), dict(kind="defeat"),
                        dict(win("TECHNOLOGY"), turn=100), dict(win("TECHNOLOGY"), victory_type="UNKNOWN")):
            game = dict(turn=172, players=[dict(dvp=16)])
            before = copy.deepcopy(game)
            self.assertEqual(apply_game_result(game, [outcome]), before)


if __name__ == "__main__":
    unittest.main()
