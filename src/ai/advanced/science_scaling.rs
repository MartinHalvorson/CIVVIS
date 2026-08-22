//! Two late-game science genes, and the tests that pin them.
//!
//! Both are opt-in (`PRODUCTION_OPT_INS`), ship off, and are priced by
//! `gene_screen` before any promotion question is asked — see
//! `docs/GENE_SCREEN.md`. They live here because they are one sentence each
//! and `src/ai/advanced.rs` is the most contended file in the repository
//! (`tools/conflict_hotspots.py` measures it at 23% of the last 200 merges).
//!
//! **The shared observation.** Science is the only economy in this controller
//! that is priced to matter LESS the longer the game runs. Three terms —
//! `RESEARCH_CAMPUS_COVERAGE`, `RESEARCH_BUILDING_DEBT` and
//! `RESEARCH_CITIZEN_TILT` — are multiplied by `research_horizon`, which is
//! `(max_turns - turn) / max_turns` and reaches zero at the turn limit, while
//! the terms they compete against are flat constants: a Theater Square's
//! **850** under Culture, the Government Plaza's first copy **420**, a
//! Diplomatic Quarter's **360**, the Monument's **+240** while `turn < 120`,
//! Ancient Walls' **+320** under threat. So a Campus at turn 150 of 250 bids
//! 40% of its price into a table that has not moved, and the empire's research
//! economy thins exactly where the tech tree stops being a convenience and
//! starts being Field Cannon against Spearmen.
//!
//! **`science-payback-horizon`.** Asks the question the investment actually
//! poses. A Campus is 54 production and pays a yield every turn afterwards, so
//! what decides it is whether there is still time to REPAY, not what fraction
//! of the game is left; `campus_payback_horizon` says exactly that, holds full
//! value until the last `RESEARCH_CAMPUS_PAYBACK` of the budget, and is already
//! what `DISTRICT_BUILDING_CHAIN_DEBT` and `CULTURE_THEATER_COVERAGE` are
//! scaled by. This gene puts the two remaining science terms on it.
//!
//! ⚠ The Campus coverage term's own comment has claimed "**A PAYBACK horizon,
//! not a game-fraction one**" since #1095 — the branch that made it true
//! belonged to `campus_every_city`, and #2235 removed that gene with the
//! bottom of the ranking, reverting the line under the comment without
//! touching the comment. `campus-every-city` was culled on a **war-regime**
//! screen (`--victories domination,score`, four players, 299 of 300 games
//! decided by the score tally at the clock), which is the one regime a
//! research treatment cannot be priced in. This is that repair measured on its
//! own, in the native six-player regime, under its own name — and only that
//! half: the culled gene's other sentence, a Campus in *every* city whatever
//! the coverage, is deliberately not here.
//!
//! **`science-multiplier-payoff`.** A building's whole worth in
//! `production_value` is `yield_value(spec.yields) * 42` off the **printed**
//! yield — Library 2, University 4, Research Lab 3. `rationalism` is
//! `campus_building_science_pct: 100`, and `strategic_policies` already slots
//! it the moment the empire owns a Campus, so the second Library is routinely
//! bought at the price of the first while earning twice as much; the Research
//! Lab's `powered_science` **5**, larger than its own printed yield, is never
//! counted at all. And the card is paid in halves that arrive LATE and
//! SEPARATELY — half at 15 Population, half where the Campus already earns 4
//! Science from its own adjacency — so a Campus building's true price RISES
//! through the game precisely where `research_horizon` sends it to zero. The
//! funnel measured over 19 live runs has the shape that predicts: 50% Campus,
//! 39% Library, **20% University, 3% Research Lab**, thinning hardest at the
//! tiers whose multiplied yield is largest.
//!
//! The two are separate genes because they are separate claims. The first says
//! the taper is the wrong shape; the second says the number being tapered was
//! measured off the wrong yield. Either can be true without the other, and the
//! foldover screen prices both from the same games.

#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::ai::advanced::PRODUCTION_OPT_INS;
    use crate::Pos;

    fn found_capital(game: &mut Game, pid: usize) -> u32 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        let city = game.found_city_for(pid, game.units[&settler].pos, None);
        game.remove_unit(settler);
        city
    }

    fn set_district(game: &mut Game, city: u32, position: Pos, district: &str) {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.district = Some(Name::new(district));
        tile.improvement = None;
        tile.pillaged = false;
        game.cities
            .get_mut(&city)
            .unwrap()
            .districts
            .insert(Name::new(district), position);
    }

    #[test]
    fn both_science_genes_are_native_opt_ins_that_ship_off() {
        for (tag, read) in [
            (
                "science-payback-horizon",
                (|ai: &AdvancedAi| ai.science_payback_horizon) as fn(&AdvancedAi) -> bool,
            ),
            ("science-multiplier-payoff", |ai: &AdvancedAi| {
                ai.science_multiplier_payoff
            }),
        ] {
            let mut ai = AdvancedAi::new();
            ai.enable_live_bridge_universe();
            assert!(!read(&ai), "{tag} ships off even under the live bridge");
            let (_, _, enable) = PRODUCTION_OPT_INS
                .iter()
                .find(|(_, row, _)| *row == tag)
                .unwrap_or_else(|| panic!("{tag} has an opt-in row"));
            enable(&mut ai);
            assert!(read(&ai), "{tag} turns on");
        }
        let mut ai = AdvancedAi::new();
        ai.enable_science_payback_horizon();
        ai.enable_science_multiplier_payoff();
        ai.disable_science_payback_horizon();
        ai.disable_science_multiplier_payoff();
        assert!(!ai.science_payback_horizon);
        assert!(!ai.science_multiplier_payoff);
    }

    /// The gene's whole sentence: full value while the investment can still
    /// repay, and the taper only inside the payback window.
    #[test]
    fn the_payback_horizon_holds_where_the_game_fraction_has_already_halved() {
        let mut g = Game::new_full(2, 28, 18, 91_779, 250, 0, false);
        let off = AdvancedAi::new();
        let mut on = AdvancedAi::new();
        on.enable_science_payback_horizon();

        // Turn 1: both arms pay essentially the whole price, so the gene
        // cannot be a disguised early-game buff.
        g.turn = 1;
        assert!(off.research_payback(&g) > 0.99);
        assert!(on.research_payback(&g) > 0.99);

        // Turn 150 of 250 — a hundred turns of compounding left, and the
        // shipped horizon has already written off 60% of the price.
        g.turn = 150;
        let shipped = off.research_payback(&g);
        let treated = on.research_payback(&g);
        assert!(
            (shipped - 0.4).abs() < 1e-9,
            "the game-fraction horizon at t150/250: {shipped}"
        );
        assert!(
            (treated - 1.0).abs() < 1e-9,
            "a Campus that can still repay is worth its price: {treated}"
        );

        // And the reason the taper existed is preserved: a Campus begun with
        // a handful of turns left still does not outbid a defender.
        g.turn = 250;
        assert_eq!(on.research_payback(&g), 0.0);
        g.turn = 230;
        assert!(
            on.research_payback(&g) < 0.51,
            "inside the payback window the ramp is live"
        );
    }

    /// The helper is checked against `Game::city_yields` itself, not against a
    /// second copy of the rule: whatever the model pays, the price knows.
    #[test]
    fn the_multiplier_credit_is_exactly_what_the_model_pays() {
        let mut game = Game::new_full(1, 24, 16, 91_989, 200, 0, false);
        let city = found_capital(&mut game, 0);
        let site = game.cities[&city]
            .owned_tiles
            .iter()
            .copied()
            .find(|position| *position != game.cities[&city].pos)
            .unwrap();
        set_district(&mut game, city, site, "campus");
        game.cities
            .get_mut(&city)
            .unwrap()
            .buildings
            .push(crate::name!("library"));
        let library = game.rules.buildings["library"].clone();

        // A small city with a low-adjacency Campus qualifies for neither half,
        // so the gene is a strict no-op there — the price is unchanged for
        // every city that has not yet earned the card.
        game.cities.get_mut(&city).unwrap().pop = 4;
        game.players[0].policies = [crate::name!("rationalism")].into_iter().collect();
        assert_eq!(
            AdvancedAi::campus_multiplier_science(&game, &game.cities[&city], &library),
            0.0,
            "neither half earned, nothing credited"
        );

        // Fifteen Population earns one half.
        //
        // ⚠ THE TWO NUMBERS ARE AT DIFFERENT ALTITUDES. `city_yields` reports
        // the city's science after the empire-wide percentages it carries — a
        // rating sweep on this fixture puts that factor at 0.7, and the
        // Library's own contribution at 2.8 rather than its printed 2.0
        // because more is added to it AFTER the card is applied. Meanwhile
        // `production_value` prices `spec.yields` RAW and scales no building
        // by anything, so the credit is raw too, deliberately: the model then
        // applies the same factor to the printed yield and to the credit
        // alike. A first draft asserted the raw difference and failed at 1.0
        // against 0.7 — which is that factor, not a pricing error.
        //
        // What survives both altitudes is the RATIO. The card's payment is
        // exactly linear in its rating (measured 0.35 / 0.70 / 1.40 / 2.80 at
        // ratings 50 / 100 / 200 / 400), so if the credit tracks the model it
        // must scale by the same factor between any two ratings. That pins
        // the halves, the gate and the base without asking what the empire's
        // percentages happen to be on this map.
        game.cities.get_mut(&city).unwrap().pop = 15;
        let card_payment = |game: &mut Game, rating: f64| -> f64 {
            std::sync::Arc::make_mut(&mut game.rules)
                .policies
                .get_mut("rationalism")
                .unwrap()
                .effects
                .insert("campus_building_science_pct".to_string(), rating);
            game.players[0].policies = [crate::name!("rationalism")].into_iter().collect();
            let with_card = game.city_yields(city).science;
            game.players[0].policies.clear();
            with_card - game.city_yields(city).science
        };
        let credit = |game: &mut Game, rating: f64| -> f64 {
            std::sync::Arc::make_mut(&mut game.rules)
                .policies
                .get_mut("rationalism")
                .unwrap()
                .effects
                .insert("campus_building_science_pct".to_string(), rating);
            game.players[0].policies = [crate::name!("rationalism")].into_iter().collect();
            let credited =
                AdvancedAi::campus_multiplier_science(game, &game.cities[&city], &library);
            game.players[0].policies.clear();
            credited
        };
        let (paid_one, paid_two) = (card_payment(&mut game, 100.0), card_payment(&mut game, 200.0));
        let (credit_one, credit_two) = (credit(&mut game, 100.0), credit(&mut game, 200.0));
        assert!(paid_one > 0.0 && credit_one > 0.0, "the Population half pays");
        assert!(
            (paid_two / paid_one - credit_two / credit_one).abs() < 1e-9,
            "credit {credit_one} -> {credit_two} against payment {paid_one} -> {paid_two}"
        );
        // And the raw credit is the printed yield times the qualifying halves,
        // which is the number `production_value` prices at 42 a point.
        assert!(
            (credit_one - library.yields.science * 0.5).abs() < 1e-9,
            "one half of a 100-rating card on a printed {}: {credit_one}",
            library.yields.science
        );

        // The gate is what keeps this honest: a city that has not earned a
        // half is priced exactly as before, and the model agrees it pays
        // nothing there.
        game.cities.get_mut(&city).unwrap().pop = 14;
        assert_eq!(card_payment(&mut game, 100.0), 0.0);
        assert_eq!(credit(&mut game, 100.0), 0.0);
        game.cities.get_mut(&city).unwrap().pop = 15;

        // And with no card at all there is nothing to credit, at any size.
        game.players[0].policies.clear();
        assert_eq!(
            AdvancedAi::campus_multiplier_science(&game, &game.cities[&city], &library),
            0.0
        );

        // A building outside the Campus is never touched by a Campus card.
        let monument = game.rules.buildings["monument"].clone();
        game.players[0].policies = [crate::name!("rationalism")].into_iter().collect();
        assert_eq!(
            AdvancedAi::campus_multiplier_science(&game, &game.cities[&city], &monument),
            0.0,
            "the Campus multiplier is a Campus multiplier"
        );
    }
}

