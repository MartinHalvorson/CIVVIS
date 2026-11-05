//! Oracle ablation: find which subsystem actually limits the agent.
//!
//! Every attempt to strengthen this AI has failed one of two ways. The first
//! is documented in `docs/AI_GAPS.md`: the evaluator could not see the
//! decision it was ranking, so no amount of training could rank it. The
//! second is the same error one level up — the change addressed a constraint
//! that was not binding. The former scoped-relief experiment is the worked
//! example: it
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
use crate::ai::{Ai, PlanReport};
use crate::game::{Game, Item};
use crate::name::Name;
use crate::world::DistrictFoundation;
use crate::Pos;
use std::collections::BTreeSet;

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
                let kind = g.units[&uid].kind;
                let Some(target) = g.unit_upgrade_target(pid, kind) else {
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
                let name = foundation.district;
                let best = g
                    .district_sites(cid, name)
                    .into_iter()
                    .map(|pos| (Self::yield_total(g.district_yields(name, pos)), pos))
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
            let mut approaches: Vec<Pos> =
                g.nbrs(city_pos)
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

    /// An oracle's grants change the world, not the agent's mind; the
    /// reasoning an observer sees is the inner agent's, reading the granted
    /// position exactly as it reads any other.
    fn attach_journal(&mut self, journal: crate::reasoning::Journal) {
        self.inner.attach_journal(journal);
    }
}

#[cfg(test)]
mod tests;
