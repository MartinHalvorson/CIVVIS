//! `denial-spies-skip-the-upkeep-wait`: a Spy the space denial still wants
//! against a racer past its Moon is not set aside by
//! `upkeep-waits-for-the-income` while the treasury carries its own upkeep.
//!
//! ## Why
//!
//! Live Deity Gran Colombia, Online, 4-player Tiny Pangaea, 2026-11-12: the
//! five games that passed turn 150 all lost to a rival Science Victory at
//! t183-210. Spies are the one denial instrument that reaches a racer we
//! cannot fight (our military stood at 0.1-0.8 times the winner's from its
//! Earth Satellite on in all five) or cannot walk to (081824Z: a city-state
//! shut the only land road to the Mapuche). `science-denial-trains-spies`
//! claimed their queues, and in four of the five games
//! `upkeep-waits-for-the-income` set the claimed Spy aside in the same frame
//! for the whole endgame (`UNIT_SPY` `Maintenance="4"`,
//! `Base/Assets/Gameplay/Data/Units.xml:769`), and the queue went to a
//! Machine Gun (a land defender on a Recovery plan, which the gate exempts)
//! or a Trader:
//!
//! | run | winner (Moon) | claims set aside | turns | Spy slots |
//! |---|---|---|---|---|
//! | 081824Z | Mapuche (t141) | 75 of 92 | t173-197 | 3-4, none held |
//! | 004621Z | Vietnam (t141) | 34 of 58 | t162-182 | 2-3 |
//! | 090241Z | Ottomans (t149) | 45 of 51 | t165-183 | 2 |
//! | 011556Z | France (t133) | 38 of 38 | t166-186 | 1; no Disrupt Rocketry all game |
//!
//! 081824Z t180: "Barinas trains a Spy against Mapuche's space race |
//! science-denial-trains-spies: 0 held and 0 queued against 3 needed", then
//! "Barinas sets aside spy until the income carries it |
//! upkeep-waits-for-the-income: 4 Gold a turn of upkeep on 20.8 income with
//! 74 more committed in other queues, and 149 Gold banked under 15 turns of
//! that deficit", then "Barinas switches to machine gun". The Mapuche landed
//! Mars at 180, launched at 186 and won at 197; no Spy of ours stood in a
//! Mapuche city after turn 147.
//!
//! The family already lets go of the treasury for this race:
//! `launcher-war-skips-the-reserve` waives the war's Gold reserve against a
//! racer past its Moon with a pad standing, because its launch ends the game
//! whatever our Gold.
//!
//! ## What the gene does
//!
//! `upkeep-waits-for-the-income` does not hold a Spy (its own test is
//! otherwise unchanged) while all of these stand:
//!
//! - `science-denial-trains-spies` is on and its losses pause
//!   (`denial-spies-count-their-losses`) is not;
//! - the denial still wants a Spy: the Spies we hold, and those at the head of
//!   our other cities' queues, are fewer than the claim's target
//!   (`denial_training_target`);
//! - a rival the claim trains against has landed the Moon
//!   ([`super::launcher_war::LAUNCHER_WAR_STAGES`] space projects) and not
//!   launched its Exoplanet Expedition (after the launch no Disrupt Rocketry
//!   stops it);
//! - the treasury holds this Spy's own upkeep for
//!   [`super::upkeep_waits_for_the_income::UPKEEP_RUNWAY_TURNS`] turns: the
//!   rest of the projected deficit is the other queues', which the gate keeps
//!   holding. 090241Z's 0-40 Gold would have kept its Spies waiting.
//!
//! The claim's journal names the racer when the waiver is what keeps its Spy.
//! Off, nothing changes.

use super::launcher_war::LAUNCHER_WAR_STAGES;
use super::upkeep_waits_for_the_income::UPKEEP_RUNWAY_TURNS;
use super::*;

impl AdvancedAi {
    /// Whether `item` is a Spy.
    fn denial_spy_item(item: &Item) -> bool {
        matches!(item, Item::Unit { unit } if unit == "spy")
    }

    /// `denial-spies-skip-the-upkeep-wait`: the racer past its Moon a Spy in
    /// `cid`'s queue answers, when the upkeep gate must let it through (see
    /// the module). `None` with the gene off, for anything but a Spy, or when
    /// any condition fails.
    pub(super) fn denial_spy_upkeep_racer(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        item: &Item,
    ) -> Option<usize> {
        if !self.denial_spies_skip_the_upkeep_wait
            || !self.science_denial_trains_spies
            || !Self::denial_spy_item(item)
            || self.spy_losses_pause_training(g)
        {
            return None;
        }
        let upkeep = Self::item_added_upkeep(g, item);
        if g.players[pid].gold < UPKEEP_RUNWAY_TURNS * upkeep {
            return None;
        }
        let racers = self.denial_training_racers(g, pid);
        if racers.is_empty() {
            return None;
        }
        let queued_elsewhere = g
            .player_city_ids(pid)
            .into_iter()
            .filter(|city| *city != cid)
            .filter(|city| {
                g.cities[city]
                    .queue
                    .first()
                    .is_some_and(Self::denial_spy_item)
            })
            .count();
        if g.spy_agents(pid) + queued_elsewhere >= self.denial_training_target(g, pid, &racers) {
            return None;
        }
        let launching: BTreeSet<usize> = racers
            .into_iter()
            .filter(|rival| {
                Self::science_denial_stages(g, *rival) >= LAUNCHER_WAR_STAGES
                    && !g.players[*rival]
                        .science_projects
                        .contains("exoplanet_expedition")
            })
            .collect();
        Self::racer_spy_leader(g, &launching)
    }

    /// `denial-spies-skip-the-upkeep-wait`: the claim's line when the waiver,
    /// not the income, is what keeps the Spy it just queued in `cid`.
    pub(super) fn journal_denial_spy_upkeep_waiver(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        item: &Item,
        plan: &StrategicPlan,
    ) {
        if !self.journal().wants(crate::reasoning::Level::Decision)
            || !self.upkeep_shortfall_holds(g, pid, cid, item, false, plan)
        {
            return;
        }
        let Some(racer) = self.denial_spy_upkeep_racer(g, pid, cid, item) else {
            return;
        };
        let player = &g.players[pid];
        let upkeep = Self::item_added_upkeep(g, item);
        let committed = Self::committed_queue_upkeep(g, pid, Some(cid));
        let name = g
            .cities
            .get(&cid)
            .map(|city| city.name.clone())
            .unwrap_or_default();
        let civ = g.players[racer].civ.to_string();
        think!(self.journal(), Economy, Decision,
            "{} keeps its Spy against {}'s launch", name, civ;
            "denial-spies-skip-the-upkeep-wait: {} has landed {} space projects and not \
             launched; {:.0} Gold a turn of upkeep on {:.1} income with {:.0} more committed \
             in other queues, and {:.0} Gold banked carries this Spy {:.0} turns",
            civ, Self::science_denial_stages(g, racer), upkeep, player.gold_per_turn,
            committed, player.gold, UPKEEP_RUNWAY_TURNS);
    }
}

#[cfg(test)]
mod tests;
