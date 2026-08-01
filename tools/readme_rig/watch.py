#!/usr/bin/env python3
"""Poll every live search game fast enough to see its result.

A finished spectator holds its result for ten seconds and then starts the next
game by itself, so a twenty-second poll reads the *following* game's turn 14 and
the outcome is simply gone. Two seconds is inside the countdown.

Usage: watch.py <port> [port...]
"""
import json
import os
import sys
import time
import urllib.request

OUT = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "seedsearch")


def state(port):
    with urllib.request.urlopen(f"http://127.0.0.1:{port}/state", timeout=15) as response:
        return json.load(response)


def main():
    ports = [int(p) for p in sys.argv[1:]]
    seen = {}
    while ports:
        for port in list(ports):
            try:
                snapshot = state(port)
            except Exception:                            # noqa: BLE001
                continue
            if snapshot.get("victory_type") and seen.get(port) != snapshot.get("seed"):
                seen[port] = snapshot.get("seed")
                civs = {p["id"]: p.get("civ") for p in snapshot.get("players", [])}
                entry = {
                    "seed": snapshot.get("seed"), "port": port,
                    "victory": snapshot["victory_type"],
                    "turn": snapshot.get("victory_turn") or snapshot.get("turn"),
                    "winner": civs.get(snapshot.get("winner")),
                    "projects": {p.get("civ"): p.get("science_projects") or []
                                 for p in snapshot.get("players", [])
                                 if not p.get("is_minor") and not p.get("is_barbarian")},
                }
                with open(os.path.join(OUT, "watched.jsonl"), "a") as handle:
                    handle.write(json.dumps(entry) + "\n")
                print(json.dumps(entry), flush=True)
                ports.remove(port)
        time.sleep(2)


if __name__ == "__main__":
    main()
