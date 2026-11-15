//! `passage-waits-past-the-opening`: the seat buys no Open Borders before
//! Online turn 80, and after it none from a rival whose military is more than
//! one and a half times ours.
//!
//! Since #5323 (2026-11-15 09:40Z) the mod closes a rival's priced answer to
//! our Open Borders ask, so the ordinary passage buy — the quick-deal quote
//! the planner takes on its six-turn trade slot, 28 Gold by
//! `Game::passage_gold_value` — now actually lands, at turns 24-42 of every
//! live game. Read off `~/civvis-civ6-runs/control/civvis-20261115T*` (Deity,
//! 6p Small Continents, Online): before #5323 a rival granted us passage by
//! turn 60 in 1 of 12 games and a rival declared on us by turn 60 in 3 of 12;
//! from 09:34Z grants by turn 60 in 31 of 35 and rival wars by turn 60 in 12
//! of 35, 9 of them by a rival that had granted us passage first, 1 to 14
//! turns before (France granted at 25 and declared at 26; Rome's attacker
//! granted at 31 and declared at 32). All nine were Unfriendly when they
//! granted and fielded 1.5 to 5.6 times our military. 8 original capitals
//! fell in those 35 games against 1 in the 12 before. Small n; the purchase
//! is what changed.
//!
//! The opinion row "We had troops on their border"
//! (`STANDARD_DIPLOMATIC_NEAR_BORDER_WARNING`, Leaders.xml: our units near
//! their culture border while at peace) appeared before none of the nine
//! declarations, and our combat units stood in or beside the attacker's land
//! after the grant in only three, so the lever here is the purchase itself,
//! not where our units walk once it is bought.
//!
//! With the gene on, every passage purchase the seat would make is withheld:
//! the planner's quick-deal buy on the trade slot (all objectives),
//! `culture-keeps-every-border-open`'s lump buys, and the live bridge's own
//! passage lane (`append_border_buy_order` in `civvis_orders`: exploration,
//! `find-the-capital`, `siege-buys-the-passage`). The trade slot goes to the
//! next deal the ordinary filters allow. Each withheld purchase journals one
//! line naming the rival, the turn and the reason. Off, nothing changes.
use super::city_state_harvest::harvest_floor;
use super::AdvancedAi;
use crate::game::{Game, QuickDeal};
use crate::think;
use std::collections::BTreeMap;

/// The Online turn before which no passage is bought; scaled to the game's
/// speed the way `city_state_harvest::harvest_floor` scales its floors.
pub(crate) const PASSAGE_OPENING_ONLINE_TURNS: u32 = 80;

/// After the opening, a rival whose military is more than this many times
/// ours is not paid for passage.
pub(crate) const PASSAGE_RIVAL_MILITARY_RATIO: f64 = 1.5;

/// Why a passage purchase from one rival is withheld this turn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PassageHold {
    /// The game is still inside the opening, which ends on `until`.
    Opening { until: u32 },
    /// The rival's military is over [`PASSAGE_RIVAL_MILITARY_RATIO`] times ours.
    Stronger { theirs: f64, ours: f64 },
}

impl PassageHold {
    fn reason(&self, turn: u32) -> String {
        match *self {
            PassageHold::Opening { until } => {
                format!("turn {turn} is inside the opening, which ends at turn {until}")
            }
            PassageHold::Stronger { theirs, ours } => format!(
                "turn {turn}: their military {theirs:.0} is {:.1} times our {ours:.0}, over {PASSAGE_RIVAL_MILITARY_RATIO}",
                theirs / ours.max(1.0)
            ),
        }
    }
}

/// Whether a quote buys us passage: their Open Borders on what we receive.
fn buys_passage(deal: &QuickDeal) -> bool {
    deal.request.open_borders
}

impl AdvancedAi {
    /// Whether `passage-waits-past-the-opening` is on (the live bridge reads
    /// it before its own passage lane).
    pub fn passage_waits_past_the_opening_enabled(&self) -> bool {
        self.passage_waits_past_the_opening
    }

    /// Why `pid` does not buy passage from `partner` this turn: every rival
    /// before the opening ends, and after it a rival over one and a half
    /// times our military. `None` with the gene off or when the purchase may
    /// go ahead.
    pub fn passage_hold(&self, g: &Game, pid: usize, partner: usize) -> Option<PassageHold> {
        if !self.passage_waits_past_the_opening || partner == pid {
            return None;
        }
        let until = harvest_floor(g, PASSAGE_OPENING_ONLINE_TURNS);
        if g.turn < until {
            return Some(PassageHold::Opening { until });
        }
        let ours = g.military_power(pid).max(0.0);
        let theirs = g.military_power(partner).max(0.0);
        (theirs > PASSAGE_RIVAL_MILITARY_RATIO * ours)
            .then_some(PassageHold::Stronger { theirs, ours })
    }

    /// Every met living major whose passage `pid` withholds this turn. Empty
    /// with the gene off.
    pub(super) fn passage_holds(&self, g: &Game, pid: usize) -> BTreeMap<usize, PassageHold> {
        if !self.passage_waits_past_the_opening {
            return BTreeMap::new();
        }
        (0..g.players.len())
            .filter(|&rival| {
                let player = &g.players[rival];
                rival != pid
                    && player.alive
                    && !player.is_minor
                    && !player.is_barbarian
                    && g.has_met(pid, rival)
            })
            .filter_map(|rival| self.passage_hold(g, pid, rival).map(|hold| (rival, hold)))
            .collect()
    }

    /// A quote survives the holds unless it buys passage from a held rival.
    pub(super) fn passage_deal_allowed(
        deal: &QuickDeal,
        holds: &BTreeMap<usize, PassageHold>,
    ) -> bool {
        !(buys_passage(deal) && holds.contains_key(&deal.partner))
    }

    /// One journal line for each held rival that is quoting us passage on a
    /// trade turn: the purchase the slot would otherwise have weighed.
    pub(super) fn journal_withheld_passages(
        &self,
        g: &Game,
        pid: usize,
        excluded_partner: Option<usize>,
        holds: &BTreeMap<usize, PassageHold>,
    ) {
        if holds.is_empty() || !self.journal().wants(crate::reasoning::Level::Decision) {
            return;
        }
        let mut quoted: Vec<usize> = g
            .quick_deals(pid)
            .iter()
            .filter(|deal| {
                buys_passage(deal)
                    && Some(deal.partner) != excluded_partner
                    && holds.contains_key(&deal.partner)
            })
            .map(|deal| deal.partner)
            .collect();
        quoted.sort_unstable();
        quoted.dedup();
        for partner in quoted {
            self.journal_withheld_passage(g, partner, &holds[&partner], "trade slot");
        }
    }

    /// The live bridge's passage lane asks the same question per target;
    /// `journal` is set on the turns the lane would ask. `true` = withheld.
    pub fn passage_withheld_on_the_bridge(
        &self,
        g: &Game,
        pid: usize,
        partner: usize,
        journal: bool,
    ) -> bool {
        let Some(hold) = self.passage_hold(g, pid, partner) else {
            return false;
        };
        if journal {
            self.journal_withheld_passage(g, partner, &hold, "passage lane");
        }
        true
    }

    fn journal_withheld_passage(&self, g: &Game, partner: usize, hold: &PassageHold, lane: &str) {
        let civ = g
            .players
            .get(partner)
            .map(|player| player.civ.to_string())
            .unwrap_or_default();
        think!(self.journal(), Diplomacy, Decision,
               "Withholding passage from {}", civ;
               "passage-waits-past-the-opening withheld the {lane} purchase: {}",
               hold.reason(g.turn));
    }
}

#[cfg(test)]
mod tests;
