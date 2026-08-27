//! Opportunistic war: declare when the board offers a prize the peace does
//! not — an unescorted enemy Settler or Builder within a short march of one
//! of our soldiers, or a cluster of unpillaged improvements beside our army —
//! take it, and sue for peace.
//!
//! ★★★★ THE DECLARATION LOGIC NEVER LOOKED AT THE BOARD. Every road to war in
//! this controller prices the empires — `military_power` ratios, a staged
//! army at an objective city, a rival's victory clock — and none of them
//! reads the one thing a human looks for before a surprise war: what is lying
//! around unguarded. A captured Settler is a free city (the engine transfers
//! it whole, `Game::resolve_entered_units`, and `do_found_city` has no
//! provenance check); a captured Builder keeps its charges; pillage pays
//! `plunder × (era + 1)` and costs the victim the tile. All three are captures
//! by movement, no combat, and all three need a state of war. The formal
//! route — denounce, wait five standard turns, declare — cannot reach a prize
//! that moves two tiles a turn, so the opening here is the surprise war, the
//! bare `Action::DeclareWar`, and it is priced as one: 150 grievances instead
//! of 100, and a neighbour that answers.
//!
//! The raid is a *bounded* war. While the only major war is the raid, the
//! grand strategy keeps its economic plan (`assess` skips the "already at
//! war" Conquest branch and the power-gap Recovery half for it), the soldiers
//! within `RAID_PURSUIT_RADIUS` of a prize walk onto it or pillage under
//! their feet, and once the engine's minimum war length has passed with no
//! prize left in reach — or `RAID_MAX_TURNS` in any case — peace is proposed
//! every turn until it is accepted. The existing peace triggers (outmatched,
//! Recovery, stalled) stay live on top.
//!
//! Off by default: registry row `opportunistic-war`. It ships
//! into the genome unmeasured and stays off until a screen says it helps
//! (`gene_ledger`); `gene_screen --genes opportunistic-war` prices it. The
//! pillage half is its own opt-in, `raid-pillage-prizes`: with it off a raid
//! is priced and pursued on civilians alone (a soldier standing on an enemy
//! improvement still pillages it), so one screen of both genes says whether
//! the tiles are worth the wars they open.

use super::{AdvancedAi, GrandStrategy, StrategicPlan};
use crate::game::{Action, ActionFamilies, Game};
use crate::think;
use crate::Pos;

/// A prize must be within this many turns' march of one of our land soldiers
/// for the declaration to count it. The declaration lands in the diplomacy
/// pass, before any unit moves, so one turn means "captured this turn".
pub(crate) const RAID_STRIKE_TURNS: i32 = 2;
/// During a raid a soldier this close to a prize walks to it.
pub(crate) const RAID_PURSUIT_RADIUS: i32 = 6;
/// A captured Settler only counts when it stands this close to one of our
/// cities: that is the walk home, and the founded city's loyalty ground.
pub(crate) const RAID_SETTLER_HOME_RADIUS: i32 = 12;
/// What an unescorted enemy Settler is worth: the 80-production unit plus the
/// city it becomes, in the same units as the pillage prizes below.
pub(crate) const SETTLER_PRIZE: f64 = 160.0;
/// What one Builder charge is worth.
pub(crate) const BUILDER_PRIZE_PER_CHARGE: f64 = 14.0;
/// The floor of a pillage prize; each tile adds `PILLAGE_PRIZE_PER_ERA` per
/// world era past the first, because plunder scales with `world_era + 1`.
pub(crate) const PILLAGE_PRIZE_BASE: f64 = 10.0;
pub(crate) const PILLAGE_PRIZE_PER_ERA: f64 = 5.0;
/// A district layer pays the victim's yields twice over: its own and what the
/// buildings on it stop producing while pillaged.
pub(crate) const DISTRICT_PRIZE_MULTIPLIER: f64 = 2.0;
/// The prize total a surprise war must clear. One Settler clears it alone; a
/// Builder needs five classical improvements beside it; a pillage-only raid
/// needs six to eight tiles in reach.
pub(crate) const RAID_WAR_MIN_VALUE: f64 = 120.0;
/// The neighbour may be this much stronger than us and still be raided: the
/// prize is taken by movement, not by winning a war, but the answer to the
/// declaration is an army on our border, and it must not be a bigger one.
pub(crate) const RAID_POWER_RATIO: f64 = 1.10;
pub(crate) const RAID_POWER_MARGIN: f64 = 5.0;
/// No raid before this standard turn: the opening has one Warrior and a
/// Slinger, and a war that early costs the second city.
pub(crate) const RAID_MIN_TURN: u32 = 20;
/// Peace is proposed regardless of prizes once the raid is this old
/// (standard turns); a raid that has not paid by then is an ordinary war.
pub(crate) const RAID_MAX_TURNS: u32 = 20;
/// The engine's own minimum war length (`DIPLOMACY_WAR_MIN_TURNS`); peace
/// cannot be concluded before it, so it is not proposed before it either.
pub(crate) const RAID_PEACE_EARLIEST: u32 = 10;
/// After a raid closes, no war is opened for this many standard turns: the
/// grievances of two surprise wars in a row on one neighbour compound, and
/// a neighbour raided every treaty is a neighbour that arms.
pub(crate) const RAID_REPEAT_COOLDOWN: u32 = 20;

/// The raid this controller opened and has not yet closed.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RaidWar {
    /// The raided major.
    pub(crate) target: usize,
    /// The turn of the declaration.
    pub(crate) declared: u32,
    /// The prize total the declaration was priced at.
    pub(crate) value: f64,
    /// Prizes counted at the declaration: Settlers, Builders, pillage tiles.
    pub(crate) settlers: usize,
    pub(crate) builders: usize,
    pub(crate) pillage_tiles: usize,
}

/// One of our land soldiers as the raid sees it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct RaidStriker {
    pub(crate) uid: u32,
    pub(crate) pos: Pos,
    /// Tiles it covers in `RAID_STRIKE_TURNS`.
    pub(crate) reach: i32,
    /// The only soldier in one of our cities: reaches civilians, not tiles.
    pub(crate) lone_garrison: bool,
}

/// One thing worth a war, where it is, and what it is worth.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum RaidPrize {
    Settler { pos: Pos, value: f64 },
    Builder { pos: Pos, value: f64 },
    Pillage { pos: Pos, value: f64 },
}

impl RaidPrize {
    pub(crate) fn pos(self) -> Pos {
        match self {
            RaidPrize::Settler { pos, .. }
            | RaidPrize::Builder { pos, .. }
            | RaidPrize::Pillage { pos, .. } => pos,
        }
    }

    pub(crate) fn value(self) -> f64 {
        match self {
            RaidPrize::Settler { value, .. }
            | RaidPrize::Builder { value, .. }
            | RaidPrize::Pillage { value, .. } => value,
        }
    }
}

/// What a raid on one neighbour would put on the table.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RaidOpportunity {
    pub(crate) target: usize,
    pub(crate) value: f64,
    pub(crate) prizes: Vec<RaidPrize>,
}

impl RaidOpportunity {
    fn count(&self, settler: bool, builder: bool) -> usize {
        self.prizes
            .iter()
            .filter(|prize| match prize {
                RaidPrize::Settler { .. } => settler,
                RaidPrize::Builder { .. } => builder,
                RaidPrize::Pillage { .. } => !settler && !builder,
            })
            .count()
    }
}

impl AdvancedAi {
    /// Our land soldiers that could take part in a raid this turn, with the
    /// distance each can cover in `RAID_STRIKE_TURNS` and whether it is the
    /// lone garrison of one of our cities.
    fn raid_strikers(&self, g: &Game, pid: usize) -> Vec<RaidStriker> {
        g.player_unit_ids(pid)
            .into_iter()
            .filter_map(|uid| {
                let unit = &g.units[&uid];
                let spec = &g.rules.units[unit.kind];
                if spec.class != "military"
                    || spec.domain.as_deref() == Some("air")
                    || spec.domain.as_deref() == Some("sea")
                    || g.is_embarked(unit)
                    || unit.hp < 50
                {
                    return None;
                }
                let reach = (g.unit_max_moves(uid).floor() as i32).max(1) * RAID_STRIKE_TURNS;
                Some(RaidStriker {
                    uid,
                    pos: unit.pos,
                    reach,
                    lone_garrison: Self::lone_garrison(g, pid, uid),
                })
            })
            .collect()
    }

    /// Whether this soldier is the only military unit in one of our cities.
    /// A lone garrison leaves for a Settler, not for a tile of pillage: the
    /// answer to a raid is a counter-raid, and an empty city is its prize.
    pub(crate) fn lone_garrison(g: &Game, pid: usize, uid: u32) -> bool {
        let pos = g.units[&uid].pos;
        let Some(cid) = g.city_at(pos) else {
            return false;
        };
        g.cities.get(&cid).is_some_and(|city| city.owner == pid)
            && !g.unit_ids_at(pos).iter().any(|other| {
                *other != uid
                    && g.units[&other].owner == pid
                    && g.rules.units[g.units[&other].kind].class == "military"
            })
    }

    /// What one tile of pillage is worth this era.
    fn pillage_prize_value(g: &Game) -> f64 {
        PILLAGE_PRIZE_BASE + PILLAGE_PRIZE_PER_ERA * g.world_era as f64
    }

    /// Every prize a war on `target` would expose to our soldiers right now:
    /// visible unescorted Settlers and Builders, and the unpillaged
    /// improvements and districts of their cities, each within
    /// `RAID_STRIKE_TURNS` of one of our land soldiers.
    ///
    /// A Settler counts only when we could use it — no Settler of our own
    /// already walking (the capture guard `decline_settlers` would otherwise
    /// refuse the tile), a practical site to found, and a home city within
    /// `RAID_SETTLER_HOME_RADIUS`. A civilian under an enemy soldier is not a
    /// prize: entering that tile is combat, not capture.
    pub(crate) fn raid_prizes_against(
        &self,
        g: &Game,
        pid: usize,
        target: usize,
        strikers: &[RaidStriker],
    ) -> Vec<RaidPrize> {
        let mut prizes = Vec::new();
        if strikers.is_empty() {
            return prizes;
        }
        let in_reach = |pos: Pos, for_pillage: bool| {
            strikers.iter().any(|striker| {
                (!for_pillage || !striker.lone_garrison)
                    && g.wdist(striker.pos, pos) <= striker.reach
            })
        };
        let visible = self.battlefront_visibility(g, pid);
        let own_cities: Vec<Pos> = g
            .player_city_ids(pid)
            .into_iter()
            .map(|cid| g.cities[&cid].pos)
            .collect();
        let settler_usable =
            self.counts(g, pid).settlers == 0 && self.base.has_practical_settle_site(g, pid);
        for unit in g.units.values() {
            if unit.owner != target || !matches!(unit.kind.as_str(), "settler" | "builder") {
                continue;
            }
            if !g.sees(&visible, unit.pos)
                || !self.battlefront_unit_visible(g, pid, unit.id)
                || !in_reach(unit.pos, false)
            {
                continue;
            }
            // Under a soldier the tile is combat, not capture; beside one it
            // is an escorted civilian whose guard steps onto it or onto us.
            let guarded = g
                .wdisk(unit.pos, 1)
                .into_iter()
                .flat_map(|position| g.unit_ids_at(position))
                .any(|oid| {
                    let other = &g.units[&oid];
                    other.owner != pid
                        && !g.players[other.owner].is_barbarian
                        && g.rules.units[other.kind].class == "military"
                });
            if guarded || g.city_at(unit.pos).is_some() {
                continue;
            }
            match unit.kind.as_str() {
                "settler" => {
                    let near_home = own_cities
                        .iter()
                        .any(|home| g.wdist(*home, unit.pos) <= RAID_SETTLER_HOME_RADIUS);
                    if settler_usable && near_home {
                        prizes.push(RaidPrize::Settler {
                            pos: unit.pos,
                            value: SETTLER_PRIZE,
                        });
                    }
                }
                _ => {
                    if unit.charges > 0 {
                        prizes.push(RaidPrize::Builder {
                            pos: unit.pos,
                            value: BUILDER_PRIZE_PER_CHARGE * unit.charges as f64,
                        });
                    }
                }
            }
        }
        if !self.raid_pillage_prizes {
            return prizes;
        }
        let tile_value = Self::pillage_prize_value(g);
        let explored = &g.players[pid].explored;
        for cid in g.player_city_ids(target) {
            let city = &g.cities[&cid];
            for pos in city.owned_tiles.iter().copied() {
                if !explored.contains(&pos) || !in_reach(pos, true) {
                    continue;
                }
                let Some(tile) = g.map.get(pos) else {
                    continue;
                };
                if tile.owner_city != Some(cid) || !g.pillageable_after_declaring(pid, pos) {
                    continue;
                }
                let value = if tile.improvement.is_some() {
                    tile_value
                } else {
                    tile_value * DISTRICT_PRIZE_MULTIPLIER
                };
                prizes.push(RaidPrize::Pillage { pos, value });
            }
        }
        prizes
    }

    /// Whether a surprise war on `target` could legally be opened this turn
    /// and the neighbour is one this empire can afford to poke.
    fn raid_target_admissible(&self, g: &Game, pid: usize, target: usize) -> bool {
        let player = &g.players[target];
        if !player.alive
            || player.is_minor
            || player.is_barbarian
            || !g.has_met(pid, target)
            || g.is_at_war(pid, target)
            || !self.campaign_target_legal(g, pid, target)
        {
            return false;
        }
        let my_power = g.military_power(pid);
        g.military_power(target) <= my_power * RAID_POWER_RATIO + RAID_POWER_MARGIN
    }

    /// The best raid on the table this turn, if any clears the bar.
    pub(crate) fn raid_opportunity(&self, g: &Game, pid: usize) -> Option<RaidOpportunity> {
        if !self.opportunistic_war
            || self.raid_war.is_some()
            || g.turn < g.standard_duration(RAID_MIN_TURN)
            || g.turn < self.peace_until
            || g.player_city_ids(pid).len() < 2
        {
            return None;
        }
        // One war at a time: a raid is an opening, not a second front.
        if g.players.iter().any(|other| {
            other.id != pid && !other.is_minor && !other.is_barbarian && g.is_at_war(pid, other.id)
        }) {
            return None;
        }
        let strikers = self.raid_strikers(g, pid);
        if strikers.is_empty() {
            return None;
        }
        let legal: Vec<usize> = g
            .legal_actions_within(pid, ActionFamilies::DIPLOMACY)
            .into_iter()
            .filter_map(|action| match action {
                Action::DeclareWar { player } => Some(player),
                _ => None,
            })
            .collect();
        let mut best: Option<RaidOpportunity> = None;
        for target in legal {
            if !self.raid_target_admissible(g, pid, target) {
                continue;
            }
            let prizes = self.raid_prizes_against(g, pid, target, &strikers);
            let value: f64 = prizes.iter().map(|prize| prize.value()).sum();
            if value + 1e-9 < RAID_WAR_MIN_VALUE {
                continue;
            }
            if best.as_ref().is_none_or(|current| value > current.value) {
                best = Some(RaidOpportunity {
                    target,
                    value,
                    prizes,
                });
            }
        }
        best
    }

    /// The declaration itself: a casus belli if one happens to be legal (a
    /// matured denouncement, a reconquest), otherwise the surprise war.
    fn raid_opening(&self, g: &Game, pid: usize, target: usize) -> Option<Action> {
        let legal = g.legal_actions_within(pid, ActionFamilies::DIPLOMACY);
        if let Some(action) = self.preferred_war_opening(g, pid, target) {
            if matches!(action, Action::DeclareWarWithCasusBelli { .. }) {
                return Some(action);
            }
        }
        legal
            .into_iter()
            .find(|action| matches!(action, Action::DeclareWar { player } if *player == target))
    }

    /// Whether the only major war this empire is in is its own raid.
    pub(crate) fn raid_only_war(&self, g: &Game, pid: usize) -> bool {
        let Some(raid) = self.raid_war.as_ref() else {
            return false;
        };
        g.players.iter().all(|other| {
            other.id == pid
                || other.is_minor
                || other.is_barbarian
                || other.id == raid.target
                || !g.is_at_war(pid, other.id)
        })
    }

    /// The diplomacy pass of the raid: close a finished one, open a new one.
    /// Returns `true` when a declaration was made this turn, so the elective
    /// war path does not also declare.
    pub(crate) fn opportunistic_war_diplomacy(
        &mut self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> bool {
        if !self.opportunistic_war {
            self.raid_war = None;
            return false;
        }
        if let Some(raid) = self.raid_war.clone() {
            if !g.is_at_war(pid, raid.target) || !g.players[raid.target].alive {
                // Peace was concluded (by either side) or the target is gone.
                // The stand-down mirrors an accepted peace offer's: no fresh
                // war for a while, this one's grievances still warm.
                self.raid_war = None;
                self.peace_until = self.peace_until.max(
                    g.turn
                        .saturating_add(g.standard_duration(RAID_REPEAT_COOLDOWN)),
                );
                self.major_war_since = None;
                return false;
            }
            self.close_raid_when_paid(g, pid, plan, &raid);
            return false;
        }
        let Some(opportunity) = self.raid_opportunity(g, pid) else {
            return false;
        };
        let Some(action) = self.raid_opening(g, pid, opportunity.target) else {
            return false;
        };
        let settlers = opportunity.count(true, false);
        let builders = opportunity.count(false, true);
        let pillage_tiles = opportunity.count(false, false);
        if self.journal().wants(crate::reasoning::Level::Strategy) {
            let my_power = g.military_power(pid);
            let their_power = g.military_power(opportunity.target);
            let casus = match &action {
                Action::DeclareWarWithCasusBelli { casus_belli, .. } => {
                    format!(
                        " under a {} casus belli",
                        crate::reasoning::plain(casus_belli)
                    )
                }
                _ => " by surprise".to_string(),
            };
            think!(self.journal(), Military, Strategy,
                   "Declaring war on {}{casus}", g.players[opportunity.target].civ;
                   "an opportunity worth {:.0}: {settlers} unescorted Settler{}, {builders} Builder{}, \
                    {pillage_tiles} tile{} to pillage, all within {RAID_STRIKE_TURNS} turns of our soldiers; \
                    {my_power:.0} power against their {their_power:.0}",
                   opportunity.value,
                   if settlers == 1 { "" } else { "s" },
                   if builders == 1 { "" } else { "s" },
                   if pillage_tiles == 1 { "" } else { "s" });
        }
        self.base.war_eve_liquidation(g, pid, &action);
        if g.apply(pid, &action).is_err() {
            return false;
        }
        // The seat's own record of the raids it opened, beside the engine's
        // `captured:*` and `pillages` counters, so an evaluator row can
        // report how often the gene fired and what it took.
        for (key, count) in [
            ("raid_wars", 1),
            ("raid_prize:settler", settlers),
            ("raid_prize:builder", builders),
            ("raid_prize:pillage", pillage_tiles),
        ] {
            *g.players[pid].counters.entry(key.to_string()).or_insert(0) += count as i64;
        }
        self.raid_war = Some(RaidWar {
            target: opportunity.target,
            declared: g.turn,
            value: opportunity.value,
            settlers,
            builders,
            pillage_tiles,
        });
        self.major_war_since = Some(g.turn);
        self.war_census.declarations += 1;
        true
    }

    /// Propose peace once the raid has paid — the engine's minimum war length
    /// has passed and nothing is left in reach — or has run its course.
    fn close_raid_when_paid(
        &mut self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
        raid: &RaidWar,
    ) {
        let age = g.turn.saturating_sub(raid.declared);
        if age < g.standard_duration(RAID_PEACE_EARLIEST) {
            return;
        }
        let expired = age >= g.standard_duration(RAID_MAX_TURNS);
        let prizes_left = if expired {
            0
        } else {
            let strikers = self.raid_strikers(g, pid);
            self.raid_prizes_against(g, pid, raid.target, &strikers)
                .into_iter()
                .filter(|prize| !matches!(prize, RaidPrize::Pillage { .. }))
                .count()
                + self.pillage_tiles_under_our_soldiers(g, pid, raid.target)
        };
        // A raid that turned into a real war — the plan now wants that
        // city — is the elective machinery's to finish.
        let campaign_wants_it = plan.strategy == GrandStrategy::Conquest
            && plan.target_player == Some(raid.target)
            && plan
                .target_city
                .and_then(|cid| g.cities.get(&cid))
                .is_some_and(|city| city.owner == raid.target)
            && self.last_campaign_progress >= raid.declared;
        if prizes_left > 0 || campaign_wants_it {
            return;
        }
        let peace_pending = g.pending_deals.iter().any(|deal| {
            deal.peace
                && ((deal.from == pid && deal.to == raid.target)
                    || (deal.from == raid.target && deal.to == pid))
                && deal.expires >= g.turn
        });
        if peace_pending || self.peace_offers.contains(&raid.target) {
            return;
        }
        think!(self.journal(), Diplomacy, Decision,
               "Offering peace to {}", g.players[raid.target].civ;
               "the raid that opened the war on turn {} has {}; {} Settler{}, {} Builder{} and {} pillage tile{} were its prizes",
               raid.declared,
               if expired { "run its course" } else { "nothing left in reach" },
               raid.settlers, if raid.settlers == 1 { "" } else { "s" },
               raid.builders, if raid.builders == 1 { "" } else { "s" },
               raid.pillage_tiles, if raid.pillage_tiles == 1 { "" } else { "s" });
        self.peace_offers.insert(raid.target);
        let _ = g.apply(
            pid,
            &Action::ProposeDeal {
                player: raid.target,
                give_gold: 0.0,
                request_gold: 0.0,
                open_borders: false,
                friendship: false,
                peace: true,
                alliance: None,
            },
        );
    }

    /// Pillage tiles of the raid target within one turn of our soldiers.
    fn pillage_tiles_under_our_soldiers(&self, g: &Game, pid: usize, target: usize) -> usize {
        let strikers: Vec<RaidStriker> = self
            .raid_strikers(g, pid)
            .into_iter()
            .map(|striker| RaidStriker {
                reach: striker.reach / RAID_STRIKE_TURNS,
                ..striker
            })
            .collect();
        self.raid_prizes_against(g, pid, target, &strikers)
            .into_iter()
            .filter(|prize| matches!(prize, RaidPrize::Pillage { .. }))
            .count()
    }

    /// The raid's unit step: pillage under our feet, or walk to the nearest
    /// prize within `RAID_PURSUIT_RADIUS`. `None` when this unit has no part
    /// in the raid this turn.
    pub(crate) fn raid_prize_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        plan: &StrategicPlan,
        decline_settlers: bool,
    ) -> Option<bool> {
        let raid = self.raid_war.clone()?;
        if !self.opportunistic_war || !g.is_at_war(pid, raid.target) {
            return None;
        }
        let unit = g.units.get(&uid)?.clone();
        let spec = &g.rules.units[unit.kind];
        if spec.class != "military"
            || spec.domain.as_deref() == Some("air")
            || spec.domain.as_deref() == Some("sea")
            || g.is_embarked(&unit)
            || unit.moves_left <= 0.0
        {
            return None;
        }
        // A soldier holding a threatened city holds it; the raid is not a
        // reason to open the gate.
        if plan.threatened_city.is_some_and(|cid| {
            g.cities
                .get(&cid)
                .is_some_and(|city| g.wdist(unit.pos, city.pos) <= 3)
        }) {
            return None;
        }
        // Standing on their improvement: pillage it.
        let on_prize = g
            .map
            .get(unit.pos)
            .and_then(|tile| tile.owner_city)
            .and_then(|cid| g.cities.get(&cid))
            .is_some_and(|city| city.owner == raid.target)
            && g.pillageable_at(pid, unit.pos);
        if on_prize {
            let kind = unit.kind.as_str();
            think!(self.journal(), Military, Decision,
                   "{kind} {uid} pillages the tile under it";
                   "a raid prize of {}", g.players[raid.target].civ;
                   unit.pos);
            return Some(g.apply(pid, &Action::Pillage { unit: uid }).is_ok());
        }
        // A guard standing with its own Settler does not leave it for a prize.
        let beside_own_settler = g.unit_ids_at(unit.pos).iter().any(|other| {
            *other != uid && g.units[other].owner == pid && g.units[other].kind == "settler"
        });
        if beside_own_settler {
            return None;
        }
        let strikers = [RaidStriker {
            uid,
            pos: unit.pos,
            reach: RAID_PURSUIT_RADIUS,
            lone_garrison: Self::lone_garrison(g, pid, uid),
        }];
        let goal = self
            .raid_prizes_against(g, pid, raid.target, &strikers)
            .into_iter()
            .filter(|prize| !(decline_settlers && matches!(prize, RaidPrize::Settler { .. })))
            .map(|prize| {
                let distance = g.wdist(unit.pos, prize.pos());
                (prize.value() / (distance as f64 + 1.0), prize.pos())
            })
            .max_by(|a, b| a.0.total_cmp(&b.0).then_with(|| b.1.cmp(&a.1)))
            .map(|(_, pos)| pos)?;
        let next = g
            .route_step(uid, goal, 0)
            .filter(|next| g.can_move(uid, *next))?;
        let kind = unit.kind.as_str();
        think!(self.journal(), Military, Decision,
               "{kind} {uid} marches on a raid prize";
               "{} has something unguarded {} tiles away", g.players[raid.target].civ, g.wdist(unit.pos, goal);
               goal);
        Some(
            g.apply(
                pid,
                &Action::Move {
                    unit: uid,
                    to: next,
                },
            )
            .is_ok(),
        )
    }
}
