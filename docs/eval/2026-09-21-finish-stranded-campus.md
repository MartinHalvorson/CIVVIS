# Finishing a stranded Campus: investigation

Native run `civvis-20260921T214546Z`, pinned to `82741ed94`, placed Popayán's
Campus around turn 62 and still had it unfinished at turn 172. Native offers
continued to allow completion: 12 turns at turn90, six at120, and two at150
and172, at a locked 70 production cost. The city produced 4.5 science while
that Campus remained unfinished. At162 the nine-city empire produced58.91
science against rivals at108.91–144.70.

The research catch-up planner previously admitted only Campus buildings,
excluding the foundation that makes those buildings possible. The experiment
admits an already placed Campus foundation for Domination under the existing
public-science shortfall gate. It retains idle-queue, home-emergency, Recovery,
war-plan, and affordability checks. The existing production commitment also
preserves that new reservation before its first hammer. New sites and other
district families do not enter this path. Yield accounting credits the
Campus's own yield, not a hypothetical future Library.

The baseline focused test fails with an empty queue despite a legal Campus
completion. Further tests and paired replay remain in progress; no native
research gain or conquest benefit is yet established.

Frozen input through172:
`/tmp/civvis-campus-finish-replay/native-events.jsonl`, SHA256
`778b5f296b6ec9cc441bbc87a0f391267efdfe2d6dbf12b827b101df0f2e97d7`.
The baseline archive is exact parent`7e6ed5e47e7b1768d02efaf5f2aa14ee6e1db41e`.
