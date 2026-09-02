#!/usr/bin/env python3
"""Does the CIVVIS board actually agree with the Civilization VI export?

One command. Every check here exists because the corresponding field was once
populated, plausible and WRONG -- so nothing below asks "is it non-empty", it
asks "does it AGREE".

    python3 tools/civ6_mirror_check.py [run-dir]        # newest run if omitted

Requires the mirror server on :8610 (see civvis-civ6-mirror/follow.py).

## What each line is guarding, and what it caught

- FOG     every exported plot must be ON the board. Remembered ground used to be
          dropped outright rather than dimmed: 2 of 6 charted plots survived.
- SETUP   Civ VI's seat settings must be the board's settings. A mirror that says
          Prince/Pangaea beside a Settler/Continents game is wrong before its first move.
- RIVERS  a Civilization VI river plot is fresh water BY DEFINITION, so CIVVIS's
          river tiles must be `fw` in the export far above the base rate. Read
          36.4% against a 25.7% base -- chance -- while the board showed the
          GENERATED map's rivers.
- LAND    every land plot the export names a continent for must carry one. Read
          200 of 776, with 336 WATER tiles carrying one they should not have.
- CLIFFS  Civilization VI exposes no cliff accessor, so any cliff on the board
          was invented by the map generator. Must be zero.
- CITIES  compare the SETS and name what is missing, never the counts.
- UNITS   likewise. "21 exported, 15 reconstructed" once read as healthy because
          nothing compared them.
- HOSTILES every hostile the seat can SEE must be on the board. FOG-GATED, and one
          direction only: the export's threat list is not fog-gated and the board is
          the seated view, so an unseen hostile is correctly absent.
- TREASURY gold and faith are BALANCES (`GetGoldBalance`, `GetFaithBalance`), not
          the per-turn rates `economy_drift` compares. Same turn, or the delta is
          just income.

## Ten ways this checker itself cried wolf

Each of these is a real bug I nearly reported, caught only by looking again:

1. It compared the LATEST export against a board published up to 30s earlier and
   called the game's own progress "13 exported plots dropped". The export is now
   bounded to the board's turn -- see `load_export(upto=)`.
2. It judged rivers on an absolute share and called a 4.3x lift over the base
   rate "no better than chance". Judge the LIFT.
3. It asserted water carries no continent. Civilization VI really does put COAST
   tiles on a continent -- 17 read CONTINENT_AUSTRALIA on one run -- so the check
   is agreement with the export, not a rule of its own.
4. TREASURY, read against the NEWEST export rather than the board's own turn,
   showed `gold 176 vs 167` and `faith 23 vs 21` -- a confident 5% shortfall that
   was one turn of income at +9 gold and +2 faith. Bounded, the same instant read
   134/134 and 27/27. That is defect 1 again, in a new check, which is why the
   note below its `latest_state` says to bound every future reader.
5. HOSTILES, on its very first run, printed `export 0  board 1` as though those
   were a matched pair. They are not: the board's non-seat units include rivals
   and city-states, the export's `hostiles` is only the threat list. The count
   comparison was removed the same minute it was written -- a line that invites a
   false reading is worse than no line.
6. HOSTILES again, one run later: it reported `8 exported, 5 NOT on the board` on
   a healthy game. `hostiles` is the planner's threat list and is NOT fog-gated;
   the board is the SEATED view. The check was asking the board to hold units the
   seat cannot see. It is now gated on `visible`, and the decider's own
   `dropped_units` -- which recorded no hostile dropped -- was what disproved it.
7. RIVALS and PUBLIC were re-indexed to `rival["player"]` (#878) on the belief
   that met-order pairing was an off-by-one. IT WAS NOT: mirror.rs compacts
   rivals into seats 1..n in export order and says so. The re-index left two
   tests in this file red on main -- Python tests are not covered by cargo-test
   -- and it SILENCED a real disagreement (Egypt at CIVVIS seat 1 where the
   export's first rival was Netherlands). Reverted. ⚠ THE LESSON IS THE
   DANGEROUS DIRECTION: the first six entries here were checks that cried wolf,
   and the reflex they build is to distrust the check. This one was the check
   being RIGHT. When a check disagrees with the board, read the mirror's own
   contract before re-indexing the ruler.
8. It treated a live decision handoff as a missing host frame. The host writes
   `state N`, CIVVIS decides and applies orders, and only then writes playable
   `turn N`; during that ordinary interval the exact state already exists and
   is the frame the viewer reconstructed. A live check now validates it and
   reports the still-completing turn instead of calling it drift.
9. It reported a JUST-MET minor's capital as "missing" from the board. `state`
   updates the moment a city-state is met, but its city plot enters the tiles
   stream only at the next `TileExportEvery` boundary, so for up to that many
   turns the board has nowhere to put a city the state already names. Measured
   live at turn 163 (Johannesburg, met between the turn-160 and turn-164
   exports; clean again at 165). A one-report disagreement that names a minor
   met within the export interval is this skew, not a dropped city -- re-check
   after the next export before reporting it.
10. It compared a board published from state frame 0 with state frames 1 and 2
    that the host had appended for the same turn. Units that moved during the
    replan then appeared to be missing, even though the next completed-turn
    publication was clean. A live checker now defers this narrow handoff until
    the `turn` completion marker exists, rather than turning an in-flight frame
    into a parity alarm.

⚠ The board served on :8610 is follow.py's FLIPPED staged copy:
`board_axial = offset_to_axial(x, TOP - y)`. The flip constant is discovered here
rather than assumed, because comparing two coordinate frames without first
proving they overlap has already produced one confident, wrong finding.
"""

import argparse
import glob
import json
import math
import os
import subprocess
import sys
import time
import urllib.request
from collections import Counter
from pathlib import Path

from civ6_fidelity import ALIASES as IDENTIFIER_ALIASES

# The same root `civ6_civvis_climb.py` writes to, resolved from $HOME rather than
# hardcoded so this works on any machine that runs the ladder.
RUNS = str(Path.home() / "civvis-civ6-runs" / "control")
PORT = int(os.environ.get("CIVVIS_MIRROR_PORT", "8610"))
VOCABULARY = json.loads(
    (Path(__file__).resolve().parent / "civ6_control" / "vocab.json").read_text()
)
MIRRORED_IMPROVEMENTS = set(json.loads(
    (Path(__file__).resolve().parent.parent / "data" / "improvements.json").read_text()
))
MIRRORED_WONDERS = set(json.loads(
    (Path(__file__).resolve().parent.parent / "data" / "wonders.json").read_text()
))
MIRRORED_UNIT_RULES = json.loads(
    (Path(__file__).resolve().parent.parent / "data" / "units.json").read_text()
)
UNIT_MODEL_FALLBACKS = {
    # These two host-only variants now have exact CIVVIS specs, so their
    # implementation prefix must stay intact in the audit. Other barbarian
    # variants still fall through to the ordinary stock role below.
    "barbarian_horseman": "barbarian_horseman",
    "barbarian_horse_archer": "barbarian_horse_archer",
    # Firaxis's Scythian Horse Archer shares the modeled Saka role.
    "horse_archer": "saka_horse_archer",
    # Exact stock roles from Firaxis's UnitReplaces table. CIVVIS does not yet
    # carry these unique specifications, but it must not erase the visible unit.
    "scottish_highlander": "ranger",
    "korean_hwacha": "field_cannon",
}
RESOURCE_RULES = json.loads(
    (Path(__file__).resolve().parent.parent / "data" / "resources.json").read_text()
)


def newest_run():
    dirs = [d for d in glob.glob(os.path.join(RUNS, "*")) if os.path.isdir(d)]
    live = [d for d in dirs if os.path.exists(os.path.join(d, "events.jsonl"))]
    return max(live, key=lambda d: os.path.getmtime(os.path.join(d, "events.jsonl")))


def live_runtime_problems(run, process_text=None, now=None, max_age=120.0):
    """Find a live Firaxis process that no longer has a state/control producer."""
    if process_text is None:
        process_text = subprocess.run(
            ["ps", "-axo", "command="], capture_output=True, text=True, check=False
        ).stdout
    now = time.time() if now is None else now
    lines = process_text.splitlines()
    game_running = any("Civ6_Exe_Child" in line for line in lines)
    tag = os.path.basename(os.path.abspath(run))
    controllers = [line for line in lines if "civ6_play.py" in line and tag in line]
    brains = [line for line in lines if "civ6_brain.py" in line and tag in line]
    events = os.path.join(run, "events.jsonl")
    try:
        age = max(0.0, now - os.path.getmtime(events))
    except OSError:
        age = float("inf")

    problems = []
    if game_running and not controllers:
        problems.append("Firaxis is running but this run's controller is absent")
    if controllers and any("--civvis-decides" in line for line in controllers) and not brains:
        problems.append("the CIVVIS decision worker is absent")
    if game_running and age > max_age:
        problems.append(f"the Firaxis export is {age:.0f}s stale")
    problems.extend(stale_rig_problems(lines))
    return problems


# The deployed binary that RENDERS the board this file compares against. It is not
# a git checkout and nothing rebuilds it.
RIG_BINARY = str(Path.home() / "civvis-civ6-mirror" / "civvis")


def stale_rig_problems(process_lines, rig=RIG_BINARY):
    """Is the board being SERVED built by older code than the board being DECIDED?

    ⚠⚠ THE LINE ABOVE USED TO SAY "export and CIVVIS worker are current" HAVING
    CHECKED ONLY THAT THE WORKER PROCESS EXISTS. Presence is not currency, and the
    difference cost a whole diagnosis.

    Measured 2026-08-02. `/Users/martin/civvis-civ6-mirror/civvis` — a DEPLOYED
    binary, not a git checkout, that nothing rebuilds — was dated Aug 1 02:52 while
    the decider's binary was minutes old. `follow.py` stages into that rig and
    `civvis play --serve` renders it, so both the CIVVIS window and every check in
    this file were reading a DAY-OLD reconstruction of a current game. CONTROL
    reported OK throughout.

    One rebuild moved four whole axes from failing to passing:

        BEFORE  setup, tiles, knowledge, city-states, public facts, city facts, unit facts
        AFTER   tiles, knowledge, unit facts

    including `SETUP ⚠ speed Civ6=online CIVVIS=standard`, which I was one step
    from chasing as a code defect. That one matters on its own: Online costs are
    HALF of Standard, so a reconstruction running Standard prices every build and
    every tech wrong.

    The decider's binary is named on the brain's own command line (`--bin`), so the
    comparison needs no configuration — it asks the running system what it is using.

    ⚠⚠⚠ AND SO IS THE SERVER'S, WHICH THIS USED TO ASSUME INSTEAD OF ASKING. `rig`
    defaulted to a hardcoded `~/civvis-civ6-mirror/civvis`, so the moment `follow.py`
    was started from a checkout — its repo copy resolves `BIN` to
    `<repo>/target/release/civvis` — this compared a file NOBODY WAS RUNNING and
    reported on it with a straight face. That is the same failure the paragraphs
    above describe, in a new costume: the answer was about the wrong artefact, and
    it read exactly like an answer about the right one. The served binary is on
    `civvis play --mirror`'s command line; take it from there and keep the constant
    only as the last resort.
    """
    def named(needle, flag):
        for line in process_lines:
            if needle not in line or flag not in line:
                continue
            parts = line.split()
            if flag in parts:
                index = parts.index(flag)
                if index + 1 < len(parts):
                    return parts[index + 1]
        return None

    wanted = named("civ6_brain.py", "--bin")
    if wanted is None:
        return []
    # `civvis play --mirror <stage>` is how follow.py serves the board; argv[0] is
    # the binary doing it. ⚠ `nice -n 5 <bin> play --mirror …` prefixes the line, so
    # find the word before `play` rather than taking the first one.
    served = None
    for line in process_lines:
        if "play" not in line or "--mirror" not in line:
            continue
        parts = line.split()
        if "play" in parts:
            index = parts.index("play")
            if index > 0 and "civ6_" not in parts[index - 1]:
                served = parts[index - 1]
                break
    served = served or rig
    try:
        rig_at = os.path.getmtime(served)
        decider_at = os.path.getmtime(wanted)
    except OSError:
        # ⚠ Absent is not stale. A rig that is not there at all is a different
        # failure and belongs to whoever starts the server.
        return []
    if rig_at >= decider_at:
        return []
    behind = (decider_at - rig_at) / 3600.0
    return [
        f"the served board is built by a rig binary {behind:.1f}h older than the "
        f"decider's ({served} vs {wanted}) — rebuild it "
        f"(cargo build --release --bin civvis), then restart follow.py AND the "
        f"server, because a running one keeps its inode. "
        f"⚠ mtime is a PROXY for the code: it warns on a fresh copy of identical "
        f"sources and stays silent on a stale binary someone touched. Confirm with "
        f"git before rebuilding"
    ]


def axial(x, y):
    return (x - ((y - (y & 1)) // 2), y)


def civ6_id(value, prefix):
    """Normalize a Civ VI type identifier to a lower-case CIVVIS-style id."""
    value = str(value or "").strip()
    if value.upper().startswith(prefix):
        value = value[len(prefix):]
    return value.lower()


def civ6_map_script(value):
    """Normalize the Civ VI map-file spelling CIVVIS mirrors."""
    value = civ6_id(value, "")
    if value.endswith(".lua"):
        value = value[:-4]
    return {"smallcontinents": "small_continents"}.get(value, value)


def civ_id_matches(civ6, civvis):
    """Compare roster ids after the bridge's singular/plural normalization."""
    civ6 = str(civ6 or "").lower().removesuffix("_stk")
    civvis = str(civvis or "").lower()
    return civ6 == civvis or civ6.rstrip("s") == civvis.rstrip("s")


def leader_id_matches(civ6, civvis):
    """CIVVIS stores the shared leader identity for Firaxis alternate personas."""
    civ6 = str(civ6 or "").lower()
    civvis = str(civvis or "").lower()
    aliases = {
        "harald_alt": "harald_hardrada",
        "suleiman_alt": "suleiman",
    }
    return civ6 == civvis or civ6.removesuffix("_alt") == civvis or aliases.get(civ6) == civvis


def public_age(source):
    """Return a host-confirmed public age, or None for an older export."""
    if source.get("heroic_golden_age") is True:
        return "heroic"
    if source.get("golden_age") is True:
        return "golden"
    if source.get("dark_age") is True:
        return "dark"
    age_flags = ("heroic_golden_age", "golden_age", "dark_age")
    if all(source.get(flag) is False for flag in age_flags):
        return "normal"
    return None


def public_government(source):
    """Normalize a host-confirmed government to the board's vocabulary."""
    government = source.get("government")
    return civ6_id(government, "GOVERNMENT_") if isinstance(government, str) else None


def rival_identity_mismatches(state, board):
    """Compare each exported rival with the compact CIVVIS seat that owns it."""
    players = {player.get("id"): player for player in board.get("players") or []}
    mismatches = []
    # ⚠⚠ COMPACTED SEATS, BY LIST POSITION — and I broke this once by "fixing" it.
    #
    # mirror.rs says it outright: "Rival entities are deliberately compacted into
    # seats 1..n in export order; `rival.player` is the original Firaxis id and is
    # used only when translating". So the Nth exported rival IS CIVVIS seat N, and
    # `enumerate(rivals, start=1)` is correct.
    #
    # #878 changed this to pair by `rival["player"]` on the strength of one live
    # run where the two happened to coincide. That did two bad things: it left two
    # tests in this file failing on main (Python tests are not covered by
    # cargo-test, so CI stayed green), and — much worse — it SILENCED A REAL
    # DEFECT. The run that motivated it genuinely had Egypt at CIVVIS seat 1 while
    # the export's first rival was Netherlands. That is exactly the disagreement
    # this axis exists to report, and I turned it off by changing the ruler.
    #
    # ⚠ If the two conventions ever genuinely diverge, fix the MIRROR or say so
    # here — do not quietly re-index the check until it agrees.
    for seat, rival in enumerate(state.get("rivals") or [], start=1):
        player = players.get(seat, {})
        expected_civ = civ6_id(rival.get("civ"), "CIVILIZATION_")
        expected_leader = civ6_id(rival.get("leader"), "LEADER_")
        actual_civ = str(player.get("civ") or "").replace(" ", "_").lower()
        actual_leader = civ6_id(player.get("leader_type"), "LEADER_") \
            if player.get("leader_type") else \
            str(player.get("leader") or "").replace(" ", "_").lower()
        wrong_civ = expected_civ and not civ_id_matches(expected_civ, actual_civ)
        wrong_leader = expected_leader and not leader_id_matches(expected_leader, actual_leader)
        if wrong_civ or wrong_leader:
            mismatches.append(
                f"seat {seat} Civ6={expected_civ or '?'} / {expected_leader or '?'} "
                f"CIVVIS={actual_civ or '?'} / {actual_leader or '?'}"
            )
    return mismatches


def public_fact_mismatches(state, board):
    """Compare public player-HUD facts against the host's current export."""
    players = {player.get("id"): player for player in board.get("players") or []}
    # Same compacted-seat rule as `rival_identity_mismatches`. See the note there,
    # including why re-indexing this to `rival["player"]` was a mistake.
    expected = [(0, state)] + list(enumerate(state.get("rivals") or [], start=1))
    mismatches = []
    for seat, source in expected:
        player = players.get(seat, {})
        government = public_government(source)
        if government and player.get("government") != government:
            mismatches.append(
                f"seat {seat} government Civ6={government} "
                f"CIVVIS={player.get('government')!r}"
            )
        age = public_age(source)
        if age and player.get("age") != age:
            mismatches.append(
                f"seat {seat} age Civ6={age} CIVVIS={player.get('age')!r}"
            )
        # ⚠ COMPARE THE MAPPING, NOT THE MODEL. `military` on the board is
        # `military_power`, which is deliberately `max(observed, our own strength
        # sum)`. For our OWN seat we can see every unit, so that sum can win the
        # max and legitimately exceed the host's figure — this check reported
        # `seat 0 military Civ6=520 CIVVIS=545` as a DISAGREEMENT when the bridge
        # was perfect and only CIVVIS's strength model differed.
        #
        # Measured before changing it: over 2,713 turn-records the host's figure
        # wins that max ~90% of the time, so the warning is rare, benign and
        # exactly the kind that teaches an operator to ignore the whole report.
        # `observed_military` is the mapped value alone, which is what a BRIDGE
        # check should verify.
        for key, board_key in (("score", "score"), ("military", "observed_military")):
            want = source.get(key)
            got = player.get(board_key)
            if isinstance(want, (int, float)) and want >= 0 \
                    and (not isinstance(got, (int, float)) or abs(got - want) > 0.51):
                mismatches.append(f"seat {seat} {key} Civ6={want:g} CIVVIS={got!r}")
        public_stats = source.get("public_stats") or {}
        if isinstance(public_stats, dict):
            for key, board_key in (
                    ("city_count", "cities"),
                    ("population", "population"),
                    ("wonder_count", "wonder_count"),
                    ("suzerain_count", "suzerain_count"),
                    ("nuclear_devices", "nuclear_devices"),
                    ("thermonuclear_devices", "thermonuclear_devices")):
                want = public_stats.get(key)
                got = player.get(board_key)
                if isinstance(want, (int, float)) and want >= 0 \
                        and (not isinstance(got, (int, float)) or abs(got - want) > 0.51):
                    label = "cities" if key == "city_count" else key
                    mismatches.append(
                        f"seat {seat} {label} Civ6={want:g} CIVVIS={got!r}"
                    )
            player_yields = player.get("yields") or {}
            for key in ("food", "production"):
                want = public_stats.get(key)
                got = player_yields.get(key)
                if isinstance(want, (int, float)) and want >= 0 \
                        and (not isinstance(got, (int, float)) or abs(got - want) > 0.11):
                    mismatches.append(
                        f"seat {seat} {key}/turn Civ6={want:g} CIVVIS={got!r}"
                    )
        # A rival's per-turn Science and Culture and its treasury cross the
        # bridge too (the host reads them for every player, as its World
        # Rankings does); the board carries them the same way as seat 0's, so
        # a rival seat must agree to the rounding as well. Seat 0's own are
        # compared below from the state's top-level fields.
        if seat > 0:
            rival_yields = player.get("yields") or {}
            for key in ("science", "culture", "faith_per_turn"):
                want = source.get(key)
                board_key = "faith" if key == "faith_per_turn" else key
                got = rival_yields.get(board_key)
                if isinstance(want, (int, float)) and want >= 0 \
                        and (not isinstance(got, (int, float)) or abs(got - want) > 0.11):
                    label = "faith" if key == "faith_per_turn" else key
                    mismatches.append(f"seat {seat} {label}/turn Civ6={want:g} CIVVIS={got!r}")
            for key in ("gold", "faith"):
                want = source.get(key)
                got = player.get(key)
                if isinstance(want, (int, float)) and want >= 0 \
                        and (not isinstance(got, (int, float)) or abs(got - want) > 0.11):
                    mismatches.append(f"seat {seat} {key} Civ6={want:g} CIVVIS={got!r}")
            # Gold per turn is a rate, and unlike the treasury can genuinely
            # be negative. The rival wire has historically used a finite
            # sentinel when the host refuses the query, so compare every
            # finite value exactly as the mirror maps it.
            want = source.get("gold_per_turn")
            got = player.get("gold_per_turn")
            if isinstance(want, (int, float)) and math.isfinite(want) \
                    and (not isinstance(got, (int, float)) or abs(got - want) > 0.11):
                mismatches.append(f"seat {seat} gold/turn Civ6={want:g} CIVVIS={got!r}")
            for key, victory in (("techs", "science"), ("civics", "culture")):
                want = source.get(key)
                got = ((player.get("victories") or {}).get(victory) or {}).get(key)
                if isinstance(want, (int, float)) and want >= 0 \
                        and (not isinstance(got, (int, float)) or abs(got - want) > 0.51):
                    mismatches.append(f"seat {seat} {key} Civ6={want:g} CIVVIS={got!r}")
            want = source.get("tourism")
            got = player.get("tourism_per_turn")
            if isinstance(want, (int, float)) and want >= 0 \
                    and (not isinstance(got, (int, float)) or abs(got - want) > 0.11):
                mismatches.append(
                    f"seat {seat} tourism/turn Civ6={want:g} CIVVIS={got!r}"
                )

    ours = players.get(0, {})
    yields = ours.get("yields") or {}
    for key in ("science", "culture"):
        want = state.get(key)
        got = yields.get(key)
        if isinstance(want, (int, float)) and want > 0 \
                and (not isinstance(got, (int, float)) or abs(got - want) > 0.11):
            mismatches.append(f"seat 0 {key}/turn Civ6={want:g} CIVVIS={got!r}")
    # Faith per turn is a RATE like science and culture, and it is NOT the
    # sum of the cities: the host pays the Great Person points of a class the
    # empire can no longer earn out as Faith (run civvis-20260816T123936Z: 100+
    # a turn banked against 49 from every city together). The mod exports it
    # from `GetReligion():GetFaithYield()`; an older export has no key, and a
    # missing answer is `null` — neither is a disagreement.
    want = state.get("faith_per_turn")
    got = yields.get("faith")
    if isinstance(want, (int, float)) and want >= 0 \
            and (not isinstance(got, (int, float)) or abs(got - want) > 0.11):
        mismatches.append(f"seat 0 faith/turn Civ6={want:g} CIVVIS={got!r}")
    for key in ("gold", "faith"):
        want = state.get(key)
        got = ours.get(key)
        if isinstance(want, (int, float)) and want >= 0 \
                and (not isinstance(got, (int, float)) or abs(got - want) > 0.11):
            mismatches.append(f"seat 0 {key} Civ6={want:g} CIVVIS={got!r}")
    want = state.get("gold_per_turn")
    got = ours.get("gold_per_turn")
    if isinstance(want, (int, float)) and math.isfinite(want) \
            and (not isinstance(got, (int, float)) or abs(got - want) > 0.11):
        mismatches.append(f"seat 0 gold/turn Civ6={want:g} CIVVIS={got!r}")
    capacity = state.get("trade_capacity")
    mirrored_capacity = (board.get("me") or {}).get("trade_capacity")
    if isinstance(capacity, (int, float)) and capacity >= 0 \
            and mirrored_capacity != capacity:
        mismatches.append(
            f"seat 0 trade_capacity Civ6={capacity:g} CIVVIS={mirrored_capacity!r}"
        )
    return mismatches


def mirrored_minor_sources(state):
    """Return real city-states and non-dormant Free Cities actors."""
    out = []
    for source in state.get("minors") or []:
        civ = source.get("civ")
        if civ == "CIVILIZATION_BARBARIAN":
            continue
        if civ == "CIVILIZATION_FREE_CITIES" \
                and not (source.get("cities") or source.get("units")):
            continue
        if civ:
            out.append(source)
    return out


def mirrored_minor_name(source):
    """Return the rendered actor name, not a potentially stale Firaxis type id."""
    cities = source.get("cities") or []
    capital = next((city for city in cities if city.get("capital")), None)
    city_name = (capital or (cities[0] if cities else {})).get("name")
    if city_name:
        return str(city_name).lower()
    return civ6_id(source.get("civ"), "CIVILIZATION_").replace("_", " ")


def minor_fact_mismatches(state, board, top):
    """Compare non-major identities, cities and public diplomacy facts."""
    sources = mirrored_minor_sources(state)
    players = list(board.get("players") or [])
    actual = [player for player in players
              if player.get("is_minor") and not player.get("is_barbarian")]
    free_cities = next((player for player in players if player.get("is_free_city")), None)
    cities = {tuple(city.get("pos") or []): city for city in board.get("cities") or []}
    host_to_board = {0: 0}
    host_to_board.update({rival.get("player"): seat
                          for seat, rival in enumerate(state.get("rivals") or [], start=1)})
    for source in sources:
        want = mirrored_minor_name(source)
        matched = free_cities if source.get("civ") == "CIVILIZATION_FREE_CITIES" else next(
            (candidate for candidate in actual
             if str(candidate.get("civ") or "").lower() == want), None
        )
        if matched is not None:
            host_to_board[source.get("player")] = matched.get("id")
    mismatches = []
    for source in sources:
        want = mirrored_minor_name(source)
        player = free_cities if source.get("civ") == "CIVILIZATION_FREE_CITIES" else next(
            (candidate for candidate in actual
             if str(candidate.get("civ") or "").lower() == want), None
        )
        if player is None:
            mismatches.append(f"missing minor actor {want or source.get('player')}")
            continue
        for key in ("score", "military"):
            expected = source.get(key)
            # `military` is the model's max(observed host strength, visible
            # unit sum).  That distinction matters for city-states too: their
            # units are often fully visible, so the reconstructed sum can be
            # higher than the host's public ribbon value.  Compare the
            # host-only field when the current board exports it, while keeping
            # the old `military` fallback for boards built before that field
            # existed.
            board_key = "observed_military" if key == "military" else key
            got = player.get(board_key, player.get(key))
            if isinstance(expected, (int, float)) and expected >= 0 \
                    and (not isinstance(got, (int, float)) or abs(got - expected) > 0.51):
                mismatches.append(f"{want} {key} Civ6={expected:g} CIVVIS={got!r}")
        if source.get("civ") != "CIVILIZATION_FREE_CITIES":
            expected_envoys = source.get("envoys")
            if isinstance(expected_envoys, (int, float)) \
                    and player.get("my_envoys") != expected_envoys:
                mismatches.append(
                    f"{want} envoys Civ6={expected_envoys:g} CIVVIS={player.get('my_envoys')!r}"
                )
            suzerain = source.get("suzerain")
            expected_suzerain = None if suzerain in (None, -1) else host_to_board.get(suzerain)
            if (suzerain in (None, -1) or expected_suzerain is not None) \
                    and player.get("suzerain") != expected_suzerain:
                mismatches.append(
                    f"{want} suzerain Civ6={suzerain!r} "
                    f"CIVVIS={player.get('suzerain')!r}"
                )
        for city in source.get("cities") or []:
            pos = axial(city.get("x", 0), top - city.get("y", 0))
            mirrored = cities.get(pos)
            if mirrored is None or mirrored.get("owner") != player.get("id"):
                mismatches.append(f"{want} city {city.get('name') or pos} missing at {pos}")
    return mismatches


def city_fact_mismatches(state, board, top):
    """Compare every host city field that has a CIVVIS representation."""
    by_pos = {tuple(city.get("pos") or []): city for city in board.get("cities") or []}
    mismatches = []
    for source in state.get("cities") or []:
        pos = axial(source.get("x", 0), top - source.get("y", 0))
        city = by_pos.get(pos)
        if city is None:
            continue
        name = source.get("name")
        if name and city.get("name") != name:
            mismatches.append(f"{name}@{pos} name={city.get('name')!r}")
        # `housing` and `amenities` are on this list since the yield-fidelity
        # work: the board carries a host-to-model correction for both (the
        # Amenity surplus already did; the count and the Housing ceiling now
        # do too), so a disagreement here is the bridge, not the model.
        # `amenities_needed` has no correction — CIVVIS's ceil(pop/2) IS the
        # host's rule — so it guards the rule itself.
        for key, board_key, tolerance in (
            ("pop", "pop", 0), ("food", "food", 0.11), ("loyalty", "loyalty", 0.11),
            ("loyalty_per_turn", "loyalty_per_turn", 0.11), ("defense", "defense", 0.11),
            ("housing", "housing", 0.11), ("amenities", "amenities", 0.11),
            ("amenities_needed", "amenities_required", 0.11),
        ):
            want, got = source.get(key), city.get(board_key)
            if isinstance(want, (int, float)) and want >= 0 \
                    and (not isinstance(got, (int, float)) or abs(got - want) > tolerance):
                mismatches.append(f"{name or pos} {key} Civ6={want:g} CIVVIS={got!r}")
        # Per-city yields: the board's figure is the host's plus nothing, by
        # construction (`observed_city_yield_adjustments`); a gap here means the
        # correction machinery itself broke, which the seat totals cannot show
        # once two cities' errors cancel.
        want_yields = source.get("yields")
        got_yields = city.get("yields")
        if isinstance(want_yields, dict) and isinstance(got_yields, dict):
            for key in ("food", "production", "gold", "science", "culture", "faith"):
                want, got = want_yields.get(key), got_yields.get(key)
                if isinstance(want, (int, float)) and want >= 0 \
                        and (not isinstance(got, (int, float)) or abs(got - want) > 0.11):
                    mismatches.append(
                        f"{name or pos} yields.{key} Civ6={want:g} CIVVIS={got!r}"
                    )
        damage, max_damage = source.get("damage"), source.get("max_damage")
        if all(isinstance(value, (int, float)) and value >= 0
               for value in (damage, max_damage)) and max_damage > 0:
            want_hp = max(1, min(200, int(200 * (max_damage - damage) / max_damage + 0.5)))
            if city.get("hp") != want_hp:
                mismatches.append(
                    f"{name or pos} hp Civ6={want_hp} CIVVIS={city.get('hp')!r}"
                )
        wall_damage = source.get("wall_damage")
        max_wall_damage = source.get("max_wall_damage")
        if all(isinstance(value, (int, float)) and value >= 0
               for value in (wall_damage, max_wall_damage)):
            want_wall_max = int(max_wall_damage + 0.5)
            want_wall_hp = int(max(0, min(max_wall_damage,
                                          max_wall_damage - wall_damage)) + 0.5)
            if city.get("wall_max") != want_wall_max:
                mismatches.append(
                    f"{name or pos} wall_max Civ6={want_wall_max} "
                    f"CIVVIS={city.get('wall_max')!r}"
                )
            if city.get("wall_hp") != want_wall_hp:
                mismatches.append(
                    f"{name or pos} wall_hp Civ6={want_wall_hp} "
                    f"CIVVIS={city.get('wall_hp')!r}"
                )
        want_religion = civ6_id(source.get("religion"), "RELIGION_").replace("_", " ")
        got_religion = str(city.get("religion") or "").lower()
        if want_religion and want_religion != got_religion:
            mismatches.append(
                f"{name or pos} religion Civ6={want_religion!r} CIVVIS={got_religion!r}"
            )
        exported_buildings = {
            IDENTIFIER_ALIASES.get(name, name)
            for value in source.get("buildings") or []
            for name in [civ6_id(value, "BUILDING_")]
        }
        # CIVVIS models the Palace intrinsically on the current capital; keeping
        # it in the ordinary building collection would add its yields twice.
        want_buildings = exported_buildings - MIRRORED_WONDERS - {"palace"}
        got_buildings = {str(value).lower() for value in city.get("buildings") or []}
        if want_buildings != got_buildings:
            mismatches.append(
                f"{name or pos} buildings missing={sorted(want_buildings - got_buildings)} "
                f"extra={sorted(got_buildings - want_buildings)}"
            )
        if "wonders" in source:
            want_wonders = {
                IDENTIFIER_ALIASES.get(
                    civ6_id(wonder.get("type"), "BUILDING_"),
                    civ6_id(wonder.get("type"), "BUILDING_"),
                ): axial(wonder.get("x", 0), top - wonder.get("y", 0))
                for wonder in source.get("wonders") or []
            }
            got_wonders = {
                str(wonder).lower(): tuple(position)
                for wonder, position in (city.get("wonders") or {}).items()
            }
            if want_wonders != got_wonders:
                mismatches.append(
                    f"{name or pos} wonders Civ6={want_wonders!r} CIVVIS={got_wonders!r}"
                )
        want_districts = {
            IDENTIFIER_ALIASES.get(
                civ6_id(district.get("type"), "DISTRICT_"),
                civ6_id(district.get("type"), "DISTRICT_"),
            )
            for district in source.get("districts") or []
            # Firaxis exposes every wonder hex as the pseudo-type
            # DISTRICT_WONDER. CIVVIS stores the actual wonder and its position
            # in `wonders`, which is compared immediately above; counting the
            # pseudo-district too reports a duplicate representation as missing.
            if civ6_id(district.get("type"), "DISTRICT_")
            not in {"city_center", "wonder"}
            # An in-progress district is a foundation, not yet an entry in the
            # city's completed-district table. Its location and queue are
            # mirrored separately; comparing it here invents a missing district.
            and district.get("complete", True)
        }
        got_districts = {str(value).lower() for value in (city.get("districts") or {})}
        if want_districts != got_districts:
            mismatches.append(
                f"{name or pos} districts missing={sorted(want_districts - got_districts)} "
                f"extra={sorted(got_districts - want_districts)}"
            )
    return mismatches


def visible_exported_units(state, board, top):
    """Yield every currently visible unit with its compact CIVVIS owner seat."""
    visible = {tuple(pos) for pos in board.get("visible") or []}
    free_city_unit_ids = {
        unit.get("id")
        for minor in state.get("minors") or []
        if minor.get("civ") == "CIVILIZATION_FREE_CITIES"
        for unit in minor.get("units") or []
        if unit.get("id", 0) > 0
    }

    def is_visible(unit):
        return axial(unit.get("x", 0), top - unit.get("y", 0)) in visible

    yield from ((board.get("view_player", 0), unit)
                for unit in state.get("units") or [])
    for seat, rival in enumerate(state.get("rivals") or [], start=1):
        yield from ((seat, unit) for unit in rival.get("units") or []
                    if is_visible(unit))
    for minor in mirrored_minor_sources(state):
        yield from ((None, unit) for unit in minor.get("units") or []
                    if is_visible(unit))
    # `hostiles` is the planner's threat list and is deliberately not fog-gated
    # in the Firaxis export. The seated board must never reveal those private
    # contacts, so only compare hostiles standing on a tile the viewer can
    # currently see. The dedicated HOSTILES check below uses the same boundary.
    yield from (
        (None, unit)
        for unit in state.get("hostiles") or []
        if is_visible(unit) and unit.get("id") not in free_city_unit_ids
    )


def unmodelled_great_person(kind):
    """Great People are named individuals in CIVVIS rather than board units."""
    name = civ6_id(kind, "UNIT_")
    return name.startswith("great_") or name == "comandante_general"


def exported_unit_kind(unit):
    """Return the unit type across Firaxis's two export field names."""
    return unit.get("kind") or unit.get("type")


def modelled_qualified_unique(raw_kind):
    """Resolve a civilization-qualified unit the way the live mirror does.

    Civilization VI calls Phoenicia's unique naval unit
    ``UNIT_PHOENICIA_BIREME`` while CIVVIS stores the modelled unique as
    ``bireme``. The Rust mirror accepts that spelling only when the stripped
    name is an explicitly unique model; the checker must apply the same guard
    or it can either report a real board unit as missing or accept an ordinary
    unit under a misleading prefix. The suffix pass covers CIVVIS names with
    a Civilopedia epithet, such as ``maryannu_chariot_archer``.
    """
    if not raw_kind or raw_kind.startswith("great_"):
        return None
    _, separator, bare = raw_kind.partition("_")
    if not separator or not bare:
        return None
    direct = MIRRORED_UNIT_RULES.get(bare)
    if isinstance(direct, dict) and direct.get("unique_to"):
        return bare
    suffix = f"_{bare}"
    matches = [
        name for name, spec in MIRRORED_UNIT_RULES.items()
        if name.endswith(suffix)
        and isinstance(spec, dict)
        and spec.get("unique_to")
    ]
    return matches[0] if len(matches) == 1 else None


def unit_fact_mismatches(state, board, top):
    """Compare visible unit presence and facts across every exported actor."""
    by_pos = {}
    for unit in board.get("units") or []:
        by_pos.setdefault(tuple(unit.get("pos") or []), []).append(unit)
    source_groups = {}
    for owner, source in visible_exported_units(state, board, top):
        pos = axial(source.get("x", 0), top - source.get("y", 0))
        raw_kind = civ6_id(exported_unit_kind(source), "UNIT_")
        kind = IDENTIFIER_ALIASES.get(raw_kind, raw_kind)
        if kind.startswith("barbarian_") and kind not in UNIT_MODEL_FALLBACKS:
            kind = kind.removeprefix("barbarian_")
        # Apply aliases after the host-only exact variants have been preserved;
        # an older ordinary barbarian prefix still resolves to its stock role.
        kind = IDENTIFIER_ALIASES.get(kind, kind)
        kind = UNIT_MODEL_FALLBACKS.get(kind, kind)
        qualified = modelled_qualified_unique(raw_kind)
        if qualified and kind == raw_kind and not raw_kind.startswith("barbarian_"):
            kind = qualified
        # ⚠⚠ AND THE BASE THE EXPORT ITSELF HANDS US, because the mirror
        # DELIBERATELY approximates a unique it does not model and says so.
        #
        # `UNIT_MALI_MANDEKALU_CAVALRY` arrives with `base: UNIT_KNIGHT`, and
        # `rebuild_from_state` plants it as a knight, recording
        # `approximated_as_knight` in `dropped_units` — 39 times on the run this
        # was measured. The board is CORRECT. This check compared raw names, found
        # no `mali_mandekalu_cavalry` on the tile, and reported five phantom drops:
        #
        #     UNIT_MALI_MANDEKALU_CAVALRY@(58,20) count Civ6=1 CIVVIS=0   x5
        #
        # Every civilization with a unique unit would do this, on every run. A
        # deliberate, recorded translation is not a missing unit.
        base_kind = civ6_id(source.get("base") or "", "UNIT_")
        base_kind = IDENTIFIER_ALIASES.get(base_kind, base_kind)
        base_kind = UNIT_MODEL_FALLBACKS.get(base_kind, base_kind)
        source_groups.setdefault((owner, pos, kind, base_kind), []).append(source)

    mismatches = []
    for (owner, pos, kind, base_kind), sources in source_groups.items():
        # The exact type, or the base the export named for it. Nothing wider: a
        # wildcard here would stop this axis catching a real drop, which is the
        # only reason it exists.
        accepted = {kind} | ({base_kind} if base_kind else set())
        candidates = [unit for unit in by_pos.get(pos, [])
                      if str(unit.get("type") or "").lower() in accepted
                      and (
                          unit.get("owner") != board.get("view_player", 0)
                          if owner is None
                          else unit.get("owner") == owner
                      )]
        if len(candidates) != len(sources):
            if not all(unmodelled_great_person(exported_unit_kind(source)) for source in sources):
                mismatches.append(
                    f"{exported_unit_kind(sources[0]) or '?'}@{pos} count "
                    f"Civ6={len(sources)} CIVVIS={len(candidates)}"
                )
            continue

        # ⚠⚠ AN UNFORTIFIED UNIT HAS NO FORTIFY_TURNS TO DISAGREE ABOUT, and
        # comparing the two sides' "none" sentinels made this axis fail on EVERY
        # unit of EVERY run.
        #
        # Civilization VI exports -1 for a unit that is not fortified; CIVVIS
        # stores 0. Both mean the same thing, and the field only carries meaning
        # when `fortified` is true — which is already the neighbouring element of
        # this key. Measured 2026-08-02: `UNITDATA` listed every warrior, scout,
        # builder and trader on the board as `fortify_turns Civ6=[-1] CIVVIS=[0]`,
        # including units that cannot meaningfully fortify at all.
        #
        # A check that fires on every unit says nothing, and it buries the ones
        # that mean something — this file already carries seven entries about
        # exactly that. So the turn count is normalised to 0 whenever the unit is
        # not fortified, on both sides, leaving a real difference in a FORTIFIED
        # unit's count fully visible.
        def fortify_pair(fortified, turns):
            if not fortified:
                return False, 0
            value = int(turns) if isinstance(turns, (int, float)) and turns >= 0 else 0
            return True, max(0, min(2, value))

        # ⚠ AND THE SAME SHAPE ONE FIELD OVER. Civilization VI exports `hp: -1`
        # for a unit whose health it is not telling us — a fogged rival, mostly —
        # while CIVVIS plants it at full. Comparing "unknown" against "assumed
        # full" produced `hp Civ6=[-1] CIVVIS=[100]` on every such unit. An
        # unknown is not a disagreement; it is an absence of evidence, and a check
        # cannot fail on evidence it was never given.
        #
        # ⚠ Our OWN units always carry a real hp, so this loses nothing that
        # matters: it silences the rivals the export declines to describe and
        # leaves every genuine health difference visible.
        exported_hp = [
            source.get("hp") for source in sources
            if isinstance(source.get("hp"), (int, float)) and source.get("hp") > 0
        ]
        hp_known = len(exported_hp) == len(sources)

        def source_key(source):
            hp = source.get("hp")
            fortified, turns = fortify_pair(
                bool(source.get("fortified")), source.get("fortify_turns")
            )
            return (
                int(hp + 0.5) if hp_known and isinstance(hp, (int, float)) else -1,
                fortified,
                turns,
            )

        def board_key(unit):
            fortified, turns = fortify_pair(
                bool(unit.get("fortified")), unit.get("fortify_turns")
            )
            return (
                int(unit.get("hp"))
                if hp_known and isinstance(unit.get("hp"), (int, float)) else -1,
                fortified,
                turns,
            )

        wanted = sorted(source_key(source) for source in sources)
        actual = sorted(board_key(unit) for unit in candidates)
        if wanted != actual:
            for field, index in (("hp", 0), ("fortified", 1), ("fortify_turns", 2)):
                wanted_values = sorted(value[index] for value in wanted)
                actual_values = sorted(value[index] for value in actual)
                if wanted_values != actual_values:
                    mismatches.append(
                        f"{exported_unit_kind(sources[0]) or '?'}@{pos} {field} "
                        f"Civ6={wanted_values!r} CIVVIS={actual_values!r}"
                    )
    return mismatches


def production_item_name(value):
    """Normalize a live Civ VI production type to CIVVIS's queue vocabulary."""
    if not isinstance(value, str) or not value.strip():
        return None
    for prefix, kind in (
        ("UNIT_", "unit"),
        ("BUILDING_", "building"),
        ("DISTRICT_", "district"),
        ("WONDER_", "wonder"),
        ("PROJECT_", "project"),
        ("PRODUCT_", "product"),
    ):
        if value.upper().startswith(prefix):
            name = civ6_id(value, prefix)
            # Reuse the audited Firaxis-to-CIVVIS vocabulary for internal
            # implementation names such as BUILDING_GOV_CITYSTATES (the
            # player-facing Foreign Ministry), unique units, and era walls.
            name = IDENTIFIER_ALIASES.get(name, name)
            # Firaxis files every wonder under the BUILDING_ prefix
            # (BUILDING_TAJ_MAHAL); the board queues it as a `wonder`, the
            # kind `queue_item_name` reads back. Without this the PRODUCTION
            # line printed a false MISMATCH for every wonder in production
            # (run civvis-20260826T184456Z: Civ6=('building','taj_mahal')
            # against CIVVIS=('wonder','taj_mahal')), a vocabulary error and
            # not a board error.
            if kind == "building" and name in MIRRORED_WONDERS:
                kind = "wonder"
            # Firaxis truncates these district type identifiers; the mirror
            # restores the full CIVVIS names when it resolves the rules table.
            if kind == "district":
                name = {
                    "government": "government_plaza",
                    "theater": "theater_square",
                }.get(name, name)
            elif kind == "project":
                name = {
                    "enhance_district_campus": "campus_research_grants",
                    "enhance_district_holy_site": "holy_site_prayers",
                    "enhance_district_commercial_hub": "commercial_hub_investment",
                    "enhance_district_harbor": "harbor_shipping",
                    "enhance_district_encampment": "encampment_training",
                    "enhance_district_industrial_zone": "industrial_zone_logistics",
                    "enhance_district_theater": "theater_square_festival",
                }.get(name, name)
            return kind, name
    return None


def queue_item_name(item):
    """Return the meaningful queue kind/name while ignoring placement metadata."""
    if not isinstance(item, dict):
        return None
    for kind in ("unit", "building", "district", "wonder", "project", "product"):
        value = item.get(kind)
        if isinstance(value, str) and value:
            return kind, value.lower()
    return None


def exported_route_pairs(state, top):
    """Active Civ VI route endpoints in the board's axial coordinate frame."""
    pairs = Counter()
    for route in state.get("trade_routes") or []:
        values = (route.get("origin_x"), route.get("origin_y"),
                  route.get("destination_x"), route.get("destination_y"))
        if not all(isinstance(value, int) and value >= 0 for value in values):
            continue
        origin_x, origin_y, destination_x, destination_y = values
        pairs[(axial(origin_x, top - origin_y),
               axial(destination_x, top - destination_y))] += 1
    return pairs


def board_route_pairs(board):
    """CIVVIS active route endpoints, resolved from route city ids to positions."""
    positions = {city.get("id"): tuple(city.get("pos") or [])
                 for city in board.get("cities") or []}
    pairs = Counter()
    for route in (board.get("me") or {}).get("routes") or []:
        origin = positions.get(route.get("origin"))
        destination = positions.get(route.get("dest"))
        if len(origin or ()) == 2 and len(destination or ()) == 2:
            pairs[(origin, destination)] += 1
    return pairs


def resource_visible_in_state(resource, state):
    if not resource or state is None:
        return True
    spec = RESOURCE_RULES.get(resource) or {}
    techs = {civ6_id(value, "TECH_") for value in state.get("techs") or []}
    civics = {civ6_id(value, "CIVIC_") for value in state.get("civics") or []}
    return (not spec.get("tech") or spec["tech"] in techs) and (
        not spec.get("civic") or spec["civic"] in civics
    )


def expected_tile_fields(plot, state=None):
    """Translate one exported plot through the same committed vocabulary as Rust."""
    terrain = VOCABULARY["terrains"].get(plot.get("t"))
    feature_name = plot.get("f")
    resource_name = plot.get("r")
    resource = VOCABULARY["resources"].get(resource_name) if resource_name else None
    if not resource_visible_in_state(resource, state):
        resource = None
    improvement_name = plot.get("im")
    improvement = None
    if isinstance(improvement_name, str):
        improvement = civ6_id(improvement_name, "IMPROVEMENT_")
        improvement = IDENTIFIER_ALIASES.get(improvement, improvement)
        if improvement not in MIRRORED_IMPROVEMENTS:
            improvement = f"<unmapped:{improvement_name}>"
    return {
        "terrain": terrain.get("terrain") if terrain else f"<unmapped:{plot.get('t')}>",
        "hills": bool(terrain.get("hills")) if terrain else None,
        "feature": (
            VOCABULARY["features"].get(feature_name)
            if feature_name else None
        ),
        "resource": resource,
        "improvement": improvement,
        "river": bool(plot.get("ri")),
        "coastal_lowland": max(0, min(3, int(plot.get("cl") or 0))),
    }


def exact_tile_mismatches(pairs, state=None, limit=12):
    """Count field-level disagreements and retain bounded coordinate examples."""
    counts, examples = Counter(), []
    for board_tile, plot in pairs:
        expected = expected_tile_fields(plot, state)
        for field, wanted in expected.items():
            actual = board_tile.get(field)
            if actual == wanted:
                continue
            counts[field] += 1
            if len(examples) < limit:
                examples.append(
                    f"{field}@{plot.get('x')},{plot.get('y')} "
                    f"Civ6={wanted!r} CIVVIS={actual!r}"
                )
    return counts, examples


def leaked_hidden_resources(pairs, state):
    leaks = []
    for _, plot in pairs:
        raw = plot.get("r")
        resource = VOCABULARY["resources"].get(raw) if raw else None
        if resource and not resource_visible_in_state(resource, state):
            leaks.append(f"{resource}@{plot.get('x')},{plot.get('y')}")
    return leaks


def latest_seat(run):
    """The latest startup seat event, which carries setup outside state patches."""
    latest = None
    with open(os.path.join(run, "events.jsonl")) as handle:
        for line in handle:
            if '"seat"' not in line:
                continue
            try:
                event = json.loads(line)
            except ValueError:
                continue
            if (event.get("kind") or event.get("event")) == "seat":
                latest = event
    return latest


def load_export(run, upto=None):
    """Latest value per plot, exactly like Snapshot::from_chunks (later wins).

    ⚠ `upto` bounds the export to what had been sent BY THAT TURN. The mirror
    republishes on a cadence, so comparing a board from turn N against an export
    that has since reached turn N+7 reports the growth as loss -- this checker
    cried wolf exactly that way ("13 exported plots are NOT on the board", all of
    them present one publish later). The board's own turn is the cutoff.
    """
    plots, turn = {}, 0
    with open(os.path.join(run, "events.jsonl")) as handle:
        for line in handle:
            try:
                event = json.loads(line)
            except ValueError:
                continue
            kind = event.get("kind") or event.get("event")
            if kind == "turn":
                turn = max(turn, int(event.get("turn") or 0))
            if kind != "tiles":
                continue
            if upto is not None and (event.get("turn") or 0) > upto:
                continue
            for plot in event.get("plots") or []:
                plots[(plot["x"], plot["y"])] = plot
    return plots, turn


def latest_terminal_turn(run):
    """Return the last victory/own-defeat frame in a completed event stream."""
    turn = None
    with open(os.path.join(run, "events.jsonl")) as handle:
        for line in handle:
            try:
                event = json.loads(line)
            except ValueError:
                continue
            kind = event.get("kind") or event.get("event")
            if kind == "victory" or (kind == "defeat" and event.get("ours")):
                turn = int(event.get("turn") or 0)
    return turn


def exact_host_frame(board_turn, state_turn, completed_turn, *, archive=False,
                     terminal_turn=None):
    """Whether the published board has an authoritative same-turn host state.

    A live turn's event order is `state` -> CIVVIS orders -> `turn`. The state is
    the exact reconstruction source, so requiring the later completion marker
    rejects a healthy frame while decisions are in flight. Completed archives
    retain the stronger boundary requirement: a playable turn or the explicit
    terminal victory/defeat event must close the state.
    """
    if state_turn != board_turn:
        return False
    if not archive:
        return True
    return completed_turn >= board_turn or terminal_turn == board_turn


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("run", nargs="?", help="run directory (newest by default)")
    parser.add_argument(
        "--archive", action="store_true",
        help="compare a completed archive without requiring a live controller",
    )
    args = parser.parse_args(argv)
    run = args.run or newest_run()
    problems: list[str] = []
    if not args.archive:
        runtime = live_runtime_problems(run)
        if runtime:
            problems.append("control")
            print("CONTROL  ⚠ " + "; ".join(runtime))
        else:
            print("CONTROL  live game, fresh export, worker present, rig binary "
                  "not behind the decider   OK")
    # The viewer can change boards while this process reads the growing export.
    # Re-sample until the published turn has an exact state. Do not require the
    # following playable `turn` event in live mode: the normal host order is
    # state -> CIVVIS decision/orders -> turn, and the state is already the exact
    # source used to reconstruct the board during that handoff.
    board = state = None
    state_frame_count = 0
    in_flight_same_turn = False
    game_turn = -1
    terminal_turn = latest_terminal_turn(run) if args.archive else None
    mirror_error = None
    for _ in range(20):
        try:
            with urllib.request.urlopen(
                f"http://127.0.0.1:{PORT}/state", timeout=30
            ) as response:
                board = json.load(response)
        except (OSError, ValueError) as exc:
            # `live_runtime_problems` can already have proved that the
            # controller/sidecar is absent. Keep that useful diagnosis intact:
            # a checker must report an unavailable mirror, not replace it with
            # a Python traceback from the same connection failure.
            board = None
            mirror_error = f"{type(exc).__name__}: {exc}"
            break
        _, game_turn = load_export(run)
        state, state_frame_count = latest_state_and_frame_count(
            run, upto=board["turn"]
        )
        state_turn = int((state or {}).get("turn") or -1)
        in_flight_same_turn = live_same_turn_frame_handoff(
            board["turn"], state_turn, game_turn, state_frame_count,
            archive=args.archive,
        )
        if in_flight_same_turn:
            time.sleep(0.1)
            continue
        if exact_host_frame(
            board["turn"], state_turn, game_turn, archive=args.archive,
            terminal_turn=terminal_turn,
        ):
            break
        time.sleep(0.1)
    if board is None:
        print(f"run   {os.path.basename(run)}")
        detail = mirror_error or "no response"
        print(f"MIRROR  ⚠ unavailable on :{PORT}/state ({detail})")
        return 1
    state_turn = int((state or {}).get("turn") or -1)
    if in_flight_same_turn:
        print(f"run   {os.path.basename(run)}")
        print(
            f"turn  game {game_turn}   board {board['turn']}   state {state_turn} "
            f"⏳ IN FLIGHT ({state_frame_count} same-turn state frames)"
        )
        print("\nMIRROR  waiting for the host turn-completion boundary; re-check shortly")
        return 0
    if not exact_host_frame(
        board["turn"], state_turn, game_turn, archive=args.archive,
        terminal_turn=terminal_turn,
    ):
        print(f"run   {os.path.basename(run)}")
        print(f"turn  game {game_turn}   board {board['turn']}   state {state_turn}   ⚠ DRIFT")
        print("\nDISAGREEMENTS: no exact host frame exists for the published board")
        return 1
    plots, _ = load_export(run, upto=board["turn"])
    tiles = {tuple(t["pos"]): t for t in board["map"]["tiles"]}
    visible = {tuple(v) for v in board["visible"]}

    print(f"run   {os.path.basename(run)}")
    phase = "   (decisions in flight)" if game_turn < board["turn"] else ""
    print(f"turn  game {state_turn}   board {board['turn']}   state {state_turn}   OK"
          f"{phase}")

    # --- lobby setup -------------------------------------------------------
    # The seat event is emitted once rather than copied into each state patch.
    # Compare its actual Civ VI identifiers to the reconstructed board instead
    # of assuming the command-line defaults used to launch the viewer survived.
    seat = latest_seat(run)
    if seat is None:
        print("SETUP    no seat event yet")
    else:
        player = next((p for p in board.get("players", [])
                       if p.get("id") == board.get("view_player", 0)), {})
        expected = {
            "speed": civ6_id(seat.get("speed"), "GAMESPEED_"),
            "difficulty": civ6_id(seat.get("difficulty"), "DIFFICULTY_"),
            "map": civ6_map_script(seat.get("map")),
            "size": civ6_id(seat.get("size"), "MAPSIZE_"),
            "civ": civ6_id(seat.get("civ"), "CIVILIZATION_"),
            "leader": civ6_id(seat.get("leader"), "LEADER_"),
        }
        actual = {
            "speed": str(board.get("game_speed") or "").lower(),
            "difficulty": str(board.get("difficulty") or "").lower(),
            "map": str(board.get("map", {}).get("script") or "").lower(),
            "size": str(board.get("map", {}).get("size") or "").lower(),
            "civ": str(player.get("civ") or "").replace(" ", "_").lower(),
            "leader": str(player.get("leader") or "").replace(" ", "_").lower(),
        }
        mismatches = [
            key for key, want in expected.items()
            if want and not (
                civ_id_matches(want, actual.get(key)) if key == "civ"
                else leader_id_matches(want, actual.get(key)) if key == "leader"
                else actual.get(key) == want
            )
        ]
        print("SETUP    "
              f"speed {actual['speed'] or '?'}; "
              f"difficulty {actual['difficulty'] or '?'}; "
              f"map {actual['map'] or '?'}/{actual['size'] or '?'}; "
              f"{player.get('civ') or '?'} / {player.get('leader') or '?'}")
        if mismatches:
            problems.append("setup")
            detail = ", ".join(f"{key} Civ6={expected[key] or '?'} "
                               f"CIVVIS={actual[key] or '?'}" for key in mismatches)
            print(f"         ⚠ {detail}")
        else:
            print("         OK")

    # --- the flip constant, discovered not assumed -------------------------
    best, best_hits = None, -1
    for top in range(40, 50):
        hits = len({axial(x, top - y) for (x, y) in plots} & set(tiles))
        if hits > best_hits:
            best, best_hits = top, hits
    if best_hits < 0.9 * len(tiles):
        print(f"⚠ coordinate frames do not line up (best {best_hits}/{len(tiles)} "
              f"at TOP={best}); every comparison below would be meaningless")
        return 1
    print(f"frame TOP={best}  overlap {best_hits}/{len(tiles)}")
    pairs = [(tiles[axial(x, best - y)], p)
             for (x, y), p in plots.items() if axial(x, best - y) in tiles]

    print()
    tile_counts, tile_examples = exact_tile_mismatches(pairs, state)
    mismatched_tiles = sum(tile_counts.values())
    print(f"TILES    {len(pairs)} paired; {mismatched_tiles} field disagreement(s)"
          + (f"   ⚠ {dict(tile_counts)}" if mismatched_tiles else "   OK"))
    if tile_examples:
        problems.append("tiles")
        print("         " + "; ".join(tile_examples))
    leaks = leaked_hidden_resources(pairs, state)
    if leaks:
        print(f"KNOWLEDGE {len(leaks)} raw resource leak(s) hidden by CIVVIS: "
              + "; ".join(leaks[:8]))
        if not args.archive:
            problems.append("knowledge")

    # --- fog memory (#713) -------------------------------------------------
    # ⚠ The invariant is "the board carries every plot the mod exported", NOT
    # "some ground is fogged". Early on, a seat with two units and no cities can
    # SEE everything it has revealed, and a fogged count of zero is correct there.
    # Checking for fog directly cried wolf on turn 1 of a healthy run.
    fogged = len(tiles) - len(visible)
    missing = len(plots) - best_hits
    print(f"FOG      board {len(tiles)} tiles, {len(visible)} visible, "
          f"{fogged} remembered-but-fogged; export has {len(plots)}")
    if missing > 0:
        problems.append("fog")
        print(f"         ⚠ {missing} exported plots are NOT on the board — "
              f"remembered ground is being dropped")
    elif fogged == 0 and len(tiles) > 60:
        print("         ⚠ nothing is fogged on a board this large; suspect a collapse "
              "onto current visibility")
    else:
        print("         OK")

    # --- rivers (#714): a Civ 6 river plot is fresh water BY DEFINITION ----
    riv = [(b, p) for b, p in pairs if b.get("river")]
    fresh = [1 for _, p in pairs if p.get("fw")]
    base = len(fresh) / max(1, len(pairs))
    hit = sum(1 for _, p in riv if p.get("fw"))
    rate = hit / max(1, len(riv))
    exported_rv = sum(1 for _, p in pairs if p.get("rv"))
    print(f"RIVERS   {len(riv)} river tiles; {hit} of them fresh in the export "
          f"({rate:.1%}) vs {base:.1%} base rate")
    # ⚠ Judge by LIFT over the base rate, not by an absolute share. A fixed 0.8 bar
    # called 73.8%-against-a-17.5%-base "chance" — a 4.2x lift. The share falls as
    # the map opens up and ocean dilutes the denominator, so only the ratio is
    # comparable across turns. It cannot reach 100%: `set_river_edge` marks the tile
    # across the segment too, and where that neighbour is unrevealed the export has
    # no `fw` to agree with.
    lift = rate / base if base > 0 else 0.0
    if not exported_rv:
        print("         ⚠ export carries no `rv` at all — old mod, rivers cannot be mirrored")
    elif not riv:
        print("         (no river tiles on the board yet)")
    elif lift < 1.5:
        problems.append("rivers")
        print(f"         ⚠ lift {lift:.1f}x over base — no better than chance, "
              f"these are the GENERATED map's rivers")
    else:
        print(f"         OK  ({lift:.1f}x base rate)")

    # --- landmass (#716) ---------------------------------------------------
    land = [(b, p) for b, p in pairs if not p.get("w")]
    named = sum(1 for _, p in land if p.get("ct"))
    with_cont = sum(1 for b, _ in land if b.get("continent") is not None)
    water_cont = sum(1 for b, p in pairs if p.get("w") and b.get("continent") is not None)
    cliffs = sum(1 for t in tiles.values() if any(t.get("cliff_edges") or []))
    print(f"LAND     {len(land)} land plots; export names a continent on {named}; "
          f"board assigns one to {with_cont}")
    if not named:
        print("         ⚠ export carries no `ct` — old mod")
    elif with_cont < named:
        problems.append("land")
        print(f"         ⚠ {named - with_cont} land plots lost their continent")
    else:
        print("         OK")
    # ⚠ NOT "water must have no continent". Civilization VI really does put COAST
    # tiles on a continent — 17 of them read CONTINENT_AUSTRALIA on this very run —
    # and carrying that is correct, because "another continent" is a rule and
    # dropping it would lose information. CIVVIS's own field doc says water has none;
    # that is CIVVIS's convention, not Civilization VI's, and the mirror follows the
    # game. So the check is agreement with the export.
    water_named = sum(1 for _, p in pairs if p.get("w") and p.get("ct"))
    if water_cont != water_named:
        print(f"         ⚠ board gives {water_cont} water tiles a continent, "
              f"the export names {water_named}")
    print(f"CLIFFS   {cliffs} " + ("⚠ invented — Civ 6 exposes no cliff accessor"
                                   if cliffs else "OK (none, as intended)"))
    if cliffs:
        problems.append("cliffs")

    # --- cities and units: entity-level, not tile-level ---------------------
    # ⚠ Counts alone are the weak check this project keeps getting burned by --
    # 21 units exported and 15 reconstructed once read as healthy because nothing
    # compared them. So compare the SETS, and name what is missing.
    # Keep entities on the same temporal boundary as terrain. A game can export
    # the next state's units between the `/state` fetch and this read; comparing
    # them to an older board reports ordinary movement as a dropped mirror unit.
    if state is None:
        print("ENTITIES (no state event yet)")
        return 0

    rival_mismatches = rival_identity_mismatches(state, board)
    if rival_mismatches:
        problems.append("rivals")
        print(f"RIVALS   {len(state.get('rivals') or [])} met   ⚠ "
              + "; ".join(rival_mismatches))
    else:
        print(f"RIVALS   {len(state.get('rivals') or [])} met identities   OK")

    minor_sources = mirrored_minor_sources(state)
    minor_mismatches = minor_fact_mismatches(state, board, best)
    if minor_mismatches:
        problems.append("city-states")
        print(f"MINORS   {len(minor_sources)} present   ⚠ "
              + "; ".join(minor_mismatches))
    elif "minors" not in state:
        print("MINORS   export has no city-state records (old control mod)")
    else:
        print(f"MINORS   {len(minor_sources)} present minor actor(s)   OK")

    public_mismatches = public_fact_mismatches(state, board)
    if public_mismatches:
        problems.append("public facts")
        print("PUBLIC   ⚠ " + "; ".join(public_mismatches))
    else:
        print("PUBLIC   HUD identity, totals, military, economy, research and tourism   OK")

    civ6_cities = {(c["x"], c["y"]) for c in state.get("cities") or []}
    board_cities = {tuple(c["pos"]) for c in board.get("cities", [])
                    if c.get("owner") == board.get("view_player", 0)}
    mapped = {axial(x, best - y) for (x, y) in civ6_cities}
    missing_cities = mapped - board_cities
    if missing_cities:
        problems.append("cities")
    print(f"CITIES   export {len(civ6_cities)}  board {len(board_cities)}"
          + (f"   ⚠ MISSING {sorted(missing_cities)}" if missing_cities else "   OK"))
    city_mismatches = city_fact_mismatches(state, board, best)
    if city_mismatches:
        problems.append("city facts")
        print("CITYDATA ⚠ " + "; ".join(city_mismatches))
    else:
        print("CITYDATA population, food, housing, amenities, yields, health, loyalty, "
              "defense, religion and development   OK")

    # --- production: an in-progress city must not read as idle -------------
    # A completed item used to stay in the mirror queue, then a new real item
    # appeared as the old one. Compare production on the same state boundary as
    # cities and units so normal turn advancement cannot look like a phantom.
    board_city_by_pos = {
        tuple(city["pos"]): city for city in board.get("cities", [])
        if city.get("owner") == board.get("view_player", 0)
    }
    production_mismatches, unmapped_production, checked_production = [], [], 0
    for city in state.get("cities") or []:
        pos = axial(city.get("x", 0), best - city.get("y", 0))
        board_city = board_city_by_pos.get(pos)
        if board_city is None:
            # The city assertion above names this loss more clearly.
            continue
        raw = city.get("producing")
        expected = production_item_name(raw)
        if raw is not None and expected is None:
            unmapped_production.append(f"{city.get('name', '?')}={raw!r}")
            continue
        checked_production += 1
        queue = list(board_city.get("queue") or [])
        actual = queue_item_name(queue[0]) if queue else None
        valid = actual == expected and (not queue if expected is None else len(queue) == 1)
        if not valid:
            production_mismatches.append(
                f"{city.get('name', '?')} Civ6={expected or 'idle'} "
                f"CIVVIS={actual or 'idle'}"
            )
    if production_mismatches or unmapped_production:
        problems.append("production")
    detail = []
    if production_mismatches:
        detail.append("MISMATCH " + "; ".join(production_mismatches))
    if unmapped_production:
        detail.append("UNMAPPED " + "; ".join(unmapped_production))
    print(f"PRODUCTION export {checked_production} city queues"
          + (f"   ⚠ {'; '.join(detail)}" if detail else "   OK"))

    # --- active trade routes ------------------------------------------------
    # A Trader remains a physical unit while travelling in Civilization VI, so
    # comparing units cannot tell us whether it is available for a new route.
    # Compare the route graph itself: these routes occupy capacity and contribute
    # yields, and a missing one made CIVVIS repeatedly order the same Trader.
    if "trade_routes" not in state:
        print("TRADE    export has no route records (old control mod)")
    else:
        exported_routes = exported_route_pairs(state, best)
        mirrored_routes = board_route_pairs(board)
        if exported_routes != mirrored_routes:
            problems.append("trade")
            print(f"TRADE    Civ6 {sum(exported_routes.values())}  "
                  f"CIVVIS {sum(mirrored_routes.values())}   ⚠ "
                  f"MISSING {list((exported_routes - mirrored_routes).elements())}; "
                  f"EXTRA {list((mirrored_routes - exported_routes).elements())}")
        else:
            print(f"TRADE    {sum(exported_routes.values())} active route(s)   OK")

    # ⚠ Name what is missing, do not just count it. A bare "1 dropped" sends the
    # reader to the wrong place; the position and type say immediately whether it is
    # a known modelling gap (Great People are not units in CIVVIS) or something new.
    civ6_units = list(state.get("units") or [])
    ours = [u for u in board.get("units", []) if u.get("owner") == board.get("view_player", 0)]
    on_board = {tuple(u["pos"]) for u in ours if u.get("pos")}
    missing_units = [
        f'{exported_unit_kind(u) or "?"}@{u.get("x")},{u.get("y")}'
        for u in civ6_units
        if axial(u.get("x", 0), best - u.get("y", 0)) not in on_board
    ]
    unit_mismatches = unit_fact_mismatches(state, board, best)
    if unit_mismatches:
        problems.append("unit facts")
        print("UNITDATA ⚠ " + "; ".join(unit_mismatches))
    else:
        print("UNITDATA type, position, health and fortification   OK")
    # ⚠ COUNT AND POSITION, because neither alone is enough. Position-matching
    # cannot see a STACKED drop -- Civilization VI puts a civilian and a military
    # unit on one tile and CIVVIS does not, so two exported units collapse onto one
    # board tile and every position still looks covered. The count catches that.
    # Position-matching, in turn, names WHICH unit is gone when the count is equal
    # but the board holds a different one.
    # ⚠ CATEGORISE, DO NOT SUPPRESS. CIVVIS does not model Great People as units at
    # all, so they are absent from the board on EVERY run. Failing the gate on a
    # documented modelling gap means the gate always fails, which is the same as
    # having no gate -- and it buries the drop that is actually new.
    #
    # They are still counted and still printed. What changes is that a known gap does
    # not set the exit status, so a NEW disappearance stands out against it.
    # ⚠ Counted from the EXPORT, not from `missing_units`. Great People stack with
    # other units, so position-matching covers them and they never appear in the
    # missing list -- the count path is where they land, and that is where they have
    # to be discounted. Getting this wrong left the gate failing on them anyway.
    great_people = [u for u in civ6_units if "GREAT_" in (u.get("kind") or "")]
    unexplained = [u for u in missing_units if "GREAT_" not in u]
    short = len(civ6_units) - len(ours) - len(great_people)
    if short > 0 or unexplained:
        problems.append("units")
    detail = ""
    if unexplained:
        detail = f"   ⚠ NOT on the board: {unexplained}"
    elif great_people and short <= 0:
        detail = (f"   OK — {len(great_people)} Great People absent, which CIVVIS does "
                  f"not model as units")
    elif short > 0:
        # ⚠ Do NOT name the cause. From the board alone a collapsed stack and an
        # unmodelled type (Great People are not units in CIVVIS) look identical, and
        # this line used to assert "a STACK was collapsed" when the decider's own
        # `dropped_units` was saying `great_person`. Report the fact, point at the
        # field that knows why.
        detail = (f"   ⚠ {short} fewer on the board beyond the {len(great_people)} "
                  f"Great People, every position covered — a stack collapsed or a type "
                  f"CIVVIS does not model; the decider's `dropped_units` names which")
    print(f"UNITS    export {len(civ6_units)}  board {len(ours)}"
          + (detail or "   OK"))

    # --- HOSTILES ----------------------------------------------------------
    #
    # ★★★★★ THE ONE THING ON THE BOARD THE SEAT MOST NEEDS TO SEE, AND NOTHING
    # CHECKED IT. Every other line here verifies what the empire OWNS. The threat
    # list is what it must plan AROUND, and until now no instrument compared it.
    #
    # Measured 2026-08-02 on run civvis-20260802T041527Z: 14 settlers were built
    # and every one vanished at hp 100 having moved 0-4 tiles from the capital,
    # while the city count sat at 1 from turn 41 to 241. Civilization VI CAPTURES
    # civilians rather than killing them, so full health at disappearance is the
    # signature of capture — and on each settler's last turn a hostile stood 1-3
    # tiles away, 8 of 13 of them ADJACENT. One of those "hostiles" was itself a
    # UNIT_SETTLER: ours, already taken.
    #
    # The first question that asks is whether the seat could SEE them, and it took
    # a hand-written pass over events.jsonl to answer (it could — 11 to 15 hostiles
    # exported on every capture turn). This line makes that answer automatic.
    #
    # ⚠ Hostiles are planted under the BARBARIAN SEAT, not the viewer, so they are
    # board units whose owner is not `view_player` — see `rebuild_from_state`'s
    # `barbarian_seat` branch, which records `no_barbarian_seat` for every hostile
    # when that seat is missing. A roster with no barbarian seat cannot hold the
    # threat list at all, and that reads here as every hostile missing.
    civ6_hostiles = list(state.get("hostiles") or [])
    seat = board.get("view_player", 0)
    theirs = [u for u in board.get("units", []) if u.get("owner") != seat]
    their_pos = {tuple(u["pos"]) for u in theirs if u.get("pos")}
    # ⚠⚠ FOG-GATE IT, OR IT CRIES WOLF ON EVERY RUN.
    #
    # `hostiles` is documented in mirror.rs as "a threat list the planner needs,
    # NOT knowledge the seat has" -- it is not fog-gated. The board on :8610 is the
    # SEATED view and shows only what the seat can currently see. Comparing the two
    # directly asks the board to contain units the seat cannot see, which it must
    # not.
    #
    # The first version of this check did exactly that and reported
    # `8 exported, 5 NOT on the board` on a healthy run, while the decider's own
    # `dropped_units` recorded no hostile dropped at all -- only three Great
    # Writers. Same shape as the TREASURY wolf: two numbers that look comparable
    # and are measured over different populations.
    #
    # What IS assertable: a hostile standing on a tile the seat can SEE must be on
    # the board. Anything beyond the fog is the planner's private threat list and
    # is none of this check's business.
    seen_hostiles = [
        h for h in civ6_hostiles
        if axial(h.get("x", 0), best - h.get("y", 0)) in visible
    ]
    missing_hostiles = [
        f'{h.get("type") or h.get("kind") or "?"}@{h.get("x")},{h.get("y")}'
        for h in seen_hostiles
        if axial(h.get("x", 0), best - h.get("y", 0)) not in their_pos
    ]
    # ⚠ `type`, not `kind`. Our own units are exported as `kind`; the hostiles list
    # uses `type`, and reading the wrong one printed every name as "?".
    if missing_hostiles:
        problems.append("hostiles")
    print(f"HOSTILES export {len(civ6_hostiles)}, {len(seen_hostiles)} in sight"
          + (f"   ⚠ {len(missing_hostiles)} visible but NOT on the board: "
             f"{missing_hostiles[:6]}"
             if missing_hostiles
             else ("   all visible ones on the board   OK" if seen_hostiles
                   else "   none in sight   OK")))

    # --- TREASURY ----------------------------------------------------------
    #
    # ⚠⚠ THESE TWO ARE STOCKS, NOT RATES, AND THAT IS WHY THEY GET THEIR OWN
    # CHECK RATHER THAN JOINING `economy_drift`.
    #
    # The mod exports `gold` from `GetTreasury():GetGoldBalance()` and `faith`
    # from `GetReligion():GetFaithBalance()` — balances. `economy_drift` compares
    # science and culture, which the mod takes from `GetTechs():GetScienceYield()`
    # — a per-turn rate. Putting a balance beside a rate under one heading is the
    # apples-to-oranges the rest of this file exists to prevent.
    #
    # ⚠ AND IT MUST BE READ AT THE SAME TURN, which is the whole reason this was
    # worth adding rather than eyeballing. Measured 2026-08-02 on run
    # civvis-20260802T030910Z: read against the NEWEST export the treasury showed
    # `gold 176 vs 167` and `faith 23 vs 21` — a confident-looking 5% shortfall
    # that is nothing but one turn of income at +9 gold and +2 faith. Bounded to
    # the board's own turn the same instant read **134 vs 134 and 27 vs 27,
    # delta 0.0 on both**. An unbounded version of this check would have reported
    # a treasury defect on a perfectly faithful mirror, every single time.
    board_me = next((p for p in board.get("players") or [] if p.get("id") == 0), None)
    if board_me is None:
        print("TREASURY no seated player 0 on the board; cannot compare")
    else:
        rows = []
        for field in ("gold", "faith"):
            theirs, ours_value = state.get(field), board_me.get(field)
            # -1 is the mod's own "could not read it" sentinel, and a missing key
            # is an older export. Neither is a disagreement — say so rather than
            # inventing a delta, the same way `economy_drift` refuses to claim
            # anything from an export carrying no yields.
            if theirs is None or ours_value is None or theirs < 0:
                rows.append(f"{field} unknown")
                continue
            delta = ours_value - theirs
            rows.append(f"{field} {theirs:g}/{ours_value:g}"
                        + ("" if abs(delta) < 0.5 else f" ⚠{delta:+g}"))
            if abs(delta) >= 0.5:
                problems.append(f"treasury:{field}")
        print(f"TREASURY {'  '.join(rows)}"
              + ("   OK" if not any("⚠" in r for r in rows) else ""))

    # ⚠ Non-zero on a real disagreement, so this can gate a run rather than only
    # inform one. A frame mismatch already returned 1 above for the same reason:
    # a comparison whose coordinates do not line up is worse than no comparison.
    if problems:
        print()
        print(f"DISAGREEMENTS: {', '.join(problems)}")
        return 1
    return 0


def latest_state(run, upto=None):
    """The `state` event as of `upto`, or the most recent one.

    ⚠ BOUND IT, for exactly the reason `load_export` is bounded. The mirror
    republishes on a cadence; the export keeps going. Comparing the LATEST unit
    positions against a board published several turns earlier reports units that
    have simply MOVED as units that were dropped -- this checker did that and
    named four healthy units as missing, while a same-turn read showed 25 against
    25 with the rosters matching type for type.

    That is the third time publish lag has fooled this file. If a future check
    reads the run directory, bound it too.
    """
    latest = None
    with open(os.path.join(run, "events.jsonl")) as handle:
        for line in handle:
            if '"state"' not in line:
                continue
            try:
                event = json.loads(line)
            except ValueError:
                continue
            if (event.get("kind") or event.get("event")) != "state":
                continue
            if upto is not None and int(event.get("turn") or 0) > upto:
                continue
            latest = event
    return latest


def latest_state_and_frame_count(run, upto=None):
    """Return the latest bounded state and its same-turn frame count.

    A live board may be held at the first state frame while the host appends
    additional replanning frames for that same turn. The latest state remains
    the right choice once the turn is complete, but it is not a safe comparison
    target while completion is still in flight. Count those frames in the same
    pass that selects the bounded state so the checker does not add another
    full read of a growing multi-megabyte event stream.
    """
    latest = None
    same_turn_frames = 0
    with open(os.path.join(run, "events.jsonl")) as handle:
        for line in handle:
            if '"state"' not in line:
                continue
            try:
                event = json.loads(line)
            except ValueError:
                continue
            if (event.get("kind") or event.get("event")) != "state":
                continue
            event_turn = int(event.get("turn") or 0)
            if upto is not None and event_turn > upto:
                continue
            latest = event
            if upto is not None and event_turn == upto:
                same_turn_frames += 1
    return latest, same_turn_frames


def live_same_turn_frame_handoff(board_turn, state_turn, completed_turn,
                                 state_frame_count, *, archive=False):
    """Whether a live board is racing newer state frames for its same turn.

    `follow.py` republishes on its own cadence. During a host replan it can
    therefore serve state frame 0 while the event stream already contains
    frames 1 and 2. There is no source-frame id in the served board, so the
    only safe signal available to this checker is multiple same-turn states
    before the host's playable `turn` marker. Archives keep their stricter
    boundary behavior and never use this live deferral.
    """
    return (
        not archive
        and state_turn == board_turn
        and completed_turn < board_turn
        and state_frame_count > 1
    )


if __name__ == "__main__":
    sys.exit(main())
