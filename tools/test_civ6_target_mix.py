import unittest
from civ6_target_mix import TARGETS, METRICS, FLOOR, fit, partition


def corpus(reverse_validation=False):
    profile = {"map": "continents", "width": 74, "height": 46, "players": 6,
               "city_states": 9, "ruleset": "RULESET_EXPANSION_2", "modes": [], "native_competitions": True}
    rows = []
    for source in ("live", "native"):
        for game in range(30):
            run = f"{source}-{game}"
            for turn in (25, 50):
                rotated = TARGETS[game % len(TARGETS):] + TARGETS[:game % len(TARGETS)]
                for seat, target in enumerate(rotated[:6] if source == "native" else ("observed_firaxis",)):
                    value = 10 if source == "live" or target == "science" else 1
                    if source == "live" and reverse_validation and partition(run) == "validation":
                        value = 1
                    rows.append({"source": source, "run": run, "seat": seat,
                                 "model_build": "test-native-build" if source == "native" else None,
                                 "target": target, "cohort": ("online", "emperor", turn),
                                 "profile": profile, "values": {metric: value for metric in METRICS}})
    return rows


class TargetMixTests(unittest.TestCase):
    def test_fits_a_proposal_without_eliminating_any_objective(self):
        result = fit(corpus())
        self.assertEqual(result["status"], "proposal")
        self.assertFalse(result["production_policy_changed"])
        self.assertAlmostEqual(sum(result["weights"].values()), 1)
        self.assertTrue(all(weight >= FLOOR - 1e-12 for weight in result["weights"].values()))
        self.assertGreater(result["weights"]["science"], .5)

    def test_validation_never_changes_the_fitted_weights(self):
        normal, reversed_reference = fit(corpus()), fit(corpus(True))
        self.assertEqual(normal["weights"], reversed_reference["weights"])
        self.assertEqual(reversed_reference["status"], "rejected_on_validation")

    def test_duplicate_input_does_not_increase_evidence(self):
        rows = corpus()
        self.assertEqual(fit(rows), fit(rows + rows))

    def test_missing_or_different_profiles_and_tiny_samples_are_refused(self):
        self.assertEqual(fit([])["status"], "insufficient_evidence")
        rows = corpus()
        self.assertEqual(fit(rows[:2])["status"], "insufficient_evidence")
        rows[0] = {**rows[0], "profile": {"map": "continents"}}
        self.assertEqual(fit(rows)["status"], "insufficient_evidence")


if __name__ == "__main__": unittest.main()
