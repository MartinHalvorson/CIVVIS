//! `capital-walls-before-the-surprise`: while a met major at peace fields
//! [`CAPITAL_WALLS_RATIO`] times our military power and one of its land
//! soldiers has stood within [`CAPITAL_WATCH_TILES`] of our original capital
//! in the last ten live turns, Masonry is the next research and the
//! capital's queue goes to its Ancient Walls (a Settler within
//! [`CAPITAL_WALLS_SETTLER_TURNS`] of done finishes first; walls that have
//! started finish). While such a major stands [`CAPITAL_DOOR_UNITS`]
//! soldiers within [`CAPITAL_DOOR_TILES`] of the capital and fewer than
//! [`CAPITAL_GARRISON`] of ours stand within [`CAPITAL_GARRISON_TILES`], one
//! land defender is bought into it above the upkeep reserve, or queued there
//! when the walls cannot be.
//!
//! **Why (10-13/14 census, 183 finished Immortal runs).** 28 lost the
//! original capital before turn 100, 16 to a war opened before turn 50; the
//! city fell 2-4 turns after the declaration in 15. Walls stood on the last
//! turn of peace in 4 of the 28, and 13 of the 16 early wars opened before we
//! had Masonry. Live civvis-20261014T040735Z: Greece denounced at turn 12,
//! fielded 151 to our 47 at 20 and 322 to 71 at 26, declared at 27 and took
//! Bogotá at 29; Masonry was researched at 28, "for wartime recovery".
//!
//! The visible army is too late a signal on its own: two Greek-style
//! soldiers within six tiles of the capital at twice our power fired before
//! 13 of the 28 losses, a median 4 turns ahead, while Masonry and 40
//! Production of walls took a median 14 turns from that turn. A single
//! soldier within eight tiles in the last ten turns at twice our power fired
//! before 27 of the 28, in time for walls to stand by the capture turn in 20,
//! and in 75 of the 149 runs that kept the capital (a median turn 29), the
//! walls' 40 Production being the cost. Walls cannot be bought: Civilization
//! VI declares no purchase yield for them. A defender can, so the door
//! signal's short lead buys the body.

use super::*;

/// A watched major's military power over ours that arms the walls.
pub(crate) const CAPITAL_WALLS_RATIO: f64 = 2.0;

/// Tiles from the original capital within which a watched major's land
/// soldier counts as seen at our door.
pub(crate) const CAPITAL_WATCH_TILES: i32 = 8;

/// Standard turns a sighting keeps the walls armed: ten at Online speed.
pub(crate) const CAPITAL_WATCH_STANDARD: u32 = 15;

/// A Settler this many turns or fewer from done finishes before the walls.
pub(crate) const CAPITAL_WALLS_SETTLER_TURNS: f64 = 2.0;

/// The defender leg: a major with this power ratio and this many soldiers
/// within this many tiles of the capital.
pub(crate) const CAPITAL_DOOR_RATIO: f64 = 2.0;
pub(crate) const CAPITAL_DOOR_TILES: i32 = 6;
pub(crate) const CAPITAL_DOOR_UNITS: usize = 2;

/// Our land soldiers within [`CAPITAL_GARRISON_TILES`] the defender leg wants.
pub(crate) const CAPITAL_GARRISON: usize = 2;
pub(crate) const CAPITAL_GARRISON_TILES: i32 = 2;

impl AdvancedAi {
    /// The original capital we still hold.
    fn capital_watch_city(g: &Game, pid: usize) -> Option<u32> {
        g.player_city_ids(pid).into_iter().find(|cid| {
            let city = &g.cities[cid];
            city.is_capital && city.original_owner == pid
        })
    }

    /// A met major we are at peace with: the only owner the watch reads.
    fn capital_watch_suspect(g: &Game, pid: usize, other: usize) -> bool {
        g.players.get(other).is_some_and(|player| {
            other != pid
                && player.alive
                && !player.is_minor
                && !player.is_barbarian
                && !player.is_free_city
                && g.has_met(pid, other)
                && !g.is_at_war(pid, other)
        })
    }

    /// A land combat unit that is not a scout.
    fn capital_watch_soldier(g: &Game, unit: &crate::game::Unit) -> bool {
        let spec = &g.rules.units[unit.kind];
        spec.class == "military"
            && spec.domain.as_deref().is_none_or(|domain| domain == "land")
            && spec.promotion_class != "recon"
            && (spec.is_melee_capable() || spec.has_ranged_attack())
    }

    /// The watched majors' soldiers the seat sees within `radius` of `at`.
    fn capital_watch_seen(
        g: &Game,
        pid: usize,
        at: Pos,
        radius: i32,
    ) -> impl Iterator<Item = &crate::game::Unit> {
        let visible = g.player_vision_frame(pid);
        g.units.values().filter(move |unit| {
            Self::capital_watch_suspect(g, pid, unit.owner)
                && Self::capital_watch_soldier(g, unit)
                && g.wdist(unit.pos, at) <= radius
                && g.sees(&visible, unit.pos)
                && g.unit_visible_to(unit.id, pid)
        })
    }

    /// Record the turn each watched major's soldier was last seen within
    /// [`CAPITAL_WATCH_TILES`] of the original capital. Runs at turn start
    /// with the hostile memory; a no-op with the gene off.
    pub(super) fn remember_capital_watch_sightings(&mut self, g: &Game, pid: usize) {
        if !self.capital_walls_before_the_surprise {
            return;
        }
        let Some(capital) = Self::capital_watch_city(g, pid) else {
            return;
        };
        let at = g.cities[&capital].pos;
        let owners: Vec<usize> = Self::capital_watch_seen(g, pid, at, CAPITAL_WATCH_TILES)
            .map(|unit| unit.owner)
            .collect();
        for owner in owners {
            self.capital_watch_sightings.insert(owner, g.turn);
        }
    }

    /// The strongest watched major seen at our door within the window that
    /// fields [`CAPITAL_WALLS_RATIO`] times our power, and that ratio.
    /// `None` with the gene off.
    pub(super) fn capital_walls_threat(&self, g: &Game, pid: usize) -> Option<(usize, f64)> {
        if !self.capital_walls_before_the_surprise {
            return None;
        }
        let window = g.standard_duration(CAPITAL_WATCH_STANDARD);
        let ours = g.military_power(pid).max(1.0);
        self.capital_watch_sightings
            .iter()
            .filter(|(other, seen)| {
                Self::capital_watch_suspect(g, pid, **other)
                    && **seen <= g.turn
                    && g.turn - **seen <= window
            })
            .map(|(other, _)| (*other, g.military_power(*other) / ours))
            .filter(|(_, ratio)| *ratio >= CAPITAL_WALLS_RATIO)
            .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)))
    }

    /// The research leg: the watched threat reads and the capital stands
    /// unwalled. Read by `defensive_walls_research_goal`.
    pub(super) fn capital_walls_research_threat(
        &self,
        g: &Game,
        pid: usize,
    ) -> Option<(usize, f64)> {
        let capital = Self::capital_watch_city(g, pid)?;
        if g.city_max_wall_hp(&g.cities[&capital]) > 0 {
            return None;
        }
        self.capital_walls_threat(g, pid)
    }

    fn capital_walls_item() -> Item {
        Item::Building {
            building: crate::name!("walls"),
        }
    }

    /// Whether the capital's walls are wanted: the gene on, no walls yet, the
    /// capital able to build them, and the threat reading or the walls
    /// already started (a started wall is finished).
    fn capital_walls_armed(&self, g: &Game, pid: usize, capital: u32) -> bool {
        let walls = Self::capital_walls_item();
        self.capital_walls_before_the_surprise
            && g.city_max_wall_hp(&g.cities[&capital]) <= 0
            && g.can_produce(pid, capital, &walls)
            && (self.capital_walls_threat(g, pid).is_some()
                || g.item_invested_production(capital, &walls) > 0.0)
    }

    /// Whether the claim still wants `item` at the head of `cid`'s queue.
    pub(super) fn capital_walls_hold(&self, g: &Game, pid: usize, cid: u32, item: &Item) -> bool {
        *item == Self::capital_walls_item()
            && Self::capital_watch_city(g, pid) == Some(cid)
            && self.capital_walls_armed(g, pid, cid)
    }

    /// A watched major at [`CAPITAL_DOOR_RATIO`] our power with
    /// [`CAPITAL_DOOR_UNITS`] soldiers in sight within [`CAPITAL_DOOR_TILES`]
    /// of the capital: (major, ratio, soldiers).
    fn capital_door_threat(
        &self,
        g: &Game,
        pid: usize,
        capital: u32,
    ) -> Option<(usize, f64, usize)> {
        if !self.capital_walls_before_the_surprise {
            return None;
        }
        let at = g.cities[&capital].pos;
        let mut near: BTreeMap<usize, usize> = BTreeMap::new();
        for unit in Self::capital_watch_seen(g, pid, at, CAPITAL_DOOR_TILES) {
            *near.entry(unit.owner).or_default() += 1;
        }
        let ours = g.military_power(pid).max(1.0);
        near.into_iter()
            .filter(|(_, soldiers)| *soldiers >= CAPITAL_DOOR_UNITS)
            .map(|(other, soldiers)| (other, g.military_power(other) / ours, soldiers))
            .filter(|(_, ratio, _)| *ratio >= CAPITAL_DOOR_RATIO)
            .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)))
    }

    /// Our land soldiers within [`CAPITAL_GARRISON_TILES`] of the capital.
    fn capital_garrison(g: &Game, pid: usize, capital: u32) -> usize {
        let at = g.cities[&capital].pos;
        g.units
            .values()
            .filter(|unit| {
                unit.owner == pid
                    && Self::capital_watch_soldier(g, unit)
                    && g.wdist(unit.pos, at) <= CAPITAL_GARRISON_TILES
            })
            .count()
    }

    /// The defender leg's purchase: one land defender into the capital above
    /// the upkeep reserve while the door signal reads and the garrison is
    /// short. True when one was bought. Exact no-op with the gene off.
    pub(super) fn capital_garrison_purchase(&self, g: &mut Game, pid: usize) -> bool {
        if !self.capital_walls_before_the_surprise {
            return false;
        }
        let Some(capital) = Self::capital_watch_city(g, pid) else {
            return false;
        };
        let Some((rival, ratio, soldiers)) = self.capital_door_threat(g, pid, capital) else {
            return false;
        };
        let garrison = Self::capital_garrison(g, pid, capital);
        if garrison >= CAPITAL_GARRISON {
            return false;
        }
        let floor = self.purchase_upkeep_floor(g, pid, 0.0);
        let Some((price, unit)) = Self::capital_defender_for_sale(g, pid, capital, floor) else {
            return false;
        };
        if g.apply(
            pid,
            &Action::Buy {
                city: capital,
                unit,
                formation: 0,
                currency: "gold".into(),
            },
        )
        .is_err()
        {
            return false;
        }
        think!(self.journal(), Military, Decision,
            "{} buys a defender ({}) before {} can surprise it",
            g.cities[&capital].name, crate::reasoning::plain(unit.as_str()), g.players[rival].civ;
            "capital-walls-before-the-surprise: {soldiers} of their soldiers within \
             {CAPITAL_DOOR_TILES} tiles at {ratio:.1}x our power, {garrison} of ours within \
             {CAPITAL_GARRISON_TILES}; {price:.0} Gold, {floor:.0} kept");
        true
    }

    /// The strongest land soldier (no scout: `buy_gold_military` would take
    /// one, and 040735Z's replay spent 60 Gold on a Scout at turn 23 when the
    /// host's menu held no Warrior) `capital` sells for Gold leaving `floor`
    /// in the bank, cheaper first on a tie, and its price.
    fn capital_defender_for_sale(
        g: &Game,
        pid: usize,
        capital: u32,
        floor: f64,
    ) -> Option<(f64, Name)> {
        let budget = g.players[pid].gold - floor;
        let mut best: Option<(f64, f64, Name)> = None;
        for (name, spec) in &g.rules.units {
            let item = Item::Unit { unit: *name };
            if spec.class != "military"
                || spec
                    .domain
                    .as_deref()
                    .is_some_and(|domain| domain != "land")
                || spec.promotion_class == "recon"
                || !(spec.is_melee_capable() || spec.has_ranged_attack())
                || !g.can_produce(pid, capital, &item)
                || g.purchase_is_blocked(capital, &item)
            {
                continue;
            }
            let Some(price) = g.unit_purchase_cost(pid, capital, name.as_str(), "gold") else {
                continue;
            };
            if price > budget + 1e-9 {
                continue;
            }
            let power = spec.strength.max(spec.ranged_attack_strength());
            if best.as_ref().is_none_or(|(old_power, old_price, old)| {
                power > *old_power + 1e-9
                    || ((power - old_power).abs() <= 1e-9
                        && (price, name.as_str()) < (*old_price, old.as_str()))
            }) {
                best = Some((power, price, *name));
            }
        }
        best.map(|(_, price, name)| (price, name))
    }

    /// The production leg, after every ordinary queue writer: the capital's
    /// walls while they are armed, else (the walls standing or not yet
    /// buildable) a land defender while the door signal reads and the
    /// garrison is short. A head that finishes this turn, or a Settler within
    /// [`CAPITAL_WALLS_SETTLER_TURNS`], finishes first. Exact no-op with the
    /// gene off.
    pub(super) fn claim_capital_walls_before_the_surprise(&self, g: &mut Game, pid: usize) {
        if !self.capital_walls_before_the_surprise {
            return;
        }
        let Some(capital) = Self::capital_watch_city(g, pid) else {
            return;
        };
        let head = g.cities[&capital].queue.first().cloned();
        if let Some(head) = &head {
            let production = g.city_yields(capital).production.max(0.1);
            let remaining = g.item_remaining_cost_for_city(pid, capital, head);
            let turns = g
                .host_production_turns(capital, head)
                .filter(|turns| turns.is_finite() && *turns > 0.0)
                .unwrap_or(remaining / production);
            if remaining <= production + f64::EPSILON
                || (matches!(head, Item::Unit { unit } if unit == "settler")
                    && turns <= CAPITAL_WALLS_SETTLER_TURNS)
            {
                return;
            }
        }
        let walls = Self::capital_walls_item();
        if self.capital_walls_armed(g, pid, capital) {
            if head.as_ref() == Some(&walls) {
                return;
            }
            let threat = self.capital_walls_threat(g, pid);
            if g.apply(
                pid,
                &Action::Produce {
                    city: capital,
                    item: walls.clone(),
                },
            )
            .is_ok()
            {
                let turns = g.host_production_turns(capital, &walls).unwrap_or_else(|| {
                    g.item_remaining_cost_for_city(pid, capital, &walls)
                        / g.city_yields(capital).production.max(0.1)
                });
                let why = threat.map_or_else(
                    || "the walls already started finish".to_string(),
                    |(rival, ratio)| {
                        format!(
                            "{} fields {ratio:.1}x our power and its soldiers stood within \
                             {CAPITAL_WATCH_TILES} tiles",
                            g.players[rival].civ
                        )
                    },
                );
                think!(self.journal(), Military, Decision,
                    "{} builds its walls before a surprise", g.cities[&capital].name;
                    "capital-walls-before-the-surprise: {why}; {turns:.1} turns{}",
                    head.map_or(String::new(), |item| format!(
                        "; {} keeps its progress", Self::plain_item(&item))));
            }
            return;
        }
        let Some((rival, ratio, soldiers)) = self.capital_door_threat(g, pid, capital) else {
            return;
        };
        let garrison = Self::capital_garrison(g, pid, capital);
        if garrison >= CAPITAL_GARRISON
            || head.as_ref().is_some_and(|item| {
                matches!(item, Item::Unit { unit } | Item::Formation { unit, .. }
                    if g.rules.units.get(unit).is_some_and(|spec| spec.class == "military"
                        && spec.domain.as_deref().is_none_or(|domain| domain == "land")))
            })
        {
            return;
        }
        let Some(unit) = self
            .base
            .best_military(g, pid, capital, Some(false))
            .or_else(|| self.base.best_military(g, pid, capital, None))
        else {
            return;
        };
        let item = Item::Unit {
            unit: Name::new(&unit),
        };
        if g.apply(
            pid,
            &Action::Produce {
                city: capital,
                item,
            },
        )
        .is_ok()
        {
            think!(self.journal(), Military, Decision,
                "{} trains a defender before {} can surprise it",
                g.cities[&capital].name, g.players[rival].civ;
                "capital-walls-before-the-surprise: {soldiers} of their soldiers within \
                 {CAPITAL_DOOR_TILES} tiles at {ratio:.1}x our power, {garrison} of ours within \
                 {CAPITAL_GARRISON_TILES}; no walls to build{}",
                head.map_or(String::new(), |item| format!(
                    "; {} keeps its progress", Self::plain_item(&item))));
        }
    }
}

#[cfg(test)]
mod tests;
