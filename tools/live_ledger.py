#!/usr/bin/env python3
"""Read the live run ledger from the `ledger` branch, on any machine.

The seat that plays Civilization VI appends every finished run — its
`summary.json` and gzipped `events.jsonl` — to an append-only orphan branch
of this repository (`civ6_ladder.py publish-run`, called by `civ6_play.py` the
moment a summary is written). This is the reader for a machine that never sat
beside that runs directory:

    python tools/live_ledger.py pull            # origin/ledger -> ~/.cache/civvis/ledger/
    python tools/live_ledger.py runs --last 10  # the newest runs, one row each
    python tools/live_ledger.py kpis --last 20  # one row per GAME with the screen KPIs
    python tools/live_ledger.py screen <gene>   # both arms of a live screen, with intervals

`kpis` and `screen` read the ledger as GAMES, not rows: a `<tag>-contN`
autosave continuation is a segment of the game `<tag>` and is joined back to
it (`docs/LIVE_SCREEN.md`).

`pull` needs no worktree and checks nothing out: it fetches the branch tip
and copies each run it has not yet seen with `git show`. The cache is laid out
exactly as the branch is (`runs/<tag>/summary.json`, `runs/<tag>/events.jsonl.gz`),
so a tool that reads a local runs directory reads the pulled ledger the same way.
"""

from __future__ import annotations

import argparse
import gzip
import json
import os
import subprocess
import sys
import math
import re
import statistics
from datetime import datetime, timezone
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_ladder  # noqa: E402
import civ6_run_report  # noqa: E402
import genes  # noqa: E402

CACHE_DEFAULT = Path.home() / ".cache" / "civvis" / "ledger"


def _git_bytes(repo: Path, *args: str, env: dict | None = None) -> bytes:
    result = subprocess.run(["git", "-C", str(repo), *args],
                            capture_output=True, check=False,
                            env={**os.environ, **(env or {})})
    if result.returncode != 0:
        raise RuntimeError(
            f"git {' '.join(args)} failed ({result.returncode}): "
            f"{result.stderr.decode(errors='replace').strip()}")
    return result.stdout


def pull(cache: Path = CACHE_DEFAULT, *, repo: Path | None = None,
         remote: str = "origin", branch: str = civ6_ladder.LEDGER_BRANCH,
         env: dict | None = None) -> list[str]:
    """Copy every run on the ledger branch the cache lacks. Returns the new tags."""
    repo = Path(repo or civ6_ladder.REPO)
    cache = Path(cache)
    tip = civ6_ladder.ledger_tip(repo, remote, branch, env=env)
    if tip is None:
        raise RuntimeError(f"{remote} has no `{branch}` branch yet")
    listing = _git_bytes(repo, "ls-tree", "-r", "--name-only", tip, env=env)
    fresh: list[str] = []
    for line in listing.decode().splitlines():
        parts = line.split("/")
        if len(parts) != 3 or parts[0] != "runs":
            continue
        target = cache / line
        if target.is_file():
            continue
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(_git_bytes(repo, "show", f"{tip}:{line}", env=env))
        if parts[2] == "summary.json":
            fresh.append(parts[1])
    (cache / "TIP").write_text(tip + "\n")
    return fresh


def run_dirs(root: Path) -> list[Path]:
    """Run directories under a pulled ledger cache OR a live runs directory."""
    root = Path(root)
    base = root / "runs" if (root / "runs").is_dir() else root
    return [path.parent for path in base.glob("*/summary.json")]


def events_path(run_dir: Path) -> Path | None:
    """The run's events, plain or gzipped, whichever the directory holds."""
    for name in ("events.jsonl", "events.jsonl.gz"):
        if (run_dir / name).is_file():
            return run_dir / name
    return None


def open_events(path: Path):
    """Text handle over events.jsonl or events.jsonl.gz."""
    if path.suffix == ".gz":
        return gzip.open(path, "rt")
    return path.open()


def summaries(root: Path, last: int | None = None) -> list[dict]:
    """Summaries under `root`, oldest first (newest `last` when given).
    Each carries `_dir`, the directory it was read from."""
    rows = []
    for run_dir in run_dirs(root):
        try:
            body = json.loads((run_dir / "summary.json").read_text())
        except (OSError, json.JSONDecodeError):
            continue
        if not isinstance(body, dict):
            continue
        body["_dir"] = run_dir
        rows.append(body)

    def stamp(body: dict) -> str:
        return body.get("finished_utc") or datetime.fromtimestamp(
            (body["_dir"] / "summary.json").stat().st_mtime,
            tz=timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    rows.sort(key=stamp)
    return rows[-last:] if last else rows


def deals_cell(deals: dict | None) -> str:
    if not deals:
        return "-"
    return (f"s{deals.get('sessions_opened', 0)}/"
            f"a{deals.get('sessions_answered', 0)}/"
            f"u{deals.get('sessions_unanswered', 0)} "
            f"c{deals.get('closed', 0)} d{deals.get('declined', 0)} "
            f"e{deals.get('expired', 0)} "
            f"p+{deals.get('peace_accepted', 0)}/-{deals.get('peace_refused', 0)}"
            + (" stood_down" if deals.get("stood_down") else ""))


def run_row(body: dict) -> list[str]:
    outcome = body.get("outcome") or {}
    victory = civ6_ladder.victory_type(body) or outcome.get("kind") or "-"
    if victory and civ6_ladder.is_win(body):
        victory = f"WON {victory}"
    applied = civ6_ladder.applied_pct(body)
    return [
        str(body.get("tag") or body["_dir"].name),
        str(body.get("finished_utc") or "-"),
        civ6_ladder.NAMES.get(body.get("difficulty"), str(body.get("difficulty") or "-")),
        str(body.get("last_turn") if body.get("last_turn") is not None else "-"),
        str(body.get("last_score") if body.get("last_score") is not None else "-"),
        str(body.get("rival_best") if body.get("rival_best") is not None else "-"),
        str(victory),
        f"{applied:.1f}%" if applied is not None else "-",
        deals_cell(body.get("deals")),
    ]


HEADER = ["tag", "finished", "difficulty", "turns", "score", "rival_best",
          "victory", "applied", "deals"]


def table(rows: list[list[str]], header: list[str] = HEADER) -> str:
    widths = [max(len(r[i]) for r in [header, *rows]) for i in range(len(header))]
    lines = ["  ".join(cell.ljust(widths[i]) for i, cell in enumerate(header))]
    for row in rows:
        lines.append("  ".join(cell.ljust(widths[i]) for i, cell in enumerate(row)))
    return "\n".join(lines)


def runs_table(root: Path, last: int) -> str:
    return table([run_row(body) for body in summaries(root, last)])



# ─── Games, not rows ─────────────────────────────────────────────────────────

#: `civ6_civvis_climb.resume_from_autosave` reloads a frozen game under
#: `<tag>-cont<N>`, a fresh run directory with a fresh `events.jsonl`, and a
#: frozen run is killed before it can publish — so the ledger holds the
#: SEGMENTS of such a game, the stem sometimes missing. A KPI must be read off
#: the game.
CONT = re.compile(r"^(?P<stem>.*?)-cont(?P<index>\d+)$")


def game_stem(tag: str) -> str:
    match = CONT.match(tag)
    return match.group("stem") if match else tag


def segment_index(tag: str) -> int:
    match = CONT.match(tag)
    return int(match.group("index")) if match else 0


#: The turns the science and tech ratios are read at: the opening deficit is
#: set by t100 (40–61 % of the rival's science on the 09-01 Emperor runs) and
#: the t150 reading is the last one every run reaches before the abandon line.
KPI_TURNS = (100, 150)


def _rival_max(state: dict, key: str) -> float:
    best = 0.0
    for rival in state.get("rivals") or []:
        value = rival.get(key)
        if isinstance(value, list):
            value = len(value)
        best = max(best, float(value or 0))
    return best


def event_kpis(path: Path) -> dict:
    """What one events file says, streamed once. Raw parts, not ratios: the
    parts of a game's segments are merged by `join_game` before any ratio is
    formed. The FIRST state frame of a turn is the board before the seat
    acted (docs/LIVE_TACTICS.md §8); every reading is taken from it."""
    parts = {
        "turns": {},          # turn -> {"science","rival_science","techs","rival_techs"}
        "boosted": set(), "inspired": set(),
        "techs_end": None, "civics_end": None, "last_turn": None,
        "launch": {},         # project -> first turn it is on `science_projects`
        "ordered": {},        # project -> first turn its launch was verified
    }
    seen_turns: set[int] = set()
    with open_events(path) as handle:
        for line in handle:
            if '"kind": "state"' in line or '"kind":"state"' in line:
                try:
                    row = json.loads(line)
                except json.JSONDecodeError:
                    continue
                if row.get("kind") != "state" or not isinstance(row.get("turn"), int):
                    continue
                turn = row["turn"]
                parts["boosted"].update(row.get("boosted_techs") or [])
                parts["inspired"].update(row.get("boosted_civics") or [])
                for project, _label in civ6_run_report.SPACE_CHAIN:
                    if project in (row.get("science_projects") or []):
                        parts["launch"].setdefault(project, turn)
                if turn in seen_turns:
                    continue
                seen_turns.add(turn)
                parts["turns"][turn] = {
                    "science": float(row.get("science") or 0),
                    "rival_science": _rival_max(row, "science"),
                    "techs": len(row.get("techs") or []),
                    "rival_techs": _rival_max(row, "techs"),
                }
                parts["techs_end"] = set(row.get("techs") or [])
                parts["civics_end"] = set(row.get("civics") or [])
                parts["last_turn"] = turn
            elif '"order_verified"' in line and "PROJECT_LAUNCH_" in line:
                try:
                    row = json.loads(line)
                except json.JSONDecodeError:
                    continue
                verb = str(row.get("verb") or "")
                for project, _label in civ6_run_report.SPACE_CHAIN:
                    if verb == project and isinstance(row.get("turn"), int):
                        parts["ordered"].setdefault(project, row["turn"])
    return parts


def _at(turns: dict, target: int) -> dict | None:
    """The first-frame reading at `target`, or the nearest later turn within
    five (a frame can be missing on the exact turn), else None."""
    for turn in range(target, target + 6):
        if turn in turns:
            return turns[turn]
    return None


def _ratio(numerator: float | None, denominator: float | None) -> float | None:
    if numerator is None or not denominator:
        return None
    return numerator / denominator


def join_game(segments: list[dict]) -> dict:
    """One game from its ledger rows, oldest segment first.

    The outcome, the last turn and the played genome are the LAST segment's;
    the opening (`cities_at_60`) and every turn reading come from the first
    segment that holds them; combat is summed; boosts accumulate across
    segments and are read against the last segment's tree.
    """
    segments = sorted(segments, key=lambda body: segment_index(body["tag"]))
    last = segments[-1]
    stem = game_stem(last["tag"])
    game = {
        "tag": stem,
        "segments": [body["tag"] for body in segments],
        "stem_present": segment_index(segments[0]["tag"]) == 0,
        "finished_utc": last.get("finished_utc"),
        "difficulty": last.get("difficulty"),
        "victory_target": last.get("victory_target"),
        "last_turn": last.get("last_turn"),
        "won": civ6_ladder.is_win(last),
        "victory": civ6_ladder.victory_type(last),
        "reason": last.get("reason"),
        "abandoned_at_150": bool((last.get("abandoned") or {}).get("rule") == "below_leader_score"),
        "reached_t200": bool((last.get("last_turn") or 0) >= 200),
        "screen_gene": None, "screen_arm": None,
        "forced": None, "withheld": None,
        "genome_treatments": last.get("genome_treatments"),
        "cities_at_60": None,
        "kills": 0, "losses": 0, "combat_seen": False,
        "boosts": last.get("boosts"),
    }
    for body in segments:
        if game["screen_gene"] is None and body.get("screen_gene"):
            game["screen_gene"] = body["screen_gene"]
            game["screen_arm"] = body.get("screen_arm")
        if game["forced"] is None and body.get("forced") is not None:
            game["forced"] = list(body["forced"])
        if game["withheld"] is None and body.get("withheld") is not None:
            game["withheld"] = list(body["withheld"])
        if game["cities_at_60"] is None and body.get("cities_at_60") is not None:
            game["cities_at_60"] = body["cities_at_60"]
        combat = body.get("combat") or {}
        if combat:
            game["combat_seen"] = True
            game["kills"] += int(combat.get("kills") or 0)
            game["losses"] += int(combat.get("losses") or 0)
    # Event-side parts, merged across segments.
    turns: dict = {}
    boosted: set = set()
    inspired: set = set()
    techs_end = civics_end = None
    launch: dict = {}
    ordered: dict = {}
    for body in segments:
        path = events_path(body["_dir"]) if body.get("_dir") else None
        if path is None:
            continue
        parts = event_kpis(path)
        for turn, reading in parts["turns"].items():
            turns.setdefault(turn, reading)
        boosted |= parts["boosted"]
        inspired |= parts["inspired"]
        if parts["techs_end"] is not None:
            techs_end, civics_end = parts["techs_end"], parts["civics_end"]
        for project, turn in parts["launch"].items():
            launch[project] = min(turn, launch.get(project, turn))
        for project, turn in parts["ordered"].items():
            ordered[project] = min(turn, ordered.get(project, turn))
    for target in KPI_TURNS:
        reading = _at(turns, target)
        game[f"science_ratio_t{target}"] = (
            _ratio(reading["science"], reading["rival_science"]) if reading else None)
        game[f"tech_ratio_t{target}"] = (
            _ratio(reading["techs"], reading["rival_techs"]) if reading else None)
    boosts = game["boosts"] or {}
    if boosts.get("techs_boosted_share") is not None:
        game["techs_boosted_share"] = boosts["techs_boosted_share"]
        game["civics_inspired_share"] = boosts.get("civics_inspired_share")
    elif techs_end:
        game["techs_boosted_share"] = len(boosted & techs_end) / len(techs_end)
        game["civics_inspired_share"] = (
            len(inspired & civics_end) / len(civics_end) if civics_end else None)
    else:
        game["techs_boosted_share"] = None
        game["civics_inspired_share"] = None
    for project, label in civ6_run_report.SPACE_CHAIN:
        key = label.split()[0]  # earth, moon, mars, exoplanet
        game[f"launch_{key}"] = launch.get(project, ordered.get(project))
    turns_played = game["last_turn"] or 0
    game["kills_per_loss"] = (game["kills"] / max(game["losses"], 1)
                              if game["combat_seen"] else None)
    game["losses_per_100_turns"] = (game["losses"] / turns_played * 100
                                    if game["combat_seen"] and turns_played else None)
    return game


def games(root: Path, *, last: int | None = None, difficulty: str | None = None,
          lane: str | None = None, since: str | None = None) -> list[dict]:
    """Every game under `root` as `join_game` reads it, oldest first."""
    by_stem: dict[str, list[dict]] = {}
    for body in summaries(root):
        tag = str(body.get("tag") or body["_dir"].name)
        body["tag"] = tag
        by_stem.setdefault(game_stem(tag), []).append(body)
    rows = [join_game(segments) for segments in by_stem.values()]
    rows.sort(key=lambda game: game.get("finished_utc") or "")
    if difficulty:
        wanted = difficulty if difficulty.startswith("DIFFICULTY_") else \
            next((key for key, name in civ6_ladder.LADDER
                  if name.lower() == difficulty.lower()), difficulty)
        rows = [game for game in rows if game["difficulty"] == wanted]
    if lane:
        rows = [game for game in rows if game["victory_target"] == lane]
    if since:
        rows = [game for game in rows if (game.get("finished_utc") or "") >= since]
    return rows[-last:] if last else rows


def arm_of(game: dict, gene: str) -> str | None:
    """Which arm of `gene` this game played: the dealt arm when the game was
    screened for it (an unarmed run of a screened gene IS the default arm),
    else the batch-wide `forced`/`withheld` words, else None."""
    if game.get("screen_gene") == gene and game.get("screen_arm") in ("on", "off"):
        return game["screen_arm"]
    if gene in (game.get("forced") or []):
        return "on"
    if gene in (game.get("withheld") or []):
        return "off"
    return None


# ─── Intervals ───────────────────────────────────────────────────────────────

def _betacf(a: float, b: float, x: float) -> float:
    """Continued fraction for the regularized incomplete beta (Numerical
    Recipes `betacf`), enough precision for a t quantile."""
    tiny = 1e-300
    qab, qap, qam = a + b, a + 1.0, a - 1.0
    c, d = 1.0, 1.0 - qab * x / qap
    d = 1.0 / (d if abs(d) > tiny else tiny)
    h = d
    for m in range(1, 300):
        m2 = 2 * m
        aa = m * (b - m) * x / ((qam + m2) * (a + m2))
        d = 1.0 + aa * d
        d = 1.0 / (d if abs(d) > tiny else tiny)
        c = 1.0 + aa / (c if abs(c) > tiny else tiny)
        h *= d * c
        aa = -(a + m) * (qab + m) * x / ((a + m2) * (qap + m2))
        d = 1.0 + aa * d
        d = 1.0 / (d if abs(d) > tiny else tiny)
        c = 1.0 + aa / (c if abs(c) > tiny else tiny)
        delta = d * c
        h *= delta
        if abs(delta - 1.0) < 3e-14:
            break
    return h


def betainc(a: float, b: float, x: float) -> float:
    if x <= 0.0:
        return 0.0
    if x >= 1.0:
        return 1.0
    front = math.exp(math.lgamma(a + b) - math.lgamma(a) - math.lgamma(b)
                     + a * math.log(x) + b * math.log(1.0 - x))
    if x < (a + 1.0) / (a + b + 2.0):
        return front * _betacf(a, b, x) / a
    return 1.0 - front * _betacf(b, a, 1.0 - x) / b


def student_t_cdf(t: float, df: float) -> float:
    x = df / (df + t * t)
    tail = 0.5 * betainc(df / 2.0, 0.5, x)
    return 1.0 - tail if t >= 0 else tail


def t_quantile(df: float, p: float = 0.975) -> float:
    """The Student t quantile, by bisection on the CDF; the normal's beyond
    df 200 (t differs from z by under 0.5 % there)."""
    if df >= 200:
        return genes.Z95 if abs(p - 0.975) < 1e-9 else _normal_quantile(p)
    lo, hi = 0.0, 50.0
    for _ in range(80):
        mid = 0.5 * (lo + hi)
        if student_t_cdf(mid, df) < p:
            lo = mid
        else:
            hi = mid
    return 0.5 * (lo + hi)


def _normal_quantile(p: float) -> float:
    lo, hi = -10.0, 10.0
    for _ in range(80):
        mid = 0.5 * (lo + hi)
        if genes.normal_cdf(mid) < p:
            lo = mid
        else:
            hi = mid
    return 0.5 * (lo + hi)


def mean_interval(values: list[float]) -> dict:
    """n, mean, sd and the 95 % t interval of the mean; sd/interval None below n=2."""
    n = len(values)
    if n == 0:
        return {"n": 0, "mean": None, "sd": None, "lo": None, "hi": None}
    mean = statistics.fmean(values)
    if n < 2:
        return {"n": 1, "mean": mean, "sd": None, "lo": None, "hi": None}
    sd = statistics.stdev(values)
    half = t_quantile(n - 1) * sd / math.sqrt(n)
    return {"n": n, "mean": mean, "sd": sd, "lo": mean - half, "hi": mean + half}


def welch(a: list[float], b: list[float]) -> dict:
    """`a` minus `b`: the difference of means with a Welch 95 % interval, and
    the games per arm that would detect the observed difference at 80 % power
    (two-sided 5 %, equal arms: n = 2 (2.8 sd / d)²)."""
    ia, ib = mean_interval(a), mean_interval(b)
    out = {"a": ia, "b": ib, "diff": None, "lo": None, "hi": None, "n80": None}
    if ia["n"] < 2 or ib["n"] < 2:
        if ia["n"] and ib["n"]:
            out["diff"] = ia["mean"] - ib["mean"]
        return out
    va, vb = ia["sd"] ** 2 / ia["n"], ib["sd"] ** 2 / ib["n"]
    se = math.sqrt(va + vb)
    diff = ia["mean"] - ib["mean"]
    out["diff"] = diff
    if se > 0:
        df = (va + vb) ** 2 / (va ** 2 / (ia["n"] - 1) + vb ** 2 / (ib["n"] - 1))
        half = t_quantile(df) * se
        out["lo"], out["hi"] = diff - half, diff + half
        pooled = math.sqrt(0.5 * (ia["sd"] ** 2 + ib["sd"] ** 2))
        out["n80"] = (math.ceil(2 * (genes.POWER_80 * pooled / abs(diff)) ** 2)
                      if abs(diff) > 0 and pooled > 0 else None)
    return out


def wilson(k: int, n: int) -> dict:
    """The Wilson 95 % score interval of k/n."""
    if n == 0:
        return {"n": 0, "k": 0, "rate": None, "lo": None, "hi": None}
    z = genes.Z95
    p = k / n
    denom = 1 + z * z / n
    centre = (p + z * z / (2 * n)) / denom
    half = z * math.sqrt(p * (1 - p) / n + z * z / (4 * n * n)) / denom
    return {"n": n, "k": k, "rate": p, "lo": centre - half, "hi": centre + half}


def rate_difference(ka: int, na: int, kb: int, nb: int) -> dict:
    """a minus b: the difference of two rates with a normal 95 % interval, the
    Wilson interval of each, and the games per arm to detect it at 80 % power."""
    wa, wb = wilson(ka, na), wilson(kb, nb)
    out = {"a": wa, "b": wb, "diff": None, "lo": None, "hi": None, "n80": None}
    if na == 0 or nb == 0:
        return out
    pa, pb = ka / na, kb / nb
    diff = pa - pb
    se = math.sqrt(pa * (1 - pa) / na + pb * (1 - pb) / nb)
    out["diff"] = diff
    out["lo"], out["hi"] = diff - genes.Z95 * se, diff + genes.Z95 * se
    if abs(diff) > 0:
        out["n80"] = math.ceil(genes.POWER_80 ** 2 * (pa * (1 - pa) + pb * (1 - pb))
                               / diff ** 2)
    return out


# ─── The screen report ───────────────────────────────────────────────────────

#: (key, label, kind) — `mean` KPIs are averaged over the games that carry
#: them; `rate` KPIs are counted over every game of the arm.
SCREEN_KPIS = (
    ("kills_per_loss", "kills per loss", "mean"),
    ("losses_per_100_turns", "combat losses per 100 turns", "mean"),
    ("cities_at_60", "cities at t60", "mean"),
    ("science_ratio_t100", "science ratio at t100", "mean"),
    ("science_ratio_t150", "science ratio at t150", "mean"),
    ("tech_ratio_t150", "tech ratio at t150", "mean"),
    ("techs_boosted_share", "techs boosted share", "mean"),
    ("civics_inspired_share", "civics inspired share", "mean"),
    ("launch_earth", "satellite launch turn", "mean"),
    ("launch_moon", "moon launch turn", "mean"),
    ("launch_mars", "mars launch turn", "mean"),
    ("abandoned_at_150", "abandoned at t150", "rate"),
    ("reached_t200", "reached t200", "rate"),
    ("won", "won", "rate"),
)


def screen(rows: list[dict], gene: str) -> dict:
    """Both arms of a live screen of `gene` over `rows` (from `games`)."""
    arms = {"on": [], "off": []}
    unassigned = 0
    for game in rows:
        arm = arm_of(game, gene)
        if arm is None:
            unassigned += 1
        else:
            arms[arm].append(game)
    kpis = []
    for key, label, kind in SCREEN_KPIS:
        if kind == "mean":
            on = [float(g[key]) for g in arms["on"] if g.get(key) is not None]
            off = [float(g[key]) for g in arms["off"] if g.get(key) is not None]
            kpis.append({"key": key, "label": label, "kind": kind, **welch(on, off)})
        else:
            on_k = sum(1 for g in arms["on"] if g.get(key))
            off_k = sum(1 for g in arms["off"] if g.get(key))
            kpis.append({"key": key, "label": label, "kind": kind,
                         **rate_difference(on_k, len(arms["on"]), off_k, len(arms["off"]))})
    return {
        "gene": gene,
        "live_arm": genes.live_arm(gene),
        "games": len(rows),
        "on": len(arms["on"]), "off": len(arms["off"]), "unassigned": unassigned,
        "segment_only": sum(1 for g in rows if not g["stem_present"]),
        "kpis": kpis,
    }


def _fmt(value, digits: int = 2) -> str:
    if value is None:
        return "-"
    if isinstance(value, float):
        return f"{value:.{digits}f}"
    return str(value)


def render_screen(report: dict) -> str:
    arm = report["live_arm"]
    lines = [
        f"screen {report['gene']}: live default {arm['live_default']}, "
        f"other arm {arm['arm_flag'] or 'none'} ({arm['reason']})",
        f"games {report['games']}: on {report['on']}, off {report['off']}, "
        f"unassigned {report['unassigned']} (excluded); "
        f"{report['segment_only']} segment-only game(s) without their stem",
        "",
        f"{'KPI':<30} {'on':>18} {'off':>18} {'on - off [95%]':>28} {'n/arm@80%':>10}",
    ]
    for kpi in report["kpis"]:
        a, b = kpi["a"], kpi["b"]
        if kpi["kind"] == "mean":
            cell_a = f"{_fmt(a['mean'])} (n={a['n']})"
            cell_b = f"{_fmt(b['mean'])} (n={b['n']})"
        else:
            cell_a = f"{_fmt(a['rate'])} ({a['k']}/{a['n']})"
            cell_b = f"{_fmt(b['rate'])} ({b['k']}/{b['n']})"
        diff = (f"{_fmt(kpi['diff'], 3)} [{_fmt(kpi['lo'], 3)}, {_fmt(kpi['hi'], 3)}]"
                if kpi["diff"] is not None else "-")
        lines.append(f"{kpi['label']:<30} {cell_a:>18} {cell_b:>18} {diff:>28} "
                     f"{_fmt(kpi['n80']):>10}")
    lines.append("")
    lines.append("n/arm@80%: games per arm that would detect the observed difference "
                 "at 80 % power (two-sided 5 %); '-' when the arms do not differ "
                 "or an arm has under two games.")
    return "\n".join(lines)


KPI_HEADER = ["game", "dif", "lane", "arm", "t", "end", "k/l", "loss/100t", "c@60",
              "sci100", "sci150", "tech150", "boost", "insp", "sat", "moon", "mars"]


def kpi_row(game: dict, gene: str | None = None) -> list[str]:
    end = "WON" if game["won"] else ("aband" if game["abandoned_at_150"] else
                                     (game["victory"] or game["reason"] or "-"))
    return [
        game["tag"] + ("" if game["stem_present"] else "*"),
        civ6_ladder.NAMES.get(game["difficulty"], str(game["difficulty"] or "-"))[:3],
        str(game["victory_target"] or "-")[:4],
        (arm_of(game, gene) or "-") if gene else (game.get("screen_arm") or "-"),
        _fmt(game["last_turn"]),
        str(end)[:8],
        _fmt(game["kills_per_loss"]),
        _fmt(game["losses_per_100_turns"], 1),
        _fmt(game["cities_at_60"]),
        _fmt(game["science_ratio_t100"]),
        _fmt(game["science_ratio_t150"]),
        _fmt(game["tech_ratio_t150"]),
        _fmt(game["techs_boosted_share"]),
        _fmt(game["civics_inspired_share"]),
        _fmt(game["launch_earth"]),
        _fmt(game["launch_moon"]),
        _fmt(game["launch_mars"]),
    ]


def kpis_table(rows: list[dict], gene: str | None = None) -> str:
    body = table([kpi_row(game, gene) for game in rows], KPI_HEADER)
    return body + "\n* = segment-only game: its stem never reached the ledger"


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--cache", type=Path, default=CACHE_DEFAULT,
                    help="where pulled runs live (default ~/.cache/civvis/ledger)")
    sub = ap.add_subparsers(dest="command", required=True)
    pl = sub.add_parser("pull", help="fetch origin/ledger into the cache")
    pl.add_argument("--remote", default="origin")
    pl.add_argument("--branch", default=civ6_ladder.LEDGER_BRANCH)
    rn = sub.add_parser("runs", help="one row per run, newest last")
    rn.add_argument("--last", type=int, default=10)
    rn.add_argument("--runs", type=Path, default=None,
                    help="read a live runs directory instead of the cache")

    def game_filters(parser):
        parser.add_argument("--last", type=int, default=None, help="the newest N games")
        parser.add_argument("--difficulty", default=None,
                            help="DIFFICULTY_KING or King: only that rung")
        parser.add_argument("--lane", default=None, help="only this victory_target")
        parser.add_argument("--since", default=None,
                            help="only games finished at or after this UTC stamp")
        parser.add_argument("--runs", type=Path, default=None,
                            help="read a live runs directory instead of the cache")
        parser.add_argument("--json", action="store_true")

    kp = sub.add_parser("kpis", help="one row per GAME with the screen KPIs")
    kp.add_argument("--gene", default=None, help="show each game's arm of this gene")
    game_filters(kp)
    sc = sub.add_parser("screen", help="both arms of a live screen of one gene")
    sc.add_argument("gene")
    game_filters(sc)
    args = ap.parse_args(argv)
    if args.command in ("kpis", "screen"):
        rows = games(args.runs or args.cache, last=args.last,
                     difficulty=args.difficulty, lane=args.lane, since=args.since)
        if args.command == "kpis":
            if args.json:
                print(json.dumps(rows, indent=1, sort_keys=True, default=str))
            else:
                print(kpis_table(rows, args.gene))
            return 0
        if genes.live_arm(args.gene)["live_default"] is None:
            print(f"{args.gene!r} is not a gene registry tag", file=sys.stderr)
            return 2
        report = screen(rows, args.gene)
        print(json.dumps(report, indent=1, sort_keys=True) if args.json
              else render_screen(report))
        return 0
    if args.command == "pull":
        fresh = pull(args.cache, remote=args.remote, branch=args.branch)
        print(f"{len(fresh)} new run(s) -> {args.cache}")
        for tag in fresh:
            print(f"  {tag}")
        return 0
    print(runs_table(args.runs or args.cache, args.last))
    return 0


if __name__ == "__main__":
    sys.exit(main())
