# A short Korean Science Seowon before parity recruitment

## Observed problem

In native Deity Korea run `civvis-20261105T152431Z-seowon-investment-pilot-01`,
Jeonju first had a legal Seowon quote on turn 23, but completed the district
on turn 94. The journal identifies near-rival deterrence as its first binding
Campus hold on 17 distinct turn/reason/queue rows beginning at turn 61. Other
holds also occurred; these rows are not 17 independently saved turns.

At the selected request 61/0, the native menu offered a five-turn Seowon:
31 production, zero invested, and four legal sites. The recorded controller
source was `a4884eab8765382ef1c928c6d9e4f41e9bd599b1`, executable SHA256
`0e401e4a02fa681040e2825fc021905e955c87b154375faceafe5fbd422d613a`.
The completed game lost to rival Culture at turn 179. This is an observed
failure case, not a representative strength sample.

## Policy change

Existing opt-in `campus-claims-the-free-slot-2` lets a Korean Science city
put its first Seowon before a near-rival parity unit when the host explicitly
quotes at most five positive finite turns. There must already be two cities,
the plan must be Science or Expansion, and no major war, threatened city,
recent attack on the host city, or rush may be active.

The ordinary Campus picker, district slot/site checks, finishing queue,
wall-breaker and other earlier claim holds remain. In particular, this does
not relax the Settler floor: its separate three-turn repeated-Settler exception
and six-city target are unchanged. Displaced military production stays banked.

Near-rival recruitment respects a committed first Seowon under the same
peaceful Science conditions. This includes its zero-progress placement replan,
when losing a worked plot can change the remaining quote. The commitment
protection does not depend on reproducing the initial quote. Recruitment can
still use other cities, and active-war or emergency defence retains priority.

No flag, registry position, deployment default, game rule, native adapter,
or runtime override is changed.

## Captured proposal evidence

The witness request was selected as the first explicit near-rival Campus-hold
turn before any query; all 18 original reads had identical captured bytes:
`25697f442ec2099339b133e493e5192e75559be800117f8f98995423bf42fd23`.
Four successful cold proposals used that exact input and the original source:

| Role | Near-rival setting | Jeonju production | Actual reads |
| --- | --- | --- | --- |
| 01-on | on | Warrior | 17 |
| 02-off-cli-corrected | off | legal Seowon at native 6,10 | 17 |
| 03-on | on | Warrior | 17 |
| 04-off | off | legal Seowon at native 6,10 | 17 |

The full replies repeat within each setting. The actual genomes differ only
by the named near-rival bit in their treatments/forced lists. The initial
02-off command inherited `--with near-rival-deterrence` and added a contradictory
`--without`: it exited 2 with empty stdout before any decision. That failure
is retained. A separate amendment made OFF omit the forced opt-in and passed
an EOF genome preflight before the remaining queries. This is not a claim
that the original four-role attempt passed unchanged.

The [raw witness archive](2026-11-05-korea-campus-parity.witness.tar.xz)
contains the original sealed captured reads, all successful replies and captures,
the failed command/log, amendment, source provenance/context, file hashes and
`verify_archive.py`. Extract it and run that script with Python 3.12.

These proposals establish which independent guard held the queue and that
removing it proposes the native-legal district. They are **not** measurements
of this candidate's activation, uninterrupted original private-state parity,
native execution, elapsed turns saved, or win-rate gain. The candidate's own
same-binary OFF/ON proposal qualification and native evaluation remain pending.

## Evaluation instrumentation corrections

A separate saved-world study mistakenly attributed these named holds to
`peacetime-deterrence`. Its actual OFF arm kept `near-rival-deterrence` on;
the settings own different mechanisms. Its qualified control lost to rival
Science at turn 179. The OFF arm was censored at 4/1 by an argv qualification
assertion, and its dependent industry arm stayed unlaunched. They are not a
completed direct-guard or industry contrast.

Reconstructing the frozen command builder shows that its `--without` pair
precedes `--capture-inputs`, whereas the private expected command appended it
afterward. Reordering only that pair exactly reproduces the builder's command.
The actual failing KERN argv was not saved, so this is an instrumentation
diagnosis, not retroactive qualification. Preserve those failed studies. Future
native preflights must use the frozen command builder and save actual argv
before assertions.

## Candidate validation

- Scoped rustfmt and whitespace checks: passed before checkpoint.
- Three new tests cover bounded handoff, banked military progress, zero-progress
  retention after a quote change, and negative scope/budget/emergency cases.
- Rust execution, full CI, exact-source native candidate build/proposals, and
  native strength evaluation: pending. No strength improvement is claimed.
