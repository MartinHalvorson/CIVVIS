#!/usr/bin/env python3
"""The Civ AI Interpretability page's record: how a CIVVIS AI agent decides.

    interpretability_page.py <source_tree>              print the record as JSON
    interpretability_page.py <source_tree> <page.html>  write it into the page
    interpretability_page.py --self-play <batches_dir> <out.json>
                                                rebuild the plan snapshot

The Civ AI Interpretability page (`beta/interpretability.html`, served at
`<lane>/charts/civ-ai-interpretability/`, the address it states itself) is the
last page under Charts/ (operator
2026-11-22: "Add a new chart to the bottom of Charts/ called AI
Interpretablity ... study our codebase, how the AI works, and do your best to
explain the AI to a human reader ... Top Down and bottom up if useful."). A
CIVVIS AI agent is a program of written rules, not a trained network, so it
can be read, and the page reads it from both ends:

- **top down, one turn in order**: every step `AdvancedAi::take_turn_inner`
  calls, in source order, as `tools/codemap.py --turn` reads them off
  `src/ai/advanced.rs`, each sorted by what its name says it does (`KINDS`).
  With it the size of the code: the shipping Rust under `src/ai`, tests left
  out.
- **the plan through a game**: what the seats of the newest finished
  tournament batch said they were doing, turn by turn. Every seat keeps a
  trace of its plan (`victory_portfolio.trace` on its row): its posture (the
  grand strategy of the turn), and the bottleneck it names for its victory.
  Beside it, what each kind of seat aimed for and what it won by, and the
  size of its empire at every 25th turn.
- **bottom up, the gene pool**: every screenable gene (one switch on one
  rule), with the difference it makes to the win rate when it is on, from
  `GENE_HEURISTIC_RANKING.md`, the table `tools/genes.py write` generates
  from the ledger.

The seats' rows are on the machine that plays them, not in the repository, so
the plans come from a snapshot, `data/interpretability/self_play_plans.json`,
which `--self-play` rebuilds from the batch directories (the same directory
the Performance and Win Analytics snapshots are rebuilt from). It reads the
newest finished batch alone: the plan is a fact about one build of the AI,
and a batch is one build.

How a seat's trace becomes turns: a trace holds a point whenever the plan
changes and at least every 17 turns, so a seat is in a point's posture from
that turn until its next point. A seat is counted from its first point to the
turn its game ended, or to its last point if it did not live to the end. A
column of the chart is `SPAN` turns, and counts seat-turns.

The same plan is also counted by how far through its game each turn is
(operator 2026-11-27: "produce another the plan through a game chart where
the x axis is % through the game"): a game that ended on turn L is cut into
`THROUGH` equal parts, turn t falling in part `(t - 1) * THROUGH // L`, so
its first turn is at 0% and the turn it ended on in the last part, before
100%. A turn weighs one over the turns its game has in that part
(`game_parts`), so every seat counts as one in every part it played
through, however long its game was: a game of 250 turns would otherwise
outweigh one of 110 two to one. A seat that did not live to the end stops
where its trace stops, as in the turns, and counts for the share of the part
it lived through.

Like the other pages' scripts this runs inside the script that publishes the
whole site, so what it cannot read costs the page that part, said on stderr,
and never costs the site its publish. A lane is filled by its own revision's
copy of this script from its own revision's tree (`fill` in
`beta/publish.sh`): the front page shows the agent as it was promoted.
"""

from __future__ import annotations

import datetime
import functools
import json
import pathlib
import re
import statistics
import sys

SLOT = re.compile(r'(<script type="application/json" id="interpretability-data">)(.*?)(</script>)', re.S)

#: The snapshot of the seats' plans, in the source tree.
PLANS = pathlib.Path("data/interpretability/self_play_plans.json")

#: The gene ranking `tools/genes.py write` generates, in the source tree.
RANKING = pathlib.Path("GENE_HEURISTIC_RANKING.md")

#: What a step of the turn does, read off its name, first match first. A step
#: no rule names is `plan`: it reads the board or keeps a plan (`assess`,
#: `maintain_war_plan`, `record_culture_curves`). The rules go by the verbs
#: the code names its steps with, so a step added tomorrow sorts itself.
KINDS = [
    ("spend", r"purchase|^buy_|_buys_|spending|upgrade|moderni[sz]e|^fund_|faith_reserve|_floor$|patronizes|band_bank|holds_the_chest"),
    ("units", r"^advanced_units$|^advanced_command_actions$|^commitment_owners_act$|^close_unit_orders$"),
    ("production", r"^claim_|^reserve_|^redirect_|production|queue|_heads$|^prioritize_|^arm_for|founder_temple|holy_site|hold_field_slot|hold_the_slots|housing|delegated_cities|saved_work|builds_its_source|^resite_"),
    ("govern", r"research|envoy|governor|government|polic|diplomacy|spies|secret_society|great_pe|pingala|curator|products|lane_spreads|offensive_posture|exit_the_drain"),
]
KIND_ORDER = ["plan", "govern", "spend", "production", "units"]

#: A batch directory is named for the moment it began: `20261121T184151Z-b41a`.
BATCH = re.compile(r"(\d{4})(\d{2})(\d{2})T\d{6}Z-[0-9a-f]+")

#: The table the snapshot keeps, as the other Charts/ snapshots do.
SETUP = {"speed": "online", "turns": 250, "players": 6, "map": "continents", "all_seats": True}

#: Turns to a column of the plan chart.
SPAN = 5
TURNS = 250
#: Columns of the plan chart by how far through the game: 2% each.
THROUGH = 50

#: The postures (`GrandStrategy::as_str`), the victories a game ends in, and
#: what a seat can be told to aim for (`civvis` is told nothing and chooses),
#: each in the order the page draws them.
POSTURES = ["expansion", "science", "culture", "religion", "diplomacy", "conquest", "recovery"]
VICTORIES = ["science", "culture", "religious", "diplomatic", "domination", "score"]
TARGETS = ["civvis", "science", "culture", "religious", "diplomatic", "domination", "score"]

#: The measures of an empire the rows carry at every 25th turn.
MEASURES = ["cities", "science", "culture", "military"]
MARKS = list(range(25, TURNS + 1, 25))

#: A gene's description that says only that it is a gene and where its code
#: is: the page shows the gene's name alone for these.
BOILERPLATE = re.compile(r"^(?:(?:Opt-in(?: gene)?|Turn on|Enable|Opt-in bottleneck reservation)\b[^.;:]*`[^`]*`[.;:]?(?:\s*see `[^`]+`\.?)?|See `[^`]+`\.?)$", re.I)
DESCRIPTION_LIMIT = 150


def kind(name: str) -> str:
    """What a step of the turn does, by its name."""
    for label, pattern in KINDS:
        if re.search(pattern, name):
            return label
    return "plan"


def turn(source: pathlib.Path) -> dict | None:
    """The steps of a turn in source order, each `[name, file, kind]`, and
    the size of the AI's code. `None` when the source cannot be read."""
    tools = str(source / "tools")
    sys.path.insert(0, tools)
    try:
        import codemap  # noqa: PLC0415 (the tree's own, wherever the tree is)

        steps = codemap.turn_order(source)
        code = [text for path, text in codemap.shipping_rust(source).items() if path.startswith("src/ai")]
    except (Exception, SystemExit) as error:  # noqa: BLE001 (never the publish)
        print(f"interpretability_page: no turn order ({error})", file=sys.stderr)
        return None
    finally:
        if tools in sys.path:
            sys.path.remove(tools)
    if not steps:
        print("interpretability_page: the turn has no steps", file=sys.stderr)
        return None
    rows = []
    for _, name, home in steps:
        path = home.rsplit(":", 1)[0]
        rows.append([name, path[len("src/ai/"):] if path.startswith("src/ai/") else path, kind(name)])
    return {
        "file": codemap.TURN_FILE,
        "fn": codemap.TURN_FN.replace("fn ", "").strip("( "),
        "kinds": KIND_ORDER,
        "steps": rows,
        "code": {"files": len(code), "lines": sum(text.count("\n") for text in code)},
    }


def described(tag: str, text: str) -> str:
    """A gene's description as the page shows it: plain, short, and empty
    when it says nothing the name does not."""
    text = text.replace("\\|", "|").strip()
    if BOILERPLATE.match(text):
        return ""
    text = re.sub(rf"^`{re.escape(tag)}`\s*(?:\(OptIn\))?:\s*", "", text).replace("`", "")
    if re.fullmatch(r"see [\w/.:]+\.?", text, re.I):
        return ""
    text = text[:1].upper() + text[1:]
    if len(text) > DESCRIPTION_LIMIT:
        text = text[:DESCRIPTION_LIMIT].rsplit(" ", 1)[0].rstrip(",;:") + "…"
    return text


def genes(source: pathlib.Path) -> dict | None:
    """Every ranked gene, best first, each `[tag, on, diff, sure, about]`:
    whether the deployed genome has it on, the on-minus-off win rate in
    percentage points over every seat that priced it, the estimated
    probability it helps in percent, and its description. With them the most
    seats any one gene was priced on."""
    try:
        text = (source / RANKING).read_text(encoding="utf-8")
    except OSError as error:
        print(f"interpretability_page: no gene ranking ({error})", file=sys.stderr)
        return None
    rows, seats = [], 0
    for line in text.splitlines():
        if not re.match(r"\| \d+ \|", line):
            continue
        cells = [cell.strip() for cell in re.split(r"(?<!\\)\|", line.strip().strip("|"))]
        if len(cells) != 14:
            continue
        tag = cells[1].strip("`")
        try:
            diff, sure = float(cells[11].rstrip("%")), float(cells[5].rstrip("%"))
        except ValueError:
            continue
        rows.append([tag, 1 if "on" in cells[4] else 0, diff, sure, described(tag, cells[2])])
        counts = [int(n.replace(",", "")) for cell in (cells[9], cells[10]) for n in re.findall(r"\(n=([\d,]+)\)", cell)[:1]]
        if len(counts) == 2:
            seats = max(seats, sum(counts))
    if not rows:
        print(f"interpretability_page: {RANKING} holds no ranked gene", file=sys.stderr)
        return None
    return {"seats": seats, "on": sum(row[1] for row in rows), "rows": rows}


def seat_spans(row: dict) -> list[tuple[int, int, str, str]] | None:
    """One seat's plan as `(from turn, to turn, posture, bottleneck)`, both
    ends counted, or `None` for a row without a trace."""
    portfolio = row.get("victory_portfolio")
    trace = portfolio.get("trace") if isinstance(portfolio, dict) else None
    end = row.get("turn")
    if not isinstance(trace, list) or not trace or not isinstance(end, int):
        return None
    points = sorted((p for p in trace if isinstance(p, dict) and isinstance(p.get("turn"), int)), key=lambda p: p["turn"])
    if not points:
        return None
    last = min(end, TURNS) if row.get("alive") else min(points[-1]["turn"], end, TURNS)
    out = []
    for index, point in enumerate(points):
        upto = points[index + 1]["turn"] - 1 if index + 1 < len(points) else last
        upto = min(upto, last)
        if upto >= point["turn"]:
            out.append((point["turn"], upto, str(point.get("posture") or ""), str(point.get("bottleneck") or "")))
    return out


@functools.lru_cache(maxsize=None)
def game_parts(length: int) -> tuple[int, ...]:
    """How many turns of a game `length` turns long fall in each of the
    `THROUGH` parts of it: 2 or 3 of 110 turns, 5 of 250, none of some parts
    of a game shorter than `THROUGH`."""
    counts = [0] * THROUGH
    for turn_ in range(1, length + 1):
        counts[min(THROUGH - 1, (turn_ - 1) * THROUGH // length)] += 1
    return tuple(counts)


def tally() -> dict:
    spans = TURNS // SPAN
    return {
        "seats": 0, "forecasting": 0, "wins": 0, "won_by": dict.fromkeys(VICTORIES, 0),
        "playing": [0] * spans, "posture": {name: [0] * spans for name in POSTURES},
        "through": {"playing": [0.0] * THROUGH, "posture": {name: [0.0] * THROUGH for name in POSTURES}},
        "bottleneck": {}, "named": 0, "turns": 0,
        "marks": {measure: [[] for _ in MARKS] for measure in MEASURES},
    }


def count(into: dict, row: dict, spans: list[tuple[int, int, str, str]]) -> None:
    into["seats"] += 1
    # The seats whose `victory-portfolio` gene is on: the ones that forecast
    # their victory and so name what is in its way.
    if (row.get("victory_portfolio") or {}).get("enabled") is True:
        into["forecasting"] += 1
    if row.get("win") is True:
        into["wins"] += 1
        if row.get("victory") in into["won_by"]:
            into["won_by"][row["victory"]] += 1
    # How long the seat's game was, for the chart by how far through it.
    length = max(1, min(int(row["turn"]), TURNS))
    held_in = game_parts(length)
    for start, upto, posture, bottleneck in spans:
        held = upto - start + 1
        into["turns"] += held
        if bottleneck:
            into["named"] += held
            into["bottleneck"][bottleneck] = into["bottleneck"].get(bottleneck, 0) + held
        for turn_ in range(max(start, 1), upto + 1):
            column = (turn_ - 1) // SPAN
            into["playing"][column] += 1
            if posture in into["posture"]:
                into["posture"][posture][column] += 1
            part = min(THROUGH - 1, (turn_ - 1) * THROUGH // length)
            into["through"]["playing"][part] += 1 / held_in[part]
            if posture in into["through"]["posture"]:
                into["through"]["posture"][posture][part] += 1 / held_in[part]
    for point in row.get("trajectory") or []:
        if not isinstance(point, dict) or point.get("turn") not in MARKS or not point.get("alive"):
            continue
        for measure in MEASURES:
            value = point.get(measure)
            if isinstance(value, (int, float)):
                into["marks"][measure][MARKS.index(point["turn"])].append(float(value))


def batch_plans(directory: pathlib.Path) -> dict | None:
    """One batch's seats, counted per target and all together, with the
    commit its build was made from. `None` for a batch whose header is not
    the table in `SETUP`, or with no seat that kept a trace."""
    tallies = {name: tally() for name in ["all", *TARGETS]}
    seeds, commit, seen = set(), "", set()
    for rows in sorted(directory.glob("rows*.jsonl")):
        with rows.open(encoding="utf-8") as lines:
            for line in lines:
                try:
                    row = json.loads(line)
                except ValueError:
                    continue
                if row.get("kind") == "header":
                    if any(row.get(key) != value for key, value in SETUP.items()):
                        return None
                    build = row.get("build")
                    if isinstance(build, str):
                        try:
                            build = json.loads(build)
                        except ValueError:
                            build = {}
                    commit = commit or str((build if isinstance(build, dict) else {}).get("commit") or "")
                    continue
                if row.get("kind") != "game" or (row.get("seed"), row.get("seat")) in seen:
                    continue
                spans = seat_spans(row)
                if spans is None:
                    continue
                seen.add((row.get("seed"), row.get("seat")))
                seeds.add(row.get("seed"))
                count(tallies["all"], row, spans)
                if row.get("player_target") in tallies:
                    count(tallies[row["player_target"]], row, spans)
    if not tallies["all"]["seats"]:
        return None
    for into in tallies.values():
        # The middle seat of those still playing at each mark, and how many.
        into["at_marks"] = [len(values) for values in into["marks"]["cities"]]
        into["marks"] = {
            measure: [round(statistics.median(values), 1) if values else None for values in marks]
            for measure, marks in into["marks"].items()
        }
        into["bottleneck"] = dict(sorted(into["bottleneck"].items(), key=lambda item: (-item[1], item[0])))
        # Seats, to the hundredth: a seat that played a part through is one.
        into["through"] = {
            "playing": [round(value, 2) for value in into["through"]["playing"]],
            "posture": {name: [round(value, 2) for value in values] for name, values in into["through"]["posture"].items()},
        }
    return {"games": len(seeds), "seats": tallies["all"]["seats"], "commit": commit[:7], "targets": tallies}


def snapshot(batches: pathlib.Path) -> dict:
    """The newest finished batch under a gene-screen state directory's
    `batches/` that plays the table in `SETUP`, as the snapshot file holds
    it. A batch is finished once its `analysis.json` is written."""
    for directory in sorted((p for p in batches.iterdir() if p.is_dir() and BATCH.fullmatch(p.name)), reverse=True):
        if not (directory / "analysis.json").is_file():
            continue
        plans = batch_plans(directory)
        if plans is None:
            continue
        year, month, day = BATCH.fullmatch(directory.name).groups()
        return {
            "about": "What the seats of the newest finished gene-screen batch said they were doing, turn by turn. "
            "Rebuilt by `beta/interpretability_page.py --self-play`; read by the Civ AI Interpretability page.",
            "taken": datetime.datetime.now(datetime.timezone.utc).date().isoformat(),
            "batch": directory.name,
            "began": f"{year}-{month}-{day}",
            "commit": plans["commit"],
            "games": plans["games"],
            "seats": plans["seats"],
            "span": SPAN,
            "through": THROUGH,
            "marks": MARKS,
            "postures": POSTURES,
            "victories": VICTORIES,
            "measures": MEASURES,
            "targets": plans["targets"],
        }
    raise SystemExit(f"interpretability_page: no finished batch of the standard table under {batches}")


def written(data: dict) -> str:
    """The snapshot as its file holds it: a line per target, so a refresh's
    diff is the targets."""
    head = {key: value for key, value in data.items() if key != "targets"}
    targets = [f'  {json.dumps(key)}: {json.dumps(value, separators=(",", ":"))}' for key, value in data["targets"].items()]
    return json.dumps(head, indent=1)[:-2] + ',\n "targets": {\n' + ",\n".join(targets) + "\n }\n}\n"


def plans(source: pathlib.Path) -> dict | None:
    """The snapshot, or `None` when it cannot be read or was taken with
    another span or other names than this script's."""
    try:
        taken = json.loads((source / PLANS).read_text(encoding="utf-8"))
        if (taken.get("span") != SPAN or taken.get("through") != THROUGH or taken.get("postures") != POSTURES
                or taken.get("marks") != MARKS):
            raise ValueError("the snapshot was taken with another span or other postures; rebuild it")
        if not taken["targets"]["all"]["seats"]:
            raise ValueError("no seat")
        return {key: value for key, value in taken.items() if key != "about"}
    except (OSError, ValueError, KeyError, TypeError, AttributeError) as error:
        print(f"interpretability_page: no plans ({error})", file=sys.stderr)
        return None


def interpretability(source: pathlib.Path) -> dict | None:
    """The record, or `None` when no part of it can be read."""
    out = {"turn": turn(source), "genes": genes(source), "plans": plans(source)}
    return out if any(out.values()) else None


def fill(page: str, data: dict | None) -> str | None:
    """The page with its slot filled, or `None` when it has no slot."""
    # `<` is escaped so no name or sentence can close the script element.
    text = json.dumps(data, ensure_ascii=False, separators=(",", ":")).replace("<", "\\u003c")
    filled, found = SLOT.subn(lambda m: m[1] + text + m[3], page, count=1)
    return filled if found else None


def main(argv: list[str]) -> int:
    if len(argv) == 4 and argv[1] == "--self-play":
        data = snapshot(pathlib.Path(argv[2]))
        out = pathlib.Path(argv[3])
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(written(data), encoding="utf-8")
        print(f"interpretability_page: the plans of {data['seats']} seats in {data['games']} games of batch {data['batch']} into {out}", file=sys.stderr)
        return 0
    if len(argv) not in (2, 3):
        print(__doc__, file=sys.stderr)
        return 2
    data = interpretability(pathlib.Path(argv[1]))
    if len(argv) == 2:
        json.dump(data, sys.stdout, ensure_ascii=False, indent=2)
        print()
        return 0
    page = pathlib.Path(argv[2])
    filled = fill(page.read_text(encoding="utf-8"), data)
    if filled is None:
        print(f"interpretability_page: {page} has no interpretability slot; left as it is", file=sys.stderr)
        return 0
    page.write_text(filled, encoding="utf-8")
    if data is None:
        print(f"interpretability_page: nothing to write into {page}", file=sys.stderr)
    else:
        steps = len((data["turn"] or {}).get("steps", []))
        ranked = len((data["genes"] or {}).get("rows", []))
        seats = (data["plans"] or {}).get("seats", 0)
        print(f"interpretability_page: wrote {steps} steps of the turn, {ranked} genes and the plans of {seats} seats into {page}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
