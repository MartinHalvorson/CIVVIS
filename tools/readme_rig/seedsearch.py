#!/usr/bin/env python3
"""Search seeds for a science victory *through the spectator server*, because
that is the only thing the recording can replay.

`civvis simulate` and `civvis play` do not play the same game for the same seed
— simulate takes a fixed civ roster off the top of the data file, play draws one
— so a seed found headless is not the seed the camera will see. The server is
deterministic per seed (verified: three `/new` at seed 777 gave the same six
civs), so it is the oracle.

Usage: seedsearch.py <first-seed> <count> [parallel]
"""
import json
import os
import shutil
import signal
import socket
import subprocess
import sys
import time
import urllib.request

RIG = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(os.path.dirname(RIG), "seedsearch")


def free_port(start):
    port = start
    while port < start + 400:
        with socket.socket() as probe:
            try:
                probe.bind(("127.0.0.1", port))
                return port
            except OSError:
                port += 1
    raise RuntimeError("no free port")


def post(port, path, body):
    request = urllib.request.Request(
        f"http://127.0.0.1:{port}{path}", data=body.encode(), method="POST")
    with urllib.request.urlopen(request, timeout=10) as response:
        return response.read()


def state(port):
    with urllib.request.urlopen(f"http://127.0.0.1:{port}/state", timeout=20) as response:
        return json.load(response)


def start(seed):
    """Launch one spectator and refuse to talk to it until the listener is ours.
    This box runs ~36 CIVVIS servers; a taken port makes `civvis play` panic into
    its own log and `curl` then happily returns somebody else's game."""
    port = free_port(9000 + (seed % 300))
    log = open(os.path.join(OUT, f"server-{seed}.log"), "w")
    child = subprocess.Popen(
        [os.path.join(RIG, "civvis"), "play", "--spectate", "--players", "6",
         "--map", "grand_canals_2", "--shape", "planet", "--speed", "online",
         "--turns", "500", "--seed", str(seed), "--port", str(port), "--no-open"],
        cwd=RIG, stdout=log, stderr=subprocess.STDOUT)
    for _ in range(120):
        time.sleep(0.5)
        owners = subprocess.run(["/usr/sbin/lsof", "-nP", f"-iTCP:{port}",
                                 "-sTCP:LISTEN", "-t"], capture_output=True, text=True)
        pids = [line for line in owners.stdout.split() if line]
        if pids:
            if str(child.pid) in pids:
                post(port, "/pace", '{"ms":0,"paused":false}')
                return {"seed": seed, "port": port, "pid": child.pid, "child": child}
            child.kill()
            raise RuntimeError(f"port {port} owned by {pids}, not {child.pid}")
        if child.poll() is not None:
            raise RuntimeError(f"server for seed {seed} died; see server-{seed}.log")
    child.kill()
    raise RuntimeError(f"server for seed {seed} never listened")


def main():
    first, count = int(sys.argv[1]), int(sys.argv[2])
    parallel = int(sys.argv[3]) if len(sys.argv) > 3 else 5
    os.makedirs(OUT, exist_ok=True)
    pending = list(range(first, first + count))
    live, results = [], []
    results_path = os.path.join(OUT, "results.jsonl")

    def record(entry):
        results.append(entry)
        with open(results_path, "a") as handle:
            handle.write(json.dumps(entry) + "\n")
        print(json.dumps(entry), flush=True)

    try:
        while pending or live:
            while pending and len(live) < parallel:
                seed = pending.pop(0)
                try:
                    live.append(start(seed))
                    print(f"started seed {seed} on port {live[-1]['port']}", flush=True)
                except Exception as err:               # noqa: BLE001
                    record({"seed": seed, "error": str(err)})
            time.sleep(20)
            for game in list(live):
                try:
                    snapshot = state(game["port"])
                except Exception:                       # noqa: BLE001
                    continue
                victory = snapshot.get("victory_type")
                turn = snapshot.get("turn")
                if victory:
                    winner = snapshot.get("winner")
                    civs = {player["id"]: player.get("civ")
                            for player in snapshot.get("players", [])}
                    launches = {}
                    for player in snapshot.get("players", []):
                        if player.get("is_minor") or player.get("is_barbarian"):
                            continue
                        launches[player.get("civ")] = len(player.get("science_projects") or [])
                    record({"seed": game["seed"], "victory": victory, "turn": turn,
                            "winner": civs.get(winner, winner), "launches": launches})
                    game["child"].kill()
                    live.remove(game)
                elif turn and turn >= 500:
                    record({"seed": game["seed"], "victory": "none", "turn": turn})
                    game["child"].kill()
                    live.remove(game)
                else:
                    print(f"  seed {game['seed']} turn {turn}", flush=True)
    finally:
        for game in live:
            try:
                game["child"].kill()
            except Exception:                           # noqa: BLE001
                pass
    print("done", json.dumps(results), flush=True)


if __name__ == "__main__":
    main()
