# Immediate city production from food improvements

Diagnostic of previously consumed native controls. The hypothesis is that a worked food
improvement can release a citizen to a production tile immediately, even when
the improved tile itself gains no Production. The probe prices a legal Farm on
static native decision-board clones and records the actual native city yield
and citizen-assignment changes. It preserves every input board unchanged.

Only previously consumed turn-25/50/75 control snapshots are used. A walk that
exhausts movement may be priced with the input Builder's start-of-frame allowance
on a separate static copy; that case is not a same-turn action, a simulated
future turn, or capture-safety proof. No game, controller decision, engine yield,
cost or private profile is changed. Normal optimized compilation, artifact freezing, and all 24 input scans completed.

The normal optimized snapshot probe compiled and evaluated all 24 assigned
consumed views. Every source file hash and parent board remained unchanged.
Four Emperor and seven Deity structural Farm models added Food without losing
tile Production. No Emperor case increased city Production immediately. Two
Deity cases increased city Production by 0.65 and 1.00, respectively, with
unchanged city Food and Science; citizen assignments changed. Both had a foreign
military unit one hex away in the input, which may be neutral. The second case
needed a fresh allowance for its static operation, and neither is an executed
job or capture-safety proof.

A passive whole-turn coverage replay is now prepared on the same eight consumed
controls to measure frequency beyond these three checkpoints and record known
hostile reach. Its complete action histories and final worlds must reproduce
the preceding controls before the observations guide an intervention. No fresh
strength test or native controller edit has started.

The first whole-turn probe compile caught a missing Action import before any
replay started. It was corrected; that failed compile is retained separately
and is not passing validation.

An additional static attribution audit prices the city immediately after the
modeled walk and before the Farm. This separates potential movement effects,
such as a tribal-village reward, from the improvement's own effect. The frozen
whole-turn observer remains unchanged while its eight controls are in flight.


The passive whole-turn observer completed all eight consumed control games.
Every action history and final world reproduced the preceding control byte for
byte, and journals were contiguous without resets or truncated turns. There
were two positive Production models before turn 75 on Emperor (both in one
map) and 24 on Deity (12 in each of two maps); repeated turns and Builders do
not count as distinct jobs. Eleven models passed an exploratory filter for no
Food/Science loss, no known hostile native reach and no camp record within
eight hexes. This is incomplete future safety, not a safety guarantee.

The whole-turn endpoint delta combines the walk and Farm. A separate 24-view
attribution audit measured city yields after walking and before improving.
Both original positive snapshot cases had exactly unchanged city yields from
walking: their +0.65 and +1.00 Production changes came from the Farm and native
citizen reassignment. These are modeled structural actions, not executed gains.

The early exploratory models reduced to one Emperor and three Deity jobs.
The Emperor Builder moved toward and built a Pasture. The Deity Builders made
no action in several modeled turns; one later moved around without completing
a Farm. This does not establish which capture, travel, reservation or job-value
rule was decisive. A Farm-only intervention is too sparsely supported to begin
fresh strength testing from this evidence alone.

An additional diagnostic now checks all legal Builder improvements on owned
reachable land, including unworked tiles. It uses an after-walk baseline for
each independent improvement copy. Its normal optimized compilation and all 24 scans completed; every parent board and input hash remained unchanged. No
native runtime source, engine rules, controller weights or private profiles
have changed; no fresh strength samples or Firaxis games have been played.


All-improvement scan: Emperor had 83 legal models, 27 with positive city
Production and 10 positive models on previously unworked tiles. Deity had 33,
eight and three respectively. Citizen assignments changed in 10 Emperor and
five Deity positive models, mostly at turn 75. Some of these exchanged Food or
Science for Production. The early sample does not support a broad immediate
reassignment premium as a demonstrated strength improvement.

Decision-trace inspection found a more concrete constraint: the idle Deity
Builders 422 and 442 repeatedly refused route steps into known Barbarian reach,
although this observer could price nearby structural Farm models. Existing
capture protection was doing useful work. The next investigation should check
whether dangerous high-ranked jobs consume the bounded route attempts and
prevent a legal protected alternate job from being considered. No safety bar
has been relaxed and no routing prototype has been implemented here.
