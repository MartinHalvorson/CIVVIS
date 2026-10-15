//! `medina-quarter-houses-the-districts`: the deck plays the one housing card
//! a district-heavy empire can reach, while its housing caps growth.
//!
//! Medina Quarter (+2 Housing in every city with at least three specialty
//! districts; Economic; Medieval Faires) is modelled in `data/policies.json`
//! (`housing_at_3_districts`), but no lane portfolio in
//! `AdvancedAi::strategic_policies` names it, and the base deck's reading does
//! not price growth, so it is never played. Insulae, the earlier housing card
//! (`ai.rs` `POLICY_PRIORITY`), leaves the host's menu at Medieval Faires, so
//! from there to New Deal the deck holds no housing card at all.
//!
//! Over 32 Emperor passers and near-passers of 10-14/15 the card was on the
//! menu in 31-32 games at t100-t140 and slotted in none. Per-city production
//! at t140 was 0.72x the top producer's, 71% of it population per city (food
//! per citizen equal; production per citizen ahead of the AI once its x1.4 is
//! removed), and 42% of our cities sat housing-limited (growth x0.25 or x0.5;
//! 26% of growth lost on average against 10% to Amenities). Housing-limited
//! cities that already hold three specialty districts: 22 at t100, 36 at t120,
//! 67 at t140 (about two a game, half of all housing-limited cities).
//!
//! With the gene on, while at least [`MEDINA_QUARTER_ENTER_CITIES`] of our
//! cities are housing-limited (Housing minus Population under 2, read without
//! the card's own +2) and hold three specialty districts, the card joins the
//! portfolio ahead of the plan's ordinary cards (behind any wartime gold or
//! treasury card the deck puts first); a held card stays wanted while at least
//! [`MEDINA_QUARTER_STAY_CITIES`] qualifies. It fills a free slot, or else
//! displaces the weakest slotted Economic card: the one the plan ranks lowest
//! (a card the plan does not want before any card it does), never a
//! production, Builder or Settler card ([`MEDINA_QUARTER_SPARES`]) nor a card
//! the deck protects (culture defence, a timed Nobel Peace card, the Builder
//! window, an upgrade card, the race card, wartime Economic Union).
//!
//! Succession: the shipped rows carry no `replaces` for Medina Quarter or New
//! Deal, so the deck's successor retirement can never drop Medina Quarter as
//! someone's predecessor; a slotted Insulae is an Economic card outside the
//! spares and is the first to make way for it.

use super::AdvancedAi;
use crate::game::Game;
use crate::name::Name;

/// Housing-limited cities with three specialty districts that bring the card
/// onto the deck.
pub(super) const MEDINA_QUARTER_ENTER_CITIES: usize = 2;

/// A slotted card stays wanted while at least this many cities qualify.
pub(super) const MEDINA_QUARTER_STAY_CITIES: usize = 1;

/// The card's Housing in a city with three specialty districts
/// (`data/policies.json`, `housing_at_3_districts`).
const MEDINA_QUARTER_HOUSING: f64 = 2.0;

/// Housing minus Population below this throttles growth
/// (`CITY_HOUSING_LEFT_50PCT_GROWTH` is 1; see `Game::housing_growth_mult`).
const MEDINA_QUARTER_HEADROOM: f64 = 2.0;

/// Production, Builder and Settler Economic cards: another lane's levers,
/// never evicted for the housing card.
pub(super) const MEDINA_QUARTER_SPARES: [&str; 11] = [
    "urban_planning",
    "serfdom",
    "public_works",
    "ilkum",
    "colonization",
    "corvee",
    "gothic_architecture",
    "five_year_plan",
    "collectivization",
    "economic_union",
    "expropriation",
];

impl AdvancedAi {
    /// Our cities that hold three specialty districts and are housing-limited
    /// without the card's own +2.
    pub(super) fn medina_quarter_qualifying_cities(g: &Game, pid: usize) -> usize {
        let held = g.players[pid]
            .policies
            .contains(&crate::name!("medina_quarter"));
        let own = if held { MEDINA_QUARTER_HOUSING } else { 0.0 };
        g.player_city_ids(pid)
            .iter()
            .filter(|cid| {
                let city = &g.cities[*cid];
                g.city_specialty_district_count(city) >= 3
                    && g.city_housing(city) - own - (city.pop as f64) < MEDINA_QUARTER_HEADROOM
            })
            .count()
    }

    /// `Some(qualifying cities)` when the gene wants the card this review.
    pub(super) fn medina_quarter_wanted(&self, g: &Game, pid: usize) -> Option<usize> {
        if !self.medina_quarter_houses_the_districts {
            return None;
        }
        let card = crate::name!("medina_quarter");
        let held = g.players[pid].policies.contains(&card);
        if !held && !g.available_policies(pid).contains(&card) {
            return None;
        }
        let qualifying = Self::medina_quarter_qualifying_cities(g, pid);
        let bar = if held {
            MEDINA_QUARTER_STAY_CITIES
        } else {
            MEDINA_QUARTER_ENTER_CITIES
        };
        (qualifying >= bar).then_some(qualifying)
    }

    /// Whether the housing card may take `current`'s slot: an Economic card
    /// outside the spares. The deck's own protections (culture defence, the
    /// timed and window cards, wartime Economic Union) are applied by the
    /// caller beside it.
    pub(super) fn medina_quarter_may_displace(g: &Game, current: &str) -> bool {
        current != "medina_quarter"
            && g.rules
                .policies
                .get(current)
                .is_some_and(|spec| spec.slot == "economic")
            && !MEDINA_QUARTER_SPARES.contains(&current)
    }

    /// Orders displaceable cards weakest first: the lowest-ranked in
    /// `desired` (an unwanted card before any wanted one), then the older
    /// civic, then the name.
    pub(super) fn medina_quarter_weakest_first(g: &Game, desired: &[&str], cards: &mut [Name]) {
        let rank = |card: &str| {
            desired
                .iter()
                .position(|wanted| *wanted == card)
                .unwrap_or(usize::MAX)
        };
        let era = |card: &str| {
            g.rules
                .policies
                .get(card)
                .and_then(|policy| policy.civic.as_ref())
                .and_then(|civic| g.rules.civics.get(civic))
                .map_or(0, |civic| civic.era)
        };
        cards.sort_by(|first, second| {
            rank(second.as_str())
                .cmp(&rank(first.as_str()))
                .then(era(first.as_str()).cmp(&era(second.as_str())))
                .then(first.cmp(second))
        });
    }
}

#[cfg(test)]
mod tests;
