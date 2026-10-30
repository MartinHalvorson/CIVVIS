//! `counterweight-defends-the-diplomat`: a Diplomatic seat with no faith of
//! its own in its cities buys the counterweight the Domination seat buys.
//!
//! A Religious Victory needs its faith to hold a strict majority in every
//! living major, so a seat whose own faith holds none of its cities vetoes it
//! only by keeping half of them on another faith. `religious_defense` buys
//! those counterweight Missionaries, and the counterweight family
//! (`counterweight-spends-the-bank`, `counterweight-from-the-first-convert`,
//! `counterweight-flips-the-small-towns`) sizes and funds them, but the whole
//! family answers only the Domination lane, and only a seat that never
//! founded: a founder goes to `religious_spending`, which buys units of its
//! own faith and so needs a city that still follows it.
//!
//! Live Immortal civvis-20261030T171157Z, the first Lv 7 diplomatic seat: we
//! founded Catholicism at turn 58; it held 3 of our 6 cities at 80 and none
//! from 100. Islam (the Aztecs) held 8 of our 10 cities from 145, Buddhism
//! (the Inca) the other 2. We stood at 16 Diplomatic Victory Points at turn
//! 160 against the rivals' 3-6, banked 300-780 Faith from turn 130 with no
//! religious unit in the field, and lost to Islam's Religious Victory at 168.
//!
//! With the gene on, on the Diplomacy lane:
//! - the counterweight family's Domination gate opens
//!   ([`AdvancedAi::counterweight_lane`]);
//! - a founder whose faith holds none of our cities defends as a faithless
//!   seat ([`AdvancedAi::counterweight_faithless`]), so `religious_defense`
//!   buys Missionaries of the faith a city of ours does follow.
//!
//! Off, and on every other lane: unchanged.
use super::{AdvancedAi, VictoryTarget};
use crate::game::{Action, Game, Item};
use crate::think;

impl AdvancedAi {
    /// Whether the counterweight family answers this seat's lane: the
    /// Domination lane, and the Diplomacy lane under the gene.
    pub(super) fn counterweight_lane(&self, g: &Game) -> bool {
        match self.active_victory_target(g) {
            Some(VictoryTarget::Domination) => true,
            Some(VictoryTarget::Diplomacy) => self.counterweight_defends_the_diplomat,
            _ => false,
        }
    }

    /// Whether `faith-war-before-the-flip` answers this seat's lane: the
    /// Domination lane, and the Diplomacy lane under
    /// `faith-war-defends-the-diplomat`. A faithless seat with no
    /// counterweight source (live 171157Z: the one Holy Site and Shrine
    /// followed the threat; native 52011/17/18: every city on the threat by
    /// turns 86-95) has no other defence; at war the founder's spreaders
    /// reached our cities at 0.26 a turn against 1.47 at peace.
    pub(super) fn faith_war_lane(&self, g: &Game) -> bool {
        match self.active_victory_target(g) {
            Some(VictoryTarget::Domination) => true,
            Some(VictoryTarget::Diplomacy) => self.faith_war_defends_the_diplomat,
            _ => false,
        }
    }

    /// Whether the seat defends as one with no faith of its own: it never
    /// founded, or (under the gene, on the Diplomacy lane) its faith holds
    /// none of its cities.
    pub(super) fn counterweight_faithless(&self, g: &Game, pid: usize) -> bool {
        let Some(own) = g.players[pid].religion.as_deref() else {
            return true;
        };
        self.counterweight_defends_the_diplomat
            && self.active_victory_target(g) == Some(VictoryTarget::Diplomacy)
            && !g
                .player_city_ids(pid)
                .iter()
                .any(|cid| g.city_religion(&g.cities[cid]) == Some(own))
    }

    /// `counterweight-builds-its-source`: a faithless Domination or
    /// Diplomacy seat that a rival faith presses (`rival_faith_presses_us`:
    /// at its lane's early-warning bar or match point) and that holds a city
    /// on a safe counterfaith but no Holy Site in any such city claims one
    /// there, the biggest city first: bought with Gold when the treasury
    /// covers it, otherwise put at the front of that city's queue.
    ///
    /// A counterweight Missionary is bought only in a city that follows the
    /// counterfaith and holds a Holy Site (`unit_purchase_cost`), and
    /// `counterweight-finishes-one-shrine` only finishes a Shrine where the
    /// district stands. Across the 13 live religious losses of 2026-10-30 a
    /// counterfaith city of ours stood in 7, and one with a Holy Site in 1.
    /// Live Immortal 171157Z (diplomatic, 16 DVP at turn 160): Buddhism held
    /// Argos and Rhodes from turn 70 to the loss at 168, neither with a Holy
    /// Site, while 337-718 Faith sat in the bank and Islam took 8 of 10.
    pub(super) fn counterweight_builds_its_source(&mut self, g: &mut Game, pid: usize) {
        if !self.counterweight_builds_its_source
            || !matches!(
                self.active_victory_target(g),
                Some(VictoryTarget::Domination) | Some(VictoryTarget::Diplomacy)
            )
            || !g.victory_conditions.religious
            || !self.counterweight_faithless(g, pid)
        {
            return;
        }
        let Some(threat) = self.rival_faith_presses_us(g, pid) else {
            return;
        };
        let family = crate::name!("holy_site");
        let mut source_stands = false;
        let mut candidates: Vec<(i32, u32, String)> = Vec::new();
        for cid in g.player_city_ids(pid) {
            let city = &g.cities[&cid];
            let Some(faith) = g.city_religion(city) else {
                continue;
            };
            if faith == threat || !self.counterfaith_is_safe(g, pid, faith) {
                continue;
            }
            if g.city_has_district_family(city, family) {
                source_stands = true;
            } else {
                candidates.push((city.pop, cid, faith.to_string()));
            }
        }
        if source_stands {
            return;
        }
        candidates.sort_by_key(|(pop, cid, _)| (std::cmp::Reverse(*pop), *cid));
        for (_, cid, faith) in candidates {
            let district = crate::ai::BasicAi::civ_district(g, pid, "holy_site");
            let Some(item) = g
                .district_sites(cid, district)
                .into_iter()
                .max_by(|left, right| {
                    g.district_yields(district, *left)
                        .total()
                        .partial_cmp(&g.district_yields(district, *right).total())
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then(left.cmp(right))
                })
                .map(|pos| Item::District { district, pos })
                .filter(|item| g.can_produce(pid, cid, item))
            else {
                continue;
            };
            let name = g.cities[&cid].name.clone();
            if let Item::District { district, pos } = item {
                if g.apply(
                    pid,
                    &Action::BuyDistrict {
                        city: cid,
                        district,
                        pos,
                        currency: "gold".to_string(),
                    },
                )
                .is_ok()
                {
                    think!(self.journal(), Faith, Decision,
                        "{} buys a Holy Site for the counterweight", name;
                        "counterweight-builds-its-source: {} presses us and no city of ours on {} holds a Holy Site, so no counterweight Missionary can be bought", threat, faith);
                    return;
                }
            }
            if g.apply(pid, &Action::Produce { city: cid, item }).is_ok() {
                think!(self.journal(), Faith, Decision,
                    "{} puts a Holy Site first for the counterweight", name;
                    "counterweight-builds-its-source: {} presses us and no city of ours on {} holds a Holy Site, so no counterweight Missionary can be bought", threat, faith);
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests;
