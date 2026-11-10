use super::*;
use serde_json::json;
use std::collections::BTreeSet;

#[test]
fn a_policy_may_retire_one_card_or_several_and_both_shapes_parse() {
    // Civilization VI's `ObsoletePolicies` is keyed by the PREDECESSOR, so a
    // successor appears once per card it retires — and three of them retire two
    // apiece. The one-card form has to keep working: 19 entries already use it,
    // and rewriting every one into a single-element list to express those three
    // would be a large diff for no gain.
    let one: PolicySpec =
        serde_json::from_value(json!({"slot": "economic", "replaces": "ilkum"})).unwrap();
    assert_eq!(one.replaces, vec![Name::new("ilkum")]);

    let many: PolicySpec =
        serde_json::from_value(json!({"slot": "economic", "replaces": ["bastions", "serfdom"]}))
            .unwrap();
    assert_eq!(
        many.replaces,
        vec![Name::new("bastions"), Name::new("serfdom")]
    );

    let none: PolicySpec = serde_json::from_value(json!({"slot": "economic"})).unwrap();
    assert!(
        none.replaces.is_empty(),
        "a card that retires nothing stays empty"
    );
}

#[test]
fn shipped_ruleset_fingerprint_tracks_the_audited_firaxis_rows() {
    // The fingerprint is the Elo ledger's binding. Firaxis-exact unique units,
    // including the Shield Bearer, Oromo Cavalry, Pairidaeza, and Armagh's
    // Monastery found by live replay, exact Natural Wonder placement rows, and
    // the complete terrain-improvement catalogue are real simulation changes;
    // older ledgers retain their original fingerprint.
    //
    // Moved again by four divergences the `civ6_fidelity` audit found against
    // the shipped database and that were each confirmed by querying it:
    // `cartography` and `mass_production` carried a `shipbuilding` prerequisite
    // the game does not have, and Armagh's Monastery (0) and the Inca Terrace
    // Farm (0.5) both under-counted housing against a DB that stores the figure
    // DOUBLED — a ratio checked across all 16 housing improvements before either
    // was touched, since matching the raw column would have doubled every one.
    //
    // Moved again by fixing the fidelity audit's content-pack load order:
    // Pike and Shot maintenance is 4, Tagma costs 220 with 4 maintenance
    // and upgrades to Cuirassier, Prasat is Faith 6 with one Relic slot,
    // Sukiennice is Gold 2, Tlachtli is Culture 2, and Eyjafjallajökull
    // gives adjacent Food 1. These are the effective rows after the
    // complete Gathering Storm manifest order, rather than values from an
    // overlay that was accidentally applied before its base row.
    //
    // Rock-Hewn Church's Hills and Volcanic Soil alternatives are carried
    // as one semantic field (`hills_or_feature`) in both the ruleset and
    // the audit; it is neither a false Hills-only rule nor an audit waiver.
    //
    // `ai_eval advanced_v1 basic --pairs 10 --players 4 --turns 200 --seed
    // 31337 --jobs 1 --deployment-comparison` was BYTE-IDENTICAL with the
    // change stashed and applied from this worktree — every entry touched is
    // another civilization's unique or a natural wonder, so Rome's frozen
    // path never reads one. Compatibility re-pin; the Elo protocol does not
    // move.
    //
    // Moved again by the twenty-three city-state suzerain bonuses salvaged
    // from an abandoned worktree, which add Lahore's levied `nihang` and its
    // seven-node promotion tree to the shipped rows.
    //
    // ⚠ Unlike every re-pin above, this one is NOT a compatibility re-pin.
    // The same `ai_eval advanced_v1 basic --pairs 10 --players 4 --turns 200
    // --seed 31337 --jobs 1 --deployment-comparison` was NOT byte-identical
    // across the change: game-win share moved 85.0% -> 75.0% and the
    // Elo-equivalent +301 -> +191, with the sign test falling from
    // SIGNIFICANT to INCONCLUSIVE. That is expected rather than alarming —
    // a suzerain bonus is read by whichever seat holds the envoys, so
    // seating twenty-three more of them perturbs both sides — and the two
    // Wilson intervals overlap heavily (+29..+574 vs -40..+422) on a single
    // seed, so this is not evidence of a strength regression. It IS a real
    // simulation change: ledgers recorded before this fingerprint are not
    // comparable with ledgers recorded after it.
    // Moved again by giving all 182 city-states their real WGS84 site, so
    // True Start Earth can seat them where they stood and the selector can
    // spread a game's draw across the world.
    //
    // ⚠ THE CONVENTIONAL COMMAND CANNOT SEE THIS CHANGE, AND SAYING SO IS
    // THE POINT. `ai_eval advanced_v1 basic --pairs 10 --players 4 --turns
    // 200 --seed 31337 --jobs 1 --deployment-comparison` came back
    // byte-identical across the change — but `ai_eval` defaults
    // `--city-states` to 0, so that run seats no city-states at all and
    // its silence is structural, not evidence. Read as a compatibility
    // re-pin it would have been a false green.
    //
    // Re-run as `... --city-states 6`, which does exercise them, it is a
    // real simulation change: game-win share 60.0% -> 70.0% and the
    // Elo-equivalent +70 -> +147. Both runs are INCONCLUSIVE by the sign
    // test (p=0.6875 and p=0.1250) and their Wilson intervals overlap
    // almost entirely (-137..+278 vs -73..+367), so this is not evidence
    // of a strength change in either direction — but a different set of
    // city-states carries a different set of Suzerain bonuses, so ledgers
    // recorded before this fingerprint are not comparable with ledgers
    // recorded after it.
    // Moved by the Oasis gaining `blocks_district` (#1607): Civilization
    // VI refuses a district on an Oasis, CIVVIS only knew that for city
    // founding, and run civvis-20260811T230324Z asked the host for a
    // Campus on one oasis tile 40 times.
    //
    // The conventional command's seat outcomes and production censuses
    // came back identical across the change (advanced_v1 17/40, basic
    // 3/40 on both sides) — read as a compatibility re-pin, with the same
    // structural caveat as the city-states entry above: the 24×16 fractal
    // seldom puts an Oasis inside a workable ring, so identity here is
    // expected and is NOT evidence the rule is inert. It binds in the
    // live regime, where the diagnosed re-ask loop lived; judge it on
    // `build_no_plot` repeat structure there.
    // Moved by giving Rome its civilization ability. `data/civs.json` had
    // only Trajan's leader ability (the free Monument); All Roads Lead to
    // Rome — a Trading Post in every city from founding and +1 Gold per
    // own city a route passes through — is `free_trading_posts` /
    // `own_trading_post_route_gold`, read by `Game::trading_post_route_gold`
    // and confirmed against the shipped database
    // (`TRAIT_GOLD_FROM_DOMESTIC_TRADING_POSTS`,
    // `MODIFIER_PLAYER_ADJUST_TRADE_ROUTE_YIELD_PER_POST_IN_OWN_CITY` 1,
    // `TRADING_POST_GOLD_IN_OWN_CITY` 0 for everyone else). Measured first
    // on live run civvis-20260816T011314Z: a domestic route Antium -> Rome
    // read +1 Gold in the host and 0 in the model for its whole life. A
    // real simulation change for Rome seats only; every other row is
    // untouched.
    // Moved by two `Improvement_BonusYieldChanges` grants the audit could
    // not see: `civ6_fidelity.py` keyed that table by `Id`, and the shipped
    // table carries a duplicate (Id 225 is both Camp/Gold/Synthetic
    // Materials and Fishing Boats/Production/Colonialism), so Colonialism's
    // +1 Production on Fishing Boats was never audited and never modelled —
    // every worked boat read one Production under the host for fifty turns
    // of run civvis-20260816T115139Z. And the XML route never applied the
    // expansions' `<Expansion>_RemoveData.xml` (Priority 1 in the modinfo),
    // so a retired base row — Robotics granting Pasture Production, moved to
    // Replaceable Parts by Gathering Storm — kept "confirming" a grant the
    // game no longer makes. Both are corrected in `tree_effects.json`;
    // confirmed against the compiled gameplay database.
    // Moved by four beliefs the shipped database has and `beliefs.json`
    // did not: Divine Inspiration (follower, +4 Faith per Wonder in a
    // following city — `MODIFIER_SINGLE_CITY_ADJUST_WONDER_YIELD_CHANGE`
    // 4), Reliquaries (follower, Relics ×4 Faith and Tourism —
    // `MODIFIER_SINGLE_CITY_ADJUST_GREATWORK_YIELD` ScalingFactor 300),
    // Lay Ministry (founder, +1 Faith per Holy Site and +1 Culture per
    // Theater Square in following cities — `BELIEF_YIELD_PER_DISTRICT`)
    // and Sacred Places (founder, +2 of each yield per following city
    // with a Wonder — `BELIEF_YIELD_PER_CITY_WITH_WONDER`). Measured
    // first on live run civvis-20260816T123936Z: Rome followed a
    // Catholicism it had not founded and read 35 Faith in the host, 23 in
    // the model, for its last twenty turns — three Wonders under Divine
    // Inspiration. A real change for every simulated seat: the Prophet
    // has four more beliefs to choose from.
    // Moved by Scottish Enlightenment's eight active Gathering Storm rows.
    // CIVVIS had invented +1 Science and Production in every Scottish city;
    // the shipped modifier data instead gives Happy cities +5% of each,
    // doubling to +10% when Ecstatic, plus one Great Scientist point per
    // active Campus and one Great Engineer point per active Industrial Zone
    // (again doubled when Ecstatic). The named Ibn Khaldun Great Scientist
    // action uses the same happiness-yield effect but remains deliberately
    // outside the modeled-person roster, rather than being conflated with
    // Scotland's civilization trait.
    // ⚠ Nothing moved it on 2026-08-18 morning. #2049 changed four Founder
    // beliefs to the base game's forms and re-pinned this to
    // `fnv1a64:2effccaa9b3512e3`; #2050 reverted the data and this went
    // back to `fnv1a64:585ff2655ffd3a6d` unchanged, which is how the revert
    // was verified rather than trusted. See `docs/FIDELITY.md`.
    //
    // Moved by five pantheons the Gathering Storm install has and
    // `beliefs.json` did not: Goddess of the Hunt (+1 Food and +1
    // Production from Camps), Stone Circles (+2 Faith from Quarries),
    // Goddess of Festivals (+1 Culture from Plantations — the expansion
    // deletes the base game's Food row), Religious Idols (+2 Faith from
    // Mines over Bonus and Luxury resources) and God of Craftsmen (+1
    // Production and +1 Faith from any improved Strategic resource — the
    // expansion deletes the base game's Mine-only row). The roster goes
    // from 6 of the game's 23 pantheons to 11, and the pantheon is the
    // earliest religious choice every civilization makes.
    //
    // ⚠ Read from the **install**'s `Expansion*/Data/*.xml` with
    // `Expansion2_RemoveData.xml` checked for every id, not from the
    // compiled cache. Two of the five are cases where the cache on a
    // base-game machine states the opposite of the shipped rule.
    //
    // Moved again by the real Gathering Storm World Games project.
    // `PROJECT_TRAIN_ATHLETES` costs 200 Production, grants 50 competition
    // score, and is unlocked by the host's World Congress effect rather
    // than a tree node. Native CIVVIS games do not surface it; live
    // mirrors do only while the authoritative tracker names World Games.
    // Moved again by the real Gathering Storm International Space Station
    // project. `PROJECT_TRAIN_ASTRONAUTS` costs 200 Production in a
    // Spaceport, grants 30 competition score, and follows the same
    // host-only availability rule while the authoritative tracker names
    // the Space Station competition.
    // Moved again by Gathering Storm's Aid Request project.
    // `PROJECT_SEND_AID` costs 200 Production, grants 200 competition
    // score, and is available through either the ordinary or military Aid
    // Request effect instead of any tech or civic gate.
    // Moved again by the Climate Accords decommissioning projects. Each
    // costs 400 Production in an Industrial Zone, grants 100 score while
    // the host tracks Climate Accords, and consumes its coal, oil, or
    // nuclear power plant rather than being repeatable in one city.
    // Moved again by 36 Gathering Storm Great People. The roster held 29
    // of the game's 213 and stopped at the Atomic era, so from the midgame
    // every class ran out and `unused_great_person_faith` paid the whole
    // Campus, Theatre Square and Harbour yield out as Faith instead --
    // 26.6% of all non-prophet Great Person points, measured over eight
    // 6-player 200-turn games. Each addition takes its class, era, cost
    // and charges from `GreatPersonIndividuals` and `Eras`, so the
    // fidelity audit still reports zero divergent fields.
    // Moved again by the shipped `StartingBuildings` table, whose one
    // difficulty-gated row — `BUILDING_WALLS`, `ERA_ANCIENT`,
    // `DISTRICT_CITY_CENTER`, `MinorOnly = 1`, `MinDifficulty =
    // DIFFICULTY_IMMORTAL` — had no home in `DifficultySpec` at all.
    // The engine already granted the walls, from a rung number written
    // into `Game::new_with`; what moved here is that the ladder now
    // *says* which rungs grant what, so a rung is transcribed data like
    // every other line of this file rather than a constant in setup code.
    // Moved again by 82 more Great People, taking the roster to 147 of 213
    // and completing four classes outright -- every shipped Writer, Artist,
    // Musician and Prophet. Class, era, cost and charges again come from
    // `GreatPersonIndividuals`, `Eras` and `GreatWorks`, and the audit
    // again reports zero divergent fields over all 147.
    //
    // Moved again by the LAST TWELVE PANTHEONS, which completes the class:
    // Desert Folklore, Dance of the Aurora and Sacred Path (Holy Site
    // terrain and feature adjacency), God of War (post-combat Faith), God
    // of Healing, River Goddess (district Amenities and Housing on a
    // river), City Patron Goddess (first-district Production), Monument to
    // the Gods (Ancient/Classical wonder Production), Initiation Rites
    // (barbarian-camp Faith and healing), Lady of the Reeds and Marshes,
    // Goddess of Fire (feature yields) and Earth Goddess (Appeal). The
    // roster goes from 11 of the game's 23 pantheons to all 23, and the
    // pantheon is the earliest religious choice every civilization makes.
    //
    // ⚠ Read from the **install**'s `Expansion*/Data/*.xml` with
    // `Expansion2_RemoveData.xml` checked for every id. Three of these
    // twelve are cases where a base-game row states the opposite of the
    // shipped rule: the expansion deletes `EARTH_GODDESS_APPEAL_FAITH`
    // (Charming, MinimumAppeal 2) and re-adds it at Breathtaking
    // (MinimumAppeal 4), deletes `RIVER_GODDESS_HOLY_SITE_AMENITY` (+1
    // Amenity, no Housing) for a +2/+2 pair, and drops
    // `LADY_OF_THE_REEDS_PRODUCTION` (+1) for `..._PRODUCTION2` (+2).
    // Initiation Rites gains a second, Gathering-Storm-only half. See
    // `docs/FIDELITY.md`.
    // Moved again by the seventeen espionage promotions. The engine has
    // always resolved them by name out of `Game::SPY_PROMOTIONS`, so the
    // Spy was the one unit class whose promotions were absent from
    // `data/promotions.json` and invisible to the pedia, the mod overlay
    // and the fidelity audit alike. Class, tier and prerequisites come
    // from `UnitPromotions` (all seventeen are `Level = 1` with no
    // `UnitPromotionPrereqs`), and each magnitude from the promotion's own
    // `UnitPromotionModifiers` row, so the audit reports zero divergent
    // fields with them in.
    //
    // Moved again by four unit stats the audit could not see. The Nau, Toa
    // and Nihang carry their civilization's name in the shipped table
    // (`UNIT_PORTUGUESE_NAU`) and their Civilopedia name in CIVVIS, so all
    // four unique units were reported missing *and* extra at once and
    // compared against nothing. Aliasing them surfaced six wrong numbers:
    // the Nau's Maintenance (4 to 2) and sight (2 to 3), the Toa's cost
    // (110 to 120), Maintenance (2 to 0) and Combat (36 to 38), and the
    // Nihang's Maintenance (0 to 2).
    // Moved again by the imported modifier catalog. `data/modifiers.json`
    // is no longer empty: `tools/civ6_modifiers.py --emit-catalog` writes
    // one bundle per shipped `Modifiers` row of a declared effect, and the
    // ruleset object the game says owns that row attaches it by name. Most
    // of the fold restores the number CIVVIS already carried, so the
    // fingerprint moves without the ruleset changing; four rows do change
    // it. Eleven civics now award the Envoys `GRANT_INFLUENCE_TOKEN` gives
    // them (CIVVIS carried two of the thirteen), Jakob Fugger awards his
    // two, Sweeping Wind gains the `MOD_MOVE_AFTER_ATTACKING` it shares
    // with Elite Guard and Breakthrough, and Computers multiplies Tourism
    // by the +25% `COMPUTERS_BOOST_ALL_TOURISM` states instead of +100%.
    // Moved again by deleting `genghis_khan`. Civilization VI ships no
    // Great General of that name — Genghis Khan is Mongolia's *leader* —
    // and `tools/civ6_fidelity.py` had begun reporting him as the roster's
    // one "only in CIVVIS" row. He duplicated `timur`, the real
    // Classical-era `land_unit_promotion_level` general, at the same era,
    // cost and effect. This is the audit's `only_ours` column reaching zero
    // on `GreatPeople`, not a balance change.
    // Moved again by wiring `tools/civ6_fidelity.py --check --max 0` into
    // CI, whose first run found ten divergences nothing had reported
    // because nothing ran it. The direct install audit now follows each
    // expansion's manifest order; the cache-only audit remains useful as
    // an independent check but is refused when it is a foreign ruleset.
    // Moved again by reading the disasters' fertility table instead of
    // guessing it. `RandomEvent_Yields` in
    // `DLC/Expansion2/Data/Expansion2_RandomEvents.xml` rates each YIELD
    // TYPE apart, and `data/disasters.json` carried one rate for
    // "fertility" and applied it to Food alone: `river_flood` and
    // `blizzard` sat at zero although the table gives them 15-60% and
    // 10-20% Food, `volcanic_eruption`'s middle tier read 55% against the
    // shipped 50%, `hurricane` read 20/40 against 30/45 and `dust_storm`
    // 25/45 against 10/20 — and the `YIELD_PRODUCTION` half of every one of
    // those rows was not modelled at all, while `Tile::disaster_production`
    // was already being summed into every tile's yields. This is a
    // transcription of the shipped file, not a balance choice; the one
    // approximation is `river_flood`, whose three Floodplains features are
    // rated apart in the table and averaged into the single per-severity
    // rate `DisasterSpec` carries.
    // Moved again by `Features_XP2.Volcano`, which the audit had never
    // read. The column names four features, not one — the generic cone and
    // the three volcanic Natural Wonders, Vesuvius, Kilimanjaro and
    // Eyjafjallajokull — so those three had been scenery: never active,
    // never drawn by the eruption lottery, and never the source of a hex
    // of Volcanic Soil.
    // Moved again by completing the five Gathering Storm belief rows the
    // fidelity audit had left outside the model: Papal Primacy and
    // Stewardship (founder yields/pressure), Holy Waters and Monastic
    // Isolation (religious healing/combat pressure), and Warrior Monks
    // (faith purchase, Temple gate and Holy Site Culture Bomb). The
    // Warrior Monk production cost was corrected from 200 to the shipped
    // 100 at the same time; the Faith price remains the host's 200.
    // Moved again by giving Phoenicia its shipped Bireme replacement: the
    // 4-move, 35-strength Galley successor was visible in the installed
    // `Expansion2_Units_Major.xml` row but CIVVIS had been falling back to
    // the weaker generic Galley.
    // Moved again by modeling Gran Colombia's Llanero from the installed
    // `GranColombia_Maya_Units.xml` and `GranColombia_Maya_Expansion2.xml`
    // rows. The native host offers it for Horses, while an unmodeled
    // Cuirassier plan had no buildable land capturer for its bomber siege.
    // Moved again by completing the unique-unit roster from the installed
    // database: 32 shipped civilization and leader uniques (the Immortal,
    // Mamluk, Impi, Conquistador, Janissary, Redcoat, Garde Imperiale,
    // Huszar, Black Army, Hwacha, Domrey, Khevsureti, Berserker,
    // Carolean, Digger, Mountie, Hetairoi and their naval and air peers)
    // were absent, so those civilizations could never train theirs and
    // their rivals' read as the stock role. Rows are the loader's
    // `Units`/`UnitReplaces`/`UnitUpgrades`/`Units_XP2.ResourceCost`
    // values; abilities beyond the stat line are not modelled.
    // Moved again by the six shipped CanBuildOutsideTerritory flags:
    // Fort, Airstrip, Roman Fort, Mountain Tunnel, Pa and Qhapaq Nan.
    // The installed Gathering Storm audit agrees across all 29 tables.
    // Deity combat then moved from3 to the native preview's explicit +4;
    // docs/fidelity/native-deity-combat-2026-11-05.json preserves the readback.
    // Mongolia's flat combat_strength=3 proxy is removed in favor of Ortoo
    // visibility and Genghis-only cavalry strength in the combat code; the
    // native Kublai Bombard preview confirms +12 intel separately from +4
    // difficulty. Older ledgers retain the pre-correction rules fingerprint.
    // Moved again by Preslav's description, now its shipped Loyalty bonus.
    // Moved again by `Units.WMDCapable` on the Bomber, Jet Bomber and Nuclear
    // Submarine, which is what lets those three carry a device.
    assert_eq!(
        Rules::shipped().source_fingerprint(),
        "fnv1a64:1f5dad846d7a815b"
    );
}

#[test]
fn gathering_storm_unique_rows_match_the_installed_database() {
    let rules = Rules::shipped();
    let pike_and_shot = &rules.units["pike_and_shot"];
    assert_eq!(pike_and_shot.maintenance, 4.0);

    let tagma = &rules.units["tagma"];
    assert_eq!(tagma.cost, 220.0);
    assert_eq!(tagma.maintenance, 4.0);
    assert_eq!(tagma.upgrade_to.as_deref(), Some("cuirassier"));

    let llanero = &rules.units["llanero"];
    assert_eq!(
        (
            llanero.cost,
            llanero.maintenance,
            llanero.moves,
            llanero.strength
        ),
        (330.0, 2.0, 5.0, 62.0)
    );
    assert_eq!(llanero.tech, Some(crate::name!("military_science")));
    assert_eq!(llanero.requires_resource, Some(crate::name!("horses")));
    assert_eq!(llanero.resource_cost, 20.0);
    assert_eq!(llanero.unique_to.as_deref(), Some("Gran Colombia"));
    assert_eq!(llanero.replaces, Some(crate::name!("cavalry")));
    assert_eq!(llanero.upgrade_to, Some(crate::name!("helicopter")));

    let prasat = &rules.buildings["prasat"];
    assert_eq!(prasat.yields.faith, 6.0);
    assert_eq!(prasat.great_work_slots["relic"], 1);
    assert_eq!(rules.buildings["sukiennice"].yields.gold, 2.0);
    assert_eq!(rules.buildings["tlachtli"].yields.culture, 2.0);

    let eyjafjallajokull = &rules.features["eyjafjallajokull"];
    assert_eq!(eyjafjallajokull.adjacent_yields.food, 1.0);
}

#[test]
fn every_firaxis_improvement_type_has_a_ruleset_entry_or_named_alias() {
    // This is the game-data type inventory rather than a hand-picked list
    // of common Builder actions.  It catches a future addition that works
    // in one subsystem but is omitted from the shared terrain catalog.
    let type_names: Vec<String> =
        serde_json::from_str(include_str!("../../data/civ6_type_names.json")).unwrap();
    let improvement_types: Vec<_> = type_names
        .iter()
        .filter(|name| name.starts_with("IMPROVEMENT_"))
        .collect();
    assert_eq!(
        improvement_types.len(),
        72,
        "the audited Firaxis type inventory changed"
    );

    let rules = Rules::embedded();
    for type_name in improvement_types {
        let improvement = match type_name.as_str() {
            // CIVVIS uses the player-facing names for these three aliases.
            "IMPROVEMENT_BEACH_RESORT" => "seaside_resort".to_string(),
            "IMPROVEMENT_MOUNTAIN_ROAD" => "qhapaq_nan".to_string(),
            "IMPROVEMENT_PYRAMID" => "nubian_pyramid".to_string(),
            _ => type_name
                .strip_prefix("IMPROVEMENT_")
                .unwrap()
                .to_ascii_lowercase(),
        };
        assert!(
            rules.improvements.contains_key(improvement.as_str()),
            "{type_name} has no CIVVIS improvement entry ({improvement})"
        );
    }

    // The scenario and Secret Society markers are real serialized map
    // objects, but must not leak into the ordinary Builder menu.
    for improvement in [
        "ancient_tower_defense",
        "ancient_trap_defense",
        "buried_treasure",
        "floating_treasure",
        "grieving_gift",
        "improvised_trap",
        "modern_tower_defense",
        "modern_trap_defense",
        "popped_goody",
        "supply_drop",
        "vampire_castle",
    ] {
        assert!(
            rules.improvements[improvement].unbuildable,
            "scenario-only {improvement} must not become a standard Builder action"
        );
    }
}

#[test]
fn rules_fingerprint_is_stable_and_content_sensitive() {
    let stock = Rules::shipped();
    let repeated = Rules::shipped();
    assert_eq!(stock.source_fingerprint(), repeated.source_fingerprint());
    assert!(stock.source_fingerprint().starts_with("fnv1a64:"));

    let mut changed = Rules::shipped_values();
    changed.get_mut("speeds").unwrap()["standard"]["turns"] = json!(499);
    let changed = Rules::from_values(changed).unwrap();
    assert_ne!(stock.source_fingerprint(), changed.source_fingerprint());
}

const TECHS: &str = "
        pottery animal_husbandry mining sailing astrology irrigation archery writing masonry
        bronze_working wheel horseback_riding currency celestial_navigation iron_working
        shipbuilding mathematics construction engineering apprenticeship buttress machinery
        military_tactics stirrups castles education military_engineering banking cartography
        gunpowder mass_production printing astronomy metal_casting siege_tactics square_rigging
        ballistics industrialization military_science scientific_theory economics rifling
        sanitation steam_power flight refining replaceable_parts steel chemistry combustion
        electricity radio advanced_ballistics advanced_flight combined_arms plastics rocketry
        computers nuclear_fission synthetic_materials composites guidance_systems lasers
        satellites stealth_technology telecommunications nanotechnology nuclear_fusion robotics
        seasteads advanced_ai advanced_power_cells cybernetics smart_materials predictive_systems
        offworld_mission future_tech";

const CIVICS: &str = "
        code_of_laws craftsmanship foreign_trade military_tradition mysticism early_empire
        state_workforce drama_poetry games_recreation political_philosophy military_training
        theology defensive_tactics recorded_history naval_tradition civil_service feudalism
        divine_right mercenaries guilds medieval_faires exploration reformed_church
        diplomatic_service humanism mercantilism the_enlightenment colonialism opera_ballet
        civil_engineering nationalism natural_history scorched_earth urbanization conservation
        mass_media mobilization capitalism class_struggle ideology suffrage totalitarianism
        nuclear_program cultural_heritage cold_war professional_sports rapid_deployment
        space_race environmentalism globalization social_media digital_democracy
        synthetic_technocracy corporate_libertarianism near_future_governance
        information_warfare global_warming_mitigation cultural_hegemony smart_power_doctrine
        exodus_imperative future_civic";

const DISTRICTS: &str = "
        city_center campus holy_site commercial_hub harbor encampment theater_square
        industrial_zone entertainment_complex water_park aqueduct neighborhood canal dam
        aerodrome spaceport government_plaza diplomatic_quarter preserve observatory seowon
        acropolis lavra ikanda thanh suguba cothon royal_navy_dockyard hansa oppidum
        street_carnival hippodrome copacabana bath mbanza";

const BUILDINGS: &str = "
        airport alchemical_society amphitheater ancestral_hall aquarium aquatics_center
        archaeological_museum arena armory art_museum audience_chamber bank barracks
        basilikoi_paides broadcast_center cathedral chancery coal_power_plant consulate
        dar_e_mehr electronics_factory factory ferris_wheel film_studio flood_barrier
        food_market foreign_ministry gilded_vault granary grand_bazaar grand_masters_chapel
        grove gurdwara hangar hydroelectric_dam intelligence_agency library lighthouse madrasa
        marae market medieval_walls meeting_house military_academy monument mosque
        national_history_museum navigation_school nuclear_power_plant oil_power_plant
        old_god_obelisk ordu pagoda palace palgum prasat queens_bibliotheque renaissance_walls
        research_lab royal_society sanctuary seaport sewer shipyard shopping_mall shrine stable
        stadium stave_church stock_exchange stupa sukiennice synagogue temple thermal_bath
        tlachtli tsikhe university walls war_department warlords_throne wat water_mill workshop zoo";

const WONDERS: &str = "
        alhambra amundsen_scott_research_station angkor_wat apadana big_ben biosphere
        bolshoi_theatre broadway casa_de_contratacion chichen_itza colosseum colossus
        cristo_redentor eiffel_tower estadio_do_maracana etemenanki forbidden_city
        golden_gate_bridge great_bath great_library great_lighthouse great_zimbabwe hagia_sophia
        hanging_gardens hermitage huey_teocalli jebel_barkal kilwa_kisiwani kotoku_in
        machu_picchu mahabodhi_temple mausoleum_at_halicarnassus meenakshi_temple mont_st_michel
        oracle orszaghaz oxford_university panama_canal petra potala_palace pyramids ruhr_valley
        st_basils_cathedral statue_of_liberty statue_of_zeus stonehenge sydney_opera_house
        taj_mahal temple_artemis terracotta_army torre_de_belem university_of_sankore
        venetian_arsenal";

#[test]
fn named_modifiers_compose_and_attach_to_any_effect_bearing_spec() {
    let mut files = Rules::shipped_values_with(json!({
        "production_seed": {
            "effects": {"city_production": 2, "builder_production_pct": 12},
            "building_yields": {"library": {"science": 2}},
            "unit_purchase_discount_pct": {"builder": 15},
            "abilities": ["public_engineering"]
        },
        "production_bundle": {
            "effects": {"builder_production_pct": 8},
            "building_yields": {"library": {"science": 1}},
            "unit_purchase_discount_pct": {"builder": 5},
            "modifiers": ["production_seed"]
        }
    }));
    files.get_mut("policies").unwrap()["urban_planning"]["modifiers"] =
        json!(["production_bundle"]);

    let rules = Rules::from_values(files).unwrap();
    // Urban Planning already carries one city Production. Attached values
    // add to local values rather than silently replacing them.
    assert_eq!(
        rules.policies["urban_planning"].effects["city_production"],
        3.0
    );
    assert_eq!(
        rules.policies["urban_planning"].effects["builder_production_pct"],
        20.0
    );
    assert!(rules.modifiers["production_bundle"].modifiers.is_empty());
    assert_eq!(
        rules.modifiers["production_bundle"].effects["builder_production_pct"],
        20.0
    );
    assert_eq!(
        rules.modifiers["production_bundle"].effects
            [&building_yield_effect_key("library", "science")],
        3.0
    );
    assert_eq!(
        rules.modifiers["production_bundle"].effects[&unit_purchase_discount_effect_key("builder")],
        20.0
    );
    assert_eq!(
        rules.modifiers["production_bundle"].effects
            [&grant_ability_effect_key("public_engineering")],
        1.0
    );
    assert_eq!(
        rules.policies["urban_planning"].effects[&building_yield_effect_key("library", "science")],
        3.0
    );
    assert!(rules.modifiers["production_bundle"]
        .building_yields
        .is_empty());
    assert!(rules.modifiers["production_bundle"]
        .unit_purchase_discount_pct
        .is_empty());
    assert!(rules.modifiers["production_bundle"].abilities.is_empty());
}

#[test]
fn modifier_requirements_support_all_any_none_and_player_collections() {
    let requirements: ModifierRequirements = serde_json::from_value(json!({
        "all": [
            {"government": "democracy"},
            {"policy": "urban_planning"}
        ],
        "any": [
            {"religion": "catholicism"},
            {"pantheon": "religious_settlements"}
        ],
        "none": [{"age": "dark"}]
    }))
    .unwrap();
    let policies = BTreeSet::from([Name::new("urban_planning")]);
    let technologies = BTreeSet::new();
    let civics = BTreeSet::new();
    let context = ModifierContext {
        player_type: Some("major"),
        civilization: Some("Rome"),
        government: Some("democracy"),
        religion: Some("catholicism"),
        pantheon: None,
        secret_society: None,
        age: Some("normal"),
        policies: Some(&policies),
        technologies: Some(&technologies),
        civics: Some(&civics),
    };
    assert!(requirements.matches(&context));

    let dark = ModifierContext {
        age: Some("dark"),
        ..context
    };
    assert!(!requirements.matches(&dark));

    let files = Rules::shipped_values_with(json!({
        "city_bundle": {
            "collection": "player_cities",
            "requirements": {"all": [{"government": "democracy"}]},
            "effects": {"city_production": 4}
        }
    }));
    let rules = Rules::from_values(files).unwrap();
    assert_eq!(
        rules.modifiers["city_bundle"].collection,
        ModifierCollection::PlayerCities
    );
    assert_eq!(
        rules.modifiers["city_bundle"].requirements.all[0]
            .government
            .as_deref(),
        Some("democracy")
    );
}

#[test]
fn contextual_modifier_attachments_are_not_flattened_into_static_rules() {
    let mut files = Rules::shipped_values_with(json!({
        "conditional": {
            "requirements": {"all": [{"government": "democracy"}]},
            "effects": {"city_production": 4}
        }
    }));
    files.get_mut("policies").unwrap()["urban_planning"]["modifiers"] = json!(["conditional"]);
    let error = Rules::from_values(files).err().unwrap();
    assert!(
        error.contains("attaches contextual modifier conditional"),
        "{error}"
    );

    let invalid = Rules::shipped_values_with(json!({"bad": {"requirements": {"all": [{}]}}}));
    let error = Rules::from_values(invalid).err().unwrap();
    assert!(error.contains("empty all requirement"), "{error}");
}

#[test]
fn modifier_graph_rejects_dangling_references_and_cycles() {
    let mut dangling = Rules::shipped_values();
    dangling.insert(
        "modifiers".to_string(),
        json!({"outer": {"modifiers": ["missing"]}}),
    );
    let error = Rules::from_values(dangling).err().unwrap();
    assert!(
        error.contains("outer attaches missing modifier missing"),
        "{error}"
    );

    let mut cycle = Rules::shipped_values();
    cycle.insert(
        "modifiers".to_string(),
        json!({
            "first": {"modifiers": ["second"]},
            "second": {"modifiers": ["first"]}
        }),
    );
    let error = Rules::from_values(cycle).err().unwrap();
    assert!(
        error.contains("modifier attachment cycle: first -> second -> first"),
        "{error}"
    );
}

#[test]
fn rules_objects_cannot_attach_an_unknown_modifier() {
    let mut files = Rules::shipped_values();
    files.get_mut("policies").unwrap()["urban_planning"]["modifiers"] = json!(["missing"]);
    let error = Rules::from_values(files).err().unwrap();
    assert!(
        error.contains("policies.json.urban_planning attaches missing modifier missing"),
        "{error}"
    );
}

fn assert_complete_tree(tree: &SpecMap<TechSpec>, expected: &str, era_counts: [usize; 9]) {
    let actual: BTreeSet<&str> = tree.keys().map(|name| name.as_str()).collect();
    let expected: BTreeSet<&str> = expected.split_whitespace().collect();
    assert_eq!(actual, expected);

    let mut counts = [0; 9];
    for (name, spec) in tree {
        assert!(spec.cost > 0.0, "{name} has no research cost");
        assert!(
            spec.era < ERA_NAMES.len(),
            "{name} has invalid era {}",
            spec.era
        );
        counts[spec.era] += 1;
        for prerequisite in &spec.requires {
            let parent = tree
                .get(prerequisite)
                .unwrap_or_else(|| panic!("{name} requires missing node {prerequisite}"));
            assert!(
                parent.era <= spec.era,
                "{name} requires later-era node {prerequisite}"
            );
        }
    }
    assert_eq!(counts, era_counts);

    // Repeatedly remove nodes whose prerequisites have been removed. If
    // anything remains, the graph contains a cycle or an unreachable root.
    let mut reached = BTreeSet::new();
    while reached.len() < tree.len() {
        let before = reached.len();
        for (name, spec) in tree {
            if spec.requires.iter().all(|node| reached.contains(node)) {
                reached.insert(*name);
            }
        }
        assert!(reached.len() > before, "tree contains a dependency cycle");
    }
}

#[test]
fn gathering_storm_technology_and_civics_trees_are_complete() {
    let rules = Rules::embedded();
    assert_complete_tree(&rules.techs, TECHS, [11, 8, 8, 9, 8, 8, 8, 9, 8]);
    assert_complete_tree(&rules.civics, CIVICS, [7, 7, 7, 6, 7, 9, 5, 7, 6]);
}

fn assert_randomized_tree_shape(
    tree: &SpecMap<TechSpec>,
    ancestors: &SpecMap<BTreeSet<String>>,
    previous: &[&str],
    gateway: Option<&str>,
    terminal: &str,
) {
    let regular: Vec<&str> = tree
        .iter()
        .filter(|(_, spec)| spec.random_costs.len() == 2)
        .map(|(name, _)| name.as_str())
        .collect();
    let first: BTreeSet<&str> = regular
        .iter()
        .copied()
        .filter(|name| tree[*name].cost == tree[*name].random_costs[0])
        .collect();
    let second: BTreeSet<&str> = regular
        .iter()
        .copied()
        .filter(|name| tree[*name].cost == tree[*name].random_costs[1])
        .collect();
    assert_eq!(first.len() + second.len(), regular.len());
    assert!(first.len() >= 2 && first.len() < regular.len());
    assert!(!second.is_empty());

    let previous: BTreeSet<&str> = previous.iter().copied().collect();
    let first_parents: BTreeSet<&str> = first
        .iter()
        .flat_map(|name| tree[*name].requires.iter().map(|name| name.as_str()))
        .collect();
    assert_eq!(first_parents, previous);
    for name in &first {
        assert!(!tree[*name].requires.is_empty());
        assert!(tree[*name]
            .requires
            .iter()
            .all(|parent| previous.contains(parent.as_str())));
    }

    let second_parents: BTreeSet<&str> = second
        .iter()
        .flat_map(|name| tree[*name].requires.iter().map(|name| name.as_str()))
        .collect();
    assert_eq!(second_parents, first);
    for name in &second {
        assert!(!tree[*name].requires.is_empty());
        assert!(tree[*name]
            .requires
            .iter()
            .all(|parent| first.contains(parent.as_str())));
    }

    let second_owned: BTreeSet<Name> = second.iter().map(|name| Name::new(name)).collect();
    match gateway {
        Some(gateway) => {
            assert_eq!(
                tree[gateway]
                    .requires
                    .iter()
                    .cloned()
                    .collect::<BTreeSet<_>>(),
                second_owned
            );
            assert_eq!(tree[terminal].requires, [Name::new(gateway)]);
        }
        None => assert_eq!(
            tree[terminal]
                .requires
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>(),
            second_owned
        ),
    }
    for node in previous
        .iter()
        .copied()
        .chain(regular.iter().copied())
        .chain(gateway)
    {
        assert!(
            ancestors[terminal].contains(node),
            "{terminal} does not descend from {node}"
        );
    }
}

#[test]
fn future_research_trees_use_the_shipped_two_column_shape() {
    let base = Rules::embedded();
    assert!(base
        .techs
        .iter()
        .filter(|(_, spec)| spec.random_prereqs)
        .all(|(_, spec)| spec.requires.is_empty()));
    assert!(base
        .civics
        .iter()
        .filter(|(_, spec)| spec.random_prereqs)
        .all(|(_, spec)| spec.requires.is_empty()));

    let mut layouts = BTreeSet::new();
    for seed in 0..64 {
        let rules = Rules::for_game(seed, None, crate::setup::FutureEra::Classic);
        assert_randomized_tree_shape(
            &rules.techs,
            &rules.tech_ancestors,
            &[
                "nanotechnology",
                "nuclear_fusion",
                "robotics",
                "stealth_technology",
                "telecommunications",
            ],
            Some("offworld_mission"),
            "future_tech",
        );
        assert_randomized_tree_shape(
            &rules.civics,
            &rules.civic_ancestors,
            &[
                "corporate_libertarianism",
                "digital_democracy",
                "near_future_governance",
                "synthetic_technocracy",
            ],
            None,
            "future_civic",
        );
        let layout = rules.future_tree_layout();
        let restored = Rules::for_game(seed, Some(&layout), crate::setup::FutureEra::Classic);
        assert_eq!(restored.future_tree_layout(), layout);
        layouts.insert(format!("{layout:?}"));
    }
    assert!(
        layouts.len() > 32,
        "64 seeds produced only {} Future-tree layouts",
        layouts.len()
    );
}

#[test]
fn malformed_saved_future_tree_is_rejected_before_installation() {
    let mut rules = Rules::embedded();
    let mut layout = FutureTreeLayout::generate(&rules, 919_191).unwrap();
    let first = layout
        .techs
        .iter()
        .find(|(name, entry)| {
            !rules.techs[*name].random_costs.is_empty()
                && entry.cost == rules.techs[*name].random_costs[0]
        })
        .map(|(name, _)| *name)
        .unwrap();
    layout.techs.get_mut(&first).unwrap().requires = vec![crate::name!("future_tech")];
    let error = apply_tree_layout("technology", &mut rules.techs, &layout.techs).unwrap_err();
    assert!(error.contains("outside the preceding column"), "{error}");

    let mut layout = FutureTreeLayout::generate(&rules, 919_191).unwrap();
    let prerequisite = layout.techs[&first].requires[0];
    layout
        .techs
        .get_mut(&first)
        .unwrap()
        .requires
        .push(prerequisite);
    let error = apply_tree_layout("technology", &mut rules.techs, &layout.techs).unwrap_err();
    assert!(error.contains("repeats a prerequisite"), "{error}");
}

#[test]
fn gathering_storm_unit_upgrade_graph_is_complete_and_acyclic() {
    let rules = Rules::embedded();
    let expected: BTreeSet<(&str, &str)> = [
        ("scout", "skirmisher"),
        ("skirmisher", "ranger"),
        ("ranger", "spec_ops"),
        ("warrior", "swordsman"),
        ("swordsman", "man_at_arms"),
        ("man_at_arms", "musketman"),
        ("musketman", "line_infantry"),
        ("line_infantry", "infantry"),
        ("infantry", "mechanized_infantry"),
        ("slinger", "archer"),
        ("archer", "crossbowman"),
        ("crossbowman", "field_cannon"),
        ("keshig", "field_cannon"),
        ("field_cannon", "machine_gun"),
        ("spearman", "pikeman"),
        ("pikeman", "pike_and_shot"),
        ("pike_and_shot", "at_crew"),
        ("at_crew", "modern_at"),
        ("horseman", "courser"),
        ("courser", "cavalry"),
        ("oromo_cavalry", "cavalry"),
        ("cavalry", "helicopter"),
        ("llanero", "helicopter"),
        ("heavy_chariot", "knight"),
        ("knight", "cuirassier"),
        ("tagma", "cuirassier"),
        ("cuirassier", "tank"),
        ("tank", "modern_armor"),
        ("catapult", "trebuchet"),
        ("trebuchet", "bombard"),
        ("bombard", "artillery"),
        ("artillery", "rocket_artillery"),
        ("battering_ram", "siege_tower"),
        ("siege_tower", "medic"),
        ("medic", "supply_convoy"),
        ("observation_balloon", "drone"),
        ("anti_air_gun", "mobile_sam"),
        ("bireme", "caravel"),
        ("galley", "caravel"),
        ("caravel", "ironclad"),
        ("ironclad", "destroyer"),
        ("quadrireme", "frigate"),
        ("frigate", "battleship"),
        ("battleship", "missile_cruiser"),
        ("privateer", "submarine"),
        ("submarine", "nuclear_submarine"),
        ("biplane", "fighter"),
        ("fighter", "jet_fighter"),
        ("bomber", "jet_bomber"),
        ("legion", "man_at_arms"),
        ("toa", "man_at_arms"),
        ("nau", "ironclad"),
        ("kongo_shield_bearer", "man_at_arms"),
        ("hoplite", "pikeman"),
        ("eagle_warrior", "swordsman"),
        ("war_cart", "knight"),
        ("pitati_archer", "crossbowman"),
        ("maryannu_chariot_archer", "crossbowman"),
        ("saka_horse_archer", "crossbowman"),
        ("winged_hussar", "tank"),
        ("crouching_tiger", "field_cannon"),
        ("cossack", "helicopter"),
        ("gaesatae", "man_at_arms"),
        ("hypaspist", "man_at_arms"),
        ("mandekalu_cavalry", "cuirassier"),
        ("rough_rider", "tank"),
        ("samurai", "musketman"),
        ("varu", "cuirassier"),
        ("voi_chien", "field_cannon"),
        ("mamluk", "cuirassier"),
        ("digger", "mechanized_infantry"),
        ("sabum_kibittum", "swordsman"),
        ("minas_geraes", "missile_cruiser"),
        ("dromon", "frigate"),
        ("okihtcitaw", "skirmisher"),
        ("viking_longship", "caravel"),
        ("sea_dog", "submarine"),
        ("redcoat", "infantry"),
        ("garde_imperiale", "infantry"),
        ("khevsureti", "musketman"),
        ("u_boat", "nuclear_submarine"),
        ("huszar", "helicopter"),
        ("black_army", "cavalry"),
        ("warakaq", "ranger"),
        ("jong", "battleship"),
        ("domrey", "bombard"),
        ("hwacha", "machine_gun"),
        ("hetairoi", "knight"),
        ("malon_raider", "cavalry"),
        ("hulche", "crossbowman"),
        ("de_zeven_provincien", "battleship"),
        ("berserker", "musketman"),
        ("barbary_corsair", "submarine"),
        ("janissary", "line_infantry"),
        ("immortal", "man_at_arms"),
        ("highlander", "spec_ops"),
        ("conquistador", "line_infantry"),
        ("carolean", "at_crew"),
        ("impi", "pike_and_shot"),
        ("p51_mustang", "jet_fighter"),
    ]
    .into_iter()
    .collect();
    let actual: BTreeSet<(&str, &str)> = rules
        .units
        .iter()
        .filter_map(|(unit, spec)| {
            spec.upgrade_to
                .as_deref()
                .map(|target| (unit.as_str(), target))
        })
        .collect();
    assert_eq!(actual, expected);

    for (source, target) in &actual {
        let target_spec = rules
            .units
            .get(target)
            .unwrap_or_else(|| panic!("{source} upgrades to missing unit {target}"));
        assert!(
            target_spec.buildable,
            "{source} upgrades to unbuildable {target}"
        );
        let mut seen = BTreeSet::new();
        let mut cursor = Some(*source);
        while let Some(unit) = cursor {
            assert!(seen.insert(unit), "unit upgrade cycle reaches {unit}");
            cursor = rules.units[unit].upgrade_to.as_deref();
        }
    }
}

#[test]
fn every_tree_unlock_is_present_gated_and_runtime_indexed() {
    let rules = Rules::embedded();
    assert_eq!(rules.techs.len(), 77);
    assert_eq!(rules.civics.len(), 61);
    assert_eq!(rules.units.len(), 134);
    assert_eq!(rules.buildings.len(), 85);
    assert_eq!(rules.districts.len(), 35);
    assert_eq!(rules.wonders.len(), 53);
    assert_eq!(rules.improvements.len(), 76);
    assert_eq!(rules.resources.len(), 52);
    assert_eq!(rules.projects.len(), 31);
    let aid = rules
        .projects
        .get(&crate::name!("send_aid"))
        .expect("the Gathering Storm Aid Request project is modeled");
    assert_eq!(aid.cost, 200.0);
    assert_eq!(
        aid.host_competition_kinds().collect::<Vec<_>>(),
        ["EMERGENCY_SEND_AID", "EMERGENCY_SEND_MILITARY_AID"]
    );
    assert_eq!(aid.competition_score, 200.0);
    assert!(aid.repeatable);
    for (project, power_plant) in [
        ("decommission_coal_power_plant", "coal_power_plant"),
        ("decommission_oil_power_plant", "oil_power_plant"),
        ("decommission_nuclear_power_plant", "nuclear_power_plant"),
    ] {
        let decommission = rules
            .projects
            .get(project)
            .expect("the Gathering Storm Climate Accords project is modeled");
        assert_eq!(decommission.cost, 400.0, "{project} costs 400 Production");
        assert_eq!(decommission.district.as_deref(), Some("industrial_zone"));
        assert_eq!(
            decommission.host_competition.as_deref(),
            Some("EMERGENCY_CLIMATE_ACCORDS")
        );
        assert_eq!(
            decommission
                .consumes_buildings
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            [power_plant]
        );
        assert_eq!(decommission.competition_score, 100.0);
        assert!(decommission.repeatable);
    }
    let athletes = rules
        .projects
        .get(&crate::name!("train_athletes"))
        .expect("the Gathering Storm World Games project is modeled");
    assert_eq!(athletes.cost, 200.0);
    assert_eq!(
        athletes.host_competition.as_deref(),
        Some("EMERGENCY_WORLD_GAMES")
    );
    assert_eq!(athletes.competition_score, 50.0);
    assert!(athletes.repeatable);
    let astronauts = rules
        .projects
        .get(&crate::name!("train_astronauts"))
        .expect("the Gathering Storm International Space Station project is modeled");
    assert_eq!(astronauts.cost, 200.0);
    assert_eq!(astronauts.district.as_deref(), Some("spaceport"));
    assert_eq!(
        astronauts.host_competition.as_deref(),
        Some("EMERGENCY_SPACE_STATION")
    );
    assert_eq!(astronauts.competition_score, 30.0);
    assert!(astronauts.repeatable);
    // 118 civic-unlocked cards plus the eleven Dark Age cards
    // (`Policies_XP1` RequiresDarkAge = 1), which no civic unlocks — a
    // Dark Age is what puts them on offer.
    assert_eq!(rules.policies.len(), 129);
    assert_eq!(
        rules.policies.values().filter(|spec| spec.dark_age).count(),
        11
    );
    assert_eq!(rules.governments.len(), 13);

    let check_gate = |kind: &str, id: &str, tech: &Option<Name>, civic: &Option<Name>| {
        if let Some(node) = tech {
            let spec = rules
                .techs
                .get(node)
                .unwrap_or_else(|| panic!("{kind} {id} references missing technology {node}"));
            assert!(
                spec.unlocks
                    .iter()
                    .any(|unlock| unlock.kind == kind && unlock.id == id),
                "technology {node} does not index {kind} {id}"
            );
        }
        if let Some(node) = civic {
            let spec = rules
                .civics
                .get(node)
                .unwrap_or_else(|| panic!("{kind} {id} references missing civic {node}"));
            assert!(
                spec.unlocks
                    .iter()
                    .any(|unlock| unlock.kind == kind && unlock.id == id),
                "civic {node} does not index {kind} {id}"
            );
        }
    };

    for (id, spec) in &rules.units {
        check_gate("unit", id, &spec.tech, &spec.civic);
        assert!(
            spec.maintenance >= 0.0,
            "{id} has negative Gold maintenance"
        );
        if let Some(resource) = &spec.requires_resource {
            assert!(
                rules.resources.contains_key(resource),
                "{id} needs {resource}"
            );
            assert!(
                spec.resource_cost > 0.0,
                "{id} must define its Gathering Storm {resource} cost"
            );
            assert!(
                spec.resource_maintenance >= 0.0,
                "{id} has negative {resource} maintenance"
            );
        } else {
            assert_eq!(
                spec.resource_cost, 0.0,
                "{id} has a cost without a resource"
            );
            assert_eq!(
                spec.resource_maintenance, 0.0,
                "{id} has maintenance without a resource"
            );
        }
        if let Some(building) = &spec.requires_building {
            assert!(
                rules.buildings.contains_key(building),
                "{id} needs {building}"
            );
        }
        if let Some(district) = &spec.requires_district {
            assert!(
                rules.districts.contains_key(district),
                "{id} needs {district}"
            );
        }
        for improvement in &spec.builds {
            assert!(
                rules.improvements.contains_key(improvement),
                "{id} builds missing improvement {improvement}"
            );
        }
    }
    for (id, spec) in &rules.buildings {
        check_gate("building", id, &spec.tech, &spec.civic);
        assert!(
            spec.maintenance >= 0.0,
            "{id} has negative Gold maintenance"
        );
    }
    for (id, spec) in &rules.districts {
        check_gate("district", id, &spec.tech, &spec.civic);
        assert!(
            spec.maintenance >= 0.0,
            "{id} has negative Gold maintenance"
        );
    }
    for (id, spec) in &rules.wonders {
        check_gate("wonder", id, &spec.tech, &spec.civic);
    }
    for (id, spec) in &rules.improvements {
        check_gate("improvement", id, &spec.tech, &spec.civic);
        for resource in &spec.resources {
            assert!(
                rules.resources.contains_key(resource),
                "{id} references missing resource {resource}"
            );
        }
    }
    for (id, spec) in &rules.resources {
        check_gate("resource", id, &spec.tech, &spec.civic);
        if !spec.improvement.is_empty() {
            assert!(
                rules.improvements.contains_key(&spec.improvement),
                "{id} references missing improvement {}",
                spec.improvement
            );
        }
    }
    for (id, spec) in &rules.projects {
        check_gate("project", id, &spec.tech, &spec.civic);
        if let Some(district) = &spec.district {
            assert!(
                rules.districts.contains_key(district),
                "{id} needs {district}"
            );
        }
        for prerequisite in &spec.requires {
            assert!(
                rules.projects.contains_key(prerequisite),
                "{id} requires missing project {prerequisite}"
            );
        }
        for building in spec
            .requires_buildings
            .iter()
            .chain(&spec.consumes_buildings)
        {
            assert!(
                rules.buildings.contains_key(building),
                "{id} requires missing building {building}"
            );
        }
    }
    for (id, spec) in &rules.policies {
        check_gate("policy", id, &None, &spec.civic);
        assert!(
            matches!(
                spec.slot.as_str(),
                "military" | "economic" | "diplomatic" | "wildcard"
            ),
            "{id} has invalid slot {}",
            spec.slot
        );
        assert!(
            !spec.effects.is_empty(),
            "policy {id} has no runtime effect"
        );
        for replaced in &spec.replaces {
            assert!(
                rules.policies.contains_key(replaced),
                "{id} replaces missing policy {replaced}"
            );
        }
    }
    for (id, spec) in &rules.governments {
        check_gate("government", id, &None, &spec.civic);
        let slots =
            spec.slots.military + spec.slots.economic + spec.slots.diplomatic + spec.slots.wildcard;
        assert!(slots > 0, "government {id} has no policy slots");
    }

    for (kind, tree) in [("technology", &rules.techs), ("civic", &rules.civics)] {
        for (node, spec) in tree {
            assert!(
                !spec.unlocks.is_empty() || !spec.effects.is_empty(),
                "{kind} {node} has neither a content unlock nor a runtime ability"
            );
            for unlock in &spec.unlocks {
                let gate = match unlock.kind.as_str() {
                    "unit" => rules.units[&unlock.id]
                        .tech
                        .as_ref()
                        .or(rules.units[&unlock.id].civic.as_ref()),
                    "building" => rules.buildings[&unlock.id]
                        .tech
                        .as_ref()
                        .or(rules.buildings[&unlock.id].civic.as_ref()),
                    "district" => rules.districts[&unlock.id]
                        .tech
                        .as_ref()
                        .or(rules.districts[&unlock.id].civic.as_ref()),
                    "wonder" => rules.wonders[&unlock.id]
                        .tech
                        .as_ref()
                        .or(rules.wonders[&unlock.id].civic.as_ref()),
                    "improvement" => rules.improvements[&unlock.id]
                        .tech
                        .as_ref()
                        .or(rules.improvements[&unlock.id].civic.as_ref()),
                    "resource" => rules.resources[&unlock.id]
                        .tech
                        .as_ref()
                        .or(rules.resources[&unlock.id].civic.as_ref()),
                    "project" => rules.projects[&unlock.id]
                        .tech
                        .as_ref()
                        .or(rules.projects[&unlock.id].civic.as_ref()),
                    "policy" => rules.policies[&unlock.id].civic.as_ref(),
                    "government" => rules.governments[&unlock.id].civic.as_ref(),
                    other => panic!("{node} indexes unknown unlock kind {other}"),
                };
                assert_eq!(gate.map(|name| name.as_str()), Some(node.as_str()));
            }
        }
    }
}

#[test]
fn gathering_storm_district_building_and_wonder_rosters_are_complete_and_linked() {
    let rules = Rules::embedded();
    fn expected(names: &str) -> BTreeSet<&str> {
        names.split_whitespace().collect()
    }
    assert_eq!(
        rules
            .districts
            .keys()
            .map(|name| name.as_str())
            .collect::<BTreeSet<_>>(),
        expected(DISTRICTS)
    );
    assert_eq!(
        rules
            .buildings
            .keys()
            .map(|name| name.as_str())
            .collect::<BTreeSet<_>>(),
        expected(BUILDINGS)
    );
    assert_eq!(
        rules
            .wonders
            .keys()
            .map(|name| name.as_str())
            .collect::<BTreeSet<_>>(),
        expected(WONDERS)
    );

    for (name, district) in &rules.districts {
        assert!(district.cost >= 0.0, "{name} has a negative cost");
        if let Some(tech) = &district.tech {
            assert!(
                rules.techs.contains_key(tech),
                "{name} has missing tech {tech}"
            );
        }
        if let Some(civic) = &district.civic {
            assert!(
                rules.civics.contains_key(civic),
                "{name} has missing civic {civic}"
            );
        }
        if let Some(base) = &district.replaces {
            assert!(
                rules.districts.contains_key(base),
                "{name} replaces missing {base}"
            );
            assert!(
                district.unique_to.is_some(),
                "{name} replacement is not unique"
            );
        }
        for excluded in &district.excludes {
            assert!(
                rules.districts.contains_key(excluded),
                "{name} excludes missing {excluded}"
            );
        }
    }

    for (name, building) in &rules.buildings {
        assert!(building.cost > 0.0, "{name} has no cost");
        if let Some(tech) = &building.tech {
            assert!(
                rules.techs.contains_key(tech),
                "{name} has missing tech {tech}"
            );
        }
        if let Some(civic) = &building.civic {
            assert!(
                rules.civics.contains_key(civic),
                "{name} has missing civic {civic}"
            );
        }
        if let Some(district) = &building.district {
            assert!(
                rules.districts.contains_key(district),
                "{name} has missing district {district}"
            );
        }
        for required in building.requires.iter().chain(&building.requires_any) {
            assert!(
                rules.buildings.contains_key(required),
                "{name} requires missing {required}"
            );
        }
        for excluded in &building.excludes {
            assert!(
                rules.buildings.contains_key(excluded),
                "{name} excludes missing {excluded}"
            );
        }
        if let Some(base) = &building.replaces {
            assert!(
                rules.buildings.contains_key(base),
                "{name} replaces missing {base}"
            );
        }
        assert!(
            !building.wonder,
            "{name} must be modeled as a map-placed wonder"
        );
    }

    for (name, wonder) in &rules.wonders {
        assert!(wonder.cost > 0.0, "{name} has no cost");
        if let Some(tech) = &wonder.tech {
            assert!(
                rules.techs.contains_key(tech),
                "{name} has missing tech {tech}"
            );
        }
        if let Some(civic) = &wonder.civic {
            assert!(
                rules.civics.contains_key(civic),
                "{name} has missing civic {civic}"
            );
        }
        if let Some(district) = &wonder.adjacent_district {
            assert!(
                rules.districts.contains_key(district),
                "{name} has missing adjacent district {district}"
            );
        }
        for required in wonder
            .requires_buildings
            .iter()
            .chain(&wonder.requires_any_buildings)
        {
            assert!(
                rules.buildings.contains_key(required),
                "{name} requires missing {required}"
            );
        }
        for terrain in &wonder.terrain {
            assert!(
                rules.terrains.contains_key(terrain),
                "{name} has missing terrain {terrain}"
            );
        }
        for feature in &wonder.feature {
            assert!(
                rules.features.contains_key(feature),
                "{name} has missing feature {feature}"
            );
        }
        if let Some(resource) = &wonder.adjacent_resource {
            assert!(
                rules.resources.contains_key(resource),
                "{name} has missing resource {resource}"
            );
        }
        if let Some(improvement) = &wonder.adjacent_improvement {
            assert!(
                rules.improvements.contains_key(improvement),
                "{name} has missing improvement {improvement}"
            );
        }
    }
}

#[test]
fn modeled_unit_classes_have_complete_promotion_trees() {
    let rules = Rules::embedded();
    let classes: BTreeSet<_> = rules
        .units
        .values()
        // Spy promotions are resolved by the off-map espionage engine,
        // not the seven-node map-unit XP trees validated here.
        .filter(|unit| !unit.promotion_class.is_empty() && unit.promotion_class != "espionage")
        .map(|unit| unit.promotion_class.as_str())
        .collect();
    let promotion_count = |class: &str| match class {
        "religious_apostle" => 9,
        "rock_band" => 12,
        _ => 7,
    };
    let expected_promotions = classes
        .iter()
        .map(|class| promotion_count(class))
        .sum::<usize>();
    // The espionage class is counted separately below: it is a flat list of
    // seventeen, not a seven-node XP tree, so it cannot be folded into the
    // per-class totals this assertion sums.
    assert_eq!(
        rules
            .promotions
            .values()
            .filter(|promotion| promotion.class != "espionage")
            .count(),
        expected_promotions,
        "modeled promotion classes: {classes:?}"
    );
    for class in classes {
        let nodes: Vec<_> = rules
            .promotions
            .iter()
            .filter(|(_, promotion)| promotion.class == class)
            .collect();
        let expected = promotion_count(class);
        assert_eq!(nodes.len(), expected, "{class} promotion tree");
        for (name, promotion) in nodes {
            assert!((1..=4).contains(&promotion.tier), "{name} tier");
            for prerequisite in &promotion.requires {
                let required = rules
                    .promotions
                    .get(prerequisite)
                    .unwrap_or_else(|| panic!("{name} requires missing {prerequisite}"));
                assert_eq!(required.class, class, "{name} crosses unit classes");
                assert!(required.tier <= promotion.tier, "{name} prerequisite tier");
            }
            assert!(
                promotion.requires.is_empty()
                    || promotion.requires.iter().any(|prerequisite| {
                        rules.promotions[prerequisite].tier < promotion.tier
                    }),
                "{name} has no prerequisite from an earlier tier"
            );
        }
    }
}

/// The Spy's tree is flat, and Civ VI says so.
///
/// `UnitPromotions` gives all seventeen `PROMOTION_CLASS_SPY` rows
/// `Level = 1` and ships no `UnitPromotionPrereqs` for any of them: a Spy
/// picks three of the seventeen in any order as it levels, which is why the
/// tier/prerequisite assertions above cannot describe it. Guarding the
/// shape here keeps that difference deliberate rather than an omission.
#[test]
fn espionage_promotions_are_a_flat_seventeen_node_class() {
    let rules = Rules::embedded();
    let nodes: Vec<_> = rules
        .promotions
        .iter()
        .filter(|(_, promotion)| promotion.class == "espionage")
        .collect();
    assert_eq!(nodes.len(), 17, "espionage promotion class");
    for (name, promotion) in nodes {
        assert_eq!(promotion.tier, 1, "{name} tier");
        assert!(promotion.requires.is_empty(), "{name} prerequisites");
        assert!(!promotion.effects.is_empty(), "{name} has no effect");
    }
}

/// Civ VI splits its Natural Wonders cleanly: a wonder a Citizen can stand
/// on pays tile yields, and one that is Impassable cannot be worked at all,
/// so it pays its neighbours instead — or, for Ik-Kil and Zhangye Danxia,
/// nothing but Appeal. Six wonders used to be passable *and* pay tile
/// yields nobody in the shipped game ever collects.
///
/// The Bermuda Triangle is the shipped exception on the other side, and it
/// is deliberate rather than sloppy: its plots are ordinary Ocean a Citizen
/// *can* work, it pays nothing for standing there, and its whole yield is
/// the +5 Science it gives every neighbour. `Feature_UnitMovements` bars
/// passage through it while allowing a unit to end on it, which is a third
/// state neither half of the split describes.
#[test]
fn an_impassable_natural_wonder_pays_its_neighbours_not_its_own_tile() {
    let rules = Rules::embedded();
    let wonders = rules
        .features
        .iter()
        .filter(|(_, spec)| spec.natural_wonder);
    let mut passable = 0;
    let mut blocked = 0;
    for (name, spec) in wonders {
        if spec.impassable {
            assert_eq!(
                spec.yields,
                Yields::default(),
                "{name} is impassable, so no Citizen can ever work its tile"
            );
            blocked += 1;
        } else if name == "bermuda_triangle" {
            assert_eq!(spec.yields, Yields::default(), "{name} pays no tile yield");
            assert_ne!(
                spec.adjacent_yields,
                Yields::default(),
                "{name} exists to pay its neighbours"
            );
            passable += 1;
        } else {
            assert_ne!(
                spec.yields,
                Yields::default(),
                "{name} is workable and must be worth working"
            );
            assert_eq!(
                spec.adjacent_yields,
                Yields::default(),
                "{name} is workable, so the shipped data pays its own tile"
            );
            passable += 1;
        }
    }
    assert!(
        passable > 0 && blocked > 0,
        "both halves of the split must be represented"
    );
}

/// A natural wonder's tile pays the wonder and nothing from the ground under
/// it. Ubsunur Hollow sits on Tundra: the host reads its tiles at exactly the
/// feature's 1 Food, 1 Production, 2 Faith (389 worked-tile-turns on run
/// civvis-20260816T040537Z), never Tundra's Food on top.
#[test]
fn a_natural_wonders_tile_pays_the_wonder_not_the_terrain_under_it() {
    let rules = Rules::embedded();
    let mut hollow = Tile::new((0, 0));
    hollow.terrain = Name::new("tundra");
    hollow.feature = Some(Name::new("ubsunur_hollow"));
    assert_eq!(
        rules.worked_tile_yields(&hollow),
        rules.features["ubsunur_hollow"].yields
    );
    // Hills under a wonder add nothing either; ordinary hills still do.
    let mut hilly_hollow = hollow.clone();
    hilly_hollow.hills = true;
    assert_eq!(
        rules.worked_tile_yields(&hilly_hollow),
        rules.features["ubsunur_hollow"].yields
    );
    let mut plain_hills = Tile::new((0, 0));
    plain_hills.terrain = Name::new("tundra");
    plain_hills.hills = true;
    assert_eq!(
        rules.worked_tile_yields(&plain_hills).production,
        rules.terrains["tundra"].yields.production + 1.0
    );
    // The catalogue sum map generation reads keeps its additive shape.
    assert_eq!(
        rules.tile_yields(&hollow).food,
        rules.terrains["tundra"].yields.food + 1.0
    );
}

/// Complete transcription of the placement columns in the shipped
/// `Features`, `Feature_*Terrains`, and `Feature_*Features` tables, plus
/// the layouts in `NaturalWonderGenerator.lua`. This is deliberately an
/// inventory test: a partial sample would let an untested wonder silently
/// fall back to a connected line again.
#[test]
fn every_natural_wonder_has_its_shipped_shape_and_constraints() {
    let rules = Rules::embedded();
    let expected: BTreeMap<&str, &str> = [
            ("great_barrier_reef", "tiles=2;shape=adjacent;terrain=coast;mountain=;hills=either;water=0;adjacent=;not_adjacent=;adjacent_feature=;avoid=ice;bare=false;no_river=true;land=1,1"),
            ("crater_lake", "tiles=1;shape=single;terrain=plains,tundra;mountain=;hills=flat;water=0;adjacent=;not_adjacent=coast;adjacent_feature=;avoid=;bare=false;no_river=true;land="),
            ("pantanal", "tiles=4;shape=diamond;terrain=grassland,plains;mountain=;hills=flat;water=0;adjacent=;not_adjacent=coast,snow;adjacent_feature=;avoid=;bare=false;no_river=true;land="),
            ("uluru", "tiles=1;shape=single;terrain=desert;mountain=;hills=either;water=0;adjacent=;not_adjacent=coast,grassland,plains,tundra,snow,mountain;adjacent_feature=;avoid=;bare=false;no_river=true;land="),
            ("yosemite", "tiles=2;shape=adjacent;terrain=plains,tundra;mountain=;hills=flat;water=0;adjacent=;not_adjacent=coast,mountain;adjacent_feature=forest;avoid=;bare=false;no_river=true;land="),
            ("dead_sea", "tiles=2;shape=adjacent;terrain=grassland,desert;mountain=;hills=flat;water=0;adjacent=;not_adjacent=coast,mountain;adjacent_feature=;avoid=;bare=true;no_river=true;land="),
            ("mount_everest", "tiles=3;shape=triangle;terrain=mountain;mountain=grassland,plains,desert,tundra;hills=either;water=0;adjacent=grassland,plains,desert,snow,tundra;not_adjacent=coast;adjacent_feature=;avoid=;bare=false;no_river=true;land="),
            ("pamukkale", "tiles=2;shape=adjacent;terrain=grassland,plains,desert;mountain=;hills=either;water=0;adjacent=;not_adjacent=coast;adjacent_feature=;avoid=;bare=false;no_river=true;land="),
            ("torres_del_paine", "tiles=2;shape=adjacent;terrain=grassland,plains,tundra;mountain=;hills=either;water=0;adjacent=;not_adjacent=coast,desert,snow,mountain;adjacent_feature=;avoid=;bare=false;no_river=true;land="),
            ("eye_of_the_sahara", "tiles=3;shape=triangle;terrain=desert;mountain=;hills=either;water=0;adjacent=;not_adjacent=coast;adjacent_feature=;avoid=;bare=false;no_river=true;land="),
            ("zhangye_danxia", "tiles=3;shape=straight;terrain=mountain;mountain=grassland,plains,desert,tundra,snow;hills=either;water=0;adjacent=;not_adjacent=coast;adjacent_feature=;avoid=;bare=false;no_river=true;land="),
            ("ha_long_bay", "tiles=2;shape=adjacent;terrain=coast;mountain=;hills=either;water=0;adjacent=;not_adjacent=;adjacent_feature=;avoid=ice;bare=false;no_river=false;land=1,1"),
            ("cliffs_of_dover", "tiles=2;shape=adjacent;terrain=grassland,plains;mountain=;hills=hills;water=0;adjacent=coast;not_adjacent=;adjacent_feature=;avoid=;bare=false;no_river=true;land="),
            ("giants_causeway", "tiles=2;shape=adjacent;terrain=grassland,plains;mountain=;hills=flat;water=1;adjacent=coast;not_adjacent=;adjacent_feature=;avoid=;bare=false;no_river=true;land="),
            ("galapagos_islands", "tiles=2;shape=adjacent;terrain=coast;mountain=;hills=either;water=0;adjacent=;not_adjacent=;adjacent_feature=;avoid=ice;bare=false;no_river=true;land=2,3"),
            ("matterhorn", "tiles=1;shape=single;terrain=mountain;mountain=grassland,plains;hills=either;water=0;adjacent=grassland,plains,desert,snow,tundra;not_adjacent=coast;adjacent_feature=;avoid=;bare=false;no_river=true;land="),
            ("kilimanjaro", "tiles=1;shape=single;terrain=mountain;mountain=grassland,plains,desert,tundra;hills=either;water=0;adjacent=;not_adjacent=coast,mountain;adjacent_feature=;avoid=;bare=false;no_river=true;land="),
            ("piopiotahi", "tiles=3;shape=coastal_triangle;terrain=grassland,plains;mountain=;hills=either;water=0;adjacent=;not_adjacent=;adjacent_feature=;avoid=;bare=false;no_river=true;land="),
            ("ik_kil", "tiles=1;shape=single;terrain=grassland,plains;mountain=;hills=either;water=0;adjacent=;not_adjacent=coast;adjacent_feature=jungle;avoid=;bare=false;no_river=true;land="),
            ("gobustan", "tiles=3;shape=triangle;terrain=plains,mountain;mountain=plains;hills=either;water=0;adjacent=;not_adjacent=coast;adjacent_feature=;avoid=;bare=false;no_river=true;land="),
            ("ubsunur_hollow", "tiles=4;shape=diamond;terrain=tundra;mountain=;hills=either;water=0;adjacent=;not_adjacent=coast;adjacent_feature=;avoid=;bare=false;no_river=true;land="),
            ("mato_tipila", "tiles=1;shape=single;terrain=grassland,plains,desert,tundra;mountain=;hills=either;water=0;adjacent=;not_adjacent=coast;adjacent_feature=;avoid=;bare=false;no_river=true;land="),
            ("delicate_arch", "tiles=1;shape=single;terrain=desert;mountain=;hills=either;water=0;adjacent=;not_adjacent=coast;adjacent_feature=;avoid=;bare=false;no_river=true;land="),
            ("chocolate_hills", "tiles=4;shape=diamond;terrain=grassland,plains,mountain;mountain=grassland,plains;hills=either;water=0;adjacent=;not_adjacent=coast;adjacent_feature=;avoid=;bare=false;no_river=true;land="),
            ("vesuvius", "tiles=1;shape=single;terrain=mountain;mountain=grassland,plains;hills=either;water=0;adjacent=grassland,plains,desert,snow,tundra;not_adjacent=;adjacent_feature=;avoid=;bare=false;no_river=true;land="),
            ("lake_retba", "tiles=2;shape=adjacent;terrain=grassland,plains;mountain=;hills=flat;water=0;adjacent=;not_adjacent=coast;adjacent_feature=;avoid=;bare=false;no_river=true;land="),
            ("bermuda_triangle", "tiles=3;shape=triangle;terrain=ocean;mountain=;hills=either;water=0;adjacent=;not_adjacent=coast;adjacent_feature=;avoid=ice;bare=false;no_river=true;land="),
            ("eyjafjallajokull", "tiles=2;shape=adjacent;terrain=snow,tundra;mountain=;hills=either;water=0;adjacent=snow,tundra;not_adjacent=coast;adjacent_feature=;avoid=;bare=false;no_river=true;land="),
            ("fountain_of_youth", "tiles=1;shape=single;terrain=grassland,plains,desert;mountain=;hills=either;water=0;adjacent=;not_adjacent=coast;adjacent_feature=;avoid=;bare=false;no_river=true;land="),
            ("lysefjord", "tiles=3;shape=coastal_triangle;terrain=plains,tundra;mountain=;hills=either;water=0;adjacent=;not_adjacent=;adjacent_feature=;avoid=;bare=true;no_river=true;land="),
            ("paititi", "tiles=3;shape=triangle;terrain=grassland,plains,desert;mountain=;hills=either;water=0;adjacent=;not_adjacent=coast;adjacent_feature=;avoid=;bare=false;no_river=true;land="),
            ("mount_roraima", "tiles=4;shape=roraima;terrain=grassland,plains,mountain;mountain=grassland,plains;hills=either;water=0;adjacent=;not_adjacent=coast;adjacent_feature=;avoid=;bare=false;no_river=true;land="),
            ("tsingy_de_bemaraha", "tiles=1;shape=single;terrain=grassland,plains,tundra;mountain=;hills=either;water=0;adjacent=;not_adjacent=coast,mountain;adjacent_feature=;avoid=;bare=false;no_river=true;land="),
            ("sahara_el_beyda", "tiles=4;shape=diamond;terrain=desert,mountain;mountain=desert;hills=either;water=0;adjacent=;not_adjacent=coast;adjacent_feature=;avoid=;bare=false;no_river=true;land="),
        ]
        .into_iter()
        .collect();
    let actual: BTreeMap<&str, String> = rules
            .features
            .iter()
            .filter(|(_, feature)| feature.natural_wonder)
            .map(|(name, feature)| {
                let placement = &feature.placement;
                let names = |values: &[Name]| {
                    values
                        .iter()
                        .map(|name| name.as_str())
                        .collect::<Vec<_>>()
                        .join(",")
                };
                let shape = serde_json::to_value(placement.shape).unwrap();
                let hills = match placement.hills {
                    Some(true) => "hills",
                    Some(false) => "flat",
                    None => "either",
                };
                let land = placement
                    .land_distance
                    .map(|[near, far]| format!("{near},{far}"))
                    .unwrap_or_default();
                (
                    name.as_str(),
                    format!(
                        "tiles={};shape={};terrain={};mountain={};hills={};water={};adjacent={};not_adjacent={};adjacent_feature={};avoid={};bare={};no_river={};land={}",
                        placement.tiles,
                        shape.as_str().unwrap(),
                        names(&placement.terrain),
                        names(&placement.mountain_terrain),
                        hills,
                        placement.water_tiles,
                        names(&placement.adjacent_terrain),
                        names(&placement.not_adjacent_terrain),
                        names(&placement.adjacent_feature),
                        names(&placement.avoid_feature),
                        placement.no_adjacent_features,
                        placement.no_river,
                        land,
                    ),
                )
            })
            .collect();
    assert_eq!(actual.len(), 34);
    assert_eq!(actual.len(), expected.len());
    for (wonder, signature) in actual {
        assert_eq!(signature, expected[wonder], "{wonder} placement");
    }
}

/// Complete placement-only transcription of the shipped `Buildings`,
/// `Building_ValidTerrains`, `Building_RequiredFeatures`, and
/// `BuildingPrereqs` rows. Effects are tested elsewhere; this oracle keeps
/// every constructed wonder's site predicate from drifting.
#[test]
fn every_world_wonder_has_its_shipped_placement_constraints() {
    let rules = Rules::embedded();
    const EXPECTED: &str = r#"
great_bath|t=;h=either;f=floodplains,grassland_floodplains,plains_floodplains;water=false;coast=false;river=false;mountain=false;religion=false;district=;resource=;improvement=;requires=;any=;placement=
etemenanki|t=;h=either;f=floodplains,grassland_floodplains,plains_floodplains,marsh;water=false;coast=false;river=false;mountain=false;religion=false;district=;resource=;improvement=;requires=;any=;placement=
hanging_gardens|t=;h=either;f=;water=false;coast=false;river=true;mountain=false;religion=false;district=;resource=;improvement=;requires=;any=;placement=
pyramids|t=desert;h=flat;f=;water=false;coast=false;river=false;mountain=false;religion=false;district=;resource=;improvement=;requires=;any=;placement=
stonehenge|t=;h=flat;f=;water=false;coast=false;river=false;mountain=false;religion=false;district=;resource=stone;improvement=;requires=;any=;placement=
temple_artemis|t=;h=either;f=;water=false;coast=false;river=false;mountain=false;religion=false;district=;resource=;improvement=camp;requires=;any=;placement=
apadana|t=;h=either;f=;water=false;coast=false;river=false;mountain=false;religion=false;district=;resource=;improvement=;requires=;any=;placement=adjacent_capital
colosseum|t=;h=flat;f=;water=false;coast=false;river=false;mountain=false;religion=false;district=entertainment_complex;resource=;improvement=;requires=arena;any=;placement=
colossus|t=;h=either;f=;water=true;coast=true;river=false;mountain=false;religion=false;district=harbor;resource=;improvement=;requires=;any=;placement=
great_library|t=;h=flat;f=;water=false;coast=false;river=false;mountain=false;religion=false;district=campus;resource=;improvement=;requires=library;any=;placement=
great_lighthouse|t=;h=either;f=;water=true;coast=true;river=false;mountain=false;religion=false;district=harbor;resource=;improvement=;requires=lighthouse;any=;placement=
jebel_barkal|t=desert;h=hills;f=;water=false;coast=false;river=false;mountain=false;religion=false;district=;resource=;improvement=;requires=;any=;placement=
machu_picchu|t=mountain;h=either;f=;water=false;coast=false;river=false;mountain=false;religion=false;district=;resource=;improvement=;requires=;any=;placement=mountain
mahabodhi_temple|t=;h=either;f=forest;water=false;coast=false;river=false;mountain=false;religion=true;district=holy_site;resource=;improvement=;requires=temple;any=;placement=
mausoleum_at_halicarnassus|t=;h=either;f=;water=false;coast=false;river=false;mountain=false;religion=false;district=harbor;resource=;improvement=;requires=;any=;placement=
oracle|t=;h=hills;f=;water=false;coast=false;river=false;mountain=false;religion=false;district=;resource=;improvement=;requires=;any=;placement=
petra|t=desert;h=flat;f=;water=false;coast=false;river=false;mountain=false;religion=false;district=;resource=;improvement=;requires=;any=;placement=
statue_of_zeus|t=;h=flat;f=;water=false;coast=false;river=false;mountain=false;religion=false;district=encampment;resource=;improvement=;requires=barracks;any=;placement=
terracotta_army|t=grassland,plains;h=flat;f=;water=false;coast=false;river=false;mountain=false;religion=false;district=encampment;resource=;improvement=;requires=;any=barracks,stable;placement=
alhambra|t=;h=hills;f=;water=false;coast=false;river=false;mountain=false;religion=false;district=encampment;resource=;improvement=;requires=;any=;placement=
angkor_wat|t=;h=either;f=;water=false;coast=false;river=false;mountain=false;religion=false;district=aqueduct;resource=;improvement=;requires=;any=;placement=
chichen_itza|t=;h=either;f=jungle;water=false;coast=false;river=false;mountain=false;religion=false;district=;resource=;improvement=;requires=;any=;placement=
hagia_sophia|t=;h=flat;f=;water=false;coast=false;river=false;mountain=false;religion=true;district=holy_site;resource=;improvement=;requires=;any=;placement=
huey_teocalli|t=lake;h=either;f=;water=true;coast=false;river=false;mountain=false;religion=false;district=;resource=;improvement=;requires=;any=;placement=lake_adjacent_land
kilwa_kisiwani|t=;h=flat;f=;water=false;coast=true;river=false;mountain=false;religion=false;district=;resource=;improvement=;requires=;any=;placement=
kotoku_in|t=;h=either;f=;water=false;coast=false;river=false;mountain=false;religion=false;district=holy_site;resource=;improvement=;requires=temple;any=;placement=
meenakshi_temple|t=;h=either;f=;water=false;coast=false;river=false;mountain=false;religion=true;district=holy_site;resource=;improvement=;requires=;any=;placement=
mont_st_michel|t=;h=either;f=floodplains,grassland_floodplains,plains_floodplains,marsh;water=false;coast=false;river=false;mountain=false;religion=false;district=;resource=;improvement=;requires=;any=;placement=
university_of_sankore|t=desert;h=either;f=;water=false;coast=false;river=false;mountain=false;religion=false;district=campus;resource=;improvement=;requires=university;any=;placement=
casa_de_contratacion|t=;h=either;f=;water=false;coast=false;river=false;mountain=false;religion=false;district=government_plaza;resource=;improvement=;requires=;any=;placement=
forbidden_city|t=;h=flat;f=;water=false;coast=false;river=false;mountain=false;religion=false;district=city_center;resource=;improvement=;requires=;any=;placement=
great_zimbabwe|t=;h=either;f=;water=false;coast=false;river=false;mountain=false;religion=false;district=commercial_hub;resource=cattle;improvement=;requires=market;any=;placement=
orszaghaz|t=;h=either;f=;water=false;coast=false;river=true;mountain=false;religion=false;district=;resource=;improvement=;requires=;any=;placement=
oxford_university|t=grassland,plains;h=flat;f=;water=false;coast=false;river=false;mountain=false;religion=false;district=campus;resource=;improvement=;requires=university;any=;placement=
potala_palace|t=;h=hills;f=;water=false;coast=false;river=false;mountain=true;religion=false;district=;resource=;improvement=;requires=;any=;placement=
st_basils_cathedral|t=;h=either;f=;water=false;coast=false;river=false;mountain=false;religion=false;district=city_center;resource=;improvement=;requires=;any=;placement=
taj_mahal|t=;h=either;f=;water=false;coast=false;river=true;mountain=false;religion=false;district=;resource=;improvement=;requires=;any=;placement=
torre_de_belem|t=;h=either;f=;water=true;coast=true;river=false;mountain=false;religion=false;district=harbor;resource=;improvement=;requires=;any=;placement=
venetian_arsenal|t=;h=either;f=;water=true;coast=true;river=false;mountain=false;religion=false;district=industrial_zone;resource=;improvement=;requires=;any=;placement=
big_ben|t=;h=either;f=;water=false;coast=false;river=true;mountain=false;religion=false;district=commercial_hub;resource=;improvement=;requires=bank;any=;placement=
bolshoi_theatre|t=;h=flat;f=;water=false;coast=false;river=false;mountain=false;religion=false;district=theater_square;resource=;improvement=;requires=;any=;placement=
hermitage|t=;h=either;f=;water=false;coast=false;river=true;mountain=false;religion=false;district=;resource=;improvement=;requires=;any=;placement=
ruhr_valley|t=;h=either;f=;water=false;coast=false;river=true;mountain=false;religion=false;district=industrial_zone;resource=;improvement=;requires=factory;any=;placement=
statue_of_liberty|t=;h=either;f=;water=true;coast=true;river=false;mountain=false;religion=false;district=harbor;resource=;improvement=;requires=;any=;placement=
biosphere|t=;h=either;f=;water=false;coast=false;river=true;mountain=false;religion=false;district=neighborhood;resource=;improvement=;requires=;any=;placement=
broadway|t=;h=flat;f=;water=false;coast=false;river=false;mountain=false;religion=false;district=theater_square;resource=;improvement=;requires=;any=;placement=
cristo_redentor|t=;h=hills;f=;water=false;coast=false;river=false;mountain=false;religion=false;district=;resource=;improvement=;requires=;any=;placement=
eiffel_tower|t=;h=flat;f=;water=false;coast=false;river=false;mountain=false;religion=false;district=city_center;resource=;improvement=;requires=;any=;placement=
estadio_do_maracana|t=;h=flat;f=;water=false;coast=false;river=false;mountain=false;religion=false;district=entertainment_complex;resource=;improvement=;requires=stadium;any=;placement=
golden_gate_bridge|t=;h=either;f=;water=true;coast=true;river=false;mountain=false;religion=false;district=;resource=;improvement=;requires=;any=;placement=golden_gate_bridge
amundsen_scott_research_station|t=snow;h=either;f=;water=false;coast=false;river=false;mountain=false;religion=false;district=campus;resource=;improvement=;requires=research_lab;any=;placement=
sydney_opera_house|t=;h=either;f=;water=true;coast=true;river=false;mountain=false;religion=false;district=harbor;resource=;improvement=;requires=;any=;placement=
panama_canal|t=;h=flat;f=;water=false;coast=false;river=false;mountain=false;religion=false;district=;resource=;improvement=;requires=;any=;placement=panama_canal
"#;
    let expected: BTreeMap<&str, &str> = EXPECTED
        .lines()
        .filter_map(|line| line.split_once('|'))
        .collect();
    let names = |values: &[Name]| {
        values
            .iter()
            .map(|name| name.as_str())
            .collect::<Vec<_>>()
            .join(",")
    };
    let actual: BTreeMap<&str, String> = rules
            .wonders
            .iter()
            .map(|(name, wonder)| {
                let hills = match wonder.hills {
                    Some(true) => "hills",
                    Some(false) => "flat",
                    None => "either",
                };
                (
                    name.as_str(),
                    format!(
                        "t={};h={};f={};water={};coast={};river={};mountain={};religion={};district={};resource={};improvement={};requires={};any={};placement={}",
                        names(&wonder.terrain),
                        hills,
                        names(&wonder.feature),
                        wonder.water,
                        wonder.coast,
                        wonder.river,
                        wonder.adjacent_mountain,
                        wonder.founded_religion,
                        wonder.adjacent_district.map_or("", |name| name.as_str()),
                        wonder.adjacent_resource.map_or("", |name| name.as_str()),
                        wonder.adjacent_improvement.map_or("", |name| name.as_str()),
                        names(&wonder.requires_buildings),
                        names(&wonder.requires_any_buildings),
                        wonder.placement,
                    ),
                )
            })
            .collect();
    assert_eq!(actual.len(), 53);
    assert_eq!(actual.len(), expected.len());
    for (wonder, signature) in actual {
        assert_eq!(signature, expected[wonder], "{wonder} placement");
    }
}

/// The collection paths skip a whole sweep when the index says no spec in
/// the family grants an effect, so an index that misses a key would make
/// a real modifier silently stop applying. Every declared key must be
/// present in its own family and in the union.
#[test]
fn the_effect_index_covers_every_key_any_spec_declares() {
    let rules = Rules::shipped();
    let index = &rules.effect_index;
    let mut checked = 0usize;
    let check = |family: &str, present: bool, in_any: bool, key: &str| {
        assert!(present, "{family} declares {key}, which its index omits");
        assert!(
            in_any,
            "{key} is declared by {family} but missing from the union"
        );
    };
    for spec in rules.policies.values() {
        for key in spec.effects.keys() {
            check("policies", index.policies(key), index.any(key), key);
            checked += 1;
        }
    }
    for spec in rules.civs.values() {
        for key in spec.effects.keys() {
            check("civs", index.civs(key), index.any(key), key);
            checked += 1;
        }
    }
    for spec in rules.buildings.values() {
        for key in spec.effects.keys() {
            check("buildings", index.buildings(key), index.any(key), key);
            checked += 1;
        }
    }
    for spec in rules.districts.values() {
        for key in spec.effects.keys() {
            check("districts", index.districts(key), index.any(key), key);
            checked += 1;
        }
    }
    for spec in rules.wonders.values() {
        for key in spec.effects.keys() {
            check("wonders", index.wonders(key), index.any(key), key);
            checked += 1;
        }
    }
    // `modifiers` is deliberately absent: it is swapped in at runtime, so
    // the collection paths fall through on the seat's own attachment list
    // rather than trusting an index of it.
    for table in [
        &rules.beliefs.pantheon,
        &rules.beliefs.founder,
        &rules.beliefs.follower,
        &rules.beliefs.enhancer,
        &rules.beliefs.worship,
    ] {
        for spec in table.values() {
            for key in spec.effects.keys() {
                check("beliefs", index.beliefs(key), index.any(key), key);
                checked += 1;
            }
        }
    }
    for spec in rules.governors.values() {
        for key in spec
            .effects
            .keys()
            .chain(spec.promotions.values().flat_map(|p| p.effects.keys()))
        {
            check("governors", index.governors(key), index.any(key), key);
            checked += 1;
        }
    }
    for key in rules.tree_effects.keys() {
        assert!(
            index.any(key),
            "tree effect {key} is missing from the union"
        );
        checked += 1;
    }
    assert!(
        checked > 500,
        "expected the shipped ruleset to declare many effects, saw {checked}"
    );
}

/// The three namespaced families are ruled out on the selector alone, so
/// every selector named by a declared key has to be indexed.
#[test]
fn the_effect_index_covers_every_namespaced_selector() {
    let rules = Rules::shipped();
    let index = &rules.effect_index;
    for key in index.any.keys() {
        if let Some(rest) = key.strip_prefix(BUILDING_YIELD_EFFECT_PREFIX) {
            let (selector, _) = rest
                .split_once(':')
                .expect("a building yield key names a yield");
            assert!(
                index.modifies_building_yields(selector),
                "{key} names building selector {selector}, which the index omits"
            );
        } else if let Some(unit) = key.strip_prefix(UNIT_PURCHASE_EFFECT_PREFIX) {
            assert!(
                index.discounts_unit_purchase(unit),
                "{key} names unit {unit}, which the index omits"
            );
        } else if let Some(ability) = key.strip_prefix(GRANT_ABILITY_EFFECT_PREFIX) {
            assert!(
                index.grants_ability(ability),
                "{key} names ability {ability}, which the index omits"
            );
        }
    }
}

#[test]
fn historic_moment_catalogue_is_complete_positive_and_windowed() {
    let rules = Rules::shipped();
    assert_eq!(rules.historic_moments.len(), 149);
    assert_eq!(
        rules
            .historic_moments
            .values()
            .map(|moment| moment.era_score)
            .sum::<i64>(),
        347
    );
    assert_eq!(
        rules
            .historic_moments
            .values()
            .filter(|moment| {
                moment.minimum_game_era.is_some()
                    || moment.maximum_game_era.is_some()
                    || moment.obsolete_era.is_some()
            })
            .count(),
        23
    );
    for id in [
        "MOMENT_FIRST_INDUSTRY",
        "MOMENT_FIRST_INDUSTRY_IN_WORLD",
        "MOMENT_FIRST_CORPORATION",
        "MOMENT_FIRST_CORPORATION_IN_WORLD",
        "MOMENT_FIRST_LUXURY_RESOURCE_MONOPOLY",
        "MOMENT_FIRST_LUXURY_RESOURCE_MONOPOLY_IN_WORLD",
    ] {
        assert!(
            rules.historic_moments.contains_key(id),
            "New Frontier Moment {id} is missing"
        );
    }
}
