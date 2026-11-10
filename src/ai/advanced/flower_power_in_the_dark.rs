//! `flower-power-in-the-dark`: a Culture seat in a peaceful Dark Age slots
//! Flower Power once Rock Bands are on the menu, and lets it go when the
//! seat needs units.
//!
//! **Why (live Deity Online Kristina, 2026-11-10).** In 6 of 12 games the
//! seat sat in a Dark Age in the Atomic era or later with Flower Power in
//! `available_policies` (142605Z, 153619Z, 160429Z from t158, 161648Z-cont2,
//! 164254Z from t174, 171451Z) and never slotted it: no deck in this brain
//! named the card. Rock Band concerts bring about 70% of our foreign
//! visitors.
//!
//! **The card** (`Expansion2_Policies.xml`, `POLICY_FLOWER_POWER`, wildcard,
//! Atomic to Future era, Dark Age only): concerts +50%
//! (`ROCK_BAND_TOURISM_BOMB_VALUE_PEACE`, toward civilizations not at war
//! with us; the in-game text says +100%), -100% Production toward units, unit
//! purchases +100% (civilians included) and a 100% Rock Band purchase
//! discount. `data/policies.json` nets the last two to a free Rock Band; the
//! host may instead net them to the normal price, so the native model can
//! overstate the card.
//!
//! **The gene.** On a Culture target, with the card held or offered, Cold
//! War known (Rock Bands purchasable) and a Rock Band in the field or one
//! affordable within [`FLOWER_POWER_FAITH_STANDARD_TURNS`] standard turns of
//! Faith income, Flower Power goes to the front of the plan's portfolio,
//! ahead of the Great Person and adjacency cards that hold the wildcard
//! seats. It keeps that rank but takes its seat after the other cards are
//! seated, from the lowest-ranked card it outranks whose removal frees a
//! wildcard seat; never Limitanei, Praetorium, a maintenance emergency's
//! relief or the Liberalism of an Amenity repair. The cards
//! spliced in ahead of the portfolio (culture defence, Nobel Peace Favor,
//! bankruptcy gold, and the rest) still rank first. Being wanted keeps it
//! past the situational-card drop.
//!
//! It is refused, and a held card dropped, while the seat needs units: a
//! surprise war's mobilization window (`surprise_defense_window`), a
//! threatened home city (`home_city_threatened`: the plan names one, the
//! plan is Recovery, or a city was attacked within four turns), or a war
//! with a major one of whose military units stands within
//! [`FLOWER_POWER_FORCE_TILES`] of our cities. Like every card, it changes
//! only when the deck may change (a free host window live). With the gene
//! off nothing changes.

use super::*;

/// A war with a major is near once one of its military units stands this
/// close to one of our cities.
pub(crate) const FLOWER_POWER_FORCE_TILES: i32 = 8;

/// A Rock Band is affordable soon when this many standard turns of Faith
/// income cover its price.
pub(crate) const FLOWER_POWER_FAITH_STANDARD_TURNS: u32 = 10;

impl AdvancedAi {
    /// `flower-power-in-the-dark`: `None` with the gene off, on another
    /// target, or when the card is neither held nor offered. Otherwise
    /// `Ok(why)` when the seat wants Flower Power and `Err(why)` when it
    /// does not.
    pub(super) fn flower_power_verdict(
        &self,
        g: &Game,
        pid: usize,
    ) -> Option<Result<String, String>> {
        if !self.flower_power_in_the_dark
            || self.lane_lost
            || self.active_victory_target(g) != Some(VictoryTarget::Culture)
        {
            return None;
        }
        let card = crate::name!("flower_power");
        let spec = g.rules.policies.get(&card)?;
        let player = g.players.get(pid)?;
        let held = player.policies.contains(&card);
        let offered = spec.offered(&player.age, g.world_era)
            && (held || g.available_policies(pid).contains(&card));
        if !offered {
            return held.then(|| Err("the card is no longer on offer".to_string()));
        }
        if !player.civics.contains(&crate::name!("cold_war")) {
            return Some(Err("Rock Bands wait for Cold War".to_string()));
        }
        if let Some(why) = self.flower_power_needs_units(g, pid) {
            return Some(Err(format!(
                "{why}; the card stops unit production and doubles unit prices"
            )));
        }
        Some(
            Self::flower_power_band_in_reach(g, pid)
                .map(|band| format!("{band}; concerts +50% Tourism, no war near our cities"))
                .ok_or_else(|| {
                    "no Rock Band in the field or within reach of our Faith".to_string()
                }),
        )
    }

    /// One line when the gene slots the card. A card the turn began with is
    /// being kept (the base deck may have taken it out first), not slotted.
    pub(super) fn journal_flower_power_slotted(
        &self,
        verdict: Option<&Result<String, String>>,
        over: Option<&Name>,
    ) {
        let Some(Ok(why)) = verdict else {
            return;
        };
        if self
            .turn_start_policies
            .contains(&crate::name!("flower_power"))
        {
            return;
        }
        match over {
            Some(current) => {
                think!(self.journal(), Policies, Decision,
                       "Slotted flower power over {}", plain(current);
                       "flower-power-in-the-dark: {why}");
            }
            None => {
                think!(self.journal(), Policies, Decision, "Slotted flower power";
                       "flower-power-in-the-dark: {why}; into a free slot");
            }
        }
    }

    /// Why the seat needs to produce or buy units now, if it does.
    fn flower_power_needs_units(&self, g: &Game, pid: usize) -> Option<String> {
        if let Some(window) = self.surprise_defense_window(g, pid) {
            return Some(format!(
                "player {}'s surprise war wants defenders until turn {}",
                window.attacker, window.ends
            ));
        }
        let threatened = match self.plan.as_ref() {
            Some(plan) => Self::home_city_threatened(g, pid, plan),
            None => g.player_city_ids(pid).iter().any(|cid| {
                let city = &g.cities[cid];
                city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4
            }),
        };
        if threatened {
            return Some("a home city is threatened".to_string());
        }
        let ours: Vec<Pos> = g
            .player_city_ids(pid)
            .into_iter()
            .map(|cid| g.cities[&cid].pos)
            .collect();
        g.units
            .values()
            .filter(|unit| {
                unit.owner != pid
                    && g.players.get(unit.owner).is_some_and(|other| {
                        other.alive && !other.is_minor && !other.is_barbarian && !other.is_free_city
                    })
                    && g.is_at_war(pid, unit.owner)
                    && g.rules
                        .units
                        .get(&unit.kind)
                        .is_some_and(|spec| spec.class == "military")
            })
            .filter_map(|unit| {
                ours.iter()
                    .map(|home| g.wdist(*home, unit.pos))
                    .min()
                    .map(|tiles| (tiles, unit.owner))
            })
            .filter(|(tiles, _)| *tiles <= FLOWER_POWER_FORCE_TILES)
            .min()
            .map(|(tiles, owner)| {
                format!("at war with player {owner}, whose unit stands {tiles} tiles from our city")
            })
    }

    /// A Rock Band in the field, or one the Faith bank reaches within
    /// [`FLOWER_POWER_FAITH_STANDARD_TURNS`]: the cheapest quote of our
    /// cities (the host's live), else the list price.
    fn flower_power_band_in_reach(g: &Game, pid: usize) -> Option<String> {
        let bands = g
            .player_unit_ids(pid)
            .iter()
            .filter(|uid| g.units[uid].kind == "rock_band")
            .count();
        if bands > 0 {
            return Some(format!("{bands} Rock Band(s) in the field"));
        }
        let city_ids = g.player_city_ids(pid);
        let quote = city_ids
            .iter()
            .filter_map(|cid| g.unit_purchase_cost(pid, *cid, "rock_band", "faith"))
            .min_by(f64::total_cmp);
        let price = quote.unwrap_or_else(|| {
            let bought = g.players[pid]
                .counters
                .get("purchased:rock_band")
                .copied()
                .unwrap_or(0) as f64;
            g.game_speed.scale(
                g.rules
                    .units
                    .get("rock_band")
                    .map_or(f64::INFINITY, |spec| spec.cost)
                    + 100.0 * bought,
            )
        });
        let faith = g.players[pid].faith;
        let income: f64 = city_ids
            .iter()
            .map(|cid| g.city_yields(*cid).faith)
            .sum::<f64>()
            + g.player_yield_extras(pid).faith;
        let turns = f64::from(g.standard_duration(FLOWER_POWER_FAITH_STANDARD_TURNS));
        (faith + income.max(0.0) * turns >= price).then(|| {
            format!("a Rock Band costs about {price:.0} Faith, {faith:.0} banked at {income:+.0} a turn")
        })
    }
}

#[cfg(test)]
mod tests;
