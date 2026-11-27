#!/usr/bin/env python3
"""The people's record: a person's finished Civilization VI games against the
Civ AI Agents, one line each on the `ledger` branch, which the ELO Rankings
page rates them from.

    people_elo.py record <run_dir>                   # a finished hotseat game
    people_elo.py record <run_dir> --winner person   # one that ended by concession
    people_elo.py list                               # the record, as the branch holds it
    people_elo.py ratings                            # each person's Elo, as the page fits it

Operator 2026-11-27: "Set me up in a 1v1 duel with the strongest civvis AI.
we should productionize this workflow ... also start posting elo on the
website. use the name dev for me". The game is `tools/civ6_hotseat.py --duel
--name dev`; this is what happens when it ends.

**The record** is `people.jsonl` at the root of the `ledger` branch, beside
the benchmark's `results.jsonl`. It is read and written through GitHub's
contents API (`gh api`), one small file at a time, and not with the plumbing
`civ6_ladder.publish_run` uses: the Mac a person plays on is rarely a seat,
and a clone that has never fetched the ledger branch would download every
run on it (gigabytes; measured on the first try, 2026-11-27) to add one
line. The API makes the commit on the branch's tip itself, so nothing is
fetched and nothing is forced; a write that lost a race (the file changed
since it was read) is read again and redone. A line is one game, under the
run's tag:

    {"tag": ..., "at": when it began, "ended": ..., "person": "dev",
     "leader": "LEADER_GILGAMESH", "agents": ["holy"], "winner": "person" or
     the agent that won, "victory": "VICTORY_RELIGIOUS", "turn": 143,
     "players": 2, "map": ..., "map_size": ..., "speed": ..., "difficulty": ...,
     "diplomatic": true, "revision": the decider's}

`record` reads the run's `summary.json` (`civ6_hotseat.py` writes the ending
for a person's table as it does for an unattended one). The harness calls it
when a named person's game ends with a victory or an elimination on record;
a game that ended any other way (the person conceded, or closed the game) has
no ending the game itself declared, and is recorded only when someone says who
won: `--winner person` or `--winner civvis`. Recording the same game twice
changes nothing; recording it again with a different answer corrects the line.

**The rating** is fitted where the page is built (`beta/elo_page.py`,
`people`): each game is a table of the person and the agents they met, each
agent at its published Elo when the game began, on the model the agents
themselves are fitted on. `ratings` prints that fit from this checkout.

What is not recorded: a `--auto-human` test stub's game, an unattended table
(no person at it), and a game whose person was not named (`--name`).
"""

from __future__ import annotations

import argparse
import base64
import json
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parent
#: The branch the record is on (`civ6_ladder.LEDGER_BRANCH`).
LEDGER_BRANCH = "ledger"
#: The people's record on the ledger branch.
PEOPLE_PATH = "people.jsonl"
#: `winner` when the person won; any other value is the agent that did.
PERSON = "person"
#: The flagship plays a CIVVIS seat no `--agents` names (`civ6_hotseat.STRATEGY`).
FLAGSHIP = "jack"
#: `victory` for a game nobody won by the game's own rules.
CONCESSION = "concession"


class NotRecorded(ValueError):
    """The run is not a person's finished game; the message says why."""


def table_agents(summary: dict) -> list[str]:
    """The agents at the table, in seat order: `--agents`' own, the flagship
    on a seat it did not name."""
    seats = [int(seat) for seat in summary.get("civvis_seats") or []]
    named = {int(seat): agent for seat, agent in (summary.get("seat_agents") or {}).items()}
    return [named.get(seat, FLAGSHIP) for seat in seats]


def game_row(summary: dict, *, winner: str | None = None, victory: str | None = None,
             name: str | None = None) -> dict:
    """One game's line from its run summary. `winner` (`person`, `civvis`, or
    an agent at the table) overrides the ending on record, for a game that
    ended by concession."""
    person = name or summary.get("person")
    if not person:
        raise NotRecorded("no person is named for this game (civ6_hotseat.py --name, or --name here)")
    agents = table_agents(summary)
    if not agents:
        raise NotRecorded("the summary names no CIVVIS seat: not a person's hotseat table")
    seats = [int(seat) for seat in summary.get("civvis_seats") or []]
    result = summary.get("result") if isinstance(summary.get("result"), dict) else None
    if winner is not None:
        if winner == "civvis":
            if len(set(agents)) != 1:
                raise NotRecorded(f"--winner civvis is ambiguous at a table of {', '.join(agents)}: "
                                  "name the agent that won")
            winner = agents[0]
        if winner != PERSON and winner not in agents:
            raise NotRecorded(f"--winner {winner}: not `person` and not at the table ({', '.join(agents)})")
        victory = victory or CONCESSION
        turn = (result or {}).get("turn") or summary.get("turn")
    elif result is None:
        raise NotRecorded("the game declared no ending (conceded or closed?): say who won "
                          "with --winner person or --winner civvis")
    elif result.get("kind") == "victory":
        # Every other seat at a person's table is ours, so a victory that is
        # not one of our seats' is the person's.
        if result.get("ours"):
            won_seat = result.get("winner_seat")
            if won_seat is None or int(won_seat) not in seats:
                raise NotRecorded("the summary does not say which CIVVIS seat won")
            winner = agents[seats.index(int(won_seat))]
        else:
            winner = PERSON
        victory = victory or result.get("victory_type") or result.get("victory")
        turn = result.get("turn")
    elif result.get("kind") == "defeat":
        # Every CIVVIS seat eliminated with the game still on: the person's.
        winner, victory, turn = PERSON, victory or "VICTORY_CONQUEST", result.get("turn")
    else:
        raise NotRecorded(f"the summary's ending is not one this reads: {result!r}")
    off = summary.get("victories_off")
    return {
        "tag": summary.get("tag"),
        "at": summary.get("started") or summary.get("utc"),
        "ended": summary.get("utc"),
        "person": person,
        "leader": summary.get("leader"),
        "agents": agents,
        "winner": winner,
        "victory": victory,
        "turn": turn,
        "players": summary.get("players"),
        "map": summary.get("map"),
        "map_size": summary.get("map_size"),
        "speed": summary.get("speed"),
        "difficulty": summary.get("difficulty"),
        "diplomatic": "VICTORY_DIPLOMATIC" not in off if isinstance(off, list) else None,
        "revision": summary.get("revision"),
    }


def parse_rows(text: str) -> dict[str, dict]:
    rows: dict[str, dict] = {}
    for line in text.splitlines():
        try:
            row = json.loads(line)
        except ValueError:
            continue
        if isinstance(row, dict) and row.get("tag"):
            rows[row["tag"]] = row
    return rows


def rows_text(rows: dict[str, dict]) -> str:
    """The record for these rows: oldest first, one compact line each, so the
    same rows are always the same bytes."""
    ordered = sorted(rows.values(), key=lambda row: (row.get("at") or "", row.get("tag") or ""))
    return "".join(json.dumps(row, sort_keys=True, separators=(",", ":")) + "\n" for row in ordered)


def gh_api(repo: Path, *args: str, body: bytes | None = None) -> tuple[int, bytes, str]:
    """One `gh api` call from the repository (it fills `{owner}/{repo}` in):
    its exit code, what it printed, and what it complained of."""
    try:
        done = subprocess.run(["gh", "api", *args], cwd=str(repo), input=body,
                              capture_output=True, check=False, timeout=120)
    except (OSError, subprocess.SubprocessError) as error:
        return 1, b"", str(error)
    return done.returncode, done.stdout, done.stderr.decode("utf-8", "replace")


def branch_record(repo: Path, branch: str = LEDGER_BRANCH, api=gh_api) -> tuple[str, str | None]:
    """The record as the branch holds it, and the file's id there (what a
    write must name to replace it): `("", None)` before its first line."""
    where = f"repos/{{owner}}/{{repo}}/contents/{PEOPLE_PATH}?ref={branch}"
    code, out, err = api(repo, where)
    if code != 0:
        if "HTTP 404" in err:
            return "", None
        raise RuntimeError(f"the people's record could not be read: {err.strip()}")
    listed = json.loads(out)
    text = base64.b64decode(listed.get("content") or "").decode("utf-8", "replace")
    if not text and listed.get("size"):
        # Past 1 MB the listing leaves the content out; the raw reading has it.
        code, out, err = api(repo, "-H", "Accept: application/vnd.github.raw+json", where)
        if code != 0:
            raise RuntimeError(f"the people's record could not be read: {err.strip()}")
        text = out.decode("utf-8", "replace")
    return text, listed.get("sha")


def publish(row: dict, *, branch: str = LEDGER_BRANCH, repo: Path | None = None,
            api=gh_api, attempts: int = 3) -> str:
    """Put one game's line on the ledger branch. Returns "published", or
    "already" when the branch's line for the game says the same.

    The write names the file it read; one that lost a race with another
    writer is refused by GitHub, read again and redone, never forced."""
    if not row.get("tag"):
        raise NotRecorded("the game has no tag to be recorded under")
    repo = Path(repo or REPO)
    last_error = ""
    for _ in range(max(1, attempts)):
        text, sha = branch_record(repo, branch, api)
        rows = parse_rows(text)
        if rows.get(row["tag"]) == row:
            return "already"
        rows[row["tag"]] = row
        body = {"message": f"ledger: people ({row['person']}, {row['tag']})",
                "content": base64.b64encode(rows_text(rows).encode()).decode(),
                "branch": branch, **({"sha": sha} if sha else {})}
        code, _, last_error = api(repo, "-X", "PUT", f"repos/{{owner}}/{{repo}}/contents/{PEOPLE_PATH}",
                                  "--input", "-", body=json.dumps(body).encode())
        if code == 0:
            return "published"
    raise RuntimeError(f"the people's record was not written after {attempts} attempts: "
                       f"{last_error.strip()}")


def said(row: dict) -> str:
    """One game as a sentence."""
    against = ", ".join(row.get("agents") or [])
    won = row.get("winner") == PERSON
    how = str(row.get("victory") or "").removeprefix("VICTORY_").lower() or "?"
    return (f"{row.get('at') or '?'}  {row.get('person')} {'beat' if won else 'lost to'} "
            f"{against if won or len(row.get('agents') or []) == 1 else row.get('winner')}"
            f"  ({how}, turn {row.get('turn') or '?'}, {row.get('players')} players, "
            f"{str(row.get('map_size') or '').removeprefix('MAPSIZE_').title()} "
            f"{str(row.get('map') or '').removesuffix('.lua')})  [{row.get('tag')}]")


def ratings_text(repo: Path, text: str) -> str:
    """Each person's Elo as the ELO Rankings page fits it: the record's
    `text`, against the published fits in this checkout's `data/`."""
    sys.path.insert(0, str(HERE.parent / "beta"))
    import elo_page  # noqa: PLC0415 - the site's own fit, read where it lives

    lines = []
    met = elo_page.people_games(repo, text) or []
    for side, on, kept, published in (("Diplo on", True, elo_page.HISTORY, elo_page.PUBLISHED),
                                      ("Diplo off", False, elo_page.NO_DIPLO_HISTORY,
                                       elo_page.NO_DIPLO_PUBLISHED)):
        points = elo_page.history(repo, published, kept)
        for person in elo_page.people([g for g in met if g["diplo"] is on], points):
            bound = {"lower": "at least ", "upper": "at most "}.get(person["bound"], "")
            lines.append(f"{person['name']}: Elo {bound}{person['elo']:.0f} \u00b1{person['margin']} "
                         f"({person['wins']} of {person['games']} won, {side})")
    return "\n".join(lines) or "nobody is rated yet"


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0],
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="command", required=True)
    rec = sub.add_parser("record", help="put a finished game on the people's record")
    rec.add_argument("run_dir", type=Path, help="the hotseat run (holds summary.json)")
    rec.add_argument("--winner", default=None, metavar="WHO",
                     help="`person`, `civvis` or an agent at the table: who won a game the "
                          "game itself did not end (a concession)")
    rec.add_argument("--victory", default=None, help=f"how (default {CONCESSION} with --winner)")
    rec.add_argument("--name", default=None, help="the person, for a game started without --name")
    rec.add_argument("--dry-run", action="store_true", help="print the line; push nothing")
    for name in ("record", "list", "ratings"):
        parser = rec if name == "record" else sub.add_parser(
            name, help="the record on the branch" if name == "list" else "each person's Elo")
        parser.add_argument("--branch", default=LEDGER_BRANCH)
        parser.add_argument("--repo", type=Path, default=REPO)
    args = ap.parse_args(argv)
    if args.command == "record":
        try:
            summary = json.loads((args.run_dir / "summary.json").read_text(encoding="utf-8"))
            row = game_row(summary, winner=args.winner, victory=args.victory, name=args.name)
        except (OSError, ValueError) as error:
            print(f"people_elo: not recorded: {error}", file=sys.stderr)
            return 2
        if args.dry_run:
            print(json.dumps(row, sort_keys=True, indent=2))
            return 0
        state = publish(row, branch=args.branch, repo=args.repo)
        print(f"people_elo: {state}: {said(row)}")
    text, _ = branch_record(args.repo, args.branch)
    if args.command == "list":
        print("\n".join(said(row) for row in parse_rows(rows_text(parse_rows(text))).values())
              or "no games on record")
    else:
        print(ratings_text(args.repo, text))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
