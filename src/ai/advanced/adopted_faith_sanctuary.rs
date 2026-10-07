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
            // `air-surge-2`, Domination lane: a faith that already holds every
            // other major needs only our empire, so its first convert here is
            // the alarm, not the half. Live King 20261001T022028Z: Catholicism
            // held Arabia (and Brazil founded it) at turn 120, took two of our
            // thirteen cities by 130 and seven by 140, and this alarm, waiting
            // for the half, started the counterfaith at 138; Brazil won at 147.
            let others = g
                .players
                .iter()
                .filter(|p| {
                    p.alive && !p.is_minor && !p.is_barbarian && p.id != pid && p.id != founder.id
                })
                .count();
            let match_point = self.air_surge_2 && others > 0 && dominated == others;
            if converted * 2 < cities.len() && !(match_point && converted > 0) {
                continue;
            }
            let better = best.as_ref().is_none_or(|(other, home, name)| {
                dominated > *other
                    || (dominated == *other
                        && (converted > *home || (converted == *home && faith < name.as_str())))
            });
            if better {
                best = Some((dominated, converted, faith.to_owned()));
            }
        }
        let threat = best.map(|(_, _, faith)| faith);
        // See `stronger_faith_than`.
        if self.counterweight_faith_is_no_threat {
            if let Some(stronger) = threat
                .as_deref()
                .and_then(|faith| Self::stronger_faith_than(g, pid, faith))
            {
                return Some(stronger);
            }
        }
        threat
    }

    /// `counterweight-faith-is-no-threat`: a living founder's faith other than
    /// `faith`, present in at least one of our cities, that holds more of the
    /// other majors (beyond its founder and us) than `faith` does -- the
    /// most such majors, then the most of our cities. A faith holding our
    /// majority is then our counterweight against it, not the threat. Live
    /// King civvis-20261005T110504Z (game 123): Catholicism, France's faith,
    /// held 6-7 of our 10 cities from turn 108, blocking the Netherlands'
    /// Protestantism (which held Vietnam); the threat then read Catholicism,
    /// so the Catholic spreaders held from turn 121 to 141, and Protestant
    /// Missionaries bought in Caracas at 124, 130 and 136 as the
    /// "counterfaith" spread Protestantism in our own cities. Our cities
    /// went 7 Catholic at 128 to 7 Protestant at 142 and 10 of 10 at 153; the
    /// Netherlands won on Religion at 166.
    pub(super) fn stronger_faith_than(g: &Game, pid: usize, faith: &str) -> Option<String> {
        let majors: Vec<(usize, Option<&str>)> = g
            .players
            .iter()
            .filter(|p| p.alive && !p.is_minor && !p.is_barbarian)
            .map(|p| (p.id, p.religion.as_deref()))
            .collect();
        let dominated = |faith: &str| -> usize {
            let founder = majors
                .iter()
                .find(|(_, founded)| *founded == Some(faith))
                .map(|(id, _)| *id);
            majors
                .iter()
                .filter(|(id, _)| {
                    *id != pid && Some(*id) != founder && g.civ_follows_religion(*id, faith)
                })
                .count()
        };
        let ours = dominated(faith);
        let home = g.player_city_ids(pid);
        majors
            .iter()
            .filter(|(id, _)| *id != pid)
            .filter_map(|(_, founded)| *founded)
            .filter(|other| *other != faith)
            .filter_map(|other| {
                let held = home
                    .iter()
                    .filter(|cid| g.city_religion(&g.cities[cid]) == Some(other))
                    .count();
                let reach = dominated(other);
                (held > 0 && reach > ours).then_some((reach, held, other))
            })
            .max_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)).then(b.2.cmp(a.2)))
            .map(|(_, _, other)| other.to_owned())
    }

    /// `safe_adopted_counterfaith`, and under `counterfaith-leaves-two-holdouts`
    /// also [`Self::counterfaith_leaves_two_holdouts_at`].
    pub(super) fn counterfaith_is_safe(&self, g: &Game, pid: usize, faith: &str) -> bool {
        Self::safe_adopted_counterfaith(g, pid, faith)
            && (!self.counterfaith_leaves_two_holdouts
                || Self::counterfaith_leaves_two_holdouts_at(g, pid, faith))
    }

    /// `counterfaith-leaves-two-holdouts`: a living founder's faith is a safe
    /// counterweight only while at least two living majors other than us and
    /// its founder do not follow it, so our own adoption still leaves it two
    /// civilizations short of a Religious Victory. A faith whose founder is
    /// gone can never win and is always safe. The shipped test
    /// (`safe_adopted_counterfaith`) asks for one such holdout, which with
    /// three living majors is the last one: live King
    /// civvis-20261005T191828Z (game 155) bought 13 Missionaries in Confucian
    /// cities from turn 106 while the Maya alone held out, took its own
    /// cities from 0 of 10 Confucian to 10 of 11 by turn 178 while a
    /// Sikh source (Vietnam's faith, founder eliminated at 64) bought 10,
    /// and Babylon won on Religion at 232.
    pub(super) fn counterfaith_leaves_two_holdouts_at(g: &Game, pid: usize, faith: &str) -> bool {
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
            .filter(|p| !g.civ_follows_religion(p.id, faith))
            .count()
            >= 2
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
        // `counterweight-from-the-first-convert`: the source starts at the
        // first city the threat takes.
        if let Some(threat) = self.first_convert_threat(g, pid) {
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
        self.counterfaith_is_safe(g, pid, faith)
            && self.adopted_faith_threat(g, pid).as_deref() != Some(faith)
    }

    /// A city that owns a Holy Site, queues one, or holds paid progress on one.
    fn sanctuary_district_investment(g: &Game, cid: u32) -> bool {
        let city = &g.cities[&cid];
        let holy_site = |district: &str| g.district_family(Name::new(district)).as_str() == "holy_site";
        g.city_has_district_family(city, crate::name!("holy_site"))
            || city.queue.iter().any(|item| {
                matches!(item, Item::District { district, .. } if holy_site(district.as_str()))
            })
            || city.production_progress.iter().any(|(key, paid)| {
                *paid > 0.0
                    && key
                        .strip_prefix("district:")
                        .and_then(|rest| rest.split(':').next())
                        .is_some_and(holy_site)
            })
    }

    /// `sanctuary-yields-a-held-queue`: a city whose queue another rule
    /// holds is not taken for the sanctuary. Walls or a defensive repair at
    /// the head, a land defender while the city shows siege evidence, or a
    /// district that already holds production. Live King
    /// 2026-10-04T205431Z: Bogotá swapped its Holy Site for a Great Person's
    /// Theater Square every frame from turn 251 to 254, and T212049Z:
    /// Guayaquil's Holy Site and its walls traded places five times from
    /// turn 148 to 152, so neither finished.
    pub(super) fn sanctuary_queue_held(&self, g: &Game, pid: usize, cid: u32) -> bool {
        let Some(head) = g.cities[&cid].queue.first() else {
            return false;
        };
        if Self::sanctuary_item(g, head) {
            return false;
        }
        match head {
            Item::Unit { .. } | Item::Formation { .. } => {
                Self::active_queue_answers_siege(g, head)
                    && self.base.besieged_city_item(g, pid, cid).is_some()
            }
            Item::District { .. } => g.item_invested_production(cid, head) > 0.0,
            _ => Self::active_queue_answers_siege(g, head),
        }
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
            match self.home_conversion_threat(g, pid) {
                Some(threat) => threat,
                // See `founder_wants_a_second_source` (`founder-keeps-two-sources`):
                // the second source starts at founding, threat or not.
                None if self.founder_wants_a_second_source(g, pid) => String::new(),
                None => return None,
            }
        } else {
            self.adopted_faith_construction_threat(g, pid)?
        };
        // Production establishes the source of both faith income and defenders.
        // Waiting for a Missionary's purchase budget can let conversion or
        // another specialty district close the last recruitment site first.
        // The eventual unit purchase still checks the actual faith balance.
        let free = |cid: &u32| {
            Some(*cid) != threatened
                && !(self.sanctuary_yields_a_held_queue && self.sanctuary_queue_held(g, pid, *cid))
        };
        let mut eligible = g
            .player_city_ids(pid)
            .into_iter()
            .filter(|cid| {
                free(cid)
                    && g.city_religion(&g.cities[cid]).is_some_and(|faith| {
                        if let Some(own) = founded {
                            faith == own
                        } else {
                            faith != threat && self.counterfaith_is_safe(g, pid, faith)
                        }
                    })
            })
            .collect::<Vec<_>>();
        // `counterweight-from-the-first-convert`: with no city on a safe
        // counterfaith, a city that follows no religion yet takes the
        // sanctuary, so a source stands when a safe faith arrives.
        if founded.is_none() && eligible.is_empty() && self.first_convert_sanctuary_fallback(g, pid)
        {
            eligible = g
                .player_city_ids(pid)
                .into_iter()
                .filter(|cid| free(cid) && g.city_religion(&g.cities[cid]).is_none())
                .collect();
        }
        // See `founder_wants_a_second_source`: under the gene a founder stops
        // at two sources, not at the first.
        let second_source = founded.is_some() && self.founder_wants_a_second_source(g, pid);
        if !second_source
            && eligible.iter().any(|cid| {
                g.cities[cid]
                    .buildings
                    .iter()
                    .any(|b| g.building_is_family(b, crate::name!("shrine")))
            })
        {
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
        // `one-sanctuary`: ★★ THE SANCTUARY CHASED EVERY CONVERSION. The
        // shrine check above only counts cities that still hold the
        // counterfaith, so each time the sanctuary's city converted, another
        // city started a Holy Site. Live King 20261004T094143Z: Maracaibo,
        // Guayaquil, Bogotá and Cuenca all started one by turn 98, and every
        // city followed the rival faith anyway; G49 made thirty starts across
        // six cities. One district is the empire's investment: only the city
        // that already owns, queues or has paid into a Holy Site may build it.
        let invested = |cid: u32| Self::sanctuary_district_investment(g, cid);
        let empire_invested =
            self.one_sanctuary && g.player_city_ids(pid).into_iter().any(invested);
        let mut best: Option<(f64, u32, Item)> = None;
        for cid in eligible {
            let production = g.city_yields(cid).production.max(1.0);
            for item in g
                .producible_items(pid, cid)
                .into_iter()
                .filter(|item| Self::sanctuary_item(g, item))
                .filter(|item| {
                    !(empire_invested && matches!(item, Item::District { .. }) && !invested(cid))
                })
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
        // See `founder_wants_a_second_source` (`founder-keeps-two-sources`).
        let second_source =
            self.founder_wants_a_second_source(g, pid) && Self::own_faith_sources(g, pid) == 1;
        if g.apply(
            pid,
            &Action::Produce {
                city: cid,
                item: item.clone(),
            },
        )
        .is_ok()
        {
            if second_source {
                think!(self.journal(), Economy, Decision,
                    "{} starts {} as a second faith source", g.cities[&cid].name, Self::plain_item(&item);
                    "founder-keeps-two-sources: our own Missionaries, Apostles and Inquisitors are bought only in a Shrine city that keeps our faith, and with one such city its conversion ends the defence");
            } else {
                think!(self.journal(), Economy, Decision,
                    "{} starts {} for religious defense", g.cities[&cid].name, Self::plain_item(&item);
                    "conversion threatens recruitment; preserve one source of counter-faith Missionaries while its faith survives");
            }
        }
    }
}

#[cfg(test)]
mod tests;
