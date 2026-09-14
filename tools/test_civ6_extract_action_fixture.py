import json
from pathlib import Path
import tempfile
import unittest

from civ6_extract_action_fixture import extract


class ExtractTests(unittest.TestCase):
    def test_preserves_inputs_and_refuses_overwrite_or_missing_evidence(self):
        with tempfile.TemporaryDirectory() as directory:
            source, destination = Path(directory) / "events.jsonl", Path(directory) / "fixture.jsonl"
            events = [{"kind": "seat"}, {"kind": "state", "private": "not needed"},
                      {"kind": "tiles"}, {"kind": "action_transition_begin"},
                      {"kind": "action_transition_end", "settled": True, "isolated": True}]
            source.write_text("\n".join(map(json.dumps, events)))
            metadata = extract(source, destination, 1)
            self.assertEqual(metadata["probes"], 1)
            self.assertNotIn("not needed", destination.read_text())
            with self.assertRaises(FileExistsError):
                extract(source, destination, 1)
            with self.assertRaises(ValueError):
                extract(source, Path(directory) / "short.jsonl", 2)
            events[-1]["settled"] = False
            source.write_text("\n".join(map(json.dumps, events)))
            with self.assertRaises(ValueError):
                extract(source, Path(directory) / "unsettled.jsonl", 1)


if __name__ == "__main__":
    unittest.main()
