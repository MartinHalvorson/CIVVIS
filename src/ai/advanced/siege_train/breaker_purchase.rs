//! `breaker-bought-for-the-siege`: a siege held outside a walled city for
//! want of a wall-breaker (`siege-needs-a-breaker`'s `siege_breaker_waits`),
//! with no breaker on its way (`waiting_for_a_breaker`) and our land army
//! within [`MUSTER_BREACH_FAR`] covering the city's bill, buys one: a
//! Battering Ram or Siege Tower that works on these walls while melee of ours
//! stand there to swing beside it, else the strongest land siege gun on sale.
//! It is bought for Gold, above the purchase upkeep floor, in the city of ours
//! nearest the siege within [`BREAKER_PURCHASE_REACH`]. One purchase a turn,
//! and none for the same city again within [`BREAKER_PURCHASE_COOLDOWN`]
//! standard turns.
//!
//! **Why (live Emperor, 62 games on 047945067).** 1,653 siege-turns read
//! "nothing to open the walls, so the train holds outside the city's reach";
//! on 1,009 of them, in 52 games, no breaker was coming. The breaker the
//! production side asks for lands in whichever city is nearest, often one
//! that is new. Lyon (civvis-20261018T011334Z) stood behind Ancient Walls
//! (100) from turn 44 with 11 of our units staged against a bill of 66, and
//! its Rams were queued in Guayaquil (3 Production, then paused for a
//! Spearman) and Maracaibo (1 Production). The first arrived at turn 79,
//! while the capital built six Spearmen and the treasury held about 150
//! Gold. A Ram works on Ancient Walls and a Tower on Medieval. With either
//! one, the melee already standing there opens the city.
//!
//! Called in the purchase phase after the city-saving purchases. Off,
//! nothing runs.

use super::*;
use crate::game::Item;
use crate::name::Name;

/// The farthest a buying city may stand from the besieged city: a breaker
/// bought further out walks past the war it was bought for.
pub(in crate::ai::advanced) const BREAKER_PURCHASE_REACH: i32 = 12;

/// Standard turns before a second breaker is bought for the same city.
pub(in crate::ai::advanced) const BREAKER_PURCHASE_COOLDOWN: u32 = 8;

/// A breaker on sale: support (a Ram or Tower) before a gun, then the
/// nearer city, the stronger gun, the cheaper price, the lower ids.
type BreakerOffer = (bool, i32, i64, i64, u32, Name, u32);

impl AdvancedAi {
    /// See the module. True when a breaker was bought. Exact no-op with the
    /// gene off.
    pub(in crate::ai::advanced) fn siege_breaker_purchase(
        &mut self,
        g: &mut Game,
        pid: usize,
    ) -> bool {
        if !self.breaker_bought_for_the_siege || !self.siege_needs_a_breaker {
            return false;
        }
        let turn = g.turn;
        let cooldown = g.standard_duration(BREAKER_PURCHASE_COOLDOWN);
        let budget = g.players[pid].gold - self.purchase_upkeep_floor(g, pid, 0.0);
        if budget <= 0.0 {
            return false;
        }
        let waiting: Vec<Pos> = self
            .siege_breaker_waits
            .iter()
            .filter(|(_, wait)| turn.saturating_sub(wait.last) <= 1)
            .map(|(pos, _)| *pos)
            .collect();
        let mut best: Option<BreakerOffer> = None;
        for pos in waiting {
            let Some(cid) = g
                .cities
                .values()
                .find(|city| city.pos == pos)
                .map(|city| city.id)
            else {
                continue;
            };
            let Some(city) = CityView::of(g, cid) else {
                continue;
            };
            if city.owner == pid
                || !g.is_at_war(pid, city.owner)
                || walls_open_to_melee(&city)
                || self.waiting_for_a_breaker(g, pid, cid)
                || self
                    .breaker_bought
                    .get(&cid)
                    .is_some_and(|bought| turn.saturating_sub(*bought) < cooldown)
            {
                continue;
            }
            let land_soldier = |unit: &crate::game::Unit| {
                let spec = &g.rules.units[unit.kind];
                unit.owner == pid
                    && spec.class == "military"
                    && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                    && g.wdist(unit.pos, city.pos) <= MUSTER_BREACH_FAR
            };
            let army: f64 = g
                .units
                .values()
                .filter(|unit| land_soldier(unit))
                .map(|unit| unit_power(g, unit.id))
                .sum();
            if army < siege_bill(g, pid, &city) {
                continue;
            }
            let swingers = g.units.values().any(|unit| {
                land_soldier(unit)
                    && crate::ai::siege_support::eligible_attacker(&g.rules.units[unit.kind])
            });
            let support: Vec<Name> = if swingers {
                ["battering_ram", "siege_tower"]
                    .into_iter()
                    .filter(|kind| g.city_allows_siege_support(cid, kind))
                    .map(Name::new)
                    .collect()
            } else {
                Vec::new()
            };
            let guns: Vec<Name> = g
                .rules
                .units
                .keys()
                .copied()
                .filter(|kind| land_gun(g, *kind))
                .collect();
            for own in g.player_city_ids(pid) {
                let distance = g.wdist(g.cities[&own].pos, city.pos);
                if distance > BREAKER_PURCHASE_REACH {
                    continue;
                }
                for (is_support, kind) in support
                    .iter()
                    .map(|kind| (true, *kind))
                    .chain(guns.iter().map(|kind| (false, *kind)))
                {
                    let item = Item::Unit { unit: kind };
                    if !g.can_produce(pid, own, &item) || g.purchase_is_blocked(own, &item) {
                        continue;
                    }
                    let Some(price) = g.unit_purchase_cost(pid, own, kind.as_str(), "gold") else {
                        continue;
                    };
                    if price > budget + 1e-9 {
                        continue;
                    }
                    let strength = g.rules.units[kind].ranged_attack_strength();
                    let offer: BreakerOffer = (
                        is_support,
                        distance,
                        (strength * 100.0).round() as i64,
                        (price * 100.0).round() as i64,
                        own,
                        kind,
                        cid,
                    );
                    let better = best.as_ref().is_none_or(|old| {
                        (
                            !offer.0,
                            offer.1,
                            -offer.2,
                            offer.3,
                            offer.4,
                            offer.5.as_str(),
                        ) < (!old.0, old.1, -old.2, old.3, old.4, old.5.as_str())
                    });
                    if better {
                        best = Some(offer);
                    }
                }
            }
        }
        let Some((_, distance, _, price, own, kind, cid)) = best else {
            return false;
        };
        if g.apply(
            pid,
            &Action::Buy {
                city: own,
                unit: kind,
                formation: 0,
                currency: "gold".into(),
            },
        )
        .is_err()
        {
            return false;
        }
        self.breaker_bought.insert(cid, turn);
        let target = g.cities[&cid].name.clone();
        let walls = (g.cities[&cid].wall_hp, g.city_max_wall_hp(&g.cities[&cid]));
        think!(self.journal(), Military, Decision,
            "{} buys a {} for the siege of {target}", g.cities[&own].name,
            crate::reasoning::plain(kind.as_str());
            "breaker-bought-for-the-siege: walls {}/{} and no breaker coming, {distance} tiles out, {:.0} Gold",
            walls.0, walls.1, price as f64 / 100.0);
        true
    }
}
