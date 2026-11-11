"""Only the explicitly armed native Domination material experiment bids cash."""
from pathlib import Path
from types import SimpleNamespace
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parent))
import civ6_play


class Defaults(SimpleNamespace):
    def __getattr__(self, name):
        return None


class StrategicCashOfferTests(unittest.TestCase):
    def config(self, **changes):
        values = dict(tag="test", game_mode=[], difficulty="DIFFICULTY_DEITY",
                      map_size="MAPSIZE_TINY", speed="GAMESPEED_ONLINE",
                      map="Pangaea.lua", leader="LEADER_ALEXANDER",
                      civvis_decides=True, civvis_victory="domination",
                      civvis_with=[], civvis_without=[])
        values.update(changes)
        return civ6_play.build_config(Defaults(**values))

    def test_default_does_not_bid(self):
        self.assertFalse(self.config()["StrategicCashCeilingBid"])

    def test_armed_domination_material_desk_bids(self):
        self.assertTrue(self.config(civvis_with=["land-army-asks-the-host-price"])
                        ["StrategicCashCeilingBid"])

    def test_withheld_experiment_does_not_bid(self):
        self.assertFalse(self.config(civvis_with=["land-army-asks-the-host-price"],
                                     civvis_without=["land-army-asks-the-host-price"])
                         ["StrategicCashCeilingBid"])

    def test_other_victory_lanes_do_not_bid(self):
        for lane in ["culture", "science", "diplomatic", None]:
            with self.subTest(lane=lane):
                self.assertFalse(self.config(civvis_victory=lane,
                                             civvis_with=["land-army-asks-the-host-price"])
                                 ["StrategicCashCeilingBid"])

    def test_builtin_controller_does_not_bid(self):
        self.assertFalse(self.config(civvis_decides=False,
                                     civvis_with=["land-army-asks-the-host-price"])
                         ["StrategicCashCeilingBid"])

    def test_other_experiment_does_not_bid(self):
        self.assertFalse(self.config(civvis_with=["research-follows-the-first-capital"])
                         ["StrategicCashCeilingBid"])


if __name__ == "__main__":
    unittest.main()
