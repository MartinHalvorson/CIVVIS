//! `denounce-only-to-declare`: the science-threat rung no longer denounces
//! the space racer; the one denunciation the seat still issues is the one a
//! war opening asks for.
//!
//! *Let your plans be dark and impenetrable as night, and when you move,
//! fall like a thunderbolt* (Sun Tzu, VII). A denunciation announces
//! hostility. It only pays when a war follows it.
//!
//! `science-threat-denial` rung 1 denounces the most pressing space racer
//! "to keep the formal launcher war legal". But the war desk already
//! denounces on its own when it means to fight: `preferred_war_opening`
//! returns `Action::Denounce` for a target we have not yet denounced, and the
//! campaign, prey and launcher openings all go through it (`launcher-war-
//! ignores-the-edge` opens the launcher war by surprise in any case). So the
//! rung's denunciation adds five turns of formal-war head start in the rare
//! game that goes to war. In every other game it is pure provocation.
//!
//! Live Deity Korea Science seats, 2026-11-04 21Z to 2026-11-05 14Z (89
//! games; scratchpad census `aow/matched.py`, `aow/hazard.py`):
//! - All 263 journalled denunciations were this rung ("their space race is a
//!   science threat"), in 41 of the 89 games. War orders of ours: 4, in 3
//!   games.
//! - A rival at peace that we denounced within 30 turns declared war on us
//!   within 10 turns on 6.0% of rival-turns (129/2157), against 2.2%
//!   (190/8780) for the rest. Stratified by science rank and military ratio,
//!   the Mantel-Haenszel risk ratio is 2.34. Immortal seats from 11-01: 2.78.
//!   A denunciation only from THEM (2.7%) is no riskier than none (2.8%), so
//!   it is our word, not the bad blood, that brings the war.
//! - Wars that began within 20 turns of our denunciation of that rival cost
//!   2.50 cities in 30 turns (n=12), against 1.35 for other wars from t100
//!   (n=23).
//!
//! With the gene on, rung 1 stands down. When it would have denounced, the
//! seat counts `denunciations_withheld`. Rung 1's refusals stay, and so do
//! rungs 2-4: no alliance, research agreement, passage or Great Work for a
//! threat, the spies, and the denial war, which reaches its denunciation
//! through `preferred_war_opening`. `culture-threat-early` is untouched:
//! the census read Science seats only. Off: unchanged.
use super::*;

impl AdvancedAi {
    /// Under the gene, the science-threat rung's denunciation waits for the
    /// war that needs it: returns `true` (the rung stands down) and counts
    /// `denunciations_withheld` when one of `ranked` is a legal denunciation
    /// this turn. Off: `false`, and nothing is read.
    pub(super) fn denunciation_waits_for_the_war(
        &self,
        g: &mut Game,
        pid: usize,
        ranked: &[usize],
    ) -> bool {
        if !self.denounce_only_to_declare {
            return false;
        }
        if !ranked.is_empty() {
            let legal = g.legal_actions_within(pid, ActionFamilies::DIPLOMACY);
            if ranked
                .iter()
                .any(|rival| legal.contains(&Action::Denounce { player: *rival }))
            {
                *g.players[pid]
                    .counters
                    .entry("denunciations_withheld".to_string())
                    .or_insert(0) += 1;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests;
