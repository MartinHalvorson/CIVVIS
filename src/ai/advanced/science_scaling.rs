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
//! **`research-tier-premium`.** `RESEARCH_BUILDING_DEBT` pays a Campus building
//! **240** for standing in a Campus that lacks it, and pays the same 240
//! whichever rung is missing — the Library (printed **2** Science), the
//! University (**4**), or the Research Lab (**3**, plus `powered_science`
//! **5** in a powered city, more than any other Campus building earns). The
//! empire is told the three rungs are worth the same, and coverage collapses
//! exactly as the yields grow. This scales the debt by the rung's own Science
//! against the chain's first, floored there so nothing is owed less than a
//! Library and capped so a modded yield cannot take the queue.
//!
//! ⚠ A first draft of this gene aimed at `DISTRICT_BUILDING_CHAIN_TIER_DECAY`
//! instead, exempting the Campus family from the chain's per-tier discount —
//! and was a **strict no-op in every game screened**, because
//! `chain_family_held` requires `district_building_chain`, which is
//! `default:off`, so a `--baseline best` seat never opens that branch. It read
//! exactly +0.0 pp on wins over two windows and 252 seat-pairs, which is what
//! an inert gene reads and is why the flat +0.0 was worth chasing rather than
//! filing as noise. **Check that a gene's branch is reachable under the
//! baseline the screen runs before spending games on it.**
//!
//! **`research-floor-holds`.** The citizen half of the taper.
//! `RESEARCH_CITIZEN_TILT` and `refresh_research_weight` — the standing tilt
//! toward beakers in every lane's emphasis, and the floor under a beaker's
//! price sliding `RESEARCH_FLOOR_EARLY` **3.0** to `RESEARCH_FLOOR_LATE`
//! **1.0** — ride the same `research_horizon`, so at t150/250 a beaker is
//! floored at **1.8** and the tilt is at 40%, and by t220 they are **1.24** and
//! 12%. The empire builds the laboratory and then declines to staff it. Both
//! move to the payback horizon. Separate from `science-payback-horizon`
//! because that gene moves what the empire BUYS and this one moves what it
//! then WORKS.
//!
//! The four are separate genes because they are separate claims: the taper is
//! the wrong shape; the number being tapered was measured off the wrong yield;
//! the debt is flat across rungs that are not; and the citizens who make the
//! beakers are tapered too. Any can be right without the others, and the
//! foldover screen prices all four from the same games at no extra cost.

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
            ("research-tier-premium", |ai: &AdvancedAi| {
                ai.research_tier_premium
            }),
            ("research-floor-holds", |ai: &AdvancedAi| {
                ai.research_floor_holds
            }),
            ("campus-finishes-first", |ai: &AdvancedAi| {
                ai.campus_finishes_first
            }),
            ("power-the-laboratory", |ai: &AdvancedAi| {
                ai.power_the_laboratory
            }),
            ("campus-adjacency-threshold", |ai: &AdvancedAi| {
                ai.campus_adjacency_threshold
            }),
            ("fifteenth-citizen", |ai: &AdvancedAi| ai.fifteenth_citizen),
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
        ai.enable_research_tier_premium();
        ai.enable_research_floor_holds();
        ai.enable_campus_finishes_first();
        ai.enable_power_the_laboratory();
        ai.enable_campus_adjacency_threshold();
        ai.enable_fifteenth_citizen();
        ai.disable_science_payback_horizon();
        ai.disable_science_multiplier_payoff();
        ai.disable_research_tier_premium();
        ai.disable_research_floor_holds();
        ai.disable_campus_finishes_first();
        ai.disable_power_the_laboratory();
        ai.disable_campus_adjacency_threshold();
        ai.disable_fifteenth_citizen();
        assert!(!ai.science_payback_horizon);
        assert!(!ai.science_multiplier_payoff);
        assert!(!ai.research_tier_premium);
        assert!(!ai.research_floor_holds);
        assert!(!ai.campus_finishes_first);
        assert!(!ai.power_the_laboratory);
        assert!(!ai.campus_adjacency_threshold);
        assert!(!ai.fifteenth_citizen);
    }

    /// The citizen half of the taper, and the proof it is a separate gene:
    /// `science-payback-horizon` does not move it and this does not move
    /// what `science-payback-horizon` moves.
    #[test]
    fn the_staffing_horizon_is_a_second_and_separate_taper() {
        let mut g = Game::new_full(2, 28, 18, 91_779, 250, 0, false);
        g.turn = 150;
        let shipped = AdvancedAi::new();
        let mut production_only = AdvancedAi::new();
        production_only.enable_science_payback_horizon();
        let mut staffing_only = AdvancedAi::new();
        staffing_only.enable_research_floor_holds();

        // At t150/250 the shipped taper has written off 60% of both halves.
        assert!((shipped.research_payback(&g) - 0.4).abs() < 1e-9);
        assert!((shipped.research_staffing_horizon(&g) - 0.4).abs() < 1e-9);

        // Each gene moves its own half and only its own half.
        assert!((production_only.research_payback(&g) - 1.0).abs() < 1e-9);
        assert!((production_only.research_staffing_horizon(&g) - 0.4).abs() < 1e-9);
        assert!((staffing_only.research_staffing_horizon(&g) - 1.0).abs() < 1e-9);
        assert!((staffing_only.research_payback(&g) - 0.4).abs() < 1e-9);

        // And what the staffing half actually sets: the floor under a beaker
        // in every lane, which the shipped slide has already halved.
        let floor = |ai: &mut AdvancedAi| {
            ai.research_economy = true;
            ai.refresh_research_weight(&g);
            ai.research_weight
        };
        let mut shipped = shipped;
        let shipped_floor = floor(&mut shipped);
        let held_floor = floor(&mut staffing_only);
        assert!(
            (shipped_floor - 1.8).abs() < 1e-9,
            "the shipped floor at t150/250: {shipped_floor}"
        );
        assert!(
            (held_floor - 3.0).abs() < 1e-9,
            "a beaker is still worth its early price while it can pay: {held_floor}"
        );
    }

    /// The rungs of the research chain are 2, 4 and 3-plus-5, and the debt
    /// that buys them is flat. This is what the gene changes and what it
    /// deliberately does not.
    #[test]
    fn the_research_debt_follows_the_rung_it_is_buying() {
        let mut game = Game::new_full(1, 24, 16, 91_989, 200, 0, false);
        let city = found_capital(&mut game, 0);
        let site = game.cities[&city]
            .owned_tiles
            .iter()
            .copied()
            .find(|position| *position != game.cities[&city].pos)
            .unwrap();
        set_district(&mut game, city, site, "campus");
        let shipped = AdvancedAi::new();
        let mut treated = AdvancedAi::new();
        treated.enable_research_tier_premium();
        let weight = |ai: &AdvancedAi, g: &Game, name: &str| {
            ai.research_tier_weight(g, &g.cities[&city], &g.rules.buildings[name])
        };

        // Off, the debt is flat: every rung reads 1.0, which is the shipped
        // `RESEARCH_BUILDING_DEBT * horizon` recovered exactly.
        for rung in ["library", "university", "research_lab"] {
            assert_eq!(weight(&shipped, &game, rung), 1.0, "{rung} off");
        }

        // On, the first rung is still 1.0 — this is not a blanket raise — and
        // the University is owed twice it, off its own printed 4 against 2.
        assert_eq!(weight(&treated, &game, "library"), 1.0);
        assert_eq!(weight(&treated, &game, "university"), 2.0);

        // The Research Lab is the rung the whole gene is about: its printed 3
        // is the SMALLER half of what it earns, because `powered_science` adds
        // **5** more than any other Campus building earns at all. The debt has
        // to see that or the 3%-coverage rung stays last forever.
        let powered = game.rules.buildings["research_lab"]
            .effects
            .get("powered_science")
            .copied()
            .unwrap();
        assert_eq!(powered, 5.0, "the Lab's power yield is what the gene reads");
        // ⚠ `city_is_powered` is `demand <= 0 || supply >= demand`, so a city
        // with nothing that CONSUMES power reads powered — and `city_yields`
        // pays the Lab its 5 there on exactly that test. The gene reads the
        // same predicate rather than a stricter one of its own, so the price
        // and the payment agree; a first draft asserted the fixture was
        // unpowered and was wrong about the model, not about the code.
        assert!(game.city_is_powered(&game.cities[&city]));
        assert_eq!(weight(&treated, &game, "research_lab"), 4.0);
        // (3 + 5) / 2 lands exactly on the cap, which is how the cap was sized.
        assert_eq!(
            (game.rules.buildings["research_lab"].yields.science + powered) / 2.0,
            super::super::RESEARCH_TIER_PREMIUM_CAP
        );
        // And the cap really binds rather than merely being met: a rung worth
        // ten Library-equivalents is still only ever owed four.
        let mut runaway = game.rules.buildings["research_lab"].clone();
        runaway.yields.science = 100.0;
        assert_eq!(
            treated.research_tier_weight(&game, &game.cities[&city], &runaway),
            super::super::RESEARCH_TIER_PREMIUM_CAP
        );
    }

    /// The brake only brakes where there is something unfinished to brake
    /// for. Both no-op cases are the point: an empire with no Campus is not
    /// being told to want research less, and neither is one that has finished
    /// every Campus it owns.
    #[test]
    fn the_campus_brake_reads_one_until_a_campus_stands_empty() {
        let mut game = Game::new_full(1, 24, 16, 91_989, 200, 0, false);
        let city = found_capital(&mut game, 0);
        let mut ai = AdvancedAi::new();
        ai.research_economy = true;
        ai.enable_campus_finishes_first();

        // No Campus anywhere: nothing to finish, so the term is untouched and
        // the first research city is bought at the shipped price.
        ai.refresh_research_chain_completion(&game, 0);
        assert_eq!(ai.research_chain_completion, 1.0, "no Campus, no brake");

        // ⚠ A Campus alone is still not a brake, and that is deliberate: a
        // city that cannot yet PRODUCE any Campus building has nothing
        // unfinished to answer for. The Library is gated on `writing`, so the
        // fixture has to grant it — a first draft did not and read 1.0 with an
        // empty Campus, which is the same fixture trap that once made a
        // purchase-ranking test pass vacuously because its only rival was not
        // producible.
        game.players[0].techs.insert(crate::name!("writing"));

        // A Campus with every building it can currently produce still missing
        // is what the census measured being bought over and over.
        let site = game.cities[&city]
            .owned_tiles
            .iter()
            .copied()
            .find(|position| *position != game.cities[&city].pos)
            .unwrap();
        set_district(&mut game, city, site, "campus");
        ai.refresh_research_chain_completion(&game, 0);
        let empty = ai.research_chain_completion;
        assert!(
            empty < 1.0,
            "a Campus standing without its chain brakes the next one: {empty}"
        );
        assert!(
            empty >= super::super::RESEARCH_COVERAGE_UNFINISHED_FLOOR,
            "and never below the floor, so a real research hole can still be \
             filled: {empty}"
        );

        // Filling what it can produce releases the brake completely.
        let producible: Vec<Name> = game
            .rules
            .buildings
            .iter()
            .filter(|(_, spec)| {
                !spec.wonder
                    && spec.district.map(|d| game.district_family(d))
                        == Some(crate::name!("campus"))
            })
            .map(|(name, _)| Name::new(name))
            .filter(|building| {
                game.can_produce(
                    0,
                    city,
                    &crate::game::Item::Building {
                        building: *building,
                    },
                )
            })
            .collect();
        assert!(!producible.is_empty(), "the fixture can build its chain");
        for building in producible {
            game.cities.get_mut(&city).unwrap().buildings.push(building);
        }
        ai.refresh_research_chain_completion(&game, 0);
        assert_eq!(
            ai.research_chain_completion, 1.0,
            "a finished Campus is not a reason to want research less"
        );

        // And with the gene off it is 1.0 whatever the board looks like.
        let mut shipped = AdvancedAi::new();
        shipped.research_economy = true;
        game.cities.get_mut(&city).unwrap().buildings.clear();
        shipped.refresh_research_chain_completion(&game, 0);
        assert_eq!(shipped.research_chain_completion, 1.0);
    }

    /// The switch, and the three cities where flipping it buys nothing.
    ///
    /// A Research Lab prints 3 Science and carries `powered_science` 5 — more
    /// than any other Campus building earns in total — and the model pays that
    /// 5 only while the city is powered. Nothing in the controller bought the
    /// switch.
    #[test]
    fn a_power_plant_is_worth_the_laboratory_it_switches_on() {
        let mut game = Game::new_full(1, 24, 16, 91_989, 200, 0, false);
        let city = found_capital(&mut game, 0);
        let site = game.cities[&city]
            .owned_tiles
            .iter()
            .copied()
            .find(|position| *position != game.cities[&city].pos)
            .unwrap();
        set_district(&mut game, city, site, "campus");
        let plant = game.rules.buildings["coal_power_plant"].clone();
        let generated = plant.effects["power_generated"];
        assert_eq!(generated, 4.0, "the plant the fixture leans on");

        // A city with nothing that CONSUMES power is already "powered" by
        // `demand <= 0`, so the plant switches nothing on. Every city before
        // the Industrial era is this city.
        assert!(game.city_is_powered(&game.cities[&city]));
        assert_eq!(
            AdvancedAi::power_switched_on(&game, &game.cities[&city], &plant).science,
            0.0,
            "no demand, nothing to switch"
        );

        // Now stand the laboratory. It draws power, so the city goes dark and
        // its 5 Science is off.
        game.cities
            .get_mut(&city)
            .unwrap()
            .buildings
            .push(crate::name!("research_lab"));
        let demand = game.city_power_demand(&game.cities[&city]);
        assert!(demand > 0.0, "a Research Lab draws power");
        assert!(
            !game.city_is_powered(&game.cities[&city]),
            "and the city is dark"
        );
        let switched = AdvancedAi::power_switched_on(&game, &game.cities[&city], &plant);
        assert_eq!(
            switched.science, game.rules.buildings["research_lab"].effects["powered_science"],
            "the plant is worth exactly the beakers it turns on"
        );

        // A plant too small to meet the demand buys nothing: the yields stay
        // off and the production is spent for a switch that does not flip.
        let mut token = plant.clone();
        token
            .effects
            .insert("power_generated".to_string(), demand / 2.0);
        assert_eq!(
            AdvancedAi::power_switched_on(&game, &game.cities[&city], &token).science,
            0.0,
            "half the demand is not half the yield, it is none of it"
        );

        // A building that generates no power is never credited, whatever the
        // city holds.
        let library = game.rules.buildings["library"].clone();
        assert_eq!(
            AdvancedAi::power_switched_on(&game, &game.cities[&city], &library),
            Yields::default()
        );

        // And with the gene off nothing above reaches a price: the flag is
        // what gates the term, and it ships off.
        let shipped = AdvancedAi::new();
        assert!(!shipped.power_the_laboratory);
    }

    /// The threshold, pinned to the engine that enforces it.
    ///
    /// A census probe found Rationalism SLOTTED and **not one Campus of nine**
    /// clearing the adjacency half it pays, while four of the ten cities still
    /// held a free plot worth exactly 4.0. The price could not see a threshold;
    /// this is the number it now sees, and the test asserts the engine agrees
    /// rather than asserting a 4 typed twice.
    #[test]
    fn the_campus_threshold_is_the_one_the_engine_actually_gates_on() {
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
        game.cities.get_mut(&city).unwrap().pop = 4;
        game.players[0].policies = [crate::name!("rationalism")].into_iter().collect();

        let raw = |game: &Game| {
            let mut yields = Yields::default();
            for source in game.district_adjacency_sources(crate::name!("campus"), site) {
                if source.source != "adjacency_bonus" {
                    yields.add(source.yields);
                }
            }
            yields.science
        };

        // Below the threshold the model pays nothing for the adjacency half,
        // and neither does the gene: a low-adjacency Campus is priced as it
        // always was.
        assert!(raw(&game) < super::super::CAMPUS_MULTIPLIER_ADJACENCY_THRESHOLD);
        let before = game.city_yields(city).science;
        let mut ai = AdvancedAi::new();
        ai.enable_campus_adjacency_threshold();
        ai.refresh_campus_multiplier_constants(&game);
        assert_eq!(
            ai.campus_threshold_bonus(&game, crate::name!("campus"), site),
            0.0,
            "adjacency {} is under the gate",
            raw(&game)
        );

        // Two mountains beside the Campus lift its RAW adjacency to the gate.
        let ring: Vec<Pos> = game
            .nbrs(site)
            .into_iter()
            .filter(|position| {
                *position != game.cities[&city].pos && game.map.get(*position).is_some()
            })
            .collect();
        for position in ring.iter().take(4) {
            let tile = game.map.tiles.get_mut(position).unwrap();
            tile.terrain = crate::name!("mountain");
            tile.feature = None;
            tile.hills = false;
            tile.improvement = None;
            tile.district = None;
        }

        // ⭐ THE ENGINE IS THE AUTHORITY, NOT THE CONSTANT. Whatever raw
        // adjacency now stands, the model pays the half exactly when it is at
        // or above the constant — so the two agree by construction, not by a
        // number written down twice.
        let now = raw(&game);
        let paid_more = game.city_yields(city).science > before;
        assert_eq!(
            now >= super::super::CAMPUS_MULTIPLIER_ADJACENCY_THRESHOLD,
            paid_more,
            "raw adjacency {now}: the constant and the engine must agree on the gate"
        );

        if paid_more {
            let bonus = ai.campus_threshold_bonus(&game, crate::name!("campus"), site);
            assert!(bonus > 0.0, "a plot at the gate is worth what it unlocks");
            // The credit is the chain's printed Science times half the card,
            // which is what the second half will pay once the chain stands.
            assert!(
                (bonus - ai.campus_chain_science * ai.campus_multiplier_half / 100.0).abs() < 1e-9
            );
            // And no other district is ever credited it.
            assert_eq!(
                ai.campus_threshold_bonus(&game, crate::name!("theater_square"), site),
                0.0
            );
            // With the gene off the constants are zeroed and the term is dead.
            let mut shipped = AdvancedAi::new();
            shipped.refresh_campus_multiplier_constants(&game);
            assert_eq!(shipped.campus_multiplier_half, 0.0);
            assert_eq!(
                shipped.campus_threshold_bonus(&game, crate::name!("campus"), site),
                0.0
            );
        }
    }

    /// The gate, pinned to the engine, and the four cities it must not pay.
    #[test]
    fn the_population_gate_pays_only_where_crossing_it_would_buy_something() {
        let mut game = Game::new_full(1, 24, 16, 91_989, 200, 0, false);
        let city = found_capital(&mut game, 0);
        let site = game.cities[&city]
            .owned_tiles
            .iter()
            .copied()
            .find(|position| *position != game.cities[&city].pos)
            .unwrap();
        set_district(&mut game, city, site, "campus");
        game.players[0].techs.insert(crate::name!("writing"));
        game.players[0].policies = [crate::name!("rationalism")].into_iter().collect();
        // ⚠ THE FIXTURE'S HOUSING CAPS AT 6, AND THE GATE IS 15. A city cannot
        // be "one citizen short and still growing" without room to grow, so a
        // first draft of this test asserted the positive case on a city the
        // gene was correctly refusing. `observed_city_housing_adjustments` is
        // the mirror's own host-minus-model channel and the cheapest honest
        // way to give the fixture a ceiling worth testing against.
        game.observed_city_housing_adjustments.insert(city, 14.0);
        assert!(game.city_housing(&game.cities[&city]) > 15.0);

        let mut ai = AdvancedAi::new();
        ai.research_economy = true;
        ai.enable_fifteenth_citizen();
        ai.refresh_campus_multiplier_constants(&game);
        let prize =
            |ai: &AdvancedAi, game: &Game| ai.population_gate_prize(game, &game.cities[&city]);

        // ⭐ THE GATE IS THE ENGINE'S, NOT A NUMBER TYPED TWICE. Walk the city
        // up one citizen at a time and find where `city_yields` starts paying
        // the half; that turn is the constant.
        game.cities
            .get_mut(&city)
            .unwrap()
            .buildings
            .push(crate::name!("library"));
        let mut engine_gate = None;
        for pop in 1..=20 {
            game.cities.get_mut(&city).unwrap().pop = pop;
            let with = game.city_yields(city).science;
            game.players[0].policies.clear();
            let without = game.city_yields(city).science;
            game.players[0].policies = [crate::name!("rationalism")].into_iter().collect();
            if with > without && engine_gate.is_none() {
                engine_gate = Some(pop as f64);
            }
        }
        assert_eq!(
            engine_gate,
            Some(super::super::CAMPUS_POPULATION_GATE),
            "the constant and city_yields must name the same threshold"
        );

        // A city one citizen short, holding a Library, growing: the whole point.
        game.cities.get_mut(&city).unwrap().pop = 14;
        let (beakers, closeness) = prize(&ai, &game).expect("one short is within reach");
        assert!(beakers > 0.0 && closeness > 0.8, "{beakers} {closeness}");
        // The prize is the HELD chain times the half, not a hoped-for chain.
        assert!(
            (beakers
                - game.rules.buildings["library"].yields.science * ai.campus_multiplier_half
                    / 100.0)
                .abs()
                < 1e-9
        );

        // Past the gate: nothing left to buy.
        game.cities.get_mut(&city).unwrap().pop = 15;
        assert!(prize(&ai, &game).is_none(), "already earned");

        // Too far short: it will not arrive before the clock.
        game.cities.get_mut(&city).unwrap().pop = 4;
        assert!(prize(&ai, &game).is_none(), "out of reach");

        // Near, but nothing standing to multiply.
        game.cities.get_mut(&city).unwrap().pop = 14;
        game.cities.get_mut(&city).unwrap().buildings.clear();
        assert!(prize(&ai, &game).is_none(), "no Campus building to double");
        game.cities
            .get_mut(&city)
            .unwrap()
            .buildings
            .push(crate::name!("library"));

        // ⚠ And near, but CAPPED. One of the six cities the census found was
        // housing-stopped; paying for its next Granary buys a threshold it
        // will never cross.
        assert!(prize(&ai, &game).is_some(), "the fixture can still grow");
        game.observed_city_housing_adjustments.remove(&city);
        assert!(
            game.city_housing_headroom(&game.cities[&city]) <= 0.0,
            "and without the ceiling it is housing-stopped"
        );
        assert!(
            prize(&ai, &game).is_none(),
            "a city that cannot grow is not near the gate, however few citizens \
             separate it"
        );
        game.observed_city_housing_adjustments.insert(city, 14.0);

        // With the gene off the term is dead whatever the board says.
        let mut shipped = AdvancedAi::new();
        shipped.research_economy = true;
        shipped.refresh_campus_multiplier_constants(&game);
        game.cities.get_mut(&city).unwrap().pop = 14;
        assert!(shipped
            .population_gate_prize(&game, &game.cities[&city])
            .is_none());
    }

    /// A constant standing in for a ruleset value has to be pinned to the
    /// ruleset, or a data change leaves the price quietly wrong.
    #[test]
    fn research_tier_premium_is_priced_against_the_shipped_library() {
        let rules = crate::rules::Rules::embedded();
        assert_eq!(
            rules.buildings["library"].yields.science,
            super::super::RESEARCH_CHAIN_FIRST_RUNG_SCIENCE,
            "the first rung of the Campus chain"
        );
        // And it really is the first rung: nothing in the family is cheaper.
        let campus_cost = |name: &str| rules.buildings[name].cost;
        for rung in ["university", "research_lab"] {
            assert!(
                campus_cost(rung) > campus_cost("library"),
                "{rung} comes after the Library"
            );
        }
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
        let (paid_one, paid_two) = (
            card_payment(&mut game, 100.0),
            card_payment(&mut game, 200.0),
        );
        let (credit_one, credit_two) = (credit(&mut game, 100.0), credit(&mut game, 200.0));
        assert!(
            paid_one > 0.0 && credit_one > 0.0,
            "the Population half pays"
        );
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
