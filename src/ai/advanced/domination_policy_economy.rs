use super::*;

impl AdvancedAi {
    /// Keep the income card that is funding a Domination army through a cash
    /// crisis.  The live t208 policy request replaced Economic Union while
    /// the treasury held 112 Gold at +28/turn; the next board read -32/turn,
    /// and the ensuing bankruptcy cost soldiers faster than production replaced
    /// them.  Price the card on this board before protecting it: an empty
    /// Commercial Hub/Harbor network must not lock up an economic slot.
    pub(super) fn domination_wartime_gold_card_pays(
        &self,
        g: &Game,
        pid: usize,
        at_major_war: bool,
        staged_conquest: bool,
        reserve: f64,
    ) -> Option<f64> {
        let cities = g.player_city_ids(pid);
        let player = &g.players[pid];
        if self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || !(at_major_war || staged_conquest)
            || player.gold >= reserve
            || player.gold_per_turn >= 5.0 * cities.len().max(1) as f64
        {
            return None;
        }
        let card = Name::new("economic_union");
        let mut probe = g.clone();
        let held = probe.players[pid].policies.remove(&card);
        if !held && !g.available_policies(pid).contains(&card) {
            return None;
        }
        let total_gold = |board: &Game| {
            let _memo = board.query_memo();
            let mut gold = board.player_yield_extras(pid).gold;
            for city in &cities {
                gold += board.city_yields(*city).gold;
            }
            gold
        };
        let without = total_gold(&probe);
        probe.players[pid].policies.insert(card);
        let marginal = total_gold(&probe) - without;
        (marginal >= 5.0).then_some(marginal)
    }

    pub(super) fn domination_multiplier_reclaims_fallback(
        &self,
        g: &Game,
        card: &str,
        current: &str,
        unproductive: &BTreeSet<String>,
    ) -> bool {
        self.active_victory_target(g) == Some(VictoryTarget::Domination)
            && matches!(
                card,
                "rationalism"
                    | "grand_opera"
                    | "five_year_plan"
                    | "aesthetics"
                    | "natural_philosophy"
            )
            && !unproductive.contains(card)
            && matches!(
                current,
                "urban_planning"
                    | "caravansaries"
                    | "town_charters"
                    | "scripture"
                    | "economic_union"
            )
            && g.rules.policies[card].slot == g.rules.policies[current].slot
    }

    /// Preserve the strategic deck's useful multipliers. A multiplier with no
    /// current yield must not occupy the slot of an available productive card.
    pub(super) fn domination_productive_policy_fallbacks(
        &self,
        g: &Game,
        pid: usize,
        desired: &mut Vec<&'static str>,
    ) -> BTreeSet<String> {
        if self.active_victory_target(g) != Some(VictoryTarget::Domination) {
            return BTreeSet::new();
        }
        const MULTIPLIERS: [&str; 5] = [
            "rationalism",
            "grand_opera",
            "five_year_plan",
            "aesthetics",
            "natural_philosophy",
        ];
        let available: BTreeSet<Name> = g.available_policies(pid).into_iter().collect();
        let relevant: Vec<&str> = MULTIPLIERS
            .into_iter()
            .filter(|card| {
                desired.contains(card)
                    && (available.contains(&Name::new(card))
                        || g.players[pid].policies.contains(&Name::new(card)))
            })
            .collect();
        if relevant.is_empty() {
            return BTreeSet::new();
        }
        // Clone once. QueryCache::clone starts empty, so toggled policies cannot
        // inherit cached city yields. No action or policy change reaches g.
        let mut probe = g.clone();
        let cities = probe.player_city_ids(pid);
        // One read-only sweep of the empire. The memo shares each city's
        // luxury and ownership derivations across the sweep and is dropped
        // before the next card is toggled.
        let total = |board: &Game| {
            let _memo = board.query_memo();
            let mut yields = board.player_yield_extras(pid);
            for city in &cities {
                yields.add(board.city_yields(*city));
            }
            yields
        };
        // Every card is priced against the slate as it stands: a held card
        // against the slate without it, any other card against the slate
        // with it. That side is the same for every card, so it is read once
        // rather than swept again for each of up to ten cards.
        let current = total(&probe);
        let mut value = |card: &str| {
            let name = Name::new(card);
            let (without, with) = if probe.players[pid].policies.remove(&name) {
                let without = total(&probe);
                probe.players[pid].policies.insert(name);
                (without, current)
            } else {
                probe.players[pid].policies.insert(name);
                let with = total(&probe);
                probe.players[pid].policies.remove(&name);
                (current, with)
            };
            self.yield_value(
                Yields {
                    food: with.food - without.food,
                    production: with.production - without.production,
                    gold: with.gold - without.gold,
                    science: with.science - without.science,
                    culture: with.culture - without.culture,
                    faith: with.faith - without.faith,
                },
                GrandStrategy::Conquest,
            )
        };
        let unproductive: BTreeSet<String> = relevant
            .into_iter()
            .filter(|card| value(card) <= f64::EPSILON)
            .map(str::to_owned)
            .collect();
        if unproductive.is_empty() {
            return unproductive;
        }
        desired.retain(|card| !unproductive.contains(*card));
        // Only ordinary yield cards: no construction-window, military,
        // diplomatic, loyalty, or tourism-defense priority is replaced here.
        let mut fallbacks: Vec<(&'static str, f64)> = [
            "urban_planning",
            "caravansaries",
            "town_charters",
            "scripture",
            "economic_union",
        ]
        .into_iter()
        .filter(|card| {
            available.contains(&Name::new(card))
                || g.players[pid].policies.contains(&Name::new(card))
        })
        .map(|card| (card, value(card)))
        .filter(|(_, value)| *value > f64::EPSILON)
        .collect();
        fallbacks.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(b.0)));
        for (card, _) in fallbacks {
            if !desired.contains(&card) {
                desired.push(card);
            }
        }
        unproductive
    }
}

#[cfg(test)]
mod tests;
