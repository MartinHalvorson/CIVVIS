use super::*;

impl AdvancedAi {
    /// A non-founder must oppose the faith taking its empire, not whichever
    /// religion happens to occupy the first city in the map.
    pub(super) fn adopted_faith_threat(&self, g: &Game, pid: usize) -> Option<String> {
        if self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || !g.victory_conditions.religious
            || g.players[pid].religion.is_some()
        {
            return None;
        }
        let cities = g.player_city_ids(pid);
        if cities.is_empty() {
            return None;
        }
        let mut best: Option<(usize, usize, String)> = None;
        for founder in g
            .players
            .iter()
            .filter(|p| p.id != pid && p.alive && !p.is_minor && !p.is_barbarian)
        {
            let Some(faith) = founder.religion.as_deref() else {
                continue;
            };
            let converted = cities
                .iter()
                .filter(|cid| g.city_religion(&g.cities[cid]) == Some(faith))
                .count();
            if converted * 2 < cities.len() {
                continue;
            }
            let dominated = g
                .players
                .iter()
                .filter(|p| {
                    p.alive
                        && !p.is_minor
                        && !p.is_barbarian
                        && p.id != pid
                        && p.id != founder.id
                        && g.civ_follows_religion(p.id, faith)
                })
                .count();
            let better = best.as_ref().is_none_or(|(other, home, name)| {
                dominated > *other
                    || (dominated == *other
                        && (converted > *home || (converted == *home && faith < name.as_str())))
            });
            if better {
                best = Some((dominated, converted, faith.to_owned()));
            }
        }
        best.map(|(_, _, faith)| faith)
    }

    /// Rebuilding our religious veto must not complete another founder's win.
    pub(super) fn safe_adopted_counterfaith(g: &Game, pid: usize, faith: &str) -> bool {
        let Some(founder) = g.players.iter().find(|p| {
            p.alive && !p.is_minor && !p.is_barbarian && p.religion.as_deref() == Some(faith)
        }) else {
            return true;
        };
        g.players
            .iter()
            .filter(|p| {
                p.alive && !p.is_minor && !p.is_barbarian && p.id != pid && p.id != founder.id
            })
            .any(|p| !g.civ_follows_religion(p.id, faith))
    }

    /// A Holy Site and Shrine need time to finish before the last alternative
    /// faith disappears. Global conversion plus an arrival at home can warn
    /// construction sooner without changing when ordinary spreaders act.
    fn adopted_faith_construction_threat(&self, g: &Game, pid: usize) -> Option<String> {
        if let Some(threat) = self.adopted_faith_threat(g, pid) {
            return Some(threat);
        }
        if self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || !g.victory_conditions.religious
            || g.players[pid].religion.is_some()
        {
            return None;
        }
        let majors: Vec<_> = g
            .players
            .iter()
            .filter(|p| p.alive && !p.is_minor && !p.is_barbarian)
            .collect();
        let home = g.player_city_ids(pid);
        let mut best: Option<(usize, usize, String)> = None;
        for founder in majors.iter().filter(|p| p.id != pid) {
            let Some(faith) = founder.religion.as_deref() else {
                continue;
            };
            let arrived = home
                .iter()
                .filter(|cid| g.city_religion(&g.cities[cid]) == Some(faith))
                .count();
            if arrived == 0 {
                continue;
            }
            let converted = majors
                .iter()
                .filter(|p| g.civ_follows_religion(p.id, faith))
                .count();
            let foreign_conversion = majors
                .iter()
                .any(|p| p.id != pid && p.id != founder.id && g.civ_follows_religion(p.id, faith));
            if !foreign_conversion || converted * 2 < majors.len() {
                continue;
            }
            if best.as_ref().is_none_or(|(old, old_home, name)| {
                converted > *old
                    || (converted == *old
                        && (arrived > *old_home || (arrived == *old_home && faith < name.as_str())))
            }) {
                best = Some((converted, arrived, faith.to_owned()));
            }
        }
        best.map(|(_, _, faith)| faith)
    }

    /// A purchased defender keeps its faith when city majorities change.
    /// Reconsider that faith before every spread: a former counterweight can
    /// become the rival victory we now need to prevent.
    pub(super) fn adopted_faith_spread_allowed(&self, g: &Game, pid: usize, faith: &str) -> bool {
        if self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || !g.victory_conditions.religious
            || g.players[pid].religion.is_some()
        {
            return true;
        }
        Self::safe_adopted_counterfaith(g, pid, faith)
            && self.adopted_faith_threat(g, pid).as_deref() != Some(faith)
    }

    fn sanctuary_item(g: &Game, item: &Item) -> bool {
        match item {
            Item::District { district, .. } => g.district_family(*district).as_str() == "holy_site",
            Item::Building { building } => g.building_is_family(building, crate::name!("shrine")),
            _ => false,
        }
    }

    /// One legal source of defensive Missionaries, using a currently
    /// observed majority and an available district slot. A founder preserves
    /// its own faith; a non-founder uses a safe adopted counterfaith. Finish
    /// a reservation before selecting another city, preferring the shortest chain.
    pub(super) fn adopted_faith_sanctuary_choice(
        &self,
        g: &Game,
        pid: usize,
        threatened: Option<u32>,
    ) -> Option<(u32, Item)> {
        if self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || !g.victory_conditions.religious
        {
            return None;
        }
        let founded = g.players[pid].religion.as_deref();
        let threat = if founded.is_some() {
            // Founding alone does not preserve a religion: without a Shrine,
            // conversion can close the only source before any defender exists.
            self.home_conversion_threat(g, pid)?
        } else {
            self.adopted_faith_construction_threat(g, pid)?
        };
        let missionary = Item::Unit {
            unit: crate::name!("missionary"),
        };
        if g.players[pid].faith < 2.0 * g.item_cost_for(pid, &missionary) {
            return None;
        }
        let eligible = g
            .player_city_ids(pid)
            .into_iter()
            .filter(|cid| {
                Some(*cid) != threatened
                    && g.city_religion(&g.cities[cid]).is_some_and(|faith| {
                        if let Some(own) = founded {
                            faith == own
                        } else {
                            faith != threat && Self::safe_adopted_counterfaith(g, pid, faith)
                        }
                    })
            })
            .collect::<Vec<_>>();
        if eligible.iter().any(|cid| {
            g.cities[cid]
                .buildings
                .iter()
                .any(|b| g.building_is_family(b, crate::name!("shrine")))
        }) {
            return None;
        }
        for cid in &eligible {
            if let Some(item) = g.cities[cid].queue.first().filter(|item| {
                Self::sanctuary_item(g, item)
                    && Self::production_commitment_is_legal(g, pid, *cid, item)
            }) {
                return Some((*cid, item.clone()));
            }
        }
        let shrine = Item::Building {
            building: crate::name!("shrine"),
        };
        let mut best: Option<(f64, u32, Item)> = None;
        for cid in eligible {
            let production = g.city_yields(cid).production.max(1.0);
            for item in g
                .producible_items(pid, cid)
                .into_iter()
                .filter(|item| Self::sanctuary_item(g, item))
            {
                let remaining = g.item_remaining_cost_for_city(pid, cid, &item)
                    + if matches!(item, Item::District { .. }) {
                        g.item_remaining_cost_for_city(pid, cid, &shrine)
                    } else {
                        0.0
                    };
                let turns = remaining / production;
                if best.as_ref().is_none_or(|(old, old_city, _)| {
                    turns.total_cmp(old).then(cid.cmp(old_city)).is_lt()
                }) {
                    best = Some((turns, cid, item));
                }
            }
        }
        best.map(|(_, cid, item)| (cid, item))
    }

    pub(super) fn reserve_adopted_faith_sanctuary(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) {
        let Some((cid, item)) = self.adopted_faith_sanctuary_choice(g, pid, plan.threatened_city)
        else {
            return;
        };
        if g.cities[&cid].queue.first() == Some(&item) {
            return;
        }
        if g.apply(
            pid,
            &Action::Produce {
                city: cid,
                item: item.clone(),
            },
        )
        .is_ok()
        {
            think!(self.journal(), Economy, Decision,
                "{} starts {} for religious defense", g.cities[&cid].name, Self::plain_item(&item);
                "conversion threatens recruitment; preserve one source of counter-faith Missionaries while its faith survives");
        }
    }
}

#[cfg(test)]
mod tests;
