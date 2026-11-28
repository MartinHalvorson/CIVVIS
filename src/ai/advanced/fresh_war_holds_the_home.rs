//! `fresh-war-holds-the-home`: for about ten standard turns after a major
//! declares war on us, while its military is at least ours, the seat keeps
//! its Settlers off the ground where the declarer's soldiers were seen and
//! buys one ranged defender with the Gold it holds.
//!
//! **The finding (hotseat `hotseat-20261128T031456Z`, seat p1, Greece, the
//! religion seat, 2026-11-27).** The person (Sumeria) declared a surprise war
//! between p1's turns 11 and 12 and took its first Settler with a Scout. The
//! seat did not change posture: on turn 16, at war, at 47 power against the
//! person's 51 (`why.log` turn 16 "power 47 against the strongest rival's
//! 51"), it started another Settler at population 2 and sent it from Athens
//! toward (46, 26), the site the first one was walking to, a tile from where
//! the person's Scout and the captured Settler had stood on turn 13 (`why.log`
//! turn 16 "Settler begins an escorted expedition | the route from (48, 29)
//! to (46, 26)"; `events.jsonl` line 322). One Warrior stayed home. The
//! treasury held 53 to 85 Gold from turn 12 to 16 and a Slinger was for sale
//! at 65 from turn 14 (`purchasable` on lines 357-421). The grand strategy
//! reads Recovery only at `my_power * 1.25 < opponent` (`assessment.rs`), so
//! parity is peace as far as the empire's posture is concerned.
//!
//! **The gene.** Inert with the flag off. General, written for a person's
//! surprise war:
//! - **A war declared on us.** At the end of our turn the seat notes the
//!   majors it is at peace with, after its own declarations
//!   ([`AdvancedAi::note_fresh_war_peace`]). A war with one of them at the
//!   start of a later turn was declared on us in between; its first turn
//!   opens a window of [`FRESH_WAR_TURNS`] standard turns
//!   ([`AdvancedAi::observe_fresh_wars`]). A war we declared was already on
//!   at our turn's end, so it opens nothing; so does a war already on when
//!   the decider first saw the game.
//! - **While the declarer's military power is at least ours**
//!   ([`AdvancedAi::fresh_war_window`]), every tile within
//!   [`FRESH_WAR_GROUND_RADIUS`] of where we have seen its soldiers since the
//!   declaration is no site for a Settler (`settler_site_is_dead`, and a
//!   Settler holding such a target gives it up), and once per window the
//!   city nearest those sightings buys the strongest ranged land unit the
//!   treasury affords ([`AdvancedAi::fresh_war_defender_purchase`]), in the
//!   same lane as the surprise-war purchases, after the city-saving ones.
//!
//! It does not touch the grand strategy, the production queues or the army's
//! orders. Settlers already built go to other sites or wait.

use super::*;
use crate::shared_sort::SharedSort;

/// Standard-speed turns a declaration on us holds the home.
pub(super) const FRESH_WAR_TURNS: u32 = 10;

/// Tiles around a sighting of the declarer's soldiers that are no site for a
/// Settler while the window holds: the capture scars' own radius
/// (`SETTLER_CAPTURE_SCAR_RADIUS`), the ground a walking Warrior or Scout
/// covers in a turn and a half.
pub(super) const FRESH_WAR_GROUND_RADIUS: i32 = 3;

/// Gold kept back from the window's purchase. None: the war is the reason
/// the Gold was being held.
pub(super) const FRESH_WAR_PURCHASE_RESERVE: f64 = 0.0;

/// The living majors other than `pid` that `pid` has met.
fn met_majors(g: &Game, pid: usize) -> Vec<usize> {
    g.players
        .iter()
        .filter(|other| {
            other.id != pid
                && other.alive
                && !other.is_minor
                && !other.is_barbarian
                && g.players[pid].met.contains(&other.id)
        })
        .map(|other| other.id)
        .collect()
}

impl AdvancedAi {
    /// End of our turn: the met majors we are at peace with now, after our
    /// own declarations. Exact no-op with the gene off.
    pub(super) fn note_fresh_war_peace(&mut self, g: &Game, pid: usize) {
        if !self.fresh_war_holds_the_home {
            return;
        }
        self.fresh_war_peace_seen = met_majors(g, pid)
            .into_iter()
            .filter(|other| !g.is_at_war(pid, *other))
            .collect();
    }

    /// Start of our turn: open a window for each war declared on us since
    /// our last turn, close the windows that ended, note where we see each
    /// declarer's soldiers, and rebuild the ground a Settler keeps off.
    /// Exact no-op with the gene off.
    pub(super) fn observe_fresh_wars(&mut self, g: &Game, pid: usize) {
        if !self.fresh_war_holds_the_home {
            return;
        }
        let turn = g.turn;
        let duration = g.standard_duration(FRESH_WAR_TURNS).max(1);
        for other in met_majors(g, pid) {
            if g.is_at_war(pid, other)
                && self.fresh_war_peace_seen.contains(&other)
                && !self.fresh_war_onsets.contains_key(&other)
            {
                self.fresh_war_onsets.insert(other, turn);
                think!(self.journal(), Military, Decision,
                    "{} declared war on us", g.players[other].civ;
                    "fresh-war-holds-the-home: at peace at the end of our last turn; for {} \
                     standard turns, while their military is at least ours, Settlers keep off \
                     the ground their soldiers are seen on and one shooter is bought",
                    FRESH_WAR_TURNS);
            }
        }
        self.fresh_war_onsets.retain(|other, onset| {
            g.is_at_war(pid, *other) && *onset <= turn && turn < onset.saturating_add(duration)
        });
        let visible = self.battlefront_visibility(g, pid);
        let sightings: Vec<(usize, Pos)> = g
            .units
            .values()
            .filter(|unit| {
                let spec = &g.rules.units[unit.kind];
                self.fresh_war_onsets.contains_key(&unit.owner)
                    && spec.class == "military"
                    && spec.domain.as_deref() != Some("air")
                    && g.unit_visible_to(unit.id, pid)
                    && g.sees(&visible, unit.pos)
            })
            .map(|unit| (unit.owner, unit.pos))
            .collect();
        for sighting in sightings {
            self.fresh_war_seen.insert(sighting, turn);
        }
        let onsets = &self.fresh_war_onsets;
        self.fresh_war_seen
            .retain(|(owner, _), _| onsets.contains_key(owner));
        let holding: BTreeSet<usize> = self
            .fresh_war_onsets
            .keys()
            .copied()
            .filter(|other| self.fresh_war_holds_against(g, pid, *other))
            .collect();
        self.fresh_war_held_ground = self
            .fresh_war_seen
            .keys()
            .filter(|(owner, _)| holding.contains(owner))
            .flat_map(|(_, pos)| g.wdisk(*pos, FRESH_WAR_GROUND_RADIUS))
            .collect();
    }

    /// Whether `other`'s fresh war holds the home now: a window is open for
    /// it, we are still at war, and its military power is at least ours.
    fn fresh_war_holds_against(&self, g: &Game, pid: usize, other: usize) -> bool {
        let duration = g.standard_duration(FRESH_WAR_TURNS).max(1);
        self.fresh_war_onsets.get(&other).is_some_and(|onset| {
            g.is_at_war(pid, other)
                && *onset <= g.turn
                && g.turn < onset.saturating_add(duration)
                && g.military_power(other) >= g.military_power(pid)
        })
    }

    /// The newest declaration whose window holds the home this turn:
    /// `(declarer, first turn of the war)`. `None` with the gene off.
    pub(super) fn fresh_war_window(&self, g: &Game, pid: usize) -> Option<(usize, u32)> {
        if !self.fresh_war_holds_the_home {
            return None;
        }
        self.fresh_war_onsets
            .iter()
            .filter(|(other, _)| self.fresh_war_holds_against(g, pid, **other))
            .max_by_key(|(other, onset)| (**onset, std::cmp::Reverse(**other)))
            .map(|(other, onset)| (*other, *onset))
    }

    /// Whether a Settler keeps off `pos`: ground near the declarer's
    /// soldiers while a fresh war holds the home. False with the gene off.
    pub(super) fn fresh_war_holds_the_ground(&self, pos: Pos) -> bool {
        self.fresh_war_holds_the_home && self.fresh_war_held_ground.contains(&pos)
    }

    /// Once per window, buy the strongest ranged land unit the treasury
    /// affords in the city nearest the declarer's sightings (the cheapest of
    /// equals). Returns whether it bought. `false` with the gene off.
    pub(super) fn fresh_war_defender_purchase(&mut self, g: &mut Game, pid: usize) -> bool {
        let Some((declarer, onset)) = self.fresh_war_window(g, pid) else {
            return false;
        };
        if self.fresh_war_bought.contains(&(declarer, onset)) {
            return false;
        }
        let budget = g.players[pid].gold - FRESH_WAR_PURCHASE_RESERVE;
        if budget <= 0.0 {
            return false;
        }
        let seen: Vec<Pos> = self
            .fresh_war_seen
            .keys()
            .filter(|(owner, _)| *owner == declarer)
            .map(|(_, pos)| *pos)
            .collect();
        let mut cities = g.player_city_ids(pid);
        cities.sort_by_key_shared(|cid| {
            let at = g.cities[cid].pos;
            let near = seen
                .iter()
                .map(|pos| g.wdist(at, *pos))
                .min()
                .unwrap_or(i32::MAX);
            (near, *cid)
        });
        for city in cities {
            let mut best: Option<(f64, f64, Name)> = None;
            for (name, spec) in &g.rules.units {
                if spec.class != "military"
                    || !spec.has_ranged_attack()
                    || matches!(spec.domain.as_deref(), Some("sea" | "air"))
                {
                    continue;
                }
                let item = Item::Unit { unit: *name };
                let Some(price) = g.unit_purchase_cost(pid, city, name, "gold") else {
                    continue;
                };
                if price > budget + 1e-9
                    || !g.can_produce(pid, city, &item)
                    || g.purchase_is_blocked(city, &item)
                {
                    continue;
                }
                let power = spec.ranged_attack_strength();
                let better = best
                    .as_ref()
                    .is_none_or(|(best_power, best_price, best_name)| {
                        power > *best_power + 1e-9
                            || ((power - *best_power).abs() <= 1e-9
                                && (price < *best_price - 1e-9
                                    || ((price - *best_price).abs() <= 1e-9
                                        && name.as_str() < best_name.as_str())))
                    });
                if better {
                    best = Some((power, price, *name));
                }
            }
            let Some((_, price, unit)) = best else {
                continue;
            };
            let bought = g
                .apply(
                    pid,
                    &Action::Buy {
                        city,
                        unit,
                        formation: 0,
                        currency: "gold".to_string(),
                    },
                )
                .is_ok();
            if bought {
                self.fresh_war_bought.insert((declarer, onset));
                think!(self.journal(), Economy, Decision,
                    "{} buys a {} against the fresh war", g.cities[&city].name, plain(&unit);
                    "fresh-war-holds-the-home: {} declared on us on turn {} and its military is \
                     at least ours; one shooter for {:.0} Gold in the city nearest its soldiers",
                    g.players[declarer].civ, onset, price; g.cities[&city].pos);
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests;
