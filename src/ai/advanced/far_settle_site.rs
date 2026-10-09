//! `settler-walks-to-the-better-site`: a Settler that can reach some site
//! this turn no longer takes it over a far better site a turn or two away.
//!
//! ## The live evidence (131 Emperor Gran Colombia runs, 2026-10-07..09)
//!
//! Of 1,224 cities we founded, 49% stood on fresh water (35% of the 5th and
//! later); the rivals' cities at t100 were 64% fresh. 410 of our 617 coastal
//! or dry cities were ordinary foundings, not stall or shelter fallbacks, and
//! 114 of those had a legal fresh-water site within two tiles. A fresh city
//! keeps 5 Housing against 3 on a coast and 2 on dry land, and at t100 our
//! cities without fresh water averaged 4.0 population against 6.0.
//!
//! The ranking already prefers those sites. The final pick does not reach
//! them: `best_reachable_settle_site_except_cached` re-prices the sites the
//! Settler can reach *this turn* (`Game::paths_to`) for route risk and, if
//! any is reachable, returns the best of those alone. Replayed with every
//! candidate printed: G034655Z t51 ranked a fresh site two tiles out at 143.5
//! and founded a dry one a tile out at 77.9; G035618Z t43 ranked a fresh
//! coastal site three tiles out at 113.0 and walked to a dry coast at 50.6.
//!
//! ## What the gene does
//!
//! After the one-turn pick, it takes the best ranked site the one-turn flood
//! cannot reach, if that site is worth more than [`FAR_SITE_MARGIN`] above
//! the one-turn pick and has a route, and sends the Settler there instead.
//! The ranked worth already carries the walk: `1.25` a tile and, under
//! `settle-sooner`, two points a turn. Nothing else changes: when no site is
//! in reach this turn the ranked search decides exactly as before. Off (the
//! default) the one-turn pick always stands.
use super::AdvancedAi;
use crate::ai::BasicAi;
use crate::game::Game;
use crate::Pos;
use std::collections::BTreeMap;

/// How much more a site beyond this turn's walk must be worth than the best
/// site within it: about a turn and a half of `settle-sooner`'s walk price
/// on top of the distance the ranking already charges, so a near tie is
/// founded now.
pub(super) const FAR_SITE_MARGIN: f64 = 12.0;

impl AdvancedAi {
    /// The far site that should replace `near`, the best one-turn pick: the
    /// best-ranked routable candidate outside `paths` worth more than
    /// `near` by [`FAR_SITE_MARGIN`]. `None` with the gene off.
    pub(super) fn better_site_beyond_the_turn(
        &self,
        g: &Game,
        uid: u32,
        candidates: &[(Pos, f64)],
        paths: &BTreeMap<Pos, Vec<Pos>>,
        near: (Pos, f64),
    ) -> Option<(Pos, f64)> {
        if !self.settler_walks_to_the_better_site {
            return None;
        }
        let far: Vec<(Pos, f64)> = candidates
            .iter()
            .filter(|(position, value)| {
                !paths.contains_key(position) && *value > near.1 + FAR_SITE_MARGIN
            })
            .copied()
            .collect();
        if far.is_empty() {
            return None;
        }
        BasicAi::first_reachable_settle_site(g, uid, &far)
    }
}

#[cfg(test)]
mod tests;
