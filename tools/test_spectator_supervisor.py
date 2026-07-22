import importlib.util
from pathlib import Path
import unittest


MODULE_PATH = Path(__file__).with_name("spectator_supervisor.py")
SPEC = importlib.util.spec_from_file_location("spectator_supervisor", MODULE_PATH)
assert SPEC and SPEC.loader
supervisor = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(supervisor)


class SessionSettingsTests(unittest.TestCase):
    def test_preserves_live_map_and_player_settings(self):
        state = {
            "players": [
                {"is_minor": False},
                {"is_minor": False},
                {"is_minor": True},
                {"is_minor": True, "is_barbarian": True},
            ],
            "map": {"width": 44, "height": 26},
        }
        defaults = {
            "players": 4,
            "width": 60,
            "height": 38,
            "city_states": 6,
            "turns": 500,
        }
        self.assertEqual(
            supervisor.session_settings(state, defaults),
            {"players": 2, "width": 44, "height": 26, "city_states": 1, "turns": 500},
        )

    def test_empty_state_uses_defaults(self):
        defaults = {
            "players": 6,
            "width": 74,
            "height": 46,
            "city_states": 9,
            "turns": 500,
        }
        self.assertEqual(supervisor.session_settings({}, defaults), defaults)


if __name__ == "__main__":
    unittest.main()
