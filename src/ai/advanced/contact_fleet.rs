//! `contact-fleet`: while the seat still has someone to meet or locate across
//! the water, the sea scout it lays down keeps its queue, sails for foreign
//! landfall, and is the faster hull when one is in reach.
//!
//! **Why (live Immortal, 6 majors, Continents, 2026-10-31).** Run
//! civvis-20261031T003455Z (Ottoman, Diplomatic target, 229 turns) held no
//! ship in any state frame: three of its five rivals stood at 0 known cities
//! from t200 to the end (Japan 0/14, Norway 0/11, America 0/7, all running
//! space projects), Norway won on Science at t229 unlocated, and only three
//! of nine city-states were met by t150. `naval-recon` was live and asked for
//! a sea scout seven times (`reserve_idle_naval_recon`), and six of the seven
//! were gone the same turn: the strategic governor rescores every committed
//! queue (`advanced_production`), and a hull's ordinary worth is small (11,
//! 30, 34) or vetoed outright (-2000), so the next Industrial Zone, Catapult,
//! Bombard, Lighthouse or Machine Gun displaced it — t75 Quadrireme at 11
//! against an Industrial Zone at 78. Cartography (t67) was not the gate: it
//! grants every naval and embarked unit Ocean (`Game::traversal_class`). The
//! land scouts cannot do the job either: an embarked land unit plans only
//! toward fog the LAND prior marks (`mirror::grow_frontier`), so once the
//! home continent is charted the scout walks to a picket post.
//!
//! **What the gene does.** The *contact gap* is open while a living major is
//! unmet, a met major has no city we know (on the board on a tile we have
//! explored, or in `remembered_cities`), or a city-state is unmet
//! (`BasicAi::city_state_contacts_remain`). While it is open:
//! - **Hold.** A sea military unit at the head of one coastal queue keeps it
//!   against the governor's rescoring ([`AdvancedAi::naval_eye_holds`]): no
//!   charting ship of ours is afloat, the city is not threatened or attacked
//!   in the last four turns, it can launch into water that still hides
//!   something, and the build is legal. One city at a time — the one that
//!   finishes its hull soonest.
//! - **Routing.** Once our ships can cross Ocean
//!   (`BasicAi::naval_recon_can_cross_ocean`: Cartography, Mana, or Knarr
//!   with Shipbuilding), a ship's exploration goal takes the long look-ahead
//!   and the foreign-landfall ranking `island-exploration` gives a full home
//!   continent (`BasicAi::exploration_goal`), so it crosses the ocean toward
//!   land that is not ours instead of shaving the home coast. A Galley before
//!   Cartography cannot reach another continent and keeps the ordinary sweep.
//! - **Hull.** Among the hulls costing up to twice the cheapest, the one with
//!   the most moves, when this city finishes it within
//!   [`CONTACT_FLEET_FAST_HULL_MAX_STANDARD`] standard turns
//!   ([`BasicAi::contact_fleet_fast_hull`]) — a Caravel over a Quadrireme in
//!   a working city, the Quadrireme in a two-Production one.
//!
//! What a contact buys the Diplomatic seat: a met major can be named on the
//! World Congress ballot (#4251: the ballot never falls back to a target on
//! us when its leader is unmet), takes a delegation, an embassy and a trade
//! route, and — once one of its cities is located — gives a Spy a pad to
//! Disrupt Rocketry and the denial counters a city to aim at. A met
//! city-state takes Envoys toward suzerainty and its Favor. In 003455Z the
//! rivals entered the seat's view at t16 (Russia) and t29 (Phoenicia), then
//! not until t154 (Japan), t177 (America) and t195 (Norway, the Science
//! winner at t229) — and none of the last three was ever located.
//!
//! The gap closes, and every part stops, once every living major is met and
//! located and no city-state is unmet; the hold also stops as soon as one
//! ship of ours is charting, and when the city has no water left to chart.
//! It runs on every lane: the cost is one hull at a time, and a Domination
//! seat needs the unlocated capitals found as much as a Diplomatic seat needs
//! the city-states (`find-the-capital` is land-only and needs ground already
//! seen). Off, nothing changes.

use super::*;

/// The longest the faster hull may take in the launching city, in standard
/// turns. A Caravel's fourth move saves a few turns of a long voyage; it is
/// not worth twice the build in a city that makes 2 Production.
pub(crate) const CONTACT_FLEET_FAST_HULL_MAX_STANDARD: u32 = 12;

impl BasicAi {
    /// Whether `pid` knows a city of `rival`: one on the board on a tile it
    /// has explored, or one in its city memory. The live board carries only
    /// known cities and the native board every city, so the explored test is
    /// what makes the two agree.
    pub(crate) fn contact_fleet_city_known(g: &Game, pid: usize, rival: usize) -> bool {
        let Some(seat) = g.players.get(pid) else {
            return false;
        };
        g.cities
            .values()
            .any(|city| city.owner == rival && seat.explored.contains(&city.pos))
            || seat
                .remembered_cities
                .values()
                .any(|city| city.owner == rival)
    }

    /// Living majors `pid` has not met, and met majors it has not located.
    pub(crate) fn contact_fleet_missing_majors(g: &Game, pid: usize) -> (usize, usize) {
        g.players
            .iter()
            .filter(|rival| {
                rival.id != pid
                    && rival.alive
                    && !rival.is_minor
                    && !rival.is_barbarian
                    && !rival.is_free_city
                    && !g.same_team(pid, rival.id)
            })
            .fold((0, 0), |(unmet, unlocated), rival| {
                if !g.has_met(pid, rival.id) {
                    (unmet + 1, unlocated)
                } else if !Self::contact_fleet_city_known(g, pid, rival.id) {
                    (unmet, unlocated + 1)
                } else {
                    (unmet, unlocated)
                }
            })
    }

    /// The contact gap: a living major unmet or unlocated, or a city-state
    /// unmet. See the module.
    pub(crate) fn contact_fleet_gap(g: &Game, pid: usize) -> bool {
        Self::city_state_contacts_remain(g, pid) || {
            let (unmet, unlocated) = Self::contact_fleet_missing_majors(g, pid);
            unmet + unlocated > 0
        }
    }

    /// The faster hull `cid` would lay down for the open contact gap: among
    /// the producible sea military units costing at most twice the cheapest,
    /// the most moves, then the cheaper, then the name — if this city
    /// finishes it within [`CONTACT_FLEET_FAST_HULL_MAX_STANDARD`] standard
    /// turns. `None` leaves `best_naval_recon` to its cheapest hull.
    pub(crate) fn contact_fleet_fast_hull(&self, g: &Game, pid: usize, cid: u32) -> Option<String> {
        if !self.contact_fleet || !Self::contact_fleet_gap(g, pid) {
            return None;
        }
        let hulls: Vec<(f64, f64, String)> = g
            .rules
            .units
            .iter()
            .filter(|(name, spec)| {
                spec.class == "military"
                    && spec.domain.as_deref() == Some("sea")
                    && g.can_produce(pid, cid, &Item::Unit { unit: **name })
            })
            .map(|(name, spec)| (spec.cost, spec.moves, name.to_string()))
            .collect();
        let cheapest = hulls.iter().map(|(cost, _, _)| *cost).reduce(f64::min)?;
        let (_, _, unit) = hulls
            .into_iter()
            .filter(|(cost, _, _)| *cost <= 2.0 * cheapest + f64::EPSILON)
            .min_by(|a, b| {
                b.1.total_cmp(&a.1)
                    .then_with(|| a.0.total_cmp(&b.0))
                    .then_with(|| a.2.cmp(&b.2))
            })?;
        let item = Item::Unit {
            unit: Name::new(&unit),
        };
        let turns = g.host_production_turns(cid, &item).unwrap_or_else(|| {
            g.item_remaining_cost_for_city(pid, cid, &item) / g.city_yields(cid).production.max(0.5)
        });
        (turns <= g.standard_duration(CONTACT_FLEET_FAST_HULL_MAX_STANDARD) as f64).then_some(unit)
    }
}

impl AdvancedAi {
    /// Whether `item` is a sea military unit `cid` may keep at the head of its
    /// queue for the contact gap: legal, unthreatened, launching into water
    /// that still hides something, with no charting ship of ours afloat.
    fn contact_fleet_hull_may_hold(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        item: &Item,
    ) -> bool {
        let Item::Unit { unit } = item else {
            return false;
        };
        let Some(spec) = g.rules.units.get(unit.as_str()) else {
            return false;
        };
        let Some(city) = g.cities.get(&cid) else {
            return false;
        };
        spec.class == "military"
            && spec.domain.as_deref() == Some("sea")
            && city.owner == pid
            && plan.threatened_city != Some(cid)
            && !(city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
            && Self::production_commitment_is_legal(g, pid, cid, item)
            && BasicAi::city_has_naval_recon_launch(g, pid, cid)
    }

    /// `contact-fleet`'s hold: the sea scout at the head of `cid` finishes
    /// while the contact gap is open and no ship of ours is charting. Of the
    /// coastal cities that qualify, only the one that finishes its hull
    /// soonest holds. Always false with the gene off. See the module.
    pub(super) fn naval_eye_holds(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        item: &Item,
    ) -> bool {
        if !self.base.contact_fleet
            || !(self.base.naval_recon_on() || self.base.island_exploration_active(g, pid))
            || !self.contact_fleet_hull_may_hold(g, pid, cid, plan, item)
            || !BasicAi::contact_fleet_gap(g, pid)
        {
            return false;
        }
        let charting = g.units.values().any(|unit| {
            unit.owner == pid
                && g.rules.units[unit.kind].class == "military"
                && g.rules.units[unit.kind].domain.as_deref() == Some("sea")
                && unit.linked_to.is_none()
                && BasicAi::naval_recon_ship_can_chart(g, pid, unit.id)
        });
        if charting {
            return false;
        }
        let turns = |city: u32, head: &Item| {
            g.item_remaining_cost_for_city(pid, city, head)
                / g.city_yields(city).production.max(1.0)
        };
        let first = g
            .player_city_ids(pid)
            .into_iter()
            .filter_map(|city| {
                let head = g.cities[&city].queue.first()?;
                self.contact_fleet_hull_may_hold(g, pid, city, plan, head)
                    .then(|| (turns(city, head), city))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)))
            .map(|(_, city)| city);
        if first != Some(cid) {
            return false;
        }
        if self.journal().wants(crate::reasoning::Level::Detail) {
            let (unmet, unlocated) = BasicAi::contact_fleet_missing_majors(g, pid);
            let city_states = g
                .players
                .iter()
                .filter(|other| {
                    other.alive
                        && other.is_minor
                        && !other.is_barbarian
                        && !other.is_free_city
                        && !g.has_met(pid, other.id)
                })
                .count();
            think!(self.journal(), Economy, Detail,
                   "{} keeps its {} for the contact fleet", g.cities[&cid].name,
                   Self::plain_item(item);
                   "contact-fleet: {unmet} major(s) unmet, {unlocated} met but unlocated, \
                    {city_states} city-state(s) unmet, and no ship of ours is charting; \
                    {:.1} turns to launch", turns(cid, item));
        }
        true
    }
}

#[cfg(test)]
mod tests;
