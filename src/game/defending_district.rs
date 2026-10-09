use super::*;

/// A non-center district's independent combat state. Encampment-family saves
/// continue to use City's existing inline fields; the other districts use a
/// position-keyed vector so a city can hold an Encampment and an Oppidum.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DefendingDistrict {
    pub kind: Name,
    pub pos: Pos,
    pub hp: i32,
    pub wall_hp: i32,
    pub struck: bool,
    pub extra_strikes_used: i32,
    pub last_attacked: u32,
    pub pillaged: bool,
}

impl DefendingDistrict {
    pub(crate) fn healthy(kind: Name, pos: Pos, wall_hp: i32) -> Self {
        Self {
            kind,
            pos,
            hp: 100,
            wall_hp,
            struck: false,
            extra_strikes_used: 0,
            last_attacked: 0,
            pillaged: false,
        }
    }

    pub(crate) fn remembered(mut self) -> Self {
        self.struck = false;
        self.extra_strikes_used = 0;
        self.last_attacked = 0;
        self
    }
}

impl Game {
    pub(crate) fn district_has_defenses(&self, kind: Name) -> bool {
        kind != "city_center"
            && self
                .rules
                .districts
                .get(kind.as_str())
                .is_some_and(|spec| spec.defense > 0.0)
    }

    pub(crate) fn defending_districts<'a>(
        &'a self,
        city: &'a City,
    ) -> impl Iterator<Item = DefendingDistrict> + 'a {
        let encampment = city.districts.iter().find_map(|(kind, pos)| {
            self.district_is_family(kind, crate::name!("encampment"))
                .then_some(DefendingDistrict {
                    kind: *kind,
                    pos: *pos,
                    hp: city.encampment_hp,
                    wall_hp: city.encampment_wall_hp,
                    struck: city.encampment_struck,
                    extra_strikes_used: city.encampment_extra_strikes_used,
                    last_attacked: city.encampment_last_attacked,
                    pillaged: city.encampment_pillaged,
                })
        });
        encampment
            .into_iter()
            .chain(city.defending_districts.iter().copied().filter(|state| {
                self.district_has_defenses(state.kind)
                    && !self.district_is_family(state.kind, crate::name!("encampment"))
                    && city
                        .districts
                        .iter()
                        .any(|(kind, pos)| *kind == state.kind && *pos == state.pos)
            }))
    }

    pub(crate) fn defending_district_state(&self, cid: u32, pos: Pos) -> Option<DefendingDistrict> {
        let city = self.cities.get(&cid)?;
        if let Some(state) = self
            .defending_districts(city)
            .find(|state| state.pos == pos)
        {
            return Some(state);
        }
        // Public legacy city records do not hold a district roster. Their
        // observed tile and inline Encampment pools still identify that actor.
        let tile = self.map.get(pos)?;
        let kind = tile.district?;
        (tile.owner_city == Some(cid) && self.district_is_family(kind, crate::name!("encampment")))
            .then_some(DefendingDistrict {
                kind,
                pos,
                hp: city.encampment_hp,
                wall_hp: city.encampment_wall_hp,
                struck: city.encampment_struck,
                extra_strikes_used: city.encampment_extra_strikes_used,
                last_attacked: city.encampment_last_attacked,
                pillaged: city.encampment_pillaged,
            })
    }

    pub(crate) fn refresh_defending_district_memory(
        &self,
        memory: &mut RememberedCity,
        visible: &BTreeSet<Pos>,
    ) {
        let Some(city) = self
            .cities
            .get(&memory.id)
            .filter(|city| city.owner == memory.owner)
        else {
            return;
        };
        memory
            .defending_districts
            .retain(|state| !visible.contains(&state.pos));
        memory.defending_districts.extend(
            self.defending_districts(city)
                .filter(|state| {
                    visible.contains(&state.pos)
                        && !self.district_is_family(state.kind, crate::name!("encampment"))
                })
                .map(DefendingDistrict::remembered),
        );
        memory
            .defending_districts
            .sort_unstable_by_key(|state| state.pos);
    }

    pub(crate) fn set_defending_district_state(&mut self, cid: u32, state: DefendingDistrict) {
        let encampment = self.district_is_family(state.kind, crate::name!("encampment"));
        let city = self
            .cities
            .get_mut(&cid)
            .expect("defending district parent exists");
        if encampment {
            city.encampment_hp = state.hp;
            city.encampment_wall_hp = state.wall_hp;
            city.encampment_struck = state.struck;
            city.encampment_extra_strikes_used = state.extra_strikes_used;
            city.encampment_last_attacked = state.last_attacked;
            city.encampment_pillaged = state.pillaged;
        } else if let Some(previous) = city
            .defending_districts
            .iter_mut()
            .find(|d| d.pos == state.pos)
        {
            *previous = state;
        } else {
            city.defending_districts.push(state);
            city.defending_districts.sort_unstable_by_key(|d| d.pos);
        }
    }

    pub(crate) fn defending_district_can_strike(
        &self,
        city: &City,
        state: &DefendingDistrict,
    ) -> bool {
        if self.map.get(state.pos).is_some_and(|tile| tile.pillaged) {
            return false;
        }
        if self.district_is_family(state.kind, crate::name!("encampment")) {
            return self.encampment_can_strike(city);
        }
        state.hp > 0
            && state.wall_hp > 0
            && !state.pillaged
            && (!state.struck
                || state.extra_strikes_used
                    < self.governor_effect(city.owner, city.id, "city_extra_strike") as i32)
    }

    pub(crate) fn defending_district_strike_action(
        &self,
        cid: u32,
        state: &DefendingDistrict,
        target: Pos,
    ) -> Action {
        if self.district_is_family(state.kind, crate::name!("encampment")) {
            Action::EncampmentStrike { city: cid, target }
        } else {
            Action::DistrictStrike {
                city: cid,
                source: state.pos,
                target,
            }
        }
    }

    /// Zero HP remains a military target until melee enters and pillages it.
    pub(crate) fn defending_district_at(&self, pos: Pos) -> Option<u32> {
        let tile = self.map.get(pos)?;
        let kind = tile.district?;
        if !self.district_has_defenses(kind) || tile.pillaged {
            return None;
        }
        let cid = tile.owner_city?;
        let state = self.defending_district_state(cid, pos)?;
        (state.kind == kind && !state.pillaged).then_some(cid)
    }

    pub(crate) fn defending_district_strength(&self, cid: u32, pos: Pos) -> f64 {
        let state = self
            .defending_district_state(cid, pos)
            .expect("defending district exists");
        self.district_strength_from_health(cid, pos, state.hp, state.wall_hp)
    }

    pub(super) fn defending_district_take_damage(
        &mut self,
        cid: u32,
        pos: Pos,
        damage: i32,
        wall_mult: f64,
        bypass_walls: bool,
    ) {
        let mut state = self
            .defending_district_state(cid, pos)
            .expect("defending district exists");
        let max = self.city_max_wall_hp(&self.cities[&cid]);
        state.last_attacked = self.turn;
        if state.wall_hp > 0 && max > 0 {
            let fraction = state.wall_hp as f64 / max as f64;
            let through = if bypass_walls {
                damage
            } else if fraction >= 0.8 {
                1
            } else if fraction >= 0.2 {
                damage / 2
            } else {
                damage
            };
            state.wall_hp =
                (state.wall_hp - ((damage as f64 * wall_mult).round() as i32).max(1)).max(0);
            state.hp -= through.max(1);
        } else {
            state.hp -= damage;
        }
        self.set_defending_district_state(cid, state);
    }

    pub(super) fn set_defending_district_hp(&mut self, cid: u32, pos: Pos, hp: i32) {
        let mut state = self
            .defending_district_state(cid, pos)
            .expect("defending district exists");
        state.hp = hp;
        self.set_defending_district_state(cid, state);
    }

    /// Upgrade old saves with an unmodeled fort, preserving all existing
    /// district pools, including a measured zero or a spent attack.
    pub(crate) fn restore_other_defending_districts(&mut self, cid: u32) {
        let city = &self.cities[&cid];
        let max = self.city_max_wall_hp(city);
        let states: Vec<_> = city
            .districts
            .iter()
            .filter(|(kind, _)| {
                self.district_has_defenses(**kind)
                    && !self.district_is_family(*kind, crate::name!("encampment"))
            })
            .map(|(kind, pos)| {
                city.defending_districts
                    .iter()
                    .find(|d| d.kind == *kind && d.pos == *pos)
                    .copied()
                    .unwrap_or_else(|| DefendingDistrict::healthy(*kind, *pos, max))
            })
            .collect();
        self.cities.get_mut(&cid).unwrap().defending_districts = states;
    }
}
