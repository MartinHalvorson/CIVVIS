//! `founder-converts-its-own-cities`: a founder's spreaders convert our own
//! cities before any foreign one.
//!
//! A Religious Victory needs more than half of a civilization's cities in the
//! winner's faith, and our own count is the only one a founder fully controls.
//! The shipped spreader list (`advanced_missionary_step`) mixes our cities
//! (+170) with foreign ones (+90) and adds twelve a citizen, so a large foreign
//! city a few tiles off outranks a small town of ours; and once a rival faith
//! holds one of our cities, `counterweight-flips-the-small-towns` (written for
//! faithless seats) charges our own cities twenty-four a citizen while foreign
//! targets keep their twelve, so a founder's Missionaries walk abroad.
//!
//! Live Emperor civvis-20261011T005957Z (pin 29f529e4c) founded Buddhism at
//! t50 and lost on Religion at t118 with 14 cities. From t53 the Holy City
//! Popayán bought a Missionary every four or five turns; each spent one charge
//! in Popayán (`founder-holds-the-holy-city`) and walked north to England's
//! Plymouth (pop 11, eight tiles off) with the rest, while Bogotá, Cartagena
//! and Cali sat on no faith and Caracas and Santa Marta were Protestant. With
//! Protestantism holding two of ours, Plymouth scored 90 + 132 - 32 = 190
//! against Bogotá's 170 + 84 - 168 - 16 = 70 (a frame-0 replay of t54-70 sends
//! every spreader to (11,9)-(12,10)). Our cities were none 7 / Buddhist 1 /
//! Protestant 2 at t80 and Judaism 7 / Buddhist 1 at t116.
//!
//! Over the 95 live Emperor founder games of October 9-11, our faith held at
//! least half of our cities at t80 in 61 of the 85 that did not lose on
//! Religion against 4 of the 10 that did (median share 0.71 against 0.38).
//! Where a rival faith already held a city of ours at t60, the share at t80
//! was 0.43 (21 of 43 at half or more, 6 Religious defeats); where none did,
//! 0.83 (44 of 51, 4 defeats).
//!
//! Under the gene, while we founded the spreader's faith and at least
//! [`FOUNDER_HOME_MIN_UNCONVERTED`] of our cities are not on it, a target list
//! that holds any of our cities drops its foreign ones; the order among our
//! own cities (small towns, threat-held cities, recruitment sources first) is
//! the shipped one. Purchases are unchanged. Off: unchanged.

use super::*;

/// Our cities off our faith that keep a founder's spreaders at home. One is
/// the ordinary churn of a growing empire; two is a count a rival can use.
pub(crate) const FOUNDER_HOME_MIN_UNCONVERTED: usize = 2;

impl AdvancedAi {
    /// Under the gene, the number of our cities not on `religion` when we
    /// founded it and at least [`FOUNDER_HOME_MIN_UNCONVERTED`] are off it.
    /// `None` with the gene off, outside the Domination lane, with Religious
    /// Victory off, or for a faith we did not found.
    pub(super) fn founder_keeps_spreaders_home(
        &self,
        g: &Game,
        pid: usize,
        religion: &str,
    ) -> Option<usize> {
        if !self.founder_converts_its_own_cities
            || !g.victory_conditions.religious
            || self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || g.players[pid].religion.as_deref() != Some(religion)
        {
            return None;
        }
        let unconverted = g
            .player_city_ids(pid)
            .into_iter()
            .filter(|cid| g.city_religion(&g.cities[cid]) != Some(religion))
            .count();
        (unconverted >= FOUNDER_HOME_MIN_UNCONVERTED).then_some(unconverted)
    }

    /// Drop the foreign cities from a founder's sorted spreader targets while
    /// our own cities need converting and one of them is on the list. Logs
    /// when the shipped first choice was foreign.
    pub(super) fn founder_home_targets<T>(
        &self,
        g: &Game,
        pid: usize,
        religion: &str,
        targets: &mut Vec<T>,
        pos_of: impl Fn(&T) -> Pos,
    ) {
        let Some(unconverted) = self.founder_keeps_spreaders_home(g, pid, religion) else {
            return;
        };
        let ours = |pos: Pos| {
            g.city_at(pos)
                .is_some_and(|cid| g.cities[&cid].owner == pid)
        };
        if !targets.iter().any(|target| ours(pos_of(target))) {
            return;
        }
        let first_foreign = targets
            .first()
            .map(&pos_of)
            .filter(|pos| !ours(*pos))
            .and_then(|pos| g.city_at(pos))
            .map(|cid| g.cities[&cid].name.clone());
        targets.retain(|target| ours(pos_of(target)));
        if let (Some(abroad), Some(home)) = (
            first_foreign,
            targets
                .first()
                .and_then(|target| g.city_at(pos_of(target)))
                .map(|cid| g.cities[&cid].name.clone()),
        ) {
            think!(self.journal(), Faith, Detail,
                "{} spreader converts our {} before {}", religion, home, abroad;
                "founder-converts-its-own-cities: {} of our cities are not on our faith, and our own count is the one a Religious Victory needs", unconverted);
        }
    }
}

#[cfg(test)]
mod tests;
