use super::*;
use crate::ai::advanced::near_rival_deterrence::{
    DETERRENCE_NEAR_TILES, DETERRENCE_START_STANDARD,
};

/// Two majors who have met at peace, one city each within the deterrence
/// radius, our capital idle and every unit removed, inside the window, our
/// seat on the Domination lane. Returns the game and our city.
fn neighbours() -> Option<(Game, u32)> {
    let mut game = Game::new_full(2, 32, 20, 936_221, 200, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")?;
    game.apply(0, &Action::FoundCity { unit: settler }).ok()?;
    for pid in [0, 1] {
        for uid in game.player_unit_ids(pid) {
            game.remove_unit(uid);
        }
    }
    let ours = game.player_city_ids(0)[0];
    let home = game.cities[&ours].pos;
    let mut sites: Vec<Pos> = game
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| {
            (5..=DETERRENCE_NEAR_TILES).contains(&game.wdist(**pos, home))
                && !game.rules.is_water(tile)
        })
        .map(|(pos, _)| *pos)
        .collect();
    sites.sort();
    let site = *sites.first()?;
    game.place_city(1, site, None);
    game.players[0].met.insert(1);
    game.players[1].met.insert(0);
    game.players[0].techs.insert(crate::name!("archery"));
    game.players[0].techs.insert(crate::name!("bronze_working"));
    let city = game.cities.get_mut(&ours).unwrap();
    city.pop = 4;
    city.queue.clear();
    game.turn = game.standard_duration(DETERRENCE_START_STANDARD) + 5;
    Some((game, ours))
}

fn power(game: &mut Game, pid: usize, value: f64) {
    std::sync::Arc::make_mut(&mut game.observed_military_power).insert(pid, value);
}

fn deterrence(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.victory_target = Some(VictoryTarget::Domination);
    ai.enable_near_rival_deterrence();
    if gene {
        ai.enable_deterrence_stops_at_a_garrison();
    }
    ai
}

fn claim(ai: &AdvancedAi, game: &Game) -> Option<Item> {
    let mut game = game.clone();
    let plan = ai.assess(&game, 0);
    ai.claim_deterrence_unit(&mut game, 0, &plan);
    let cid = game.player_city_ids(0)[0];
    game.cities[&cid].queue.first().cloned()
}

/// Against a neighbour of 2.5x our power, a city that holds a ranged
/// garrison trains no deterrent (off, it trains one); a city without one
/// trains only an Archer; within reach (1.5x) the garrisoned city trains as
/// before.
#[test]
fn an_out_of_reach_deterrent_stops_at_a_ranged_garrison() {
    let Some((mut game, ours)) = neighbours() else {
        panic!("fixture: two capitals within the neighbour radius");
    };
    power(&mut game, 0, 100.0);
    power(&mut game, 1, 250.0);
    let archer = Item::Unit {
        unit: crate::name!("archer"),
    };

    let bare = claim(&deterrence(true), &game);
    assert_eq!(bare, Some(archer.clone()), "no garrison: an Archer");

    let mut garrisoned = game.clone();
    let pos = garrisoned.cities[&ours].pos;
    garrisoned.spawn_unit("archer", 0, pos);
    assert!(AdvancedAi::city_has_ranged_garrison(&garrisoned, 0, ours));
    assert!(
        claim(&deterrence(false), &garrisoned).is_some(),
        "off: a deterrent"
    );
    assert_eq!(claim(&deterrence(true), &garrisoned), None, "on: none");

    let mut near = garrisoned.clone();
    power(&mut near, 1, 150.0);
    assert!(
        claim(&deterrence(true), &near).is_some(),
        "within reach the deterrent stands"
    );
}

/// The hold: out of reach, a queued land unit in a garrisoned city no longer
/// holds the queue against the economy's claims (off, it does); a
/// garrisonless city's Archer still does.
#[test]
fn an_out_of_reach_deterrent_holds_only_a_garrison() {
    let Some((mut game, ours)) = neighbours() else {
        panic!("fixture: two capitals within the neighbour radius");
    };
    power(&mut game, 0, 100.0);
    power(&mut game, 1, 250.0);
    let archer = Item::Unit {
        unit: crate::name!("archer"),
    };
    game.cities.get_mut(&ours).unwrap().queue = vec![archer.clone()];
    assert!(deterrence(true).deterrence_unit_holds(&game, 0, &archer));

    let pos = game.cities[&ours].pos;
    game.spawn_unit("archer", 0, pos);
    assert!(deterrence(false).deterrence_unit_holds(&game, 0, &archer));
    assert!(!deterrence(true).deterrence_unit_holds(&game, 0, &archer));
}

/// The split: `hopeless-army-yields-the-queue` alone no longer caps the
/// deterrent (a garrisoned city still trains one against 2.5x), and the cap
/// reads nothing on another lane.
#[test]
fn only_this_gene_caps_the_deterrent_and_only_on_the_domination_lane() {
    let Some((mut game, ours)) = neighbours() else {
        panic!("fixture: two capitals within the neighbour radius");
    };
    power(&mut game, 0, 100.0);
    power(&mut game, 1, 250.0);
    let pos = game.cities[&ours].pos;
    game.spawn_unit("archer", 0, pos);

    let mut siege_only = deterrence(false);
    siege_only.enable_hopeless_army_yields_the_queue();
    assert!(!siege_only.deterrence_out_of_reach(&game, 0, 250.0));
    assert!(
        claim(&siege_only, &game).is_some(),
        "the siege gene leaves the deterrent alone"
    );

    let mut science = deterrence(true);
    science.victory_target = Some(VictoryTarget::Science);
    assert!(!science.deterrence_out_of_reach(&game, 0, 250.0));
    assert!(deterrence(true).deterrence_out_of_reach(&game, 0, 250.0));
}
