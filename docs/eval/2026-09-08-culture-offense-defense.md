# Culture offense and defense after Rome's turn-208 loss

The retained Emperor/Online run `civvis-20260908T173748Z` ended in a rival
culture victory at turn 208. Its state exports show Canada's visitors rising
from 65 at turn 175 to 143 at 190, 180 at 200 and 267 at 208. Rome's domestic
tourists were only 61, 69, 54 and 47 at those turns. Another rival held the
largest domestic count, so simply switching Rome to Culture in the last few
turns would not have raised the victory threshold.

The source of truth is the host's published tourist counters, not score or
an inferred total from our model's short culture history. The shipped
`Base/Assets/UI/PartialScreens/WorldRankings.lua:1674-1687` reads
`GetStaycationers()` and `GetTouristsTo()` and finds the largest rival
staycationer count. These are the observations the mirror already imports.

## Offense

The culture production objective now gives additional value to Theater
Square coverage and the completed Amphitheater/Museum/Broadcast Center chain,
including its Great Work capacity and cultural Great Person points. This
applies when an explicit Culture target is temporarily planning expansion,
as well as to an adaptive Culture strategy. The premium fades when there
is too little time left after completion, capped at the normal game-speed
turn limit even when live verification allows 650 turns. It does not override illegal
production or military/emergency refusal sentinels, or create a blanket
wonder-building priority. Culture targets also use the same low-reserve,
negative-income recovery gate as science targets even with war economy off;
empty queues repair solvency before adding cultural upkeep.

Tourism route bonuses now apply only to living opposing major civilizations.
Previously city-state destinations received the same bonus despite not being
tourism markets. The first route to each rival retains the bonus, duplicate
routes do not stack, and an expired route can earn it again. Among uncovered
markets, greater domestic resistance increases the priority: drawing visitors
from the civilization holding the bar also erodes that bar. Ordinary yields
are still priced separately, so internal and city-state routes remain valid
economic options. Existing park, resort, Rock Band, Great Work purchase,
research and culture policy machinery remains in use.

## Defense

Known opponents reaching 50% of the *global* culture-victory threshold trigger
preparation. This is a policy threshold, not a time-to-victory prediction.
The controller stops selling Open Borders to those rivals and preserves
its Great Works while any such threat exists. Purchases, ordinary resource
trade, and harmless passage sales remain possible. Borders are directional:
our buying their passage helps our tourism; granting ours helps theirs.
The shipped `Base/Assets/Gameplay/Data/GlobalParameters.xml:575` sets
`TOURISM_OPEN_BORDERS_BONUS` to 25.

Space Tourism becomes a policy priority while a threat exists. Music
Censorship joins it when a threatening opponent knows Cold War. Both still
require their own unlocked, legally available policy. They may replace an
ordinary desired card of the same slot type, so a full deck no longer silently
blocks the defense. They protect each other from replacement and lose their
special priority when the threat disappears. Music Censorship has a real
amenity cost; it is not an unconditional opening policy. Shipped evidence:
`DLC/Expansion2/Data/Expansion2_Policies.xml:188` requires Space Race;
lines 296-301 attach the Rock Band entry blocker and amenity loss modifiers.

A seat at least 75% of the largest domestic count can also prioritize timely
culture buildings and Theater Squares, including lifting another victory
lane's Great Work-building veto for a useful culture building. A seat far
below that bar retains its chosen objective and existing culture floor rather
than pretending a hopeless last-minute pivot is effective defense. Buildings
that cannot finish within 30 Standard-equivalent turns receive no emergency
premium. No culture defenses activate when Culture victory is disabled, and
the frozen legacy controller retains its previous behavior.

## Evaluation

Regression coverage uses observed tourist counters, actual bilateral trades,
occupied policy slots, production scoring, completion deadlines, duplicate
and expired routes, city-states, teammates, disabled victories, and legacy
behavior. Completed-game soaks test both Culture-only and mixed victory
conditions. These checks establish working decisions, not a proven live
win-rate increase.

For the next frozen-revision batch, retain actual binary/genome identity and
complete outcomes. Track our tourist count relative to the largest opposing
domestic count; filled cultural buildings and Great Works; distinct tourism
route markets; the first rival crossing 50%; defensive policy readback; and
whether we issue tourism-enabling sales after that crossing. Track solvency
alongside these metrics: an unaffordable cultural build order is still a
losing build order. Compare completed games rather than counting resumed
copies as independent wins.

Local validation completed with 3,179 tests passed and 50 ignored. Six-player
Emperor/Online soaks (250-turn cap) finished Culture-only seeds 820260908 and
820260909 with a culture victory at 131 and a draw at 250; mixed-victory seeds
820260910 and 820260911 ended in science victories at 189 and 183. The draw is
retained as a draw, not counted as a culture win.

A read-only `civvis_orders` replay of the retained live turn-200 board, with
its science target and original treatment flags, emits a `policy_deck` that
includes `POLICY_MUSIC_CENSORSHIP` while retaining the science objective.
This verifies host-order generation, not host acceptance or a changed outcome
for the already completed game. Replay output and validation logs are retained
in the host's `civvis-culture-evidence-20260908` directory.
