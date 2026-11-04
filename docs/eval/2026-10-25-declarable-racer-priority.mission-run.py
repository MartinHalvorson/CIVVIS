"""Run every frozen observational case; preserve zeros and reconstruction errors."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    manifest = json.loads(args.manifest.read_text())
    args.out.mkdir(exist_ok=False)
    sha = lambda data: hashlib.sha256(data).hexdigest()
    binary_hash = sha(args.binary.read_bytes())
    result = {"scope": __doc__, "manifest_sha256": sha(args.manifest.read_bytes()),
              "binary_sha256": binary_hash, "cases": []}
    for index, case in enumerate(manifest["cases"]):
        entry = {"index": index, "tag": case["tag"], "turn": case["turn"],
                 "input_status": case["status"]}
        result["cases"].append(entry)
        if case["status"] != "prefix_frozen":
            continue
        data = Path(case["path"]).read_bytes()
        assert sha(data) == case["input_sha256"]
        command = [str(args.binary), str(args.manifest), str(index)]
        try:
            done = subprocess.run(command, capture_output=True, timeout=120)
            (args.out / f"{index}.stdout").write_bytes(done.stdout)
            (args.out / f"{index}.stderr").write_bytes(done.stderr)
            entry["exit_code"] = done.returncode
            if done.returncode == 0:
                entry["result"] = json.loads(done.stdout)
        except subprocess.TimeoutExpired as error:
            entry["timeout"] = True
            (args.out / f"{index}.stdout").write_bytes(error.stdout or b"")
            (args.out / f"{index}.stderr").write_bytes(error.stderr or b"")
        (args.out / "result.json").write_text(json.dumps(result, indent=2) + "\n")
        print(index, case["tag"], case["turn"], entry.get("exit_code", "timeout"), flush=True)
    assert sha(args.binary.read_bytes()) == binary_hash
    result["status"] = "complete_all_cases_retained"
    (args.out / "result.json").write_text(json.dumps(result, indent=2) + "\n")


if __name__ == "__main__":
    main()
