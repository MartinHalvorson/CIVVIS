//! Victory: which victories are open, how close each player is, and who
//! has won.
//!
//! `effective_victory_conditions`, `victory_eligible` and
//! `victory_lane_open` say which victories this game can be won by.
//! `score_parts`, the science-victory points, `culture_turns_to_victory`,
//! `victory_races`, `victory_threat` and `leading_victory_lanes` measure the
//! races. `victory_label` names them, and `set_winner` ends the game.
//!
//! How a game ends: `check_elimination` and `check_domination` after a
//! unit or city is lost (`on_unit_lost`), the turn limit
//! (`turn_limit`, `finish_at_turn_limit`, `reported_turn`), draws
//! (`is_draw`, `declare_draw`), playing on past a victory (`play_on`,
//! `play_on_blocks`, `played_on`), the victories a win needs
//! (`effective_required_victories`), and `crown`. These moved verbatim out
//! of the main `impl Game` block in `game.rs` later the same day.
//!
//! Moved verbatim out of the main `impl Game` block in `game.rs`
//! (2026-11-05). Methods the rest of `game` calls are `pub(super)`; public
//! methods stay public; the code is otherwise unchanged.

use super::*;

impl Game {
    /// Score by category, in Civ 6 tiebreaker order: civics, cities,
    /// districts (with buildings, which have no tiebreaker slot of their
    /// own), Population, Great People, religion, technologies, wonders, then
    /// Era Score (which the shipped tiebreaker list omits).
    pub fn score_parts(&self, pid: usize) -> [i64; 9] {
        if self.is_arena() {
            // On an arena a side's standing *is* its army. Keep a health-
            // weighted material score for observations, odds, and reports;
            // the battle deadline itself is a draw and never crowns this
            // material leader.
            let mut weight = 0.0;
            let mut standing = 0;
            for unit in self.units.values().filter(|unit| unit.owner == pid) {
                weight += self.rules.units[unit.kind].strength * f64::from(unit.hp) / 100.0;
                standing += 1;
            }
            return [weight.round() as i64, standing, 0, 0, 0, 0, 0, 0, 0];
        }
        let p = &self.players[pid];
        let cities: Vec<&City> = self.cities.values().filter(|c| c.owner == pid).collect();
        let districts = cities
            .iter()
            .flat_map(|c| c.districts.keys())
            .map(|d| {
                if self.rules.districts[d].unique_to.is_some() {
                    4
                } else {
                    2
                }
            })
            .sum::<i64>();
        let buildings = cities.iter().map(|c| c.buildings.len() as i64).sum::<i64>();
        let religion = match p.religion.as_deref() {
            Some(faith) => {
                let followers = self
                    .cities
                    .values()
                    .filter(|c| c.owner != pid && self.city_religion(c) == Some(faith))
                    .count() as i64;
                10 + 2 * followers
            }
            None => 0,
        };
        let wonders = cities.iter().map(|c| c.wonders.len() as i64).sum::<i64>();
        [
            3 * p.civics.len() as i64,
            5 * cities.len() as i64,
            districts + buildings,
            cities.iter().map(|c| c.pop as i64).sum::<i64>(),
            5 * p.great_people.len() as i64,
            religion,
            2 * p.techs.len() as i64,
            15 * wonders,
            p.era_score,
        ]
    }

    /// The exact Science Victory Points the host reports for `pid`, or the
    /// native game's simulated expedition distance when this is not a live
    /// mirror (or an older control mod did not carry the reading yet).
    pub fn science_victory_points(&self, pid: usize) -> f64 {
        self.observed_public_empire_stats
            .get(&pid)
            .and_then(|stats| stats.science_victory_points)
            .filter(|points| points.is_finite() && *points >= 0.0)
            .unwrap_or(self.players[pid].exoplanet_distance)
    }

    /// The Science Victory Points required in this game. Civilization VI's
    /// game-speed setting owns this number on a mirrored board, while the
    /// native simulator retains its fixed destination.
    pub fn science_victory_points_needed(&self, pid: usize) -> f64 {
        self.observed_public_empire_stats
            .get(&pid)
            .and_then(|stats| stats.science_victory_points_needed)
            .filter(|points| points.is_finite() && *points > 0.0)
            .unwrap_or(EXOPLANET_DESTINATION)
    }

    /// Science-victory movement per turn. A live host's World Rankings value
    /// wins here so Lagrange and Terrestrial Laser Stations reflect their
    /// actual current effect; native and older-mod boards use the model.
    pub fn exoplanet_speed(&self, pid: usize) -> f64 {
        if !self.players[pid]
            .science_projects
            .contains("exoplanet_expedition")
        {
            return 0.0;
        }
        self.observed_public_empire_stats
            .get(&pid)
            .and_then(|stats| stats.science_victory_points_per_turn)
            .filter(|speed| speed.is_finite() && *speed >= 0.0)
            .unwrap_or_else(|| self.modeled_exoplanet_speed(pid))
    }

    /// The religion a majority of `pid`'s cities follow — the shipped test for
    /// a civilization being converted (`GetReligionInMajorityOfCities`,
    /// WorldRankings.lua:2049): the host's answer where the mirror recorded
    /// one. A native empire with no observed verdict stays unknown; complete
    /// simulator boards use `following * 2 > cities`. `None` when no majority
    /// can be established.
    pub fn majority_religion_of(&self, pid: usize) -> Option<&str> {
        if let Some(observed) = self.observed_majority_religion.get(&pid) {
            return Some(observed.as_str());
        }
        let cities = self.player_city_ids(pid);
        if cities.is_empty() {
            return None;
        }
        // A native city's last observed religion may be stale, and its city
        // list may cover only part of the empire. Missing host majority data
        // cannot establish conversion from that subset. The mirror supplies
        // our own majority from its complete current owned-city export.
        if self.observed_public_empire_stats.contains_key(&pid) {
            return None;
        }
        let mut tally: BTreeMap<&str, usize> = BTreeMap::new();
        for cid in &cities {
            if let Some(religion) = self.city_religion(&self.cities[cid]) {
                *tally.entry(religion).or_insert(0) += 1;
            }
        }
        tally
            .into_iter()
            .find(|(_, following)| following * 2 > cities.len())
            .map(|(religion, _)| religion)
    }

    /// Whether `pid` counts as converted to `religion` for the Religious
    /// Victory: a majority of its cities follow it. See
    /// [`Self::majority_religion_of`] for where the answer comes from.
    pub fn civ_follows_religion(&self, pid: usize, religion: &str) -> bool {
        self.majority_religion_of(pid) == Some(religion)
    }

    /// Cities anywhere following `pid`'s religion — the religion lane's own
    /// number on the shipped World Rankings overview
    /// (`GetNumCitiesFollowingReligion`, WorldRankings.lua:44): the host's
    /// count where the mirror recorded one, since it covers cities the seat
    /// has never seen, else the board's.
    pub fn cities_following_religion(&self, pid: usize) -> usize {
        if let Some(observed) = self
            .observed_public_empire_stats
            .get(&pid)
            .and_then(|stats| stats.cities_following_religion)
        {
            return observed;
        }
        let Some(religion) = self
            .players
            .get(pid)
            .and_then(|player| player.religion.as_deref())
        else {
            return 0;
        };
        self.cities
            .values()
            .filter(|city| self.city_religion(city) == Some(religion))
            .count()
    }

    /// The host's own turns until `pid` wins on Culture, when observed: see
    /// `ObservedPublicEmpireStats::culture_turns_to_victory`. A native game
    /// has none.
    pub fn culture_turns_to_victory(&self, pid: usize) -> Option<f64> {
        self.observed_public_empire_stats
            .get(&pid)
            .and_then(|stats| stats.culture_turns_to_victory)
    }

    /// How far each civilization has come along every victory race, as a
    /// percentage of what that race requires.
    ///
    /// This is the one implementation: `obs.rs` formats it for the victory
    /// tracker and the AI reads it to decide who is worth fighting. Keeping two
    /// copies of "how close is this player to winning" is how a HUD and an AI
    /// end up disagreeing about who is about to win the game.
    pub fn victory_races(&self, pid: usize, leading_score: i64) -> VictoryRaces {
        let player = &self.players[pid];
        let all_majors: Vec<usize> = self
            .players
            .iter()
            .filter(|candidate| !candidate.is_minor && !candidate.is_barbarian)
            .map(|candidate| candidate.id)
            .collect();
        let living_majors: Vec<usize> = all_majors
            .iter()
            .copied()
            .filter(|candidate| self.players[*candidate].alive)
            .collect();

        let science_projects = [
            "launch_earth_satellite",
            "launch_moon_landing",
            "launch_mars_colony",
            "exoplanet_expedition",
        ];
        let completed_projects = science_projects
            .iter()
            .filter(|project| player.science_projects.contains(**project))
            .count();
        let exoplanet_distance = self.science_victory_points(pid);
        let exoplanet_distance_target = self.science_victory_points_needed(pid);
        let exoplanet_speed = self.exoplanet_speed(pid);
        let science = if player.science_projects.contains("exoplanet_expedition") {
            75.0 + 25.0 * exoplanet_distance / exoplanet_distance_target
        } else {
            match completed_projects {
                0 => 0.0,
                1 => 25.0,
                2 => 45.0,
                _ => 65.0,
            }
        }
        .clamp(0.0, 100.0);

        let rival_domestic = living_majors
            .iter()
            .filter(|candidate| **candidate != pid && !self.same_team(pid, **candidate))
            .map(|candidate| self.domestic_tourists(*candidate))
            .max()
            .unwrap_or(0);
        let culture_target = rival_domestic + 1;
        let leading_domestic = all_majors
            .iter()
            .map(|candidate| self.domestic_tourists(*candidate))
            .max()
            .unwrap_or(0);
        let foreign_tourists = self.foreign_tourists(pid);
        let culture = if culture_target > 0 {
            100.0 * foreign_tourists as f64 / culture_target as f64
        } else {
            0.0
        }
        .clamp(0.0, 100.0);

        let team_religions = self
            .team_members(pid)
            .into_iter()
            .filter_map(|member| self.players[member].religion.as_deref())
            .collect::<Vec<_>>();
        let religious_rivals = living_majors
            .iter()
            .copied()
            .filter(|candidate| player.team.is_none() || !self.same_team(pid, *candidate))
            .collect::<Vec<_>>();
        // A civilization is converted when a majority of its cities follow
        // the religion. `civ_follows_religion` asks the host's own majority
        // answer where the mirror recorded one — a rival's unseen cities
        // count there and cannot on the board — and the board's cities
        // otherwise, by the same `following * 2 > cities` rule as before.
        let converted_civs = religious_rivals
            .iter()
            .filter(|candidate| {
                team_religions
                    .iter()
                    .any(|religion| self.civ_follows_religion(**candidate, religion))
            })
            .count();
        let religious_target = religious_rivals.len();
        let religious = if religious_target > 0 {
            100.0 * converted_civs as f64 / religious_target as f64
        } else {
            0.0
        };

        let capital_target = all_majors.len();
        let controlled_capitals = if player.team.is_some() {
            all_majors
                .iter()
                .filter(|original_owner| {
                    let capital = self
                        .cities
                        .values()
                        .find(|city| city.is_capital && city.original_owner == **original_owner);
                    if **original_owner == pid || self.same_team(pid, **original_owner) {
                        capital.is_some_and(|capital| capital.owner == **original_owner)
                    } else {
                        capital.is_none_or(|capital| capital.owner != **original_owner)
                    }
                })
                .count()
        } else {
            all_majors
                .iter()
                .filter(|original_owner| {
                    self.cities
                        .values()
                        .find(|city| city.is_capital && city.original_owner == **original_owner)
                        .map_or(
                            **original_owner == pid || !self.players[**original_owner].alive,
                            |capital| capital.owner == pid,
                        )
                })
                .count()
        };
        let domination = if capital_target > 0 {
            100.0 * controlled_capitals as f64 / capital_target as f64
        } else {
            0.0
        };

        let diplomatic_points = player.dvp.max(0);
        let diplomatic = (100.0 * diplomatic_points as f64
            / DIPLOMATIC_VICTORY_POINTS.max(1) as f64)
            .clamp(0.0, 100.0);

        let score_points = self.team_score_rank_key(pid).0;
        let clock = self
            .turn_limit()
            .filter(|limit| *limit > 0 && *limit < 100_000)
            .map_or(1.0, |limit| {
                (self.turn as f64 / limit as f64).clamp(0.0, 1.0)
            });
        let score = if leading_score > 0 {
            (100.0 * clock * score_points.max(0) as f64 / leading_score as f64).clamp(0.0, 100.0)
        } else {
            0.0
        };
        let observed = self.observed_public_empire_stats.get(&pid);

        VictoryRaces {
            science,
            science_projects: completed_projects,
            science_project_target: science_projects.len(),
            exoplanet_distance,
            exoplanet_distance_target,
            exoplanet_speed,
            techs: observed
                .and_then(|stats| stats.techs)
                .unwrap_or(player.techs.len()),
            tech_total: self.rules.techs.len(),
            culture,
            foreign_tourists,
            culture_target,
            civics: observed
                .and_then(|stats| stats.civics)
                .unwrap_or(player.civics.len()),
            civic_total: self.rules.civics.len(),
            domestic_tourists: self.domestic_tourists(pid),
            rival_domestic,
            leading_domestic,
            religious,
            converted_civs,
            religious_target,
            cities_following_religion: self.cities_following_religion(pid),
            diplomatic,
            diplomatic_points,
            domination,
            controlled_capitals,
            capital_target,
            score,
            score_points,
        }
    }

    /// How close `pid` is to winning by any enabled race another empire could
    /// actually interfere with, as a percentage.
    ///
    /// Score is deliberately excluded. It is not a race to a threshold but a
    /// standing measured when the clock runs out, so its meter fills for
    /// everybody as the game ages and folding it in would turn this back into
    /// "who has the biggest empire" — the signal the AI already had.
    pub fn victory_threat(&self, pid: usize) -> f64 {
        if !self.victory_eligible(pid) {
            return 0.0;
        }
        let leading_score = self
            .players
            .iter()
            .filter(|p| !p.is_minor && !p.is_barbarian)
            .map(|p| self.team_score_rank_key(p.id).0)
            .max()
            .unwrap_or(0);
        let races = self.victory_races(pid, leading_score);
        let enabled = &self.victory_conditions;
        [
            (enabled.science, races.science),
            (enabled.culture, races.culture),
            (enabled.religious, races.religious),
            (enabled.diplomatic, races.diplomatic),
            (enabled.domination, races.domination),
        ]
        .into_iter()
        .filter_map(|(on, progress)| on.then_some(progress))
        .fold(0.0_f64, f64::max)
    }

    /// Which open victory lanes `pid` is furthest along, by the same race
    /// progress [`Self::victory_threat`] takes the maximum of.
    ///
    /// This is what the Mercy Rule's notation names, so it answers with the
    /// lanes rather than the number, and with every lane tied at the front
    /// rather than one of them: a seat holding two races level is a seat the
    /// notation has two things to say about. Lanes are read through
    /// [`Self::victory_lane_open`], so an arena — where four of the six have
    /// nowhere to happen — can only ever be named for the battle it is
    /// actually deciding. Score is left out for the reason it is left out of
    /// the race term in `odds`: it is the standing when the clock runs out,
    /// not a race anybody is running.
    ///
    /// A seat with no progress in any open lane gets an empty answer. That is
    /// a real state — a leader can cross the odds threshold on standing and
    /// tempo alone — and the notation says so by naming no lane.
    pub fn leading_victory_lanes(&self, pid: usize) -> Vec<String> {
        if !self.victory_eligible(pid) {
            return Vec::new();
        }
        let leading_score = self
            .players
            .iter()
            .filter(|p| !p.is_minor && !p.is_barbarian)
            .map(|p| self.team_score_rank_key(p.id).0)
            .max()
            .unwrap_or(0);
        let races = self.victory_races(pid, leading_score);
        let lanes = [
            ("science", races.science),
            ("culture", races.culture),
            ("religious", races.religious),
            ("diplomatic", races.diplomatic),
            ("domination", races.domination),
        ];
        let open = || {
            lanes
                .iter()
                .filter(|(lane, _)| self.victory_lane_open(lane))
        };
        // `total_cmp` rather than `==`: the same ordering the rest of the
        // engine compares race progress with, and the tie this looks for is
        // an exact one — two lanes computed to the same value, not two values
        // that happen to be close.
        let best = open().map(|(_, progress)| *progress).fold(0.0, f64::max);
        if best <= 0.0 {
            return Vec::new();
        }
        open()
            .filter(|(_, progress)| progress.total_cmp(&best) == std::cmp::Ordering::Equal)
            .map(|(lane, _)| (*lane).to_string())
            .collect()
    }

    /// How this game's result is written down and shown: the victory type,
    /// except that a Mercy Rule ending carries the lane notation instead of
    /// the bare word. `None` while no result stands — including a world
    /// playing on past one, which keeps its verdict in
    /// [`Decided::victory_label`].
    pub fn victory_label(&self) -> Option<String> {
        let victory_type = self.victory_type.as_deref()?;
        Some(if victory_type == MERCY_VICTORY {
            mercy_label(&self.mercy_lanes)
        } else {
            victory_type.to_string()
        })
    }

    pub(super) fn victory_eligible(&self, pid: usize) -> bool {
        self.players
            .get(pid)
            .is_some_and(|p| p.alive && !p.is_minor && !p.is_barbarian)
    }

    /// Whether this world can be won this way at all.
    ///
    /// A Civ world answers from the lobby's victory checkboxes. A Tactics
    /// arena answers for itself: there are no cities on it and no way to
    /// found one, so science, culture, religion and diplomacy have nothing to
    /// happen in and could never be reached however the checkboxes were left.
    /// What can happen on an arena is that one side is left standing — the
    /// Domination lane, which on a world with no capitals to capture reduces
    /// to exactly that. Its clock is a deadline rather than a victory lane:
    /// both armies surviving it produces a draw.
    pub(super) fn victory_lane_open(&self, vtype: &str) -> bool {
        if self.is_arena() {
            return vtype == "domination";
        }
        self.victory_conditions.is_enabled(vtype)
    }

    /// The victory lanes this world actually offers, whatever a lobby or a
    /// command line left in `victory_conditions`.
    ///
    /// This is what a client is told, so that the victory tracker shows the
    /// battle a Tactics game is deciding rather than a science race nobody on
    /// the field can run: an arena has no cities, so four of the six lanes
    /// have nowhere to happen and [`Self::victory_lane_open`] refuses them in
    /// any case. Publishing the effective set keeps the display and the rule
    /// the same answer however the world was set up.
    pub fn effective_victory_conditions(&self) -> VictoryConditions {
        if !self.is_arena() {
            return self.victory_conditions;
        }
        VictoryConditions {
            science: false,
            culture: false,
            religious: false,
            diplomatic: false,
            domination: true,
            score: false,
        }
    }

    pub(super) fn set_winner(&mut self, pid: usize, vtype: &str) -> bool {
        if self.play_on_blocks(pid, vtype) {
            return false;
        }
        if !self.is_finished()
            && self.victory_eligible(pid)
            && (self.victory_lane_open(vtype)
                // Mercy answers to its own setup option, not the victory
                // checkboxes: it is a concession rule, not a victory lane.
                || (vtype == MERCY_VICTORY && self.mercy_rule.is_some())
                // The flag answers to the arena objective it was planted by:
                // only a battle set up around one can be won by taking it.
                || (vtype == FLAG_VICTORY && !self.arena_flags.is_empty()))
        {
            // Require-N: short of the requirement, a victory type banks a
            // milestone instead of ending the world. The set dedupes, because
            // culture, religious and diplomatic checks re-fire every turn
            // while their condition holds. Mercy stays terminal: it is a
            // judgement about the whole game, not one lane of it.
            let required = self.effective_required_victories();
            if vtype != MERCY_VICTORY && required > 1 {
                let banked = {
                    let types = self.victories_won.entry(pid).or_default();
                    if !types.insert(vtype.to_string()) {
                        return false;
                    }
                    types.len()
                };
                if banked < required {
                    let name = self.civ_name(pid);
                    let seats: Vec<usize> = self.players.iter().map(|player| player.id).collect();
                    for seat in seats {
                        self.note(
                            seat,
                            "General",
                            format!("{name} banked a {vtype} victory ({banked}/{required} types)"),
                            None,
                        );
                    }
                    return false;
                }
            }
            self.crown(pid, vtype)
        } else {
            false
        }
    }

    pub(super) fn on_unit_lost(&mut self, pid: usize) {
        self.check_elimination(pid);
        self.check_domination();
    }

    pub(super) fn check_elimination(&mut self, pid: usize) {
        // The camp-based Barbarian seat is permanently alive. The separate
        // Free Cities seat becomes dormant again after its last city joins a
        // civilization, so it must pass through ordinary elimination here.
        if !self.players[pid].alive || self.barb_pid == Some(pid) {
            return;
        }
        // Last army standing. On an arena a side is its army: it has no city
        // to lose and no Settler to found one with, so the ordinary test —
        // no city and no Settler left — would wipe out both sides the moment
        // the first casualty of the battle fell. What ends a side here is
        // running out of units, and nothing else does.
        if self.is_arena() {
            if self.units.values().any(|unit| unit.owner == pid) {
                return;
            }
            // A side that still holds its city is not finished *if the city
            // can put a unit back on the field*: granted Production finishes
            // the next unit off the queue, and granted Gold buys one. Losing
            // the last unit ends a side only when it has nothing left to
            // field one with — which is the moment the last unit falls when
            // the arena seats no cities, and equally when it seats one but
            // grants neither Production nor Gold, the stock arena since the
            // grants went to zero. Without this the stock battle could not
            // be won by annihilation: the side that lost its whole army
            // would stand "alive" behind a city that will never build, and
            // the side that destroyed it, if it had no melee left to walk
            // in, would be handed a draw at the clock for winning the fight.
            if self.cities.values().any(|city| city.owner == pid)
                && (self.tactics.production > 0 || self.tactics.gold > 0)
            {
                return;
            }
            self.players[pid].alive = false;
            self.sync_war_log();
            return;
        }
        if self.unseen_city_owners.contains(&pid) {
            return;
        }
        if self.cities.values().any(|c| c.owner == pid) {
            return;
        }
        if self
            .units
            .values()
            .any(|u| u.owner == pid && u.kind == "settler")
        {
            return;
        }
        self.players[pid].alive = false;
        for city in self.cities.values_mut() {
            if city.occupied_from == Some(pid) {
                city.occupied_from = None;
                city.occupation_grievance = None;
            }
        }
        if self.players[pid].is_minor {
            for player in self.players.iter_mut() {
                player.envoys.retain(|(minor, _)| *minor != pid);
            }
        }
        for uid in self.player_unit_ids(pid) {
            self.remove_unit(uid);
        }
        // Every war this civilization was fighting ended the moment it
        // stopped existing; close those records now so they carry the turn it
        // happened rather than the turn something else touched diplomacy.
        self.sync_war_log();
    }

    pub(super) fn check_domination(&mut self) {
        let majors: Vec<usize> = self
            .players
            .iter()
            .filter(|p| !p.is_minor && !p.is_barbarian)
            .map(|p| p.id)
            .collect();
        if majors.len() < 2 {
            return;
        }
        for candidate in majors.iter().copied().filter(|p| self.victory_eligible(*p)) {
            if self.players[candidate].team.is_some() {
                let team = self.team_members(candidate);
                let team_holds_its_capitals = team.iter().all(|member| {
                    self.cities.values().any(|capital| {
                        capital.is_capital
                            && capital.original_owner == *member
                            && capital.owner == *member
                    })
                });
                let every_opponent_lost_their_capital = majors
                    .iter()
                    .copied()
                    .filter(|original_owner| {
                        *original_owner != candidate && !self.same_team(candidate, *original_owner)
                    })
                    .all(|original_owner| {
                        self.cities
                            .values()
                            .find(|capital| {
                                capital.is_capital && capital.original_owner == original_owner
                            })
                            .is_none_or(|capital| capital.owner != original_owner)
                    });
                if team_holds_its_capitals && every_opponent_lost_their_capital {
                    self.set_winner(candidate, "domination");
                    return;
                }
                continue;
            }
            // "To win a Domination victory, you must capture all original
            // civilization Capitals" — all of them, the candidate's own
            // included. A civilization that has lost its own Capital cannot
            // win this way until it takes it back, which is exactly the rule
            // the team branch above spells out for a whole side.
            let controls_every_original_capital = majors.iter().copied().all(|original_owner| {
                match self
                    .cities
                    .values()
                    .find(|c| c.is_capital && c.original_owner == original_owner)
                {
                    // On an arena a side can fall with its city still
                    // standing in its own name — its last unit gone and no
                    // grant to field another — and a battle is decided by
                    // the armies, so a defeated seat's empty city satisfies
                    // the lane the way a defeated seat with no city does
                    // below. A world never reaches this: a civilization that
                    // owns a city is by definition still in the game.
                    Some(capital) => {
                        capital.owner == candidate
                            || (self.is_arena() && !self.players[original_owner].alive)
                    }
                    // The engine begins with settlers, so a Capital can be
                    // missing rather than lost. Defeating a civ before it
                    // founds one satisfies that seat, and a candidate that
                    // has never founded its own has none to have lost.
                    None => original_owner == candidate || !self.players[original_owner].alive,
                }
            });
            if controls_every_original_capital {
                self.set_winner(candidate, "domination");
                return;
            }
        }
    }

    /// Whether the game has been asked to carry on past a decided result.
    pub fn played_on(&self) -> bool {
        self.decided.is_some()
    }

    /// Whether this game has reached any terminal result. Most results name a
    /// winner; a Tactics battle that reaches its selected deadline is the one
    /// deliberate exception.
    pub fn is_finished(&self) -> bool {
        self.winner.is_some() || self.is_draw()
    }

    /// Whether the terminal result is a draw rather than a victory.
    pub fn is_draw(&self) -> bool {
        self.winner.is_none() && self.victory_type.as_deref() == Some(DRAW_RESULT)
    }

    /// A played-on world has explicitly left its original turn cap behind.
    /// Keep the configured cap itself intact so a save or successor world can
    /// still recover the game settings that created this one.
    pub fn turn_limit(&self) -> Option<u32> {
        (!self.played_on()).then_some(self.max_turns)
    }

    /// Finish an over-cap world when Score is not one of its enabled victory
    /// lanes. The normal end-turn path always tries the score tiebreak at the
    /// cap, but `set_winner` correctly refuses a disabled lane; an unattended
    /// server would otherwise keep stepping the same world forever.
    ///
    /// Returns `true` when this call made the world terminal. A played-on
    /// world has no cap, and a Score-enabled world is left to the ordinary
    /// tiebreak in [`Self::do_end_turn`].
    pub(crate) fn finish_at_turn_limit(&mut self) -> bool {
        if self.is_finished()
            || self.played_on()
            || self.turn <= self.max_turns
            || self.victory_lane_open("score")
        {
            return false;
        }
        if self.is_arena() {
            return self.declare_draw();
        }

        self.victory_type = Some(DRAW_RESULT.to_string());
        let seats: Vec<usize> = self.players.iter().map(|player| player.id).collect();
        for seat in seats {
            self.note(
                seat,
                "General",
                format!(
                    "The game reached its turn limit ({}) without an enabled victory",
                    self.max_turns
                ),
                None,
            );
        }
        true
    }

    /// The turn this world is reported on.
    ///
    /// A score victory or Tactics draw is settled on the wrap out of the final
    /// turn — the one place `turn` ever passes the limit. That wrap is
    /// bookkeeping, not a turn anybody plays: a 250-turn game is decided on
    /// turn 250, so turn 250 is what every account of the result says. Every
    /// other reading, live game or any other victory, is the turn being played.
    pub fn reported_turn(&self) -> u32 {
        match (self.victory_type.as_deref(), self.turn_limit()) {
            (Some("score" | DRAW_RESULT), Some(limit)) => limit.min(self.turn),
            _ => self.turn,
        }
    }

    /// Carry on past the result this game already reached: "one more turn".
    ///
    /// The verdict is kept in `decided` and the world becomes live again with
    /// no turn limit. `UntilNextVictory` suppresses only that exact winner and
    /// path, so an already-satisfied Culture or Domination condition cannot
    /// instantly re-declare the same result; every genuinely later result can
    /// still land. `Indefinite` suppresses every later result.
    ///
    /// Returns `false` when there is no result to play on past.
    pub fn play_on(&mut self, mode: PlayOnMode) -> bool {
        let (Some(winner), Some(victory_type)) = (self.winner, self.victory_type.clone()) else {
            return false;
        };
        // Read before `decided` is set: a played-on world has no turn limit
        // left to date a score victory against.
        let decided_on = self.reported_turn();
        // Replace rather than retain an older verdict: a world can reach a new
        // victory after `UntilNextVictory`, and another press must continue
        // past the result currently on its finish screen.
        self.decided = Some(Decided {
            winner,
            victory_type: victory_type.clone(),
            // Moved rather than copied: the extension is a live, undecided
            // world again, and a later ending of its own must not be labelled
            // with the lanes that ended the game before it.
            mercy_lanes: std::mem::take(&mut self.mercy_lanes),
            turn: decided_on,
            mode,
        });
        self.winner = None;
        self.victory_type = None;
        let seats: Vec<usize> = self.players.iter().map(|player| player.id).collect();
        for seat in seats {
            self.note(
                seat,
                "General",
                // A note is a predicate: the chronicle prints it after the
                // seat's own name, so "Rome plays on until the next victory".
                match mode {
                    PlayOnMode::UntilNextVictory => "plays on until the next victory".to_string(),
                    PlayOnMode::Indefinite => "plays on indefinitely".to_string(),
                },
                None,
            );
        }
        // A decided game stopped mid-turn: whoever is up has to be given their
        // turn again, or the extension opens on a seat that cannot act.
        self.begin_turn(self.current);
        self.sync_war_log();
        true
    }

    /// Whether a play-on extension refuses this result outright.
    pub(super) fn play_on_blocks(&self, pid: usize, vtype: &str) -> bool {
        self.decided
            .as_ref()
            .is_some_and(|decided| match decided.mode {
                PlayOnMode::Indefinite => true,
                PlayOnMode::UntilNextVictory => {
                    decided.winner == pid && decided.victory_type == vtype
                }
            })
    }

    /// The Require-N setting, clamped to what the lobby actually enabled:
    /// requiring more victory types than exist could never be satisfied, and
    /// zero — a save written before the option existed — is the stock
    /// single-victory game.
    pub fn effective_required_victories(&self) -> usize {
        // A battle has one victory type: last army standing. Its clock ends
        // in a draw and is not a second lane to require.
        if self.is_arena() {
            return 1;
        }
        let enabled = VictoryConditions::NAMES
            .iter()
            .filter(|name| self.victory_lane_open(name))
            .count();
        self.required_victory_types.clamp(1, enabled.max(1))
    }

    /// End a Tactics battle at its deadline without crediting either side.
    pub(super) fn declare_draw(&mut self) -> bool {
        if self.is_finished() || self.played_on() {
            return false;
        }
        self.victory_type = Some(DRAW_RESULT.to_string());
        let seats: Vec<usize> = self.players.iter().map(|player| player.id).collect();
        for seat in seats {
            self.note(
                seat,
                "General",
                format!(
                    "The Tactics battle ended in a draw after {} turns",
                    self.max_turns
                ),
                None,
            );
        }
        true
    }

    /// Declare the result: the tail of every victory, shared by the normal
    /// single-victory path, the Require-N completion, and the turn-limit
    /// most-banked-types award. Checks only what every caller must respect —
    /// play-on and an already-decided world — so type legality stays with
    /// [`Self::set_winner`].
    pub(super) fn crown(&mut self, pid: usize, vtype: &str) -> bool {
        if self.is_finished() || self.play_on_blocks(pid, vtype) {
            return false;
        }
        self.winner = Some(pid);
        self.victory_type = Some(vtype.to_string());
        let winners = self.team_members(pid);
        let winner = if winners.len() > 1 {
            let names = winners
                .iter()
                .map(|winner| self.civ_name(*winner))
                .collect::<Vec<_>>()
                .join(" and ");
            format!("Team {names}")
        } else {
            self.civ_name(pid)
        };
        // A Mercy Rule ending is announced by its notation rather than called
        // a "mercy victory": the rule is a concession, and what there is to
        // report is the lane the board was being decided on when it fired.
        let verdict = if vtype == MERCY_VICTORY {
            format!("won by {}", mercy_label(&self.mercy_lanes))
        } else if vtype == FLAG_VICTORY {
            // The verdict names the deed: a flag battle is not "a flag
            // victory", it is the flag captured.
            "captured the flag".to_string()
        } else {
            format!("won a {vtype} victory")
        };
        let seats: Vec<usize> = self.players.iter().map(|player| player.id).collect();
        for seat in seats {
            self.note(seat, "General", format!("{winner} {verdict}"), None);
        }
        true
    }
}
