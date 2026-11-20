//! `religion-seat-second-city-first`: a one-city Religion seat builds its
//! first Settler as soon as the capital can train one, ahead of the Holy Site
//! chain.
//!
//! **Why (live Lv 8 Deity, Online, 2026-11-18/20).** Of 26 Religion-lane
//! games, 10 founded their second city at t28-44 and 3 never did, against a
//! median t18 on Science and Culture seats. Cities following our religion at
//! t100 were about 13 when the second city came by t23 (11 games) and 6.7 when
//! it came at t28 or later (10). The founding turn was the same in both groups
//! (median t22). Read frame by frame, the capital's first Settler was pushed
//! off by the religion claims, never by a lost race:
//! - Russia `civvis-20261119T195219Z`: the Settler started t10 (11 of 40) and
//!   `religion-seat-founds-first`, which `religion-seat-races-the-prophet`
//!   lets take a busy queue, put the Lavra over it at t11; Prayers held the
//!   head t14-22 and `religion-seat-first-shrine` went ahead of the re-asked
//!   Settler at t23. Settler t28, second city t33.
//! - Arabia `civvis-20261120T003425Z`: the same Holy Site claim displaced the
//!   Settler (14 of 40) at t11; Prayers t17-21, then the Shrine ahead of the
//!   re-asked Settler at t25. Second city t35.
//! - Mali `civvis-20261120T005009Z`: the founds-first claim runs ahead of the
//!   opening book, so the idle queue at t11 went to the Holy Site before the
//!   book or `capital-settler-after-completion` could ask; Prayers t17-23, the
//!   Shrine ahead of the Settler at t24. First Settler t29; the capital fell at
//!   t38 with no second city.
//! - Russia `civvis-20261119T235505Z`: the Settler started t5 and finished at
//!   t10, the turn the Lavra came. Second city t13, religion t19.
//!
//! **What the gene does.** While the seat aims at Religion, holds one city and
//! has no Settler on the board, and the capital is at the host's Settler
//! floor, unthreatened and with a practical site to settle, the capital's
//! queue ends the production pass on a Settler. `religion-seat-founds-first`
//! does not place a Holy Site there meanwhile. The head keeps its place when
//! it is already a Settler, finishes this turn, is Walls, is a Holy Site with
//! production already paid into it (it finishes, then the Settler), or is a
//! soldier while hostile military is in reach or the barbarian alarm still
//! asks the capital for a defender (`barbarian_defense_item`, the answer the
//! opening's preempt plays). Everything else (a new Holy Site, Prayers, the Shrine, a Builder
//! or a routine unit) waits with its progress kept. The claim runs after
//! every ordinary writer and before the paused-district, Prayers-guard and
//! defence handoffs, which keep the last word.
//!
//! **Slot guard.** Before founding, with no Prophet in hand and at most one
//! religion slot left (`max_religions` less those founded or pending), a Holy
//! Site and Prayers go first: that race is the close one. Arabia never takes
//! the exception; Civ VI hands it the last Great Prophet.
//!
//! Off, for another target, or once a second city or a Settler exists,
//! nothing changes.

use super::*;

/// A capital attacked this recently keeps its queue.
const SECOND_CITY_ATTACK_MEMORY: u32 = 4;

impl AdvancedAi {
    /// The capital that should be training the seat's first Settler now, or
    /// `None` (gene off, another target, a second city or a Settler already,
    /// below the floor, threatened, or nowhere to settle).
    pub(super) fn second_city_settler_due(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> Option<u32> {
        if !self.religion_seat_second_city_first
            || self.base.minor
            || self.base.barb
            || self.active_victory_target(g) != Some(VictoryTarget::Religion)
        {
            return None;
        }
        let cities = g.player_city_ids(pid);
        let [capital] = cities.as_slice() else {
            return None;
        };
        let city = &g.cities[capital];
        if !city.is_capital
            || (city.pop as f64) < crate::ai::HOST_SETTLER_MIN_POP
            || plan.threatened_city == Some(*capital)
            || (city.last_attacked > 0
                && g.turn.saturating_sub(city.last_attacked) <= SECOND_CITY_ATTACK_MEMORY)
            || g.player_unit_ids(pid)
                .into_iter()
                .any(|uid| g.units[&uid].kind == "settler")
            || !g.can_produce(pid, *capital, &Self::second_city_settler())
            || !self.base.has_practical_settle_site(g, pid)
        {
            return None;
        }
        Some(*capital)
    }

    fn second_city_settler() -> Item {
        Item::Unit {
            unit: crate::name!("settler"),
        }
    }

    /// Whether the founding race is close enough for the Holy Site and its
    /// Prayers to go before the Settler: unfounded, no Prophet in hand, the
    /// race still open and at most one slot left. Never for Arabia.
    pub(super) fn second_city_slot_guard(&self, g: &Game, pid: usize) -> bool {
        let player = &g.players[pid];
        if player.religion.is_some()
            || player.prophet_pending
            || player.civ == "Arabia"
            || !self.prophet_race_open_for(g, pid)
        {
            return false;
        }
        let claimed = g.religions_founded()
            + g.players
                .iter()
                .filter(|candidate| candidate.prophet_pending)
                .count();
        g.max_religions().saturating_sub(claimed) <= 1
    }

    /// Whether `religion-seat-founds-first` leaves `cid` to its Settler.
    pub(super) fn second_city_holds_the_holy_site(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
    ) -> bool {
        self.second_city_settler_due(g, pid, plan) == Some(cid)
            && !self.second_city_slot_guard(g, pid)
    }

    /// Why the capital's current head keeps its place, or `None` when the
    /// Settler takes it.
    fn second_city_head_keeps(&self, g: &Game, pid: usize, cid: u32) -> Option<&'static str> {
        let head = g.cities[&cid].queue.first()?;
        let holy_site = matches!(head, Item::District { district, .. }
            if g.district_family(*district) == "holy_site");
        let prayers = matches!(head, Item::Project { project } if *project == "holy_site_prayers");
        match head {
            Item::Unit { unit } if *unit == "settler" => return Some("already a Settler"),
            Item::Unit { unit } | Item::Formation { unit, .. }
                if g.rules
                    .units
                    .get(unit)
                    .is_some_and(|spec| spec.class == "military")
                    && (self.hint_city_in_reach_of_war(g, pid, cid)
                        || self.base.barbarian_defense_item(g, pid, cid).is_some()) =>
            {
                return Some("a defender with hostiles in reach");
            }
            Item::Building { building }
                if g.rules.buildings.get(building).is_some_and(|spec| {
                    spec.outer_defense > 0 || spec.replaces.as_deref() == Some("walls")
                }) =>
            {
                return Some("Walls");
            }
            _ => {}
        }
        if (holy_site || prayers) && self.second_city_slot_guard(g, pid) {
            return Some("one religion slot left");
        }
        let invested = g.item_invested_production(cid, head);
        if holy_site && invested > 0.0 {
            return Some("a Holy Site already under way");
        }
        if g.item_cost_for_city(pid, cid, head) - invested <= g.city_yields(cid).production {
            return Some("finishes this turn");
        }
        None
    }

    /// See the module. Exact no-op with the gene off.
    pub(super) fn claim_religion_seat_second_city(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) {
        let Some(capital) = self.second_city_settler_due(g, pid, plan) else {
            return;
        };
        if self.second_city_head_keeps(g, pid, capital).is_some() {
            return;
        }
        let displaced = g.cities[&capital].queue.first().cloned();
        if g.apply(
            pid,
            &Action::Produce {
                city: capital,
                item: Self::second_city_settler(),
            },
        )
        .is_ok()
        {
            think!(self.journal(), Cities, Decision,
                "{} trains its first Settler", g.cities[&capital].name;
                "religion-seat-second-city-first: one city at population {}, {} of {} religions founded, ahead of {}",
                g.cities[&capital].pop, g.religions_founded(), g.max_religions(),
                displaced.map_or_else(|| "an idle queue".to_string(), |head| format!(
                    "{} (it keeps its progress)", Self::plain_item(&head))));
        }
    }
}

#[cfg(test)]
mod tests;
