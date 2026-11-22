//! `religious-seat-dedicates-to-the-evangelists`: a Religion seat with a
//! founded religion (or the Great Prophet race still open to it) takes
//! Exodus of the Evangelists whenever the age offers it, ahead of the
//! measured ranking.
//!
//! **Why (live Tournaments 6 and 7, 2026-11-22).** Exodus pays +2 Era Score
//! for every city converted to our founded religion for the first time in a
//! Normal or Dark Age, and in a Golden Age gives Missionaries, Apostles and
//! Inquisitors +2 Movement, newly trained ones +2 Charges, and +4 Great
//! Prophet points a turn: the one Dedication written for a religion seat. The
//! measured ranking ([`crate::ai::choose_dedications`]) reads last era's
//! trigger tally, and on a native board that tally is reconstructed from the
//! log by `mirror/dedication_history.rs`, which counts research and
//! construction only ("not invented counts for conversions"). Exodus's
//! `city_converted` trigger therefore always projects 0 live, and no religion
//! seat in the 14 recorded games since 11-21 ever took it in a Normal or Dark
//! Age: they took Free Inquiry, Pen, Brush and Voice, or Monumentality.
//! Against what the chosen Dedication actually earned (Eurekas and science
//! buildings, Inspirations and culture buildings, districts), 2 Era Score per
//! rise in the seat's cities-following count out-earned it in 29 of 35 Normal
//! and Dark eras (mean +2.9), and would have turned the next age better in
//! 10 (Dark to Normal or Normal to Golden) and worse in 2. Live Mali
//! `civvis-t7-20261122T120728Z` (Lv 7): Dark Ages at t83, t103 (55 of 58) and
//! t153 (83 of 84). Peter `civvis-t7-20261122T130745Z` (Lv 7, Religious win
//! t164) spent its Medieval Golden Age on Monumentality.
//!
//! Off, or on any other seat, or when Exodus is not offered, nothing changes:
//! the ordinary chooser runs as before, and also takes any Dedication left
//! after this one (a Heroic Age offers three).

use super::*;

/// The Dedication this gene takes.
pub(super) const EVANGELISTS: &str = "exodus_of_the_evangelists";

impl AdvancedAi {
    /// Whether `pid` is a seat Exodus of the Evangelists serves: the gene is
    /// on, the seat's active target is Religion, and it has founded a religion
    /// or the Great Prophet race is still open to it.
    pub(super) fn evangelist_seat(&self, g: &Game, pid: usize) -> bool {
        self.religious_seat_dedicates_to_the_evangelists
            && pid < g.players.len()
            && self.active_victory_target(g) == Some(VictoryTarget::Religion)
            && (g.players[pid].religion.is_some() || self.prophet_race_open_for(g, pid))
    }

    /// See the module. Takes Exodus of the Evangelists when the age offers it
    /// and the seat is one it serves; true when it did. Exact no-op otherwise.
    pub(super) fn dedicate_to_the_evangelists(&mut self, g: &mut Game, pid: usize) -> bool {
        if !self.evangelist_seat(g, pid)
            || !g
                .available_dedications(pid)
                .iter()
                .any(|name| *name == EVANGELISTS)
        {
            return false;
        }
        let taken = g
            .apply(
                pid,
                &Action::ChooseDedication {
                    dedication: Name::new(EVANGELISTS),
                },
            )
            .is_ok();
        if taken {
            let age = g.players[pid].age.clone();
            think!(self.journal(), Diplomacy, Decision,
                   "Dedicating the {age} age to Exodus of the Evangelists";
                   "religious-seat-dedicates-to-the-evangelists: +2 Era Score a city \
                    converted to our religion, or in a Golden Age +2 Movement and \
                    Charges for our religious units; the measured ranking never \
                    sees a conversion on this board");
        }
        taken
    }
}

#[cfg(test)]
mod tests;
