//! `science-seat-campus-before-the-claims`: on a Science seat a city's first
//! Campus, then its Library, go ahead of the economy claims that keep the
//! queue, and the empire's first Campus goes ahead of the floor's Settler.
//!
//! **Why (live Deity, 6-player Continents, Online, 250 turns, the nine games
//! of 2026-11-18 04:58Z to 11:16Z: five on Science, four on Diplomacy).** No
//! Science seat won, and none held more than 0.71 of the best rival's techs
//! at t150 (a Science win has needed 0.82). Writing landed at t27-37 in every
//! game, but in cities founded by t70 the Science seats placed the first
//! Campus at a median t79 against the Diplomatic seats' t73, and the
//! empire's first three Campuses at t55 / t68 / t79 against t38 / t58 / t64.
//! The Science capitals placed theirs at t35, t68, t80, t83 and t104; Lisbon
//! (`civvis-20261118T084012Z`) held a four-to-seven-turn Campus on offer for
//! 77 turns and trained five Settlers, five soldiers, three Traders, two
//! Catapults, a Siege Tower, Walls, two Granaries, a Harbor and an
//! Entertainment Complex first.
//!
//! Over the 1,405 city-turns between a Campus on the host's menu and its
//! placing (39 Science cities), `campus-claims-the-free-slot` journalled
//! what held it on 560: the early floor's Settler 162, the backlog Builder
//! 100, near-rival deterrence 86, a head it may not defer 82 (Walls 58, a
//! Harbor with progress 16), the bound Granary 77, an item finishing that
//! turn 28. Of the Settler's 162, 93 were turns with no Campus anywhere in
//! the empire, at three to five cities; of the other 69, 52 came at six
//! cities or more and 17 at four or five. The rest wrote no line: 264 turns the Campus
//! took over the claim's fifteen-turn budget, 249 the city's slot had gone to
//! a Government Plaza or Diplomatic Quarter, 206 fell under the war's hold,
//! 113 in a city of one citizen.
//!
//! **This is not a Science-lane rule.** The claim chain is the same on every
//! seat: the Campus claim runs after the Settler, Builder, Monument, Granary,
//! deterrent, Trader, Aqueduct and Industrial Zone claims
//! (`run_idle_queue_claims`) and yields to each one's head, and the two genes
//! that let it pass them, `first-campus-before-the-settler` and
//! `campus-and-library-before-the-claims`, read
//! [`AdvancedAi::conquest_or_score_seat`], so they do nothing on a Science or
//! Diplomatic seat. The Diplomatic seats' holds have the same shape (per
//! city: Settler 2.4, Builder 2.4, Granary 1.8 turns against the Science
//! seats' 4.2, 2.6 and 2.0). What separated the nine games was not the lane:
//! three of the four Diplomatic games, both wins among them, ran without six
//! early-defence genes that all five Science games carried (Walls and
//! soldiers took 23% of Science production over t30-100 against 12%), three
//! of the five Science games were at war before t100 and no Diplomatic game
//! was, and three Diplomatic capitals stood idle the turn Writing landed.
//! The paired simulator, one genome on both lanes, reads the Science lane
//! ahead (Science 79 against 68 at t100).
//!
//! **The gene.** While the active victory target is Science, it plays the
//! two Domination genes' rules for the first Campus and the Library, with or
//! without those genes, in a city of ours that is not the plan's threatened
//! city, was not attacked within four turns and, while a major is at war
//! with us, stands away from the front:
//!
//! 1. The empire's first Campus goes ahead of the early floor's Settler once
//!    two cities stand, by `first-campus-before-the-settler`'s rule: no war
//!    with a major, no rush, no threatened city, no raider in the ring, a
//!    Campus of at most seven Online turns on the empire's best site. The
//!    Settler keeps its progress and the committed Campus keeps its queue
//!    against both Settler claims until a Campus stands.
//! 2. Any city's first Campus that `campus-claims-the-free-slot` (or `-2`)
//!    would claim but for a hold, and a standing Campus's Library, go ahead
//!    of a Builder (the backlog claims' included), a Granary, an Aqueduct,
//!    an Industrial Zone and a Commercial Hub or its Market. A Settler with
//!    at most half its cost paid waits for either from
//!    [`SCIENCE_SEAT_CAMPUS_SETTLER_MIN_CITIES`] cities, counting founded
//!    cities plus Settlers walking. Every such city claims its Library each
//!    turn; a first Campus with Production in keeps its queue against both
//!    Settler claims at any count.
//!
//! **What keeps its queue.** Every soldier: unlike the Domination gene this
//! one never passes the near-rival deterrent, a siege reservation's gun or a
//! routine unit, at peace or at war, and the deterrent claim does not spare
//! a city for it. The faith defence's Holy Site and Shrine are not passed
//! either, and its writers are not held off. Walls, the capital's archers
//! and the garrison writers run after the claims and are not read here. So
//! do a wonder, a project, a Trader, a district with progress other than
//! those named, a Settler below the city count or more than half paid, saved
//! work within two turns of done under `near-done-work-finishes`, and
//! anything finishing this turn.
//! The University keeps `campus-building-claims-the-queue`'s own rules: on
//! these seats it waited a median four turns, behind the Plaza's buildings
//! and repairs, not behind a claim. Off, nothing changes.
//!
//! Grep the journal for `science-seat-campus-before-the-claims:`.

use super::*;

/// The journal tag of the gene's lines.
pub(crate) const SCIENCE_SEAT_CAMPUS_TAG: &str = "science-seat-campus-before-the-claims";

/// Founded cities plus Settlers walking from which a Science seat's first
/// Campus goes ahead of a Settler with at most half its cost paid. The
/// Library's count is `campus-and-library-before-the-claims`' four.
pub(crate) const SCIENCE_SEAT_CAMPUS_SETTLER_MIN_CITIES: usize = 4;

/// EXPERIMENT-ONLY (the paired-sim arms; replaced by the constant before the
/// PR is ready): the count, read once from the environment.
pub(super) fn science_seat_campus_settler_min_cities() -> usize {
    static COUNT: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *COUNT.get_or_init(|| {
        std::env::var("CIVVIS_EXPERIMENT_SCIENCE_CAMPUS_SETTLER_CITIES")
            .ok()
            .and_then(|text| text.parse().ok())
            .unwrap_or(SCIENCE_SEAT_CAMPUS_SETTLER_MIN_CITIES)
    })
}

impl AdvancedAi {
    /// Whether the gene plays for this seat: it is on and the active victory
    /// target is Science. Always false with the gene off.
    pub(super) fn science_seat_campus_first(&self, g: &Game) -> bool {
        self.science_seat_campus_before_the_claims
            && self.active_victory_target(g) == Some(VictoryTarget::Science)
    }

    /// Whether either gene that puts the first Campus and the Library ahead
    /// of the claims is on. Each still asks its own seat
    /// ([`Self::campus_and_library_reach`]).
    pub(super) fn campus_and_library_armed(&self) -> bool {
        self.campus_and_library_before_the_claims || self.science_seat_campus_before_the_claims
    }

    /// The gene a first Campus or Library put first is journalled under on
    /// this seat.
    pub(super) fn campus_and_library_tag(&self, g: &Game) -> &'static str {
        if self.science_seat_campus_first(g) {
            SCIENCE_SEAT_CAMPUS_TAG
        } else {
            "campus-and-library-before-the-claims"
        }
    }
}

#[cfg(test)]
mod tests;
