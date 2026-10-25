"""Freeze the registered eighteen observational frames before mission analysis."""
import argparse
import hashlib
import json
from pathlib import Path


def sha(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--census", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    census = json.loads(args.census.read_text())
    args.out.mkdir(exist_ok=False)
    result = {"scope": __doc__, "census_sha256": sha(args.census.read_bytes()),
              "source_sha256": sha(Path(__file__).read_bytes()), "cases": []}
    for tag in dict.fromkeys(c["tag"] for c in census["candidate_frames"]):
        cases = [{**c, "status": "missing_boundary"} for c in census["candidate_frames"]
                 if c["tag"] == tag]
        result["cases"].extend(cases)
        source = Path.home() / "civvis-civ6-runs/control" / tag / "events.jsonl"
        pending = {c["turn"]: c for c in cases}
        prefix = bytearray()
        try:
            stream = source.open("rb")
        except FileNotFoundError:
            for case in cases:
                case["status"] = "missing_events"
            continue
        with stream:
            for line in stream:
                event = json.loads(line)
                case = pending.get(event.get("turn"))
                if case and event.get("kind") == "state":
                    if "state_sha256" in case:
                        case["status"] = "second_state_before_await"
                        del pending[case["turn"]]
                    else:
                        case.update(state_sha256=sha(line), state_start=len(prefix),
                                    state_frame=event.get("frame"))
                if case and case.get("state_sha256") and event.get("kind") == "await":
                    path = args.out / f"{tag}-t{case['turn']}.jsonl"
                    path.write_bytes(prefix)
                    case.update(status="prefix_frozen", path=str(path.resolve()),
                                input_sha256=sha(prefix), input_bytes=len(prefix))
                    del pending[case["turn"]]
                prefix.extend(line)
                if not pending:
                    break
    (args.out / "manifest.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps([{k: c[k] for k in ("tag", "turn", "status")} for c in result["cases"]]))


if __name__ == "__main__":
    main()
