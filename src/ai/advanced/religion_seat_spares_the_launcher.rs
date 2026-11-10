//! `religion-seat-spares-the-launcher`: a Religion seat that owns a religion
//! does not open a war to deny the space race of a rival it has yet to
//! convert.
//!
//! A Religious Victory needs the majority of every living major, the
//! launcher included, and a war closes its cities to the corps: the campaign
//! takes no target in a city at war with us (`advanced_missionary_step`), and
//! every soldier of the rival condemns the Apostles already there. Each
//! condemnation also takes our pressure out of the cities around it.
//!
//! Live Russia, Deity, Online speed, six players, 2026-11-10, with the denial
//! war armed. Every late declaration of ours in the five full games read
//! "to deny the space race":
//!
//! - 100022Z: war on Sumer at turn 168. Our faith held the majority of three
//!   majors and led in 6 of Sumer's 12 cities, with 32 Apostles and 107
//!   charges in the field. Sumer refused six peace offers. Seven Apostles
//!   were condemned beside its cities, our lead there fell to 2 of 14 by turn
//!   185, Australia's majority reverted, and Sumer launched at 203 with our
//!   faith on four of five majors and 88 charges unspent.
//! - 093457Z: war on the Inca at 147, refused three peace offers, lost at 176.
//! - 091939Z: war on Mongolia at 166; England and the Cree joined it.
//! - 065910Z, the one win: war on Babylon at 148, peace at 165, and the
//!   Religious Victory two turns after the peace, at 167.
//!
//! None of the four wars stopped a launch. Under the gene the seat keeps the
//! peace with a launcher whose majority it still needs, and its Apostles keep
//! converting it; the spies' rungs of the denial, running wars, defensive
//! faith wars and every other victory target are untouched. Off by default.
//!
//! A launcher that already follows our religion is not spared. Its majority
//! is won, so the war closes nothing to the corps. Across the day's games
//! that opened the denial war (17, with 19 first declarations):
//!
//! - on a rival whose majority was not ours at the declaration: 11 games,
//!   every one lost to its Science victory;
//! - on a rival that already followed our religion: 8 games, and both of the
//!   day's Religious wins are among them (065910Z, Babylon at 148 with three
//!   of five majors ours, won at 167; 113856Z, Rome at 148 and again at 172,
//!   won at 175);
//! - the first four games that spared every launcher (115325Z to 123943Z)
//!   opened no denial war and all lost to a Science victory by turn 200.

use super::*;

impl AdvancedAi {
    /// Whether this seat withholds the space-race war on `rival`. `false`
    /// with the gene off, off the Religion target, with Religious Victory
    /// disabled, before the seat owns a religion, or when `rival` already
    /// follows it (the reading `religion-finishes-majorities` uses).
    pub(super) fn launcher_is_spared(&self, g: &Game, pid: usize, rival: usize) -> bool {
        if !self.religion_seat_spares_the_launcher
            || !g.victory_conditions.religious
            || self.active_victory_target(g) != Some(VictoryTarget::Religion)
        {
            return false;
        }
        let Some(religion) = g.players[pid].religion.as_deref() else {
            return false;
        };
        let converted = match g.observed_majority_religion.get(&rival) {
            Some(faith) => faith == religion,
            None if g.observed_public_empire_stats.contains_key(&rival) => false,
            None => g.civ_follows_religion(rival, religion),
        };
        if converted {
            return false;
        }
        think!(self.journal(), Military, Detail,
               "Religion keeps the peace with {}", g.players[rival].civ;
               "religion-seat-spares-the-launcher: its majority is still needed, and a war \
                closes its cities to our Apostles and lets its soldiers condemn them");
        true
    }
}
