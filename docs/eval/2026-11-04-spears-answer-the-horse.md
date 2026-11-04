# A timely spear for a visible mounted threat

`spears-answer-the-horse` is an opt-in, default-OFF refinement of the existing
emergency production handoff. It creates no additional unit demand. A city
already asking for a non-ranged land defender may choose legal anti-cavalry
when its worst local class matchup improves, with no increase in the host's
quoted completion time, production cost, base upkeep or strategic resource demand.

## Native development case

Native `civvis-20261104T061724Z` ran Standard-speed Immortal Gran Colombia,
Continents map, Small size, six players. At turn 106/frame 0, Bogotá had full
city health and walls but a visible Sumerian Knight (50 strength) and Catapult
nearby. The baseline emitted `UNIT_HEAVY_CHARIOT` for city 65536. Its raw menu
also offered Spearman: both cost 33 with zero saved progress; Spearman took
three turns and Heavy Chariot four. The raw city's Battering Ram had 29 paid
production; the existing baseline already reclaimed that queue for defense.
This treatment refines the defender choice, rather than newly interrupting
that paid siege build.

Raw strength ranks Chariot 28 above Spearman 25. Against the Knight, the spear's
class bonus makes that 35: still outmatched by 50, but a better class margin
than 28 and available one turn earlier at the same observed cost. This is the
development motivation, not a survival or win-rate result.

The installed game supports those class rules: shipped
`Base/Assets/Gameplay/Data/UnitAbilities.xml:1382` defines
`ANTI_CAVALRY_COMBAT_BONUS`, `Amount=10`. Its opponent requirements include
heavy, light and ranged cavalry tags. `DebugGameplay.sqlite` records
`ANTI_SPEAR`, `Amount=5`, for the melee-versus-anti-cavalry modifier. The
Sumerian War Cart has the heavy-cavalry promotion class but lacks the qualifying
cavalry tag; its immunity is retained. The archived rules excerpt contains the
queries' rows, XML excerpts and XML hash. No game combat rule is changed.

## Policy boundaries

The existing pass must first establish its own emergency and select a land
defender. Walls, repairs and an existing ranged defender/selection keep their
priority. A paid or one-turn defensive queue is retained. When a fresh defensive
head would otherwise be retained, compare against that exact head's budget,
rather than a stronger uncommitted fallback.

The candidate considers visible, non-embarked hostile melee-capable land units
with a route to the city's approaches within six steps. At least one must be
qualifying cavalry. It scores both sides' class modifiers against every such
contact and requires a strictly better worst margin. A stronger mixed melee
force can therefore reject the spear. This is a class comparison, not a complete
combat forecast; it does not score ranged-only attackers or infer hidden troops.
Known positive host time and cost quotes are required. Ordinary standalone
simulator games without those menus cannot activate this policy; strength
measurement must use native games. Unknown quotes, increased
base upkeep, or a new strategic material requirement preserve the baseline.

Existing one-turn Campus-building and paused-district releases are evaluated
before the refinement. A change to an already defensive fresh head ranks behind
all existing unsafe-queue rescues, then retains the same damage/city priority
among defensive reservations. At most one city's production is changed by the
pass, and its returned claim retains authority through later production writers.

## Repeated captured-history qualification

Qualified source: `f43dbc036b7f2e93379906c5e87ddc2f56fe6c71`.
Native candidate SHA-256:
`deea2177befbdc7426f45981cb1f7e205d4e8bcc385280852c118773c50dcdaa`.
The candidate used the repository's unmodified optimized `ci` profile; the
separate local compiler experiment was not used in this proof.

| Check | Result |
| --- | --- |
| OFF native controls | Two 425-request runs through turn 172/frame 1 match complete orders/decision payloads and all ordered input reads |
| ON captured prefixes | Both first differ at turn 106/frame 0, request 250; earlier order lists match |
| Successful persistent paired queries | Two OFF/ON pairs each consume the 250-request prefix, match their corresponding complete reply prefixes and actual reads, and exit 0 on EOF |
| Independent verifier | Reconstructs input byte versions/count/order, verifies receipt/source hashes, checks only the named argument/genome bit differs, and requires the same-city HeavyChariot-to-Spearman production change |

At the first difference, **one Heavy Chariot production order is removed and
one Spearman production order is added for city 65536**. No other order is
added or removed, and the other orders retain their sequence. This is an intended production change, unlike the rejected
Campus prototype's order permutation in #4413.

These are qualified proposals on development inputs. No divergent world is
continued after the first change; the host did not execute these replay orders.
No earlier defender completion, saved city, science gain or stronger play is
established. The flag remains off pending separate native screening.

## Validation and integration

Eight focused regressions pass: actual handoff/withholding and later claim
retention; paid/one-turn protection; host legality/cost/time boundaries;
War Cart and stronger mixed melee; fortification/ranged selection and unarmed
controller protection; competing unsafe-city priority; the existing one-turn
Library release; and local hostile-contact requirements. Formatting, 14 append
point regressions, gene metadata and evaluation metadata checks pass.

Main was merged before publication. The relevant production and regression
source hashes still match the qualified source; upstream added the Campus audit
and an unrelated air-resource scan optimization. The full local `ci` suite was
started on the qualified source before that merge and passed 6,828 tests
(53 skipped, plus four ignored doctests). It is not an exact final-head claim.
The integrated `cargo check --locked` passes; required CI must validate the
final integrated tree separately.

## Evidence

[`2026-11-04-spears-answer-the-horse.native.tar.xz`](2026-11-04-spears-answer-the-horse.native.tar.xz)
contains raw native requests, sealed input bytes/indexes, all comparison replies,
actual successful-query receipts and relocation maps, source snapshots, drivers,
verifier, native quote/rules excerpts and focused build/test logs. Executables
and event FIFOs are excluded. The archived README explains verification and
host-path prerequisites. No strength screen or activation waiver is submitted.

Archive: 2,977,612 bytes; SHA-256
`2949e5ff7bba47100e3e99de935c0a6b11ab605634f4aa41794a365239e65b0f`.
