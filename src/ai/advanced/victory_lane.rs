//! Six genes for the victory lanes: the race the empire is actually in,
//! reaching the deciders that still read the expansion posture instead.
//!
//! All six are opt-in (`PRODUCTION_OPT_INS`), ship off, and are priced by
//! `gene_screen` before any promotion question is asked — see
//! `docs/GENE_SCREEN.md` and `docs/VICTORY_GENES.md`. They live in their own
//! file because they are one sentence each and `src/ai/advanced.rs` is the
//! most contended file in the repository (`tools/conflict_hotspots.py`).
//!
//! ## The measurement all five lane genes come from
//!
//! `assess` returns one `StrategicPlan::strategy`, and `take_turn_inner`
//! hands it to every lane-shaped subsystem in the turn: the World Congress
//! ballot, Great Person patronage, the policy deck, the Culture faith pass,
//! the space race. That is right for a plan that carries victory content.
//! It is not what the plan carries while the empire is still settling:
//! `assess` returns **`Expansion`** — for an assigned lane explicitly
//! ("the assigned lane can still afford to expand first"), and for an
//! adaptive seat whenever it is short of cities with land still open.
//!
//! Measured at the deployment shape (6 players, 74×46, Online, 250 turns,
//! six games a lane, seeds 52000000.. and 53000000..):
//!
//! | seat | Expansion | Conquest | Recovery | on a victory lane |
//! |---|---:|---:|---:|---:|
//! | `--target culture` | 20.6% | 0.0% | 0.0% | 79.4% |
//! | `--target diplomatic` | 19.7% | 0.0% | 0.0% | 80.3% |
//! | `--target science` | 18.9% | 0.0% | 0.0% | 81.1% |
//! | `--target religious` | 5.2% | 0.0% | 0.0% | 94.8% |
//! | adaptive (no target) | 15.0% | 22.1% | 10.9% | **52.0%** |
//!
//! A targeted seat spends about a fifth of the game with no victory content
//! in its plan, and those are the **opening** turns — the ones
//! `docs/OPENINGS.md` measures as the correlate that decides the game. An
//! adaptive seat, which is what production ships and what every seat in a
//! native `gene_screen` game is, spends **48% of its seat-turns** under
//! Expansion, Conquest or Recovery.
//!
//! `victory_focus` already answers "which victory is this empire racing" for
//! both kinds of seat — the assigned target when there is one, the
//! best-progress lane when there is not — and `take_turn_inner` already uses
//! exactly that resolution for city dispositions. These genes extend it, one
//! decider at a time, to the deciders that still read the plan.
//!
//! ## Two scopes, because there are two kinds of decider
//!
//! **A choice among options the empire is making anyway** — the ballot, the
//! patronage ranking, the policy deck — only needs the lane when the plan has
//! none. [`AdvancedAi::raced_lane`] therefore answers `None` for every plan
//! that is not `Expansion`: a `Conquest` or `Recovery` plan is a *deliberate*
//! refusal of the economic lanes, and overriding it is a different and much
//! riskier claim than filling in a posture that simply has no victory content
//! yet. The war case is left for its own gene and its own screen.
//!
//! **A whole pass switched off unless the plan names the lane** —
//! `culture_spending`, `space_race_production` — is different, and the
//! difference was measured rather than reasoned. Restricted to `Expansion`,
//! both were **strictly inert**: 0 of 4 paired `victory_eval` games diverged
//! for either, because a targeted seat's expansion window shuts at
//! `standard_duration(175)` (turn ~87 at Online) and neither pass can do
//! anything before the `conservation`, `cold_war` and `rocketry` era. What is
//! actually missing is not the settling turns: an **adaptive** seat holds the
//! Culture plan for 5.0% of its turns and the Science plan for 25.3%, so
//! those passes are all but unreachable without an assigned target. These two
//! follow the race under any posture short of `Recovery`.
//!
//! Pricing a currency is a third case and needs no posture test at all:
//! `competition-victory-points` asks only whether this empire is racing
//! Diplomacy.
//!
//! ## The genes
//!
//! **`lane-congress-ballot`.** The World Congress ballot is scored, and
//! backed with Favor, by `plan.strategy`. A Diplomacy seat still settling
//! therefore scores `world_leader` as an expansion problem — the 1,000-point
//! "nominate ourselves" branch is keyed on `GrandStrategy::Diplomacy` — and
//! casts the free single vote instead of `congress_affordable_votes`. An
//! exact prediction of a resolution's winning outcome *and* target is +1
//! Diplomatic Victory Point, one twentieth of that victory, and a losing
//! ballot is refunded in full. With this on the ballot and its weight read
//! the raced lane.
//!
//! **`lane-great-people`.** `advanced_great_people` ranks the classes by
//! `(strategy, kind)` affinity — Scientist 2.5 to Science, the three writer
//! classes 2.6 to Culture, Merchant 2.0 to Diplomacy — and an Expansion plan
//! reads 1.8 for Engineer and Merchant and 0.85 for everything else. A Great
//! Person is a finite global race: the class the lane needs is gone by the
//! time the plan says so.
//!
//! **`lane-policy-deck`.** `strategic_policies` picks the cards by the same
//! argument. The Culture lane's completed games seat `heritage_tourism`,
//! `satellite_broadcasts` and `sports_media`; an expansion deck seats none
//! of them.
//!
//! **`lane-culture-spending`.** `culture_spending` — the Naturalist that
//! founds a National Park, the Rock Bands that tour — runs only when
//! `plan.strategy == Culture`, and the Faith reserve that keeps a Naturalist
//! affordable is chosen by the same value. Both follow the race instead.
//!
//! **`lane-space-race`.** Every gate in `science_production` asks for an
//! **explicitly assigned** `VictoryTarget::Science`: the pad count (1 rather
//! than the 3 the parallel laser race needs), the city a launch project may
//! claim, and the city a pad may be sited in. Production's agent has no
//! assigned target at all, so it races the space race at one pad and only in
//! cities with nothing else queued. A seat whose `victory_focus` is Science
//! is treated as a Science seat by all three, and the pass itself opens for
//! it. `score_horizon` still refuses a race that cannot finish.
//!
//! **`competition-victory-points`.** A scored competition's first place pays
//! Diplomatic Victory Points: 2 for the Climate Accords, Send Aid and Send
//! Military Aid, 1 for the World Games, the World's Fair and the
//! International Space Station (`Game::NATIVE_COMPETITIONS`). Thirteen of
//! the twenty points a diplomatic victory needs come from the Congress and
//! these competitions, and `host_competition_score_value` prices **none of
//! them** — it prices the competition's own score, at the same rate for a
//! Conquest seat as for a Diplomacy one. This is the absence
//! `strategic_wonder_value` already fixed for wonders, in the other half of
//! the same lane, using the same rule and the same denominator: one point is
//! `STRATEGIC_WONDER_VICTORY_VALUE / DIPLOMATIC_VICTORY_POINTS`. Paid only
//! where the points are actually collectable — this empire is racing
//! Diplomacy, the competition is open, and this completion would put the seat
//! at or in front of the leader.

use super::{AdvancedAi, GrandStrategy, StrategicPlan};
use crate::game::Game;

/// What one Diplomatic Victory Point is worth to a Diplomacy lane, in the
/// units `production_value` ranks in. The same number
/// `strategic_wonder_value` pays for the Statue of Liberty's four points —
/// `STRATEGIC_WONDER_VICTORY_VALUE / DIPLOMATIC_VICTORY_POINTS` — because a
/// point won in the Congress and a point built in a city are the same point.
pub(super) fn victory_point_value() -> f64 {
    super::STRATEGIC_WONDER_VICTORY_VALUE / crate::game::DIPLOMATIC_VICTORY_POINTS.max(1) as f64
}

impl AdvancedAi {
    /// The victory this empire is racing, when its plan carries no victory
    /// content of its own.
    ///
    /// `None` unless the plan is `Expansion` — see the module header for why
    /// a war posture is deliberately out of scope — and `None` when the
    /// raced lane is not one of the four economic victories, so a seat whose
    /// best lane is Conquest or the score tally keeps the plan it already
    /// has.
    pub(super) fn raced_lane(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> Option<GrandStrategy> {
        if plan.strategy != GrandStrategy::Expansion {
            return None;
        }
        let lane = self.victory_focus(g, pid).strategy;
        matches!(
            lane,
            GrandStrategy::Science
                | GrandStrategy::Culture
                | GrandStrategy::Religion
                | GrandStrategy::Diplomacy
        )
        .then_some(lane)
    }

    /// The raced lane when `armed` and the plan has no victory content of its
    /// own, the plan's own strategy otherwise. Every lane gene below is one
    /// call to this at one decider.
    fn lane_or_plan(
        &self,
        armed: bool,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> GrandStrategy {
        if armed {
            self.raced_lane(g, pid, plan).unwrap_or(plan.strategy)
        } else {
            plan.strategy
        }
    }

    /// `lane-congress-ballot`: the lane the World Congress ballot is **scored**
    /// by — which outcome and target this seat names.
    ///
    /// ⚠ **The Favor stake is a separate gene, and the screen is why.** These
    /// were one gene, and in the lane's own regime
    /// (`--victories diplomatic,score`, 120 pairs) the composite read −0.61 pp
    /// of score share at z −2.33 — a screen flag *against* it, while its
    /// neighbour `congress-banks-decided` read +6.7 pp of wins on the same
    /// games.
    ///
    /// ★★★ **THE SPLIT WAS RIGHT AND THE REASON GIVEN FOR IT WAS WRONG.** The
    /// argument for splitting was that the harm had to be on the staking
    /// side: `congress_affordable_votes` empties a treasury that a **winning**
    /// ballot does not refund, while naming the right outcome costs nothing.
    /// Priced apart (`docs/VICTORY_GENES.md` §8.5, 570 pairs, ±2.8 pp), the
    /// stake is the **positive** half in all four windows (+3.7, +4.2, +2.0,
    /// +1.4) and **this** gene — naming the ballot for the raced lane — is the
    /// one carrying the negative (−1.9, +0.0, −2.0, −1.8). Nothing is
    /// resolved either way, so the ordering is a suggestion; but the
    /// mechanism that motivated the split is not the one the split found.
    ///
    /// What the ordering suggests instead is that a seat still settling has
    /// different interests in a resolution than the diplomat it intends to
    /// become, and voting as that diplomat costs it the Favor refund a losing
    /// ballot would have paid.
    pub(super) fn congress_lane(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> GrandStrategy {
        self.lane_or_plan(self.lane_congress_ballot, g, pid, plan)
    }

    /// `lane-congress-favor`: the lane the Favor **stake** behind a ballot is
    /// decided by. The staking half of what `congress_lane` used to be; see
    /// its doc for the reading that split them.
    pub(super) fn congress_favor_lane(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> GrandStrategy {
        self.lane_or_plan(self.lane_congress_favor, g, pid, plan)
    }

    /// `lane-great-people`: the lane Great Person patronage and the Great
    /// Person points a project earns are ranked by.
    ///
    /// ⚠ **This is the one gene here that overrides a war plan, and the scope
    /// was forced by two fires-checks.** Restricted to `Expansion` it was
    /// inert in both regimes — 0 of 4 targeted games, 0 of 36 native
    /// seat-pairs — for a reason the expansion window makes unavoidable:
    /// patronage needs a bank the opening rarely has, and a district project
    /// needs districts the opening has not built. Everything this decider
    /// ranks exists only after the settling turns are over.
    ///
    /// So this one asks the question at the posture where it is actually
    /// live, and where it is genuinely contestable: a `Conquest` plan ranks
    /// Generals and Admirals at 2.3 and the class the empire's *race* needs at
    /// 0.85. A Great Person is a finite global race and a war does not change
    /// which class wins it — but a war does need Generals, and which of those
    /// two is worth more is exactly what a screen is for. It is deliberately
    /// the cheapest and most reversible decider to test that on: no production
    /// is committed and no card is slotted, only a ranking among people the
    /// empire is competing for anyway. `Recovery` — losing ground at home —
    /// still keeps its own strategy.
    pub(super) fn great_person_lane(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> GrandStrategy {
        if !self.lane_great_people || plan.strategy == GrandStrategy::Recovery {
            return plan.strategy;
        }
        let lane = self.victory_focus(g, pid).strategy;
        if matches!(
            lane,
            GrandStrategy::Science
                | GrandStrategy::Culture
                | GrandStrategy::Religion
                | GrandStrategy::Diplomacy
        ) {
            lane
        } else {
            plan.strategy
        }
    }

    /// `lane-policy-deck`: the lane the policy cards are chosen for.
    pub(super) fn policy_lane(&self, g: &Game, pid: usize, plan: &StrategicPlan) -> GrandStrategy {
        self.lane_or_plan(self.lane_policy_deck, g, pid, plan)
    }

    /// `lane-culture-spending`: the lane the Culture Faith pass and its
    /// reserve read.
    ///
    /// ⚠ **NOT `lane_or_plan`, and the difference is the whole gene.** The
    /// three genes above choose between options the empire is picking anyway,
    /// so filling in an absent lane is enough. This one and
    /// [`AdvancedAi::space_race_lane`] switch a whole pass ON, and the pass
    /// they switch on can only act late: `culture_spending` needs the
    /// `conservation` civic for a National Park site and `cold_war` for a
    /// Rock Band, and the `Expansion` window shuts at
    /// `standard_duration(175)` — turn ~87 at Online. Restricted to
    /// Expansion, this gene was **strictly inert**: 0 of 4
    /// `victory_eval --target culture` games diverged. What is actually
    /// missing is not the settling turns; it is that an ADAPTIVE seat holds
    /// the Culture plan for 5.0% of its turns, so the Culture lane's only
    /// Faith purchases are all but unreachable without an assigned target.
    ///
    /// `Recovery` still refuses: an empire losing ground at home has better
    /// uses for its Faith than a Rock Band, and `military_faith_spending`
    /// runs after this.
    pub(super) fn culture_lane_spends(&self, g: &Game, pid: usize, plan: &StrategicPlan) -> bool {
        self.lane_culture_spending
            && plan.strategy != GrandStrategy::Recovery
            && self.victory_focus(g, pid).strategy == GrandStrategy::Culture
    }

    /// The lane the Culture Faith reserve is sized for: `Culture` when this
    /// empire is racing it, the plan's own strategy otherwise.
    pub(super) fn culture_faith_lane(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> GrandStrategy {
        if self.culture_lane_spends(g, pid, plan) {
            GrandStrategy::Culture
        } else {
            plan.strategy
        }
    }

    /// `lane-space-race`: whether this empire is racing Science, for the
    /// space race's purposes.
    ///
    /// Every gate in `science_production` asks for an **explicitly assigned**
    /// `VictoryTarget::Science` — the pad count (1 rather than the 3 the
    /// parallel laser race needs), the city a launch project may claim, and
    /// the city a pad may be sited in. The agent production ships has no
    /// assigned target at all, so it races the space race at one pad and only
    /// in cities with nothing else queued. With this on, a seat whose
    /// `victory_focus` is Science is treated as a Science seat by all three,
    /// and the pass itself opens for it — `score_horizon` still refuses a race
    /// that cannot finish, and `Recovery` still refuses outright.
    pub(super) fn space_race_lane(&self, g: &Game, pid: usize) -> bool {
        self.lane_space_race && self.victory_focus(g, pid).strategy == GrandStrategy::Science
    }

    /// The dispatcher's half of `lane-space-race`: run the pass for a Science
    /// racer whose plan has not named the lane, short of a Recovery posture.
    pub(super) fn space_race_lane_opens(&self, g: &Game, pid: usize, plan: &StrategicPlan) -> bool {
        plan.strategy != GrandStrategy::Recovery && self.space_race_lane(g, pid)
    }

    /// `competition-victory-points`: what the Diplomatic Victory Points a
    /// scored competition's first place pays are worth to this seat, on top
    /// of the competition score `host_competition_score_value` already
    /// prices.
    ///
    /// Zero unless the gene is on, the raced lane is Diplomacy, the
    /// competition actually pays points, and this completion would put the
    /// seat at or in front of the current leader — points that go to
    /// somebody else are not this project's to claim.
    pub(super) fn competition_victory_point_value(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
        kind: &str,
        score_gain: f64,
    ) -> f64 {
        // ⚠ ORDERED CHEAPEST FIRST, BECAUSE THIS RUNS INSIDE
        // `production_value` — once per legal item per city per turn, which
        // the profile puts at ~95% of the main thread. The flag, a table
        // lookup and the competition's own record all answer without touching
        // the board; `raced_lane` walks every lane's progress and is asked
        // last, only for a project that is actually a scored competition's
        // and that this seat could actually take first place in.
        if !self.competition_victory_points {
            return 0.0;
        }
        let points = Game::competition_victory_points(kind);
        if points == 0 {
            return 0.0;
        }
        let Some(competition) = g.host_competition(pid, kind) else {
            return 0.0;
        };
        if competition.ours + score_gain + f64::EPSILON < competition.leader {
            return 0.0;
        }
        // ⚠ NOT `raced_lane`, which requires an `Expansion` plan: every
        // scored competition is gated on world era 5 to 8
        // (`Game::NATIVE_COMPETITIONS`), and a targeted seat's expansion
        // window shuts at `standard_duration(175)` — turn ~87 at Online. Under
        // that restriction this priced nothing at all: 0 of 4
        // `victory_eval --target diplomatic` games diverged. Pricing a
        // currency correctly is not a posture override, so the test is simply
        // whether this empire is racing Diplomacy. The plan answers for free
        // in the common case; `victory_focus` is asked only when it does not.
        if plan.strategy != GrandStrategy::Diplomacy
            && self.victory_focus(g, pid).strategy != GrandStrategy::Diplomacy
        {
            return 0.0;
        }
        points as f64 * victory_point_value()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::advanced::VictoryTarget;
    use crate::game::{Game, GameOptions};
    use std::collections::BTreeMap;

    fn game() -> Game {
        Game::new_with(GameOptions {
            barbarians: false,
            speed: "online".to_string(),
            ..GameOptions::new(4, 40, 26, 77_000, 250, 4)
        })
    }

    fn expansion_plan() -> StrategicPlan {
        StrategicPlan {
            strategy: GrandStrategy::Expansion,
            target_player: None,
            target_city: None,
            threatened_city: None,
            desired_cities: 6,
            assessed_turn: 0,
            rush: false,
        }
    }

    /// Every one of the six is off in production and comes back off.
    #[test]
    fn the_seven_victory_lane_genes_are_native_opt_ins() {
        let mut ai = AdvancedAi::new();
        assert!(!ai.lane_congress_ballot);
        assert!(!ai.lane_congress_favor);
        assert!(!ai.lane_great_people);
        assert!(!ai.lane_policy_deck);
        assert!(!ai.lane_culture_spending);
        assert!(!ai.lane_space_race);
        assert!(!ai.competition_victory_points);

        ai.enable_lane_congress_ballot();
        ai.enable_lane_congress_favor();
        ai.enable_lane_great_people();
        ai.enable_lane_policy_deck();
        ai.enable_lane_culture_spending();
        ai.enable_lane_space_race();
        ai.enable_competition_victory_points();
        assert!(ai.lane_congress_ballot);
        assert!(ai.lane_congress_favor);
        assert!(ai.lane_great_people);
        assert!(ai.lane_policy_deck);
        assert!(ai.lane_culture_spending);
        assert!(ai.lane_space_race);
        assert!(ai.competition_victory_points);

        ai.disable_lane_congress_ballot();
        ai.disable_lane_congress_favor();
        ai.disable_lane_great_people();
        ai.disable_lane_policy_deck();
        ai.disable_lane_culture_spending();
        ai.disable_lane_space_race();
        ai.disable_competition_victory_points();
        assert!(!ai.lane_congress_ballot);
        assert!(!ai.lane_congress_favor);
        assert!(!ai.lane_great_people);
        assert!(!ai.lane_policy_deck);
        assert!(!ai.lane_culture_spending);
        assert!(!ai.lane_space_race);
        assert!(!ai.competition_victory_points);
    }

    /// The two halves of what used to be one ballot gene answer separately.
    #[test]
    fn the_ballot_score_and_the_favor_stake_are_two_genes() {
        let g = game();
        let plan = expansion_plan();
        let mut scoring = AdvancedAi::targeting(VictoryTarget::Diplomacy);
        scoring.enable_lane_congress_ballot();
        assert_eq!(
            scoring.congress_lane(&g, 0, &plan),
            GrandStrategy::Diplomacy,
            "the ballot is scored for the raced lane"
        );
        assert_eq!(
            scoring.congress_favor_lane(&g, 0, &plan),
            GrandStrategy::Expansion,
            "and the treasury is not staked on it"
        );

        let mut staking = AdvancedAi::targeting(VictoryTarget::Diplomacy);
        staking.enable_lane_congress_favor();
        assert_eq!(
            staking.congress_lane(&g, 0, &plan),
            GrandStrategy::Expansion
        );
        assert_eq!(
            staking.congress_favor_lane(&g, 0, &plan),
            GrandStrategy::Diplomacy
        );
    }

    /// The premise, stated as a test: a seat told to play for Culture and
    /// still settling hands every lane decider `Expansion`, and the raced
    /// lane is `Culture` the whole time.
    #[test]
    fn an_assigned_lane_is_invisible_to_the_deciders_while_the_empire_settles() {
        let g = game();
        let ai = AdvancedAi::targeting(VictoryTarget::Culture);
        let plan = expansion_plan();
        assert_eq!(
            ai.raced_lane(&g, 0, &plan),
            Some(GrandStrategy::Culture),
            "the assigned lane is what the empire is racing whatever the plan says"
        );
        assert_eq!(
            ai.congress_lane(&g, 0, &plan),
            GrandStrategy::Expansion,
            "off, the ballot still reads the expansion posture"
        );
        let mut armed = AdvancedAi::targeting(VictoryTarget::Culture);
        armed.enable_lane_congress_ballot();
        armed.enable_lane_great_people();
        armed.enable_lane_policy_deck();
        armed.enable_lane_culture_spending();
        assert_eq!(armed.congress_lane(&g, 0, &plan), GrandStrategy::Culture);
        assert_eq!(
            armed.great_person_lane(&g, 0, &plan),
            GrandStrategy::Culture
        );
        assert_eq!(armed.policy_lane(&g, 0, &plan), GrandStrategy::Culture);
        assert_eq!(
            armed.culture_faith_lane(&g, 0, &plan),
            GrandStrategy::Culture
        );
    }

    /// A war posture is a decision, not a gap: the three genes that choose
    /// between options the empire is picking anyway leave it alone.
    #[test]
    fn a_war_plan_keeps_its_own_strategy_under_the_three_choice_genes() {
        let g = game();
        let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
        ai.enable_lane_congress_ballot();
        ai.enable_lane_policy_deck();
        ai.enable_lane_culture_spending();
        for strategy in [GrandStrategy::Conquest, GrandStrategy::Recovery] {
            let plan = StrategicPlan {
                strategy,
                ..expansion_plan()
            };
            assert_eq!(ai.raced_lane(&g, 0, &plan), None, "{strategy:?}");
            assert_eq!(ai.congress_lane(&g, 0, &plan), strategy);
            assert_eq!(ai.policy_lane(&g, 0, &plan), strategy);
        }
    }

    /// `lane-great-people` is the exception, and says so: it overrides a
    /// Conquest plan and stops at Recovery.
    #[test]
    fn the_great_person_gene_outlives_a_war_but_not_a_rout() {
        let g = game();
        let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
        ai.enable_lane_great_people();
        let war = StrategicPlan {
            strategy: GrandStrategy::Conquest,
            ..expansion_plan()
        };
        assert_eq!(ai.great_person_lane(&g, 0, &war), GrandStrategy::Science);
        let rout = StrategicPlan {
            strategy: GrandStrategy::Recovery,
            ..expansion_plan()
        };
        assert_eq!(ai.great_person_lane(&g, 0, &rout), GrandStrategy::Recovery);
        let mut off = AdvancedAi::targeting(VictoryTarget::Science);
        off.disable_lane_great_people();
        assert_eq!(off.great_person_lane(&g, 0, &war), GrandStrategy::Conquest);
    }

    /// The two capability genes switch a whole pass on, and the pass can only
    /// act in an era the expansion window never reaches — so they are scoped
    /// to the race rather than to the posture. `Recovery` still refuses.
    #[test]
    fn the_two_capability_genes_follow_the_race_and_stop_at_recovery() {
        let g = game();
        let mut science = AdvancedAi::targeting(VictoryTarget::Science);
        science.enable_lane_space_race();
        let mut culture = AdvancedAi::targeting(VictoryTarget::Culture);
        culture.enable_lane_culture_spending();
        for strategy in [
            GrandStrategy::Expansion,
            GrandStrategy::Conquest,
            GrandStrategy::Science,
        ] {
            let plan = StrategicPlan {
                strategy,
                ..expansion_plan()
            };
            assert!(
                science.space_race_lane_opens(&g, 0, &plan),
                "the space race follows a Science racer under {strategy:?}"
            );
            assert!(
                culture.culture_lane_spends(&g, 0, &plan),
                "the Culture Faith pass follows a Culture racer under {strategy:?}"
            );
        }
        let losing = StrategicPlan {
            strategy: GrandStrategy::Recovery,
            ..expansion_plan()
        };
        assert!(
            !science.space_race_lane_opens(&g, 0, &losing),
            "an empire losing ground at home does not start a Spaceport"
        );
        assert!(
            !culture.culture_lane_spends(&g, 0, &losing),
            "nor buy a Rock Band"
        );
    }

    /// A plan that already carries its own victory lane is left alone: the
    /// genes fill in an absent lane, they do not re-aim a present one.
    #[test]
    fn a_plan_that_already_names_a_lane_is_unchanged() {
        let g = game();
        let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
        ai.enable_lane_congress_ballot();
        let plan = StrategicPlan {
            strategy: GrandStrategy::Religion,
            ..expansion_plan()
        };
        assert_eq!(ai.raced_lane(&g, 0, &plan), None);
        assert_eq!(ai.congress_lane(&g, 0, &plan), GrandStrategy::Religion);
    }

    /// A seat racing the score tally or a conquest has no economic lane to
    /// substitute, so `Expansion` stands.
    #[test]
    fn a_seat_racing_the_tally_keeps_the_expansion_posture() {
        let g = game();
        let mut ai = AdvancedAi::targeting(VictoryTarget::Score);
        ai.enable_lane_congress_ballot();
        let plan = expansion_plan();
        assert_eq!(ai.raced_lane(&g, 0, &plan), None);
        assert_eq!(ai.congress_lane(&g, 0, &plan), GrandStrategy::Expansion);
    }

    /// The space race gene opens the pass for a Science racer and for nobody
    /// else, and stays shut until it is armed.
    #[test]
    fn the_space_race_pass_opens_only_for_a_science_racer() {
        let g = game();
        let plan = expansion_plan();
        let mut science = AdvancedAi::targeting(VictoryTarget::Science);
        assert!(
            !science.space_race_lane_opens(&g, 0, &plan),
            "off by default"
        );
        assert!(!science.space_race_lane(&g, 0), "off by default");
        science.enable_lane_space_race();
        assert!(science.space_race_lane_opens(&g, 0, &plan));
        assert!(
            science.space_race_lane(&g, 0),
            "and the pad count, project city and pad site read the same answer"
        );

        let mut culture = AdvancedAi::targeting(VictoryTarget::Culture);
        culture.enable_lane_space_race();
        assert!(!culture.space_race_lane_opens(&g, 0, &plan));
        assert!(!culture.space_race_lane(&g, 0));
    }

    /// One point of the twenty is priced the same whether a wonder or a
    /// competition pays it.
    #[test]
    fn a_congress_point_is_worth_what_a_wonder_point_is_worth() {
        assert_eq!(
            victory_point_value(),
            super::super::STRATEGIC_WONDER_VICTORY_VALUE
                / crate::game::DIPLOMATIC_VICTORY_POINTS as f64
        );
        assert!(
            victory_point_value() > 0.0,
            "a point of the win is worth something"
        );
    }

    /// The competition table is read from the engine's own specs, so a seat
    /// cannot be told a competition pays points it does not pay.
    #[test]
    fn the_competition_points_come_from_the_engines_own_table() {
        assert_eq!(
            Game::competition_victory_points("EMERGENCY_CLIMATE_ACCORDS"),
            2
        );
        assert_eq!(Game::competition_victory_points("EMERGENCY_WORLD_GAMES"), 1);
        assert_eq!(Game::competition_victory_points("EMERGENCY_SEND_AID"), 2);
        assert_eq!(Game::competition_victory_points("not_a_competition"), 0);
    }

    /// ★★★ THE GENE IS INERT IN A DEFAULT NATIVE GAME, AND THAT IS A
    /// PROPERTY OF THE RULES, NOT OF THE GENE.
    ///
    /// `Game::native_competitions` ships **off** — its own doc says so and
    /// why: turning it on changes what every participant faces and moves the
    /// frozen rating anchor, so it is an arm to be priced (`--native-competitions`,
    /// `docs/ELO_REPINS.md`), not a silent rules change. With it off,
    /// `open_native_competition` returns immediately and no scored competition
    /// is ever seated, so `gene_screen`'s native games cannot price this gene
    /// — it reads exactly +0.0 there, which is what an unreachable branch
    /// reads. The two regimes it IS live in are the `--native-competitions`
    /// arm and the live Civilization VI bridge, whose mirror supplies real
    /// Gathering Storm competitions in `Game::host_competitions`.
    ///
    /// This test seats one the way both of those do, so the branch is proved
    /// rather than assumed.
    #[test]
    fn an_open_competition_pays_its_points_to_a_diplomacy_racer() {
        let mut g = game();
        g.native_competitions = true;
        g.competition = Some(crate::game::Competition {
            kind: "EMERGENCY_CLIMATE_ACCORDS".to_string(),
            ends: g.turn + 30,
            target: None,
            scores: BTreeMap::new(),
        });
        let plan = StrategicPlan {
            strategy: GrandStrategy::Diplomacy,
            ..expansion_plan()
        };
        let mut off = AdvancedAi::targeting(VictoryTarget::Diplomacy);
        assert_eq!(
            off.competition_victory_point_value(&g, 0, &plan, "EMERGENCY_CLIMATE_ACCORDS", 100.0),
            0.0,
            "off by default"
        );
        off.enable_competition_victory_points();
        assert_eq!(
            off.competition_victory_point_value(&g, 0, &plan, "EMERGENCY_CLIMATE_ACCORDS", 100.0),
            2.0 * victory_point_value(),
            "the Climate Accords pay two of the twenty"
        );

        // And not to a seat that could not take first place with it.
        g.competition.as_mut().unwrap().scores.insert(1, 500.0);
        assert_eq!(
            off.competition_victory_point_value(&g, 0, &plan, "EMERGENCY_CLIMATE_ACCORDS", 100.0),
            0.0,
            "a completion that still leaves the leader ahead claims no points"
        );
    }

    /// No competition is open in a fresh game, so nothing is priced — the
    /// gene cannot pay for points that are not on offer.
    #[test]
    fn no_open_competition_pays_nothing() {
        let g = game();
        let mut ai = AdvancedAi::targeting(VictoryTarget::Diplomacy);
        ai.enable_competition_victory_points();
        let plan = expansion_plan();
        assert_eq!(
            ai.competition_victory_point_value(&g, 0, &plan, "EMERGENCY_CLIMATE_ACCORDS", 100.0),
            0.0
        );
    }

    /// And a lane that is not Diplomacy never claims them, whatever is open.
    #[test]
    fn only_the_diplomacy_lane_prices_the_points() {
        let g = game();
        let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
        ai.enable_competition_victory_points();
        let plan = expansion_plan();
        assert_eq!(
            ai.competition_victory_point_value(&g, 0, &plan, "EMERGENCY_SEND_AID", 200.0),
            0.0
        );
    }
}
