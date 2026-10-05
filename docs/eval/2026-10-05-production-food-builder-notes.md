# Immediate city production from food improvements

Snapshot diagnostic in progress. The hypothesis is that a worked food
improvement can release a citizen to a production tile immediately, even when
the improved tile itself gains no Production. The probe prices a legal Farm on
static native decision-board clones and records the actual native city yield
and citizen-assignment changes. It preserves every input board unchanged.

Only previously consumed turn-25/50/75 control snapshots are used. A walk that
exhausts movement may be priced with the input Builder's start-of-frame allowance
on a separate static copy; that case is not a same-turn action, a simulated
future turn, or capture-safety proof. No game, controller decision, engine yield,
cost or private profile is changed. Compilation and input freezing are pending.

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
