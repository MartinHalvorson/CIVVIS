"""The visual backstop must not override capital privacy with blind clicks."""
import sys
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))
from civ6_control import popup_clear
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


if __name__ == "__main__":
    unittest.main()
