#!/usr/bin/env python3
"""Play one seed through a spectator and write down when everything happened.

The recording is a single take, so the camera has to be in the right place
*before* a launch rather than after it — a launch flight lasts under four
seconds and at Lightning pace that is a dozen turns. The server is deterministic
per seed and `/new` reproduces a CLI-started game exactly (verified on seed
2003: same six civs), so a dry run at full speed buys an exact schedule for the
take.

Usage: timeline.py <seed> [port]
Writes timeline-<seed>.json next to the seedsearch output.
"""
import json
import os
import socket
import subprocess
import sys
import time
import urllib.request

RIG = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(os.path.dirname(RIG), "seedsearch")
NEW = {"force": True, "paused": True, "num_players": 6,
       "map_script": "grand_canals_2", "map_topology": "planet",
       "game_speed": "online", "max_turns": 500}


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
        f"http://127.0.0.1:{port}{path}", data=json.dumps(body).encode(), method="POST")
    with urllib.request.urlopen(request, timeout=15) as response:
        return response.read()


def state(port):
    with urllib.request.urlopen(f"http://127.0.0.1:{port}/state", timeout=20) as response:
        return json.load(response)


def main():
    seed = int(sys.argv[1])
    os.makedirs(OUT, exist_ok=True)
    port = free_port(int(sys.argv[2]) if len(sys.argv) > 2 else 9400)
    log = open(os.path.join(OUT, f"timeline-{seed}.log"), "w")
    child = subprocess.Popen(
        [os.path.join(RIG, "civvis"), "play", "--spectate", "--players", "6",
         "--map", "grand_canals_2", "--shape", "planet", "--speed", "online",
         "--turns", "500", "--seed", str(seed), "--port", str(port), "--no-open"],
        cwd=RIG, stdout=log, stderr=subprocess.STDOUT)
    events, projects, first_expedition = [], {}, None
    try:
        for _ in range(120):
            time.sleep(0.5)
            owners = subprocess.run(["/usr/sbin/lsof", "-nP", f"-iTCP:{port}",
                                     "-sTCP:LISTEN", "-t"], capture_output=True, text=True)
            pids = [line for line in owners.stdout.split() if line]
            if pids:
                if str(child.pid) not in pids:
                    raise RuntimeError(f"port {port} owned by {pids}, not {child.pid}")
                break
        else:
            raise RuntimeError("server never listened")

        post(port, "/new", dict(NEW, seed=seed))
        time.sleep(3)
        opening = state(port)
        civs = {p["id"]: p.get("civ") for p in opening.get("players", [])}
        print(f"seed {seed} port {port} civs {[civs[i] for i in sorted(civs) if civs[i]][:6]}",
              flush=True)
        post(port, "/pace", {"ms": 0, "paused": False})

        victory = None
        while victory is None:
            time.sleep(0.4)
            try:
                snapshot = state(port)
            except Exception:                            # noqa: BLE001
                continue
            turn = snapshot.get("turn")
            for player in snapshot.get("players", []):
                if player.get("is_minor") or player.get("is_barbarian"):
                    continue
                civ = player.get("civ")
                held = set(projects.setdefault(civ, []))
                for project in player.get("science_projects") or []:
                    if project not in held:
                        projects[civ].append(project)
                        events.append({"turn": turn, "civ": civ, "project": project})
                        print(f"  turn {turn}: {civ} completed {project}", flush=True)
                distance = player.get("exoplanet_distance") or 0
                if distance and first_expedition is None:
                    first_expedition = {"turn": turn, "civ": civ}
            if snapshot.get("victory_type"):
                victory = {"type": snapshot["victory_type"],
                           "turn": snapshot.get("victory_turn") or turn,
                           "winner": civs.get(snapshot.get("winner"))}
                print("  victory", json.dumps(victory), flush=True)
            elif turn and turn >= 500:
                victory = {"type": "none", "turn": turn}
        result = {"seed": seed, "civs": [civs[i] for i in sorted(civs)],
                  "events": events, "victory": victory,
                  "first_expedition": first_expedition}
        with open(os.path.join(OUT, f"timeline-{seed}.json"), "w") as handle:
            json.dump(result, handle, indent=2)
        print(json.dumps(victory), flush=True)
    finally:
        child.kill()


if __name__ == "__main__":
    main()
