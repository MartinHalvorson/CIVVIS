//! Eras: the era a player and a unit belong to, historic moments, era
//! score, dedications and golden ages.
//!
//! `player_era`, `era_from_progress`, `unit_era` and `wonder_era`;
//! historic moments (`historic_moment_spec`, `add_historic_moment`,
//! `first_historic_moment`, the district and wonder thresholds) and the
//! era score they award; dedications (`available_dedications`,
//! `projected_dedication_score`, `dedication_active`,
//! `do_choose_dedication`); and `golden_age_threshold`.
//!
//! The notes that award moments as things happen: a unit or formation
//! created (`note_unit_created_moments`, `note_formation_moment`), a
//! district, building or project completed, an underdog kill
//! (`note_underdog_kill`), and the world-first moments that repeat
//! (`repeatable_world_first_moment`). These moved verbatim out of the main
//! `impl Game` block in `game.rs` later the same day.
//!
//! Moved verbatim out of the main `impl Game` block in `game.rs`
//! (2026-11-05). Methods the rest of `game` calls are `pub(super)`; public
//! methods stay public; the code is otherwise unchanged.

use super::*;

impl Game {
    /// Civilization VI's Advanced Start: a game set to open past the first age
    /// begins with the earlier ages already behind it.
    ///
    /// This is where the start era stops being a label and becomes a rule.
    /// `new_with` has already clamped it to an era the rules have a tree for,
    /// so every value reaching here can actually be cut at.
    ///
    /// Every civilization on the board is handed it: majors and city-states
    /// alike get every technology and civic of an earlier era, and each of
    /// their starting units is walked as far up its own upgrade chain as that
    /// knowledge reaches. A world where the majors have rifles and the minors
    /// have slings is not the setting anybody asked for, and neither is a
    /// Modern-era start defended by Warriors. Barbarians are left alone
    /// deliberately: their camps spawn against the world era, which this
    /// moves, so they arrive era-appropriate without being handed a start.
    ///
    /// What this does not do is grant the extra Settlers, gold and finished
    /// districts a shipped Advanced Start also hands out. Those are balance
    /// dials; an era's worth of research is the part that decides what the
    /// game *is*.
    pub(super) fn open_in_start_era(&mut self) {
        if self.start_era == 0 {
            return;
        }
        let era = self.start_era;
        let techs: Vec<Name> = self
            .rules
            .techs
            .iter()
            .filter(|(_, spec)| spec.era < era)
            .map(|(name, _)| *name)
            .collect();
        let civics: Vec<Name> = self
            .rules
            .civics
            .iter()
            .filter(|(_, spec)| spec.era < era)
            .map(|(name, _)| *name)
            .collect();
        for pid in 0..self.players.len() {
            if self.players[pid].is_barbarian {
                continue;
            }
            self.players[pid].techs.extend(techs.iter().cloned());
            self.players[pid].civics.extend(civics.iter().cloned());
            // Nothing may be left researching something it already knows.
            if self.players[pid]
                .research
                .as_deref()
                .is_some_and(|tech| techs.iter().any(|known| known == tech))
            {
                self.players[pid].research = None;
                self.players[pid].research_progress = 0.0;
            }
            if self.players[pid]
                .civic
                .as_deref()
                .is_some_and(|civic| civics.iter().any(|known| known == civic))
            {
                self.players[pid].civic = None;
                self.players[pid].civic_progress = 0.0;
            }
        }
        self.world_era = era;

        // Re-spawn rather than rewrite: `spawn_unit` owns charges, religion
        // and the strongest-built counters, so a Musketman that arrives this
        // way is the same unit as one that was trained.
        let starting: Vec<(u32, usize, Pos, Name)> = self
            .units
            .values()
            .filter(|unit| !self.players[unit.owner].is_barbarian)
            .map(|unit| (unit.id, unit.owner, unit.pos, unit.kind))
            .collect();
        // A custom arena with an era pool is dealt *across* the pool rather
        // than walked to the top: each side's Nth unit is capped at the
        // pool's Nth era, cycling, so both sides carry the identical mix and
        // every pooled era puts something real on the field. The count is
        // kept per side because the two armies are dealt one after the other
        // and must come out the same. Everywhere else the pool is empty and
        // nothing is capped.
        let pool = if self.map_script.is_battlefield() && !self.map_script.is_scenario() {
            self.tactics.era.pool_eras()
        } else {
            Vec::new()
        };
        let mut dealt: BTreeMap<usize, usize> = BTreeMap::new();
        for (uid, owner, pos, kind) in starting {
            let cap = (!pool.is_empty()).then(|| {
                let index = dealt.entry(owner).or_insert(0);
                let era = pool[*index % pool.len()];
                *index += 1;
                era
            });
            let mut upgraded = kind;
            // The chain is acyclic and every rung is checked against what the
            // owner now knows, so this walks to the newest unit the start era
            // — or this unit's slice of the era pool — reaches and stops.
            while let Some(next) = self.unit_upgrade_target(owner, upgraded) {
                if cap.is_some_and(|era| !self.unit_within_era(next, era)) {
                    break;
                }
                upgraded = next;
            }
            if upgraded == kind {
                continue;
            }
            self.remove_unit(uid);
            self.spawn_unit(&upgraded, owner, pos);
        }
    }

    /// The era a unit belongs to, taken from the node that unlocks it. Unit
    /// Production policy cards are gated on it: Agoge boosts Ancient and
    /// Classical infantry, not every Infantry ever built.
    pub(crate) fn unit_era(&self, unit: &str) -> usize {
        let spec = &self.rules.units[unit];
        if let Some(tech) = spec.tech.as_deref() {
            return self.rules.techs.get(tech).map(|t| t.era).unwrap_or(0);
        }
        if let Some(civic) = spec.civic.as_deref() {
            return self.rules.civics.get(civic).map(|c| c.era).unwrap_or(0);
        }
        0
    }

    /// Pay Exodus of the Evangelists for every city that has just taken its
    /// founder's religion for the first time. The founder is credited, not the
    /// city's owner: converting somebody else's city is the historic moment.
    pub(super) fn award_conversion_era_score(&mut self, before: &[(u32, Option<String>)]) {
        for (cid, was) in before {
            let Some(now) = self
                .cities
                .get(cid)
                .and_then(|city| self.city_religion(city))
                .map(str::to_string)
            else {
                continue;
            };
            if was.as_deref() == Some(now.as_str()) {
                continue;
            }
            let Some(founder) = self.religion_founder(&now) else {
                continue;
            };
            if self.players[founder].converted_cities.insert(*cid) {
                self.dedication_trigger(founder, "city_converted", 1);
            }
            let owner = self.cities[cid].owner;
            if owner != founder && self.is_at_war(founder, owner) {
                self.add_historic_moment(
                    founder,
                    "MOMENT_CITY_CHANGED_RELIGION_ENEMY_CITY_DURING_WAR",
                );
            }
            if owner != founder && self.players[owner].holy_city == Some(*cid) {
                self.add_historic_moment(founder, "MOMENT_CITY_CHANGED_RELIGION_OTHER_HOLY_CITY");
            }
        }
    }

    #[cfg(test)]
    /// `MOMENT_BARBARIAN_CAMP_DESTROYED` is available from Ancient through
    /// Medieval. Keep this compatibility accessor for clients while deriving
    /// its value and window from the same catalogue as every other Moment.
    pub(crate) fn barbarian_camp_era_score(&self) -> i64 {
        self.historic_moment_spec("MOMENT_BARBARIAN_CAMP_DESTROYED")
            .map_or(0, |spec| spec.era_score)
    }

    fn historic_moment_spec(&self, moment: &str) -> Option<&crate::rules::HistoricMomentSpec> {
        let spec = self.rules.historic_moments.get(moment)?;
        (spec.era_score > 0
            && spec
                .minimum_game_era
                .is_none_or(|minimum| self.world_era >= minimum)
            && spec
                .maximum_game_era
                .is_none_or(|maximum| self.world_era <= maximum)
            && spec
                .obsolete_era
                .is_none_or(|obsolete| self.world_era < obsolete))
        .then_some(spec)
    }

    /// Award a named Historic Moment and record the concrete catalogue row.
    /// Taj Mahal adds one Era Score to Moments whose base value is at least
    /// two. No call site supplies a score or reimplements an era window.
    pub(super) fn add_historic_moment(&mut self, pid: usize, moment: &str) -> bool {
        if pid >= self.players.len()
            || self.players[pid].is_minor
            || self.players[pid].is_barbarian
            || self.players[pid].is_free_city
        {
            return false;
        }
        let Some(amount) = self.historic_moment_spec(moment).map(|spec| spec.era_score) else {
            return false;
        };
        let bonus = if amount >= 2 {
            self.empire_wonder_effect(pid, "historic_moment_bonus") as i64
        } else {
            0
        };
        self.players[pid].era_score += amount + bonus;
        *self.players[pid]
            .counters
            .entry(format!("historic_moment_awards:{moment}"))
            .or_insert(0) += 1;
        true
    }

    pub(super) fn first_historic_moment(
        &mut self,
        pid: usize,
        key: &str,
        ordinary_moment: Option<&str>,
        world_first_moment: Option<&str>,
    ) -> bool {
        if pid >= self.players.len()
            || self.players[pid].is_minor
            || self.players[pid].is_barbarian
            || self.players[pid].is_free_city
        {
            return false;
        }
        let counter = format!("historic_moment:{key}");
        if self.players[pid].counters.contains_key(&counter) {
            return false;
        }
        let first_in_world = !self.players.iter().any(|other| {
            other.id != pid
                && !other.is_minor
                && !other.is_barbarian
                && !other.is_free_city
                && other.counters.contains_key(&counter)
        });
        self.players[pid].counters.insert(counter, 1);
        let preferred = first_in_world
            .then_some(world_first_moment)
            .flatten()
            .filter(|moment| self.historic_moment_spec(moment).is_some());
        if let Some(moment) = preferred.or(ordinary_moment) {
            self.add_historic_moment(pid, moment);
        }
        first_in_world
    }

    pub(super) fn district_historic_moment_threshold(family: &str) -> Option<f64> {
        match family {
            "campus" | "holy_site" | "theater_square" => Some(3.0),
            "commercial_hub" | "harbor" | "industrial_zone" => Some(4.0),
            _ => None,
        }
    }

    pub(super) fn wonder_historic_moment(&self, wonder: &str) -> &'static str {
        if self.wonder_era(wonder) >= self.world_era {
            "MOMENT_BUILDING_CONSTRUCTED_GAME_ERA_WONDER"
        } else {
            "MOMENT_BUILDING_CONSTRUCTED_PAST_ERA_WONDER"
        }
    }

    /// World era from the most advanced researched technology or civic.
    ///
    /// Era metadata lives on the tree nodes, so late-game progress cannot be
    /// capped by a hard-coded count table when the ruleset grows.
    pub(super) fn player_era(&self, pid: usize) -> usize {
        self.players[pid]
            .techs
            .iter()
            .filter_map(|name| self.rules.techs.get_interned(*name).map(|spec| spec.era))
            .chain(
                self.players[pid]
                    .civics
                    .iter()
                    .filter_map(|name| self.rules.civics.get_interned(*name).map(|spec| spec.era)),
            )
            .max()
            .unwrap_or(0)
            // Nothing is ever before the era the world was set up to open in.
            // An Advanced Start hands out every tech of the eras *below* the
            // chosen one, so the most advanced thing known is one era short of
            // it until the first new tech lands; without this floor a Medieval
            // game would call itself Classical on its opening turn.
            .max(self.start_era)
    }

    pub(super) fn era_from_progress(&self) -> usize {
        let mut eras: Vec<usize> = self
            .players
            .iter()
            .filter(|player| {
                player.alive && !player.is_minor && !player.is_barbarian && !player.is_free_city
            })
            .map(|player| self.player_era(player.id))
            .collect();
        if eras.is_empty() {
            return self.start_era;
        }
        eras.sort_unstable();
        eras[eras.len() / 2]
    }

    pub fn available_dedications(&self, pid: usize) -> Vec<Name> {
        if pid >= self.players.len() || self.players[pid].dedication_choices == 0 {
            return Vec::new();
        }
        let era = self.world_era;
        self.rules
            .dedications
            .iter()
            .filter(|(name, spec)| {
                spec.available_in(era) && !self.players[pid].dedications.contains(name.as_str())
            })
            .map(|(name, _)| *name)
            .collect()
    }

    /// What a Dedication would have paid over the era that just ended, in Era
    /// Score, given what this civilization actually did in it.
    ///
    /// This is a *measured* projection, not a prior: the count of every trigger
    /// firing is kept whether or not the trigger was dedicated, so the answer
    /// to "which Dedication should I take" is the same shape as "which one
    /// would have paid me most last era". A civilization's behaviour is the
    /// most autocorrelated thing about it, which is why last era's tally beats
    /// any static ranking — a warmonger has been killing Corps, a builder has
    /// been laying Districts, and the tally already says which one you are.
    pub fn projected_dedication_score(&self, pid: usize, dedication: &str) -> i64 {
        let Some(spec) = self.rules.dedications.get(dedication) else {
            return 0;
        };
        let Some(player) = self.players.get(pid) else {
            return 0;
        };
        spec.triggers
            .iter()
            .map(|(trigger, amount)| {
                amount * player.last_era_triggers.get(trigger).copied().unwrap_or(0)
            })
            .sum()
    }

    /// A Dedication's Golden-Age half, which only a Golden or Heroic Age turns
    /// on.
    pub(super) fn dedication_active(&self, pid: usize, dedication: &str) -> bool {
        matches!(self.players[pid].age.as_str(), "golden" | "heroic")
            && self.players[pid].dedications.contains(dedication)
    }

    /// A Dedication's Normal-Age half: Era Score for the behaviour it names,
    /// which is the whole point of dedicating a Normal or Dark Age, since that
    /// score is what buys the next Golden one.
    ///
    /// **A Golden Age normally pays no Era Score.** Every quest modifier hangs off
    /// `PLAYER_ELIGIBLE_FOR_COMMEMORATION_QUEST`, a `TEST_ANY` set whose only
    /// two members are an inverted `REQUIREMENT_PLAYER_HAS_GOLDEN_AGE` and a
    /// `REQUIREMENT_PLAYER_ALWAYS_ALLOWED_COMMEMORATION_QUEST` that nothing in
    /// the shipped data grants. So the two halves are exclusive, and that is
    /// what normally stops a Golden Age from financing its own successor: it
    /// hands out its bonus and banks nothing, while a Dark or Normal Age banks
    /// the score that buys the next one. Georgia's Strength in Unity is the
    /// explicit exception in the civilization data for Golden and Heroic Ages.
    ///
    /// `count` is how many times the trigger just happened, so a kill that
    /// resolves several units at once pays for all of them.
    pub(super) fn dedication_trigger(&mut self, pid: usize, trigger: &str, count: i64) {
        if count <= 0 || pid >= self.players.len() {
            return;
        }
        // Counted before either gate, so the tally is the behaviour itself and
        // not the subset a past choice happened to be paid for.
        *self.players[pid]
            .era_triggers
            .entry(trigger.to_string())
            .or_insert(0) += count;
        if matches!(self.players[pid].age.as_str(), "golden" | "heroic")
            && self.civ_effect(pid, "golden_dedication_era_score") <= 0.0
        {
            return;
        }
        let earned: i64 = self.players[pid]
            .dedications
            .iter()
            .filter_map(|dedication| self.rules.dedications.get(dedication.as_str()))
            .filter_map(|spec| spec.triggers.get(trigger))
            .sum();
        if earned > 0 {
            // Dedications are not Historic Moments, so Taj Mahal does not
            // increase their score.
            self.players[pid].era_score += earned * count;
        }
    }

    pub(super) fn do_choose_dedication(
        &mut self,
        pid: usize,
        dedication: &str,
    ) -> Result<(), String> {
        if !self
            .available_dedications(pid)
            .iter()
            .any(|available| available == dedication)
        {
            return Err("dedication is unavailable".into());
        }
        self.players[pid].dedications.insert(dedication.to_string());
        self.players[pid].dedication_choices -= 1;
        Ok(())
    }

    pub(super) fn golden_age_threshold(
        era: usize,
        cities: i64,
        past_dark: i64,
        past_golden: i64,
    ) -> i64 {
        28 + if era == 0 { -3 } else { 0 } + cities - 5 * past_dark + 5 * past_golden
    }

    /// The era a wonder belongs to: the era of its unlocking node.
    pub(super) fn wonder_era(&self, wonder: &str) -> usize {
        let spec = &self.rules.wonders[wonder];
        spec.tech
            .as_deref()
            .and_then(|t| self.rules.techs.get(t).map(|node| node.era))
            .or_else(|| {
                spec.civic
                    .as_deref()
                    .and_then(|c| self.rules.civics.get(c).map(|node| node.era))
            })
            .unwrap_or(0)
    }

    pub(super) fn repeatable_world_first_moment(
        &mut self,
        pid: usize,
        key: &str,
        ordinary_moment: &str,
        world_first_moment: &str,
    ) -> bool {
        if pid >= self.players.len()
            || self.players[pid].is_minor
            || self.players[pid].is_barbarian
            || self.players[pid].is_free_city
        {
            return false;
        }
        let counter = format!("historic_moment:{key}");
        let first_in_world = !self.players.iter().any(|other| {
            !other.is_minor
                && !other.is_barbarian
                && !other.is_free_city
                && other.counters.contains_key(&counter)
        });
        *self.players[pid].counters.entry(counter).or_insert(0) += 1;
        let moment = if first_in_world {
            world_first_moment
        } else {
            ordinary_moment
        };
        self.add_historic_moment(pid, moment);
        first_in_world
    }

    pub(super) fn note_unit_created_moments(&mut self, pid: usize, unit: &str) {
        if pid >= self.players.len()
            || !self.cities.values().any(|city| city.owner == pid)
            || !self.rules.units.contains_key(unit)
        {
            return;
        }
        let spec = self.rules.units[unit].clone();
        if spec
            .unique_to
            .as_deref()
            .is_some_and(|civilization| self.owns_civ_unique(pid, civilization))
        {
            self.first_historic_moment(
                pid,
                &format!("unique_unit:{unit}"),
                Some("MOMENT_UNIT_CREATED_FIRST_UNIQUE"),
                None,
            );
        }
        match spec.domain.as_deref() {
            Some("air") => {
                self.first_historic_moment(
                    pid,
                    "first_air_unit",
                    Some("MOMENT_UNIT_CREATED_FIRST_DOMAIN_AIR"),
                    Some("MOMENT_UNIT_CREATED_FIRST_DOMAIN_AIR_IN_WORLD"),
                );
            }
            Some("sea") => {
                self.first_historic_moment(
                    pid,
                    "first_sea_unit",
                    Some("MOMENT_UNIT_CREATED_FIRST_DOMAIN_SEA"),
                    Some("MOMENT_UNIT_CREATED_FIRST_DOMAIN_SEA_IN_WORLD"),
                );
            }
            _ => {}
        }
        if let Some(resource) = spec.requires_resource {
            self.first_historic_moment(
                pid,
                &format!("first_unit_using_resource:{resource}"),
                Some("MOMENT_UNIT_CREATED_FIRST_REQUIRING_STRATEGIC"),
                Some("MOMENT_UNIT_CREATED_FIRST_REQUIRING_STRATEGIC_IN_WORLD"),
            );
        }
    }

    pub(super) fn note_formation_moment(&mut self, pid: usize, uid: u32) {
        let Some(unit) = self.units.get(&uid) else {
            return;
        };
        let formation = unit.formation;
        let naval = self.rules.units[unit.kind].domain.as_deref() == Some("sea");
        let (name, ordinary, world_first) = match (naval, formation) {
            (false, 1) => (
                "corps",
                "MOMENT_FORMATION_CORPS_FIRST",
                "MOMENT_FORMATION_CORPS_FIRST_IN_WORLD",
            ),
            (false, 2) => (
                "army",
                "MOMENT_FORMATION_ARMY_FIRST",
                "MOMENT_FORMATION_ARMY_FIRST_IN_WORLD",
            ),
            (true, 1) => (
                "fleet",
                "MOMENT_FORMATION_FLEET_FIRST",
                "MOMENT_FORMATION_FLEET_FIRST_IN_WORLD",
            ),
            (true, 2) => (
                "armada",
                "MOMENT_FORMATION_ARMADA_FIRST",
                "MOMENT_FORMATION_ARMADA_FIRST_IN_WORLD",
            ),
            _ => return,
        };
        self.first_historic_moment(
            pid,
            &format!("formation:{name}"),
            Some(ordinary),
            Some(world_first),
        );
    }

    pub(super) fn note_district_completed_moments(&mut self, pid: usize, district: &str, pos: Pos) {
        let spec = self.rules.districts[district].clone();
        if spec
            .unique_to
            .as_deref()
            .is_some_and(|civilization| self.owns_civ_unique(pid, civilization))
        {
            self.first_historic_moment(
                pid,
                &format!("unique_district:{district}"),
                Some("MOMENT_DISTRICT_CONSTRUCTED_FIRST_UNIQUE"),
                None,
            );
        }
        let family = self.district_family(Name::new(district));
        if family == "canal" {
            self.first_historic_moment(
                pid,
                "canal_district",
                Some("MOMENT_DISTRICT_CONSTRUCTED_CANAL"),
                None,
            );
        }
        // Mbanza is explicitly excluded from the Neighborhood Moment.
        if district == "neighborhood" {
            self.first_historic_moment(
                pid,
                "neighborhood_district",
                Some("MOMENT_DISTRICT_CONSTRUCTED_NEIGHBORHOOD_FIRST"),
                Some("MOMENT_DISTRICT_CONSTRUCTED_NEIGHBORHOOD_FIRST_IN_WORLD"),
            );
        }
        // Historic Moments use the district's *starting* adjacency. Percentage
        // modifiers from policy cards, governors, and alliances are therefore
        // deliberately excluded from this total.
        let mut adjacency = Yields::default();
        for source in self.district_adjacency_sources(Name::new(district), pos) {
            if source.percent == 0.0 {
                adjacency.add(source.yields);
            }
        }
        let value = match family.as_str() {
            "campus" => adjacency.science,
            "holy_site" => adjacency.faith,
            "commercial_hub" | "harbor" => adjacency.gold,
            "theater_square" => adjacency.culture,
            "industrial_zone" => adjacency.production,
            _ => 0.0,
        };
        // The six unique replacement districts are explicitly excluded from
        // their base district's high-adjacency Moment.
        if spec.unique_to.is_none()
            && Self::district_historic_moment_threshold(&family)
                .is_some_and(|threshold| value >= threshold)
        {
            let moment = match family.as_str() {
                "campus" => "MOMENT_DISTRICT_CONSTRUCTED_HIGH_ADJACENCY_CAMPUS",
                "holy_site" => "MOMENT_DISTRICT_CONSTRUCTED_HIGH_ADJACENCY_HOLY_SITE",
                "commercial_hub" => "MOMENT_DISTRICT_CONSTRUCTED_HIGH_ADJACENCY_COMMERCIAL_HUB",
                "harbor" => "MOMENT_DISTRICT_CONSTRUCTED_HIGH_ADJACENCY_HARBOR",
                "theater_square" => "MOMENT_DISTRICT_CONSTRUCTED_HIGH_ADJACENCY_THEATER_SQUARE",
                "industrial_zone" => "MOMENT_DISTRICT_CONSTRUCTED_HIGH_ADJACENCY_INDUSTRIAL_ZONE",
                _ => unreachable!(),
            };
            self.first_historic_moment(
                pid,
                &format!("high_adjacency:{family}"),
                Some(moment),
                None,
            );
        }
    }

    pub(super) fn note_building_completed_moments(&mut self, pid: usize, building: &str) {
        let spec = self.rules.buildings[building].clone();
        if self.grants_city_state_unique_bonus(pid, "Ayutthaya") && !spec.wonder {
            self.players[pid].civic_overflow += self.game_speed.scale(spec.cost * 0.10);
        }
        if spec
            .unique_to
            .as_deref()
            .is_some_and(|civilization| self.owns_civ_unique(pid, civilization))
        {
            self.first_historic_moment(
                pid,
                &format!("unique_building:{building}"),
                Some("MOMENT_BUILDING_CONSTRUCTED_FIRST_UNIQUE"),
                None,
            );
        }
        for (terminal, family) in [
            ("airport", "aerodrome"),
            ("military_academy", "encampment"),
            ("stadium", "entertainment_complex"),
            ("aquatics_center", "water_park"),
        ] {
            if self.building_is_family(Name::new(building), Name::new(terminal)) {
                let moment = match family {
                    "aerodrome" => "MOMENT_BUILDING_CONSTRUCTED_FULL_AERODROME_FIRST",
                    "encampment" => "MOMENT_BUILDING_CONSTRUCTED_FULL_ENCAMPMENT_FIRST",
                    "entertainment_complex" => {
                        "MOMENT_BUILDING_CONSTRUCTED_FULL_ENTERTAINMENT_COMPLEX_FIRST"
                    }
                    "water_park" => {
                        "MOMENT_BUILDING_CONSTRUCTED_FULL_WATER_ENTERTAINMENT_COMPLEX_FIRST"
                    }
                    _ => unreachable!(),
                };
                self.first_historic_moment(
                    pid,
                    &format!("full_district:{family}"),
                    Some(moment),
                    None,
                );
            }
        }
    }

    pub(super) fn note_project_founded_moment(&mut self, pid: usize, project: &str) {
        match project {
            "manhattan_project" => {
                self.add_historic_moment(pid, "MOMENT_PROJECT_FOUNDED_MANHATTEN");
            }
            "operation_ivy" => {
                self.add_historic_moment(pid, "MOMENT_PROJECT_FOUNDED_OPERATION_IVY");
            }
            "launch_earth_satellite"
            | "launch_moon_landing"
            | "launch_mars_colony"
            | "exoplanet_expedition" => {
                let first_in_world = !self
                    .players
                    .iter()
                    .any(|other| other.id != pid && other.science_projects.contains(project));
                let (ordinary, world_first) = match project {
                    "launch_earth_satellite" => (
                        "MOMENT_PROJECT_FOUNDED_SATELLITE_LAUNCH",
                        "MOMENT_PROJECT_FOUNDED_SATELLITE_LAUNCH_FIRST_IN_WORLD",
                    ),
                    "launch_moon_landing" => (
                        "MOMENT_PROJECT_FOUNDED_MOON_LANDING",
                        "MOMENT_PROJECT_FOUNDED_MOON_LANDING_FIRST_IN_WORLD",
                    ),
                    "launch_mars_colony" => (
                        "MOMENT_PROJECT_FOUNDED_MARS",
                        "MOMENT_PROJECT_FOUNDED_MARS_FIRST_IN_WORLD",
                    ),
                    "exoplanet_expedition" => (
                        "MOMENT_PROJECT_FOUNDED_EXOPLANET",
                        "MOMENT_PROJECT_FOUNDED_EXOPLANET_FIRST_IN_WORLD",
                    ),
                    _ => unreachable!(),
                };
                self.add_historic_moment(
                    pid,
                    if first_in_world {
                        world_first
                    } else {
                        ordinary
                    },
                );
            }
            _ => {}
        }
    }

    /// Record a combat kill with the detail the Eureka triggers ask about:
    /// what did the killing, what died, and whether it was a Barbarian.
    pub(super) fn note_underdog_kill(&mut self, pid: usize, attacker: &Unit, victim: &Unit) {
        if self.rules.units[attacker.kind].class != "military"
            || self.rules.units[victim.kind].class != "military"
        {
            return;
        }
        if attacker.formation < victim.formation {
            self.add_historic_moment(pid, "MOMENT_UNIT_KILLED_UNDERDOG_MILITARY_FORMATION");
        }
        if victim.promotions.len() >= attacker.promotions.len().saturating_add(2) {
            self.add_historic_moment(pid, "MOMENT_UNIT_KILLED_UNDERDOG_PROMOTIONS");
        }
    }
}
