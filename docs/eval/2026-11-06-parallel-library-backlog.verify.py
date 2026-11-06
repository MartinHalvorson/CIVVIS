"""Verify the archived chronological proposal evidence; never starts a game.

Usage: python3 THIS_FILE [native-queries.tar.xz]
Only the Python standard library is required. Reads archive members in place.
"""
import hashlib
import json
from pathlib import Path, PurePosixPath
import sys
import tarfile

TAG = "parallel-library-backlog"


def digest(data):
    return hashlib.sha256(data).hexdigest()


def exact(a, b):
    # Preserve JSON scalar types (including integers versus floats).
    if type(a) is not type(b):
        return False
    if isinstance(a, dict):
        return a.keys() == b.keys() and all(exact(a[k], b[k]) for k in a)
    if isinstance(a, list):
        return len(a) == len(b) and all(exact(x, y) for x, y in zip(a, b))
    return a == b


def payload(reply):
    return {k: reply[k] for k in ("orders", "decision") if k in reply}


def paired_controls(off, on):
    genomes = [dict(r["genome"]) for r in (off, on)]
    for field in ("treatments", "forced"):
        a, b = [g.pop(field) for g in genomes]
        assert len(a) == len(set(a)) and len(b) == len(set(b))
        assert TAG not in a and set(b) == set(a) | {TAG}
    # Includes ledger_withheld: no excluded genome metadata.
    assert exact(*genomes)
    argv = [list(r["command"]) for r in (off, on)]
    for args in argv:
        assert args.count("--mirror") == 1
        args[args.index("--mirror") + 1] = "<same-input>"
    matches = [i for i in range(len(argv[1])-1) if argv[1][i:i+2] == ["--with", TAG]]
    assert len(matches) == 1
    del argv[1][matches[0]:matches[0]+2]
    assert argv[0] == argv[1]


class Captures:
    def __init__(self, read, directory):
        self.data = read(directory + "/bytes.bin")
        self.rows = [json.loads(line) for line in read(directory + "/snapshots.jsonl").splitlines()]
        self.hashes = {}
        offset = 0
        for i, r in enumerate(self.rows):
            assert set(r) == {"id", "parent", "offset", "append_bytes", "total_bytes"}
            assert all(type(r[k]) is int and r[k] >= 0 for k in ("id", "offset", "append_bytes", "total_bytes"))
            p = r["parent"]
            assert r["id"] == i and r["offset"] == offset
            assert p is None or type(p) is int and 0 <= p < i
            assert r["total_bytes"] == (0 if p is None else self.rows[p]["total_bytes"]) + r["append_bytes"]
            offset += r["append_bytes"]
        assert offset == len(self.data)

    def snapshot(self, i):
        assert type(i) is int and 0 <= i < len(self.rows)
        chunks = []
        while i is not None:
            r = self.rows[i]
            chunks.append(self.data[r["offset"]:r["offset"]+r["append_bytes"]])
            i = r["parent"]
        return b"".join(reversed(chunks))

    def sequence(self, capture):
        assert capture["schema"] == 1 and capture["complete"] is True and capture["error"] is None
        assert capture["archive"] == "bytes.bin" and capture["index"] == "snapshots.jsonl"
        result = []
        for r in capture["reads"]:
            assert set(r) == {"snapshot_id", "count"} and type(r["count"]) is int and r["count"] > 0
            i = r["snapshot_id"]
            if i not in self.hashes:
                data = self.snapshot(i)
                self.hashes[i] = (digest(data), len(data))
            result.extend([self.hashes[i]] * r["count"])
        assert result
        return result


def verify(path):
    with tarfile.open(path, "r:*") as archive:
        names = set()
        for m in archive.getmembers():
            p = PurePosixPath(m.name)
            assert m.isfile() and not p.is_absolute() and ".." not in p.parts and m.name not in names
            names.add(m.name)

        def read(name):
            return archive.extractfile(name).read()

        def load(name):
            return json.loads(read(name))

        def lines(name):
            return [json.loads(line) for line in read(name).splitlines()]

        manifest = load("manifest.json")
        assert names == set(manifest["files"]) | {"manifest.json"}
        for name, sha in manifest["files"].items():
            assert digest(read(name)) == sha, name
        build = load("build/build-receipt.json")
        assert build["exit"] == 0 and build["source_unchanged"] is True
        assert build["source_revision"] == manifest["source"]
        assert build["binary_sha256"] == manifest["binary_sha256"]
        total_queries = total_reads = 0
        for case in manifest["cases"]:
            prefix = case["directory"]
            intent = load(prefix + "/intent.json")
            assert intent["source"] == manifest["source"] and intent["binary_sha256"] == manifest["binary_sha256"]
            for filename, sha in intent["runtime_manifest"].items():
                assert digest(read("runtime/" + case["name"] + "/" + filename)) == sha
            native = lines(prefix + "/native-decisions.jsonl")
            captures = {original: Captures(read, directory) for original, directory in case["native_captures"].items()}
            native_sequences = []
            for record in native:
                unsigned = dict(record)
                sha = unsigned.pop("sha256")
                assert digest(json.dumps(unsigned, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()) == sha
                c = record["input_capture"]
                native_sequences.append(captures[c["directory"]].sequence(c))
            replies_by_arm, results = [], []
            for arm in ("0-off", "1-on", "2-on", "3-off"):
                directory = prefix + "/" + arm
                result = load(directory + "/result.json")
                assert result["complete"] is True and result["exit"] == 0 and result["error"] is None
                replies = lines(directory + "/replies.jsonl")
                assert len(replies) == len(result["requests"])
                expected_count = 152 if case["positive"] and arm.endswith("on") else len(native)
                assert len(replies) == expected_count
                genome_rows = []
                for line in read(directory + "/stderr.log").splitlines():
                    try:
                        row = json.loads(line)
                    except ValueError:
                        continue
                    if row.get("kind") == "genome":
                        genome_rows.append(row)
                assert len(genome_rows) == 1 and exact(genome_rows[0], result["genome"])
                current = Captures(read, directory + "/recaptured")
                for i, (reply, receipt) in enumerate(zip(replies, result["requests"])):
                    identity = [native[i]["decision"][k] for k in ("turn", "frame")]
                    assert receipt["request"] == identity and type(reply["turn"]) is int and reply["turn"] == identity[0]
                    assert [reply["decision"][k] for k in ("turn", "frame")] == identity
                    assert current.sequence(reply["input_capture"]) == native_sequences[i]
                    different = case["positive"] and arm.endswith("on") and i == len(replies)-1
                    assert exact(payload(native[i]), payload(reply)) is not different
                    total_reads += len(native_sequences[i])
                total_queries += len(replies)
                replies_by_arm.append(replies)
                results.append(result)
            for off, on in ((0, 1), (3, 2)):
                paired_controls(results[off], results[on])
            for a, b in ((0, 3), (1, 2)):
                assert exact([payload(r) for r in replies_by_arm[a]], [payload(r) for r in replies_by_arm[b]])
            for repeat, (off, on) in enumerate(((0, 1), (3, 2)), 1):
                index = len(replies_by_arm[on])-1
                pair = load(f"pairs/{case['name']}-repeat-{repeat}.json")
                assert pair["complete"] is True and pair["request"] == results[on]["requests"][index]["request"]
                assert len(pair["reads"]) == 1
                entry = pair["reads"][0]
                assert entry["complete"] is True
                assert (entry["sha256"], entry["bytes"]) == native_sequences[index][-1]
                assert entry["ordered_input_read_sha256"] == [h for h, n in native_sequences[index]]
                for a, arm_index in zip(entry["arms"], (off, on)):
                    r = results[arm_index]
                    assert a["exit"] == r["exit"] and a["argv"] == r["command"]
                    assert exact(a["genome"], r["genome"]) and exact(a["payload"], payload(replies_by_arm[arm_index][index]))
            if case["positive"]:
                assert results[1]["requests"][-1]["request"] == [66, 0]
                before = replies_by_arm[0][151]["orders"]
                after = replies_by_arm[1][151]["orders"]
                expected = json.loads(json.dumps(before))
                swaps = {65536: ("UNIT_BUILDER", "BUILDING_LIBRARY"), 196610: ("BUILDING_LIBRARY", "UNIT_BUILDER")}
                seen = set()
                for order in expected:
                    if order["kind"] == "produce" and order["subject"] in swaps:
                        old, new = swaps[order["subject"]]
                        assert order["verb"] == old
                        seen.add(order["subject"])
                        order["verb"] = new
                assert seen == set(swaps) and exact(expected, after)
                decision = json.loads(json.dumps(replies_by_arm[0][151]["decision"]))
                seen = set()
                for action in decision["native_actions"]:
                    if action["type"] == "produce" and action["city"] in (50, 52):
                        old, new = ({"unit": "builder"}, {"building": "library"}) if action["city"] == 50 else ({"building": "library"}, {"unit": "builder"})
                        assert action["item"] == old
                        action["item"] = new
                        seen.add(action["city"])
                assert seen == {50, 52} and exact(decision, replies_by_arm[1][151]["decision"])
        assert total_queries == 1390
        print(json.dumps({"verified": True, "queries": total_queries, "actual_input_reads": total_reads,
                          "scope": "Repeated offline proposals only; no host execution or strength result."}, indent=2))


if __name__ == "__main__":
    verify(Path(sys.argv[1]) if len(sys.argv) > 1 else Path(__file__).with_name("2026-11-06-parallel-library-backlog.native-queries.tar.xz"))
