//! `religion-seat-reaches-reformed-church`: a founder whose victory target is
//! Religion keeps a civic goal after Theology.
//!
//! The Religion lane's only civic goal is Theology. After it the seat follows
//! the ordinary civic value, and the live Deity Khmer seat
//! civvis-20261109T022744Z-cont1 played Monarchy and then Communism with
//! military cards and never adopted Reformed Church in 170 turns. That civic
//! opens everything the lane's own card and government lists already name:
//! Simultaneum (+100% Faith from Holy Site buildings), Religious Orders (+5
//! theological strength) and Theocracy. A list that names unreachable cards
//! does nothing.
//!
//! With the gene on, a Religion-target seat that has founded and has Theology
//! beelines Reformed Church. Off, or on any other lane, nothing changes.
//!
//! The existing `religion-seat-fields-apostles` treatment first reaches
//! Theology itself. Native Russia civvis-20261109T154422Z founded at t19,
//! but at t49 took Games and Recreation toward Divine Right and only reached
//! Theology at t81. A later government's prerequisites must not postpone the
//! Temple that enables the treatment's Apostles. The caller still puts the
//! first government and urgent civic goals ahead of this ordinary lane goal.

use super::*;

impl AdvancedAi {
    /// Reach the Apostle unlock, then the separately enabled later Faith civic.
    pub(super) fn religion_seat_civic_goal(&self, g: &Game, pid: usize) -> Option<&'static str> {
        let player = &g.players[pid];
        if self.active_victory_target(g) != Some(VictoryTarget::Religion)
            || player.religion.is_none()
        {
            return None;
        }
        let has_theology = player.civics.contains(&crate::name!("theology"));
        if self.religion_seat_fields_apostles && !has_theology {
            return Some("theology");
        }
        (self.religion_seat_reaches_reformed_church
            && has_theology
            && !player.civics.contains(&crate::name!("reformed_church")))
        .then_some("reformed_church")
    }
}

#[cfg(test)]
mod tests;
