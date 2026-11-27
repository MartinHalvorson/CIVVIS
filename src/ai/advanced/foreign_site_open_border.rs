//! `foreign-site-buys-the-open-border`: a held Great Person whose only legal
//! sites lie behind a closed border buys one Open Borders grant there.
//!
//! Two Civilization VI Great Scientists cannot be used on the seat's own
//! ground alone:
//!
//! - **Margaret Mead** (Babylon DLC, Atomic era; 1000 Science and 1000
//!   Culture, about 500 each on Online) activates only on a completed Campus
//!   of a FOREIGN civilization that is not hostile and lets us in (Open
//!   Borders or an Alliance), or of a city-state we are Suzerain of. Her row
//!   reads `ActionRequiresCompletedDistrictType="DISTRICT_CAMPUS"
//!   ActionRequiresOwnedTile="false" ActionRequiresNonHostileTerritory="True"`,
//!   and our own Campuses never qualify.
//! - **Charles Darwin** activates on or beside a natural wonder
//!   (`ActionRequiresOnOrAdjacentNaturalWonder="true"`, owner free).
//!
//! The control mod exports only the activation plots we own (a foreign
//! highlight caused endless MOVE_TOs), so the mirror reads either person as a
//! need with no plot, and the Lua driver walks to a raw highlight only when
//! the host pathfinder reaches it. Without passage it never does: on live
//! T7/T8 runs of 2026-11-26 Mead stood idle t143-t178 (Ethiopia,
//! civvis-t7-20261126T180657Z) and t147-t193 (Russia,
//! civvis-t8-20261126T094623Z) beside 6-9 of our own completed Campuses,
//! with no Open Borders from any of the five majors and no Campus city-state
//! of ours; Darwin stood idle t132-t206 (civvis-t7-20261126T230955Z-cont1)
//! with every known natural wonder inside Brazil, Sweden or Persia. Where a
//! site was open — Palenque or Hattusa as our city-state, an Inca grant, an
//! unowned tile beside Pamukkale or Lake Retba — the person walked there by
//! itself and activated.
//!
//! With the gene on, while the seat holds such a person (any victory lane)
//! and none of that person's known sites is reachable, the seat buys ONE
//! Open Borders grant: from the met, living, not-at-war major whose site
//! tile lies nearest our cities (the mirror does not place Great People on
//! its board, so their own tile is not known here), the next-nearest if that
//! one will not sell. The grant is priced and filtered exactly as the
//! culture lane's ([`AdvancedAi::lump_open_borders_quote`], the culture and
//! science denial filters, `passage-waits-past-the-opening`'s hold,
//! [`AdvancedAi::open_borders_affordable`]'s reserve), with the view's hidden
//! civics revealed for the purchase and restored after it
//! ([`AdvancedAi::reveal_enforcing_civics`]). A site is reachable when it is
//! ours (Darwin only), explored, unowned and unsealed (Darwin only), or
//! held by a seat at peace that lets us in (`Game::has_open_borders`; for
//! Mead a city-state only as its Suzerain). Once the person is used the need is
//! gone and nothing more happens. Headless games retire Great People at
//! once, so only the live mirror can fire this; off, nothing changes.
use super::AdvancedAi;
use crate::game::Game;
use crate::think;
use crate::Pos;
use std::collections::{BTreeMap, BTreeSet};

/// What a held person needs to stand on, read from the host's individual.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum ForeignSite {
    /// Margaret Mead: a foreign civilization's completed Campus.
    ForeignCampus,
    /// Charles Darwin: a natural wonder tile or one beside it.
    NaturalWonder,
}

impl ForeignSite {
    /// The site a held individual (the mirror's spelling, prefix removed and
    /// lowercased) needs; `None` for anyone else.
    pub(super) fn of(individual: &str) -> Option<Self> {
        match individual {
            "margaret_mead" => Some(Self::ForeignCampus),
            "charles_darwin" => Some(Self::NaturalWonder),
            _ => None,
        }
    }

    fn person(self) -> &'static str {
        match self {
            Self::ForeignCampus => "Margaret Mead",
            Self::NaturalWonder => "Charles Darwin",
        }
    }

    fn place(self) -> &'static str {
        match self {
            Self::ForeignCampus => "foreign Campus",
            Self::NaturalWonder => "natural wonder",
        }
    }
}

impl AdvancedAi {
    /// Buy one Open Borders grant for a held person whose sites all lie
    /// behind closed borders. Returns whether one closed.
    pub(super) fn foreign_site_buys_the_open_border(
        &self,
        g: &mut Game,
        pid: usize,
        excluded_partner: Option<usize>,
        culture_threats: &BTreeSet<usize>,
        science_threats: &BTreeSet<usize>,
    ) -> bool {
        if !self.foreign_site_buys_the_open_border {
            return false;
        }
        for site in Self::held_foreign_site_people(g, pid) {
            let Some(sellers) = Self::foreign_site_sellers(g, pid, site, excluded_partner) else {
                continue;
            };
            if self.buy_foreign_site_passage(
                g,
                pid,
                site,
                &sellers,
                culture_threats,
                science_threats,
            ) {
                return true;
            }
        }
        false
    }

    /// The sites the seat's held, unused persons need, each once. Empty in a
    /// headless game, which retires Great People when they are recruited.
    pub(super) fn held_foreign_site_people(g: &Game, pid: usize) -> BTreeSet<ForeignSite> {
        g.players[pid]
            .live_great_person_activation_needs
            .iter()
            .filter_map(|need| need.individual.as_deref().and_then(ForeignSite::of))
            .collect()
    }

    /// Every known tile that would serve `site`, with the seat that owns it.
    fn foreign_site_tiles(g: &Game, site: ForeignSite) -> Vec<(Pos, Option<usize>)> {
        let owner = |pos: Pos| {
            g.map
                .get(pos)
                .and_then(|tile| tile.owner_city)
                .and_then(|city| g.cities.get(&city))
                .map(|city| city.owner)
        };
        match site {
            ForeignSite::ForeignCampus => {
                let campus = crate::name!("campus");
                let mut tiles: Vec<(Pos, Option<usize>)> = g
                    .cities
                    .values()
                    .flat_map(|city| {
                        city.districts
                            .iter()
                            .filter(|(district, _)| g.district_family(**district) == campus)
                            .map(|(_, pos)| (*pos, Some(city.owner)))
                    })
                    .collect();
                tiles.sort_unstable();
                tiles
            }
            ForeignSite::NaturalWonder => {
                let mut tiles = BTreeSet::new();
                for (pos, tile) in g.map.tiles.iter() {
                    if g.tile_is_natural_wonder(tile) {
                        tiles.insert(*pos);
                        tiles.extend(g.nbrs(*pos));
                    }
                }
                tiles
                    .into_iter()
                    .filter(|pos| g.map.tiles.contains_key(pos))
                    .map(|pos| (pos, owner(pos)))
                    .collect()
            }
        }
    }

    /// Whether a person needing `site` may stand on `pos` today.
    fn foreign_site_reachable(
        g: &Game,
        pid: usize,
        site: ForeignSite,
        pos: Pos,
        owner: Option<usize>,
    ) -> bool {
        match owner {
            // Only ground the seat has explored reads as unowned: a live
            // mirror fills the fog with invented, ownerless tiles (beside
            // Torres del Paine three of ten were never revealed on
            // civvis-t7-20261126T230955Z-cont1). Sealed ground belongs to a
            // seat whose city we have not seen.
            None => {
                site == ForeignSite::NaturalWonder
                    && g.players[pid].explored.contains(&pos)
                    && !g.closed_borders.contains(&pos)
            }
            Some(owner) if owner == pid => site == ForeignSite::NaturalWonder,
            Some(owner) => {
                if g.is_at_war(pid, owner) {
                    return false;
                }
                if site == ForeignSite::ForeignCampus && g.players[owner].is_minor {
                    g.suzerain_of(owner) == Some(pid)
                } else {
                    g.has_open_borders(pid, owner)
                }
            }
        }
    }

    /// `None` when a site is already reachable (or none is known); else every
    /// met, living, not-at-war major holding a site, with its site's distance
    /// from our nearest city.
    fn foreign_site_sellers(
        g: &Game,
        pid: usize,
        site: ForeignSite,
        excluded_partner: Option<usize>,
    ) -> Option<BTreeMap<usize, i32>> {
        let anchors: Vec<Pos> = g
            .player_city_ids(pid)
            .into_iter()
            .map(|cid| g.cities[&cid].pos)
            .collect();
        let mut sellers: BTreeMap<usize, i32> = BTreeMap::new();
        for (pos, owner) in Self::foreign_site_tiles(g, site) {
            if Self::foreign_site_reachable(g, pid, site, pos, owner) {
                return None;
            }
            let Some(owner) = owner else {
                continue;
            };
            let player = &g.players[owner];
            if owner == pid
                || Some(owner) == excluded_partner
                || !player.alive
                || player.is_minor
                || player.is_barbarian
                || g.same_team(pid, owner)
                || !g.has_met(pid, owner)
                || g.is_at_war(pid, owner)
            {
                continue;
            }
            let distance = anchors
                .iter()
                .map(|anchor| g.wdist(*anchor, pos))
                .min()
                .unwrap_or(i32::MAX);
            sellers
                .entry(owner)
                .and_modify(|nearest| *nearest = (*nearest).min(distance))
                .or_insert(distance);
        }
        (!sellers.is_empty()).then_some(sellers)
    }

    /// One grant from the nearest seller that will sell. Returns whether one
    /// closed.
    fn buy_foreign_site_passage(
        &self,
        g: &mut Game,
        pid: usize,
        site: ForeignSite,
        sellers: &BTreeMap<usize, i32>,
        culture_threats: &BTreeSet<usize>,
        science_threats: &BTreeSet<usize>,
    ) -> bool {
        let mut ranked: Vec<(i32, usize)> = sellers
            .iter()
            .map(|(partner, distance)| (*distance, *partner))
            .collect();
        ranked.sort_unstable();
        let revealed = Self::reveal_enforcing_civics(g, pid);
        let mut bought = None;
        for &(distance, partner) in &ranked {
            let Some(deal) = Self::lump_open_borders_quote(g, pid, partner)
                .filter(|deal| {
                    self.culture_deal_allowed(deal, culture_threats)
                        && self.science_denial_deal_allowed(deal, science_threats)
                })
                .filter(|deal| Self::open_borders_affordable(g, pid, deal))
                // `passage-waits-past-the-opening`; `None` with it off.
                .filter(|deal| self.passage_hold(g, pid, deal.partner).is_none())
            else {
                continue;
            };
            let price = deal.offer.gold;
            if self.base.close_quick_deal(g, pid, deal) {
                bought = Some((partner, distance, price));
                break;
            }
        }
        for rival in revealed {
            g.players[rival]
                .civics
                .remove(&crate::name!("early_empire"));
        }
        let person = site.person();
        let place = site.place();
        match bought {
            Some((partner, distance, price)) => {
                think!(self.journal(), Diplomacy, Decision,
                       "Buying passage from {} for {person}", g.players[partner].civ;
                       "foreign-site-buys-the-open-border: {person} has no reachable {place}; \
                        theirs is {distance} tiles from our nearest city, the grant costs \
                        {price:.0} Gold");
                true
            }
            None => {
                if g.turn % 6 == pid as u32 % 6 {
                    think!(self.journal(), Diplomacy, Decision,
                           "No passage on sale for {person}";
                           "foreign-site-buys-the-open-border: {person} has no reachable {place} \
                            and none of the {} seats holding one sells passage",
                           ranked.len());
                }
                false
            }
        }
    }
}

#[cfg(test)]
mod tests;
