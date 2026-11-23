//! `science-seat-campus-before-the-settler`: a Science seat's first Campus
//! goes ahead of the early Settler floor's Settler once two cities stand, on
//! `first-campus-before-the-settler`'s terms.
//!
//! **Why (live T6/T7 Civ VI, 1 CIVVIS seat + 5 Firaxis AI, Online, the 44
//! Emperor, Immortal and Deity Science-seat games of 2026-11-21 20:38Z to
//! 2026-11-23 05:50Z).** Writing landed at a median t31, the first Campus
//! (Seowon and Observatory counted) was placed a median 9.5 turns later and
//! finished 4.5 turns after that, so the wait sat in the claim, not in the
//! build. In 15 of the 44 the first Campus came more than 15 turns after
//! Writing (t54 to t92); those 15 won none, the other 24 won 8 (confounded:
//! the slow 15 also stood at 4 cities at t60 against 6). Over the 1,319
//! city-turns `campus-claims-the-free-slot` journalled a postponed first
//! Campus, the early Settler floor held 542, the most of any claim (backlog
//! Builder 219, bound Granary 195, deterrence 159). It held the first Campus
//! in 39 of the 44 games, a median 9 turns from its first hold to the Campus
//! placed (18 to 57 turns in 12 games), and in 352 of its 563 holds the
//! Campus was 7 turns or less from done.
//!
//! `first-campus-before-the-settler` already answers exactly that hold, but
//! only for a Domination or Score seat; `science-campus-before-repeat-settler`
//! covers the Science seat only while another Settler is already walking and
//! the Campus quotes 5 turns or less against a longer Settler, and it never
//! fired in those 44 games.
//!
//! **The gene.** With the active victory target Science (and the Science
//! victory on), `AdvancedAi::first_campus_before_the_settler_safe` passes
//! as it does for a Domination seat, so the same release and the same hold
//! apply unchanged: no Campus standing, founded or queued anywhere, two
//! cities, no war with a major, no rush, threat, raider or fresh attack, a
//! Campus within 7 Online turns (cost-scaled at other speeds) on the best
//! first-Campus site of the empire, and the Settler keeps its progress. The
//! committed Campus keeps its queue against both Settler claims until a
//! Campus stands. It rides `campus-claims-the-free-slot` (or `-2`).
//!
//! **What it costs.** One Settler, once a game, delayed by at most the
//! Campus's own build (7 Online turns, a median 5), about a third of a city
//! at t60, less when another city trains the Settler meanwhile.
//!
//! Grep the journal for `science-seat-campus-before-the-settler:`.

use super::*;

impl AdvancedAi {
    /// Whether this gene lets a Science seat pass
    /// `first_campus_before_the_settler_safe`'s seat test.
    /// Always false with the gene off.
    pub(super) fn science_seat_takes_the_first_campus(&self, g: &Game) -> bool {
        self.science_seat_campus_before_the_settler
            && g.victory_conditions.science
            && self.active_victory_target(g) == Some(VictoryTarget::Science)
    }
}

#[cfg(test)]
mod tests;
