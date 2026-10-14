//! `phantom-capture-keeps-the-siege`: a capture the planning board makes on
//! damage the same frame dealt commits no siege state, and a committed
//! capture the host never executed resumes its siege over a falling city.
//!
//! **Why (live Emperor civvis-20261014T130619Z, census 10-12..14).** Egypt's
//! Scone stood behind fallen walls at 20-60 health on turns 180-190 and never
//! fell. On turn 183 the opening board applied the bombers' volley (60 -> 1)
//! and the siege's helicopter "closed in" and "joined the assault ... captured
//! true" behind it; `air_assault::defer_followups` withheld every order after
//! the volley until the host had executed it, so no helicopter order ever
//! left. The board's capture had already written the siege to Hold, released
//! its taker, counted a capture and logged "Captured Scone: keep"; the freed
//! bodies took other work ("rides out to raid behind the wing"), and the
//! frames re-planned from the host found a 6-unit force (384 strength)
//! against a 580 bill, so "the mustered train holds" while the bombed city
//! healed. 12 of 40 "Captured X" decisions since 10-12 were never captures
//! (6 runs); 3 of 12 "joins the assault / taken by" claims (Sais, Plock,
//! Scone).
//!
//! The rule reads the host, not the board: `observe_frame_cities` pins each
//! city's owner and health as the host reported them when the frame opened.
//! A capture made while the board holds the city below that health leans on
//! this frame's own earlier damage — a volley the executor defers, a shot or
//! another unit's blow that per-unit queues do not order before it — and so
//! is contingent: the siege keeps its stage, taker and members until a host
//! board reports the city as ours, and its keep/raze line waits with it.

use super::siege_train::SiegeStage;
use super::AdvancedAi;
use crate::game::{Action, Game};

impl AdvancedAi {
    /// Pin each city's host-reported owner and health before this frame is
    /// planned. Live adapters call it once per frame; nothing else sets it,
    /// so off the live adapter (and with the gene off) no capture is ever
    /// contingent.
    pub fn observe_frame_cities(&mut self, g: &Game) {
        if !self.phantom_capture_keeps_the_siege {
            return;
        }
        self.phantom_frame_cities = Some(
            g.cities
                .values()
                .map(|city| (city.id, (city.owner, city.hp)))
                .collect(),
        );
        self.phantom_contingent_captures.clear();
    }

    /// Whether a capture of `cid` attempted on this board now leans on damage
    /// this frame's board dealt: the host reported the city, under the same
    /// owner, at more health than the board holds it at.
    pub(super) fn capture_is_contingent(&self, g: &Game, cid: u32) -> bool {
        self.phantom_capture_keeps_the_siege
            && self
                .phantom_frame_cities
                .as_ref()
                .and_then(|cities| cities.get(&cid))
                .zip(g.cities.get(&cid))
                .is_some_and(|(&(owner, health), city)| owner == city.owner && city.hp < health)
    }

    /// A batch that strikes before it captures (an air maneuver's volley and
    /// its capture body) is contingent whenever it carries a strike.
    pub(super) fn batch_capture_is_contingent(
        &self,
        g: &Game,
        cid: u32,
        actions: &[Action],
    ) -> bool {
        self.capture_is_contingent(g, cid)
            || (self.phantom_capture_keeps_the_siege
                && self.phantom_frame_cities.is_some()
                && actions.iter().any(|action| {
                    matches!(action, Action::Ranged { .. } | Action::AirStrike { .. })
                }))
    }

    /// After the board applied a capture: whether its siege may commit it.
    /// A contingent capture is remembered, for the keep/raze journal, and
    /// commits nothing.
    pub(super) fn board_capture_commits(
        &mut self,
        g: &Game,
        pid: usize,
        cid: u32,
        contingent: bool,
    ) -> bool {
        if !contingent || !g.cities.get(&cid).is_some_and(|city| city.owner == pid) {
            return true;
        }
        self.phantom_contingent_captures.insert(cid);
        false
    }

    /// Note a board capture that took no siege path (a breach finisher, an
    /// air maneuver), so its keep/raze line reads as provisional.
    pub(super) fn note_board_capture(&mut self, g: &Game, pid: usize, cid: u32, contingent: bool) {
        let _ = self.board_capture_commits(g, pid, cid, contingent);
    }

    /// A committed capture is about to turn its siege to Hold: keep the stage
    /// and taker it held, so a host that never executes the capture finds the
    /// siege as it was.
    pub(super) fn remember_hold_from(&mut self, cid: u32) {
        if !self.phantom_capture_keeps_the_siege {
            return;
        }
        if let Some(siege) = self.sieges.get(&cid) {
            if siege.stage != SiegeStage::Hold {
                self.phantom_hold_from
                    .insert(cid, (siege.stage, siege.taker));
            }
        }
    }

    /// The stage and taker a phantom Hold over `cid` replaced.
    pub(super) fn phantom_held_from(&self, cid: u32) -> Option<(SiegeStage, Option<u32>)> {
        self.phantom_hold_from.get(&cid).copied()
    }

    /// Forget the remembered stage once the host settles the city either way.
    pub(super) fn forget_hold_from(&mut self, cid: u32) {
        self.phantom_hold_from.remove(&cid);
    }

    /// `assess_siege`: this frame's board captured the city while the host
    /// still reported it as the enemy's. The record stays as it stands until
    /// a host board settles it.
    pub(super) fn board_only_capture(&self, g: &Game, pid: usize, cid: u32) -> bool {
        self.phantom_capture_keeps_the_siege
            && g.cities.get(&cid).is_some_and(|city| city.owner == pid)
            && self
                .phantom_frame_cities
                .as_ref()
                .and_then(|cities| cities.get(&cid))
                .is_some_and(|&(owner, _)| owner != pid)
    }

    /// The journal's line for a contingent capture, in place of the siege's
    /// "taken" bookkeeping.
    pub(super) fn journal_contingent_capture(&self, g: &Game, cid: u32, kind: &str) {
        if !self.journal().wants(crate::reasoning::Level::Detail) {
            return;
        }
        let Some(city) = g.cities.get(&cid) else {
            return;
        };
        let reported = self
            .phantom_frame_cities
            .as_ref()
            .and_then(|cities| cities.get(&cid))
            .map_or(0, |&(_, health)| health);
        crate::think!(self.journal(), Military, Detail,
            "Siege of {}: the {} plans the capture", city.name, crate::reasoning::plain(kind);
            "it leans on this frame's own damage (the host reported {reported} health); \
             the siege commits when a host board shows the city as ours";
            city.pos);
    }

    /// The keep/raze line for a contingent capture waits for the host.
    pub(super) fn capture_is_provisional(&self, cid: u32) -> bool {
        self.phantom_capture_keeps_the_siege && self.phantom_contingent_captures.contains(&cid)
    }
}
