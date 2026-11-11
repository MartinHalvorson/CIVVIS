//! `city-state-harvest`: take a soft city-state city beside the empire.
//!
//! ## Why (live Immortal, 2026-11-02 20:00Z-23:40Z, 17 games)
//!
//! The Lv 7 gap is population, not science per citizen. At turn 100 the
//! median rival held 56 citizens in 7 cities against our 36 in 5, while its
//! science per citizen (2.07 against 1.61) sat within the Immortal +24%
//! handicap. None of the 17 games captured a city. At turn 80 the median seat
//! had two city-state cities within eight tiles of its own, at population
//! 5-12 (our own cities averaged about 5.5), whose public military was 18-110
//! against our 240-566. Nothing in the controller aims at one for its own
//! sake: an assigned Domination seat prefers capitals by design
//! (`domination_chooses_a_required_capital_before_a_cheaper_city_state`), and
//! the opening conquest (`early_conquest.rs`) and the city campaign
//! (`city_campaign.rs`) skip minors.
//!
//! A city-state carries no major-civilization handicap (the
//! `HIGH_DIFFICULTY_*` modifiers attach to `TRAIT_LEADER_MAJOR_CIV`), so its
//! garrison fights at par; at Immortal and above it starts walled
//! (`Eras.xml`: `MinorOnly="true" MinDifficulty="DIFFICULTY_IMMORTAL"
//! Building="BUILDING_WALLS"`).
//!
//! ## What it does
//!
//! While no other overlay owns the war (no timed-war appointment), no war
//! with a major is running and the empire is past [`HARVEST_FROM_TURN`] with
//! two cities, it names the best city that is (a city-state's only from
//! [`HARVEST_CITY_STATE_FROM_TURN`], after the expansion window, and never
//! on a Science seat):
//!
//! - owned by a city-state we have MET, or by the Free Cities (a city that
//!   rebelled on loyalty: across 72 recorded runs of 11-02, 24 of our cities
//!   flipped and 8 were taken back), its centre EXPLORED (fog-honest: the
//!   `players[pid].explored` idiom `early_conquest.rs` uses);
//! - within [`HARVEST_REACH_TILES`] of one of our cities;
//! - at population [`HARVEST_MIN_POP`] or more;
//! - held by a city-state with no suzerain but us, at most
//!   [`HARVEST_MAX_OUR_ENVOYS`] of our envoys (an invested suzerainty is
//!   worth more than the city: a scientific city-state with nine of our
//!   envoys pays about +16 Science), and public military at most
//!   [`HARVEST_MAX_POWER_SHARE`] of ours.
//!
//! The lane keeps its own grand strategy. Version one turned the plan into a
//! Conquest for the window, and the paired Immortal read showed what that
//! costs: in the pairs where it fired, science at turn 150 fell 169 -> 138
//! and 129 -> 84 (policies, government and research all follow the Conquest
//! posture) while one pop-seven city came back. Instead the harvest declares
//! itself, through the shipped `preferred_war_opening`, once
//! [`HARVEST_MUSTER_BODIES`] land units of ours stand within
//! [`HARVEST_MUSTER_TILES`] of the city with [`HARVEST_LOCAL_EDGE`] times its
//! owner's public power; and only while we are at war with it does
//! [`AdvancedAi::apply_city_state_harvest_to_strategy`] point the plan's
//! target at the city, which is what the objective board's Siege row, the
//! siege train and the breaker purchase read. A city-state war is never
//! offered peace by the shipped desk (its gate skips minors), so the siege
//! runs until the city falls or the harvest releases it.
//!
//! The name is released when the city changes hands, when a major is at war
//! with us, when no muster formed within [`HARVEST_DECLARE_PATIENCE`] turns,
//! or when the war has run [`HARVEST_SIEGE_PATIENCE`] turns without the
//! capture. A release without a capture waits [`HARVEST_COOLDOWN`] turns
//! before another name.
//!
//! Off, every entry point returns before reading the board.

use super::outclassed_siege::{field_outclasses, STREAK_TURNS};
use super::*;
use crate::think;

/// The earliest turn any harvest is named (Online speed): a rebel Free City.
pub(crate) const HARVEST_FROM_TURN: u32 = 40;
/// The earliest turn a CITY-STATE city is named. The paired Immortal read of
/// version two (2026-11-03, live genome ± the gene, seeds 43540000+) fired
/// in three of thirteen pairs and took a city in each, but where it fired
/// inside the expansion window it cost the economy: declared at turn 50,
/// science at turn 150 169 -> 132 and score share .159 -> .140 (the city
/// count fell behind from turn 60); at turn 80, science at 100 76 -> 45 and
/// share .119 -> .112; at turn 125, science 45 -> 49 and share .092 -> .094.
pub(crate) const HARVEST_CITY_STATE_FROM_TURN: u32 = 100;

/// The two turn floors above were read on Online games (250 turns). The live
/// lane moved to Standard on 2026-11-03 (500 turns; the operator doubled the
/// retire turn for the same reason), so a floor is the same point of the game
/// at every speed: the Online turn times the speed's turn budget over
/// Online's (Quick 330, Standard 500, Epic 750, Marathon 1500). Patience and
/// cooldown stay in turns: a march and a siege take the same turns at any
/// speed.
pub(crate) fn harvest_floor(g: &Game, online_turns: u32) -> u32 {
    use crate::setup::GameSpeed;
    let budget = match g.game_speed {
        GameSpeed::Online => 250,
        GameSpeed::Quick => 330,
        GameSpeed::Standard => 500,
        GameSpeed::Epic => 750,
        GameSpeed::Marathon => 1500,
    };
    online_turns * budget / 250
}
/// Tiles from the nearest of our cities to the city-state city.
pub(crate) const HARVEST_REACH_TILES: i32 = 6;
/// The city-state city's least population worth a war.
pub(crate) const HARVEST_MIN_POP: i32 = 4;
/// Our envoys at the city-state above which its suzerainty is kept.
pub(crate) const HARVEST_MAX_OUR_ENVOYS: i64 = 1;
/// The city-state's public military as a share of ours.
pub(crate) const HARVEST_MAX_POWER_SHARE: f64 = 0.35;
/// Turns from the name to the declaration before the name is released.
pub(crate) const HARVEST_DECLARE_PATIENCE: u32 = 10;
/// Our land units that must stand near the city before we declare.
pub(crate) const HARVEST_MUSTER_BODIES: usize = 3;
/// Tiles from the city within which a land unit counts toward the muster.
pub(crate) const HARVEST_MUSTER_TILES: i32 = 6;
/// The muster's strength as a multiple of the owner's public power.
pub(crate) const HARVEST_LOCAL_EDGE: f64 = 2.0;
/// Turns from the declaration to the capture before the name is released.
pub(crate) const HARVEST_SIEGE_PATIENCE: u32 = 20;
/// Turns after a release without a capture before another name.
pub(crate) const HARVEST_COOLDOWN: u32 = 25;

/// The city-state city the harvest is aimed at.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct HarvestTarget {
    pub(crate) player: usize,
    pub(crate) city: u32,
    pub(crate) named_turn: u32,
    pub(crate) declared_turn: Option<u32>,
    /// The city's tile and its owner's civilization at the naming. Live, the
    /// mirror's city ids and minor seats are not stable from one decision
    /// frame to the next (live Immortal 11-03: 115249Z named Kwila, 121631Z
    /// Madrid, 122920Z Tutub, and each released "the city changed hands" in
    /// the same turn, 122920Z's release naming Cardiff), so the target is
    /// re-read from these every frame.
    pub(crate) pos: Pos,
    pub(crate) owner_civ: String,
    /// Turns running on which the city's visible field outclassed our army
    /// (`outclassed_siege::field_outclasses`), counted once a turn.
    pub(crate) outclassed_turns: u32,
    pub(crate) outclass_seen_turn: u32,
}

/// The living player of civilization `civ`, the stable key for an owner.
fn player_by_civ(g: &Game, civ: &str) -> Option<usize> {
    g.players
        .iter()
        .find(|player| player.alive && player.civ == civ)
        .map(|player| player.id)
}

impl HarvestTarget {
    /// Re-read the city id and owner seat from the tile and the owner's
    /// civilization; returns the tile's current owner.
    fn resolve(&mut self, g: &Game) -> Option<usize> {
        let cid = g.city_at(self.pos)?;
        self.city = cid;
        let owner = g.cities.get(&cid)?.owner;
        if g.players
            .get(owner)
            .is_some_and(|p| p.civ == self.owner_civ)
        {
            self.player = owner;
        }
        Some(owner)
    }
}

impl AdvancedAi {
    fn harvest_count(g: &mut Game, pid: usize, key: &str) {
        *g.players[pid]
            .counters
            .entry(format!("city_state_harvest:{key}"))
            .or_insert(0) += 1;
    }

    fn harvest_major_war(g: &Game, pid: usize) -> bool {
        g.players.iter().any(|player| {
            player.id != pid
                && player.alive
                && !player.is_minor
                && !player.is_barbarian
                && g.is_at_war(pid, player.id)
        })
    }

    /// Our land military bodies within the muster radius of `city`, and
    /// their strength.
    fn harvest_muster(g: &Game, pid: usize, city: u32) -> (usize, f64) {
        let Some(target) = g.cities.get(&city) else {
            return (0, 0.0);
        };
        g.units
            .values()
            .filter(|unit| {
                unit.owner == pid
                    && g.rules.units[unit.kind].class == "military"
                    && !matches!(
                        g.rules.units[unit.kind].domain.as_deref(),
                        Some("sea" | "air")
                    )
                    && g.wdist(unit.pos, target.pos) <= HARVEST_MUSTER_TILES
            })
            .fold((0, 0.0), |(bodies, power), unit| {
                (
                    bodies + 1,
                    power + g.rules.units[unit.kind].strength * unit.hp as f64 / 100.0,
                )
            })
    }

    /// The softest qualifying city-state city, or `None`.
    pub(crate) fn city_state_harvest_candidate(
        &self,
        g: &Game,
        pid: usize,
    ) -> Option<(usize, u32)> {
        let ours = g.player_city_ids(pid);
        if ours.len() < 2 {
            return None;
        }
        let our_power = g.military_power(pid);
        if our_power <= 0.0 {
            return None;
        }
        let mut best: Option<(f64, usize, u32)> = None;
        // A city-state waits for the expansion window to close, and a Science
        // seat (the lane of the first Lv 7 non-Diplomatic win, 025902Z) never
        // opens a city-state war: a Campus plan is where the war costs most.
        let city_states_open = g.turn >= harvest_floor(g, HARVEST_CITY_STATE_FROM_TURN)
            && self.active_victory_target(g) != Some(VictoryTarget::Science);
        // A Free Cities seat carries `is_barbarian` and is always at war; a
        // city of ours that rebelled into it is the cheapest city on the
        // board (live Immortal 221554Z: Okayama, population 7, flipped at
        // turn 90 with no defender and was never retaken).
        for minor in g.players.iter().filter(|p| {
            p.alive && ((city_states_open && p.is_minor && !p.is_barbarian) || p.is_free_city)
        }) {
            let m = minor.id;
            if !g.has_met(pid, m) {
                continue;
            }
            if g.suzerain_of(m).is_some_and(|suzerain| suzerain != pid) {
                continue;
            }
            if g.envoys_at(pid, m) > HARVEST_MAX_OUR_ENVOYS {
                continue;
            }
            if g.military_power(m) > HARVEST_MAX_POWER_SHARE * our_power {
                continue;
            }
            if !g.is_at_war(pid, m) && !self.campaign_target_legal(g, pid, m) {
                continue;
            }
            for cid in g.player_city_ids(m) {
                let city = &g.cities[&cid];
                if city.pop < HARVEST_MIN_POP || !g.players[pid].explored.contains(&city.pos) {
                    continue;
                }
                // An army the city's own field out-classes cannot take it:
                // live 224444Z's harvest declared on Johannesburg, whose field
                // cannons one-shot our catapults, and the siege never formed.
                // Total power (73 against our 400) could not see it. See
                // `advanced/outclassed_siege.rs`.
                if field_outclasses(g, pid, m, city.pos) {
                    continue;
                }
                let distance = ours
                    .iter()
                    .map(|own| g.wdist(g.cities[own].pos, city.pos))
                    .min()
                    .unwrap_or(i32::MAX);
                if distance > HARVEST_REACH_TILES {
                    continue;
                }
                // Population is the prize; distance and walls are the bill.
                let score = city.pop as f64 * 10.0
                    - distance as f64 * 4.0
                    - city.wall_hp.max(0) as f64 / 20.0
                    - g.military_power(m) / our_power * 30.0;
                let better = best.as_ref().is_none_or(|(top, top_m, top_c)| {
                    score > *top || (score == *top && (m, cid) < (*top_m, *top_c))
                });
                if better {
                    best = Some((score, m, cid));
                }
            }
        }
        best.map(|(_, m, cid)| (m, cid))
    }

    /// Name, keep or release the harvest target, before the plan is read.
    pub(crate) fn maintain_city_state_harvest(&mut self, g: &mut Game, pid: usize) {
        if !self.city_state_harvest {
            return;
        }
        // The harvest's war outlives its name: the shipped peace desk offers a
        // minor no peace, so a siege that ran out of patience leaves the war
        // standing. Forget it only once it is over.
        if self
            .city_state_harvest_war
            .as_deref()
            .is_none_or(|civ| player_by_civ(g, civ).is_none_or(|cs| !g.is_at_war(pid, cs)))
        {
            self.city_state_harvest_war = None;
        }
        if let Some(mut target) = self.city_state_harvest_target.take() {
            let owner = target.resolve(g);
            let at_war = g.is_at_war(pid, target.player);
            // A declaration the host refused. Live, `civvis_orders` applies
            // the opening to its own model, where it holds for the frame,
            // and the host's `CanDeclareWarOn` can still say no (a city-state
            // peace treaty the mirror does not carry: live 152836Z declared
            // on Bandar Brunei every frame t175-185 against `war_refused`).
            // The next frame reads no war, so the name goes.
            let refused = target.declared_turn.is_some() && !at_war && owner == Some(target.player);
            let outclassed =
                owner == Some(target.player) && field_outclasses(g, pid, target.player, target.pos);
            if target.outclass_seen_turn != g.turn {
                target.outclass_seen_turn = g.turn;
                target.outclassed_turns = if outclassed {
                    target.outclassed_turns + 1
                } else {
                    0
                };
            }
            if at_war && target.declared_turn.is_none() {
                target.declared_turn = Some(g.turn);
                Self::harvest_count(g, pid, "declared");
            }
            if !at_war
                && !refused
                && !outclassed
                && owner == Some(target.player)
                && !Self::harvest_major_war(g, pid)
            {
                let (bodies, power) = Self::harvest_muster(g, pid, target.city);
                let theirs = g.military_power(target.player).max(1.0);
                if bodies >= HARVEST_MUSTER_BODIES && power >= HARVEST_LOCAL_EDGE * theirs {
                    if let Some(opening) = self.preferred_war_opening(g, pid, target.player) {
                        if g.apply(pid, &opening).is_ok() && g.is_at_war(pid, target.player) {
                            target.declared_turn = Some(g.turn);
                            self.city_state_harvest_war = Some(target.owner_civ.clone());
                            Self::harvest_count(g, pid, "declared");
                            self.plan = None;
                            if self.journal().wants(crate::reasoning::Level::Strategy) {
                                let c = &g.cities[&target.city];
                                think!(self.journal(), Military, Strategy,
                                       "City-state harvest declares on {}", c.name;
                                       "{bodies} bodies within {HARVEST_MUSTER_TILES} tiles at \
                                        {power:.0} strength against its {theirs:.0}"; c.pos);
                            }
                        }
                    }
                }
            }
            let release = if owner == Some(pid) {
                Self::harvest_count(g, pid, "captured");
                self.city_state_harvest_cooldown = 0;
                Some("the city is ours")
            } else if owner != Some(target.player) {
                Some("the city changed hands")
            } else if refused {
                Self::harvest_count(g, pid, "refused");
                Some("the host refused the war")
            } else if target.outclassed_turns >= STREAK_TURNS {
                // A streak, as the siege board's own stand-down reads it: one
                // reading would have forfeited 7 of 13 live captures there.
                Self::harvest_count(g, pid, "outclassed");
                Some("the city's field outclasses our army")
            } else if Self::harvest_major_war(g, pid) {
                Some("a major is at war with us")
            } else if target.declared_turn.is_none()
                && g.turn >= target.named_turn + HARVEST_DECLARE_PATIENCE
            {
                Some("no muster formed within the patience")
            } else if target
                .declared_turn
                .is_some_and(|declared| g.turn >= declared + HARVEST_SIEGE_PATIENCE)
            {
                Some("the siege ran out of patience")
            } else {
                None
            };
            if let Some(why) = release {
                if owner != Some(pid) {
                    Self::harvest_count(g, pid, "released");
                    self.city_state_harvest_cooldown = g.turn + HARVEST_COOLDOWN;
                }
                if self.journal().wants(crate::reasoning::Level::Strategy) {
                    let name = g
                        .cities
                        .get(&target.city)
                        .map(|city| city.name.clone())
                        .unwrap_or_default();
                    think!(self.journal(), Military, Strategy,
                           "City-state harvest releases {}", name;
                           "{why}, {} turns after it was named", g.turn - target.named_turn);
                }
                self.plan = None;
            } else {
                self.city_state_harvest_target = Some(target);
            }
            return;
        }
        // `city-state-before-the-first-capital`: no second city-state name
        // while the opening stands on one. `false` with that gene off.
        if g.turn < harvest_floor(g, HARVEST_FROM_TURN)
            || g.turn < self.city_state_harvest_cooldown
            || self.city_state_stone_stands()
            || self.war_plan.is_some()
            || Self::harvest_major_war(g, pid)
            || g.emergency_objective(pid).is_some()
        {
            return;
        }
        let Some((player, city)) = self.city_state_harvest_candidate(g, pid) else {
            return;
        };
        Self::harvest_count(g, pid, "named");
        if self.journal().wants(crate::reasoning::Level::Strategy) {
            let c = &g.cities[&city];
            think!(self.journal(), Military, Strategy,
                   "City-state harvest names {}", c.name;
                   "pop {}, walls {}, its military {:.0} against our {:.0}, {} of our envoys",
                   c.pop, c.wall_hp, g.military_power(player), g.military_power(pid),
                   g.envoys_at(pid, player); c.pos);
        }
        self.city_state_harvest_target = Some(HarvestTarget {
            player,
            city,
            named_turn: g.turn,
            declared_turn: g.is_at_war(pid, player).then_some(g.turn),
            pos: g.cities[&city].pos,
            owner_civ: g.players[player].civ.clone(),
            outclassed_turns: 0,
            outclass_seen_turn: g.turn,
        });
        self.plan = None;
    }

    /// The overlay: while the harvest's war runs, the plan's target is that
    /// city, under the lane's own grand strategy. A threatened city of ours
    /// keeps `Recovery` and its own target, as does a timed-war appointment.
    ///
    /// ★ A Recovery with no threatened city is lifted. `assess` reads
    /// Recovery's power gap against the strongest MET major, at peace or not
    /// (`recovery_opponent_power` is `strongest_rival` unless
    /// `recovery-reads-the-war` is on), so declaring on a city-state of 73
    /// power put a 400-power Culture seat into Recovery because a 905-power
    /// major existed elsewhere (siege_replay 224444Z t80 → t122, "at war and
    /// losing ground at home"). That posture, not the city, is what the
    /// paired reads priced. The harvest war is the only war while it runs (a
    /// major war releases it), so the gap is not ours to close.
    pub(crate) fn apply_city_state_harvest_to_strategy(
        &self,
        g: &Game,
        pid: usize,
        plan: &mut StrategicPlan,
    ) {
        if !self.city_state_harvest || self.war_plan.is_some() {
            return;
        }
        let named = self
            .city_state_harvest_target
            .as_ref()
            .filter(|target| target.declared_turn.is_some());
        if named.is_none() && self.city_state_harvest_war.is_none() {
            return;
        }
        if plan.strategy == GrandStrategy::Recovery {
            if plan.threatened_city.is_some() || Self::harvest_major_war(g, pid) {
                return;
            }
            let Some(lane) = self.active_victory_target(g) else {
                return;
            };
            plan.strategy = lane.strategy();
            if self.journal().wants(crate::reasoning::Level::Strategy) {
                think!(self.journal(), Strategy, Strategy,
                       "City-state harvest keeps the {} lane", plan.strategy.as_str();
                       "the Recovery read a power gap to a major we are not fighting; \
                        no city of ours is threatened");
            }
        }
        if let Some(target) = named {
            plan.target_player = Some(target.player);
            plan.target_city = Some(target.city);
        }
    }
}

#[cfg(test)]
mod tests;
