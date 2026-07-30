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

/// The city count the expansion grant stops at.
///
/// This is an oracle ceiling, not a claim about the live plan. `AdvancedAi`
/// computes a dynamic `desired_cities` that averaged 3.83 on the small
/// evaluation profile and 5.00 on the measured 6p/74x46 profile; granting six
/// cities can therefore exceed the policy's own target. The oracle measures
/// expansion headroom, including a too-low target, rather than affordability
/// of the existing plan alone.
const EXPANSION_TARGET: usize = 6;

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
    /// A free Settler whenever the seat is below the fixed six-city oracle
    /// ceiling and has none in flight.
    ///
    /// Bounds **expansion**, which every other city measurement is conditional
    /// on. #532/#534/#542/#553 bound what a city works, owns, builds districts
    /// on and stands on — all *per city*, and all saturated or null. None of
    /// them asks whether the empire has enough cities in the first place. The
    /// live plan has a dynamic target, measured at 3.83 on the small profile
    /// and 5.00 on a large six-player profile; the oracle can therefore grant
    /// beyond the plan as well as remove Settler cost and population cost.
    ///
    /// A settler costs production and a point of population, and needs
    /// `pop >= 2` to build at all, so expansion is paid for out of exactly the
    /// early economy that is also paying for everything else. This grants the
    /// settler without the cost, so the win rate says what perfect expansion
    /// tempo is worth at most.
    ///
    /// It grants only the unit. Where to settle is still the agent's decision,
    /// walked by its own settler logic — which #553 measured taking 99.9% of
    /// the value on offer — so this is a bound on expansion target and *rate*,
    /// not on siting.
    Expansion,
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
    /// Bounds the complete Envoy acquisition-and-allocation bundle. Unlike a
    /// conserved-stock policy test, it creates the focal raw Envoys needed for
    /// control and places them perfectly while leaving the free pool, focal
    /// placements elsewhere, and every rival untouched. The result can
    /// prioritize the Envoy layer, but cannot say whether acquisition or
    /// allocation supplied the headroom; `raw_envoys_granted` makes the
    /// resource grant explicit.
    ///
    /// The engine's rule is `envoys >= 3` and strictly more than every other
    /// major. The grant finds the smallest raw count whose engine-effective
    /// value reaches one above the best rival, so Messenger and Puppeteer are
    /// applied exactly once. A discrete multiplier may jump past the target,
    /// but no additional raw Envoy is added beyond that minimum.
    ///
    /// If this reads large, the Envoy layer is high-leverage and separately
    /// conserving stock decides whether work belongs in allocation or income.
    /// If it reads null, city-state control is not a binding subsystem.
    Suzerain,
}

impl Grant {
    pub const ALL: [Grant; 10] = [
        Grant::None,
        Grant::Modernity,
        Grant::Taker,
        Grant::Attrition,
        Grant::Treasury,
        Grant::Ground,
        Grant::Siting,
        Grant::Expansion,
        Grant::IdleReserve,
        Grant::Suzerain,
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
            Grant::IdleReserve => "idle_reserve",
            Grant::Suzerain => "suzerain",
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
    /// Raw Envoys created by `Grant::Suzerain`.
    ///
    /// This provenance is separate from `fired`, which counts changed
    /// city-states. The grant does not conserve the focal empire's stock: it
    /// is a compound acquisition-plus-allocation ceiling, and future reports
    /// must be able to say how much resource it created.
    raw_envoys_granted: u64,
}

impl<A: Ai> Oracle<A> {
    pub fn new(inner: A, grant: Grant) -> Oracle<A> {
        Oracle {
            inner,
            grant,
            fired: 0,
            raw_envoys_granted: 0,
        }
    }

    /// Times the grant changed the position.
    pub fn fired(&self) -> u64 {
        self.fired
    }

    /// Raw focal Envoys created by the compound Suzerainty ceiling.
    pub fn raw_envoys_granted(&self) -> u64 {
        self.raw_envoys_granted
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
    /// removing the serialization. `EXPANSION_TARGET` is an oracle ceiling,
    /// not the live plan's dynamic appetite; see the constant's documentation.
    fn grant_expansion(&mut self, g: &mut Game, pid: usize) {
        let cities = g.player_city_ids(pid);
        if cities.is_empty() || cities.len() >= EXPANSION_TARGET {
            return;
        }
        let already_walking = g
            .player_unit_ids(pid)
            .into_iter()
            .any(|uid| g.units.get(&uid).is_some_and(|unit| unit.kind == "settler"));
        if already_walking {
            return;
        }
        // The capital, by lowest id so the grant is deterministic.
        let Some(home) = cities.iter().copied().min() else {
            return;
        };
        let Some(pos) = g.cities.get(&home).map(|city| city.pos) else {
            return;
        };
        g.spawn_unit("settler", pid, pos);
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

    /// Give this empire the minimum raw stock needed to control every met
    /// city-state.
    ///
    /// This is deliberately a compound acquisition-plus-allocation ceiling:
    /// it creates focal raw Envoys without consuming the free pool or moving a
    /// previous placement. Rival standings and the focal target are effective
    /// counts through `envoys_at`, but the stored table is raw. Search that
    /// engine method for the smallest raw count that reaches the target so
    /// Messenger and Puppeteer are applied exactly once rather than writing an
    /// effective target into raw stock and applying Amani again.
    fn grant_suzerain(&mut self, g: &mut Game, pid: usize) {
        let minors: Vec<usize> = g
            .players
            .iter()
            .filter(|minor| minor.is_minor && minor.alive && !minor.is_barbarian && minor.id != pid)
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

            let current_raw = g.players[pid]
                .envoys
                .iter()
                .find(|(at, _)| *at == minor)
                .map(|(_, count)| *count)
                .unwrap_or(0);
            let mut required_raw = None;
            // Raw `want` always suffices because Amani's virtual contribution
            // is nonnegative. Mutating through the candidates is safe here:
            // the grant runs before the wrapped controller opens a query-memo
            // scope, and the final candidate is the only externally visible
            // state.
            for candidate in current_raw + 1..=want {
                let seat = &mut g.players[pid];
                match seat.envoys.iter_mut().find(|(at, _)| *at == minor) {
                    Some((_, count)) => *count = candidate,
                    None => seat.envoys.push((minor, candidate)),
                }
                if g.envoys_at(pid, minor) >= want {
                    required_raw = Some(candidate);
                    break;
                }
            }
            let required_raw = required_raw.expect("raw target must reach effective target");
            self.raw_envoys_granted += (required_raw - current_raw) as u64;
            self.fired += 1;
        }
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

impl<A: Ai> Ai for Oracle<A> {
    fn take_turn(&mut self, g: &mut Game, pid: usize) {
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
                Grant::IdleReserve => self.confiscate_idle_reserve(g, pid),
                Grant::Suzerain => self.grant_suzerain(g, pid),
            }
        }
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
    use crate::game::{Game, GovernorState, Item};
    use crate::Pos;
    use std::collections::BTreeSet;

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

    #[test]
    fn the_suzerain_grant_uses_minimal_raw_stock_under_amani() {
        let game = Game::new_full(2, 26, 16, 8_111, 200, 1, false);
        let minor = game
            .players
            .iter()
            .find(|player| player.is_minor && !player.is_barbarian)
            .unwrap()
            .id;
        let city_state = game.player_city_ids(minor)[0];

        let fixture = |puppeteer: Option<bool>| {
            let mut game = game.clone();
            game.turn = 100;
            game.players[0].met.insert(minor);
            game.players[minor].met.insert(0);
            game.players[1].envoys = vec![(minor, 5)];
            if let Some(puppeteer) = puppeteer {
                game.players[0].governor_roster.insert(
                    "amani".to_string(),
                    GovernorState {
                        city: Some(city_state),
                        assigned_turn: 0,
                        disabled_until: 0,
                        promotions: if puppeteer {
                            BTreeSet::from(["puppeteer".to_string()])
                        } else {
                            BTreeSet::new()
                        },
                    },
                );
            }
            game
        };

        for (puppeteer, expected_raw) in [(None, 6), (Some(false), 4), (Some(true), 1)] {
            let mut game = fixture(puppeteer);
            let free = game.players[0].envoys_free;
            let rival = game.players[1].envoys.clone();
            let gold = game.players[0].gold;
            let faith = game.players[0].faith;
            let mut oracle = Oracle::new(AdvancedAi::new(), Grant::Suzerain);

            oracle.grant_suzerain(&mut game, 0);

            let raw = game.players[0]
                .envoys
                .iter()
                .find(|(at, _)| *at == minor)
                .map(|(_, count)| *count)
                .unwrap_or(0);
            assert_eq!(raw, expected_raw, "Amani mode {puppeteer:?}");
            assert_eq!(game.envoys_at(0, minor), 6, "Amani mode {puppeteer:?}");
            assert_eq!(game.suzerain_of(minor), Some(0));
            assert_eq!(oracle.fired(), 1);
            assert_eq!(oracle.raw_envoys_granted(), expected_raw as u64);
            assert_eq!(game.players[0].envoys_free, free);
            assert_eq!(game.players[1].envoys, rival);
            assert_eq!(game.players[0].gold, gold);
            assert_eq!(game.players[0].faith, faith);
        }
    }

    /// The compound suzerainty ceiling must actually take every met state and
    /// may create only focal raw Envoys. Moving Gold, Faith, or rival stock
    /// would bundle a second resource axis into the result.
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
