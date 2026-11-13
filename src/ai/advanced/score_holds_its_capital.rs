//! `score-seat-holds-its-capital` (opt-in): a Score seat walls its capital
//! and its frontier cities tier by tier, and keeps a shooter in each of
//! their City Centers.
//!
//! **Why (live Deity Rome, 6-player Continents, Online, turn limit 170,
//! Score target).** Rivals declared on the seat in most games; it declared
//! none. Rome itself fell in 4 of 8 games from `civvis-20261112T235738Z`
//! against 2 of the 21 before, each loss costing 250-400 points:
//!
//! | run | war | power | Rome fell | walls | centre at the declaration |
//! |---|---|---|---|---|---|
//! | `235738Z` | Hungary t149 | 1,921 vs 475 | t153 | Ancient t33; Castles known t132, no Castle | a Builder and an AT Crew |
//! | `003251Z` | England t53 | 805 vs 217 | t65 | Ancient t32 (no Castles) | empty |
//! | `011722Z` | Gran Colombia t89 | 689 vs 407 | t92 | Ancient t26; Castles known t71, no Castle | empty; Angkor Wat in the queue |
//! | `020922Z` | Egypt t75 | 584 vs 409 | t120 | Ancient t35; Castles known t83, no Castle | empty |
//!
//! The Ancient Walls stood in every case: `capital-walls-before-the-surprise`
//! and `frontier-walls-before-the-surprise` build them on a sighting. Nothing
//! builds the next tier at peace, so a capital with Castles in hand for 18 to
//! 37 turns met the declaration behind 100 points of Ancient Walls, and in
//! `011722Z` the threat handoff paused Angkor Wat for Medieval Walls on the
//! turn of the declaration, three turns before the city fell. In three of the
//! four the City Center held no soldier when the war came.
//!
//! **The gene.** While the active victory target is Score:
//!
//! 1. **Held cities.** The original capital from
//!    [`SCORE_CAPITAL_WALLS_STANDARD`] standard turns (turn 30 at Online), and
//!    any city of ours within [`SCORE_FRONTIER_TILES`] tiles of a living rival
//!    major's city.
//! 2. **Walls, tier by tier.** A held city's next Walls tier it can build
//!    (Ancient, Medieval, Renaissance, or the civilization's own) goes to the
//!    head of its queue. The next tier is legal only over full-health walls,
//!    so a damaged wall's repair comes first.
//! 3. **A shooter in the centre.** A held city with no ranged land soldier
//!    of ours within [`SCORE_SHOOTER_REACH`] tiles of its centre trains its
//!    best one, after its Walls. A ranged land soldier standing in a held
//!    City Center keeps that post, at peace as at war: it fires from the
//!    centre and makes no other move (`centre-keeps-its-garrison`'s keeper
//!    step, which this gene opens for these posts).
//!
//! Both go ahead of a wonder, a district, a building that is not a defence
//! and a repeatable project, and of a Settler from the empire's third city on
//! (counting Settlers walking) while at most half of it is paid; the first two
//! Settlers keep their queue, as do the early Settler floor's. A Builder, a
//! Trader, a soldier, Walls, a repair, anything finishing this turn and saved
//! work within [`super::near_done_work_finishes::NEAR_DONE_MAX_TURNS`] turns
//! of done keep theirs. The displaced item keeps its progress. The governor's
//! rescoring, the bound Granary and capped Aqueduct claims, the Campus and
//! Monument claims, the peace Commercial Hub claim and the Score wonder
//! claim then leave the claimed item alone
//! ([`AdvancedAi::score_capital_holds`]). It declares nothing and moves no
//! unit out of a city.
//!
//! Off, or for another victory target, nothing changes. Grep the journal for
//! `score-seat-holds-its-capital:`.

use super::*;

/// Standard turns from which the original capital is held whatever the
/// threat: turn 30 at Online (Online durations are two thirds of Standard).
pub(crate) const SCORE_CAPITAL_WALLS_STANDARD: u32 = 45;

/// Tiles from a living rival major's city within which a city of ours is
/// held.
pub(crate) const SCORE_FRONTIER_TILES: i32 = 8;

/// Tiles from a held centre within which a ranged soldier of ours counts as
/// its shooter (it can step in).
pub(crate) const SCORE_SHOOTER_REACH: i32 = 1;

/// Founded cities plus Settlers walking below which a Settler keeps its
/// queue: the capital and the first two Settlers.
pub(crate) const SCORE_HOLD_SETTLER_CITIES: usize = 3;

impl AdvancedAi {
    /// The gene is on and the seat's active target is Score.
    fn score_holds_on(&self, g: &Game) -> bool {
        self.score_seat_holds_its_capital
            && self.active_victory_target(g) == Some(VictoryTarget::Score)
    }

    /// Whether `cid` is a held city of ours: the original capital from
    /// [`SCORE_CAPITAL_WALLS_STANDARD`], or a city within
    /// [`SCORE_FRONTIER_TILES`] of a living rival major's city. False with
    /// the gene off.
    pub(super) fn score_held_city(&self, g: &Game, pid: usize, cid: u32) -> bool {
        if !self.score_holds_on(g) {
            return false;
        }
        let Some(city) = g.cities.get(&cid) else {
            return false;
        };
        if city.owner != pid {
            return false;
        }
        if city.is_capital
            && city.original_owner == pid
            && g.turn >= g.standard_duration(SCORE_CAPITAL_WALLS_STANDARD)
        {
            return true;
        }
        g.cities.values().any(|other| {
            other.owner != pid
                && g.players.get(other.owner).is_some_and(|owner| {
                    owner.alive && !owner.is_minor && !owner.is_barbarian && !owner.is_free_city
                })
                && !g.same_team(pid, other.owner)
                && g.wdist(city.pos, other.pos) <= SCORE_FRONTIER_TILES
        })
    }

    /// The next Walls tier `cid` can build now: the cheapest legal building
    /// with outer defence in its City Center that it does not hold.
    pub(super) fn score_next_walls(g: &Game, pid: usize, cid: u32) -> Option<Item> {
        let city = g.cities.get(&cid)?;
        g.rules
            .buildings
            .iter()
            .filter(|(name, spec)| {
                spec.outer_defense > 0 && !spec.wonder && !city.buildings.contains(name)
            })
            .map(|(name, spec)| (spec.cost, Item::Building { building: *name }))
            .filter(|(_, item)| g.can_produce(pid, cid, item))
            .min_by(|(left, _), (right, _)| left.total_cmp(right))
            .map(|(_, item)| item)
    }

    /// A ranged land soldier: military, on land, of the ranged promotion
    /// class.
    fn score_shooter_spec(spec: &crate::rules::UnitSpec) -> bool {
        spec.class == "military"
            && spec.domain.is_none()
            && spec.promotion_class == "ranged"
            && spec.has_ranged_attack()
    }

    /// The best ranged land soldier `cid` can train, when no ranged soldier
    /// of ours stands within [`SCORE_SHOOTER_REACH`] of its centre.
    fn score_centre_shooter_item(g: &Game, pid: usize, cid: u32) -> Option<Item> {
        let at = g.cities.get(&cid)?.pos;
        let posted = g.units.values().any(|unit| {
            unit.owner == pid
                && unit.hp > 0
                && g.wdist(unit.pos, at) <= SCORE_SHOOTER_REACH
                && g.rules
                    .units
                    .get(&unit.kind)
                    .is_some_and(Self::score_shooter_spec)
        });
        if posted {
            return None;
        }
        g.rules
            .units
            .iter()
            .filter(|(_, spec)| Self::score_shooter_spec(spec))
            .map(|(name, spec)| (spec.ranged_attack_strength(), Item::Unit { unit: *name }))
            .filter(|(_, item)| g.can_produce(pid, cid, item))
            .max_by(|(left, _), (right, _)| left.total_cmp(right))
            .map(|(_, item)| item)
    }

    /// What the gene claims in `cid` and why: its next Walls tier, else the
    /// shooter its centre lacks. `None` with the gene off, for a city not
    /// held, and once both stand.
    pub(super) fn score_capital_item(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
    ) -> Option<(Item, &'static str)> {
        if !self.score_held_city(g, pid, cid) {
            return None;
        }
        Self::score_next_walls(g, pid, cid)
            .map(|walls| (walls, "its next Walls"))
            .or_else(|| {
                Self::score_centre_shooter_item(g, pid, cid).map(|unit| (unit, "a shooter"))
            })
    }

    /// Whether `item` at the head of `cid` is what this gene claims there:
    /// the claims and the governor's rescoring that would take the head back
    /// leave it alone. Always false with the gene off.
    pub(super) fn score_capital_holds(&self, g: &Game, pid: usize, cid: u32, item: &Item) -> bool {
        self.score_holds_on(g)
            && matches!(item, Item::Building { .. } | Item::Unit { .. })
            && self
                .score_capital_item(g, pid, cid)
                .is_some_and(|(claimed, _)| claimed == *item)
    }

    /// The head `item` of `cid` the claim goes ahead of, as the journal names
    /// it; `None` when the head keeps its queue.
    fn score_capital_passes(&self, g: &Game, pid: usize, cid: u32, item: &Item) -> Option<String> {
        let invested = g.item_invested_production(cid, item);
        let cost = g.item_cost_for_city(pid, cid, item);
        if cost - invested <= g.city_yields(cid).production {
            return None;
        }
        if invested > 0.0
            && self.production_build_turns(g, pid, cid, item)
                <= super::near_done_work_finishes::NEAR_DONE_MAX_TURNS
        {
            return None;
        }
        let name = Self::plain_item(item);
        match item {
            Item::Unit { unit } => {
                let cities = g.player_city_ids(pid).len() + Self::early_settler_census(g, pid).0;
                (*unit == "settler"
                    && cities >= SCORE_HOLD_SETTLER_CITIES
                    && invested * 2.0 <= cost)
                    .then(|| format!("the Settler ({invested:.0} of {cost:.0} Production paid)"))
            }
            Item::Building { building } => {
                let spec = g.rules.buildings.get(building)?;
                (spec.outer_defense <= 0).then(|| format!("the {name}"))
            }
            Item::District { .. } | Item::Wonder { .. } => Some(format!("the {name}")),
            Item::Project { project } => g
                .rules
                .projects
                .get(project)
                .is_some_and(|spec| spec.repeatable)
                .then(|| format!("the {name}")),
            _ => None,
        }
    }

    /// See the module: every held city puts its next Walls, then its shooter,
    /// at the head of an idle or routine queue. Exact no-op with the gene off
    /// or for another target.
    pub(super) fn claim_score_capital_defences(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) {
        if !self.score_holds_on(g) {
            return;
        }
        for cid in g.player_city_ids(pid) {
            let Some((item, what)) = self.score_capital_item(g, pid, cid) else {
                continue;
            };
            if self.early_settler_floor_holds(g, pid, cid, plan) {
                continue;
            }
            let head = g.cities[&cid].queue.first().cloned();
            if head.as_ref() == Some(&item) {
                continue;
            }
            let passed = match &head {
                Some(head) => match self.score_capital_passes(g, pid, cid, head) {
                    Some(passed) => format!("{passed} (it keeps its progress)"),
                    None => continue,
                },
                None => "an idle queue".to_string(),
            };
            let name = g.cities[&cid].name.clone();
            let built = Self::plain_item(&item);
            if g.apply(pid, &Action::Produce { city: cid, item }).is_ok() {
                think!(self.journal(), Military, Decision,
                       "{name} raises {what} ({built})";
                       "score-seat-holds-its-capital: {name} is held (the capital, or within \
                        {SCORE_FRONTIER_TILES} tiles of a rival city); the {built} goes ahead of \
                        {passed}");
            }
        }
    }

    /// Whether `uid` is the ranged land soldier of ours that keeps a held
    /// City Center (see the module, point 3). Always false with the gene off.
    pub(super) fn score_holds_the_centre(&self, g: &Game, pid: usize, uid: u32) -> bool {
        if !self.score_holds_on(g) {
            return false;
        }
        let Some(unit) = g.units.get(&uid) else {
            return false;
        };
        unit.owner == pid
            && unit.hp > 0
            && unit.linked_to.is_none()
            && !g.is_embarked(unit)
            && g.rules
                .units
                .get(&unit.kind)
                .is_some_and(Self::score_shooter_spec)
            && g.city_at(unit.pos)
                .is_some_and(|cid| self.score_held_city(g, pid, cid))
    }
}

#[cfg(test)]
mod tests;
