//! Native competitions: the World Congress's scored contests between
//! civilizations.
//!
//! Opening and closing a native competition and the host's own
//! (`open_native_competition`, `close_native_competition`,
//! `host_competition`, `replace_host_competitions`); whether a game meets a
//! competition's trait and a player can score it; scoring by holdings,
//! favor, Great Person points or the competition's own rule
//! (`score_*_competition`); and the diplomatic victory points a result
//! awards.
//!
//! Moved verbatim out of the `impl Game` block in `game/city.rs`
//! (2026-11-05). Methods the rest of `city` calls are `pub(super)`;
//! methods that were `pub(super)` (all of `game`) are
//! `pub(in crate::game)`, the same reach; the code is otherwise unchanged.

use super::*;

impl Game {
    /// Replace the current live-host competition snapshot. The production
    /// catalog persists beyond an ordinary read-only memo scope, so a host
    /// competition starting or ending must explicitly retire any menu cached
    /// before the new snapshot arrived.
    pub(crate) fn replace_host_competitions(&mut self, competitions: Vec<HostCompetition>) {
        if *self.host_competitions != competitions {
            self.host_competitions = Arc::new(competitions);
            self.query_memo.producible.borrow_mut().clear();
        }
    }

    /// The current host competition of this mirrored seat, if it is still
    /// active. Host player ids do not match generated CIVVIS seats, so the
    /// mirror intentionally supplies opportunities for seat zero only.
    pub fn host_competition(&self, pid: usize, kind: &str) -> Option<HostCompetition> {
        // A competition CIVVIS is running itself answers for every seat, and
        // presents in the same shape a mirrored one does — so the production
        // catalog and the AI's valuation cannot tell them apart and neither
        // needed changing.
        if let Some(native) = self
            .competition
            .as_ref()
            .filter(|running| running.kind == kind && running.ends > self.turn)
        {
            if native.target == Some(pid) {
                return None;
            }
            let ours = native.scores.get(&pid).copied().unwrap_or(0.0);
            let leader = native.scores.values().copied().fold(0.0, f64::max);
            return Some(HostCompetition {
                kind: native.kind.clone(),
                ends: native.ends,
                ours,
                leader,
            });
        }
        (pid == 0)
            .then(|| {
                self.host_competitions
                    .iter()
                    .find(|competition| competition.kind == kind && competition.ends > self.turn)
                    .cloned()
            })
            .flatten()
    }

    /// The competitions CIVVIS can seat itself, with the Diplomatic Victory
    /// Points Gathering Storm pays their winner.
    ///
    /// Every field is read off the installed Gathering Storm ruleset —
    /// `EmergencyAlliances` for the trigger, `Duration` and `LockoutTime`, that
    /// row's `TargetRequirementSet` for the era window, `EmergencyScoreSources`
    /// for what counts, and `EmergencyRewards` joined to `ModifierArguments`
    /// for the award:
    ///
    /// | competition | era | scores | first place |
    /// |---|---|---|---|
    /// | World's Fair | Modern only | Great Person Points per turn | 1 point, 50 Favor |
    /// | World Games | Atomic+ | athletes project, Stadiums and Aquatics Centers per turn | 1 point, 50 Favor |
    /// | Climate Accords | Information+ | the three decommissioning projects | 2 points, 100 Favor |
    /// | International Space Station | Future+ | astronauts project, Spaceports and Campuses per turn | 1 point, 50 Favor |
    /// | Nobel Peace Prize | Industrial+, Sweden in the game | Diplomatic Favor per turn | 1 point |
    /// | Send Aid | any | the aid project | 2 points, 100 Favor |
    /// | Send Military Aid | any | the aid project | 2 points, 100 Favor |
    ///
    /// ⚠ **The Nobel prizes for Literature and Physics are deliberately absent
    /// and are not a gap.** They are scored competitions like the rest, but
    /// `EmergencyRewards` gives neither of them a
    /// `NON_EMERGENCY_FIRST_PLACE_VICTORY_POINT` row — Literature's first place
    /// takes cheaper Rock Bands and Physics' a technology boost — so neither is
    /// a source of a Diplomatic Victory Point at all. Only Peace is.
    ///
    /// ⚠ **Order is the seating preference**, because the shipped data does not
    /// say how the congress picks among the competitions it could offer: that
    /// choice lives in the compiled engine, not in `Emergencies_XP2`. The list
    /// is newest-era-first, so the latest competition an era has unlocked takes
    /// the seat and the Nobel Peace Prize — the only one available before the
    /// Modern era — takes it in the Industrial era and in the gaps another
    /// competition's per-kind lockout leaves.
    pub(in crate::game) const NATIVE_COMPETITIONS: &'static [NativeCompetitionSpec] = &[
        NativeCompetitionSpec {
            kind: "EMERGENCY_SPACE_STATION",
            diplomatic_victory_points: 1,
            first_place_favor: 50.0,
            scoring: &[
                CompetitionScoreSource::Project,
                CompetitionScoreSource::DistrictPerTurn {
                    district: "spaceport",
                    amount: 5.0,
                },
                CompetitionScoreSource::DistrictPerTurn {
                    district: "campus",
                    amount: 1.0,
                },
            ],
            trigger: NativeCompetitionTrigger::Congress,
            duration: 29,
            lockout: 60,
            minimum_world_era: 8,
            maximum_world_era: None,
            required_civilization_ability: None,
        },
        NativeCompetitionSpec {
            kind: "EMERGENCY_CLIMATE_ACCORDS",
            diplomatic_victory_points: 2,
            first_place_favor: 100.0,
            scoring: &[CompetitionScoreSource::Project],
            trigger: NativeCompetitionTrigger::Congress,
            duration: 29,
            lockout: 60,
            minimum_world_era: 7,
            maximum_world_era: None,
            required_civilization_ability: None,
        },
        NativeCompetitionSpec {
            kind: "EMERGENCY_WORLD_GAMES",
            diplomatic_victory_points: 1,
            first_place_favor: 50.0,
            scoring: &[
                CompetitionScoreSource::Project,
                CompetitionScoreSource::BuildingPerTurn {
                    building: "stadium",
                    amount: 1.0,
                },
                CompetitionScoreSource::BuildingPerTurn {
                    building: "aquatics_center",
                    amount: 1.0,
                },
            ],
            trigger: NativeCompetitionTrigger::Congress,
            duration: 29,
            lockout: 60,
            minimum_world_era: 6,
            maximum_world_era: None,
            required_civilization_ability: None,
        },
        NativeCompetitionSpec {
            kind: "EMERGENCY_WORLDS_FAIR",
            diplomatic_victory_points: 1,
            first_place_favor: 50.0,
            scoring: &[CompetitionScoreSource::GreatPersonPointsPerTurn],
            trigger: NativeCompetitionTrigger::Congress,
            duration: 29,
            lockout: 60,
            minimum_world_era: 5,
            maximum_world_era: Some(5),
            required_civilization_ability: None,
        },
        // Sweden's `TRAIT_CIVILIZATION_NOBEL_PRIZE` is the whole reason the
        // Nobel prizes exist in a game: `NOBEL_PRIZE_TARGET_REQUIREMENTS` tests
        // `REQUIREMENT_GAME_HAS_CIVILIZATION_OR_LEADER_TRAIT` for it. Without
        // Sweden on the board the congress never offers one, which is why this
        // competition is rare rather than a route every empire has.
        NativeCompetitionSpec {
            kind: "EMERGENCY_NOBEL_PRIZE_PEACE",
            diplomatic_victory_points: 1,
            first_place_favor: 0.0,
            scoring: &[CompetitionScoreSource::DiplomaticFavorPerTurn],
            trigger: NativeCompetitionTrigger::Congress,
            duration: 29,
            lockout: 60,
            minimum_world_era: 4,
            maximum_world_era: None,
            required_civilization_ability: Some("nobelinstitution"),
        },
        NativeCompetitionSpec {
            kind: "EMERGENCY_SEND_AID",
            diplomatic_victory_points: 2,
            first_place_favor: 100.0,
            scoring: &[
                CompetitionScoreSource::Project,
                CompetitionScoreSource::GoldGift,
            ],
            trigger: NativeCompetitionTrigger::RandomDisasterPopulationLoss,
            duration: 30,
            lockout: 30,
            minimum_world_era: 0,
            maximum_world_era: None,
            required_civilization_ability: None,
        },
        NativeCompetitionSpec {
            kind: "EMERGENCY_SEND_MILITARY_AID",
            diplomatic_victory_points: 2,
            first_place_favor: 100.0,
            scoring: &[
                CompetitionScoreSource::Project,
                CompetitionScoreSource::GoldGift,
            ],
            trigger: NativeCompetitionTrigger::WarWithGrievances,
            duration: 30,
            lockout: 30,
            minimum_world_era: 0,
            maximum_world_era: None,
            required_civilization_ability: None,
        },
    ];

    pub(in crate::game) fn native_competition(
        kind: &str,
    ) -> Option<&'static NativeCompetitionSpec> {
        Self::NATIVE_COMPETITIONS
            .iter()
            .find(|competition| competition.kind == kind)
    }

    /// The Diplomatic Victory Points this competition's first place pays, or
    /// zero for a competition CIVVIS does not seat itself.
    ///
    /// The table above is the authority on what a competition is worth to the
    /// diplomatic race; a planner that wants to price those points must not
    /// carry its own copy of it. See `AdvancedAi::competition_victory_point_value`.
    pub fn competition_victory_points(kind: &str) -> i64 {
        Self::native_competition(kind).map_or(0, |spec| spec.diplomatic_victory_points)
    }

    /// Seat a competition if one may run and an empire could score in it.
    pub(in crate::game) fn open_native_competition(&mut self) {
        if !self.native_competitions || self.competition.is_some() {
            return;
        }
        let majors: Vec<usize> = self
            .players
            .iter()
            .filter(|player| self.victory_eligible(player.id))
            .map(|player| player.id)
            .collect();
        let Some(competition) = Self::NATIVE_COMPETITIONS.iter().find(|competition| {
            competition.trigger == NativeCompetitionTrigger::Congress
                && competition.offered_in_world_era(self.world_era)
                && self.game_meets_competition_trait(competition)
                && self
                    .competition_lockout_until
                    .get(competition.kind)
                    .is_none_or(|until| self.turn >= *until)
                && majors
                    .iter()
                    .any(|pid| self.can_score_competition(*pid, competition.kind))
        }) else {
            return;
        };
        self.competition = Some(Competition {
            kind: competition.kind.to_string(),
            ends: self.turn + self.standard_duration(competition.duration),
            target: None,
            scores: BTreeMap::new(),
        });
        self.query_memo.producible.borrow_mut().clear();
    }

    /// Whether the game contains the civilization a competition's emergency
    /// requires before it exists at all.
    ///
    /// `NOBEL_PRIZE_TARGET_REQUIREMENTS` tests
    /// `REQUIREMENT_GAME_HAS_CIVILIZATION_OR_LEADER_TRAIT` for
    /// `TRAIT_CIVILIZATION_NOBEL_PRIZE`, which `CivilizationTraits` gives to
    /// `CIVILIZATION_SWEDEN` alone. A game with no Sweden in it never sees a
    /// Nobel prize, and that is the shipped rule rather than a simplification.
    pub(in crate::game) fn game_meets_competition_trait(
        &self,
        competition: &NativeCompetitionSpec,
    ) -> bool {
        let Some(ability) = competition.required_civilization_ability else {
            return true;
        };
        self.players
            .iter()
            .any(|player| self.has_ability(player.id, ability))
    }

    /// Whether this empire could score at all in a competition, after its exact
    /// era gate has selected it.
    ///
    /// Every source the competition declares is asked, because a competition
    /// nobody can score in pays nobody and still spends its lockout.
    pub(in crate::game) fn can_score_competition(&self, pid: usize, kind: &str) -> bool {
        let Some(competition) = Self::native_competition(kind) else {
            return false;
        };
        competition.scoring.iter().any(|source| match source {
            CompetitionScoreSource::GoldGift => self.players[pid].gold >= 1.0,
            // Every empire generates Great Person Points and Diplomatic Favor,
            // so there is no ground to hold and nothing to gate on.
            CompetitionScoreSource::GreatPersonPointsPerTurn
            | CompetitionScoreSource::DiplomaticFavorPerTurn => true,
            CompetitionScoreSource::DistrictPerTurn { district, .. } => {
                let district = Name::new(district);
                self.cities
                    .values()
                    .any(|city| city.owner == pid && city.districts.contains_key(district))
            }
            CompetitionScoreSource::BuildingPerTurn { building, .. } => {
                let building = Name::new(building);
                self.cities
                    .values()
                    .any(|city| city.owner == pid && city.buildings.contains(&building))
            }
            CompetitionScoreSource::Project => self.rules.projects.iter().any(|(_, spec)| {
                if spec.competition_score <= 0.0
                    || !spec.host_competition_kinds().any(|k| k == kind)
                {
                    return false;
                }
                // ⚠ The district is not the whole requirement. A decommissioning
                // project also eats a power plant, and a competition offered to an
                // empire that holds none is a competition nobody can score in: the
                // first trace of this seated Climate Accords on turn 100 and closed
                // it on 119 with no score at all, having spent the lockout.
                self.cities.values().any(|city| {
                    city.owner == pid
                        && spec
                            .district
                            .is_none_or(|district| city.districts.contains_key(district))
                        && spec
                            .consumes_buildings
                            .iter()
                            .all(|building| city.buildings.contains(&Name::new(building)))
                })
            }),
        })
    }

    /// Add `amount` to this seat's score, if the running competition counts
    /// `source`.
    ///
    /// ⚠ **Nothing is paid on the mirrored path.** `competition` is set only by
    /// native seating; a mirrored competition lives in `host_competitions`, and
    /// the host has already counted its own score and paid its own award.
    pub(in crate::game) fn score_native_competition(
        &mut self,
        pid: usize,
        source: CompetitionScoreSource,
        amount: f64,
    ) {
        if !amount.is_finite() || amount <= 0.0 || !self.victory_eligible(pid) {
            return;
        }
        let turn = self.turn;
        let counts = self
            .competition
            .as_ref()
            .filter(|running| running.ends > turn && running.target != Some(pid))
            .and_then(|running| Self::native_competition(&running.kind))
            .is_some_and(|competition| competition.counts(source));
        if !counts {
            return;
        }
        if let Some(running) = self.competition.as_mut() {
            *running.scores.entry(pid).or_insert(0.0) += amount;
        }
    }

    /// The World's Fair's eight `WORLDS_FAIR_SCORE_GPP_*` rows: one point per
    /// Great Person Point generated this turn, whatever the class.
    ///
    /// ⚠ This counts **points**, not people. Every row is `ScoreAmount="1"`
    /// against a `FromGreatPerson` class, and their shared description is
    /// `LOC_EMERGENCY_SCORE_GPP_DESC`, "Generating Great People Points Per
    /// Turn". Counting recruits instead — which is what CIVVIS did until this
    /// change — reads the same table two orders of magnitude too small, and
    /// leaves a 29-turn competition to be decided by whether two empires each
    /// happened to claim one person, which is a tie, and a tie pays nobody.
    pub(in crate::game) fn score_great_person_point_competition(
        &mut self,
        pid: usize,
        points: f64,
    ) {
        self.score_native_competition(
            pid,
            CompetitionScoreSource::GreatPersonPointsPerTurn,
            points,
        );
    }

    /// `NOBEL_PRIZE_PEACE_SCORE_FROM_FAVOR`: one point per Diplomatic Favor
    /// generated this turn.
    ///
    /// The score source is described "Generating [ICON_Favor] Diplomatic
    /// Favor", the same "Generating … Per Turn" cadence the World's Fair uses,
    /// so it is this turn's favor *income* — what `process_diplomacy` computes
    /// and banks in the `diplomatic_favor` counter. It is deliberately not the
    /// balance, and deliberately not favor that merely arrives: a congress
    /// refund, a trade, or an emergency award is not favor the empire
    /// generated.
    pub(in crate::game) fn score_favor_competition(&mut self, pid: usize, favor: f64) {
        self.score_native_competition(pid, CompetitionScoreSource::DiplomaticFavorPerTurn, favor);
    }

    /// The `FromDistrict` and `FromBuilding` rows, which pay for *maintaining*
    /// what they name and therefore accrue every turn the competition runs.
    ///
    /// The International Space Station counts Spaceports at 5 and Campuses at
    /// 1; the World Games counts Stadiums and Aquatics Centers at 1. Without
    /// them a competition seated over ground nobody chooses to spend production
    /// on closes with an empty score table and pays nobody, which is exactly
    /// what the first native trace recorded.
    pub(in crate::game) fn score_competition_holdings(&mut self, pid: usize) {
        // ⚠ Majors only. `begin_turn` runs for every seat, and a city-state
        // holds Campuses like anyone else — but an emergency's members are the
        // majors, and a city-state that outscored them would take a Diplomatic
        // Victory Point off the board for nobody.
        if !self.victory_eligible(pid) {
            return;
        }
        let turn = self.turn;
        let Some(spec) = self
            .competition
            .as_ref()
            .filter(|running| running.ends > turn && running.target != Some(pid))
            .and_then(|running| Self::native_competition(&running.kind))
            .copied()
        else {
            return;
        };
        for source in spec.scoring {
            let held = match source {
                CompetitionScoreSource::DistrictPerTurn { district, amount } => {
                    let district = Name::new(district);
                    let cities = self
                        .cities
                        .values()
                        .filter(|city| city.owner == pid && city.districts.contains_key(district))
                        .count();
                    amount * cities as f64
                }
                CompetitionScoreSource::BuildingPerTurn { building, amount } => {
                    let building = Name::new(building);
                    let cities = self
                        .cities
                        .values()
                        .filter(|city| city.owner == pid && city.buildings.contains(&building))
                        .count();
                    amount * cities as f64
                }
                _ => 0.0,
            };
            if held > 0.0 {
                if let Some(running) = self.competition.as_mut() {
                    *running.scores.entry(pid).or_insert(0.0) += held;
                }
            }
        }
    }

    /// Close a finished competition, paying its winner what Gathering Storm
    /// pays: the Diplomatic Victory Point, and the Favor beside it.
    ///
    /// ⚠ First place only, and ties pay nobody — a tie has no first place, and
    /// inventing a tiebreak here would be inventing a rule. Nothing is paid on
    /// the mirrored path either: a host has already counted its own.
    pub(in crate::game) fn close_native_competition(&mut self) {
        let Some(running) = self.competition.as_ref() else {
            return;
        };
        if self.turn < running.ends {
            return;
        }
        let spec = Self::native_competition(&running.kind).copied();
        let award = spec
            .map(|competition| competition.diplomatic_victory_points)
            .unwrap_or(0);
        let kind = running.kind.clone();
        let until = self.turn
            + self.standard_duration(spec.map(|competition| competition.lockout).unwrap_or(60));
        let best = running.scores.values().copied().fold(0.0, f64::max);
        let winners: Vec<usize> = running
            .scores
            .iter()
            .filter(|(_, score)| **score >= best && best > 0.0)
            .map(|(pid, _)| *pid)
            .collect();
        let favor = spec
            .map(|competition| competition.first_place_favor)
            .unwrap_or(0.0);
        if let [winner] = winners[..] {
            self.players[winner].dvp += award;
            self.players[winner].diplomatic_favor += favor;
            self.add_historic_moment(winner, "MOMENT_PLAYER_EARNED_DIPLOMATIC_VICTORY_POINT");
        }
        self.competition = None;
        self.competition_lockout_until.insert(kind, until);
        self.query_memo.producible.borrow_mut().clear();
    }
}
