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
