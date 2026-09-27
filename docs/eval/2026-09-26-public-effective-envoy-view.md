# Public effective Envoy readback

This is an observed-board correctness repair, not a new domination policy or
evidence that native games resumed. The requested game remains level4/Prince,
Simón/Gran Colombia, four-player Tiny Pangaea, domination, all victories enabled.

## Reproduced refusal

Two independent read-only instrumented replays of the already registered
Prince seed37930100 reproduce the frozen511c candidate's entire outcome record
and every canonical applied action: OFF48547, ON47985. Original source, policy
bundle, seeds and negative campaign records are retained. Instrumentation only
plans with silent controller clones and probes disposable world/view copies;
no diagnostic result is fed back into the original controller.

At the focal opening frame236 there are no finishing targets and97 ordinary
actions. The first ordinary order, `SendEnvoy{player:9}`, succeeds on the
untouched observed copy and fails on the untouched authoritative copy with
`invalid city-state`. A tactical refusal aborts the ordinary batch in the
existing observed executor; no dispatcher refusal behavior is changed here.

The second replay establishes the differing legality predicate, rather than
inferring it from a journal purchase message:

| Opening236 fact | Authoritative world | Observed view |
| --- | --- | --- |
| Acting seat / free Envoys | 0 /10 | 0 /10 |
| Minor9 alive / met | true /true | true /true |
| Explicit war / host permission | false /none | false /none |
| Own delegation | 18 | 18 |
| India's raw / effective delegation | 18 /20 | 18 /18 |
| Minor9 Suzerain | India, seat3 | none, false18–18 tie |
| Derived war with minor9 | true | false |

Rival governor redaction correctly removes India's private Amani record, but
the old Envoy copy retained its raw18 instead of the public effective20.
Removing the two-Envoy Messenger contribution invents a tie, which loses the
city-state's derived war and offers the invalid first order.

Evidence is under
`~/civvis-tactics-results/2026-09-26/air-resource-expedition-pr3793/`:
`purchase-refusal-37930100/` and `envoy-refusal-37930100/` retain plans, hashes,
complete pair outputs, canonical traces, logs and reproduction receipts.
The follow-up's first launch failed before any AI turn because its trace
directory was missing; the empty output and failure log remain, and the exact
same frozen binary/seed were rerun after creating only that directory.

## Public-source boundary

The shipped game reads effective tokens, not a rival's private governor roster:
`Base/Assets/UI/PartialScreens/CityStates.lua:1452` reads
`pPlayerInfluence:GetTokensReceived(pLocalPlayer:GetID())`; line1458 reads
`pPlayerInfluence:GetTokensReceived(iInfluencePlayer)` for each alive major.
Lines1453–1454 also read the largest delegation and current Suzerain. These
lines were read from this machine's Civilization VI install. Existing native
mirroring already treats rival delegations as effective host readings.

The bounded repair copies public effective delegations for known rivals at
known city-states while leaving rival governor records redacted. It enumerates
known city-states, rather than only existing raw entries, because Puppeteer
can create four effective Envoys from zero raw placements. The observing
player's raw balance and own Amani record remain unchanged to avoid adding
their bonus twice. No new identity, hidden city, governor assignment/promotion,
AI policy, tactical guard, force bundle or native runtime write is introduced.

## Validation status

Eight focused regression tests have been added, including the exact false-tie
legality, zero-raw Puppeteer, private-state/non-mutation boundary, own-Amani
double-count guard, unmet-minor privacy, repeated observations, neutralization
refresh, and the full observed-player dispatcher. The unchanged implementation
is being run first to establish a genuine failing regression. Fix validation,
full-suite/cost results, and integrated campaign evidence are pending.

No later Recon purchase success, aluminum allocation repair, stronger
Domination result, native quit-dialog recovery or resumed native game is claimed.
