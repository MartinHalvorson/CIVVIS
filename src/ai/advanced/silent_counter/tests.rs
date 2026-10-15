use super::super::{AdvancedAi, VictoryTarget};
use crate::game::Game;

/// A land strip: us at the west end, a civilization with closed borders
/// across the middle, the racer at the east end. Everyone has met.
fn strip() -> (Game, AdvancedAi) {
    let mut g = Game::new_full(3, 40, 12, 931_036, 300, 0, false);
    for unit in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(unit);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = if (4..=8).contains(&tile.pos.1) && (2..=36).contains(&tile.pos.0) {
            crate::name!("grassland")
        } else {
            crate::name!("ocean")
        };
        tile.feature = None;
        tile.hills = false;
    }
    g.found_city_for(0, (4, 6), None);
    let screen = g.found_city_for(1, (18, 6), None);
    g.found_city_for(2, (32, 6), None);
    for tile in g.map.tiles.values_mut() {
        if (14..=22).contains(&tile.pos.0) && (4..=8).contains(&tile.pos.1) {
            tile.owner_city = Some(screen);
        }
    }
    g.players[1].borders_enforced = Some(true);
    for other in [1, 2] {
        g.record_contact(0, other);
    }
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_denial_needs_a_road();
    (g, ai)
}

/// The clause names the civilization whose closed borders shut the road,
/// says "across water" when no border is to blame, and "not met" first.
#[test]
fn the_clause_names_what_holds_the_counter() {
    let (mut g, ai) = strip();
    let clause = ai.silent_counter_clause(&g, 0, 2);
    assert_eq!(clause.kind, "closed-borders");
    assert_eq!(clause.blocker, Some(1));
    assert_eq!(
        clause.text,
        format!(
            "has no land road (behind {}'s closed borders)",
            g.players[1].civ
        )
    );
    // Cut the strip west of the middle civilization: water, not borders.
    for tile in g.map.tiles.values_mut() {
        if (10..=11).contains(&tile.pos.0) {
            tile.terrain = crate::name!("ocean");
        }
    }
    assert_eq!(ai.silent_counter_clause(&g, 0, 2).kind, "water");

    let (mut g, ai) = strip();
    g.players[0].met.remove(&2);
    g.players[2].met.remove(&0);
    assert!(!g.has_met(0, 2), "fixture: unmet");
    assert_eq!(ai.silent_counter_clause(&g, 0, 2).kind, "unmet");
}

/// An urgent Science racer behind closed borders: the Conquest counter has
/// no road, nothing is chosen, and the journal says why once per window and
/// again after it. It decides nothing.
#[test]
fn an_urgent_racer_with_no_road_is_journalled_and_decides_nothing() {
    let (mut g, mut ai) = strip();
    for project in [
        "launch_earth_satellite",
        "launch_moon_landing",
        "launch_mars_colony",
        "exoplanet_expedition",
    ] {
        g.players[2].science_projects.insert(project.to_string());
    }
    let denial = ai.actionable_victory_denial(&g, 0);
    assert_ne!(
        denial.map(|(rival, _)| rival),
        Some(2),
        "fixture: no executable counter against the racer"
    );
    ai.base.journal = crate::reasoning::Journal::recording();
    let headline = format!("No counter for {}", g.players[2].civ);
    let lines = |ai: &AdvancedAi| -> Vec<String> {
        ai.base
            .journal
            .since(0)
            .thoughts
            .iter()
            .filter(|thought| thought.headline == headline)
            .map(|thought| thought.detail.clone())
            .collect()
    };
    ai.journal_silent_counter(&g, 0);
    let first = lines(&ai);
    assert_eq!(first.len(), 1, "journalled");
    assert!(
        first[0].starts_with("its science race reads "),
        "{}",
        first[0]
    );
    assert!(
        first[0].ends_with(&format!(
            "urgent; the Conquest counter has no land road (behind {}'s closed borders)",
            g.players[1].civ
        )),
        "{}",
        first[0]
    );
    ai.journal_silent_counter(&g, 0);
    assert_eq!(lines(&ai).len(), 1, "not repeated inside the note window");
    g.turn += g.standard_duration(super::SILENT_COUNTER_NOTE_TURNS);
    ai.journal_silent_counter(&g, 0);
    assert_eq!(lines(&ai).len(), 2, "repeated after the window");
    assert_eq!(
        ai.actionable_victory_denial(&g, 0),
        denial,
        "it decides nothing"
    );

    // A war on the middle civilization opens the road: the counter is
    // executable and the silent line stops.
    g.at_war.insert((0, 1));
    g.at_war.insert((1, 0));
    g.turn += g.standard_duration(super::SILENT_COUNTER_NOTE_TURNS);
    ai.journal_silent_counter(&g, 0);
    assert_eq!(lines(&ai).len(), 2, "an executable counter is not silent");
}

/// `strip` with a Science racer past its Exoplanet launch at the east end,
/// military read from the host -- ours 4,000, the screen's `screen`, the
/// racer's 500 -- under `racer-blocker-declares` when `gene`.
fn racer_strip(screen: f64, gene: bool) -> (Game, AdvancedAi) {
    let (mut g, mut ai) = strip();
    for project in [
        "launch_earth_satellite",
        "launch_moon_landing",
        "launch_mars_colony",
        "exoplanet_expedition",
    ] {
        g.players[2].science_projects.insert(project.to_string());
    }
    for (seat, power) in [(0, 4000.0), (1, screen), (2, 500.0)] {
        std::sync::Arc::make_mut(&mut g.observed_military_power).insert(seat, power);
    }
    g.turn = 150;
    if gene {
        ai.enable_racer_blocker_declares();
    }
    (g, ai)
}

fn screen_city(g: &Game) -> u32 {
    g.player_city_ids(1)[0]
}

/// A weak screen is the road-blocker front, its city on the road the
/// objective; with the gene off, neither.
#[test]
fn a_weak_single_blocker_is_the_front_and_its_road_city_the_objective() {
    let (g, ai) = racer_strip(600.0, true);
    assert_eq!(ai.racer_blocker_front(&g, 0), Some(1));
    assert_eq!(ai.road_blocker_front(&g, 0), Some(1));
    assert_eq!(
        ai.racer_blocker_objective(&g, 0),
        Some((1, screen_city(&g)))
    );
    let (g, ai) = racer_strip(600.0, false);
    assert_eq!(ai.racer_blocker_front(&g, 0), None, "gene off");
    assert_eq!(ai.road_blocker_front(&g, 0), None, "gene off");
}

/// The objective is the blocker's city whose ground the road enters, not
/// its capital further off the road.
#[test]
fn the_objective_is_the_city_on_the_road_not_the_capital() {
    let (mut g, ai) = racer_strip(600.0, true);
    let capital = g.found_city_for(1, (18, 10), None);
    for city in g.cities.values_mut() {
        city.is_capital = city.id == capital;
    }
    assert_eq!(
        ai.racer_blocker_objective(&g, 0),
        Some((1, screen_city(&g)))
    );
}

/// A screen past half our power is no front, and the journal says why.
#[test]
fn a_strong_blocker_is_a_near_miss_not_a_front() {
    let (g, mut ai) = racer_strip(3000.0, true);
    assert_eq!(ai.racer_blocker_front(&g, 0), None);
    assert_eq!(ai.road_blocker_front(&g, 0), None);
    ai.base.journal = crate::reasoning::Journal::recording();
    ai.journal_silent_counter(&g, 0);
    let held = format!("Racer's road blocker held: {}", g.players[1].civ);
    let thoughts = ai.base.journal.since(0).thoughts;
    let line = thoughts
        .iter()
        .find(|thought| thought.headline == held)
        .expect("the near miss is journalled");
    assert!(
        line.detail
            .contains("our 4000 power against their steady 3000"),
        "{}",
        line.detail
    );
}

/// Our own city under threat holds the front: the last plan defended it.
#[test]
fn a_threatened_city_of_ours_holds_the_front() {
    let (g, mut ai) = racer_strip(600.0, true);
    ai.plan = Some(super::super::StrategicPlan {
        strategy: super::super::GrandStrategy::Conquest,
        target_player: None,
        target_city: None,
        threatened_city: g.player_city_ids(0).first().copied(),
        desired_cities: 4,
        assessed_turn: g.turn,
        rush: false,
    });
    assert_eq!(ai.racer_blocker_front(&g, 0), None);
}

/// Two screens, each alone short of opening the road: no single blocker,
/// no front.
#[test]
fn two_blockers_leave_it_unchanged() {
    let mut g = Game::new_full(4, 40, 12, 931_036, 300, 0, false);
    for unit in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(unit);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = if (4..=8).contains(&tile.pos.1) && (2..=36).contains(&tile.pos.0) {
            crate::name!("grassland")
        } else {
            crate::name!("ocean")
        };
        tile.feature = None;
        tile.hills = false;
    }
    g.found_city_for(0, (4, 6), None);
    let first = g.found_city_for(1, (14, 6), None);
    let second = g.found_city_for(2, (24, 6), None);
    g.found_city_for(3, (33, 6), None);
    for tile in g.map.tiles.values_mut() {
        if (4..=8).contains(&tile.pos.1) {
            if (11..=17).contains(&tile.pos.0) {
                tile.owner_city = Some(first);
            } else if (21..=27).contains(&tile.pos.0) {
                tile.owner_city = Some(second);
            }
        }
    }
    for screen in [1, 2] {
        g.players[screen].borders_enforced = Some(true);
    }
    for other in [1, 2, 3] {
        g.record_contact(0, other);
    }
    for project in [
        "launch_earth_satellite",
        "launch_moon_landing",
        "launch_mars_colony",
        "exoplanet_expedition",
    ] {
        g.players[3].science_projects.insert(project.to_string());
    }
    for (seat, power) in [(0, 4000.0), (1, 500.0), (2, 500.0), (3, 500.0)] {
        std::sync::Arc::make_mut(&mut g.observed_military_power).insert(seat, power);
    }
    g.turn = 150;
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_denial_needs_a_road();
    ai.enable_racer_blocker_declares();
    assert_eq!(ai.silent_counter_clause(&g, 0, 3).kind, "borders");
    assert_eq!(ai.racer_blocker_front(&g, 0), None);
}

/// The desk declares on the weak screen with the plan aimed at its road
/// city, and only under the gene.
#[test]
fn the_desk_declares_on_a_weak_racer_blocker() {
    for gene in [false, true] {
        let (mut g, mut ai) = racer_strip(600.0, gene);
        // The desk asks for a second city of ours before any elective war.
        g.found_city_for(0, (8, 4), None);
        g.players[0].gold = 3000.0;
        ai.coalition_before_war = false;
        ai.coalition_before_war_2 = false;
        ai.coalition_before_war_3 = false;
        let mut plan = super::super::StrategicPlan {
            strategy: super::super::GrandStrategy::Conquest,
            target_player: Some(1),
            target_city: Some(screen_city(&g)),
            threatened_city: None,
            desired_cities: 4,
            assessed_turn: g.turn,
            rush: false,
        };
        ai.base.journal = crate::reasoning::Journal::recording();
        for _ in 0..12 {
            if g.is_at_war(0, 1) {
                break;
            }
            plan.assessed_turn = g.turn;
            ai.advanced_diplomacy(&mut g, 0, &plan);
            g.turn += 1;
        }
        assert_eq!(g.is_at_war(0, 1), gene, "gene {gene}");
        if gene {
            let declaring = format!("Declaring war on {}", g.players[1].civ);
            let thoughts = ai.base.journal.since(0).thoughts;
            let line = thoughts
                .iter()
                .find(|thought| thought.headline == declaring)
                .expect("the declaration is journalled");
            assert!(
                line.detail.contains(&format!(
                    "racer-blocker-declares: they shut the only land road to {}",
                    g.players[2].civ
                )),
                "{}",
                line.detail
            );
        }
    }
}
