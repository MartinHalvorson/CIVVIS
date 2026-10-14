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
    let (kind, text) = ai.silent_counter_clause(&g, 0, 2);
    assert_eq!(kind, "closed-borders");
    assert_eq!(
        text,
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
    assert_eq!(ai.silent_counter_clause(&g, 0, 2).0, "water");

    let (mut g, ai) = strip();
    g.players[0].met.remove(&2);
    g.players[2].met.remove(&0);
    assert!(!g.has_met(0, 2), "fixture: unmet");
    assert_eq!(ai.silent_counter_clause(&g, 0, 2).0, "unmet");
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
