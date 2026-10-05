# Rejected productive Builder escort

The opt-in escort prototype improved Emperor production but reduced Deity
production on the assigned consumed diagnostic maps. Remove the prototype;
retain existing civilian coordination unchanged. No fresh pilot or confirmation
was played and no production default or private live controller was changed.

## Treatment

Runtime checkpoint `37f669c9157b8185ab77e965cdb9e5aa65422597` reserves at most
one additional healthy ground guard per frame for an owned, currently worked
land improvement inside raider reach. The improvement must add net Production
without losing Food. Existing emergency Builder support and Settler reservations
keep priority. Recovery, threatened cities, final own-city garrisons, wounded
guards, and guards that cannot survive the existing expected-damage bar are
excluded.

A speculative clone must accept both units' walks to the same destination.
It includes an immediate legal improvement when movement permits. If the walk
spends the Builder's last movement, a separate native model copy validates
structural improvement legality with a fresh allowance; the real board executes
only legal walks and the next frame revalidates the operation. No movement is
granted on the real board. The guard survival bar is rechecked after any actual
planned improvement. Existing no-job emergency pairs retain their behavior.

Both arms use public `targeting(Domination)` and `enable_live_bridge()`, unchanged
public weights, stock adaptive rivals, four majors, six city states, 60×38
Pangaea, Online speed, Gran Colombia focal, and a 150-turn cap. Only the focal
seat is exempt from difficulty yield bonuses. The candidate alone enables the
new flag, and setup files read it back. These are native simulations, with no
Firaxis strength or host-execution verification.

## Completed diagnostic and rejection

Four consumed Emperor seeds 61008600–603 and four consumed Deity seeds
61008700–703 were played in both arms. All 16 executions completed with exit
zero before analysis. Binary, normal optimized library and dependencies, source,
compiler, probe, protocol and analyzer were frozen before play. All eight
controls reproduce the preceding citizen-growth controls' actions and final
worlds byte for byte. Paired setup weights, handicaps and deployment tags match.

Turn-75 averages include every assigned seed, scoring death or an unreached
checkpoint as zero:

| Difficulty | Production control → candidate | Change | Science control → candidate | T75 survivors |
| --- | ---: | ---: | ---: | ---: |
| Emperor | 53.675 → 58.900 | +9.73% | 27.563 → 30.900 | 4 → 4 |
| Deity | 37.238 → 35.050 | −5.87% | 22.306 → 20.366 | 4 → 4 |

Emperor cumulative Production through T75 increased 3.94%; Deity declined
2.95%. Final survival stayed 4/4 Emperor and 2/4 Deity. Science clears the 90%
guard on both difficulties, but Deity fails the positive Production gate.

There are 27 accepted improvement actions matching reserved Builder/job plans;
25 also show the designated guard at the destination in the next world-turn
snapshot. That snapshot is after opponents act and does not directly prove
every guard's position at the instant of improvement. Repeated plans are
deduplicated by turn, Builder and improvement. Plans alone are not completed
work. The first Emperor game visibly walks both units together at T47 and
completes its pasture at T48 with the guard retained there.

Two Deity maps lose T75 Production: 61008701 goes 51.4 → 44.5 with cities 7 → 5,
and 61008702 goes 31.15 → 29.3 with cities 5 → 4. The other two are unchanged
at T75. These outcomes show an empire opportunity cost alongside completed
local work; they do not establish which displaced military or settlement order
caused it. No fresh sample is justified by this failed diagnostic.

## Validation and reproduction

The corrected feedback suite passes all 17 civilian-coordination tests, with
4,356 filtered out, using opt-level zero only for fixtures. It covers immediate
improvement, a real native turn boundary after a roadless hill walk, full unit
driver guard reservation, Settler/wounded-guard refusal, final garrison retention,
and invalidated-pair atomicity. Append-point policy passes all 14 tests.
The separate normal `cargo build --profile ci --locked --lib` completed
successfully; standalone game probe compilation uses opt-level three. Earlier
failed fixture/type attempts remain recorded in retained validation logs and
are not passing evidence.

The final tree restores all three native files to task base
`3dd8b24332be223c14ee1020122a895d33405c91`. After one current-main merge, the final native/Cargo/data tree matches both
main and the completed #3943 normal full-suite source exactly: 4,591 passed,
zero failed, 54 ignored. This reuses source-equivalent evidence rather than
claiming a new full Cargo run. The only changed standalone probe passes
formatting; the final PR checks are pending.
The tracked probe deliberately calls the removed experimental API: reproduce it
with checkpoint `37f669c91` or the frozen experimental library, rather than the
restored current runtime. The adjacent manifest, protocol, results and coverage
audit record hashes, settings, all seed outcomes and the decision. Raw binary,
dependencies, accepted actions, frames, journals and saves are retained outside
the worktree in the production evidence directory.
