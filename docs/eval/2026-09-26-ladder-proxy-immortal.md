# The ladder proxy at Immortal: the Science lane, Settlers that walk three times the distance, and Builders the capture lessons hold back

_2026-09-26 · `claude-opus55-0926` on `martins-m4-air` · simulator rounds, no native games_

Follow-up to `docs/eval/2026-09-24-ladder-proxy.md` and
`docs/eval/2026-09-24-ladder-proxy-round-2.md`. Same instrument
(`examples/ladder_proxy.rs`, private, not committed) and shape: 4 majors, Tiny
60×38 Pangaea, Online, 6 city-states, the focal seat the live controller with
`handicap_exempt`, three rival seats `AdvancedAi::new()` +
`enable_live_bridge()` carrying the rung's full AI bonuses. This round is at
**Immortal** — the native ladder's next rung (Emperor was claimed 2026-09-09;
Immortal is 0 for 21 finished attempts). Every batch is seeds 61000000+, full
250-turn clock, paired by seed; "share" is the focal seat's end score over the
four majors' total.

New instruments: a Settler lifetime census (turns from the Settler's first
sighting to the city it founds, its trip distance, and losses), a barbarian
camp census (how long each camp within seven tiles of a seat's cities stands,
who clears it), Science per turn and earned boosts at turns 60/100, and a
luxury audit (luxury types improved, types owned but unimproved, and for every
owned, improvable luxury tile the turns it waits).

## The rung, and how well the proxy tracks the native game

| Immortal, 32 games | proxy (unassigned `civvis` seat) | native ladder (21 finished) |
|---|---:|---:|
| focal wins | 0 | 0 |
| focal techs at turn 100 | 24.6 | ~25 |
| best rival's techs at turn 100 | 42.3 | ~39 |
| score share | 10.3% (25% is parity) | — |

Immortal rivals take +24% Science and Culture, +60% Production and Gold, three
free Eurekas and three free Inspirations each era, and a free Settler. By turn
50 the focal holds 10 technologies against the rivals' 15–22 at equal
population; at turn 100 it makes 53 Science a turn against a rival mean of 167
(population 34 against 63; 1.6 Science per citizen against 2.6).

## The lane: Science beats the unassigned seat at Immortal

| Immortal, paired against the unassigned seat | Δ share | z | alive at the end |
|---|---:|---:|---:|
| Science lane (32) | **+1.46 pp** | **+2.62** | 27 → 31 |
| Culture lane (32) | −0.15 pp | −0.20 | 27 → 25 |
| Diplomacy (8) | +0.78 pp | +0.49 | 7 → 7 |
| Religion (8) | +0.51 pp | +0.48 | 7 → 7 |
| Domination (8) | −0.94 pp | −0.82 | 7 → 7 |

At King round 2 found the unassigned seat 2.3–3.0 pp ahead of the Science lane;
at Emperor the two were level. At Immortal the order has turned: the lane's
wider, better-defended first half pays, as it began to at Emperor. The native
seat already runs the Science lane at Immortal; this is the proxy agreeing.

## Default-off genes: most never fire for the live seat, and none that fires moves Immortal

A three-seed, 100-turn inertness screen of 45 default-off genes on the
unassigned seat found 32 byte-identical to the baseline — among them
`skip-the-prophet-race-2` and `campus-before-halfway`, which only act for an
assigned Science seat. On the Science seat, 38 of 52 were inert. The genes that
fired and showed a three-seed signal were played 32 paired games:

| Immortal, unassigned seat, 32 paired | Δ share | z |
|---|---:|---:|
| `boost-unlock-research` | +0.27 pp | +0.48 |
| `district-planning-3` | −0.46 pp | −1.16 (alive 29 → 24) |
| `early-archers` | −0.36 pp | −0.81 (alive 29 → 26) |
| `enter-the-prophet-race-2` | −0.21 pp | −0.74 |
| `camp-party` | −0.43 pp | −0.71 |

`camp-party` read +1.10 pp on six seeds before it read −0.43 on 32: six-seed
arms here carry a standard error near 1 pp and only pick what to confirm.

## Settlers walk two to three times the distance

Every seat, focal and rival, King and Immortal alike, takes a mean **10.6
turns** from a Settler's first sighting to its city (median 9, p90 22). A
six-tile trip takes a median 13 turns for the focal seat and 8 for a rival,
against three to four for the walk itself. Settlers are almost never lost (none
of the focal's 55 over eight Immortal games): the holds buy safety with time.
Without barbarians the mean falls only to 8.5, so raiders explain about two of
the extra turns. Turning the Settler holds off one at a time does not help —
`settler-guard-holds-2` off reads −0.79 pp with trips of 12.2 turns,
`settler-never-idles` off 11.9, `settler-walk-deadline` off −1.32 pp;
`settler-target-hysteresis-2`, `settler-site-gate` and `settler-target-floor`
are inert.

## Camps: a force sent where it cannot win, and the camps that matter left standing

A per-camp trace of one Immortal game (seed 61000004) shows the objective
board's ClearCamp force — two Warriors — standing beside an eight-tile coastal
camp held by a fortified Spearman and three Galleys for five turns without an
attack (correctly: the exchange is bad), while a camp five tiles from a city,
the one whose raiders blocked a Settler's target, drew no unit for twelve
turns. The ClearCamp row asks for no ranged unit and values every camp in its
nine-tile radius alike. Two prototypes (`camp-needs-a-shooter`: a guarded
camp's row requires a shooter; `camp-nearest-first`: a nearer camp is worth
more) read +0.18 pp and −1.32 pp on six seeds; neither was taken further.

## Builders the capture lessons hold back

At turn 50 the Science-lane focal seat has 44% of its cities Displeased (−10%
yields, −15% growth) against the rivals' 11%, and has improved 0.6 luxury
types. The luxury audit found why: owned luxuries whose improvement is legal
and unlocked (Camp, Plantation, Quarry), on tiles the city is **working**, wait
a median 15 turns (mean 23) to be improved, with a Builder one to three tiles
away the whole time. In seed 61000000 Tobacco waited 46 turns (33 → 79) and
Marble 44. The Builders' journal gives the reason on nearly every turn: *"Builder
waits outside a barbarian's reach"*, *"Builder retreats from a hostile's
reach"*. Those holds are the live seat's capture lessons
(`live-settler-capture-lessons`, host-only, 2026-08-28), which reconstruct
every Settler lost on the live seat — and which `civilian_reach_safety_on`
applies to Builders as well: a job tile any raider could stand on next turn is
not a job, and a route step into any raider's reach is refused.

`builders-work-through-raiders` (opt-in) keeps the lessons for the Settlers and
gives the Builder the native Builder safety (`builder-barbarian-safety`, which
still refuses a tile a raider can take this turn):

| Immortal, Science lane, 32 paired (61000000+) | without | with the gene |
|---|---:|---:|
| luxury wait, median / mean turns | 15 / 23.2 | **7 / 9.4** |
| luxury types improved at turn 50 | 0.62 | 1.59 |
| Displeased cities at turn 50 | 1.38 | 0.91 |
| population at turn 60 / 100 | 17.0 / 36.0 | 18.3 / 38.8 |
| Science a turn at turn 100 | 56.3 | 59.6 |
| technologies at turn 150 | 36.9 | 38.1 |
| Settlers lost (all 32 games) | 4 | 0 |
| end score | 397 | 427 |
| **score share** | 11.6% | **+0.46 pp (z +0.97)** |

Two costs show. A Builder now spends its charges, so the governor replaces it
sooner (four games: 23 Builders built to turn 150 against 17, while fewer were
lost, 0.75 against 1.25), and that Production is not a Settler — cities at turn
100 are level (6.25 against 6.28), not ahead. And turning the whole lesson set
off for the focal seat (eight seeds, Settlers included) moved the economy
further (population at turn 100 +5.2, Science +17) for one Settler lost: the
Settler half of the lessons costs growth too; repricing it belongs to the
Settler holds, not to this gene.

## Aggressive rivals: the native failure the default proxy never plays

The native ladder loses Immortal by elimination as often as by a rival's
victory: 7 of 21 finished attempts ended with the seat destroyed, and cities
were lost in most of the rest. The proxy's default rivals (the deployment
genome, adaptive) almost never do that — the focal seat survives 27–31 of 32 —
and they never let the focal win, even at King (0 of 32 on every arm this
round): they race Science and Culture flat out. A second rival mode retargets
every rival to the Domination lane (`AdvancedAi::retarget(VictoryTarget::Domination)`
on the live-bridge genome, Immortal bonuses unchanged). Against it the focal is
eliminated in a third to a half of the games, loses 3–5 cities a game, and the
games that survive run to the turn limit — the native shape — where the focal
can win:

| Immortal, every rival on the Domination lane (61000000+) | wins | alive at the end | cities lost / game | score share |
|---|---:|---:|---:|---:|
| Science lane (32) | 4 (12%) | 22 | 3.1 | 16.0% |
| unassigned `civvis` (31) | 6 (19%), all Religious | 14 | 4.7 | 13.5% |
| Religion lane (32) | 7 (22%), 6 Religious | 14 | 5.1 | 12.8% |

Rivals racing only conquest leave religion open, and the seats that take it win
more often but die more often: paired against the Science lane, Religion won 6
games Science lost and lost 3 Science won (not significant at 32), for −3.26 pp
of share (z −1.56). The Science lane stays the recommendation.

A seed the focal lost (61000002) shows how: the seat holds 3 cities from turn 31
to 101 while its Settlers wander (one stood on legal sites at turns 60 and 100
without founding), a rival snowballs from 10 cities to 35, and everything is
taken by turn 180. The worst seed (61000029) had grown to 10 cities with 400–650
Gold banked against a 400 war reserve, then lost one every few turns from 164
to the rival's 4-to-1 army. A screen of twelve default-off defence genes on this
mode (six seeds each) found seven inert for the live seat and none worth
shipping at 32 paired games: `upgrade-the-garrison` +0.50 pp (z +0.55, alive
22 → 24), `naval-threat-triage` −0.44 pp, `relief-column-marches` −0.72 pp on 17.

Two engine bugs surfaced here and are fixed:

- **#3799** — a lethal WMD strike on a loaded Aircraft Carrier removed the
  carrier's aircraft with it and then indexed them (`unit N is not present`),
  ending the game. About one game in 31 on this mode.
- **#3803** — a city's own queued Spy counted against the capacity that decides
  whether that Spy is legal, so with one free slot the AI resumed it and
  replaced it every turn without finishing it (turns 105–124 of seed 61000029).
  Paired, the fix read +4.12 pp (z +1.31) on this mode and −0.26 pp on the
  default one.

## Friendships nobody made

A declared friendship forbids a war declaration between the pair while it
lasts (`start_war`: "friendship and alliance declarations must expire before
war"), and a seat values an offered friendship at +40 (`incoming_deal_value`;
+80 on the Diplomacy plan). A census of every seat at turns 60, 100 and 150
found **no friendship, alliance or defensive pact between any two majors** —
Immortal, both rival modes. The advanced controller's only friendship offer
rides `propose_strategic_alliance`, which waits for Civil Service on both sides
and bundles an alliance kind the partner must also want; the stock
controller's friendship cadence (`BasicAi::diplomacy`) is not on the advanced
turn. Against Domination-lane rivals the focal seat was attacked by neighbours
it had never asked.

`befriend-the-strongest` (opt-in, #3810) offers a friendship — and nothing
else — every third turn to the strongest met major it is at peace with, not
already a friend and not denounced either way:

| Science lane, 32 paired unless noted | Δ share | z | focal wins | alive | cities lost / game | friends at turn 100 |
|---|---:|---:|---:|---:|---:|---:|
| Immortal, Domination-lane rivals | **+10.65 pp** | **+5.22** | **4 → 19** | 22 → 25 | 3.06 → 1.81 | 2.5 of 3 |
| Immortal, adaptive rivals (16) | +1.11 pp | +2.60 | 0 → 0 | 16 → 16 | 0.75 → 0.44 | 3 of 3 |
| Immortal, adaptive rivals, fresh seeds 64000000+ (2026-09-27) | **+1.56 pp** | **+3.43** | 0 → 0 | 30 → 32 | 0.97 → 0.16 | 2.9 of 3 |
| Emperor, adaptive rivals, seeds 65000000+ (2026-09-27) | **+2.67 pp** | **+4.58** | 0 → 0 | 27 → 32 | 1.59 → 0.16 | 2.8 of 3 |
| King, adaptive rivals (16) | −0.10 pp | −0.13 | 1 → 2 | 16 → 16 | — | — |

The focal seat's 19 wins against Domination-lane rivals are 9 Religious, 5
Science and 5 Score: its would-be attackers spend their armies on each other.
⚠ The rivals here are our own genome, which accepts any friendship it values
above zero; Civilization VI's leaders accept by their opinion of the proposer
(`DiplomaticActions.xml` prices a declaration at Worth 15 from Friendly, −10
from Neutral, −40 from Unfriendly). A friendship-only deal now crosses the
bridge as a `friendship` order the agent opens as the shipped view does,
`RequestSession(…, "DECLARE_FRIEND")` (#3811), only toward a rival the host
reads as Friendly, and the verdict reads the host's `GetDeclaredFriendshipTurn`.
The gene itself now chooses among the rivals the host reads as Friendly
toward us (#3814, `Player::observed_diplomatic_state`), so a Neutral strongest
rival no longer holds the offer while a Friendly one waits; on a native board
nothing is observed and play is byte-identical. The gene stays unforced;
forcing it is what would let the ladder find out.

The fresh-seed row (2026-09-27, main `621c5c6`, 250-turn clock, the games end
at the first rival victory) repeats the adaptive-rival reading on 32 seeds the
first block never saw: every game the seat survived, and it lost one sixth of
the cities it lost without the gene. Pooled over the 48 adaptive Immortal
games the share reading is positive in both blocks. Emperor, the rung the
native ladder last won, reads the strongest of all: every one of the 32 games
survived where 27 had, and the seat declared 2 wars against 9 while ending
with 4 foreign cities against 1. The King reading (16) stays null.

### No engine seat can propose an alliance

With `befriend-the-strongest` on, the focal seat held about three friendships
at turn 150 in both the Immortal and the Emperor blocks, yet no major formed an
alliance with anyone, focal or rival. Counting the proposals in 8 Emperor games
(seeds 65000000+, main `5670348`) found **zero alliance proposals from any
seat**, although every seat had Civil Service between turns 38 and 90.
`propose_strategic_alliance` ran on its cadence, and every candidate failed
its partner filter on `other.civics.contains(civil_service)`.

The controller plans on `Game::player_decision_view`, which rebuilds each met
rival's `Player` from its public fields. Civics and techs are not among them;
only their counts cross (`observed_public_empire_stats`). On the view, then,
no rival holds Civil Service, Early Empire or Scientific Theory, and the
engine's own legality check on that view refuses every alliance, research
agreement and open-borders bundle with a rival. The host's diplomacy screen
shows which of those deals a met rival can sign, so the gating civics are
public there.

A bench-only change that carried just those three (`civil_service` and
`early_empire` civics, `scientific_theory` tech) into each met rival's view
brought alliances back: 0.9 per game for the focal seat and 1.5 for each
rival by turn 100. The rivals proposed 85, 39 and 35 alliances against the
focal Science seat's 16, since that seat asks only for a Research alliance,
which waits for Scientific Theory. Over 16 paired Emperor games the focal
share moved −0.88 pp (z −1.62): the world gains more from alliances than the
focal seat does. Not shipped. The live seat's bridge carries no alliance, so
the native ladder is unaffected either way. For whoever next touches the view
or the alliance desk: alliances are dead in every engine game until the view
carries those facts, and reviving them needs the Science seat to ask for the
kinds it can sign before Scientific Theory.

### The forced genome, audited one gene at a time

Each of the 21 genes in `deploy/live-force-on.txt` was switched off alone,
the rest of the deployed genome unchanged: Emperor, Science lane against
adaptive rivals, 16 paired games per gene (seeds 69000000+, main `5670348`).

| Off | Δ share | z | games changed |
|---|---:|---:|---:|
| `lane-delegates-production-2` | −2.33 pp | −2.52 | 16 |
| `builders-work-through-raiders` | −1.41 pp | −1.45 | 16 |
| `garrison-under-fire` | −0.98 pp | −0.82 | 16 |
| `early-conquest-opening` | −0.80 pp | −1.46 | 2 |
| `ranged-hp-reserve` | −0.44 pp | −0.80 | 6 |
| `rapid-city-expansion-2` | −0.43 pp | −0.36 | 16 |
| `siege-train` | +0.69 pp | +1.40 | 8 |
| `peacetime-deterrence` | +0.73 pp | +1.16 | 15 |

The other thirteen changed at most five of the sixteen games and moved share
by 0.13 pp or less: `city-campaign-2`, `domination-lane-hands-over` and
`science-expansion-phase` changed none, and most of the rest are Domination
machinery this seat never runs. The two suspects were re-run on 32 fresh
Emperor seeds (70000000+): `siege-train` off read −0.22 pp (z −0.53), noise;
`peacetime-deterrence` off read **+0.73 pp (z +1.83)** again, with cities
lost per game 1.12 → 0.50. At Immortal (32, seeds 71000000+) it read
+0.23 pp (z +0.94).

`peacetime-deterrence` sizes the peacetime army against the strongest met
major. A friendship forbids that major's war for thirty turns, which is the
threat the floor buys an army against, so the two were measured together at
Immortal on the same 32 seeds:

| Immortal, 32 paired (seeds 71000000+) | Δ share | z | cities lost / game |
|---|---:|---:|---:|
| `befriend-the-strongest` on | +1.28 pp | +2.79 | 0.97 → 0.22 |
| … and `peacetime-deterrence` off, against befriend on | +0.29 pp | +1.35 | 0.22 → 0.16 |
| **both, against the deployed genome** | **+1.56 pp** | **+3.82** | **0.97 → 0.16** |

This is the third independent Immortal block for `befriend-the-strongest`.

## What was decided

- **Shipped, off** (#3781): `builders-work-through-raiders`, an opt-in gene
  with its single-gene fires probe
  (`docs/gene_screens/fires/builders-work-through-raiders.json`).
- **Forced on the live seat** (#3790) after the lower rungs read positive on
  the same instrument (Science lane, 32 paired each; a second Immortal block,
  61000032+, read +0.72 pp, z +1.63 — pooled over 64 Immortal games +0.59 pp,
  z +1.84):

  | rung | Δ share | z | population at turn 100 | Science at turn 100 | luxury types at turn 50 |
  |---|---:|---:|---:|---:|---:|
  | King (37140000+) | **+2.09 pp** | **+3.48** | 39.0 → 43.4 | 64.9 → 77.2 | 0.78 → 1.72 |
  | Emperor (51000000+) | +0.39 pp | +0.70 | 36.7 → 38.2 | 56.5 → 60.8 | 0.97 → 1.44 |
  | Immortal (61000000+) | +0.46 pp | +0.97 | 36.0 → 38.8 | 56.3 → 59.6 | 0.62 → 1.59 |

  Displeased cities at turn 50 fall at every rung (King 1.41 → 0.88, Emperor
  1.50 → 0.84, Immortal 1.38 → 0.91). Settlers lost: King 0 → 3 (three
  games, one each; the traced one wandered 35 turns and fell to a rival at
  war), Emperor 5 → 5, Immortal 4 → 0. The live seat is the Civilization VI
  seat and its raiders are Firaxis', which these games do not play; the
  Settlers keep every capture lesson, and a Builder still refuses a tile a
  raider can take this turn.
- **Shipped, off** (#3810): `befriend-the-strongest` — against Domination-lane
  rivals the focal seat's wins 4 → 19 of 32 (z +5.22 on share); against
  adaptive rivals +1.11 pp (16) and +1.56 pp (32 fresh, z +3.43) at Immortal,
  +2.67 pp (32, z +4.58) at Emperor. The live
  seat can now carry it: the friendship order (#3811) and the host's attitude
  (#3814).
- **Recommended to the operator, not changed**: one live package, forcing
  `befriend-the-strongest` and removing `peacetime-deterrence` from
  `deploy/live-force-on.txt` (+1.56 pp, z +3.82 at Immortal against the
  deployed genome; the deterrence half alone +0.73 pp, z +1.83 at Emperor).
  Deterrence was forced after a native loss to a rival's army at t157
  (`civvis-20260803T220954Z`); the friendship covers that threat from the
  strongest rival, and the native ladder is where the package would be
  proven. The bridge's `DECLARE_FRIEND` session has not yet run in a native
  game; its answer arrives as a leader scene the autoclose already dismisses
  for delegations and denouncements, and the verdict ledger will show
  `friendship` orders as verified (`friendship_turn`) or `not_friends`.
- **Recommended to the operator, not changed**: the Science lane at Immortal
  (+1.46 pp, z +2.62, over the unassigned seat), which the native ladder
  already runs.
- **Not shipped**: `camp-party` (−0.43 pp), `boost-unlock-research`,
  `district-planning-3`, `early-archers`, `enter-the-prophet-race-2` (all null
  or negative at 32 games), the camp prototypes, and every single Settler-hold
  switch.
