"""The visual backstop must not override capital privacy with blind clicks."""
import sys
import tempfile
import unittest
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
from civ6_control import install, popup_clear
import civ6_play


class CapitalPrivacyTest(unittest.TestCase):
    def test_leader_button_geometry_never_authorizes_an_answer(self):
        self.assertIsNone(popup_clear.click_target("leader", [(10, 10), (10, 20)], 100))
        self.assertIsNone(popup_clear.click_target("leader", [(10, 20)], 100))
        self.assertEqual(popup_clear.click_target("notice", [(10, 20)], 100), (10, 20))

    def test_legacy_dialogue_command_does_not_sweep_options(self):
        with patch.object(civ6_play, "dismiss_visually_confirmed_popup", return_value=(False, "leader")), patch.object(civ6_play, "click_at") as click:
            self.assertFalse(civ6_play.dismiss_leader_dialogue())
        click.assert_not_called()


class DominationCapitalExchangeWiringTest(unittest.TestCase):
    """A Domination seat's first-meet capital exchange is gated on the lane the
    run bakes into the AutoClose script; first_meet_privacy_test.lua drives the
    Lua side of the same gate."""

    @staticmethod
    def _config(**changes):
        class Defaults(SimpleNamespace):
            def __getattr__(self, name):
                return None

        return civ6_play.build_config(
            Defaults(tag="t", game_mode=[], dialogue_seconds=0.25,
                     difficulty="DIFFICULTY_DEITY", map_size="MAPSIZE_SMALL",
                     speed="GAMESPEED_ONLINE", map="Continents.lua",
                     leader="LEADER_ALEXANDER", **changes))

    def _installed_autoclose(self, config: dict) -> str:
        with tempfile.TemporaryDirectory() as tmp:
            install._write_mod(Path(tmp), dict(config))
            return (Path(tmp) / "CivvisControlAutoClose.lua").read_text()

    def test_domination_lane_reaches_the_autoclose_prelude(self):
        config = self._config(civvis_decides=True, civvis_victory="domination")
        self.assertEqual(config["VictoryLane"], "domination")
        script = self._installed_autoclose(config)
        prelude, _, body = script.partition("}\n\n")
        self.assertIn('VictoryLane = "domination"', prelude)
        self.assertIn("local cfg = CivvisControlConfig or {};", body)
        self.assertIn('cfg.VictoryLane == "domination"', body)

    def test_no_decider_bakes_no_lane_so_the_exchange_stays_declined(self):
        config = self._config(civvis_victory="domination")
        self.assertIsNone(config["VictoryLane"])
        prelude = self._installed_autoclose(config).partition("}\n\n")[0]
        self.assertIn("VictoryLane = nil", prelude)

    def test_other_lanes_bake_their_own_name(self):
        for lane in civ6_play.VICTORY_LANES:
            if lane == "domination":
                continue
            with self.subTest(lane=lane):
                config = self._config(civvis_decides=True, civvis_victory=lane)
                self.assertEqual(config["VictoryLane"], lane)


if __name__ == "__main__":
    unittest.main()
