"""Verify the retained diagnostic evidence; never access the native game."""
import gzip
import hashlib
import json
from pathlib import Path

here = Path(__file__).resolve().parent
repo = here.parents[2]
read = lambda name: json.loads(gzip.decompress((here / name).read_bytes()))
v = read("validation.json.gz")
assert v["red"]["qualified"] and v["red"]["returncode"] != 0
assert "export the signed native Favor rate" in v["red_log"]
assert v["red"]["tests_sha256"] == v["green"]["test_sha256"]
assert v["green"]["runtime"] == "Lua 5.1"
assert len(v["green"]["tests"]) == 104
assert all(t["returncode"] == 0 for t in v["green"]["tests"])
assert len(v["additional"]["syntax"]) == 111
assert all(t["ok"] for t in v["additional"]["syntax"])
assert v["additional"]["unchanged_findings"] == 169
assert v["source_change_only_this_block"]
source = (repo / "tools/civ6_control/mod/CivvisControlAgent.lua").read_text()
assert v["added_block"] in source
test = repo / "tools/civ6_control/mod/favor_readback_test.lua"
assert hashlib.sha256(test.read_bytes()).hexdigest() == v["green"]["test_sha256"]
api = "\n".join(v["local_engine_api_proof"]["lines"].values())
assert "localPlayer:GetFavorPerTurn()" in api
assert "localPlayer:GetFavorPerTurnToolTip()" in api
p = read("native-loss.json.gz")
assert p["registration"]["run"] == "civvis-20261026T014409Z"
assert p["registration"]["source"]["correction_readback"]
assert [s["turn"] for s in p["timeline"]] == list(range(1, 240))
assert len(p["congress"]) == 72
assert sum(p["event_kinds"][k] for k in p["event_kinds"] if k.startswith("wc_")) == 72
turn = {s["turn"]: s for s in p["timeline"]}
assert turn[180]["favor"] == 227 and turn[194]["favor"] == 0
assert all(s["favor_per_turn"] is None and s["favor_sources"] is None for s in p["timeline"])
assert turn[239]["gold"] == 0 and turn[239]["gold_per_turn"] == -243.797
assert turn[239]["active_routes"] == 16 and turn[239]["active_route_gold"] == 0
assert any(c["original_owner"] == 1 for c in turn[220]["held_original_capitals"])
ballot = [e for e in p["congress"] if e["kind"] == "wc_ballot_verdict"
          and e.get("resolution") == "WC_RES_DIPLOVICTORY" and e["turn"] == 239]
assert len(ballot) == 1
b = ballot[0]
assert b["favor_at_ballot"] == 0 and b["recorded"] == 1
assert b["option_recorded"] == 2 and b["target_recorded"] == "1"
assert b["registered"] and b["count_matches"] and b["option_matches"] and b["target_matches"]
last = [r for e in p["congress"] if e["kind"] == "wc_outcome" and e["turn"] == 239
        for r in e["resolutions"] if r["type"] == "WC_RES_DIPLOVICTORY"]
assert len(last) == 1 and last[0]["a"] == 7 and last[0]["b"] == 5
assert last[0]["chosen"] == "A" and last[0]["target"] == "1"
assert p["outcomes"] == [p["registration"]["outcome"]]
assert not p["outcomes"][0]["won"]
print("Verified native loss ledger, complete Congress records, genuine RED,104 GREEN tests and111 Lua5.1 syntax checks.")
