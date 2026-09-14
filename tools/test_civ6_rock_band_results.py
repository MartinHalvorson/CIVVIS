"""Both stages of a concert must reach the registered automatic closer."""
import unittest
import xml.etree.ElementTree as ET
from pathlib import Path


class RockBandResultsRegistrationTest(unittest.TestCase):
    def test_movie_and_followup_results_both_load_the_closer(self):
        mod = Path(__file__).parent / "civ6_control" / "mod"
        root = ET.parse(mod / "CivvisControl.modinfo").getroot()
        actions = root.findall("./InGameActions/ReplaceUIScript")
        files = {entry.text for entry in root.findall("./Files/File")}
        for context in ("RockBandMoviePopup", "RockBandPopup"):
            with self.subTest(context=context):
                matches = [action for action in actions
                           if action.findtext("Properties/LuaContext") == context]
                self.assertEqual(len(matches), 1, "concert stage must have one closer")
                script = matches[0].findtext("Properties/LuaReplace")
                self.assertEqual(script, "CivvisControlAutoClose.lua")
                self.assertIn(script, files)
                self.assertTrue((mod / script).is_file())


if __name__ == "__main__":
    unittest.main()
