//! Oracle ablation: find which subsystem actually limits the agent.
//!
//! Every attempt to strengthen this AI has failed one of two ways. The first
//! is documented in `docs/AI_GAPS.md`: the evaluator could not see the
//! decision it was ranking, so no amount of training could rank it. The
//! second is the same error one level up — the change addressed a constraint
//! that was not binding. `advanced_relief_scoped` is the worked example: it
//! cut the force groups frozen far from an emergency from 19.0% of
//! force-group turns to 10.4%, exactly as designed, and measured
//! Elo-equivalent -6 over 120 mirrored maps.
//!
//! Both mistakes are avoidable by asking a cheaper question first. For a
//! subsystem S, what would this agent's win rate be if S simply could not
//! fail? The gap between the stock agent and the S-oracle is the *headroom*
//! in S: an upper bound on everything any amount of work on S could ever be
//! worth. A subsystem whose oracle wins nothing is a settled question, and
//! settling one costs a batch of games instead of a design, an
//! implementation and a pre-registered run.
//!
//! These grants cheat, deliberately and visibly. They are diagnostics, never
//! entrants: [`Oracle`] is constructed only by `src/bin/ablate.rs`, and
//! nothing in `elo.rs` can name one, so an oracle result can never be
//! recorded as an agent's rating.
//!
//! Each grant is applied at the start of the seat's turn, before the wrapped
//! agent plays, because `AdvancedAi::take_turn` ends its own turn.
use crate::name::Name;
use crate::ai::{Ai, PlanReport};
use std::collections::BTreeSet;
use crate::game::{Game, Item};
use crate::world::DistrictFoundation;
use crate::Pos;

/// The engine's workable ring. `plot_purchase_cost` prices rings one through
/// three and nothing beyond, so this is exactly the ground a citizen could
/// ever be assigned to.
const CITY_WORK_RADIUS: usize = 3;

/// The city count the expansion grant stops at — the same six
/// `StrategicPlan::desired_cities` aims at, so the grant removes the cost of
/// the agent's own appetite rather than inventing a larger one.
/// `pub` so `src/bin/rebate_census.rs` can report when a seat stopped being
/// eligible for a payout, which is the length of the window both grants act in.
pub const EXPANSION_TARGET: usize = 6;

/// The city an expansion grant would pay out of, or `None` when this seat is
/// not short of its city target or already has a Settler in flight.
///
/// Shared by [`Grant::Expansion`] and its cost-matched control
/// [`Grant::Rebate`] so the two cannot fire on different schedules. That is
/// not tidiness: the control is only a control if the two grants are handed
/// out at the same moments, and two copies of this condition would be free to
/// drift apart and quietly turn the comparison into a comparison of firing
/// rates.
///
/// `pub` because `src/bin/rebate_census.rs` has to recognise a payment turn
/// from outside the crate in order to record what the payout city was
/// building when the money landed — the observation that says whether a
/// rebate buys a city or something else.
///
/// The capital is taken by lowest id so both grants are deterministic.
pub fn expansion_payout_city(g: &Game, pid: usize) -> Option<u32> {
    let cities = g.player_city_ids(pid);
    if cities.is_empty() || cities.len() >= EXPANSION_TARGET {
        return None;
    }
    let already_walking = g
        .player_unit_ids(pid)
        .into_iter()
        .any(|uid| g.units.get(&uid).is_some_and(|unit| unit.kind == "settler"));
    if already_walking {
        return None;
    }
    cities.iter().copied().min()
}

/// Turns of Gold income a seat may keep under `Grant::IdleReserve`. Generous by
/// design: the deployment map's median holding is 9.4 turns, so this leaves the
/// typical reserve untouched and takes only what sits above it.
const IDLE_RESERVE_TURNS: f64 = 10.0;

/// The engine's floor for suzerainty: `suzerain_of_uncached` requires at least
/// this many envoys as well as a strict lead over every other major.
const SUZERAIN_ENVOYS: i64 = 3;

/// A capability granted to the wrapped agent for free.
///
/// Each one is chosen to bound one measured failure, so a null result closes
/// that question rather than leaving it open.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Grant {
    /// Nothing. The control: an ablation run must measure this at parity, or
    /// the harness is reporting its own noise as headroom.
    None,
    /// Every unit that has an upgrade takes it, free, every turn.
    ///
    /// Bounds the standing-army problem. Measured over 48 six-player games,
    /// 81% of the 4,912 military units alive at the end were three or more
    /// eras behind the world era, and 15% had a researched upgrade they had
    /// never taken. This asks what perfect modernization would be worth
    /// without asking anyone to pay for it.
    Modernity,
    /// Whenever an enemy city is standing open, put a melee unit next to it.
    ///
    /// Bounds the siege-conversion problem. Over the same games the AI left
    /// 210 cities at zero garrison and had a melee unit adjacent with
    /// movement in hand on 46 of them — 22%. The engine's own note says the
    /// two candidate explanations are "declines a capture it could make" and
    /// "never has anyone there to make it". This grant removes the second
    /// one by teleporting the nearest melee unit into position, so what the
    /// win rate does next distinguishes them.
    Taker,
    /// Every unit starts every turn at full health.
    ///
    /// Bounds combat micro — `AI_GAPS.md` item 4, which its own re-sequencing
    /// leaves explicitly unmeasured: "treat it as unknown rather than cheap".
    /// Retreat-and-heal cycling, refusing an unfavourable trade, and pulling a
    /// wounded unit out before it is killed all cash out as the same thing:
    /// health a better player would still have. This grants the outcome
    /// without granting the skill, so the win rate says what the skill is
    /// worth at most.
    ///
    /// It does not make units immortal. A blow large enough to kill still
    /// kills; what disappears is accumulated damage.
    Attrition,
    /// A large, unearned pile of Gold and Faith every turn.
    ///
    /// Not a subsystem. This is the instrument's calibration: it grants an
    /// advantage nobody would argue is small, so a run can establish that the
    /// harness detects an advantage at all. Without it a null from any other
    /// grant is ambiguous between "this subsystem does not limit the agent"
    /// and "this design cannot resolve anything", and those call for opposite
    /// next steps.
    ///
    /// Deliberately crude and deliberately huge — a stock empire finishes
    /// these games with a few hundred Gold, so this is worth orders of
    /// magnitude more than any honest improvement to any subsystem. If it
    /// does not register, nothing else measured here means anything.
    Treasury,
    /// Every city instantly owns every unclaimed tile inside its workable
    /// three-ring radius.
    ///
    /// Bounds the *ceiling* that #532's saturation result was conditional on.
    /// `city_decision_census` measured the citizen governor claiming 89.3% of
    /// its city's food ceiling and 99.5% of its production ceiling — but that
    /// ceiling is computed over the tiles the city **already owns**, so a
    /// governor allocating 89% of a poor endowment still reads as saturated.
    /// Five scripted city-strategy arms measured null against that ceiling;
    /// this asks whether the ceiling itself is the thing that binds.
    ///
    /// Border growth is paid for in accumulated Culture and plot purchase in
    /// Gold, and both are slow. This grants the outcome — the ground — without
    /// granting the Culture or the Gold, so the win rate says what perfect
    /// territorial acquisition is worth at most.
    ///
    /// It deliberately never takes a tile another city already owns. That
    /// would grant conquest and the measured headroom would belong to the
    /// bundle rather than to border growth.
    Ground,
    /// Every district under construction is re-sited onto the legal tile in
    /// its own city where it would yield most.
    ///
    /// Bounds district siting — the last city decision nothing had measured.
    /// #532 bounded what a city *works* (89.3% of its food ceiling, 99.5% of
    /// its production ceiling) and #534 bounded what it *owns* (perfect border
    /// growth, p=0.7283). Neither says anything about where a district goes,
    /// and adjacency is the whole reason district placement is a decision at
    /// all: the same Campus is worth several times more beside mountains than
    /// on open flatland.
    ///
    /// It re-sites **foundations**, not finished districts. A foundation is
    /// exactly the decision under test — the moment the site is chosen — and
    /// moving one cannot disturb a completed district's buildings, specialists
    /// or defenses.
    ///
    /// The candidate set comes from the engine's own `district_sites` rather
    /// than from a reimplemented legality check. That filter is long (flooding,
    /// natural wonders, non-bonus resources, feature-removal techs, national
    /// parks, Vietnam's specialty rule, family and specialty-capacity limits)
    /// and any private copy of it would drift and quietly over-grant. The
    /// foundation is lifted first so the engine will enumerate alternatives —
    /// and its own tile — instead of reporting the site as taken.
    Siting,
    /// A free Settler whenever the seat is below its city target and has none
    /// in flight.
    ///
    /// Bounds **expansion**, which every other city measurement is conditional
    /// on. #532/#534/#542/#553 bound what a city works, owns, builds districts
    /// on and stands on — all *per city*, and all saturated or null. None of
    /// them asks whether the empire has enough cities in the first place, and
    /// the evals say it does not: seats finish four-player games holding
    /// **2.1 to 2.8** cities while `StrategicPlan::desired_cities` targets six.
    ///
    /// A settler costs production and a point of population, and needs
    /// `pop >= 2` to build at all, so expansion is paid for out of exactly the
    /// early economy that is also paying for everything else. This grants the
    /// settler without the cost, so the win rate says what perfect expansion
    /// tempo is worth at most.
    ///
    /// It grants only the unit. Where to settle is still the agent's decision,
    /// walked by its own settler logic — which #553 measured taking 99.9% of
    /// the value on offer — so this is a bound on expansion *rate* and not on
    /// siting.
    Expansion,
    /// What a Settler *costs*, handed to the capital on exactly the schedule
    /// [`Grant::Expansion`] hands over a Settler — and no Settler.
    ///
    /// This is the control [`Grant::Expansion`] never had, and the whole
    /// expansion programme rests on it. Expansion measured 23.0% → 52.3%
    /// (400 maps, p=0.0000) and is the only grant here that has ever returned
    /// headroom, so the repo has spent a dozen pull requests on the expansion
    /// pipeline. But that grant is not free: it fires ~5.6 times a game and
    /// each firing is worth a Settler's production *plus* the point of
    /// population `Game::finish_unit` charges for one. Nothing so far
    /// separates "this empire needed cities" from "this empire needed
    /// resources", and every honest treatment on the pipeline — parallel
    /// settlers, capital food, destination commitment, production preemption,
    /// a settler priced at 100x, the map-capacity ceiling — has measured null
    /// or inert. A bundle worth thirty points whose every component is worth
    /// nothing is exactly what a mis-attributed grant looks like.
    ///
    /// So this pays the same price on the same schedule into the same city and
    /// buys nothing in particular: banked production the empire may spend on
    /// whatever its own governor ranks first, and the population back.
    ///
    /// The two outcomes call for opposite next steps, which is the point:
    ///
    /// - **Rebate also wins.** The headroom is generic early economy, not
    ///   expansion, and the expansion programme is aimed at a symptom. The
    ///   agent is then free to buy a Settler with the rebate and declines to —
    ///   which relocates the question from what expansion costs to what the
    ///   production ranking is doing with an unconstrained budget.
    /// - **Rebate is null while Expansion wins.** Expansion is genuinely
    ///   special: cities are worth more than the resources that buy them, the
    ///   grant is measuring the thing it names, and the pipeline work is
    ///   correctly aimed.
    ///
    /// Deliberately *not* cost-matched by handing over Gold. Gold would route
    /// the grant through unit purchasing, a separate subsystem with its own
    /// failure modes, and a null could then belong to either. `city.production`
    /// is the field ordinary production accumulates into, so this credits the
    /// empire in the currency the settler would have been paid for in.
    Rebate,
    /// [`Grant::Expansion`], restricted to Settlers the agent's **own** plan
    /// was already asking for.
    ///
    /// Half of a partition. `Expansion` pays while the seat holds fewer than
    /// the hardcoded [`EXPANSION_TARGET`], but the agent has a target of its
    /// own — `StrategicPlan::desired_cities`, which ramps as
    /// `(3 + turn/standard_duration(90)).min(map_capacity).min(6)` and so asks
    /// for three cities through the whole opening. `rebate_census` measured
    /// the stock agent **already holding as many cities as its own plan asked
    /// for on 47.9% of the turns the grant fires**, so a large share of what
    /// the grant buys is cities the agent had not decided to want.
    ///
    /// That is the one split that reconciles the two results on this axis.
    /// Raising the target is null (`advanced_wide_opening`, 49.6% over 240
    /// pairs, #588) and so is paying the settler's price ([`Grant::Rebate`],
    /// +0.45 cities against the grant's +3.05) — yet the grant itself is worth
    /// 23.0% to 52.3%. Splitting it says which half of its work carries that:
    ///
    /// - **`Wanted` carries it** — the agent's target is fine and it simply
    ///   cannot execute against it. Price is already excluded, so what is left
    ///   is the pipeline: empire-wide serialization, the `pop >= 2` floor, and
    ///   transit.
    /// - **`Beyond` carries it** — the target is the constraint after all, and
    ///   raising it measured null only because the pipeline could not deliver
    ///   the extra cities. That is the combination #588 declined to run.
    ///
    /// A seat that has not yet assessed a plan is counted here: it holds its
    /// capital alone and every target the ramp can produce is at least three,
    /// so it is unambiguously short of whatever it is about to decide. That
    /// also keeps the partition exact — every turn `Expansion` would fire is
    /// claimed by exactly one of the two.
    ExpansionWanted,
    /// [`Grant::Expansion`], restricted to Settlers **past** what the agent's
    /// own plan asked for. The other half of [`Grant::ExpansionWanted`]'s
    /// partition; the rationale is written there.
    ExpansionBeyond,
    /// A Settler at the head of the payout city's queue — and the empire pays
    /// for it in full.
    ///
    /// The third member of a decomposition that now spans [`Grant::Expansion`]:
    ///
    /// | grant | the decision | the cost | the wait |
    /// |---|---|---|---|
    /// | [`Grant::Rebate`] | agent's | **free** | agent's |
    /// | `ExpansionOrder` | **forced** | agent pays | agent's |
    /// | [`Grant::Expansion`] | **forced** | **free** | **none** |
    ///
    /// `Rebate` gave the cost without the decision and measured null (+0.45
    /// cities against the grant's +3.05, zero extra Settlers trained). This
    /// gives the decision without the cost relief, so the pair brackets which
    /// of the two the grant's 23.0%-to-52.5% actually rests on.
    ///
    /// **It survives the agent's turn**, which is what makes it viable at all:
    /// `advanced_production` skips a city whose queue is non-empty outright at
    /// the shipped `preempt_margin` of 1.0 (*"without preemption a non-empty
    /// queue is skipped outright, so `production_value` is only ever consulted
    /// on an idle city"*), so an order placed before the agent plays is not
    /// re-decided by it. A fires-check asserts that rather than trusting it.
    ///
    /// ⚠ **The `pop >= 2` floor stays in force.** The engine stalls a Settler
    /// at the head of a city below population two — `city.production` keeps
    /// accumulating but the item never completes and everything behind it
    /// waits — so forcing one there would measure a frozen queue and call it a
    /// decision. This grant declines those turns and lets the floor bind.
    ///
    /// Placed through `Action::Produce`, the same path `AdvancedAi` uses, so
    /// the displaced item's progress is banked by `item_progress_key` exactly
    /// as it would be if the agent had switched builds itself.
    ExpansionOrder,
    /// [`Grant::Expansion`] with the walk taken out: the same free Settler, plus
    /// enough movement each turn to reach wherever the agent sent it.
    ///
    /// The last unmeasured candidate in the `wanted` half. `Grant::Expansion`
    /// hands over the unit and **still pays transit**, so transit sits inside
    /// its +30 rather than outside it, and the gap between this grant and that
    /// one is what the walk costs.
    ///
    /// `docs/OPENINGS.md` measured a Settler covering **0.81 tiles a turn
    /// against a shipped `moves` of 2** on 32x22, with 70% of its
    /// standing-still turns holding no destination, and concluded *"the
    /// design's ceiling is the map"*. But #559 then found "no settle site in
    /// reach" to be a pure 24x16 artifact that never fires once at deployment
    /// density, so that conclusion has never been checked on a roomy map. This
    /// checks it the only way that cannot be argued with: by removing the walk
    /// and seeing what the win rate does.
    ///
    /// **Siting stays the agent's.** Only `moves_left` is topped up; where the
    /// Settler goes is still its own `best_reachable_settle_site`, which #553
    /// measured taking 99.9% of the value on offer. So this bounds transit, not
    /// destination choice — and a Settler that stands still for want of
    /// anywhere to go is unaffected by it, which is exactly the discrimination
    /// wanted: if the 70%-no-destination reading transfers to a roomy map, this
    /// grant is worth little.
    ///
    /// ⚠ **Deliberately NOT budget-matched to [`Grant::Expansion`].** A Settler
    /// that founds sooner frees the `already_walking` slot sooner, so this
    /// fires more often over a game. That is the effect under test rather than
    /// a confound — "what is transit worth" includes "how many more cities
    /// arrive, and sooner" — but it does mean the two arms are not a
    /// cost-matched pair the way [`Grant::Rebate`] is, and the firing counts
    /// must be read as part of the result rather than checked for equality.
    ExpansionSwift,
    /// Confiscate every Gold above ten turns of income, every turn.
    ///
    /// **An ablation, not a grant** — the only one in this enum, and the sign
    /// of its result reads the other way. Everything else here hands a seat a
    /// capability and asks what perfection is worth. This takes a resource away
    /// and asks whether it was doing anything at all.
    ///
    /// `idle_treasury_census` (#590) measures a median seat holding **9 to 16
    /// turns of income** in Gold, with a real tail — a quarter of eval-scale
    /// seat-turns above thirty turns of income, a peak of 4031 against 7.7 a
    /// turn. The obvious question is whether spending that would help, and the
    /// obvious experiment is the wrong one: correlating balances with outcomes
    /// finds that hoarding seats lose and establishes nothing, which is how
    /// three findings in this repository were retracted.
    ///
    /// Confiscation settles it cleanly and in the safe direction. If a seat is
    /// **unharmed** by losing everything above a working buffer, that money was
    /// dead capital and there is real headroom in deploying it. If it is
    /// **hurt**, the reserve is doing a job — insurance against an emergency
    /// purchase is correct play at Civ VI prices — and the axis closes with no
    /// treatment built.
    ///
    /// A null here is therefore an *informative* null either way, which is not
    /// true of most measurements this harness runs.
    IdleReserve,
    /// Suzerain of every city-state this empire has met.
    ///
    /// Bounds the envoy layer, which nothing in `docs/` has measured. It is the
    /// direct analogue of `Taker`: that grant puts the piece where perfect play
    /// would have put it, and this one puts the envoys where perfect play would
    /// have put them. It grants an *outcome*, not a resource — no extra envoys
    /// are created anywhere else, and a rival's own envoys are left untouched.
    ///
    /// The engine's rule is `envoys >= 3` and strictly more than every other
    /// major, so this raises the seat to one above the best rival at each met
    /// city-state and no further. Ties lose under `suzerain_of_uncached`, which
    /// is why it is `+1` rather than a match.
    ///
    /// If this reads large, the envoy layer is high-leverage and worth work,
    /// and `advanced_envoys` is where that work goes. If it reads null, 48
    /// city-states are decoration and the axis closes.
    Suzerain,
    /// Every subsystem grant at once.
    ///
    /// The question the ladder forced. At Prince, `Expansion` is worth **+29.5
    /// points** (23.0% to 52.5%, 95 discordant cells, p=0.0000). At Deity the
    /// same grant on the same maps and seed is worth **+1.0 point** on two
    /// discordant cells — and it is not inert there, it fires **7.2 times a
    /// game against Prince's 5.6**. A perfect subsystem stops mattering once
    /// the opposition is strong.
    ///
    /// One reading is that the seat at Deity is behind on *every* subsystem at
    /// once, so relieving one changes nothing. This tests that directly:
    /// relieve them all. It is the only grant here not trying to isolate
    /// anything — it is an upper bound on the whole modelled agent.
    ///
    /// - **Compound moves Deity off the floor** — the subsystems are jointly
    ///   sufficient and the work is additive, so the collapse at Deity is a
    ///   floor effect rather than a transfer failure.
    /// - **Compound is still ~0 at Deity** — perfecting every subsystem this
    ///   harness models does not beat the handicap, and the rest of the gap is
    ///   in something nobody has instrumented (tactics, timing, diplomacy) or
    ///   in the handicap itself. Either way the roadmap should stop pricing
    ///   work against single-subsystem oracles measured at Prince.
    ///
    /// **Excludes [`Grant::Treasury`]**, the instrument's calibration rather
    /// than a subsystem — folding in an unearned pile of Gold would make a win
    /// unreadable. **Excludes the expansion splits and [`Grant::Rebate`]**,
    /// which are subsets or controls of `Expansion` and would double-count it.
    Compound,
}

impl Grant {
    pub const ALL: [Grant; 16] = [
        Grant::None,
        Grant::Modernity,
        Grant::Taker,
        Grant::Attrition,
        Grant::Treasury,
        Grant::Ground,
        Grant::Siting,
        Grant::Expansion,
        Grant::Rebate,
        Grant::ExpansionWanted,
        Grant::ExpansionBeyond,
        Grant::ExpansionOrder,
        Grant::ExpansionSwift,
        Grant::IdleReserve,
        Grant::Suzerain,
        Grant::Compound,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Grant::None => "none",
            Grant::Modernity => "modernity",
            Grant::Taker => "taker",
            Grant::Attrition => "attrition",
            Grant::Treasury => "treasury",
            Grant::Ground => "ground",
            Grant::Siting => "siting",
            Grant::Expansion => "expansion",
            Grant::Rebate => "rebate",
            Grant::ExpansionWanted => "expansion_wanted",
            Grant::ExpansionBeyond => "expansion_beyond",
            Grant::ExpansionOrder => "expansion_order",
            Grant::ExpansionSwift => "expansion_swift",
            Grant::IdleReserve => "idle_reserve",
            Grant::Suzerain => "suzerain",
            Grant::Compound => "compound",
        }
    }

    pub fn from_id(id: &str) -> Option<Grant> {
        Self::ALL.into_iter().find(|grant| grant.name() == id)
    }
}

/// One wrapped agent plus the capability it is being handed.
pub struct Oracle<A: Ai> {
    inner: A,
    grant: Grant,
    /// How many times the grant actually did something. A grant that never
    /// fires measures the stock agent under another name, which is the exact
    /// failure the provenance work in `elo.rs` exists to prevent — so the
    /// harness reports it rather than letting a null be ambiguous.
    fired: u64,
}

impl<A: Ai> Oracle<A> {
    pub fn new(inner: A, grant: Grant) -> Oracle<A> {
        Oracle {
            inner,
            grant,
            fired: 0,
        }
    }

    /// Times the grant changed the position.
    pub fn fired(&self) -> u64 {
        self.fired
    }

    /// Walk every unit to the top of its unlocked upgrade chain, free, with
    /// none of the frictions a real upgrade pays.
    ///
    /// Deliberately *not* routed through `Action::UpgradeUnit`. That path
    /// requires the unit to be in friendly territory with movement in hand
    /// and outside a zone of control — faithful to Civ 6, and exactly the
    /// friction a field army never satisfies, which is a large part of why
    /// 81% of the standing army finishes three or more eras stale. An oracle
    /// that respected those preconditions would measure "free upgrades for
    /// the garrison" and report a null for the wrong reason: the first
    /// version of this grant did precisely that and fired zero times, which
    /// `the_modernity_grant_actually_fires` caught.
    ///
    /// Strength, range and every other combat property are read from
    /// `rules.units[kind]` at query time, so rewriting `kind` is what
    /// modernization *is* here. HP is a 0..100 condition independent of the
    /// unit type and is deliberately preserved: this grants a better army,
    /// not a healed one.
    fn grant_modernity(&mut self, g: &mut Game, pid: usize) {
        for uid in g.player_unit_ids(pid) {
            // Bounded by the length of an upgrade chain; the guard only stops
            // a cycle in a malformed ruleset.
            for _ in 0..16 {
                let kind = g.units[&uid].kind.clone();
                let Some(target) = g.unit_upgrade_target(pid, &kind) else {
                    break;
                };
                if target == kind {
                    break;
                }
                if let Some(unit) = g.units.get_mut(&uid) {
                    unit.kind = Name::new(&target);
                }
                self.fired += 1;
            }
        }
    }

    /// Hand over Gold and Faith at a rate no economy in these games reaches.
    /// Hand every city the unclaimed ground inside its workable radius.
    ///
    /// Radius three is the engine's own workable ring — `plot_purchase_cost`
    /// prices rings one through three and nothing beyond — so this grants
    /// exactly the tiles a citizen could ever be assigned to and no more.
    /// Reached by three rounds of neighbour expansion rather than by scanning
    /// the map, because this runs once per city per turn.
    fn grant_ground(&mut self, g: &mut Game, pid: usize) {
        for cid in g.player_city_ids(pid) {
            let Some(city) = g.cities.get(&cid) else {
                continue;
            };
            let mut frontier = vec![city.pos];
            let mut seen: BTreeSet<Pos> = frontier.iter().copied().collect();
            for _ in 0..CITY_WORK_RADIUS {
                let mut next = Vec::new();
                for pos in frontier.drain(..) {
                    for neighbor in g.nbrs(pos) {
                        if seen.insert(neighbor) {
                            next.push(neighbor);
                        }
                    }
                }
                frontier = next;
            }
            for pos in seen {
                // Never take ground another city holds: that would be a
                // conquest grant, and the headroom would belong to the bundle.
                let unclaimed = g
                    .map
                    .tiles
                    .get(&pos)
                    .is_some_and(|tile| tile.owner_city.is_none());
                if !unclaimed {
                    continue;
                }
                if let Some(tile) = g.map.tiles.get_mut(&pos) {
                    tile.owner_city = Some(cid);
                }
                if let Some(city) = g.cities.get_mut(&cid) {
                    if !city.owned_tiles.contains(&pos) {
                        city.owned_tiles.push(pos);
                    }
                }
                self.fired += 1;
            }
        }
    }

    /// Re-site every district foundation onto its best-yielding legal tile.
    ///
    /// `Game::item_progress_key` embeds the position for a district, and so
    /// does the queued `Item::District`, so a move has to carry all three:
    /// the tile, the queue entry, and the banked progress. Moving only the
    /// tile silently strands whatever the city had already invested and would
    /// measure a production penalty as if it were a siting result.
    fn grant_siting(&mut self, g: &mut Game, pid: usize) {
        for cid in g.player_city_ids(pid) {
            let Some(city) = g.cities.get(&cid) else {
                continue;
            };
            let foundations: Vec<(Pos, DistrictFoundation)> = city
                .owned_tiles
                .iter()
                .filter_map(|pos| {
                    g.map
                        .tiles
                        .get(pos)
                        .and_then(|tile| tile.district_foundation.clone())
                        .map(|foundation| (*pos, foundation))
                })
                .collect();
            for (old, foundation) in foundations {
                // Lift it so the engine counts neither the tile as taken nor
                // the foundation against this district family's limit.
                if let Some(tile) = g.map.tiles.get_mut(&old) {
                    tile.district_foundation = None;
                }
                let name = foundation.district.clone();
                let best = g
                    .district_sites(cid, &name)
                    .into_iter()
                    .map(|pos| (Self::yield_total(g.district_yields(&name, pos)), pos))
                    // Ties broken on position so a re-run is bit-identical.
                    .max_by(|a, b| a.0.total_cmp(&b.0).then_with(|| b.1.cmp(&a.1)))
                    .map(|(_, pos)| pos)
                    .unwrap_or(old);
                if let Some(tile) = g.map.tiles.get_mut(&best) {
                    tile.district_foundation = Some(foundation);
                }
                if best == old {
                    continue;
                }
                let old_key = format!("district:{name}:{},{}", old.0, old.1);
                let new_key = format!("district:{name}:{},{}", best.0, best.1);
                if let Some(city) = g.cities.get_mut(&cid) {
                    for item in city.queue.iter_mut() {
                        if let Item::District { district, pos } = item {
                            if *district == name && *pos == old {
                                *pos = best;
                            }
                        }
                    }
                    if let Some(progress) = city.production_progress.remove(&old_key) {
                        *city.production_progress.entry(new_key).or_insert(0.0) += progress;
                    }
                }
                self.fired += 1;
            }
        }
    }

    /// A single scalar for comparing sites. Every yield counts once: the grant
    /// is an upper bound, so it should not be handicapped by guessing which
    /// yield this particular city wanted.
    fn yield_total(ys: crate::rules::Yields) -> f64 {
        ys.food + ys.production + ys.gold + ys.science + ys.culture + ys.faith
    }

    /// Hand the seat a free Settler while it is short of its city target.
    ///
    /// One at a time, which is the engine's own standing constraint on
    /// expansion rate, so this removes the *cost* of a settler without also
    /// removing the serialization. The target is `EXPANSION_TARGET`, the same
    /// six `StrategicPlan::desired_cities` aims at, so the grant stops exactly
    /// where the agent's own appetite stops rather than expanding forever.
    fn grant_expansion(&mut self, g: &mut Game, pid: usize) {
        let Some(home) = expansion_payout_city(g, pid) else {
            return;
        };
        let Some(pos) = g.cities.get(&home).map(|city| city.pos) else {
            return;
        };
        g.spawn_unit("settler", pid, pos);
        self.fired += 1;
    }

    /// The city count the wrapped agent's own plan is currently asking for.
    ///
    /// `None` before the agent has assessed a plan. Callers treat that as
    /// "short of whatever it is about to decide", which is true by
    /// construction at that point: the seat holds its capital alone and the
    /// ramp's own floor is three.
    fn planned_cities(&self) -> Option<usize> {
        self.inner.plan_report().map(|plan| plan.desired_cities)
    }

    /// [`Grant::Expansion`] split on whether the agent's own plan had already
    /// asked for this city.
    ///
    /// `wanted` selects the half: `true` grants only while the seat is short
    /// of `desired_cities`, `false` only while it is at or above it and still
    /// under [`EXPANSION_TARGET`]. The two are exhaustive and disjoint over
    /// every turn `grant_expansion` would fire, so their firing counts sum to
    /// its own and neither can quietly become the whole grant.
    fn grant_expansion_split(&mut self, g: &mut Game, pid: usize, wanted: bool) {
        let Some(home) = expansion_payout_city(g, pid) else {
            return;
        };
        // No plan yet means the seat has not assessed one, which only happens
        // while it holds the capital alone. Any target the ramp can produce is
        // at least three, so it is short — the `wanted` half claims the turn.
        let short = match self.planned_cities() {
            Some(target) => g.player_city_ids(pid).len() < target,
            None => true,
        };
        if short != wanted {
            return;
        }
        let Some(pos) = g.cities.get(&home).map(|city| city.pos) else {
            return;
        };
        g.spawn_unit("settler", pid, pos);
        self.fired += 1;
    }

    /// The expansion grant, plus movement enough that the walk costs nothing.
    ///
    /// The top-up is applied to every Settler the seat holds, not only granted
    /// ones: a Settler the empire built itself pays the same transit, and
    /// exempting it would measure "free Settlers walk fast" rather than "the
    /// walk is free".
    ///
    /// `SWIFT_MOVES` is far past any map's useful step count, so the Settler is
    /// limited by its path and its destination rather than by its allowance.
    /// Movement is topped up before the agent plays, which is when the engine
    /// has already refreshed it, so the agent sees the allowance it will spend.
    fn grant_expansion_swift(&mut self, g: &mut Game, pid: usize) {
        const SWIFT_MOVES: f64 = 64.0;
        for uid in g.player_unit_ids(pid) {
            let Some(unit) = g.units.get_mut(&uid) else {
                continue;
            };
            if unit.kind == "settler" && unit.moves_left < SWIFT_MOVES {
                unit.moves_left = SWIFT_MOVES;
                self.fired += 1;
            }
        }
        self.grant_expansion(g, pid);
    }

    /// Order a Settler in the payout city and let the empire pay for it.
    fn grant_expansion_order(&mut self, g: &mut Game, pid: usize) {
        let Some(home) = expansion_payout_city(g, pid) else {
            return;
        };
        let settler = Item::Unit {
            unit: crate::name!("settler"),
        };
        let Some(city) = g.cities.get(&home) else {
            return;
        };
        // Already ordered: nothing to force, and re-issuing would churn the
        // banked progress for no reason.
        if city.queue.first() == Some(&settler) {
            return;
        }
        // The engine stalls a Settler at the head of a city below population
        // two. Forcing one there freezes the queue behind it, which is not a
        // decision and would be counted as one.
        if city.pop < 2 {
            return;
        }
        if !g.can_produce(pid, home, &settler) {
            return;
        }
        if g
            .apply(
                pid,
                &crate::game::Action::Produce {
                    city: home,
                    item: settler,
                },
            )
            .is_ok()
        {
            self.fired += 1;
        }
    }

    /// Pay the capital what a Settler would have cost it, and hand over no
    /// Settler.
    ///
    /// Cost-matched to [`Grant::Expansion`] on all three axes that grant
    /// actually moves: the same firing schedule (`expansion_payout_city`), the
    /// same city, and the same price — this city's own current Settler cost
    /// from `item_cost_for_city`, which already carries game speed, cost
    /// progression and any per-city modifier, plus the point of population
    /// `Game::finish_unit` charges for a Settler under the same
    /// `settler_no_population` governor exemption it checks.
    ///
    /// `city.production` is the active build's banked progress, so the empire
    /// receives the payment in the currency it would have paid in and spends
    /// it on whatever its own governor ranks first — including, if it wants
    /// one, a Settler. Overflow with an empty queue is the engine's existing
    /// unassigned-progress case and needs no special handling here.
    fn grant_rebate(&mut self, g: &mut Game, pid: usize) {
        let Some(home) = expansion_payout_city(g, pid) else {
            return;
        };
        let settler = Item::Unit {
            unit: crate::name!("settler"),
        };
        let cost = g.item_cost_for_city(pid, home, &settler);
        // A non-finite or non-positive price means this seat cannot train a
        // Settler here at all — a Congress ban, say. Paying nothing is the
        // honest match for that, and firing would be counted as an effect.
        if !cost.is_finite() || cost <= 0.0 {
            return;
        }
        // Cost-match on the BUDGET, because matching the rate does not work.
        //
        // The expansion grant hands over one Settler per city it is short of
        // `EXPANSION_TARGET`, so over a game that climbs from the capital to
        // the target it pays five times, and it measured 5.6 — the surplus is
        // Settlers lost and cities retaken. Serializing the rebate on unspent
        // money alone left it at 34.6 payments a game, still six times over,
        // because nothing about a lump of production imposes a Settler's
        // fifteen turns of transit.
        //
        // So the rebate is capped at one Settler's price per city the target
        // permits beyond the capital. That is the same accounting the
        // expansion grant does implicitly, and it is deliberately the
        // *conservative* side of 5.6: a control that is trying to fail should
        // not be handed more than the thing it is a control for.
        //
        // Note this cannot be expressed as "pay again once the last payment
        // bought a city" — the tempting version — because the finding under
        // test is precisely that the money does *not* buy cities, and that
        // rule would cut the control's budget to one payment exactly when the
        // hypothesis is true.
        if self.fired >= EXPANSION_TARGET as u64 - 1 {
            return;
        }
        // Serialize on unspent money, the way the expansion grant serializes
        // on a Settler already walking.
        //
        // ⚠ This is the correction that made the grant a control at all. The
        // firing *condition* is shared with `grant_expansion` and is identical
        // instant by instant, but the two have different consequences: a
        // granted Settler occupies the `already_walking` slot for its whole
        // transit and switches its own trigger off, while a lump of banked
        // production switches nothing off. Measured over 20 cells, that made
        // the rebate pay **66.5 times a game against the expansion grant's
        // 5.6** — twelve times the gift, which is not a cost-matched control
        // but a much larger one wearing its name. 87.0% of those payments
        // landed on a city whose queue was already empty and still holding the
        // last one.
        //
        // A fires-check that compares the two on one shared position cannot
        // see this: they agree on every instant and diverge only over a
        // trajectory. **The rate has to be measured over whole games**, which
        // is what `rebate_census` is for.
        if g.cities.get(&home).is_some_and(|city| city.production >= cost) {
            return;
        }
        let keeps_population = g.governor_effect(pid, home, "settler_no_population") > 0.0;
        let Some(city) = g.cities.get_mut(&home) else {
            return;
        };
        city.production += cost;
        if !keeps_population {
            city.pop += 1;
        }
        self.fired += 1;
    }

    /// Take every Gold above `IDLE_RESERVE_TURNS` turns of income.
    ///
    /// The threshold is a multiple of *income*, not an absolute balance,
    /// because 300 Gold is a prudent buffer at 5 a turn and dead weight at 40 —
    /// the same reason `idle_treasury_census` reports turns of income rather
    /// than Gold. Ten turns is deliberately generous: it leaves more than the
    /// median seat's own holding on the deployment map (9.4 turns), so
    /// everything taken is above what this agent typically keeps.
    fn confiscate_idle_reserve(&mut self, g: &mut Game, pid: usize) {
        let income: f64 = g
            .player_city_ids(pid)
            .into_iter()
            .map(|cid| g.city_yields(cid).gold)
            .sum::<f64>()
            .max(1.0);
        let ceiling = income * IDLE_RESERVE_TURNS;
        let Some(seat) = g.players.get_mut(pid) else {
            return;
        };
        if seat.gold > ceiling {
            seat.gold = ceiling;
            self.fired += 1;
        }
    }

    /// Put this empire one envoy above the best rival at every met city-state.
    ///
    /// Reads every rival's standing through `envoys_at`, which includes the
    /// Amani governor's bonus, so the target is the *effective* count the
    /// suzerainty rule compares rather than the raw one. Writes the raw list,
    /// because that is the only thing an agent can normally influence.
    fn grant_suzerain(&mut self, g: &mut Game, pid: usize) {
        let minors: Vec<usize> = g
            .players
            .iter()
            .filter(|minor| {
                minor.is_minor && minor.alive && !minor.is_barbarian && minor.id != pid
            })
            .map(|minor| minor.id)
            .filter(|minor| g.has_met(pid, *minor))
            .collect();
        for minor in minors {
            let best_rival = g
                .players
                .iter()
                .filter(|other| !other.is_minor && other.alive && other.id != pid)
                .map(|other| g.envoys_at(other.id, minor))
                .max()
                .unwrap_or(0);
            // `>= 3` and a strict lead; a tie is nobody's suzerainty.
            let want = (best_rival + 1).max(SUZERAIN_ENVOYS);
            if g.envoys_at(pid, minor) >= want {
                continue;
            }
            let Some(seat) = g.players.get_mut(pid) else {
                continue;
            };
            match seat.envoys.iter_mut().find(|(at, _)| *at == minor) {
                Some((_, count)) => *count = want,
                None => seat.envoys.push((minor, want)),
            }
            self.fired += 1;
        }
    }

    /// Apply every subsystem grant to the same seat on the same turn.
    ///
    /// Order is fixed and deliberate rather than alphabetical: ground before
    /// siting, because `grant_siting` re-sites onto the best legal tile and
    /// `grant_ground` is what makes the good tiles legal to begin with; and
    /// expansion last, so a Settler granted this turn is not immediately
    /// re-sited or counted against a city that does not exist yet.
    ///
    /// `self.fired` accumulates across all of them, so a compound run's firing
    /// count is a sum over grants and is NOT comparable to any single grant's.
    /// The harness only uses it to prove the treatment happened at all.
    fn grant_compound(&mut self, g: &mut Game, pid: usize) {
        self.grant_modernity(g, pid);
        self.grant_attrition(g, pid);
        self.grant_taker(g, pid);
        self.grant_ground(g, pid);
        self.grant_siting(g, pid);
        self.confiscate_idle_reserve(g, pid);
        self.grant_suzerain(g, pid);
        self.grant_expansion(g, pid);
    }

    fn grant_treasury(&mut self, g: &mut Game, pid: usize) {
        g.players[pid].gold += 200.0;
        g.players[pid].faith += 100.0;
        self.fired += 1;
    }

    /// Restore every unit to full health.
    ///
    /// Deliberately health only: no movement refresh, no extra attacks, no
    /// promotions. Those would grant tempo and experience alongside
    /// preservation and the measured headroom would belong to the bundle.
    fn grant_attrition(&mut self, g: &mut Game, pid: usize) {
        for uid in g.player_unit_ids(pid) {
            if let Some(unit) = g.units.get_mut(&uid) {
                if unit.hp < 100 {
                    unit.hp = 100;
                    self.fired += 1;
                }
            }
        }
    }

    /// Every enemy city this empire is actually reducing gets the nearest
    /// melee unit placed beside it.
    ///
    /// The trigger is *at war with its owner*, not *already open* and not
    /// *currently under siege*. Both narrower versions were tried and fired
    /// zero times: a spent garrison heals before the grant comes round again,
    /// and conditioning on an active siege only positions a taker where the
    /// agent already chose to attack, which is the case it least needs help
    /// with.
    ///
    /// What is being bounded is not "walk into the open city" — the attack
    /// evaluator already pays 520+ for a capture. It is the logistics
    /// failure underneath: over 48 six-player games the AI left 210 cities at
    /// zero garrison and had a melee unit adjacent with movement in hand on
    /// 46 of them, 22%. So this keeps one melee unit standing at every enemy
    /// city, permanently, and lets the agent's own evaluator do the rest.
    ///
    /// It may well *lose*. A lone unit parked beside a defended city dies,
    /// and the grant takes no view on whether being there is wise. That is a
    /// real outcome for an upper bound to have, and the harness reports it as
    /// HARMFUL rather than as evidence that logistics are fine.
    fn grant_taker(&mut self, g: &mut Game, pid: usize) {
        let open: Vec<(u32, Pos)> = g
            .cities
            .values()
            .filter(|city| city.owner != pid && g.is_at_war(pid, city.owner))
            .map(|city| (city.id, city.pos))
            .collect();
        for (_, city_pos) in open {
            // Somewhere legal to stand: an adjacent land tile with nobody on
            // it. Sorted so the grant is deterministic across runs.
            let mut approaches: Vec<Pos> = g
                .nbrs(city_pos)
                .into_iter()
                .filter(|position| {
                    g.map.get(*position).is_some_and(|tile| {
                        g.rules.is_passable(tile) && !g.rules.is_water(tile)
                    }) && g.units_at(*position).is_empty()
                })
                .collect();
            approaches.sort_unstable();
            let Some(landing) = approaches.first().copied() else {
                continue;
            };
            // The nearest melee unit that is not already in contact. One that
            // is already adjacent needs no help, and moving it would be the
            // harness playing the game rather than removing a constraint.
            let mut candidates: Vec<(i32, u32)> = g
                .units
                .values()
                .filter(|unit| unit.owner == pid)
                .filter(|unit| {
                    let spec = &g.rules.units[unit.kind];
                    spec.class == "military" && spec.is_melee_capable()
                })
                .filter(|unit| g.wdist(unit.pos, city_pos) > 1)
                .map(|unit| (g.wdist(unit.pos, city_pos), unit.id))
                .collect();
            candidates.sort_unstable();
            let Some((_, uid)) = candidates.first().copied() else {
                continue;
            };
            // Placed, not marched, and through `relocate` rather than by
            // writing `pos`. The engine keeps a tile->units occupancy index;
            // a bare `pos` write leaves the unit listed at its old tile, and
            // `units_at` then returns an id `units` no longer holds. That
            // panics, but only much later and in unrelated code — combat
            // resolution, disaster damage, support auras — which is how the
            // first version of this grant crashed eight worker threads in
            // four different files.
            //
            // Movement for the turn is already refreshed at this point, so
            // the unit arrives able to act. That is the whole point: the
            // constraint being removed is "the piece that could take the city
            // is not there, or is already spent".
            g.relocate(uid, landing);
            self.fired += 1;
        }
    }
}

impl<A: Ai> Oracle<A> {
    /// Apply the grant and nothing else.
    ///
    /// Split out of `take_turn` so a test can ask "would this grant have fired
    /// here" without also advancing the agent — which would move the position
    /// out from under the question.
    fn apply_grant(&mut self, g: &mut Game, pid: usize) {
        if g.winner.is_none() && !g.players[pid].is_barbarian && !g.players[pid].is_minor {
            match self.grant {
                Grant::None => {}
                Grant::Modernity => self.grant_modernity(g, pid),
                Grant::Taker => self.grant_taker(g, pid),
                Grant::Attrition => self.grant_attrition(g, pid),
                Grant::Treasury => self.grant_treasury(g, pid),
                Grant::Ground => self.grant_ground(g, pid),
                Grant::Siting => self.grant_siting(g, pid),
                Grant::Expansion => self.grant_expansion(g, pid),
                Grant::Rebate => self.grant_rebate(g, pid),
                Grant::ExpansionWanted => self.grant_expansion_split(g, pid, true),
                Grant::ExpansionBeyond => self.grant_expansion_split(g, pid, false),
                Grant::ExpansionOrder => self.grant_expansion_order(g, pid),
                Grant::ExpansionSwift => self.grant_expansion_swift(g, pid),
                Grant::IdleReserve => self.confiscate_idle_reserve(g, pid),
                Grant::Suzerain => self.grant_suzerain(g, pid),
                Grant::Compound => self.grant_compound(g, pid),
            }
        }
    }
}

impl<A: Ai> Ai for Oracle<A> {
    fn take_turn(&mut self, g: &mut Game, pid: usize) {
        self.apply_grant(g, pid);
        self.inner.take_turn(g, pid);
    }

    fn strategy_label(&self) -> Option<&'static str> {
        self.inner.strategy_label()
    }

    fn plan_report(&self) -> Option<PlanReport> {
        self.inner.plan_report()
    }

    fn review_census(&self) -> Option<crate::strategic::ReviewCensus> {
        self.inner.review_census()
    }
}

#[cfg(test)]
mod tests {
    use super::{Grant, Oracle, CITY_WORK_RADIUS, EXPANSION_TARGET, IDLE_RESERVE_TURNS};
    use crate::ai::{AdvancedAi, Ai};
    use crate::game::{Game, Item};
    use crate::Pos;

    #[test]
    fn grant_ids_round_trip() {
        for grant in Grant::ALL {
            assert_eq!(Grant::from_id(grant.name()), Some(grant));
        }
        assert_eq!(Grant::from_id("nonsense"), None);
    }

    /// The control must be exactly the wrapped agent: same seed, same game.
    /// If it were not, every measured headroom would include the harness.
    #[test]
    fn the_null_grant_changes_nothing() {
        let play = |grant: Option<Grant>| {
            let mut g = Game::new(2, 24, 16, 8_100, 90, 0);
            let mut plain = AdvancedAi::new();
            let mut wrapped = Oracle::new(AdvancedAi::new(), grant.unwrap_or(Grant::None));
            let mut other = AdvancedAi::new();
            while g.winner.is_none() && g.turn <= g.max_turns {
                let pid = g.current;
                match (pid, grant) {
                    (0, Some(_)) => wrapped.take_turn(&mut g, pid),
                    (0, None) => plain.take_turn(&mut g, pid),
                    _ => other.take_turn(&mut g, pid),
                }
                if g.winner.is_none() && g.current == pid {
                    let _ = g.apply(pid, &crate::game::Action::EndTurn);
                }
            }
            (g.turn, g.winner, g.score(0), g.score(1))
        };
        assert_eq!(
            play(None),
            play(Some(Grant::None)),
            "the null grant must reproduce the unwrapped agent exactly"
        );
    }

    /// A grant that never fires would measure the stock agent under another
    /// name — the failure `elo.rs`'s provenance work exists to prevent. The
    /// harness reports the count; this pins that it is not always zero.
    #[test]
    fn the_modernity_grant_actually_fires() {
        let mut g = Game::new(4, 28, 18, 8_101, 140, 2);
        let mut oracle = Oracle::new(AdvancedAi::new(), Grant::Modernity);
        let mut others = AdvancedAi::fleet(&g);
        while g.winner.is_none() && g.turn <= g.max_turns {
            let pid = g.current;
            if pid == 0 {
                oracle.take_turn(&mut g, pid);
            } else {
                others[pid].take_turn(&mut g, pid);
            }
            if g.winner.is_none() && g.current == pid {
                let _ = g.apply(pid, &crate::game::Action::EndTurn);
            }
        }
        assert!(
            oracle.fired() > 0,
            "the modernity grant never upgraded anything, so the run would \
             have measured the stock agent under an oracle's name"
        );
    }

    /// The taker grant must fire too. It measures a logistics failure, and a
    /// logistics failure that never presents itself is not evidence about
    /// logistics.
    #[test]
    fn the_taker_grant_actually_fires() {
        let mut g = Game::new(4, 28, 18, 8_103, 250, 2);
        let mut oracle = Oracle::new(AdvancedAi::new(), Grant::Taker);
        let mut others = AdvancedAi::fleet(&g);
        while g.winner.is_none() && g.turn <= g.max_turns {
            let pid = g.current;
            if pid == 0 {
                oracle.take_turn(&mut g, pid);
            } else {
                others[pid].take_turn(&mut g, pid);
            }
            if g.winner.is_none() && g.current == pid {
                let _ = g.apply(pid, &crate::game::Action::EndTurn);
            }
        }
        assert!(
            oracle.fired() > 0,
            "the taker grant never positioned anybody, so the run would have \
             measured the stock agent under an oracle's name"
        );
    }

    /// Attrition must fire too, and must grant health and nothing else.
    #[test]
    fn the_attrition_grant_fires_and_only_heals() {
        let mut g = Game::new(4, 28, 18, 8_104, 200, 2);
        let mut oracle = Oracle::new(AdvancedAi::new(), Grant::Attrition);
        let mut others = AdvancedAi::fleet(&g);
        while g.winner.is_none() && g.turn <= 160 {
            let pid = g.current;
            if pid == 0 {
                let mut probe = g.clone();
                let before: Vec<u32> = probe.player_unit_ids(0);
                let gold = probe.players[0].gold;
                oracle.grant_attrition(&mut probe, 0);
                assert_eq!(before, probe.player_unit_ids(0), "the grant changed the roster");
                assert_eq!(gold, probe.players[0].gold, "the grant moved the treasury");
                assert!(
                    probe.player_unit_ids(0).iter().all(|uid| probe.units[uid].hp == 100),
                    "a healed empire must have no wounded units left"
                );
                oracle.take_turn(&mut g, pid);
            } else {
                others[pid].take_turn(&mut g, pid);
            }
            if g.winner.is_none() && g.current == pid {
                let _ = g.apply(pid, &crate::game::Action::EndTurn);
            }
        }
        assert!(oracle.fired() > 0, "the attrition grant never healed anything");
    }


    /// The ground grant must actually hand a city ground, must leave every
    /// other city's territory alone, and must never reach past the workable
    /// ring. A grant that never fires measures the stock agent under another
    /// name; a grant that takes a rival's tiles measures conquest.
    #[test]
    fn the_ground_grant_fires_and_only_takes_neutral_ground() {
        let mut g = Game::new(4, 28, 18, 8_106, 200, 2);
        let mut oracle = Oracle::new(AdvancedAi::new(), Grant::Ground);
        let mut others = AdvancedAi::fleet(&g);
        let mut ever_grew = false;
        while g.winner.is_none() && g.turn <= 120 {
            let pid = g.current;
            if pid == 0 {
                let mut probe = g.clone();
                let before: Vec<(u32, usize)> = probe
                    .cities
                    .values()
                    .map(|city| (city.id, city.owned_tiles.len()))
                    .collect();
                // A city can already hold ground outside the workable ring --
                // territory inherited when a neighbour was razed, for one --
                // so the radius assertion below must scope to what the grant
                // itself added, not to everything the city owns.
                let held_before: std::collections::BTreeSet<Pos> = probe
                    .player_city_ids(0)
                    .into_iter()
                    .flat_map(|cid| probe.cities[&cid].owned_tiles.clone())
                    .collect();
                let rival_ground: Vec<(Pos, Option<u32>)> = probe
                    .map
                    .tiles
                    .values()
                    .filter(|tile| {
                        tile.owner_city
                            .is_some_and(|cid| probe.cities.get(&cid).is_some_and(|c| c.owner != 0))
                    })
                    .map(|tile| (tile.pos, tile.owner_city))
                    .collect();
                let gold = probe.players[0].gold;

                oracle.grant_ground(&mut probe, 0);

                assert_eq!(gold, probe.players[0].gold, "the grant charged for ground");
                for (pos, owner) in &rival_ground {
                    assert_eq!(
                        probe.map.tiles[pos].owner_city, *owner,
                        "the grant took ground from another city"
                    );
                }
                for cid in probe.player_city_ids(0) {
                    let city = &probe.cities[&cid];
                    for pos in &city.owned_tiles {
                        if !held_before.contains(pos) {
                            assert!(
                                probe.wdist(city.pos, *pos) <= CITY_WORK_RADIUS as i32,
                                "the grant reached past the workable ring"
                            );
                        }
                        assert_eq!(
                            probe.map.tiles[pos].owner_city,
                            Some(cid),
                            "owned_tiles and owner_city disagree after the grant"
                        );
                    }
                }
                let after: Vec<(u32, usize)> = probe
                    .cities
                    .values()
                    .map(|city| (city.id, city.owned_tiles.len()))
                    .collect();
                if before != after {
                    ever_grew = true;
                }
                oracle.take_turn(&mut g, pid);
            } else {
                others[pid].take_turn(&mut g, pid);
            }
            if g.winner.is_none() && g.current == pid {
                let _ = g.apply(pid, &crate::game::Action::EndTurn);
            }
        }
        assert!(oracle.fired() > 0, "the ground grant never claimed a tile");
        assert!(ever_grew, "the grant never enlarged a city's territory");
    }


    /// The siting grant must actually move foundations, must never lose a
    /// city's banked production when it does, must never leave two tiles
    /// holding the same foundation, and must only ever improve a site.
    #[test]
    fn the_siting_grant_fires_and_never_strands_progress() {
        let mut g = Game::new(4, 28, 18, 8_107, 200, 2);
        let mut oracle = Oracle::new(AdvancedAi::new(), Grant::Siting);
        let mut others = AdvancedAi::fleet(&g);
        let mut ever_moved = false;
        while g.winner.is_none() && g.turn <= 140 {
            let pid = g.current;
            if pid == 0 {
                let mut probe = g.clone();
                let banked: f64 = probe
                    .player_city_ids(0)
                    .into_iter()
                    .map(|cid| probe.cities[&cid].production_progress.values().sum::<f64>())
                    .sum();
                let before: Vec<(Pos, crate::name::Name)> = foundations_of(&probe, 0);
                let gold = probe.players[0].gold;

                oracle.grant_siting(&mut probe, 0);

                assert_eq!(gold, probe.players[0].gold, "the grant charged for siting");
                let after = foundations_of(&probe, 0);
                assert_eq!(
                    before.len(),
                    after.len(),
                    "the grant created or destroyed a foundation"
                );
                let banked_after: f64 = probe
                    .player_city_ids(0)
                    .into_iter()
                    .map(|cid| probe.cities[&cid].production_progress.values().sum::<f64>())
                    .sum();
                assert!(
                    (banked - banked_after).abs() < 1e-6,
                    "moving a foundation stranded banked production: {banked} -> {banked_after}"
                );
                // Each district that moved must have moved somewhere better,
                // and a queued Item::District must point at the tile that now
                // holds its foundation.
                for cid in probe.player_city_ids(0) {
                    for item in &probe.cities[&cid].queue {
                        if let Item::District { district, pos } = item {
                            assert_eq!(
                                probe.map.tiles[pos]
                                    .district_foundation
                                    .as_ref()
                                    .map(|f| f.district.as_str()),
                                Some(district.as_str()),
                                "a queued district points at a tile with no matching foundation"
                            );
                        }
                    }
                }
                if before != after {
                    ever_moved = true;
                }
                oracle.take_turn(&mut g, pid);
            } else {
                others[pid].take_turn(&mut g, pid);
            }
            if g.winner.is_none() && g.current == pid {
                let _ = g.apply(pid, &crate::game::Action::EndTurn);
            }
        }
        assert!(oracle.fired() > 0, "the siting grant never moved a foundation");
        assert!(ever_moved, "no foundation ever changed tile");
    }

    fn foundations_of(g: &Game, pid: usize) -> Vec<(Pos, crate::name::Name)> {
        let mut out: Vec<(Pos, crate::name::Name)> = g
            .player_city_ids(pid)
            .into_iter()
            .flat_map(|cid| g.cities[&cid].owned_tiles.clone())
            .filter_map(|pos| {
                g.map.tiles[&pos]
                    .district_foundation
                    .as_ref()
                    .map(|f| (pos, f.district.clone()))
            })
            .collect();
        out.sort();
        out
    }


    /// The expansion grant must actually hand out Settlers, must hand out
    /// exactly one at a time, must stop at the target, and must grant nothing
    /// else. A grant that quietly also moved the treasury would attribute
    /// treasury's headroom to expansion.
    #[test]
    fn the_expansion_grant_fires_and_grants_only_settlers() {
        let mut g = Game::new(4, 28, 18, 8_108, 200, 2);
        let mut oracle = Oracle::new(AdvancedAi::new(), Grant::Expansion);
        let mut others = AdvancedAi::fleet(&g);
        let mut ever_granted = false;
        while g.winner.is_none() && g.turn <= 140 {
            let pid = g.current;
            if pid == 0 {
                let mut probe = g.clone();
                let gold = probe.players[0].gold;
                let faith = probe.players[0].faith;
                let before: Vec<u32> = probe.player_unit_ids(0);
                let settlers_before = probe
                    .player_unit_ids(0)
                    .into_iter()
                    .filter(|uid| probe.units[uid].kind == "settler")
                    .count();
                let cities = probe.player_city_ids(0).len();

                oracle.grant_expansion(&mut probe, 0);

                assert_eq!(gold, probe.players[0].gold, "the grant moved the treasury");
                assert_eq!(faith, probe.players[0].faith, "the grant moved Faith");
                let after: Vec<u32> = probe.player_unit_ids(0);
                let added = after.len() - before.len();
                assert!(added <= 1, "the grant handed out {added} units at once");
                if added == 1 {
                    ever_granted = true;
                    assert!(
                        settlers_before == 0,
                        "the grant stacked a second settler on a seat that had one"
                    );
                    assert!(
                        cities < EXPANSION_TARGET,
                        "the grant fired at or above its own target"
                    );
                    let fresh: Vec<u32> =
                        after.iter().copied().filter(|uid| !before.contains(uid)).collect();
                    assert_eq!(fresh.len(), 1);
                    assert_eq!(
                        probe.units[&fresh[0]].kind, "settler",
                        "the grant handed out something other than a settler"
                    );
                }
                oracle.take_turn(&mut g, pid);
            } else {
                others[pid].take_turn(&mut g, pid);
            }
            if g.winner.is_none() && g.current == pid {
                let _ = g.apply(pid, &crate::game::Action::EndTurn);
            }
        }
        assert!(oracle.fired() > 0, "the expansion grant never handed out a settler");
        assert!(ever_granted, "no settler was ever observed being granted");
    }

    /// When the rebate pays, it pays exactly a Settler's price into exactly
    /// the city the expansion grant would have used, returns exactly one
    /// population, and hands over no unit.
    ///
    /// ⚠ It asserts a **subset**, not an equality: every turn the rebate pays
    /// is a turn the expansion grant would also have fired, but not the
    /// reverse. The first version of this test asserted equality, which was
    /// true of the code as first written and was exactly the wrong property to
    /// pin down — the two grants agree on every shared position and diverge
    /// over a trajectory, so equality here certified a rebate that went on to
    /// pay 66.5 times a game against the expansion grant's 5.6. The budget is
    /// what makes it a control, and
    /// `the_rebate_never_pays_more_than_the_expansion_grant_could` is where
    /// that lives.
    #[test]
    fn the_rebate_pays_a_settlers_price_where_the_expansion_grant_would() {
        let mut g = Game::new(4, 28, 18, 8_108, 200, 2);
        let mut rebate = Oracle::new(AdvancedAi::new(), Grant::Rebate);
        let mut expansion = Oracle::new(AdvancedAi::new(), Grant::Expansion);
        let mut others = AdvancedAi::fleet(&g);
        let mut ever_paid = false;
        while g.winner.is_none() && g.turn <= 140 {
            let pid = g.current;
            if pid == 0 {
                // Two clones of the same position, so the schedules are
                // compared on identical inputs rather than on two games that
                // have already diverged.
                let mut paid = g.clone();
                let mut settled = g.clone();
                let fired_before = rebate.fired();
                let expansion_fired_before = expansion.fired();

                let units_before = paid.player_unit_ids(0).len();
                let gold = paid.players[0].gold;
                let home = super::expansion_payout_city(&paid, 0);
                let banked = home.map(|cid| paid.cities[&cid].production);
                let pop = home.map(|cid| paid.cities[&cid].pop);
                let price = home.map(|cid| {
                    paid.item_cost_for_city(
                        0,
                        cid,
                        &Item::Unit {
                            unit: crate::name!("settler"),
                        },
                    )
                });

                rebate.grant_rebate(&mut paid, 0);
                expansion.grant_expansion(&mut settled, 0);

                let rebate_fires = rebate.fired() - fired_before;
                let expansion_fires = expansion.fired() - expansion_fired_before;
                assert!(
                    rebate_fires <= expansion_fires,
                    "the rebate paid on a turn the expansion grant would not have"
                );
                assert_eq!(
                    units_before,
                    paid.player_unit_ids(0).len(),
                    "the rebate handed out a unit"
                );
                assert_eq!(gold, paid.players[0].gold, "the rebate moved the treasury");

                if rebate.fired() > fired_before {
                    ever_paid = true;
                    let cid = home.expect("a payout city, since the grant fired");
                    let price = price.expect("a price, since the grant fired");
                    assert!(price > 0.0, "the rebate paid a non-positive price");
                    assert!(
                        (paid.cities[&cid].production - (banked.unwrap() + price)).abs() < 1e-6,
                        "the rebate banked something other than a settler's cost"
                    );
                    assert_eq!(
                        paid.cities[&cid].pop,
                        pop.unwrap() + 1,
                        "the rebate did not return the settler's population"
                    );
                }
                rebate.take_turn(&mut g, pid);
            } else {
                others[pid].take_turn(&mut g, pid);
            }
            if g.winner.is_none() && g.current == pid {
                let _ = g.apply(pid, &crate::game::Action::EndTurn);
            }
        }
        assert!(rebate.fired() > 0, "the rebate never paid out");
        assert!(ever_paid, "no payment was ever observed");
    }

    /// The rebate's whole-game budget is capped at one Settler's price per
    /// city the target permits beyond the capital.
    ///
    /// This is the assertion `the_rebate_matches_the_expansion_grants_schedule_and_price`
    /// cannot make. That test compares the two grants on one shared position,
    /// where they agree by construction; the quantity that actually decides
    /// whether the rebate is a cost-matched control is the **total** it hands
    /// over across a whole game, and the two diverge there because a granted
    /// Settler switches its own trigger off for its entire transit and banked
    /// production switches nothing off. Measured before this cap existed, the
    /// rebate paid 66.5 times a game and then 34.6 with serialization alone,
    /// against the expansion grant's 5.6.
    #[test]
    fn the_rebate_never_pays_more_than_the_expansion_grant_could() {
        let mut g = Game::new(4, 28, 18, 8_108, 200, 2);
        let mut rebate = Oracle::new(AdvancedAi::new(), Grant::Rebate);
        let mut others = AdvancedAi::fleet(&g);
        while g.winner.is_none() && g.turn <= 200 {
            let pid = g.current;
            if pid == 0 {
                rebate.take_turn(&mut g, pid);
                assert!(
                    rebate.fired() < EXPANSION_TARGET as u64,
                    "the rebate paid {} times, past its own budget",
                    rebate.fired()
                );
            } else {
                others[pid].take_turn(&mut g, pid);
            }
            if g.winner.is_none() && g.current == pid {
                let _ = g.apply(pid, &crate::game::Action::EndTurn);
            }
        }
        assert!(rebate.fired() > 0, "the rebate never paid, so the cap is untested");
    }

    /// The compound grant must actually apply every component, not silently
    /// become one of them.
    ///
    /// A composite is the easiest kind of treatment to get wrong without
    /// noticing: if one component's precondition never holds, the run measures
    /// the others under a name that promises all of them, and a null reads as
    /// "perfecting everything is worth nothing" when it means "one grant was
    /// switched off". So this asserts each component fires at least once
    /// within the same game, checked one at a time against a solo oracle on
    /// the identical position.
    #[test]
    fn the_compound_grant_applies_every_component() {
        let components = [
            Grant::Modernity,
            Grant::Attrition,
            Grant::Taker,
            Grant::Ground,
            Grant::Siting,
            Grant::IdleReserve,
            Grant::Suzerain,
            Grant::Expansion,
        ];
        let mut g = Game::new(4, 28, 18, 8_108, 200, 2);
        let mut compound = Oracle::new(AdvancedAi::new(), Grant::Compound);
        let mut others = AdvancedAi::fleet(&g);
        let mut ever: std::collections::BTreeMap<&str, bool> =
            components.iter().map(|c| (c.name(), false)).collect();
        while g.winner.is_none() && g.turn <= 120 {
            let pid = g.current;
            if pid == 0 {
                // Each component is applied to its own clone of the same
                // position, so "did it fire here" is asked of every one of
                // them under identical conditions.
                for component in components {
                    let mut probe = g.clone();
                    let mut solo = Oracle::new(AdvancedAi::new(), component);
                    solo.apply_grant(&mut probe, 0);
                    if solo.fired() > 0 {
                        ever.insert(component.name(), true);
                    }
                }
                compound.take_turn(&mut g, pid);
            } else {
                others[pid].take_turn(&mut g, pid);
            }
            if g.winner.is_none() && g.current == pid {
                let _ = g.apply(pid, &crate::game::Action::EndTurn);
            }
        }
        assert!(compound.fired() > 0, "the compound grant never fired at all");
        let silent: Vec<&str> = ever
            .iter()
            .filter(|(_, fired)| !**fired)
            .map(|(name, _)| *name)
            .collect();
        assert!(
            silent.is_empty(),
            "these components never fired in this game, so a compound result \
would not be about them: {silent:?}"
        );
    }

    /// The swift grant must actually buy transit — checked at **both** map
    /// scales, because the answer is not the same one.
    ///
    /// ⚠ At 28x18 with four players (126 tiles a seat) swift Settlers covered
    /// **0.74 tiles a turn against the plain grant's 0.98** — *less* ground,
    /// with 64 movement points in hand. That is not a bug in the grant. It is
    /// the cramped-map regime #559 identified, where "no settle site in reach"
    /// is the dominant blocker: a Settler that founds instantly frees the
    /// `already_walking` slot instantly, the next one spawns into a map whose
    /// sites are already consumed, and it stands still holding no destination.
    /// Movement it never needed was replaced by turns it could not use.
    ///
    /// So the assertion is made where the grant is meant to act — a roomy map
    /// at deployment-like density — and the cramped reading is kept above as
    /// the finding it is. [[civvis-eval-defaults-are-not-the-deployment]] in
    /// spirit: fires-check both scales on any expansion treatment, because one
    /// of them will lie to you.
    #[test]
    fn the_swift_grant_actually_moves_settlers_further() {
        fn tiles_per_settler_turn(grant: Grant, width: i32, height: i32) -> (f64, u64) {
            let mut g = Game::new(4, width, height, 8_108, 200, 2);
            let mut oracle = Oracle::new(AdvancedAi::new(), grant);
            let mut others = AdvancedAi::fleet(&g);
            let mut was: std::collections::BTreeMap<u32, crate::Pos> = Default::default();
            let (mut tiles, mut turns) = (0i32, 0u64);
            while g.winner.is_none() && g.turn <= 100 {
                let pid = g.current;
                if pid == 0 {
                    oracle.take_turn(&mut g, pid);
                    let now: std::collections::BTreeMap<u32, crate::Pos> = g
                        .player_unit_ids(0)
                        .into_iter()
                        .filter_map(|uid| g.units.get(&uid))
                        .filter(|unit| unit.kind == "settler")
                        .map(|unit| (unit.id, unit.pos))
                        .collect();
                    for (uid, pos) in &now {
                        if let Some(before) = was.get(uid) {
                            tiles += g.wdist(*before, *pos).max(0);
                            turns += 1;
                        }
                    }
                    was = now;
                } else {
                    others[pid].take_turn(&mut g, pid);
                }
                if g.winner.is_none() && g.current == pid {
                    let _ = g.apply(pid, &crate::game::Action::EndTurn);
                }
            }
            (tiles as f64 / turns.max(1) as f64, turns)
        }
        // 48x30 at four players is 360 tiles a seat, against the cramped
        // default's 126 and the live exhibition's 567.
        let (plain, plain_turns) = tiles_per_settler_turn(Grant::Expansion, 48, 30);
        let (swift, swift_turns) = tiles_per_settler_turn(Grant::ExpansionSwift, 48, 30);
        assert!(plain_turns > 0 && swift_turns > 0, "no settler turns observed");
        assert!(
            swift > plain,
            "on a roomy map swift Settlers covered {swift:.2} tiles/turn against \
plain's {plain:.2}, so the grant buys no transit anywhere and there is no point \
spending an ablation batch on it"
        );
    }

    /// The ordered Settler must still be at the head of the queue after the
    /// agent has played its whole turn.
    ///
    /// This is the assumption the grant rests on and the one most likely to be
    /// wrong. `advanced_production` reconsiders a city's build every turn, and
    /// if it re-decided over the order the grant would be inert and report a
    /// null for the wrong reason. It does not, because at the shipped
    /// `preempt_margin` of 1.0 it skips any city whose queue is non-empty —
    /// but that is a property of code this module does not own, so it is
    /// asserted rather than assumed.
    #[test]
    fn an_ordered_settler_survives_the_agents_own_turn() {
        let mut g = Game::new(4, 28, 18, 8_108, 200, 2);
        let mut oracle = Oracle::new(AdvancedAi::new(), Grant::ExpansionOrder);
        let mut others = AdvancedAi::fleet(&g);
        let settler = Item::Unit {
            unit: crate::name!("settler"),
        };
        let mut survived = 0usize;
        let mut overridden = 0usize;
        while g.winner.is_none() && g.turn <= 160 {
            let pid = g.current;
            if pid == 0 {
                let before = oracle.fired();
                // The grant runs inside `take_turn`, before the agent plays.
                let ordered = super::expansion_payout_city(&g, 0);
                oracle.take_turn(&mut g, pid);
                if oracle.fired() > before {
                    let cid = ordered.expect("a payout city, since the grant fired");
                    if g.cities.get(&cid).and_then(|city| city.queue.first())
                        == Some(&settler)
                    {
                        survived += 1;
                    } else {
                        overridden += 1;
                    }
                }
            } else {
                others[pid].take_turn(&mut g, pid);
            }
            if g.winner.is_none() && g.current == pid {
                let _ = g.apply(pid, &crate::game::Action::EndTurn);
            }
        }
        assert!(oracle.fired() > 0, "the order grant never fired");
        assert!(
            survived > overridden,
            "the agent re-decided over {overridden} of {} orders, so the grant is \
mostly inert and a null from it would say nothing",
            survived + overridden
        );
    }

    /// `expansion_wanted` and `expansion_beyond` must partition `expansion`:
    /// on every turn, exactly one of them fires if and only if `expansion`
    /// does.
    ///
    /// A partition is the whole point of the split. If the two overlapped,
    /// each would carry some of the other's effect and neither result would
    /// localise anything; if they left a gap, their two effects would not sum
    /// to the grant they are decomposing and a missing share would look like
    /// an interaction. All three are driven off clones of one position so the
    /// comparison is exact rather than statistical.
    /// ⚠ All three arms are probed off **one** acting oracle. Three separate
    /// `Oracle`s would each wrap their own `AdvancedAi`, and only the one that
    /// actually took turns would ever assess a plan — so `planned_cities()`
    /// would read `None` forever on the other two, the `wanted` half would
    /// claim every eligible turn by its no-plan fallback, and the partition
    /// would look perfect while testing nothing.
    #[test]
    fn the_expansion_split_partitions_the_expansion_grant() {
        let mut g = Game::new(4, 28, 18, 8_108, 200, 2);
        // Plays stock, so the split is measured over the trajectory it is
        // meant to describe rather than over one the grant already changed.
        let mut agent = Oracle::new(AdvancedAi::new(), Grant::None);
        let mut others = AdvancedAi::fleet(&g);
        let mut ever_wanted = false;
        let mut ever_beyond = false;
        let mut ever_whole = false;
        while g.winner.is_none() && g.turn <= 160 {
            let pid = g.current;
            if pid == 0 {
                let (mut a, mut b, mut c) = (g.clone(), g.clone(), g.clone());
                let before = agent.fired();
                agent.grant_expansion(&mut a, 0);
                let whole = agent.fired() - before;
                let before = agent.fired();
                agent.grant_expansion_split(&mut b, 0, true);
                let wanted = agent.fired() - before;
                let before = agent.fired();
                agent.grant_expansion_split(&mut c, 0, false);
                let beyond = agent.fired() - before;

                assert_eq!(
                    wanted + beyond,
                    whole,
                    "the split does not sum to the whole grant"
                );
                assert!(
                    wanted == 0 || beyond == 0,
                    "both halves fired on the same turn"
                );
                ever_whole |= whole > 0;
                ever_wanted |= wanted > 0;
                ever_beyond |= beyond > 0;
                agent.take_turn(&mut g, pid);
            } else {
                others[pid].take_turn(&mut g, pid);
            }
            if g.winner.is_none() && g.current == pid {
                let _ = g.apply(pid, &crate::game::Action::EndTurn);
            }
        }
        assert!(ever_whole, "the whole grant never fired, so nothing is partitioned");
        assert!(
            ever_wanted || ever_beyond,
            "neither half ever fired while the whole grant did"
        );
    }


    /// The confiscation must actually take Gold, must leave a working buffer
    /// rather than emptying the treasury, and must take nothing else.
    #[test]
    fn the_idle_reserve_ablation_fires_and_leaves_a_buffer() {
        let mut g = Game::new(4, 28, 18, 8_109, 200, 2);
        let mut oracle = Oracle::new(AdvancedAi::new(), Grant::IdleReserve);
        let mut others = AdvancedAi::fleet(&g);
        let mut ever_took = false;
        while g.winner.is_none() && g.turn <= 140 {
            let pid = g.current;
            if pid == 0 {
                let mut probe = g.clone();
                let before = probe.players[0].gold;
                let faith = probe.players[0].faith;
                let units = probe.player_unit_ids(0);
                let cities = probe.player_city_ids(0);
                let income: f64 = cities
                    .iter()
                    .map(|cid| probe.city_yields(*cid).gold)
                    .sum::<f64>()
                    .max(1.0);

                oracle.confiscate_idle_reserve(&mut probe, 0);

                let after = probe.players[0].gold;
                assert!(after <= before + 1e-9, "the ablation ADDED Gold");
                assert_eq!(faith, probe.players[0].faith, "the ablation moved Faith");
                assert_eq!(units, probe.player_unit_ids(0), "the ablation changed the roster");
                assert_eq!(cities, probe.player_city_ids(0), "the ablation changed the cities");
                // Whatever it leaves must still be a working buffer.
                assert!(
                    after >= (income * IDLE_RESERVE_TURNS).min(before) - 1e-6,
                    "the ablation cut into the buffer it is supposed to leave: \
                     {after} against {income}/turn"
                );
                if after < before - 1e-9 {
                    ever_took = true;
                }
                oracle.take_turn(&mut g, pid);
            } else {
                others[pid].take_turn(&mut g, pid);
            }
            if g.winner.is_none() && g.current == pid {
                let _ = g.apply(pid, &crate::game::Action::EndTurn);
            }
        }
        assert!(oracle.fired() > 0, "the idle-reserve ablation never took anything");
        assert!(ever_took, "no confiscation was ever observed");
    }


    /// The suzerainty grant must actually take suzerainties, must take them at
    /// every met city-state, and must not touch a rival's envoys or anything
    /// else. A grant that also moved Gold would attribute treasury's very large
    /// headroom to diplomacy.
    #[test]
    fn the_suzerain_grant_fires_and_only_moves_its_own_envoys() {
        let mut g = Game::new(4, 28, 18, 8_110, 200, 4);
        let mut oracle = Oracle::new(AdvancedAi::new(), Grant::Suzerain);
        let mut others = AdvancedAi::fleet(&g);
        let mut ever_gained = false;
        while g.winner.is_none() && g.turn <= 140 {
            let pid = g.current;
            if pid == 0 {
                let mut probe = g.clone();
                let gold = probe.players[0].gold;
                let faith = probe.players[0].faith;
                let rival_envoys: Vec<Vec<(usize, i64)>> = probe
                    .players
                    .iter()
                    .filter(|p| p.id != 0)
                    .map(|p| p.envoys.clone())
                    .collect();
                let met: Vec<usize> = probe
                    .players
                    .iter()
                    .filter(|m| m.is_minor && m.alive && !m.is_barbarian)
                    .map(|m| m.id)
                    .filter(|m| probe.has_met(0, *m))
                    .collect();
                let before = met
                    .iter()
                    .filter(|m| probe.suzerain_of(**m) == Some(0))
                    .count();

                oracle.grant_suzerain(&mut probe, 0);

                assert_eq!(gold, probe.players[0].gold, "the grant moved Gold");
                assert_eq!(faith, probe.players[0].faith, "the grant moved Faith");
                let rivals_after: Vec<Vec<(usize, i64)>> = probe
                    .players
                    .iter()
                    .filter(|p| p.id != 0)
                    .map(|p| p.envoys.clone())
                    .collect();
                assert_eq!(rival_envoys, rivals_after, "the grant moved a rival's envoys");
                for minor in &met {
                    assert_eq!(
                        probe.suzerain_of(*minor),
                        Some(0),
                        "a met city-state was left unclaimed"
                    );
                }
                if !met.is_empty() && before < met.len() {
                    ever_gained = true;
                }
                oracle.take_turn(&mut g, pid);
            } else {
                others[pid].take_turn(&mut g, pid);
            }
            if g.winner.is_none() && g.current == pid {
                let _ = g.apply(pid, &crate::game::Action::EndTurn);
            }
        }
        assert!(oracle.fired() > 0, "the suzerain grant never placed an envoy");
        assert!(ever_gained, "no suzerainty was ever actually gained");
    }

    /// The grant must be modernization and nothing else: no Gold, no health,
    /// no extra units. Otherwise a measured headroom would be the bundle's,
    /// not the subsystem's.
    #[test]
    fn the_modernity_grant_changes_only_the_unit_types() {
        let mut g = Game::new(4, 28, 18, 8_102, 200, 2);
        let mut oracle = Oracle::new(AdvancedAi::new(), Grant::Modernity);
        let mut others = AdvancedAi::fleet(&g);
        let mut checked = 0usize;
        while g.winner.is_none() && g.turn <= 160 {
            let pid = g.current;
            if pid == 0 {
                let mut probe = g.clone();
                let before: Vec<(u32, i32)> =
                    probe.units.iter().map(|(id, u)| (*id, u.hp)).collect();
                let gold = probe.players[0].gold;
                oracle.grant_modernity(&mut probe, 0);
                let after: Vec<(u32, i32)> =
                    probe.units.iter().map(|(id, u)| (*id, u.hp)).collect();
                assert_eq!(before, after, "the grant changed unit count or health");
                assert_eq!(gold, probe.players[0].gold, "the grant moved the treasury");
                checked += 1;
                oracle.take_turn(&mut g, pid);
            } else {
                others[pid].take_turn(&mut g, pid);
            }
            if g.winner.is_none() && g.current == pid {
                let _ = g.apply(pid, &crate::game::Action::EndTurn);
            }
        }
        assert!(checked > 50, "only {checked} turns exercised");
    }
}
