//! `threatened-capital-trains-its-shooters`: while a power at war with us
//! stands a land soldier within [`THREAT_SOLDIER_TILES`] of our original
//! capital (or a siege gun, Battering Ram or Siege Tower within
//! [`THREAT_ENGINE_TILES`]), the capital's queue head is its best land
//! shooter until [`SHOOTERS_WANTED`] of ours stand within
//! [`SHOOTER_TILES`], then its next wall tier.
//!
//! ## Why (live Deity, 6 players, Small Continents, Online; 11-18/19)
//!
//! Over 129 live games, 103 wars declared on us brought an enemy soldier
//! within two tiles of the original capital; 34 of those capitals fell, and
//! a fallen capital retires the game. Read from `events.jsonl` and
//! `decisions.jsonl`:
//!
//! - **The capital's first order after the declaration was rarely a
//!   shooter.** It was a building, district or project in 14 of the 34
//!   falls (`civvis-20261119T021027Z`, England: Send Aid on the turn Russia
//!   declared, the capital lost six turns later) and a ranged unit in 6.
//!   Over the whole war the capital ordered any ranged unit in 11 of the 34,
//!   cavalry in 19 and Battering Rams, Siege Towers or siege guns in 10.
//! - **No purchase answered it.** The capital bought nothing in 34 of 34
//!   falls: at the first threat turn its Gold paid for a ranged unit in 3,
//!   and Walls have no Gold purchase in Civilization VI.
//! - **The existing writers do not reach it.** `surprise-war-mobilization`
//!   reads `Game::wars`, which the live mirror never fills, and
//!   `redirect_unsafe_city_queue_for_defense` claims one queue a turn behind
//!   its own siege proof; neither chooses a shooter for the capital.
//!
//! ## What it does
//!
//! After every other production writer of the turn, at the original capital
//! while we hold it:
//!
//! - **The watch.** A land soldier (no recon, not embarked) of a power at war
//!   with us, a major or a city-state but never the barbarians, in sight
//!   within [`THREAT_SOLDIER_TILES`]; or its siege gun, Ram or Tower within
//!   [`THREAT_ENGINE_TILES`]. The turn of the last reading is remembered and
//!   holds the claim [`THREAT_HOLD_TURNS`] turns after it, while a war with
//!   the same kind of power lasts.
//! - **The head.** Below [`SHOOTERS_WANTED`] land shooters of ours (a ranged
//!   attack, no siege gun, no recon) within [`SHOOTER_TILES`], the shooter
//!   the capital can train with the highest ranged strength, the quicker on a
//!   tie. With enough shooters, or none trainable, the next wall tier it can
//!   build (Ancient, Medieval, Renaissance).
//! - **What it leaves alone.** A head that finishes this turn; a repair; the
//!   Repair Outer Defenses project; a wall tier; a land shooter while a
//!   shooter is wanted. Switched production is banked by the game, and the
//!   ordinary writers take the queue back once the claim lapses.
//!
//! It feeds `shooters-man-the-capital`, which moves the shooters we own into
//! the capital's ring; this gene only trains them there. Off, nothing here
//! runs and the controller is byte-identical. Grep the journal for
//! `threatened-capital-trains-its-shooters:`.

use super::*;

/// Tiles from the original capital within which an enemy land soldier at
/// war with us arms the claim. The fallen capitals' first threat turn had a
/// median two enemy soldiers within two tiles and three within three.
pub(crate) const THREAT_SOLDIER_TILES: i32 = 3;
/// Tiles within which an enemy siege gun, Battering Ram or Siege Tower arms
/// the claim: a Trebuchet or Bombard strikes the walls from two or three.
pub(crate) const THREAT_ENGINE_TILES: i32 = 4;
/// Live turns the claim holds after the last reading: a party that steps
/// back out of reach for a turn to heal is still at the door.
pub(crate) const THREAT_HOLD_TURNS: u32 = 3;
/// Our land shooters within [`SHOOTER_TILES`] of the capital at which the
/// head turns to walls. The fallen capitals' war turns averaged 1.6 of ours
/// within two tiles against a peak of 4.6 enemy soldiers.
pub(crate) const SHOOTERS_WANTED: usize = 3;
/// Tiles from the capital a shooter counts within: an Archer or Crossbowman
/// two tiles out still reaches the City Center's neighbours.
pub(crate) const SHOOTER_TILES: i32 = 2;

/// The wall tiers in the order they are built.
const WALL_TIERS: [&str; 3] = ["walls", "medieval_walls", "renaissance_walls"];

/// What the watch read at the capital this turn.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct CapitalThreat {
    pub(crate) soldiers: usize,
    pub(crate) engines: usize,
}

impl AdvancedAi {
    /// A power whose units at the capital arm the claim: alive, at war with
    /// us, and neither the barbarians nor a free city.
    fn capital_threat_enemy(g: &Game, pid: usize, other: usize) -> bool {
        other != pid
            && g.players
                .get(other)
                .is_some_and(|player| player.alive && !player.is_barbarian && !player.is_free_city)
            && g.is_at_war(pid, other)
    }

    /// The watch: enemy soldiers in sight within [`THREAT_SOLDIER_TILES`] and
    /// engines within [`THREAT_ENGINE_TILES`] of `capital`. `None` when
    /// neither stands there.
    pub(crate) fn capital_threat_reading(
        g: &Game,
        pid: usize,
        capital: u32,
    ) -> Option<CapitalThreat> {
        let home = g.cities[&capital].pos;
        let visible = g.player_vision_frame(pid);
        let mut reading = CapitalThreat::default();
        for unit in g.units.values() {
            if unit.owner == pid || unit.hp <= 0 {
                continue;
            }
            let distance = g.wdist(unit.pos, home);
            if distance > THREAT_ENGINE_TILES.max(THREAT_SOLDIER_TILES) {
                continue;
            }
            let spec = &g.rules.units[unit.kind];
            let land = !matches!(spec.domain.as_deref(), Some("sea" | "air"));
            let engine = land
                && ((spec.class == "military" && spec.siege)
                    || matches!(unit.kind.as_str(), "battering_ram" | "siege_tower"));
            let soldier = !engine
                && land
                && spec.class == "military"
                && spec.promotion_class != "recon"
                && (spec.is_melee_capable() || spec.has_ranged_attack())
                && !g.is_embarked(unit);
            let counted = (engine && distance <= THREAT_ENGINE_TILES)
                || (soldier && distance <= THREAT_SOLDIER_TILES);
            if !counted
                || !Self::capital_threat_enemy(g, pid, unit.owner)
                || !g.sees(&visible, unit.pos)
                || !g.unit_visible_to(unit.id, pid)
            {
                continue;
            }
            if engine {
                reading.engines += 1;
            } else {
                reading.soldiers += 1;
            }
        }
        (reading.soldiers + reading.engines > 0).then_some(reading)
    }

    /// Remember the turn the watch last read, from the turn-start frame.
    /// No-op with the gene off.
    pub(super) fn remember_threatened_capital(&mut self, g: &Game, pid: usize) {
        if !self.threatened_capital_trains_its_shooters || self.base.minor || self.base.barb {
            return;
        }
        let Some(capital) = Self::capital_watch_city(g, pid) else {
            return;
        };
        if Self::capital_threat_reading(g, pid, capital).is_some() {
            self.threatened_capital_seen = Some(g.turn);
        }
    }

    /// Whether the claim holds at `capital` this turn: the watch reads now,
    /// or read within [`THREAT_HOLD_TURNS`] while a war with a power the
    /// watch counts lasts.
    fn threatened_capital_armed(&self, g: &Game, pid: usize, capital: u32) -> bool {
        if Self::capital_threat_reading(g, pid, capital).is_some() {
            return true;
        }
        self.threatened_capital_seen
            .is_some_and(|seen| seen <= g.turn && g.turn - seen <= THREAT_HOLD_TURNS)
            && (0..g.players.len()).any(|other| Self::capital_threat_enemy(g, pid, other))
    }

    /// A land shooter for the claim: a military unit with a ranged attack
    /// that is neither a siege gun nor recon.
    fn capital_shooter_spec(spec: &crate::rules::UnitSpec) -> bool {
        spec.class == "military"
            && spec.has_ranged_attack()
            && !spec.siege
            && spec.promotion_class != "recon"
            && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
    }

    /// Our land shooters within [`SHOOTER_TILES`] of `capital`.
    pub(crate) fn capital_shooters_near(g: &Game, pid: usize, capital: u32) -> usize {
        let home = g.cities[&capital].pos;
        g.units
            .values()
            .filter(|unit| {
                unit.owner == pid
                    && unit.hp > 0
                    && g.wdist(unit.pos, home) <= SHOOTER_TILES
                    && Self::capital_shooter_spec(&g.rules.units[unit.kind])
                    && !g.is_embarked(unit)
            })
            .count()
    }

    /// The shooter `capital` can train with the highest ranged strength, the
    /// quicker on a tie, then by name.
    fn capital_best_shooter(g: &Game, pid: usize, capital: u32) -> Option<Item> {
        let production = g.city_yields(capital).production.max(0.1);
        g.producible_items(pid, capital)
            .into_iter()
            .filter_map(|item| {
                let Item::Unit { unit } = &item else {
                    return None;
                };
                let spec = g.rules.units.get(unit)?;
                if !Self::capital_shooter_spec(spec) {
                    return None;
                }
                let turns = g.item_remaining_cost_for_city(pid, capital, &item) / production;
                Some((spec.ranged_attack_strength(), turns, unit.to_string(), item))
            })
            .max_by(|a, b| {
                a.0.total_cmp(&b.0)
                    .then_with(|| b.1.total_cmp(&a.1))
                    .then_with(|| b.2.cmp(&a.2))
            })
            .map(|(_, _, _, item)| item)
    }

    /// The next wall tier `capital` can build.
    fn capital_next_walls(g: &Game, pid: usize, capital: u32) -> Option<Item> {
        WALL_TIERS
            .iter()
            .map(|building| Item::Building {
                building: Name::new(building),
            })
            .find(|wall| g.can_produce(pid, capital, wall))
    }

    /// Whether `head` already answers the claim's `want`: a repair, the
    /// Repair Outer Defenses project, a wall tier, or a land shooter while a
    /// shooter is wanted.
    fn capital_head_answers(g: &Game, head: &Item, shooter_wanted: bool) -> bool {
        match head {
            Item::Repair { .. } => true,
            Item::Project { project } => project.as_str() == "repair_outer_defenses",
            Item::Building { building } => WALL_TIERS.contains(&building.as_str()),
            Item::Unit { unit } | Item::Formation { unit, .. } => {
                shooter_wanted
                    && g.rules
                        .units
                        .get(unit)
                        .is_some_and(Self::capital_shooter_spec)
            }
            _ => false,
        }
    }

    /// The claim, after every other production writer of the turn: see the
    /// module. Returns the item put at the head. Exact no-op with the gene
    /// off.
    pub(super) fn claim_threatened_capital_shooters(
        &self,
        g: &mut Game,
        pid: usize,
    ) -> Option<Item> {
        if !self.threatened_capital_trains_its_shooters || self.base.minor || self.base.barb {
            return None;
        }
        let capital = Self::capital_watch_city(g, pid)?;
        if !self.threatened_capital_armed(g, pid, capital) {
            return None;
        }
        let head = g.cities[&capital].queue.first().cloned();
        if let Some(head) = &head {
            let production = g.city_yields(capital).production.max(0.1);
            if g.item_remaining_cost_for_city(pid, capital, head) <= production + f64::EPSILON {
                return None;
            }
        }
        let shooters = Self::capital_shooters_near(g, pid, capital);
        let shooter_wanted = shooters < SHOOTERS_WANTED;
        if head
            .as_ref()
            .is_some_and(|head| Self::capital_head_answers(g, head, shooter_wanted))
        {
            return None;
        }
        let shooter = shooter_wanted
            .then(|| Self::capital_best_shooter(g, pid, capital))
            .flatten();
        let item = shooter
            .clone()
            .or_else(|| Self::capital_next_walls(g, pid, capital))?;
        g.apply(
            pid,
            &Action::Produce {
                city: capital,
                item: item.clone(),
            },
        )
        .ok()?;
        let reading = Self::capital_threat_reading(g, pid, capital).unwrap_or_default();
        think!(self.journal(), Military, Decision,
            "{} trains {} while the enemy is at its walls",
            g.cities[&capital].name, Self::plain_item(&item);
            "threatened-capital-trains-its-shooters: {} enemy soldier(s) within \
             {THREAT_SOLDIER_TILES} tiles and {} engine(s) within {THREAT_ENGINE_TILES}, \
             {shooters} of our shooters within {SHOOTER_TILES} (wanted {SHOOTERS_WANTED}){}",
            reading.soldiers, reading.engines,
            head.map_or(String::new(), |head| format!(
                "; {} keeps its progress", Self::plain_item(&head))));
        Some(item)
    }
}

#[cfg(test)]
mod tests;
