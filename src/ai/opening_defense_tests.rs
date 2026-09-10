use super::*;

fn raid(raiders: usize, defenders: usize) -> (Game, u32) {
    let mut g = Game::new_full(1, 24, 18, 91_527, 80, 0, true);
    let founder = g
        .player_unit_ids(0)
        .into_iter()
        .find(|id| g.units[id].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: founder }).unwrap();
    let city = g.player_city_ids(0)[0];
    let home = g.cities[&city].pos;
    for id in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(id);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    g.barb_camp_guards.clear();
    for tile in g.map.tiles.values_mut() {
        if tile.improvement.as_deref() == Some("barbarian_camp") {
            tile.improvement = None;
        }
    }
    let positions: Vec<_> = g
        .wdisk(home, 3)
        .into_iter()
        .filter(|p| {
            *p != home
                && g.map
                    .get(*p)
                    .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
        })
        .collect();
    assert!(positions.len() >= raiders + defenders);
    for pos in positions.iter().take(defenders) {
        g.spawn_test_unit("archer", 0, *pos);
    }
    for pos in positions.iter().skip(defenders).take(raiders) {
        g.spawn_test_unit("warrior", g.barb_pid.unwrap(), *pos);
    }
    (g, city)
}

#[test]
fn a_large_opening_raid_still_requests_defenders_when_two_are_present() {
    let (g, city) = raid(5, 2);
    let mut ai = BasicAi::new();
    ai.garrison_under_fire = true;
    assert_eq!(ai.barbarian_defense_gap(&g, 0, city), 2);
    assert!(
        matches!(
            ai.barbarian_defense_item(&g, 0, city),
            Some(Item::Unit { .. })
        ),
        "two local archers must not turn a five-raider attack into an economic queue"
    );
}

#[test]
fn raid_mobilization_is_bounded_and_releases_when_the_attack_shrinks() {
    let (mut g, city) = raid(6, 4);
    let mut ai = BasicAi::new();
    ai.garrison_under_fire = true;
    assert_eq!(
        ai.barbarian_defense_gap(&g, 0, city),
        0,
        "four defenders bound the local response"
    );
    for id in g.player_unit_ids(0).into_iter().take(2) {
        g.remove_unit(id);
    }
    assert_eq!(ai.barbarian_defense_gap(&g, 0, city), 2);
    for id in g.player_unit_ids(g.barb_pid.unwrap()).into_iter().take(4) {
        g.remove_unit(id);
    }
    assert_eq!(
        ai.barbarian_defense_gap(&g, 0, city),
        0,
        "two defenders cover the remaining pair"
    );
    for id in g.player_unit_ids(g.barb_pid.unwrap()) {
        g.remove_unit(id);
    }
    assert_eq!(ai.barbarian_defense_gap(&g, 0, city), 0);
}

#[test]
fn ordinary_small_raids_and_the_unarmed_controller_keep_their_floor() {
    for count in 1..=2 {
        let (g, city) = raid(count, 1);
        let mut ai = BasicAi::new();
        let old = ai.barbarian_defense_gap(&g, 0, city);
        ai.garrison_under_fire = true;
        assert_eq!(ai.barbarian_defense_gap(&g, 0, city), old);
    }
    let (g, city) = raid(5, 2);
    assert_eq!(BasicAi::new().barbarian_defense_gap(&g, 0, city), 0);
}

#[test]
fn the_opening_raid_floor_ends_with_the_classical_window() {
    let (mut g, city) = raid(5, 2);
    let mut ai = BasicAi::new();
    ai.garrison_under_fire = true;
    let later = *g
        .rules
        .techs
        .iter()
        .find(|(_, spec)| spec.era > 1)
        .unwrap()
        .0;
    g.players[0].techs.insert(later);
    assert_eq!(ai.barbarian_defense_gap(&g, 0, city), 0);
}
