//! `captives-come-home`: buy our captured Spies back from their captors when
//! the network needs the slots.
//!
//! ## Why
//!
//! In Gathering Storm a captured Spy is held by its captor and keeps its place
//! in our Spy capacity until a deal returns it; the host refuses a new Spy in
//! every city while the network looks short. The unit export drops the agent,
//! and until `captives_of_ours` (#4537) nothing on the seat could see one, nor
//! ask for it: the live bridge skipped every deal holding a captive.
//!
//! Live Deity Korea science lane, 2026-11-05, 11 games past turn 100: Spy
//! capacity 31% filled; under capacity on 511 of 733 turns, and on 374 of
//! those the host refused `UNIT_SPY` everywhere. Net of Spies in transit,
//! captives held about 116 / 103 / 96 / 63 / 45 slot-turns in 092403Z /
//! 095229Z / 081633Z / 074830Z / 093803Z. 092403Z held no Spy from turn 138
//! to its end at capacity 2, so nothing could Disrupt the winners' rockets
//! or steal another Eureka. A Spy that comes home also keeps its experience
//! and promotions; a new one starts over at level 1 for 225-262 Production.
//!
//! ## What the gene does
//!
//! Inert with the flag off. On, when the network wants its slots back
//! ([`AdvancedAi::captives_come_home_wanted`]: below capacity, and either
//! under half of it held or a space racer on the denial board, where a Spy in
//! place for Disrupt Rocketry is worth most), the live bridge asks one captor
//! at peace with us for one of our Spies (`buy CAPTIVE`) on that captor's
//! `RANSOM_CADENCE` turn. The rival names its price (EQUALIZE) and the mod
//! closes only on exactly that captive against Gold within the ceiling: the
//! treasury over `RANSOM_GOLD_RESERVE`, capped at `RANSOM_CEILING_MAX`, and
//! no ask under `RANSOM_CEILING_MIN` (all in `civvis_orders`, beside
//! `append_captive_ransom_order`). `Kind::OptIn`.
//!
//! `held` is the bridge's count of Spies on the map, so a Spy in transit
//! (lifted off at `SPY_TRAVEL_NEW_CITY`, re-created on arrival) reads as an
//! empty slot and the half-full test fires a little early. The ask still
//! needs a rival holding one of ours, and that captive is a lost slot
//! however many travel, so the cost is at most an ask declined.
//!
//! Proven on captured live input (`docs/gene_screens/fires/
//! captives-come-home.action-pairs.json`): six boards of civvis-20261105T111305Z
//! where Scythia held two of our Spies; every off arm holds, five on arms ask
//! Scythia (`buy CAPTIVE`, ceilings 125-200), the sixth holds for another deal
//! to the same rival that turn.

use super::*;

impl AdvancedAi {
    /// Whether `captives-come-home` is on, for the live bridge.
    pub fn captives_come_home_enabled(&self) -> bool {
        self.captives_come_home
    }

    /// Whether the network wants its captured agents back this turn: the gene
    /// on, `held` below `capacity`, and either under half of the capacity
    /// held or a space racer for the denial network to stand in. Always false
    /// with the gene off.
    pub fn captives_come_home_wanted(
        &self,
        g: &Game,
        pid: usize,
        held: usize,
        capacity: usize,
    ) -> bool {
        self.captives_come_home
            && held < capacity
            && (held * 2 < capacity || !self.denial_spy_racers(g, pid).is_empty())
    }
}

#[cfg(test)]
mod tests;
