//! `founder-funds-the-inquisition`: once a rival faith threatens a founder at
//! home, its Faith goes first to the Apostle that launches the Inquisition and
//! then to the Inquisitors, before cover Missionaries, Faith-bought Builders,
//! Great People, buildings or units.
//!
//! The Inquisition is a founder's real defence: Inquisitors remove a rival
//! faith from our cities and win theological combat at home, and they can be
//! bought only after an Apostle (200 Faith on Online, behind a Temple and
//! Theology) launches it. `prepare_defensive_inquisition` already saves for
//! the Apostle, but spends the savings on a cover Missionary whenever a rival
//! spreader nears a source, and the other Faith sinks never knew about the
//! saving at all. Live Emperor G215 (civvis-20261006T092204Z): Buddhism
//! founded at 47, Temple at 57, Theology at 52; from 48 to 92 the seat spent
//! over 700 Faith on seven Missionaries (three of them "Preserving a
//! threatened faith source" covers) and three Faith-bought Builders, and the
//! Apostle was first affordable at 92, two turns before Bogotá fell to
//! Orthodoxy; Georgia won on Religion at 136. G213 (civvis-20261006T084640Z),
//! the only October 6 Emperor founder that launched the Inquisition (Apostle
//! at 61, launched at 64), held as the last major outside Confucianism from
//! turn 100 to 149.
//!
//! Under the gene, while [`AdvancedAi::inquisition_first_price`] names the
//! next unit (the Apostle until the Inquisition is launched, then Inquisitors
//! up to the shipped Inquisitor cap):
//! - `prepare_defensive_inquisition` buys no cover Missionary;
//! - Great Person patronage, Faith buildings and Faith-bought military keep
//!   its price in the bank, and the delegated governor's Faith-Builder
//!   threshold (`w.faith_builder`) is raised by it for the call.
//! Off: unchanged.

use super::*;

impl AdvancedAi {
    /// A rival faith has reached a founder at home: the shipped conversion
    /// threat (`home_conversion_threat`), or a city of ours whose majority is
    /// another founder's faith.
    pub(super) fn rival_faith_at_home(&self, g: &Game, pid: usize) -> bool {
        let Some(own) = g.players[pid].religion.as_deref() else {
            return false;
        };
        self.home_conversion_threat(g, pid).is_some()
            || g.player_city_ids(pid).into_iter().any(|cid| {
                g.city_religion(&g.cities[&cid]).is_some_and(|faith| {
                    faith != own
                        && g.players
                            .iter()
                            .any(|p| p.id != pid && p.religion.as_deref() == Some(faith))
                })
            })
    }

    /// The Faith price of the next unit the Inquisition needs, while the gene
    /// asks for it first: the Apostle until the Inquisition is launched (none
    /// once an Apostle of ours is in the field), then an Inquisitor while
    /// fewer than the shipped cap (`religious_spending_with_reserve`'s
    /// `inquisitor_cap`) are in the field. `None` with the gene off, for a
    /// seat with no religion, without a rival faith at home, before the
    /// Inquisition can be prepared (`defensive_inquisition_ready`), or when no
    /// city of our faith sells the unit.
    pub(super) fn inquisition_first_price(&self, g: &Game, pid: usize) -> Option<f64> {
        if !self.founder_funds_the_inquisition
            || g.players[pid].religion.is_none()
            || !self.rival_faith_at_home(g, pid)
            || !self.defensive_inquisition_ready(g, pid)
        {
            return None;
        }
        let own = g.players[pid].religion.clone()?;
        let launched = g.players[pid]
            .counters
            .get("inquisition")
            .copied()
            .unwrap_or(0)
            > 0;
        let in_field = |kind: &str| {
            g.units
                .values()
                .filter(|unit| {
                    unit.owner == pid
                        && unit.kind == kind
                        && unit.charges > 0
                        && unit.religion.as_deref() == Some(own.as_str())
                })
                .count()
        };
        let wanted = if launched {
            let pressed = g
                .player_city_ids(pid)
                .into_iter()
                .any(|cid| Self::city_needs_religious_support(g, pid, &g.cities[&cid], &own));
            let veto = self.religious_veto_engaged(g, pid);
            let cap = if pressed {
                2 + Self::religious_veto_extra_inquisitors(veto.as_ref())
            } else {
                0
            };
            if in_field("inquisitor") >= cap {
                return None;
            }
            "inquisitor"
        } else {
            if in_field("apostle") > 0 {
                return None;
            }
            "apostle"
        };
        g.player_city_ids(pid)
            .into_iter()
            .filter(|cid| g.city_religion(&g.cities[cid]) == Some(own.as_str()))
            .filter_map(|cid| Self::inquisition_unit_price(g, pid, cid, wanted))
            .min_by(f64::total_cmp)
            // `founder-defends-its-cities`: a unit more than a few turns of
            // Faith away holds nothing back.
            .filter(|price| !self.founder_inquisition_out_of_reach(g, pid, *price))
    }

    /// The Faith price of `unit` in `cid`. The host's purchase menu lists only
    /// what the bank can pay for now, and the mirror takes the menu as the
    /// answer, so an Apostle we are saving for has no quote at all on a live
    /// board (G215 turn 65: 114 Faith, Bogotá with Holy Site, Shrine and
    /// Temple, no Apostle on the menu). Then the model's own price stands in,
    /// twice the unit's cost at the game's speed (the Apostle's 200 on Online,
    /// the host's quote at G215 turn 92), where the city could sell it: a Holy
    /// Site and the unit's required building.
    pub(super) fn inquisition_unit_price(
        g: &Game,
        pid: usize,
        cid: u32,
        unit: &str,
    ) -> Option<f64> {
        g.unit_purchase_cost(pid, cid, unit, "faith").or_else(|| {
            let spec = g.rules.units.get(unit)?;
            let city = &g.cities[&cid];
            let required = spec.requires_building.is_none_or(|required| {
                city.buildings.iter().any(|building| {
                    !city.pillaged_buildings.contains(building)
                        && g.building_is_family(building, required)
                })
            });
            (required && g.city_has_district_family(city, crate::name!("holy_site")))
                .then(|| g.game_speed.scale(spec.cost * 2.0))
        })
    }

    /// Faith the other Faith sinks leave for the Inquisition: the price from
    /// [`Self::inquisition_first_price`], else zero.
    pub(super) fn inquisition_faith_reserve(&self, g: &Game, pid: usize) -> f64 {
        self.inquisition_first_price(g, pid).unwrap_or(0.0)
    }
}

#[cfg(test)]
mod tests;
