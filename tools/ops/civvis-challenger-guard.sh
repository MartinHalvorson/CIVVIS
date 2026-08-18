#!/bin/zsh
# Keep provisional challengers in contention for a seat.
#
# Seating is contested PER CIV, not on the global ladder: `display_elo_for`
# returns a strategy's leader/civ table once that combination has games, and
# only falls back to the global rating when it has none. `seat_by_civ_seeded`
# then takes the top THREE of the still-unused actives for that civ. So the bar
# to be seated is the third-best PER-CIV rating for whichever civ the seat
# draws -- measured here at a median of ~1753 across 99 civs, while the roster's
# global median is ~1703.
#
# A veteran with hundreds of games has a converged table on every civ and
# competes on it. A fresh entrant has none, so it competes with its global
# rating against everyone else's civ-specialist number and loses nearly every
# civ. Seeded at the global median it sat below the bar on 99 of 99 civs, and
# `religious-elite` went 0 games across a whole evening while ranked 6th.
#
# The rating here is a SEEDING device only -- the verdict comes from
# matches.csv (wins and placements), which this never touches. Restoring a
# still-provisional entrant to a competitive seed costs no evidence. Once it
# has PROVISIONAL_GAMES behind it, hands off: it has its own per-civ tables by
# then and stands on them.
# Called from the keeper's 15s loop as well as ad hoc, so throttle here rather
# than at every call site: the per-civ bar moves on the timescale of games, not
# seconds. Touch-then-check, so a crash cannot wedge it off.
STAMP=$HOME/.civvis-guard-last
if [[ -f $STAMP ]]; then
  age=$(( $(date +%s) - $(cat $STAMP 2>/dev/null || echo 0) ))
  (( age < 240 )) && exit 0
fi
date +%s > $STAMP

exec /usr/bin/python3 - <<'PY'
import json, os, statistics, tempfile

LEAGUE = "$HOME/civvis-spectator-src/league/league.json"
# 40, not 12. Releasing at 12 was too early to be a test: a challenger that
# starts badly drops below the seat bar and is frozen out at once, so its
# sample stops at whatever it happened to have. Observed -- religious-elite
# went 0-for-13 and was never seated again, winbred-1 drew 1 seat in 12 rounds.
# At ~6.5 seats parity is ~15%, so 40 games expects ~6 wins: 0 of 40 is
# p=0.0025 and means something, while 0 of 13 is p=0.12 and means nothing.
# The cost is carrying a weak entrant in the show for ~4 hours at the current
# rate, which is the price of actually testing it.
PROVISIONAL_GAMES = 40
CHALLENGERS = {"winbred-1", "deck-legacy", "religious-elite"}

league = json.load(open(LEAGUE))
round_before = league["round"]
active = [s for s in league["strategies"] if not s["retired"] and not s["human"]]
if not active:
    raise SystemExit(0)


def per_civ(s, civ):
    """What `display_elo_for` would show this strategy for this civ."""
    best = None
    for civs in s.get("leader_elo", {}).values():
        r = civs.get(civ)
        if r and r["games"] > 0:
            best = r["rating"] if best is None else max(best, r["rating"])
    return s["rating"] if best is None else best


# Only civs that several actives have actually played are informative. On a civ
# with no history everyone falls back to their global rating, so the "bar"
# there is meaninglessly low and drags the estimate down.
def played_count(civ):
    return sum(1 for s in active
               for cs in s.get("leader_elo", {}).values()
               if cs.get(civ, {}).get("games", 0) > 0)

civs = sorted({c for s in active for lv in s.get("leader_elo", {}).values() for c in lv})
bars = sorted(sorted((per_civ(s, civ) for s in active), reverse=True)[2]
              for civ in civs if played_count(civ) >= 3 and len(active) >= 3)
if not bars:
    raise SystemExit(0)

# The barrier is not that veterans are inflated -- a per-civ table sits only
# ~13 points above its own global rating on median. It is that the top-3 pool
# for a civ is the third-best of a FIFTEEN-way maximum, and a newcomer brings
# one flat number to that contest. Measured here: p50 1769, p75 1800, against
# a roster whose best global rating is ~1762.
#
# Seed at the 60th percentile of that bar, so a challenger makes the pool on
# roughly six civs in ten and draws about one seat a game -- enough to build a
# real sample without simply displacing the proven roster. The guard stops
# entirely at PROVISIONAL_GAMES, by which point it has its own per-civ tables
# and competes on the same terms as everyone else.
seed = bars[int(0.60 * (len(bars) - 1))]
# Sanity bound on the same scale the seed lives on. Clamping to the best
# GLOBAL rating (~1761) was the wrong comparison -- global and per-civ are
# different distributions, and that clamp silently undid the p60 calibration.
seed = min(seed, bars[int(0.90 * (len(bars) - 1))])

moved = []
for s in active:
    if s["name"] not in CHALLENGERS or s["games"] >= PROVISIONAL_GAMES:
        continue
    # Lift only, never lower. The `and rd > 349` form pulled CardShark down
    # from an earned 1858 to the bar because its RD had converged -- the guard
    # exists to stop a challenger falling out of contention, not to hold it at
    # the bar. A challenger at or above the bar is already seatable; leave it.
    if s["rating"] >= seed - 1.0:
        continue
    moved.append(f"{s['username']}({s['name']}) {s['rating']:.0f}->{seed:.0f} "
                 f"after {s['games']} games")
    s["rating"] = seed

if not moved:
    raise SystemExit(0)

# A game finishing mid-edit would rate into the copy we read; bail rather than
# write a roster that has lost a rating period.
if json.load(open(LEAGUE))["round"] != round_before:
    raise SystemExit(0)
fd, tmp = tempfile.mkstemp(dir=os.path.dirname(LEAGUE), suffix=".tmp")
with os.fdopen(fd, "w") as fh:
    json.dump(league, fh)
os.replace(tmp, LEAGUE)
print(f"CHALLENGER-RESEEDED (per-civ seat bar {seed:.0f}) " + "; ".join(moved))
PY
