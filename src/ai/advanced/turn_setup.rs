//! Unchanged strategic setup for an AI turn, kept separate from turn execution.

use super::*;

pub(super) struct TurnSetup {
    pub(super) active_victory_target: Option<VictoryTarget>,
    pub(super) specialization_active: bool,
    pub(super) lane_delegating: bool,
    pub(super) plan: StrategicPlan,
}

impl AdvancedAi {
    #[inline(never)]
    pub(super) fn prepare_turn(&mut self, g: &mut Game, pid: usize) -> Option<TurnSetup> {
        // See `last_civ_standing`: "one more turn" leaves one lane.
        self.plays_for_the_last_civ(g, pid);
        // `culture-keeps-pingala-home`: both Loyalty passes read the runway.
        // See `advanced/culture_pingala_first.rs`.
        self.base.pingala_loyalty_runway =
            self.culture_pingala_stays_home(g).map(|(_, runway)| runway);
        // `districts-price-the-improvement`: what a District could bulldoze.
        self.snapshot_bulldoze_sites(g, pid);
        // `breach-support-reads-the-wall-tier`: see `Game::observed_wall_tier_rules`.
        g.observed_wall_tier_rules = self.breach_support_reads_the_wall_tier;
        // `builders-route-around-mountains`: see
        // `Game::observed_builders_avoid_mountains`.
        g.observed_builders_avoid_mountains = self.builders_route_around_mountains;
        // `trade-quotes-pay-the-lump`: see `Game::lump_quote_viewer`.
        g.lump_quote_viewer = self.trade_quotes_pay_the_lump.then_some(pid);
        // The settings above are written on the board without an action.
        // See `Game::keep_rival_yields`.
        g.board_written();
        // `nusantara-reads-the-coast`: see `Game::nusantara_site_planner`.
        self.seat_nusantara_site_planner(g, pid);
        // See `defeated_majors` (`defeated-majors-leave-the-board`): before
        // any living-majors reading of this turn.
        self.retire_defeated_majors(g, pid);
        self.turn_start_policies = g.players[pid].policies.clone();
        // See `founder_faith` (`founder-spreads-only-its-faith`).
        self.record_turn_start_faith(g, pid);
        self.air_city_assault = None;
        self.base.gun_lethal_tiles.clear();
        self.base.unit_lethal_blows.clear();
        self.clear_priced_risk();
        self.builder_support.clear();
        self.battlefront_frame = None;
        self.settlement_atlas.borrow_mut().clear();
        self.lane_progress_cache.0.borrow_mut().clear();
        // Before anything in this turn is priced. Every science term downstream
        // reads this one number, so the horizon cannot drift between the
        // production ordering, the citizen governor, and the search evaluator.
        self.refresh_research_weight(g);
        // `denial-spies-count-their-losses`: the base production fallback
        // reads this. See `advanced/denial_spies_count_their_losses.rs`.
        self.base.spy_training_paused = self.spy_losses_pause_training(g);
        // `parity-upgrades-before-it-buys-2`: the base defender purchases
        // read this. See `advanced/parity_upgrades_before_it_buys.rs`.
        self.base.obsolete_military_buys_barred = self.obsolete_military_buys_barred();
        self.base.obsolete_buys_read_our_techs = self.obsolete_buys_read_our_techs();
        // `plaza-serves-the-scientist` (and version 2, which carries it): the
        // Plaza's tier list reads this.
        self.base.plaza_serves_science =
            self.plaza_serves_science_on() || self.native_culture_plaza_economy(g, pid);
        // `score-seat-expands-hard`: the Ancestral Hall, then the Chapel. See
        // `advanced/score_expands_hard.rs`.
        self.base.plaza_hall_first = self.score_expands_hard();
        self.refresh_campus_multiplier_constants(g);
        self.resolve_vanished_settlers(g, pid);
        self.base.minor = g.players[pid].is_minor;
        self.base.barb = g.players[pid].is_barbarian;
        self.maintain_victory_portfolio(g, pid);
        // `religion-lane-hands-over`: a Religion seat that lost the Prophet
        // race takes Science before the turn reads its target. See
        // `advanced/religion_lane_hands_over.rs`.
        self.hand_over_a_lost_religion_lane(g, pid);
        // `religion-seat-takes-the-faith-beliefs`: read by the founding in
        // `BasicAi`'s ancillary pass, after any hand-over above.
        self.base.founder_takes_the_faith_beliefs = self.founder_takes_the_faith_beliefs(g);
        // `religion-seat-charts-the-rivals`: read by the contact gap in
        // `BasicAi`'s exploration and the hull claims, after any hand-over.
        self.base.charts_rival_majorities = self.religion_seat_charts_the_rivals_active(g, pid);
        // `domination-charts-every-capital`: the same read for a lane charting
        // gene's own gap.
        self.base.lane_charting_gap = self.lane_charts_the_water(g, pid);
        // `stalled-conquest-takes-science`: see
        // `advanced/stalled_conquest_takes_science.rs`.
        self.hand_over_a_stalled_conquest(g, pid);
        let active_victory_target = self.active_victory_target(g);
        let specialization_active = self.phase_specialization_active(g);
        // See `lane_delegates_production`: this turn an assigned lane's
        // cities may take the unassigned seat's dispatch below.
        let lane_delegating = self.lane_delegates_now(active_victory_target, specialization_active);
        // See `skip_the_prophet_race_2`: an adaptive seat pursues a religion
        // unconditionally, and in this regime that trade is measured negative;
        // v2 leaves the race at the last call.
        // See `religion_race_is_closed`: the world's religions are all
        // founded and none of them is ours, so no prophet this empire ever
        // recruits can found one. Kongo is exempt -- Taxis pays it for
        // everyone else's religion, so its faith economy is unconditional.
        let religion_race_closed = self.religion_race_closed_for(g, pid);
        let skip_prophet_race = self.skip_prophet_race_2_for(g, pid, active_victory_target);
        self.base.pursue_religion = (g.has_ability(pid, "taxis")
            || active_victory_target == Some(VictoryTarget::Religion)
            || (active_victory_target.is_none() && !skip_prophet_race))
            && !religion_race_closed;
        // The prize and the entry fee are two different gates. Clearing
        // `pursue_religion` alone discards the winnings while still paying for
        // the Holy Site that contests the race, so carry the flag down to the
        // reservation as well — but never against a seat whose own lane is
        // Religion, and never for Kongo, whose Taxis ability makes the faith
        // economy unconditional.
        // See `diplomatic_lane_refuses_fossil_power`: the baseline governor,
        // which fills a delegating lane's routine queues, refuses the plants
        // too.
        self.base.exclude_fossil_power =
            self.diplomatic_lane_refuses_fossil_power("coal_power_plant");
        self.base.skip_prophet_race = (skip_prophet_race || religion_race_closed)
            && !g.has_ability(pid, "taxis")
            && active_victory_target != Some(VictoryTarget::Religion);
        // `enter-the-prophet-race-2`: the entry fee AND the prize, together,
        // while a slot is still open for this seat — and the prize again once
        // it holds a religion. The closed case above still wins, because
        // `prophet_race_open_for` is false by then.
        // `found-against-a-rival-faith` keeps the prize once its race won a
        // religion. See `advanced/faith_veto.rs`.
        let prophet_race_enabled = self.prophet_race_enabled_for(active_victory_target)
            || (self.found_against_a_rival_faith
                && active_victory_target == Some(VictoryTarget::Domination));
        let prophet_race_open = self.prophet_race_enterable_for(g, pid, active_victory_target);
        self.base.enter_prophet_race = prophet_race_open;
        // `prophet-race-takes-divine-spark`: see
        // `advanced/astrology_before_the_second_city.rs`.
        self.base.pantheon_for_the_prophet_race =
            self.prophet_race_takes_divine_spark && self.race_points_wanted(g, pid);
        if prophet_race_open || (prophet_race_enabled && g.players[pid].religion.is_some()) {
            self.base.pursue_religion = true;
        }
        // An explicit Science target is a production contract, not only a
        // research tie-break. The live genome may withhold the opt-in
        // `science-building-first` treatment, but a targeted seat must still
        // finish the Campus chain before spending its cities on unrelated
        // buildings. Keep the opt-in available for adaptive seats while
        // making the named Science lane self-consistent.
        self.base.science_building_first = self.science_building_first
            || (specialization_active && active_victory_target == Some(VictoryTarget::Science));
        // A targeted Science seat also has to get through the Advanced
        // governor's reservation path. `science-building-first` only
        // partitions BasicAi's final building list; districts are ranked
        // before that list, so an owed Library would otherwise still lose to
        // a Theater or another unrelated district. Keep the gene opt-in for
        // adaptive seats while making the named lane self-consistent.
        self.first_research_building_reserve = self.first_research_building_reserve
            || (specialization_active && active_victory_target == Some(VictoryTarget::Science));
        // One reading a turn: `Game::victory_races` walks every city of every
        // major, and `production_value` asks this question per item per city.
        self.lane_lost = self.assigned_lane_is_lost(g, pid);
        if self.base.minor || self.base.barb {
            self.base.take_turn(g, pid);
            return None;
        }
        // `city-strike-takes-the-gun`: the delegated city loop leaves the
        // strikes to `advanced_city_strikes`, which runs only under victory
        // planning. See `advanced/siege_at_the_walls_dies_first.rs`.
        self.base.city_strikes_left_to_the_planner =
            self.city_strike_takes_the_gun && self.victory_planning;
        // The fallback record for `turn_start_hostiles`: nothing of ours has
        // acted yet on a native turn. The live bridge records earlier, before
        // its finishing volley, and this call then does nothing.
        self.observe_turn_start_hostiles(g, pid);
        self.capture_battlefront_frame(g, pid);
        // Campaign and battlefront planning share this fog-safe history. A
        // refresh sees only the acting seat's current vision, so a hidden
        // heal, garrison move, or ownership change cannot rewrite a plan.
        // `advanced_v1` keeps its original opt-in pressure observation only.
        if self.battlefront_observation || self.belief_pressure {
            self.belief.observe(g, pid);
        }
        let rush_routes_frozen = self.freeze_rush_route_targets(g, pid);
        // `science_victory_drive`: read the field and decide whether the
        // seat drives the space race this turn. Exact no-op while off. See
        // `advanced/science_victory_drive.rs`.
        self.maintain_science_drive(g, pid);
        // `growth_to_settle`: hand the citizen governor this turn's food
        // appetite. Exact no-op while off. See
        // `advanced/growth_to_settle.rs`.
        self.maintain_growth_to_settle(g, pid);
        let disposition_strategy = active_victory_target
            .map(VictoryTarget::strategy)
            .unwrap_or_else(|| self.victory_focus(g, pid).strategy);
        self.resolve_city_dispositions(g, pid, disposition_strategy);
        self.observe_campaign(g, pid);
        self.observe_victory_conversion(g, pid);
        // `early-conquest-opening`: name or keep the opening's target, count
        // the war's losses, and pin the campaign it has handed over — before
        // the shipped campaign maintenance reads the plan, and before
        // `assess` aims the army. Exact no-op while off. See
        // `advanced/early_conquest.rs`.
        self.maintain_conquest_opening(g, pid);
        // `city-state-harvest`: name, keep or release the city-state city
        // the plan conquers next. Exact no-op while off. See
        // `advanced/city_state_harvest.rs`.
        self.maintain_city_state_harvest(g, pid);
        // `city_campaign`: drop the cities the plan has taken, expire one
        // never launched, draw or refresh one at peace. See
        // `advanced/city_campaign.rs`.
        self.maintain_city_campaign(g, pid);
        // One stock-pressure sample per rival per turn, before anything reads
        // urgency this turn. See `projected_stock_denial`.
        self.record_stock_pressures(g, pid);
        // See `capital_prey_beside_the_front`: the prey and its near misses,
        // once a turn, at war or at peace.
        self.journal_capital_prey(g, pid);
        // See `journal_held_second_front`: a named counter the second front
        // holds, and the gates that hold it. Journal only.
        self.journal_held_second_front(g, pid);
        // See `journal_silent_counter`: an urgent racer whose Conquest counter
        // cannot be executed says why. Journal only.
        self.journal_silent_counter(g, pid);
        // See `denial_keeps_its_rival`: this turn's counter rival, chosen
        // against last turn's, is the incumbent for the next.
        if self.denial_keeps_its_rival {
            self.denial_incumbent = self
                .actionable_victory_denial(g, pid)
                .map(|(rival, _)| rival);
        }
        // `denial-nearest-finish`: one culture-curve reading per rival.
        self.record_culture_curves(g, pid);
        // See `engine_culture_clock`: one reading per rival per turn.
        self.record_engine_culture_clock(g, pid);
        self.maintain_war_plan(g, pid);
        // The air surge's own lifecycle, after the melee appointment so the
        // two can never both own the grand strategy in the same turn: the
        // surge overlay below runs second and only from its Strike phase.
        // Exact no-op while `air_surge` is off. See `advanced/air_surge.rs`.
        self.maintain_air_surge(g, pid);
        if rush_routes_frozen || self.plan_stale(g, pid) {
            let mut next = self.assess(g, pid);
            self.apply_war_plan_to_strategy(&mut next);
            self.apply_city_state_harvest_to_strategy(g, pid, &mut next);
            // `city-state-before-the-first-capital`: no-op with the gene off.
            self.apply_city_state_stone_to_strategy(g, pid, &mut next);
            self.apply_air_surge_to_strategy(g, pid, &mut next);
            self.plan = Some(next);
        } else if let Some(mut current) = self.plan.take() {
            self.apply_war_plan_to_strategy(&mut current);
            self.apply_city_state_harvest_to_strategy(g, pid, &mut current);
            self.apply_city_state_stone_to_strategy(g, pid, &mut current);
            self.apply_air_surge_to_strategy(g, pid, &mut current);
            self.plan = Some(current);
        }
        // Read the current physical package after both persistent overlays,
        // including turns on which the ordinary assessment stays current.
        if self.elimination_yields_to_a_launch_2 {
            if let Some(mut current) = self.plan.take() {
                self.apply_culture_air_interception_to_strategy(g, pid, &mut current);
                self.plan = Some(current);
            }
        }
        let plan = self.plan.clone().unwrap();
        Some(TurnSetup {
            active_victory_target,
            specialization_active,
            lane_delegating,
            plan,
        })
    }
}
