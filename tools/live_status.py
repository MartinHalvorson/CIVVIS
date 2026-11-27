#!/usr/bin/env python3
"""Where CIVVIS stands against Civilization VI now, read from the live ledger.

    python3 tools/live_status.py                       the state, as text
    python3 tools/live_status.py --since 2026-11-22    games launched on or after a UTC day (`all`: every game)
    python3 tools/live_status.py --tag civvis-t8-      one tournament's games (repeatable)
    python3 tools/live_status.py --json                the same, for a page (`beta/publish.sh` writes status.json)
    python3 tools/live_status.py --markdown            the same, for a PR body or an issue
    python3 tools/live_status.py --ledger results.jsonl   a copy of the ledger instead of fetching one

⭐ THE DIRECTION DOCS CARRY NO NUMBER THIS PRINTS (`docs/ROADMAP.md`,
objective 8). `docs/ROADMAP.md` said for three months that the front line was
making Settler repeatable, while the live seat was winning one Deity game in
five; `README.md` said "Deity is fairly beat". Both were true once. A number
written into prose is true the day it is written, so the prose says what to
run, and this prints what is true today.

Where the numbers come from. The live seat appends every run it plays to the
`ledger` branch (`tools/civ6_ladder.py publish-run`), one line per run in
`results.jsonl`. By default this fetches that branch (`git fetch origin
ledger`, read from `FETCH_HEAD`); when the fetch fails it reads the file from
GitHub's raw address, and when that fails too, the clone's
`refs/remotes/origin/ledger` as it is. `--ref` reads a ref without fetching,
`--ledger` reads a file.

How a game is counted, exactly as the site's Tournaments page counts it
(`beta/home_rankings.tournament_series`, which `beta/tournaments_page.py`
draws; `tools/test_live_status.py` holds the two to one answer):

- A game is its root tag: a `<tag>-contN` continuation from an autosave is
  the same game.
- A game counts once, by its best run, in the order `OUTCOMES` lists: a win,
  then our elimination (`defeat`), a rival's victory, a game the harness
  conceded (`abandoned`), and `unfinished`.
- A game whose every run is `unfinished` (a launch that never reached a
  result) is not a game. It still occupied the host, so it counts towards
  the host's busy time below.
- A win is `outcome == "win"`; every other counted game is a loss.

By default only the benchmark setup counts towards the win rates: one CIVVIS
agent and five Firaxis AI (`players == 6`, a single civ in `civ`), on
`Continents.lua`. `--any-setup` counts every game. The window is the last
seven UTC days of the ledger, by the day a game was launched (the stamp in its
tag); `--since` moves it. The day tables use the day a game ENDED (`utc`),
as the Tournaments page's benchmark does.

Equal odds ("par") is one in the number of players, the share of wins a seat
would take if every seat were as strong. The interval is Wilson's at 95%, as
the Tournaments page draws it.

Which genes the live seat plays (the last section). `src/ai/advanced/genes.rs`
is the registry; `docs/gene_ledger.json`'s `deployment_genome` is what the
ledger turns on (compiled into `civvis_orders`, `gene_ledger.rs`); host-only
and repair rows are on unless the ledger holds them off; and
`deploy/live-force-on.txt` is the list the game supervisor
(`tools/ops/civvis-game-supervisor.sh`) reads at each batch boundary and
passes to every game as `--with` (`CIVVIS_WITH` → `civ6_civvis_climb.py
--with` → `civ6_play.py --civvis-with` → `civvis_orders --with`), unless the
host has a local override (`~/.civvis-live-force-on`) or a `CIVVIS_WITH`
environment. So what a game REALLY played is read back from the game: each
finished run's `summary.json` on the ledger branch records the `forced` list
it was launched with and the decider's own `genome_treatments`. This prints
the newest one beside the file, and says when they differ.
"""

from __future__ import annotations

import argparse
import datetime as dt
import gzip
import json
import math
import os
import re
import subprocess
import sys
import urllib.request
import zlib
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parent

RAW_URL = "https://raw.githubusercontent.com/MartinHalvorson/CIVVIS/ledger/results.jsonl"
LEDGER_BRANCH = "ledger"
RESULTS = "results.jsonl"
TRACKING_REF = "refs/remotes/origin/ledger"

#: A run's ending, best first (`beta/home_rankings.OUTCOMES`): a game reloaded
#: from its autosave is several runs, and it ended the best way any of them did.
OUTCOMES = ["win", "defeat", "rival_victory", "abandoned", "unfinished"]

#: Civilization VI's ladder, lowest first; a rung's place is its level number.
RUNGS = ["settler", "chieftain", "warlord", "prince", "king", "emperor", "immortal", "deity"]

#: Civilization VI's victory types under the names its own screens use.
VICTORIES = {
    "VICTORY_TECHNOLOGY": "Science",
    "VICTORY_CULTURE": "Culture",
    "VICTORY_CONQUEST": "Domination",
    "VICTORY_RELIGIOUS": "Religious",
    "VICTORY_DIPLOMATIC": "Diplomatic",
    "VICTORY_SCORE": "Score",
}

#: The benchmark setup the win rates count by default.
SETUP = {"players": 6, "map": "Continents.lua"}
DEFAULT_DAYS = 7
DEFAULT_LEVEL = "deity"
Z95 = 1.96
STAMP = re.compile(r"(\d{4})(\d{2})(\d{2})T(\d{2})(\d{2})(\d{2})Z")
FAMILY = re.compile(r"\d{8}T\d{6}Z.*$")

GENES_RS = REPO / "src" / "ai" / "advanced" / "genes.rs"
GENE_LEDGER = REPO / "docs" / "gene_ledger.json"
FORCE_FILE = REPO / "deploy" / "live-force-on.txt"
MOD_ARMS_FILE = REPO / "deploy" / "live-mod-arms.txt"


# ─── Reading the ledger ──────────────────────────────────────────────────────


def parse_rows(text: str) -> list[dict]:
    """Every JSON object line of a `results.jsonl`; anything else is skipped."""
    rows = []
    for line in text.splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            row = json.loads(line)
        except ValueError:
            continue
        if isinstance(row, dict):
            rows.append(row)
    return rows


def _git(repo: Path, *args: str, timeout: float = 120.0) -> subprocess.CompletedProcess:
    return subprocess.run(["git", "-C", str(repo), *args], capture_output=True,
                          timeout=timeout, check=False,
                          env={**os.environ, "GIT_NO_LAZY_FETCH": "1", "GIT_TERMINAL_PROMPT": "0"})


def show(repo: Path, ref: str, path: str) -> bytes | None:
    try:
        shown = _git(repo, "show", f"{ref}:{path}")
    except (OSError, subprocess.SubprocessError):
        return None
    return shown.stdout if shown.returncode == 0 else None


def resolve(repo: Path, ref: str) -> str | None:
    try:
        found = _git(repo, "rev-parse", "--verify", "--quiet", f"{ref}^{{commit}}")
    except (OSError, subprocess.SubprocessError):
        return None
    return found.stdout.decode().strip() or None if found.returncode == 0 else None


def load(args) -> tuple[list[dict], dict]:
    """The ledger's rows and where they came from: `{"source", "ref"}`, where
    `ref` is a commit holding the ledger tree (for the run summaries), or None."""
    repo = Path(args.repo)
    if args.ledger:
        text = sys.stdin.read() if args.ledger == "-" else Path(args.ledger).read_text(encoding="utf-8")
        return parse_rows(text), {"source": f"file {args.ledger}", "ref": None}
    if args.ref:
        commit = resolve(repo, args.ref)
        blob = show(repo, args.ref, RESULTS) if commit else None
        if blob is None:
            raise SystemExit(f"live_status: {args.ref} has no {RESULTS} in {repo}")
        return parse_rows(blob.decode("utf-8", "replace")), {"source": f"{args.ref} {commit[:8]}", "ref": commit}
    notes = []
    try:
        fetched = _git(repo, "fetch", "--quiet", "origin", LEDGER_BRANCH, timeout=args.fetch_timeout)
        commit = resolve(repo, "FETCH_HEAD") if fetched.returncode == 0 else None
        if commit:
            blob = show(repo, commit, RESULTS)
            if blob is not None:
                return parse_rows(blob.decode("utf-8", "replace")), {
                    "source": f"origin/{LEDGER_BRANCH} {commit[:8]} (fetched)", "ref": commit}
        notes.append(f"git fetch origin {LEDGER_BRANCH} failed: "
                     f"{fetched.stderr.decode(errors='replace').strip()[:200]}")
    except (OSError, subprocess.SubprocessError) as error:
        notes.append(f"git fetch origin {LEDGER_BRANCH} failed: {error}")
    try:
        with urllib.request.urlopen(RAW_URL, timeout=args.fetch_timeout) as answer:
            text = answer.read().decode("utf-8", "replace")
        for note in notes:
            print(f"live_status: {note}; read {RAW_URL}", file=sys.stderr)
        return parse_rows(text), {"source": RAW_URL, "ref": None}
    except OSError as error:
        notes.append(f"{RAW_URL} failed: {error}")
    commit = resolve(repo, TRACKING_REF)
    blob = show(repo, commit, RESULTS) if commit else None
    if blob is not None:
        for note in notes:
            print(f"live_status: {note}", file=sys.stderr)
        print(f"live_status: reading {TRACKING_REF} as this clone last fetched it; it may be stale",
              file=sys.stderr)
        return parse_rows(blob.decode("utf-8", "replace")), {
            "source": f"{TRACKING_REF} {commit[:8]} (NOT refreshed)", "ref": commit}
    raise SystemExit("live_status: no ledger could be read:\n  " + "\n  ".join(notes))


# ─── Counting games ──────────────────────────────────────────────────────────


def game_key(row: dict) -> str | None:
    """The game a run belongs to: its root tag, without a `-contN` resume."""
    key = row.get("game") or row.get("tag")
    return re.sub(r"-cont\d+$", "", key) if isinstance(key, str) and key else None


def level(difficulty) -> int | None:
    """A difficulty's level number: `DIFFICULTY_DEITY` is 8."""
    rung = str(difficulty or "").removeprefix("DIFFICULTY_").lower()
    return RUNGS.index(rung) + 1 if rung in RUNGS else None


def level_name(lv: int) -> str:
    return f"Lv {lv} {RUNGS[lv - 1].title()}"


def parse_utc(text) -> dt.datetime | None:
    try:
        return dt.datetime.strptime(str(text or ""), "%Y-%m-%dT%H:%M:%SZ").replace(tzinfo=dt.timezone.utc)
    except ValueError:
        return None


def launched_day(key: str, runs: list[dict]) -> str:
    """The UTC day a game was launched: the stamp in its tag, else the day of
    its earliest recorded run (`beta/home_rankings.began`)."""
    found = STAMP.search(key)
    if found:
        return f"{found[1]}-{found[2]}-{found[3]}"
    return min((str(row["utc"])[:10] for row in runs if row.get("utc")), default="")


def family(key: str) -> str:
    """A tag's family, its stamp and anything after cut off: `civvis-t8-`."""
    return FAMILY.sub("", key) or key


def games(rows: list[dict]) -> list[dict]:
    """Every counted game, oldest key first: `{"key", "row", "runs",
    "launched", "family"}`, where `row` is its best run."""
    grouped: dict[str, list[dict]] = {}
    for row in rows:
        key = game_key(row)
        if key and row.get("outcome") in OUTCOMES:
            grouped.setdefault(key, []).append(row)
    out = []
    for key in sorted(grouped):
        runs = grouped[key]
        best = min(runs, key=lambda row: OUTCOMES.index(row["outcome"]))
        if best["outcome"] == "unfinished":
            continue
        out.append({"key": key, "row": best, "runs": runs,
                    "launched": launched_day(key, runs), "family": family(key)})
    return out


def benchmark_setup(row: dict) -> bool:
    civ = str(row.get("civ") or "")
    return (all(row.get(field) == value for field, value in SETUP.items())
            and bool(civ) and "+" not in civ)


def wilson(wins: int, n: int, z: float = Z95) -> tuple[float, float] | None:
    """Wilson's 95% interval of a win rate, as fractions."""
    if not n:
        return None
    p = wins / n
    d = 1 + z * z / n
    centre = (p + z * z / (2 * n)) / d
    half = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / d
    return max(0.0, centre - half), min(1.0, centre + half)


def percentiles(values: list[float], points=(10, 25, 50, 75, 90)) -> dict | None:
    """Linear-interpolated percentiles (numpy's default), with `n`."""
    values = sorted(v for v in values if isinstance(v, (int, float)))
    if not values:
        return None
    out: dict = {"n": len(values)}
    for p in points:
        at = (len(values) - 1) * p / 100
        low = math.floor(at)
        high = min(low + 1, len(values) - 1)
        value = values[low] + (values[high] - values[low]) * (at - low)
        out[f"p{p}"] = round(value, 3)
    return out


def record(counted: list[dict]) -> dict:
    """Wins, games, rate, interval, par and the verdict against par."""
    n = len(counted)
    wins = sum(1 for game in counted if game["row"].get("outcome") == "win")
    pars = [1 / game["row"]["players"] for game in counted
            if isinstance(game["row"].get("players"), int) and game["row"]["players"] > 0]
    par = sum(pars) / len(pars) if pars else None
    interval = wilson(wins, n)
    verdict = None
    if interval and par is not None:
        verdict = "above par" if interval[0] > par else "below par" if interval[1] < par else "par within interval"
    return {
        "wins": wins, "games": n,
        "rate": round(wins / n, 4) if n else None,
        "interval": [round(interval[0], 4), round(interval[1], 4)] if interval else None,
        "par": round(par, 4) if par is not None else None,
        "verdict": verdict,
    }


def victory_name(row: dict) -> str:
    return VICTORIES.get(row.get("victory"), str(row.get("victory") or "unknown"))


def loss_cause(row: dict) -> str:
    outcome = row.get("outcome")
    if outcome == "rival_victory":
        return f"rival {victory_name(row)}"
    if outcome == "defeat":
        return "eliminated"
    return "conceded"


def tally(labels: list[str]) -> list[list]:
    counts: dict[str, int] = {}
    for label in labels:
        counts[label] = counts.get(label, 0) + 1
    return [[label, count] for label, count in sorted(counts.items(), key=lambda item: (-item[1], item[0]))]


def focus(counted: list[dict], lv: int, victory_settings: dict | None = None) -> dict:
    """The one level's detail: how it loses, when games end, how far behind."""
    at = [game for game in counted if level(game["row"].get("difficulty")) == lv]
    won = [game["row"] for game in at if game["row"]["outcome"] == "win"]
    lost = [game["row"] for game in at if game["row"]["outcome"] != "win"]
    turns: dict[str, dict | None] = {}
    for name in sorted({victory_name(row) for row in lost if row["outcome"] == "rival_victory"}):
        turns[f"rival {name} wins"] = percentiles(
            [row.get("turns") for row in lost if row["outcome"] == "rival_victory" and victory_name(row) == name])
    for name in sorted({victory_name(row) for row in won}):
        turns[f"our {name} wins"] = percentiles([row.get("turns") for row in won if victory_name(row) == name])
    turns["our eliminations"] = percentiles([row.get("turns") for row in lost if row["outcome"] == "defeat"])

    def ratios(rows):
        return percentiles([row["score"] / row["rival_best"] for row in rows
                            if isinstance(row.get("score"), (int, float))
                            and isinstance(row.get("rival_best"), (int, float)) and row["rival_best"] > 0])

    families: dict[str, list[dict]] = {}
    for game in at:
        families.setdefault(game["family"], []).append(game)
    out = {
        "level": lv,
        "name": level_name(lv),
        "record": record(at),
        "wins_by_victory": tally([victory_name(row) for row in won]),
        "losses": tally([loss_cause(row) for row in lost]),
        "turns": turns,
        "score_ratio": {"wins": ratios(won), "losses": ratios(lost)},
        "by_family": {name: record(group) for name, group in sorted(families.items())},
        "by_day": by_day(at),
    }
    if victory_settings is not None:
        known = {key: value for key, value in victory_settings.items() if value is not None}
        out["diplomatic"] = {
            "off": record([game for game in at if known.get(game["key"]) is False]),
            "on": record([game for game in at if known.get(game["key"]) is True]),
            "unknown": sum(1 for game in at if game["key"] not in known),
        }
    return out


def by_day(counted: list[dict]) -> list[dict]:
    """Win/games by the UTC day a game ended."""
    days: dict[str, list[dict]] = {}
    for game in counted:
        days.setdefault(str(game["row"].get("utc") or "")[:10] or "unknown", []).append(game)
    return [{"day": day, **record(group)} for day, group in sorted(days.items())]


def throughput(rows: list[dict], since: str | None, through: dt.datetime | None) -> dict:
    """Finished games per UTC day and the share of wall time no game ran.

    Busy time is the union of every launch's `[utc - seconds, utc]`, finished
    or not (a launch that never reached a board still held the host). Only
    launches that recorded `seconds` can be placed, so the measured span starts
    at the first of them; a day's idle share is over the part of it inside the
    span."""
    finished = games(rows)
    per_day: dict[str, int] = {}
    for game in finished:
        day = str(game["row"].get("utc") or "")[:10]
        if day and (not since or day >= since):
            per_day[day] = per_day.get(day, 0) + 1
    spans = []
    for row in rows:
        end = parse_utc(row.get("utc"))
        seconds = row.get("seconds")
        if end is None or not isinstance(seconds, (int, float)) or seconds <= 0:
            continue
        start = end - dt.timedelta(seconds=float(seconds))
        if since and end.strftime("%Y-%m-%d") < since:
            continue
        spans.append((start, end))
    spans.sort()
    merged: list[list[dt.datetime]] = []
    for start, end in spans:
        if merged and start <= merged[-1][1]:
            merged[-1][1] = max(merged[-1][1], end)
        else:
            merged.append([start, end])
    out: dict = {"finished_per_day": [[day, count] for day, count in sorted(per_day.items())],
                 "launches_timed": len(spans)}
    if not merged:
        out.update({"idle_share": None, "span": None, "idle_by_day": []})
        return out
    first = merged[0][0]
    if since:
        first = max(first, dt.datetime.strptime(since, "%Y-%m-%d").replace(tzinfo=dt.timezone.utc))
    last = max(end for _, end in merged)
    if through is not None:
        last = max(last, through)
    wall = (last - first).total_seconds()
    busy = sum((min(end, last) - max(start, first)).total_seconds()
               for start, end in merged if end > first)
    days = []
    day = first.replace(hour=0, minute=0, second=0)
    while day < last:
        nxt = day + dt.timedelta(days=1)
        lo, hi = max(day, first), min(nxt, last)
        covered = (hi - lo).total_seconds()
        used = sum(max(0.0, (min(end, hi) - max(start, lo)).total_seconds()) for start, end in merged)
        if covered > 0:
            days.append([day.strftime("%Y-%m-%d"), round(1 - used / covered, 4), round(covered / 3600, 2)])
        day = nxt
    out.update({
        "idle_share": round(1 - busy / wall, 4) if wall > 0 else None,
        "span": [first.strftime("%Y-%m-%dT%H:%M:%SZ"), last.strftime("%Y-%m-%dT%H:%M:%SZ")],
        "idle_by_day": days,
    })
    return out


# ─── The diplomatic setting, read from each game's own seat event ────────────


def seat_victories(blob: bytes) -> dict | None:
    """The `victories` map of the first `seat` event in a gzipped events log,
    decompressing only as far as that event."""
    inflate = zlib.decompressobj(16 + zlib.MAX_WBITS)
    pending = b""
    for at in range(0, len(blob), 1 << 16):
        try:
            pending += inflate.decompress(blob[at:at + (1 << 16)])
        except zlib.error:
            return None
        *lines, pending = pending.split(b"\n")
        for line in lines:
            if b'"seat"' not in line:
                continue
            try:
                event = json.loads(line)
            except ValueError:
                continue
            if isinstance(event, dict) and event.get("kind") == "seat":
                victories = event.get("victories")
                return victories if isinstance(victories, dict) else None
    return None


def diplomatic_settings(repo: Path, ref: str | None, runs_dir: Path | None,
                        keys: list[str]) -> dict[str, bool | None]:
    """Whether each game was played with the Diplomatic victory on, from its
    root run's events: a local runs directory (`live_ledger.py pull` layout)
    first, else the ledger tree at `ref` through one `git cat-file --batch`."""
    found: dict[str, bool | None] = {}
    missing = []
    for key in keys:
        path = runs_dir / key / "events.jsonl.gz" if runs_dir else None
        if path is not None and path.is_file():
            victories = seat_victories(path.read_bytes())
            found[key] = None if victories is None else bool(victories.get("diplomatic"))
        else:
            missing.append(key)
    if not missing or not ref:
        return found
    try:
        batch = subprocess.Popen(["git", "-C", str(repo), "cat-file", "--batch"],
                                 stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL,
                                 env={**os.environ, "GIT_NO_LAZY_FETCH": "1"})
    except OSError:
        return found
    try:
        for key in missing:
            batch.stdin.write(f"{ref}:runs/{key}/events.jsonl.gz\n".encode())
            batch.stdin.flush()
            header = batch.stdout.readline().decode().split()
            if len(header) != 3 or header[1] != "blob":
                continue
            blob = batch.stdout.read(int(header[2]))
            batch.stdout.read(1)
            victories = seat_victories(blob)
            found[key] = None if victories is None else bool(victories.get("diplomatic"))
    finally:
        batch.stdin.close()
        batch.wait(timeout=30)
        batch.stdout.close()
    return found


# ─── The gene backlog and what the live seat really plays ────────────────────


def split_tags(text: str) -> list[str]:
    return [tag for tag in re.split(r"[,\s]+", text) if tag and not tag.startswith("#")]


def gene_backlog(registry_text: str | None, ledger: dict | None, force_text: str | None,
                 played: dict | None = None) -> dict:
    """Registry rows by kind, what is on by default, what the force-on file
    adds, the opt-ins nothing arms, and the armed set a game really played."""
    sys.path.insert(0, str(HERE))
    import genes as registry  # noqa: PLC0415 - stdlib-only sibling, read lazily

    if registry_text is None:
        return {"error": f"no registry at {GENES_RS.relative_to(REPO)}"}
    rows = registry.genes_from_text(registry_text)
    kinds: dict[str, int] = {}
    for row in rows:
        kinds[_kind(row)] = kinds.get(_kind(row), 0) + 1
    default_on = set(registry.deployment_genome(ledger or {}))
    known = {row.tag: row for row in rows}
    on_by_kind: dict[str, int] = {}
    for tag in default_on:
        if tag in known:
            on_by_kind[_kind(known[tag])] = on_by_kind.get(_kind(known[tag]), 0) + 1
    forced = split_tags(force_text) if force_text is not None else None
    out: dict = {
        "registry_rows": len(rows),
        "kinds": dict(sorted(kinds.items(), key=lambda item: -item[1])),
        "default_on": len(default_on & set(known)),
        "default_on_by_kind": dict(sorted(on_by_kind.items(), key=lambda item: -item[1])),
        "live_seat_default": len(registry.deployment_treatments(ledger or {}, rows)),
    }
    armed = set(forced or ())
    if forced is not None:
        out["force_file"] = {
            "tags": len(set(forced)),
            "opt_ins_it_adds": sum(1 for tag in set(forced) if tag in known and known[tag].opt_in
                                   and tag not in default_on),
            "already_default_on": sum(1 for tag in set(forced) if tag in default_on),
            "held_live_rows_it_restores": sum(1 for tag in set(forced) if tag in known and known[tag].live
                                              and tag not in default_on),
            "unknown": sorted(tag for tag in set(forced) if tag not in known),
        }
    out["armed_from"] = "deploy/live-force-on.txt" if forced is not None else None
    if played and isinstance(played.get("forced"), list):
        armed = set(played["forced"])
        out["armed_from"] = f"the forced list {played.get('tag')} played"
    out["opt_in_not_armed"] = sum(1 for row in rows if row.opt_in and row.tag not in default_on
                                  and row.tag not in armed)
    if played is not None:
        entry = dict(played)
        if forced is not None and isinstance(played.get("forced"), list):
            entry["vs_force_file"] = {
                "only_in_game": sorted(set(played["forced"]) - set(forced)),
                "only_in_file": sorted(set(forced) - set(played["forced"])),
            }
        out["played"] = entry
    return out


def _kind(row) -> str:
    return row.kind.removeprefix("Kind::").split("(")[0]


def raw_summary(tag: str, timeout: float = 120.0) -> bytes | None:
    """One run's `summary.json` from GitHub's raw address of the ledger branch."""
    try:
        with urllib.request.urlopen(RAW_URL.replace(RESULTS, f"runs/{tag}/summary.json"),
                                    timeout=timeout) as answer:
            return answer.read()
    except OSError:
        return None


def played_genome(repo: Path, ref: str | None, runs_dir: Path | None, rows: list[dict],
                  online: bool = False) -> dict | None:
    """What the newest finished game recorded it played: its `forced` list,
    `withheld` list and the decider's `genome_treatments`. Partial summaries
    (a run stopped before it wrote its own) carry none, so the newest run that
    has them is the one read: from a local runs directory, else the ledger
    tree at `ref`, else (`online`) GitHub's raw address."""
    candidates = sorted((row for row in rows if row.get("outcome") in OUTCOMES[:3] and row.get("tag")),
                        key=lambda row: str(row.get("utc") or ""), reverse=True)
    for row in candidates[:40 if (runs_dir or ref) else 5]:
        tag = row["tag"]
        blob = None
        path = runs_dir / tag / "summary.json" if runs_dir else None
        if path is not None and path.is_file():
            blob = path.read_bytes()
        elif ref:
            blob = show(repo, ref, f"runs/{tag}/summary.json")
        elif online:
            blob = raw_summary(tag)
        if blob is None:
            continue
        try:
            summary = json.loads(blob)
        except ValueError:
            continue
        if not isinstance(summary, dict) or ("forced" not in summary and "genome_treatments" not in summary):
            continue
        treatments = summary.get("genome_treatments") or {}
        return {
            "tag": tag,
            "utc": row.get("utc"),
            "forced": summary.get("forced"),
            "withheld": summary.get("withheld"),
            "mod_arms": summary.get("mod_arms"),
            "decider_forced": treatments.get("forced"),
            "decider_treatments": len(treatments["treatments"]) if isinstance(treatments.get("treatments"), list)
            else None,
        }
    return None


# ─── The whole status ────────────────────────────────────────────────────────


def status(rows: list[dict], *, since: str | None = "", tags: list[str] | None = None, any_setup: bool = False,
           focus_level: int = 8, victory_settings: dict | None = None, genes_section: dict | None = None,
           source: dict | None = None) -> dict:
    """Everything this prints, as one value. `since` is a `YYYY-MM-DD` UTC
    day, `None` for every game, or `""` for the default window."""
    counted_all = games(rows)
    through = max((parse_utc(row.get("utc")) for row in rows if parse_utc(row.get("utc"))), default=None)
    if since == "" and through is not None:
        since = (through - dt.timedelta(days=DEFAULT_DAYS - 1)).strftime("%Y-%m-%d")
        window = f"the last {DEFAULT_DAYS} UTC days of the ledger"
    elif since:
        window = f"launched on or after {since}"
    else:
        since = None
        window = "every game on the ledger"
    in_window = [game for game in counted_all if not since or game["launched"] >= since]
    if tags:
        in_window = [game for game in in_window if any(game["key"].startswith(tag) for tag in tags)]
    counted = in_window if any_setup else [game for game in in_window if benchmark_setup(game["row"])]
    seen = {game["key"] for game in counted_all}
    never: dict[str, list[dict]] = {}
    for row in rows:
        key = game_key(row)
        if key and key not in seen and row.get("outcome") == "unfinished":
            never.setdefault(key, []).append(row)
    unfinished = sum(1 for key, runs in never.items() if (not since or launched_day(key, runs) >= since)
                     and (not tags or any(key.startswith(tag) for tag in tags)))
    levels = sorted({lv for lv in (level(game["row"].get("difficulty")) for game in counted) if lv})
    return {
        "source": (source or {}).get("source"),
        "through": through.strftime("%Y-%m-%dT%H:%M:%SZ") if through else None,
        "window": {"since": since, "means": window, "tags": tags or [],
                   "setup": "any" if any_setup else "1 CIVVIS + 5 Firaxis AI, 6P Continents.lua"},
        "counting": {"games": len(counted), "outside_setup": len(in_window) - len(counted),
                     "launches_never_finished": unfinished},
        "levels": [{"level": lv, "name": level_name(lv),
                    **record([game for game in counted if level(game["row"].get("difficulty")) == lv])}
                   for lv in levels],
        "focus": focus(counted, focus_level, victory_settings),
        "throughput": throughput(rows, since, through),
        "genes": genes_section,
    }


# ─── Rendering ───────────────────────────────────────────────────────────────


def pct(value) -> str:
    return "-" if value is None else f"{100 * value:.0f}%"


def interval_text(rec: dict) -> str:
    return "-" if not rec.get("interval") else f"{pct(rec['interval'][0])}-{pct(rec['interval'][1])}"


def turns_text(p: dict | None) -> str:
    if not p:
        return "none"
    return (f"n={p['n']:<4} p10 {p['p10']:.0f}  p25 {p['p25']:.0f}  median {p['p50']:.0f}  "
            f"p75 {p['p75']:.0f}  p90 {p['p90']:.0f}")


def ratio_text(p: dict | None) -> str:
    if not p:
        return "none"
    return f"n={p['n']:<4} median {p['p50']:.2f}  (p25 {p['p25']:.2f}, p75 {p['p75']:.2f})"


def text(state: dict) -> str:
    f = state["focus"]
    out = [
        f"CIVVIS live status: {state['source']}, through {state['through']}",
        f"window: {state['window']['means']} ({state['window']['since'] or 'all'}); "
        f"setup: {state['window']['setup']}"
        + (f"; tags {', '.join(state['window']['tags'])}" if state['window']['tags'] else ""),
        f"counted: {state['counting']['games']} games (a game counts once, by its best run); "
        f"{state['counting']['outside_setup']} outside the setup; "
        f"{state['counting']['launches_never_finished']} launches never finished",
        "",
        "Win rate by level  (par = equal odds, 1 in players; 95% Wilson interval)",
        f"  {'level':<14}{'wins/games':>11}{'rate':>7}{'95%':>11}{'par':>6}  vs par",
    ]
    for rec in state["levels"]:
        out.append(f"  {rec['name']:<14}{rec['wins']:>5}/{rec['games']:<5}{pct(rec['rate']):>7}"
                   f"{interval_text(rec):>11}{pct(rec['par']):>6}  {rec['verdict'] or '-'}")
    rec = f["record"]
    out += ["", f"{f['name']}: {rec['wins']}/{rec['games']} = {pct(rec['rate'])} "
                f"(95% {interval_text(rec)}, par {pct(rec['par'])}, {rec['verdict'] or 'no games'})"]
    if "diplomatic" in f:
        d = f["diplomatic"]
        out.append(f"  Diplomatic victory off: {d['off']['wins']}/{d['off']['games']} = {pct(d['off']['rate'])} "
                   f"(95% {interval_text(d['off'])}); on: {d['on']['wins']}/{d['on']['games']} = "
                   f"{pct(d['on']['rate'])}; setting unknown: {d['unknown']}")
    else:
        out.append("  Diplomatic setting not read (add --victory-settings; it reads each game's seat event)")
    out.append("  by UTC day the game ended:")
    for day in f["by_day"]:
        out.append(f"    {day['day']}  {day['wins']:>3}/{day['games']:<4} {pct(day['rate']):>5}")
    out.append("  by tag family:")
    for name, fam in f["by_family"].items():
        out.append(f"    {name:<22} {fam['wins']:>3}/{fam['games']:<4} {pct(fam['rate']):>5}")
    out.append("  wins by victory: " + (", ".join(f"{name} {count}" for name, count in f["wins_by_victory"]) or "none"))
    losses = sum(count for _, count in f["losses"])
    out.append(f"  losses ({losses}): " + (", ".join(
        f"{name} {count} ({pct(count / losses)})" for name, count in f["losses"]) or "none"))
    out.append("  turn the game ended:")
    for name, p in f["turns"].items():
        out.append(f"    {name:<22} {turns_text(p)}")
    out.append("  our score / best rival's:")
    out.append(f"    {'in wins':<22} {ratio_text(f['score_ratio']['wins'])}")
    out.append(f"    {'in losses':<22} {ratio_text(f['score_ratio']['losses'])}")
    t = state["throughput"]
    out += ["", "Live host throughput (every finished game in the window, any setup)"]
    idle = {day: (share, hours) for day, share, hours in t["idle_by_day"]}
    shown = t["finished_per_day"][-14:]
    if len(shown) < len(t["finished_per_day"]):
        out.append(f"  ({len(t['finished_per_day']) - len(shown)} earlier days in --json)")
    for day, count in shown:
        share = idle.get(day)
        out.append(f"  {day}  {count:>4} finished" + (f"   idle {pct(share[0])} of {share[1]:.1f} h"
                                                       if share else ""))
    out.append(f"  idle share of wall time: {pct(t['idle_share'])} over "
               + (f"{t['span'][0]} to {t['span'][1]}" if t["span"] else "no timed launch")
               + f" ({t['launches_timed']} launches with a recorded duration)")
    g = state.get("genes")
    if g:
        out += ["", "Gene backlog"]
        if "error" in g:
            out.append(f"  {g['error']}")
        else:
            out.append(f"  registry rows: {g['registry_rows']} ("
                       + ", ".join(f"{kind} {count}" for kind, count in g["kinds"].items()) + ")")
            out.append(f"  default on (gene ledger deployment genome): {g['default_on']} ("
                       + ", ".join(f"{kind} {count}" for kind, count in g["default_on_by_kind"].items())
                       + f"); the live seat's default genome, host-only and repair rows included: "
                         f"{g['live_seat_default']}")
            ff = g.get("force_file")
            if ff:
                out.append(f"  deploy/live-force-on.txt: {ff['tags']} tags; {ff['opt_ins_it_adds']} opt-ins it adds, "
                           f"{ff['already_default_on']} already on by default, {ff['held_live_rows_it_restores']} "
                           f"held live rows it restores, {len(ff['unknown'])} not in the registry")
            out.append(f"  opt-in and not armed: {g['opt_in_not_armed']} (neither on by default nor in "
                       f"{g.get('armed_from') or 'any force-on list'})")
            played = g.get("played")
            if played:
                vs = played.get("vs_force_file")
                forced = played.get("forced")
                effective = played.get("decider_forced")
                out.append(f"  armed set the newest recorded game played ({played['tag']}, {played['utc']}): "
                           f"{len(forced) if isinstance(forced, list) else 'no'} tags forced at launch "
                           f"({len(effective) if isinstance(effective, list) else '?'} of them not already on), "
                           f"{played.get('decider_treatments') or '?'} treatments in the decider's genome")
                if vs is not None:
                    if not vs["only_in_game"] and not vs["only_in_file"]:
                        out.append("    equals deploy/live-force-on.txt")
                    else:
                        out.append(f"    DIFFERS from deploy/live-force-on.txt: {len(vs['only_in_game'])} only in the "
                                   f"game, {len(vs['only_in_file'])} only in the file. The seat plays a list "
                                   f"the repository does not hold (a host's ~/.civvis-live-force-on)")
                        if vs["only_in_file"]:
                            out.append("    in the file, not played: " + ", ".join(vs["only_in_file"][:10])
                                       + (" ..." if len(vs["only_in_file"]) > 10 else ""))
                        out.append("    (--json lists every tag)")
            else:
                out.append("  armed set the live seat played: not readable here (no run summary with a `forced` "
                           "list on the ledger tree read; --ref or --runs DIR to point at one)")
    return "\n".join(out) + "\n"


def markdown(state: dict) -> str:
    f = state["focus"]
    out = [
        f"**CIVVIS live status**, {state['source']}, through {state['through']}. "
        f"Window: {state['window']['means']} ({state['window']['since'] or 'all'}); "
        f"setup: {state['window']['setup']}"
        + (f"; tags {', '.join(state['window']['tags'])}" if state['window']['tags'] else "")
        + f". {state['counting']['games']} games, each counted once by its best run.",
        "",
        "| level | wins / games | rate | 95% | par | vs par |",
        "|---|---:|---:|---:|---:|---|",
    ]
    for rec in state["levels"]:
        out.append(f"| {rec['name']} | {rec['wins']} / {rec['games']} | {pct(rec['rate'])} | "
                   f"{interval_text(rec)} | {pct(rec['par'])} | {rec['verdict'] or '-'} |")
    out += ["", f"**{f['name']}** by UTC day the game ended:", "", "| day | wins / games | rate |", "|---|---:|---:|"]
    for day in f["by_day"]:
        out.append(f"| {day['day']} | {day['wins']} / {day['games']} | {pct(day['rate'])} |")
    if "diplomatic" in f:
        d = f["diplomatic"]
        out += ["", f"Diplomatic victory off: {d['off']['wins']} / {d['off']['games']} ({pct(d['off']['rate'])}); "
                    f"on: {d['on']['wins']} / {d['on']['games']} ({pct(d['on']['rate'])}); unknown: {d['unknown']}."]
    losses = sum(count for _, count in f["losses"])
    out += ["", f"{f['name']} losses ({losses}): "
            + ", ".join(f"{name} {count} ({pct(count / losses)})" for name, count in f["losses"]) + ". Wins: "
            + (", ".join(f"{name} {count}" for name, count in f["wins_by_victory"]) or "none") + "."]
    out += ["", "| turn the game ended | n | p10 | p25 | median | p75 | p90 |", "|---|---:|---:|---:|---:|---:|---:|"]
    for name, p in f["turns"].items():
        if p:
            out.append(f"| {name} | {p['n']} | {p['p10']:.0f} | {p['p25']:.0f} | {p['p50']:.0f} | "
                       f"{p['p75']:.0f} | {p['p90']:.0f} |")
    ratio = f["score_ratio"]
    medians = [f"{ratio[side]['p50']:.2f} in {side}" for side in ("wins", "losses") if ratio[side]]
    if medians:
        out += ["", "Our score / best rival's, median: " + ", ".join(medians) + "."]
    t = state["throughput"]
    per_day = ", ".join(f"{day[5:]} {count}" for day, count in t["finished_per_day"])
    out += ["", f"Finished games per UTC day: {per_day}. Idle share of wall time: {pct(t['idle_share'])}."]
    g = state.get("genes")
    if g and "error" not in g:
        ff = g.get("force_file") or {}
        out += ["", f"Genes: {g['registry_rows']} registry rows, {g['default_on']} on by default, "
                    f"{ff.get('tags', '?')} tags in `deploy/live-force-on.txt` ({ff.get('opt_ins_it_adds', '?')} "
                    f"opt-ins it adds), {g['opt_in_not_armed']} opt-ins not armed."]
    return "\n".join(out) + "\n"


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    source = parser.add_mutually_exclusive_group()
    source.add_argument("--ledger", metavar="PATH", help="read this results.jsonl (`-` for stdin) instead of fetching")
    source.add_argument("--ref", metavar="REF", help="read REF:results.jsonl without fetching "
                                                     "(e.g. refs/remotes/origin/ledger)")
    parser.add_argument("--repo", default=str(REPO), help="the clone to fetch into and read refs from")
    parser.add_argument("--fetch-timeout", type=float, default=600.0, metavar="SECONDS")
    parser.add_argument("--since", default="", metavar="YYYY-MM-DD",
                        help=f"games launched on or after this UTC day; `all` for every game "
                             f"(default: the last {DEFAULT_DAYS} UTC days of the ledger)")
    parser.add_argument("--tag", action="append", default=[], metavar="PREFIX",
                        help="only games whose tag starts with PREFIX (repeatable): civvis-t8-")
    parser.add_argument("--any-setup", action="store_true",
                        help="count every game, not only 1 CIVVIS + 5 AI on 6P Continents")
    parser.add_argument("--level", default=DEFAULT_LEVEL, choices=RUNGS,
                        help="the level the detail sections read (default: deity)")
    parser.add_argument("--victory-settings", action="store_true",
                        help="split the level by the Diplomatic victory setting, read from each game's "
                             "seat event on the ledger tree (one events blob per game)")
    parser.add_argument("--runs", metavar="DIR", help="a local runs directory (runs/<tag>/summary.json, "
                                                      "`live_ledger.py pull` layout) to read before the ledger tree")
    parser.add_argument("--no-genes", action="store_true", help="leave out the gene backlog")
    output = parser.add_mutually_exclusive_group()
    output.add_argument("--json", action="store_true")
    output.add_argument("--markdown", action="store_true")
    args = parser.parse_args(argv)

    since = args.since
    if since == "all":
        since = None
    elif since and not re.fullmatch(r"\d{4}-\d{2}-\d{2}", since):
        parser.error("--since takes YYYY-MM-DD or all")
    rows, origin = load(args)
    repo = Path(args.repo)
    runs_dir = Path(args.runs).expanduser() if args.runs else None
    focus_level = RUNGS.index(args.level) + 1
    settings = None
    if args.victory_settings:
        keys = [game["key"] for game in games(rows) if level(game["row"].get("difficulty")) == focus_level]
        settings = diplomatic_settings(repo, origin["ref"], runs_dir, keys)
    genes_section = None
    if not args.no_genes:
        try:
            ledger = json.loads(GENE_LEDGER.read_text(encoding="utf-8")) if GENE_LEDGER.is_file() else {}
            genes_section = gene_backlog(
                GENES_RS.read_text(encoding="utf-8") if GENES_RS.is_file() else None,
                ledger,
                FORCE_FILE.read_text(encoding="utf-8") if FORCE_FILE.is_file() else None,
                played_genome(repo, origin["ref"], runs_dir, rows, online=origin["source"] == RAW_URL))
        except Exception as error:  # noqa: BLE001 - the backlog must not cost the win rates
            genes_section = {"error": f"gene backlog unreadable: {error!r}"}
    state = status(rows, since=since, tags=args.tag, any_setup=args.any_setup, focus_level=focus_level,
                   victory_settings=settings, genes_section=genes_section, source=origin)
    if args.json:
        print(json.dumps(state, indent=1, sort_keys=False))
    elif args.markdown:
        print(markdown(state), end="")
    else:
        print(text(state), end="")
    return 0


if __name__ == "__main__":
    sys.exit(main())
