# Evaluation integrity: what failed, why it was possible, and what closes it

The 2026-07-31 stack audit (PR #659) found six defects. Five of them are in the
*evaluator*, not in the agents — which matters more than it sounds, because the
evaluator is the only thing that decides what ships. An engine optimized for
winning cannot be steered by an instrument that can report a negative result as
a promotion.

This document is the remediation plan. For each defect it separates **the fix**
from **the root cause fix**, because every one of these was a recurrence: the
repository has repaired the *instance* of each of these classes before and the
class came back.

Nothing here is a measurement. `docs/EVAL.md` holds evidence; this holds design.

---

## 1. The shape the defects share

Four root causes generate all six defects.

| # | root cause | defects it produced |
|---|---|---|
| **R1** | **Agent identity is a label, and the label is maintained separately from the thing it labels.** | confounded controls; inverted self-comparison guard |
| **R2** | **Degradation is silent and permitted by default**: constructing an agent that cannot do what its name promises *succeeds*. | three controllers that never run; and the guard inversion, which is only reachable on a degraded name |
| **R3** | **A decision procedure is reused as an estimator.** A promotion gate is tuned to accept or reject; its point estimate is then quoted as the size of the effect. | every published effect size is selected on having passed, so every one is biased upward |
| **R4** | **Derived display artifacts are bound neither to their source nor to their semantics.** | the README ranking table |

R1 and R3 are the expensive ones. R1 decides whether a comparison means
anything; R3 decides whether a number means what it says.

---

## 2. R1 — identity is a label maintained apart from the thing

### The defect

`elo::builtin_ai` (`src/elo.rs:1169`) is a `match` over 78 names that
constructs agents. `elo::builtin_provenance` (`src/elo.rs:2059`) is a *second,
independent* `match` over the same 78 names that reports what those agents are.
Its own doc comment states the contract — “Resolve what `builtin_ai(name, _)`
will actually construct” — and **nothing binds the two**. No test, no shared
table, no type.

They have drifted, and the drift is not partial. `builtin_provenance` maps a
net-less `policy` to effective name `"advanced"` and a net-less `neural` to
`"basic"`, on the model that with no artifacts these names fall all the way
back to the stock scripted agents. That model was true when it was written. It
stopped being true when `#469`/`#471` embedded the champion genome:
`evolve::load_champion` now resolves local → `data/` → **embedded**, and
`embedded_champion` fires for exactly the directory production uses
(`ARTIFACT_DIR = "evolved"`, `src/elo.rs:1968`). The genome can no longer be
absent in production, so netless `policy` is *always* champion-weight
`AdvancedAi` — which is `advanced_evolved`, not `advanced`.

The mapping is therefore not occasionally wrong. **It cannot be right.**

Two consequences, both reproduced in the audit:

- **The self-comparison guard fires backwards.** `collapsed_entrants`
  (`src/elo.rs:2283`) compares `left.effective == right.effective`. So
  `ai_eval neural basic` warns “both play as basic … says nothing about either
  name” and then reports 68.8%, +137 Elo-equivalent, p < 0.0001, **gate PASS**;
  while `ai_eval policy advanced_evolved`, which are byte-identical in
  behaviour (20 of 20 mirrored maps neutral, every diagnostic column equal to
  the digit), warns nothing.
- **Controls are not matched.** 38 of the 78 arms are built from
  `load_champion("evolved")` while the habitual control `advanced` is stock
  `AdvancedAi::new()`. `X vs advanced` therefore measures X *plus* a genome
  worth +61 on `AdvancedAi` and +137 on `BasicAi`. Note this half is
  independent of degradation — `production` loads every artifact it wants and
  is still confounded, because the *control* is the arm carrying less.

The cost is not hypothetical. `ProductionSearchAi` is a **retained negative
result**; against `advanced` it reads +76 and **passes the promotion gate**,
and against a genome-matched control it is 45.8%/−29, reproducing the
repository's own recorded 45.0%. The wrong control alone promotes a known-bad
agent.

### Why the existing test did not catch it

`a_bare_checkout_reports_the_agent_that_actually_plays` (`src/elo.rs:2909`)
pins the effective-name table. It passes because it resolves against a
temporary directory, and `embedded_champion` deliberately fires *only* for
`"evolved"`. **The test exercises a configuration production can never be in.**
It is a correct test of a world that ceased to exist when the genome was
embedded, and it will keep passing no matter how far the mapping drifts from
what `builtin_ai` does.

### The fix

Give the netless names honest effective identities: `policy` →
`advanced_evolved`, `neural` → a new evaluator-only `basic_evolved`
(champion-weight `BasicAi`, which has no name today and is a legitimate control
in its own right — it is the arm that measured +137). Re-point the pinning test
at `ARTIFACT_DIR` so it asserts the resolution production actually performs; if
a bare-directory case is still worth pinning, it belongs in a second test that
says so in its name.

### The root cause fix

**One table, not two.** The two `match` statements must become one, so a name's
construction and its description cannot drift:

```rust
/// What an entrant *is*, on the axes an experiment can vary.
/// Two entrants are the same agent iff their specs are equal.
/// A comparison is controlled iff their specs differ on exactly one axis.
#[derive(PartialEq, Eq, Clone, Debug)]
pub struct AgentSpec {
    pub architecture: Architecture,   // Advanced | Basic | Random | Strategic{..} | Policy{..} | Production
    pub weights:      WeightSource,   // Stock | Champion | Named(&'static str)
    pub evaluator:    EvaluatorSource,// ScoreShare | Net{width} | UntrainedDefaults
    pub treatment:    &'static [&'static str],
}

pub struct Arm { pub spec: AgentSpec, pub build: fn(u64) -> Box<dyn Ai> }
pub fn builtin_arm(name: &str) -> Option<Arm>;   // the single source of truth
```

`builtin_ai` and `builtin_provenance` both become thin readers of `builtin_arm`.
Then:

- **The self-comparison guard compares specs, not strings.** Same spec ⇒ same
  agent ⇒ warn. Different spec ⇒ no warning. Both current failures invert.
- **`ai_eval` can name the axes under test.** Print the symmetric difference of
  the two specs — `arms differ on: architecture, weights` — and **refuse a
  multi-axis comparison** unless the caller passes `--deployment-comparison`,
  which is the legitimate “should this replace stock?” question and is not an
  attribution of the effect to any one component.

### The test that locks it

One cheap, decisive invariant, and it is the one the doc comment already
promises:

> **An agent must play identically to the agent its provenance says it plays as.**

```rust
#[test]
fn every_name_plays_as_the_agent_its_provenance_claims() {
    for name in BUILTIN_AIS.iter().chain(EVAL_ONLY_AIS) {
        let claimed = builtin_provenance(name, ARTIFACT_DIR).effective;
        assert_eq!(
            short_game_fingerprint(name),
            short_game_fingerprint(claimed),
            "{name} does not play as {claimed}",
        );
    }
}
```

Two seeded 30-turn games per name and a hash of the terminal diagnostics. It
runs in seconds, it resolves `ARTIFACT_DIR` the way production does, and it
fails immediately on today's tree for `policy` and `neural`. Add the dual —
distinct specs must *not* fingerprint-collide — and the `policy` ==
`advanced_evolved` false negative is closed too.

---

## 3. R2 — degradation succeeds by default

### The defect

`builtin_ai` returns `Box<dyn Ai>` unconditionally. A name that promises a
learned model returns a scripted agent when the model is absent, and the
caller cannot tell without consulting a *parallel, advisory* channel it is free
to ignore. `ai_eval` exits non-zero only when asked (`--require-artifacts`
→ exit 3); by default it prints a warning and produces a number anyway.

No `valuenet.json` is tracked at either tier `ValueNet::load` searches, so
`NeuralAi`, `PolicyAi` and the learned half of `StrategicAi` degrade in every
checkout on every machine. Their deployed impact is exactly zero.

### Why this keeps happening

This is the **fourth** repair of the same class: `#469`/`#471` for the champion
genome (worth +49 Elo when fixed), `#490` for the league roster, `#635` for the
value-net path, and now the effective-name mapping that the first of those
invalidated. Each repair fixed a **path**. None changed the **contract**, which
is that silent substitution is legal.

### The fix

Ship a net or do not offer the names. Three trained nets exist locally — two
25-wide (`evolve::features`) and one 34-wide (`decision_features`), which
`load_width` correctly refuses to each other — and the best-documented has test
BCE 0.4058 against a 0.5636 constant baseline, accuracy 0.809, ECE 0.035.
Committing one is a strength question needing its own paired run at the
deployment profile, and calibration is not a licence for an argmax: the 34-wide
net was calibrated *and* measured −313. Until one ships, `neural`, `policy` and
the learned `strategic` should not be selectable without an explicit opt-in.

### The root cause fix

**Make artifact-dependent construction fallible, and strict by default.**

```rust
pub fn builtin_ai(name: &str, seed: u64) -> Result<Box<dyn Ai>, Degraded>;
pub fn builtin_ai_degraded(name: &str, seed: u64) -> Box<dyn Ai>;  // explicit opt-in
```

A caller that wants a game to start regardless — the exhibition supervisor,
a soak — calls the second and says so at the call site. Every *evaluation* path
calls the first, so `--require-artifacts` stops being a flag anyone has to
remember: strictness is the default and `--allow-degraded` is the escape.

The general rule, stated once so a fifth instance does not have to rediscover
it:

> **A name that promises a trained artifact must fail rather than silently
> deliver an untrained agent. If a fallback is wanted, it gets its own name.**

`basic_evolved` and `advanced_evolved` already demonstrate the pattern: the
fallback *is* a legitimate agent, so give it its own name and let the learned
name fail honestly.

---

## 4. R3 — a gate is being used as an estimator

### The defect

Effect sizes in this repository are the point estimates of the runs that
promoted them, and they do not replicate at size:

| claim | recorded | re-measured, disjoint seed, same profile |
|---|---|---|
| `advanced` v `advanced_v1`, 6p 74×46 Online | 76.7%, **+207**, PASS | 62.1%, **+86**, PASS |
| `advanced` v `advanced_v1`, 4p compact | +114 | +98 |
| `strategic_deep` v `strategic` | **+45**, PASS | −8 (220 maps, PR #482); and the comparison is confounded |
| `advanced_evolved` v `advanced` | 58.3%, +58 | 58.8%, +61 |

The direction and the significance replicate. The **size** does not, and it
fails in one direction: downward.

### The root cause

That asymmetry is the tell, and it is not bad luck. A promotion gate accepts
when the observed effect is large enough to clear parity. Conditioning on
“passed the gate” therefore conditions on the estimate being *large*, so

> E[observed effect | gate PASS] > true effect.

This is the winner's curse, and **every headline number in the repository is
conditioned on having passed a gate**, so every one is inflated by an amount
that grows as the true effect shrinks and as the run gets shorter. `+207`
against `+86` is exactly the signature. The gate is doing its job — the
decision to keep `advanced` was correct both times. What is wrong is reusing
the decision statistic as a description.

Note the repository's own discipline already covers the *direction* of this
(“confirm on a seed the result was not found on”) and it is honour-system, not
enforced, and never applied to sizes.

### The fix

Restate the affected numbers with the discovery estimate marked as biased, and
quote confirmation-run estimates where they exist. The audit's re-measurements
give confirmations for the four rows above.

### The root cause fix

**Separate the decision from the estimate, in the tool.**

1. `ai_eval` labels a gate-passing effect size at its true epistemic status:

   ```
   promotion gate: PASS — clears parity after 120 maps
   effect size:    +207 (DISCOVERY ESTIMATE — selected on passing, biased upward;
                          not quotable until confirmed on a disjoint seed)
   ```

2. A `--confirm <prior-seed>` mode that requires a disjoint seed, reports the
   confirmation estimate and the pooled estimate, and marks *those* quotable.
3. A documentation rule with a mechanical check: **no effect size enters
   `docs/` or `README.md` without a confirmation run recorded beside it.** The
   same `tools/civvis_collab.py check-pr` gate that already validates ownership
   can grep added lines for an Elo-equivalent figure and require an
   accompanying seed pair.

This costs one extra run per promoted change and buys numbers that mean what
they say.

---

## 5. R4 — a display artifact bound to neither source nor semantics

### The defect

The README's per-civilization table ranks strategies by the league's
**placement** Glicko. The league's own selection contract abandoned placement
and orders parents, retirement and live seating by conservative outright-win
bounds — precisely because placement compressed who won. Over the 14 active
strategies with 400+ games the two orderings agree at Spearman ρ = 0.31.
Ranked by placement, `basic` — the city-state and barbarian controller — places
8th of 14, ahead of five strategies bred to beat it. The table's own top pick,
`winbred-1`, wins 15.8% of its league games against stock `advanced`'s 21.5%.

Separately, no row is distinguishable: Fisher exact on each pair's top two
gives **0 of 52** at p < 0.05, smallest p = 0.088, before correcting for 52
tests. And the table is not reproducible from the repository at all — its
source is gitignored, the committed snapshot covers 4 of 50 pairs, and the
documented refresh command exits 2 on a fresh clone.

### The root cause

The league type exposes two orderings as equally available fields, and the
display layer picked the one that is a single float per pair. Nothing marks
`rating` as “matchmaking only”, and nothing marks the win bound as “the
strength ordering”. A convenience decision in a rendering script silently
became the project's public definition of “best”.

### The root cause fix

1. **One strength accessor.** `Strategy::strength_bound()` becomes the only
   public ordering key for any “which is better” question; `rating` is
   documented and, where possible, made private or renamed
   `matchmaking_rating`. Ranking code then cannot reach the wrong statistic.
2. **A generated table must be reproducible from the repository or must not
   ship.** Either commit a league snapshot with enough coverage to regenerate
   it, or have `update_readme_rankings.py` refuse to write a table it cannot
   rebuild from a tracked source.
3. **Apply the table's own bar honestly.** Print a row only where the leading
   strategy's win bound actually separates from the field. The honest
   replacement for the rest is a coverage table — games per pair, and what
   would be needed to resolve it.

### What switching the statistic actually changes

Recomputed over the 52 qualifying pairs of round 3143, ranking by the
conservative outright-win bound instead of placement:

- the two statistics **name a different strategy in 26 of 52 pairs** — the
  agreement rate is a coin flip;
- **44 of 52** printed leaders do not beat the pooled win rate of the very
  field they are being ranked against.

One row shows the whole failure. For Cleopatra the placement table prints
`deck-legacy`, which has won **8 of 43** games with that pair (18.6%); the win
bound prints `g28-28`, which has won **230 of 622** (37.0%). The shipped table
names a strategy that wins at half the rate, on a twentieth of the evidence,
and calls it Egypt's best.

Switching the statistic is therefore not a cosmetic change — it is most of the
table. That it changes half the rows is also the clearest possible argument for
fix (1): as long as two orderings are equally reachable, a rendering script
will keep reaching for whichever is one float per pair.

---

## 6. The one gap that is not an evaluator defect

**No searching agent has ever played a live game.** Loading a league
force-marks `strategic` as `anchor` and `league_only` (`src/league.rs:1701`),
and live seating filters `!league_only` (`src/server.rs:2931`). The exclusion
is defensible on cost — a searching seat measured ~6.4× the game-turn cost of
an all-scripted six-seat fleet — but it was made once and has no review
condition, so the only component with a plausible route to superhuman play is
permanently outside the thing being optimized.

For an engine whose purpose is winning, this is the most consequential item in
the audit even though it is not a bug.

**The complete course of action:**

1. Measure `strategic` against a genome-matched control at the deployment
   profile, 120+ pairs. The audit measured +61 (CI −1..+124, direction
   p = 0.0003, gate INCONCLUSIVE) at 4p compact; the deployment answer is
   unknown and is the number that decides this.
2. Replace the `league_only` bool with a reason and a review condition, so an
   exclusion carries its own expiry:
   `Exclusion { reason: "6.4x turn cost", revisit_when: "cost < 2x or deployment run > +50" }`.
3. If it wins at deployment, seat it and pay the cost. If it does not, record
   that search does not transfer to the deployment profile — which would be the
   most important negative result in the project, because search is the
   repository's own stated best lever.

---

## 7. The accretion, which is the same root cause wearing different clothes

The audit also found 78 evaluator arms and 44 research binaries behind seven
controllers, with 41 arms having no entry in `docs/EVAL.md` and 13 named
nowhere in `docs/` at all. Most name axes the repository has already closed.

None of it costs anything at runtime, and the instinct to delete it is mostly
wrong — a closed axis with its arm still present is *reproducible*, which is
worth more than tidiness. The real problem is that you cannot tell the
reproducible ones from the abandoned ones.

The mechanism is the same as R1. `EVAL_ONLY_AIS` (`src/elo.rs:41`) is a flat
`[&str; 70]`: an arm is a bare string in an array, so nothing can require it to
say why it exists, what it measured, or whether that question is still open. An
experiment arm has **no lifecycle** — it is added for one run and lives
forever.

**The fix** is the same shape as the R1 fix, and falls out of it for free once
arms are structured rather than stringly-typed: give each arm the evidence that
justifies it.

```rust
pub struct EvalArm {
    pub name: &'static str,
    pub evidence: &'static str,   // e.g. "docs/EVAL.md#2026-07-26-strategic-deep"
    pub status: ArmStatus,        // Open | Closed { verdict: &'static str }
}
```

with one CI check: **every eval-only arm names a document section that
exists.** An arm whose axis is closed keeps its name and carries its verdict,
so the next agent finds the negative result instead of rediscovering it — which
is the failure mode this repository loses whole iterations to, and which
`docs/EVAL.md` exists to prevent but cannot when the arm and the record are not
linked.

## 8. Order of work

Ordered by damage prevented per unit cost. The first two are prerequisites for
trusting anything measured afterwards.

| # | change | closes | cost |
|---|---|---|---|
| 1 | Single arm table (`AgentSpec` + `builtin_arm`), spec-based collapse check, the plays-as-it-claims test | R1 — both defects | ~1 day, `src/elo.rs` |
| 2 | Fallible strict `builtin_ai`; explicit degraded entry point | R2 | ~half a day |
| 3 | Discovery-vs-confirmed effect sizes in `ai_eval`; the docs rule | R3 | ~half a day |
| 4 | `strength_bound()` as the only ordering; table reproducible or gone | R4 | ~half a day |
| 5 | Arm lifecycle: `EvalArm` with evidence + status, CI check that the section exists | §7 | falls out of 1 |
| 6 | Restate affected published numbers against matched controls | fallout of 1 and 3 | compute |
| 7 | Deployment-profile decision on seating search | §6 | compute, largest |

Items 1–4 are code and are independent of each other. Item 5 cannot be trusted
before item 1 lands, because it is exactly the measurement item 1 makes
possible.

## 9. What this is worth

Items 1–3 do not make the AI stronger by a single Elo point. They make the
number that says so trustworthy — and on current evidence the instrument has
already promoted one retained negative result and overstated the project's
flagship improvement by more than a factor of two. An engine optimized for
winning is only as good as its ability to tell which of two agents wins, and
that is the capability these defects take away.
