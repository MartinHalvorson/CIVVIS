use super::*;

impl AdvancedAi {
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
        let total = |board: &Game| {
            let mut yields = board.player_yield_extras(pid);
            for city in &cities {
                yields.add(board.city_yields(*city));
            }
            yields
        };
        let mut value = |card: &str| {
            let name = Name::new(card);
            let held = probe.players[pid].policies.remove(&name);
            let without = total(&probe);
            probe.players[pid].policies.insert(name);
            let with = total(&probe);
            if !held {
                probe.players[pid].policies.remove(&name);
            }
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
