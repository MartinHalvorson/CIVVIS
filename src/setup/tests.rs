use std::collections::BTreeSet;

use crate::game::{Action, Game, GameOptions, Item};

use super::{
    last_start_era, playable_start_eras, start_era_from_id, start_era_id, stock_start_era_id,
    BaseRuleset, GameMode, GameSpeed, MapPoles, MapScript, MapSize, MapTopology, MatchSeries,
    TacticsEra, TacticsRules, BASE_RULESETS, BATTLEFIELD_SIZES, CIV6_GAME_SPEEDS, CIV6_MAP_SCRIPTS,
    CIV6_MAP_SIZES, MAP_POLES, MAP_TOPOLOGIES, START_ERAS, TACTICS_GLOBE_DIAMETERS,
};

/// One ruleset is offered, and the setting still has to behave like a
/// setting: it resolves by id, it round-trips, and an id from some other
/// game is refused rather than quietly played as Civilization VI.
#[test]
fn the_base_ruleset_is_civ6_and_nothing_else_resolves() {
    assert_eq!(BASE_RULESETS.len(), 1);
    for spec in BASE_RULESETS {
        assert_eq!(BaseRuleset::from_id(spec.id), Some(spec.ruleset));
        assert_eq!(spec.ruleset.id(), spec.id);
    }
    assert_eq!(BaseRuleset::default(), BaseRuleset::Civ6);
    assert_eq!(BaseRuleset::Civ6.id(), "civ6");
    assert_eq!(BaseRuleset::from_id("civ5"), None);
    assert_eq!(BaseRuleset::from_id(""), None);
}

/// The ladder is a timeline and its order is a claim. The rungs that can
/// be played are the ruleset's own eras, in the ruleset's own order — rung
/// `n` *is* era `n`, which is what lets a start era cut the trees — and
/// everything declared but unbuilt sits ahead of them, because what is
/// missing from this list is prehistory rather than some later age.
#[test]
fn the_start_ladder_is_the_rulesets_own_eras_behind_the_prehistory_it_still_owes() {
    let mut seen = BTreeSet::new();
    let mut history_has_begun = false;
    for spec in START_ERAS {
        assert!(seen.insert(spec.id), "{} is listed twice", spec.id);
        // An unbuilt rung still has to be described, or the lobby has
        // nothing honest to show for it.
        assert!(
            !spec.description.is_empty(),
            "{} has no description",
            spec.id
        );
        match spec.era {
            Some(_) => history_has_begun = true,
            // Prehistory precedes history: no unbuilt rung may appear
            // after a playable one, or the ladder stops being a timeline.
            None => assert!(!history_has_begun, "{} sits after a playable era", spec.id),
        }
    }

    let playable: Vec<_> = playable_start_eras().collect();
    assert_eq!(playable.len(), crate::rules::ERA_NAMES.len());
    for (index, spec) in playable.iter().enumerate() {
        assert_eq!(spec.era, Some(index), "{}", spec.id);
        assert_eq!(spec.id, crate::rules::ERA_NAMES[index], "era {index}");
        assert_eq!(start_era_from_id(spec.id), Some(index));
        assert_eq!(start_era_id(index), spec.id);
    }
    assert_eq!(stock_start_era_id(), "ancient");
    assert_eq!(last_start_era(), playable.len() - 1);
    // Future is the last playable start, and an era off the end of the
    // ladder names the stock start rather than nothing at all.
    assert_eq!(
        start_era_from_id("future"),
        Some(crate::rules::ERA_NAMES.len() - 1)
    );
    assert_eq!(start_era_id(playable.len()), "ancient");
    assert_eq!(start_era_from_id("holocene"), None);
}

/// The Stone Age is on the ladder ahead of Ancient and cannot be played
/// yet. Being listed is the point — a lobby that opens on "Ancient" with
/// nothing above it quietly claims human history begins with a Settler —
/// but it resolves to nothing rather than to the stock start, so nobody is
/// handed the Ancient era while believing they asked for something else.
#[test]
fn the_stone_age_is_offered_ahead_of_ancient_and_refused_rather_than_substituted() {
    let stone_age = START_ERAS[0];
    assert_eq!(stone_age.id, "stone_age");
    assert_eq!(START_ERAS[1].id, "ancient");
    assert!(!stone_age.is_playable());
    assert_eq!(stone_age.era, None);
    assert_eq!(start_era_from_id("stone_age"), None);
    // It is not one of the ruleset's eras under another name, so no index
    // can ever resolve back to it.
    assert!(!crate::rules::ERA_NAMES.contains(&stone_age.id));
    assert!((0..crate::rules::ERA_NAMES.len()).all(|era| start_era_id(era) != stone_age.id));

    // The lobby is told whether a rung can be chosen, and never the index
    // behind it: resolving an id stays the server's job.
    let wire = serde_json::to_value(START_ERAS).unwrap();
    assert_eq!(wire[0]["id"], "stone_age");
    assert_eq!(wire[0]["name"], "Stone Age");
    assert_eq!(wire[0]["playable"], false);
    assert_eq!(wire[1]["playable"], true);
    assert!(
        wire[0].get("era").is_none(),
        "the era index reached the wire"
    );
}

/// A world set up to open in a later age of human history, built through
/// the one real constructor rather than described in the lobby.
fn world_opening_in(era: usize) -> Game {
    let size = MapSize::for_players(2);
    Game::new_with(GameOptions {
        barbarians: false,
        start_era: era,
        city_states: size.default_city_states,
        ..GameOptions::new(
            2,
            size.width,
            size.height,
            909,
            250,
            size.default_city_states,
        )
    })
}

/// The start era is the setting that actually changes the game: a world
/// that opens in the Renaissance opens with everything before it known,
/// by everyone on the board, with an army to match — and it says it is in
/// the Renaissance from its first turn, rather than reporting the era it
/// has just finished.
#[test]
fn a_game_that_opens_past_the_first_age_starts_with_the_earlier_eras_behind_it() {
    let ancient = world_opening_in(0);
    assert_eq!(ancient.world_era, 0);
    assert_eq!(ancient.start_era, 0);
    assert!(ancient.players.iter().all(|player| player.techs.is_empty()));

    let era = start_era_from_id("renaissance").unwrap();
    let renaissance = world_opening_in(era);
    assert_eq!(renaissance.start_era, era);
    assert_eq!(renaissance.world_era, era);
    let earlier: BTreeSet<&crate::name::Name> = renaissance
        .rules
        .techs
        .iter()
        .filter(|(_, spec)| spec.era < era)
        .map(|(name, _)| name)
        .collect();
    assert!(!earlier.is_empty());
    // Majors and city-states alike: a minor still holding Ancient spears
    // in a Renaissance world is free conquest, not a setting.
    for player in renaissance
        .players
        .iter()
        .filter(|player| !player.is_barbarian)
    {
        for tech in &earlier {
            assert!(
                player.techs.contains(&crate::name::Name::new(tech)),
                "{} lacks {tech}",
                player.civ
            );
        }
        assert!(
            player
                .techs
                .iter()
                .all(|tech| renaissance.rules.techs[tech].era < era),
            "{} was handed a technology of its own era or later",
            player.civ
        );
        assert!(
            player
                .civics
                .iter()
                .all(|civic| renaissance.rules.civics[civic].era < era),
            "{} was handed a civic of its own era or later",
            player.civ
        );
        // Nothing may be left researching what it already knows.
        assert!(player
            .research
            .as_ref()
            .is_none_or(|tech| !player.techs.contains(&crate::name::Name::new(tech))));
    }
    // The starting army came up its own upgrade chain with the research.
    let kinds: BTreeSet<&str> = renaissance
        .units
        .values()
        .map(|unit| unit.kind.as_str())
        .collect();
    assert!(
        !kinds.contains("warrior"),
        "a Renaissance world still opens on Warriors: {kinds:?}"
    );
    assert!(
        kinds.contains("settler"),
        "the Settler is not an upgradeable unit: {kinds:?}"
    );

    // The final rung is a real opening too: it carries every earlier age
    // forward while leaving the Future tree for the new world to play.
    let future_era = start_era_from_id("future").unwrap();
    let future = world_opening_in(future_era);
    assert_eq!(future.start_era, future_era);
    assert_eq!(future.world_era, future_era);
    assert!(future
        .rules
        .techs
        .values()
        .any(|spec| spec.era == future_era));
    assert!(future
        .rules
        .civics
        .values()
        .any(|spec| spec.era == future_era));
    for player in future.players.iter().filter(|player| !player.is_barbarian) {
        assert!(player
            .techs
            .iter()
            .all(|tech| future.rules.techs[tech].era < future_era));
        assert!(player
            .civics
            .iter()
            .all(|civic| future.rules.civics[civic].era < future_era));
        assert!(future
            .rules
            .techs
            .iter()
            .filter(|(_, spec)| spec.era == future_era)
            .all(|(tech, _)| !player.techs.contains(tech)));
        assert!(future
            .rules
            .civics
            .iter()
            .filter(|(_, spec)| spec.era == future_era)
            .all(|(civic, _)| !player.civics.contains(civic)));
    }

    // The whole setup survives a save, or a reloaded world would quietly
    // fall back to the Ancient era and undo its own floor.
    let restored: Game =
        serde_json::from_str(&serde_json::to_string(&renaissance).unwrap()).unwrap();
    assert_eq!(restored.start_era, era);
    assert_eq!(restored.base_ruleset, BaseRuleset::Civ6);
}

/// A rung past the end of the ladder is clamped rather than fatal, and may
/// not produce a world that claims an era it cannot play.
#[test]
fn a_rung_past_the_end_of_the_ladder_opens_at_the_last_age_that_exists() {
    let last = last_start_era();
    let beyond = world_opening_in(last + 5);
    assert_eq!(beyond.start_era, last);
    assert_eq!(beyond.world_era, last);
}

/// The map menu has a deliberate reading order. Earth and True Start Earth
/// sit together above Continents; Fjords follows Small Continents even
/// though its terrain mix is intentionally not a land-share rung.
#[test]
fn the_world_types_follow_the_requested_menu_order() {
    assert_eq!(
        CIV6_MAP_SCRIPTS
            .iter()
            .map(|spec| spec.script)
            .collect::<Vec<_>>(),
        vec![
            MapScript::LandOnly,
            MapScript::Lakes,
            MapScript::InlandSea,
            MapScript::TeninsBall,
            MapScript::GrandCanals,
            MapScript::GrandCanalsTwo,
            MapScript::Pangaea,
            MapScript::Earth,
            MapScript::TrueStartEarth,
            MapScript::Continents,
            MapScript::SmallContinents,
            MapScript::Fjords,
            MapScript::Islands,
            MapScript::WaterWorld,
            MapScript::Battlefield,
            MapScript::TacticsPlanet,
            MapScript::TacticsOcean,
            MapScript::Trafalgar,
        ]
    );
    assert_eq!(MapScript::LandOnly.land_percent(), 95);
    assert_eq!(MapScript::WaterWorld.land_percent(), 5);
    assert_eq!(MapScript::Fjords.land_percent(), 60);
    assert_eq!(
        MapScript::Earth.land_percent(),
        MapScript::TrueStartEarth.land_percent()
    );
    // Every type is reachable by the id the protocol carries, and the list
    // holds each of them exactly once.
    let mut seen = BTreeSet::new();
    for spec in CIV6_MAP_SCRIPTS {
        assert_eq!(
            MapScript::from_id(spec.id),
            Some(spec.script),
            "{}",
            spec.id
        );
        assert_eq!(spec.script.id(), spec.id);
        assert!(seen.insert(spec.id), "{} is listed twice", spec.id);
    }
}

#[test]
fn tennis_ball_uses_its_correct_display_name() {
    let tennis_ball = CIV6_MAP_SCRIPTS
        .iter()
        .find(|spec| spec.id == "tenins_ball")
        .expect("the Tennis Ball map is listed");
    assert_eq!(tennis_ball.name, "Tennis Ball");
}

/// Tactics maps are a separate roster from Civ worlds. The Battlefield is
/// a bounded arena; Planet and Ocean keep their globes and are each
/// offered at the same four diameters; and Trafalgar is a scenario on a
/// fixed chart.
#[test]
fn the_battlefield_is_an_arena_rather_than_a_world() {
    for spec in CIV6_MAP_SCRIPTS {
        assert_eq!(
            spec.script.is_battlefield(),
            matches!(
                spec.script,
                MapScript::Battlefield
                    | MapScript::TacticsPlanet
                    | MapScript::TacticsOcean
                    | MapScript::Trafalgar
            ),
            "{}",
            spec.id
        );
        // A globe is a Tactics map that kept its sphere, and the naval
        // one is the globe fought on water. Neither predicate may reach
        // outside the mode: a Civ world routed down the arena's seating
        // and deployment would open two capitals as armies.
        assert!(!spec.script.is_planet_battlefield() || spec.script.is_battlefield());
        assert!(!spec.script.is_naval_battlefield() || spec.script.is_planet_battlefield());
        // Every scenario is a Tactics map; almost no Tactics map is a
        // scenario. Getting that backwards would send an arena down the
        // fixed-chart path and panic the generator.
        assert!(
            !spec.script.is_scenario() || spec.script.is_battlefield(),
            "{}",
            spec.id
        );
    }
    assert_eq!(
        MapScript::from_id("battlefield"),
        Some(MapScript::Battlefield)
    );
    assert_eq!(
        MapScript::from_id("tactics_planet"),
        Some(MapScript::TacticsPlanet)
    );
    assert_eq!(MapScript::from_id("trafalgar"), Some(MapScript::Trafalgar));
    for size in BATTLEFIELD_SIZES {
        if size.script.is_scenario() {
            // A scenario's size is its chart's, so it is not written as
            // WxH and is not the player's to choose.
            assert_eq!(size.script, MapScript::Trafalgar, "{}", size.id);
            assert_eq!((size.width, size.height), (30, 24), "{}", size.id);
            assert_eq!(size.topology, MapTopology::Flat, "{}", size.id);
        } else if size.script == MapScript::Battlefield {
            let ground: Vec<i32> = size
                .id
                .split('x')
                .map(|side| side.parse().expect("battlefield ids read WxH"))
                .collect();
            assert_eq!(ground.len(), 2, "{}", size.id);
            assert_eq!(
                size.width, ground[0],
                "{} is its own fighting ground",
                size.id
            );
            assert_eq!(size.height, ground[1], "{}", size.id);
            // Both sides are seated at opposite ends of the long axis,
            // which is the north-south one at every offered size.
            assert!(
                size.height >= size.width,
                "{} is not taller than it is wide",
                size.id
            );
            assert_eq!(size.topology, MapTopology::Flat, "{}", size.id);
        } else {
            // A globe entry is stored in the rectangle its own diameter
            // implies, and nothing else: 5n by 2n+2. Read the diameter
            // back out of the width and check the height against it, so a
            // hand-edited row cannot advertise a size the sphere would
            // then refuse to build.
            assert!(size.script.is_planet_battlefield(), "{}", size.id);
            assert_eq!(size.topology, MapTopology::Planet, "{}", size.id);
            assert_eq!(size.width % 5, 0, "{} is not 5n wide", size.id);
            let diameter = size.width / 5;
            assert_eq!(
                size.height,
                2 * diameter + 2,
                "{} is not 2n+2 tall",
                size.id
            );
            assert!(TACTICS_GLOBE_DIAMETERS.contains(&diameter), "{}", size.id);
            // The name carries the number, because a menu row reading
            // "Planet" twice would be two indistinguishable choices.
            assert!(
                size.name.ends_with(&format!("diameter {diameter}")),
                "{}",
                size.name
            );
        }
        // No *flat* battlefield collides with a real map size: the
        // smallest world is wider than the largest arena several times
        // over, so a bounded field can never be mistaken for a world.
        //
        // A globe is exempt, and has to be. Its rectangle is a pure
        // function of its diameter — 5n by 2n+2 — so the diameter-15
        // globe is 75x32 whether it is a Tactics battlefield or the globe
        // the Civ Tiny size lays out, which also runs at frequency 15.
        // That is arithmetic rather than a clash, and nothing reads a
        // globe's size out of the flat table anyway: a globe is
        // re-expressed through `mapgen::globe_frequency` on every setup
        // that touches its size or its shape, and the flat lookup is only
        // reached by maps that are not globes.
        if size.topology.is_globe() {
            assert!(
                MapSize::from_dimensions(size.width, size.height).is_none_or(|found| {
                    (found.globe_width(), found.globe_height()) == (size.width, size.height)
                })
            );
        } else {
            assert!(
                MapSize::from_dimensions(size.width, size.height).is_none(),
                "{}",
                size.id
            );
        }
    }
    // Both globe families offer the same ladder, and only that ladder. The
    // two are the same experiment on opposite elements, so a diameter
    // present on one and missing from the other would leave a battle with
    // nothing to be read against.
    for script in [MapScript::TacticsPlanet, MapScript::TacticsOcean] {
        let offered: Vec<i32> = BATTLEFIELD_SIZES
            .iter()
            .filter(|size| size.script == script)
            .map(|size| size.width / 5)
            .collect();
        assert_eq!(offered, TACTICS_GLOBE_DIAMETERS, "{script:?}");
    }
    // Diameter 8 keeps the bare `planet` id it shipped with, so a saved
    // game, a sweep script or a lobby deep-link that names it still lands
    // on the world it named.
    assert!(BATTLEFIELD_SIZES.iter().any(|size| size.id == "planet"
        && size.script == MapScript::TacticsPlanet
        && (size.width, size.height) == (40, 18)));
    assert_eq!(BATTLEFIELD_SIZES.len(), 12);
    // Exactly one entry per scenario script, because a scenario has one
    // size and a menu offering it twice would be offering two battles.
    assert_eq!(
        BATTLEFIELD_SIZES
            .iter()
            .filter(|size| size.script.is_scenario())
            .count(),
        CIV6_MAP_SCRIPTS
            .iter()
            .filter(|spec| spec.script.is_scenario())
            .count()
    );
}

/// A scenario refuses the arena economy the Tactics card offers and keeps
/// only the two settings that are about how long you want to play.
#[test]
fn a_scenario_is_fought_under_its_own_economy() {
    let generous = TacticsRules {
        cities: 1,
        production: 200,
        gold: 200,
        turns_per_tech: 4,
        turn_limit: 150,
        best_of: 3,
        unique_units: true,
        fog: true,
        flag: true,
        heal: false,
        era: TacticsEra::Random,
    };
    let fought = generous.for_script(MapScript::Trafalgar);
    assert_eq!(
        fought,
        TacticsRules {
            cities: 0,
            production: 0,
            gold: 0,
            turns_per_tech: 0,
            turn_limit: 150,
            best_of: 3,
            unique_units: false,
            fog: false,
            flag: false,
            heal: false,
            // The era choice goes with the economy: Trafalgar is 1805.
            era: TacticsEra::Start,
        }
    );
    // An arena still gets exactly what it asked for, sanitized and no
    // more: `for_script` must not become a second place economies change.
    assert_eq!(
        generous.for_script(MapScript::Battlefield),
        generous.sanitized(),
        "an arena's economy is still the player's"
    );
    assert_eq!(
        generous.for_script(MapScript::Pangaea),
        generous.sanitized()
    );
}

/// The three games CIVVIS offers, and how a world says which one it is.
///
/// Civ is the whole thing; Tactics and Sim City each take one half of it
/// and ask whether you can play that half. The map script is the only
/// marker — an arena is not a world — so no save, setting or protocol row
/// can disagree with the map about which game is being played.
#[test]
fn every_world_says_which_of_the_three_games_it_is() {
    for mode in GameMode::ALL {
        assert_eq!(GameMode::from_id(mode.id()), Some(mode), "{}", mode.id());
        assert!(!mode.name().is_empty());
    }
    assert_eq!(GameMode::from_id("battlefield"), None);
    assert_eq!(GameMode::default(), GameMode::Civ);
    for spec in CIV6_MAP_SCRIPTS {
        let mode = GameMode::for_script(spec.script);
        assert_eq!(
            mode,
            if spec.script.is_battlefield() {
                GameMode::Tactics
            } else {
                GameMode::Civ
            },
            "{}",
            spec.id
        );
        assert!(
            mode.is_playable(),
            "{} builds a mode nobody can play",
            spec.id
        );
    }
    // Sim City is declared and not yet built: it has a name, an id and a
    // ladder of its own, and no map script produces one.
    assert!(!GameMode::SimCity.is_playable());
    assert!(CIV6_MAP_SCRIPTS
        .iter()
        .all(|spec| GameMode::for_script(spec.script) != GameMode::SimCity));
}

/// A match is a series between two civilizations, and its score belongs
/// to the civilizations rather than to the seats they were sitting in —
/// because a match swaps the sides over between battles. It is always an
/// odd number of battles, so wins cannot split evenly; draws can still
/// The flag is a replacement objective, not an addition: asking for it
/// plays city-less whatever the cities knob said, and old saves — written
/// before the shape existed — load with it off.
#[test]
fn a_flag_battle_is_always_city_less() {
    assert!(
        !TacticsRules::default().flag,
        "cities decide a battle unless a flag is asked for"
    );
    let flagged = TacticsRules {
        flag: true,
        cities: 1,
        ..TacticsRules::default()
    }
    .sanitized();
    assert_eq!(
        flagged.cities, 0,
        "the flag replaces the city objective outright"
    );
    assert!(flagged.flag);
    let mut old = serde_json::to_value(TacticsRules::default()).unwrap();
    old.as_object_mut().unwrap().remove("flag");
    assert!(!serde_json::from_value::<TacticsRules>(old).unwrap().flag);
}

/// A new battle is fogged; a battle saved before fog existed is not.
///
/// The two answers are deliberately different, which is the one thing
/// worth pinning here. Every save without the field was written when the
/// arena had no fog and was therefore played with the whole field lit, so
/// loading it as fogged would restate how it was fought — and a rated
/// ladder replayed off those saves would stop being the experiment it
/// recorded. `#[serde(default)]` would have taken `bool::default()` and
/// got the right answer for the wrong reason; the named function is what
/// keeps the two from drifting together the next time the default moves.
#[test]
fn a_new_battle_is_fogged_and_an_old_save_is_not() {
    assert!(
        TacticsRules::default().fog,
        "a battle is fought through its own fog"
    );
    assert!(
        TacticsRules::default().sanitized().fog,
        "sanitizing must not lift the fog"
    );
    let mut old = serde_json::to_value(TacticsRules::default()).unwrap();
    old.as_object_mut().unwrap().remove("fog");
    assert!(
        !serde_json::from_value::<TacticsRules>(old).unwrap().fog,
        "a save from before fog existed was played unfogged and reloads that way"
    );
    // A save that does carry the field keeps what it was played under,
    // both ways round.
    for fog in [false, true] {
        let written = serde_json::to_value(TacticsRules {
            fog,
            ..TacticsRules::default()
        });
        let read: TacticsRules = serde_json::from_value(written.unwrap()).unwrap();
        assert_eq!(read.fog, fog);
    }
    // A scenario still overrules it: the Combined Fleet could see the
    // British columns coming for four hours.
    assert!(!TacticsRules::default().for_script(MapScript::Trafalgar).fog);
}

/// exhaust it without a winner.
#[test]
fn a_match_is_an_odd_series_scored_by_civilization() {
    // An even length is not a match: best of four is best of three with a
    // dead rubber attached, and can be split with nothing left to play.
    for asked in [0, 1, 2, 3, 4, 5, 6, 1_000] {
        let length = TacticsRules {
            best_of: asked,
            ..TacticsRules::default()
        }
        .sanitized()
        .best_of;
        assert_eq!(length % 2, 1, "asked for {asked}, got {length}");
        assert!(
            (1..=TacticsRules::MAX_BEST_OF).contains(&length),
            "{length}"
        );
    }
    assert_eq!(
        TacticsRules {
            best_of: 5,
            ..TacticsRules::default()
        }
        .wins_needed(),
        3
    );
    assert_eq!(
        TacticsRules::default().best_of,
        1,
        "one battle unless a match is asked for"
    );
    assert!(
        !TacticsRules::default().unique_units,
        "even rosters unless asked otherwise"
    );

    let mut series = MatchSeries::new(5, ["Greece".to_string(), "Egypt".to_string()]);
    assert_eq!(series.wins_needed(), 3);
    assert!(!series.decided());
    series.record(Some("Greece"));
    series.record(Some("Egypt"));
    series.record(Some("Greece"));
    assert!(!series.decided(), "two of five is not a match");
    assert_eq!(series.scoreline(), "Greece 2 – Egypt 1 (best of 5)");
    series.record(Some("Greece"));
    assert!(series.decided());
    assert_eq!(series.winner(), Some("Greece"));
    assert_eq!(
        series.played(),
        4,
        "a match stops at the battle that settles it"
    );
    assert_eq!(series.scoreline(), "Greece 3 – Egypt 1");

    // A battle nobody wins is counted rather than dropped, and enough of
    // them end the match without a winner.
    let mut drawn = MatchSeries::new(3, ["Rome".to_string(), "Nubia".to_string()]);
    for _ in 0..3 {
        drawn.record(None);
    }
    assert!(drawn.decided());
    assert_eq!(drawn.winner(), None);
    assert_eq!(drawn.played(), 3);
    assert!(
        drawn.scoreline().contains("3 drawn"),
        "{}",
        drawn.scoreline()
    );
}

/// A battle clock is a small, deliberate Tactics setting rather than an
/// arbitrary world-length number. Hand-written requests are normalized to
/// the closest offered value, a new lobby opens on the 250-turn clock,
/// and a save from before the field existed keeps the 100-turn clock it
/// was fought under rather than being restated to the new default.
#[test]
fn tactics_turn_limits_use_the_published_ladder_and_survive_old_saves() {
    assert_eq!(
        TacticsRules::TURN_LIMITS,
        [10, 20, 30, 40, 50, 100, 150, 200, 250]
    );
    assert_eq!(
        TacticsRules::default().turn_limit,
        TacticsRules::DEFAULT_TURN_LIMIT
    );
    assert_eq!(TacticsRules::DEFAULT_TURN_LIMIT, 250);
    assert!(TacticsRules::TURN_LIMITS.contains(&TacticsRules::DEFAULT_TURN_LIMIT));
    for limit in TacticsRules::TURN_LIMITS {
        assert_eq!(
            TacticsRules {
                turn_limit: limit,
                ..TacticsRules::default()
            }
            .sanitized()
            .turn_limit,
            limit
        );
    }
    assert_eq!(
        TacticsRules {
            turn_limit: 0,
            ..TacticsRules::default()
        }
        .sanitized()
        .turn_limit,
        10
    );
    assert_eq!(
        TacticsRules {
            turn_limit: 149,
            ..TacticsRules::default()
        }
        .sanitized()
        .turn_limit,
        150
    );

    let mut old = serde_json::to_value(TacticsRules::default()).unwrap();
    old.as_object_mut().unwrap().remove("turn_limit");
    assert_eq!(
        serde_json::from_value::<TacticsRules>(old)
            .unwrap()
            .turn_limit,
        100,
        "a save from before the clock was a setting keeps its 100 turns"
    );
}

/// The stock arena is two standing armies and no reinforcements: a city
/// each to hold, but nothing to build with and nothing to upgrade with,
/// so the battle is decided by the units dealt at turn one. The grants
/// stay real settings — anyone who wants reinforcements raises them.
#[test]
fn the_stock_arena_deals_two_standing_armies_and_no_reinforcements() {
    let stock = TacticsRules::default();
    assert_eq!(
        stock.cities, 1,
        "each side holds the one city it is dropped in with"
    );
    assert_eq!(stock.production, 0, "the stock city builds nothing");
    assert_eq!(
        stock.gold, 0,
        "the stock side banks nothing to upgrade with"
    );
    assert_eq!(stock.sanitized(), stock, "the stock arena is already sane");
    let reinforced = TacticsRules {
        production: 30,
        gold: 30,
        ..stock
    }
    .sanitized();
    assert_eq!(
        reinforced.production, 30,
        "Production per turn is still the player's to set"
    );
    assert_eq!(
        reinforced.gold, 30,
        "Gold per turn is still the player's to set"
    );
}

/// The world's shape and its poles are settings of their own, orthogonal to
/// what fills the world. This includes fixed geography: Earth's known
/// longitudes and latitudes work on either projection.
#[test]
fn the_world_shape_and_its_poles_are_asked_for_separately_from_the_world_type() {
    for spec in MAP_TOPOLOGIES {
        assert_eq!(MapTopology::from_id(spec.id), Some(spec.topology));
        assert_eq!(spec.topology.id(), spec.id);
    }
    for spec in MAP_POLES {
        assert_eq!(MapPoles::from_id(spec.id), Some(spec.poles));
        assert_eq!(spec.poles.id(), spec.id);
    }
    assert!(MapTopology::Planet.is_globe());
    assert!(!MapTopology::Flat.is_globe());
    assert!(MapPoles::Poles.has_poles());
    // Randomized heat has cold ground but no cold *ends*, so it does not
    // get the polar sea-ice band or the polar cap on start placement.
    assert!(!MapPoles::Randomized.has_poles());
    // Heat is laid out two ways and no more: either latitude decides it or
    // noise does. A world with no cold end at all is not on offer, and the
    // spellings that used to ask for one — including the `off` of the era
    // when this was a boolean — name nothing rather than quietly landing on
    // a world nobody asked for.
    assert_eq!(MAP_POLES.len(), 2);
    assert_eq!(MAP_POLES[0].poles, MapPoles::Poles);
    assert_eq!(MAP_POLES[1].poles, MapPoles::Randomized);
    assert_eq!(MapPoles::from_id("on"), Some(MapPoles::Poles));
    assert_eq!(MapPoles::from_id("randomized"), Some(MapPoles::Randomized));
    for retired in ["no_poles", "none", "off", "false"] {
        assert_eq!(
            MapPoles::from_id(retired),
            None,
            "{retired} still names a world"
        );
    }
    assert_eq!(MapPoles::from_id("hot_and_cold"), None);
    // A save is not a lobby. A checkpoint written while that world was on
    // offer still holds a whole game, so it reads back as the default
    // rather than failing to load at all.
    assert_eq!(
        serde_json::from_str::<MapPoles>("\"no_poles\"").unwrap(),
        MapPoles::Poles
    );
    // A flat world is what a lobby gets if it says nothing, and a world
    // with poles is: both are what CIVVIS shipped before either was a
    // choice, so a client that has not been taught about them is unmoved.
    assert_eq!(MapTopology::default(), MapTopology::Flat);
    assert_eq!(MapPoles::default(), MapPoles::Poles);
    assert_eq!(MapScript::default(), MapScript::Pangaea);

    // Both Earth choices are the same world every game. That says how their
    // land is chosen, not which of the independently selected shapes
    // receives it. Only one of them also fixes civilizations to homelands.
    for spec in CIV6_MAP_SCRIPTS {
        assert_eq!(
            spec.script.is_fixed_geography(),
            matches!(spec.script, MapScript::Earth | MapScript::TrueStartEarth),
            "{}",
            spec.id
        );
        assert_eq!(
            spec.script.is_true_start(),
            spec.script == MapScript::TrueStartEarth,
            "{}",
            spec.id
        );
    }

    // `planet` named a world type before the globe became a shape of its
    // own. The old name still resolves, to the type that script generated.
    assert_eq!(
        MapScript::from_id("planet"),
        Some(MapScript::SmallContinents)
    );
    assert_eq!(MapTopology::from_id("planet"), Some(MapTopology::Planet));
    assert_eq!(MapScript::from_id("pangea"), Some(MapScript::Pangaea));

    // Every size resolves from either shape of rectangle, and the two
    // shapes never collide across sizes.
    for size in CIV6_MAP_SIZES {
        assert_eq!(
            MapSize::from_dimensions(size.width, size.height).map(|found| found.id),
            Some(size.id)
        );
        assert_eq!(
            MapSize::from_dimensions(size.globe_width(), size.globe_height()).map(|found| found.id),
            Some(size.id)
        );
        assert_eq!(
            size.dimensions(MapTopology::Planet),
            (size.globe_width(), size.globe_height())
        );
        assert_eq!(
            size.dimensions(MapTopology::Flat),
            (size.width, size.height)
        );
    }
}

#[test]
fn stock_game_speeds_scale_costs_durations_and_turn_limits() {
    // CostMultiplier and the GameSpeed_Turns increments summed, then the
    // SCALING_HALF DefaultCostMultiplier, then the GameSpeed_Durations row
    // for a 30-turn duration -- the length of an alliance and of a World
    // Congress session, so the one that matters most.
    let expected = [
        (GameSpeed::Online, 50, 250, 66, 20),
        (GameSpeed::Quick, 67, 330, 87, 25),
        (GameSpeed::Standard, 100, 500, 100, 30),
        (GameSpeed::Epic, 150, 750, 125, 40),
        (GameSpeed::Marathon, 300, 1500, 200, 60),
    ];
    assert_eq!(CIV6_GAME_SPEEDS.len(), expected.len());
    for (speed, percent, turns, duration_percent, thirty) in expected {
        assert_eq!(speed.cost_percent(), percent);
        assert_eq!(speed.turn_limit(), turns);
        assert_eq!(speed.scale(100.0), percent as f64);
        assert_eq!(speed.duration_percent(), duration_percent);
        assert_eq!(speed.scale_turns(30), thirty);
        assert_eq!(GameSpeed::from_id(speed.id()), Some(speed));
    }

    // A duration is not a cost. Marathon triples what a Settler costs but
    // only doubles how long a peace treaty holds, and Online halves the
    // cost while keeping two thirds of the duration.
    assert_eq!(GameSpeed::Marathon.scale(100.0), 300.0);
    assert_eq!(GameSpeed::Marathon.scale_turns(60), 120);
    assert_eq!(GameSpeed::Online.scale(100.0), 50.0);
    assert_eq!(GameSpeed::Online.scale_turns(60), 40);

    // Every row of GameSpeed_Durations, which does not follow the
    // multiplier: 87% of 29 is 26 but the table says 24, and no speed
    // shortens a 5-turn duration at all.
    let rows: [(u32, [u32; 5]); 6] = [
        (5, [5, 5, 5, 10, 15]),
        (10, [8, 9, 10, 15, 25]),
        (15, [10, 12, 15, 20, 30]),
        (29, [19, 24, 29, 39, 59]),
        (30, [20, 25, 30, 40, 60]),
        (60, [40, 50, 60, 80, 120]),
    ];
    let order = [
        GameSpeed::Online,
        GameSpeed::Quick,
        GameSpeed::Standard,
        GameSpeed::Epic,
        GameSpeed::Marathon,
    ];
    for (standard, scaled) in rows {
        for (speed, want) in order.iter().zip(scaled) {
            assert_eq!(
                speed.scale_turns(standard),
                want,
                "{speed:?} for {standard}"
            );
        }
    }
}

#[test]
fn every_speed_is_applied_to_live_research_growth_and_production_costs() {
    let size = MapSize::for_players(2);
    let mut game = Game::new_with_setup(
        2,
        size.width,
        size.height,
        701,
        GameSpeed::Online.turn_limit(),
        0,
        MapScript::Pangaea,
        GameSpeed::Online,
        false,
    );
    let settler = game
        .units
        .values()
        .find(|unit| unit.owner == 0 && unit.kind == "settler")
        .unwrap()
        .id;
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = game.player_city_ids(0)[0];
    let monument = Item::Building {
        building: crate::name!("monument"),
    };
    for speed in [
        GameSpeed::Online,
        GameSpeed::Quick,
        GameSpeed::Standard,
        GameSpeed::Epic,
        GameSpeed::Marathon,
    ] {
        game.game_speed = speed;
        let multiplier = speed.cost_percent() as f64 / 100.0;
        assert_eq!(
            game.tech_cost("pottery"),
            game.rules.techs["pottery"].cost * multiplier
        );
        assert_eq!(game.growth_cost(1), 15.0 * multiplier);
        // Durations ride the separate SCALING_HALF curve, not `multiplier`.
        assert_eq!(game.standard_duration(30), speed.scale_turns(30));
        assert_eq!(
            game.item_cost_for_city(0, city, &monument),
            game.rules.buildings["monument"].cost * multiplier
        );
    }

    game.map_script = MapScript::SmallContinents;
    let restored: Game = serde_json::from_str(&serde_json::to_string(&game).unwrap()).unwrap();
    assert_eq!(restored.game_speed, GameSpeed::Marathon);
    assert_eq!(restored.map_script, MapScript::SmallContinents);
}

#[test]
fn requested_player_counts_use_civ6_dimensions_and_defaults() {
    let tiny = MapSize::for_players(4);
    assert_eq!((tiny.name, tiny.width, tiny.height), ("Tiny", 60, 38));
    assert_eq!(
        (
            tiny.default_city_states,
            tiny.natural_wonders,
            tiny.max_religions,
            tiny.continents
        ),
        (6, 3, 3, 2)
    );

    let small = MapSize::for_players(6);
    assert_eq!((small.name, small.width, small.height), ("Small", 74, 46));
    assert_eq!(
        (
            small.default_city_states,
            small.natural_wonders,
            small.max_religions,
            small.continents
        ),
        (9, 4, 4, 3)
    );
}

#[test]
fn dimensions_round_trip_for_every_stock_size() {
    for players in [2, 4, 6, 8, 10, 12, 15, 20, 50, 100] {
        let size = MapSize::for_players(players);
        assert_eq!(
            MapSize::from_dimensions(size.width, size.height),
            Some(size)
        );
    }
}

/// The four worlds past Huge are extrapolations, not shipped rows, so what
/// pins them is the ratios they extrapolate from. Every stock size holds
/// about 580 tiles per major civilization on a roughly 1.6:1 rectangle and
/// seats exactly 1.5 city-states per major; a scaled row that quietly drifts
/// off those is a world that no longer plays like a Civilization VI map.
#[test]
fn the_scaled_worlds_keep_the_stock_ratios_and_stay_inside_the_roster() {
    let scaled = [
        ("massive", 15),
        ("enormous", 20),
        ("colossal", 50),
        ("ludicrous", 100),
    ];
    for (id, players) in scaled {
        let size = CIV6_MAP_SIZES.iter().find(|size| size.id == id).unwrap();
        assert_eq!(size.default_players, players, "{id} seats");
        assert_eq!(
            MapSize::for_players(players).id,
            id,
            "{id} is chosen for {players}"
        );

        let tiles = (size.width * size.height) as f64;
        let per_civ = tiles / players as f64;
        assert!(
            (560.0..=600.0).contains(&per_civ),
            "{id} gives each civilization {per_civ:.0} tiles, outside the stock 567-583 band"
        );
        let aspect = size.width as f64 / size.height as f64;
        assert!((1.55..=1.65).contains(&aspect), "{id} is {aspect:.2}:1");
        // Exactly 1.5 per major, rounded down: an odd seat count cannot
        // halve, and Massive's fifteen majors take 22 rather than 22.5.
        assert_eq!(
            size.default_city_states,
            players * 3 / 2,
            "{id} should seat 1.5 city-states per major"
        );
        assert_eq!(size.continents, players / 2, "{id} continents");

        // Nothing may seat more majors than there are civilizations to
        // seat, or more city-states than the ruleset has identities for.
        assert!(
            size.max_players <= crate::game::CIV_NAMES.len(),
            "{id} seats up to {} majors but the roster holds {}",
            size.max_players,
            crate::game::CIV_NAMES.len()
        );
        assert!(
            size.max_city_states <= crate::game::CITY_STATE_NAMES.len(),
            "{id} seats up to {} city-states but the ruleset names {}",
            size.max_city_states,
            crate::game::CITY_STATE_NAMES.len()
        );
        // Religions and natural wonders both follow `players / 2 + 1`, but
        // a map cannot draw more wonders than the ruleset defines, so that
        // one is the rule capped by the catalogue. Ludicrous is the only
        // size the cap actually binds.
        assert_eq!(size.max_religions, players / 2 + 1, "{id} religions");
        let catalogue = crate::rules::Rules::embedded()
            .features
            .values()
            .filter(|feature| feature.natural_wonder)
            .count();
        assert_eq!(
            size.natural_wonders,
            (players / 2 + 1).min(catalogue),
            "{id} should draw players/2+1 wonders, capped at the {catalogue} the ruleset carries"
        );
    }
}

fn assert_generated_profile(players: usize, seed: u64) {
    let size = MapSize::for_players(players);
    let mut game = Game::new_full(
        players,
        size.width,
        size.height,
        seed,
        50,
        size.default_city_states,
        false,
    );
    assert_eq!((game.map.width, game.map.height), (size.width, size.height));
    assert_eq!(game.map.tiles.len(), (size.width * size.height) as usize);
    assert_eq!(game.players.iter().filter(|p| !p.is_minor).count(), players);
    assert_eq!(
        game.players
            .iter()
            .filter(|p| p.is_minor && !p.is_barbarian)
            .count(),
        size.default_city_states
    );
    let wonders: BTreeSet<&str> = game
        .map
        .tiles
        .values()
        .filter_map(|tile| {
            let feature = tile.feature.as_deref()?;
            game.rules.features[feature]
                .natural_wonder
                .then_some(feature)
        })
        .collect();
    assert_eq!(
        wonders.len(),
        size.natural_wonders,
        "{} generated unexpected natural wonders: {wonders:?}",
        size.name
    );
    let continents: BTreeSet<usize> = game
        .map
        .tiles
        .values()
        .filter_map(|tile| tile.continent)
        .collect();
    assert_eq!(continents.len(), size.continents);
    assert_eq!(game.max_religions(), size.max_religions);

    for pid in 0..size.max_religions {
        game.players[pid].religion = Some(format!("Religion {pid}"));
    }
    if size.max_religions < players {
        let blocked = size.max_religions;
        game.players[blocked].prophet_pending = true;
        assert!(!game
            .legal_actions(blocked)
            .iter()
            .any(|action| matches!(action, Action::FoundReligion { .. })));
    }
}

#[test]
fn every_selectable_world_generates_its_complete_profile() {
    for (players, seed) in [(2, 21), (4, 41), (6, 61), (8, 81), (10, 101), (12, 121)] {
        assert_generated_profile(players, seed);
    }
}
