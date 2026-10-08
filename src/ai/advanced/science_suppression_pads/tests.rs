use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;

/// Two majors at war. Seat 1 has a capital with a Theatre Square and two
/// more cities with Spaceports: `near` (nearer our capital) and `far`. Its
/// three space projects are landed. Returns (game, capital, near, far).
fn board() -> (Game, u32, u32, u32) {
    let mut g = Game::new_full(2, 40, 26, 91_021, 250, 0, false);
    for pid in 0..2 {
        let settler = g
            .player_unit_ids(pid)
            .into_iter()
            .find(|uid| g.units[uid].kind == "settler")
            .expect("each fixture major begins with a Settler");
        let position = g.units[&settler].pos;
        g.found_city_for(pid, position, None);
        for uid in g.player_unit_ids(pid) {
            g.remove_unit(uid);
        }
    }
    let home = g.cities[&g.player_city_ids(0)[0]].pos;
    let capital = g.player_city_ids(1)[0];
    let mut founded = Vec::new();
    for _ in 0..2 {
        let site = g
            .map
            .tiles
            .keys()
            .copied()
            .filter(|pos| {
                let tile = &g.map.tiles[pos];
                tile.owner_city.is_none()
                    && g.rules.is_passable(tile)
                    && !g.rules.is_water(tile)
                    && g.cities.values().all(|city| g.wdist(city.pos, *pos) >= 5)
            })
            .min_by_key(|pos| (g.wdist(g.cities[&capital].pos, *pos), *pos))
            .expect("room for another rival city");
        founded.push(g.found_city_for(1, site, None));
    }
    founded.sort_by_key(|cid| (g.wdist(home, g.cities[cid].pos), *cid));
    let (near, far) = (founded[0], founded[1]);
    assert!(g.wdist(home, g.cities[&near].pos) < g.wdist(home, g.cities[&far].pos));
    let capital_pos = g.cities[&capital].pos;
    g.cities
        .get_mut(&capital)
        .unwrap()
        .districts
        .insert(crate::name!("theater_square"), capital_pos);
    for cid in [near, far] {
        place_pad(&mut g, cid);
    }
    for project in [
        "launch_earth_satellite",
        "launch_moon_landing",
        "launch_mars_colony",
    ] {
        g.players[1].science_projects.insert(project.to_string());
    }
    g.turn = 220;
    g.record_contact(0, 1);
    g.at_war.insert((0, 1));
    g.at_war.insert((1, 0));
    (g, capital, near, far)
}

/// A standing Spaceport on a free land neighbour of `cid`.
fn place_pad(g: &mut Game, cid: u32) -> Pos {
    let centre = g.cities[&cid].pos;
    let pad = g
        .wdisk(centre, 1)
        .into_iter()
        .find(|pos| {
            *pos != centre
                && g.map
                    .get(*pos)
                    .is_some_and(|tile| tile.district.is_none() && !g.rules.is_water(tile))
        })
        .expect("a free land neighbour");
    let tile = g.map.tiles.get_mut(&pad).unwrap();
    tile.owner_city = Some(cid);
    tile.district = Some(crate::name!("spaceport"));
    tile.pillaged = false;
    tile.improvement = None;
    g.cities
        .get_mut(&cid)
        .unwrap()
        .districts
        .insert(crate::name!("spaceport"), pad);
    pad
}

fn pillage_pad(g: &mut Game, cid: u32) {
    let pad = *g.cities[&cid]
        .districts
        .get(crate::name!("spaceport"))
        .unwrap();
    g.map.tiles.get_mut(&pad).unwrap().pillaged = true;
}

/// G408's reading: the racer counted on Culture.
fn culture() -> VictoryFocus {
    VictoryFocus {
        strategy: GrandStrategy::Culture,
        progress: 89,
    }
}

fn stock() -> AdvancedAi {
    AdvancedAi::targeting(VictoryTarget::Domination)
}

fn armed() -> AdvancedAi {
    let mut ai = stock();
    ai.enable_science_suppression_hits_the_pads();
    ai
}

#[test]
fn hits_the_pads_is_opt_in() {
    opt_in_off_in_both_controllers("science-suppression-hits-the-pads", |ai| {
        ai.science_suppression_hits_the_pads
    });
}

#[test]
fn a_culture_reading_racer_is_suppressed_at_its_nearest_standing_pad() {
    let (g, capital, near, _) = board();
    assert_eq!(
        stock().victory_suppression_city(&g, 0, 1, culture()),
        Some(capital),
        "stock: the Culture lane names the Theatre Square city"
    );
    assert_eq!(
        armed().victory_suppression_city(&g, 0, 1, culture()),
        Some(near)
    );
}

#[test]
fn a_pillaged_pad_is_passed_over_for_a_standing_one() {
    // G408: Faras' pad, pillaged by our spy, was the one named.
    let (mut g, capital, near, far) = board();
    pillage_pad(&mut g, near);
    assert_eq!(
        armed().victory_suppression_city(&g, 0, 1, culture()),
        Some(far)
    );
    pillage_pad(&mut g, far);
    assert_eq!(
        armed().victory_suppression_city(&g, 0, 1, culture()),
        Some(capital),
        "no standing pad: the lane's choice stands"
    );
}

#[test]
fn a_rival_short_of_the_moon_keeps_the_lane_choice() {
    let (mut g, capital, _, _) = board();
    g.players[1].science_projects.remove("launch_mars_colony");
    g.players[1].science_projects.remove("launch_moon_landing");
    assert_eq!(
        armed().victory_suppression_city(&g, 0, 1, culture()),
        Some(capital)
    );
}

#[test]
fn a_mars_landed_racer_is_the_domination_counter_until_it_launches() {
    let (mut g, _, _, _) = board();
    assert_eq!(stock().mars_racer_counter(&g, 0), None, "off: nothing");
    assert_eq!(
        armed().mars_racer_counter(&g, 0),
        Some((1, GrandStrategy::Conquest))
    );
    let mut science_lane = AdvancedAi::targeting(VictoryTarget::Science);
    science_lane.enable_science_suppression_hits_the_pads();
    assert_eq!(
        science_lane.mars_racer_counter(&g, 0),
        None,
        "only a Domination army answers with war here"
    );
    g.players[1]
        .science_projects
        .insert("exoplanet_expedition".to_string());
    assert_eq!(
        armed().mars_racer_counter(&g, 0),
        None,
        "past the launch no war counts"
    );
    g.players[1].science_projects.remove("exoplanet_expedition");
    g.players[1].science_projects.remove("launch_mars_colony");
    assert_eq!(armed().mars_racer_counter(&g, 0), None, "short of Mars");
}
