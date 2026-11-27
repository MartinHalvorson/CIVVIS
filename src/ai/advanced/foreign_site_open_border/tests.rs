use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::ai::advanced::{GrandStrategy, VictoryTarget};
use crate::game::LiveGreatPersonActivationNeed;

const TAG: &str = "foreign-site-buys-the-open-border";

/// Four majors and a city-state, each with one city; seat 0 has met them all
/// and explored the whole map, every major can trade passage and holds Gold,
/// and no natural wonder is on the map until a test lays one. Turn 151 is off
/// seat 0's six-turn trade cadence, so any grant on the board came from this
/// gene.
fn board() -> Game {
    let mut g = Game::new_full(4, 30, 16, 79_104, 200, 1, false);
    for pid in 0..g.players.len() {
        let settler = g
            .player_unit_ids(pid)
            .into_iter()
            .find(|id| g.units[id].kind == "settler");
        if let Some(uid) = settler {
            let pos = g.units[&uid].pos;
            g.found_city_for(pid, pos, None);
            g.remove_unit(uid);
        }
    }
    for other in 1..g.players.len() {
        g.record_contact(0, other);
    }
    g.turn = 151;
    for pid in 0..4 {
        g.players[pid].gold = 1_000.0;
        g.players[pid].civics.insert(crate::name!("early_empire"));
    }
    let wonders: Vec<Pos> = g
        .map
        .tiles
        .iter()
        .filter(|(_, tile)| g.tile_is_natural_wonder(tile))
        .map(|(pos, _)| *pos)
        .collect();
    for pos in wonders {
        g.map.tiles.get_mut(&pos).unwrap().feature = None;
    }
    let everywhere: Vec<Pos> = g.map.tiles.iter().map(|(pos, _)| *pos).collect();
    g.players[0].explored.extend(everywhere);
    g
}

fn city_state(g: &Game) -> usize {
    (0..g.players.len())
        .find(|pid| g.players[*pid].is_minor && !g.player_city_ids(*pid).is_empty())
        .expect("the board has a city-state with a city")
}

fn home(g: &Game) -> Pos {
    g.cities[&g.player_city_ids(0)[0]].pos
}

/// A completed Campus in `owner`'s city; returns its tile.
fn campus(g: &mut Game, owner: usize) -> Pos {
    let cid = g.player_city_ids(owner)[0];
    let centre = g.cities[&cid].pos;
    let site = g.cities[&cid]
        .owned_tiles
        .iter()
        .copied()
        .find(|pos| *pos != centre && g.map.tiles[pos].district.is_none())
        .expect("a free tile in the city");
    g.cities
        .get_mut(&cid)
        .unwrap()
        .districts
        .insert(crate::name!("campus"), site);
    g.map.tiles.get_mut(&site).unwrap().district = Some(crate::name!("campus"));
    site
}

/// A natural wonder on a tile of `owner`'s city whose every neighbour that
/// city also owns.
fn wonder_inside(g: &mut Game, owner: usize) -> Pos {
    let cid = g.player_city_ids(owner)[0];
    let centre = g.cities[&cid].pos;
    let site = g.nbrs(centre).into_iter().next().unwrap();
    let mut claimed = vec![site];
    claimed.extend(g.nbrs(site));
    for pos in claimed {
        let previous = g.map.tiles.get(&pos).and_then(|tile| tile.owner_city);
        if let Some(old) = previous.and_then(|old| g.cities.get_mut(&old)) {
            old.owned_tiles.retain(|held| *held != pos);
        }
        g.map.tiles.get_mut(&pos).unwrap().owner_city = Some(cid);
        let city = g.cities.get_mut(&cid).unwrap();
        if !city.owned_tiles.contains(&pos) {
            city.owned_tiles.push(pos);
        }
    }
    g.map.tiles.get_mut(&site).unwrap().feature = Some(crate::name!("uluru"));
    site
}

fn hold(g: &mut Game, individual: &str) {
    g.players[0]
        .live_great_person_activation_needs
        .push(LiveGreatPersonActivationNeed {
            kind: "scientist".into(),
            individual: Some(individual.into()),
            ..Default::default()
        });
}

fn seat(enabled: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    if enabled {
        ai.enable_foreign_site_buys_the_open_border();
    }
    ai
}

fn buy(g: &mut Game, enabled: bool) -> bool {
    seat(enabled).foreign_site_buys_the_open_border(g, 0, None, &BTreeSet::new(), &BTreeSet::new())
}

fn grants(g: &Game) -> Vec<usize> {
    (1..4)
        .filter(|rival| g.has_open_borders(0, *rival))
        .collect()
}

#[test]
fn foreign_site_buys_the_open_border_is_opt_in() {
    opt_in_off_in_both_controllers(TAG, |ai| ai.foreign_site_buys_the_open_border);
}

#[test]
fn a_stranded_mead_buys_one_grant_from_the_nearest_campus_holder() {
    let mut g = board();
    campus(&mut g, 0);
    let sites = [(1, campus(&mut g, 1)), (2, campus(&mut g, 2))];
    hold(&mut g, "margaret_mead");
    let (_, nearest) = sites
        .iter()
        .map(|(rival, pos)| (g.wdist(home(&g), *pos), *rival))
        .min()
        .unwrap();
    let gold = g.players[0].gold;

    assert!(buy(&mut g, true));

    assert_eq!(grants(&g), [nearest], "one grant, from the nearest");
    assert!(g.players[0].gold < gold, "paid for in Gold");
    assert!(!buy(&mut g, true), "a held grant is not bought twice");
}

#[test]
fn the_strategic_trade_turn_buys_it_in_any_lane() {
    let mut g = board();
    campus(&mut g, 1);
    hold(&mut g, "margaret_mead");

    seat(true).strategic_bilateral_trade(&mut g, 0, None, GrandStrategy::Science);
    assert_eq!(grants(&g), [1]);

    let mut off = board();
    campus(&mut off, 1);
    hold(&mut off, "margaret_mead");
    seat(false).strategic_bilateral_trade(&mut off, 0, None, GrandStrategy::Science);
    assert!(grants(&off).is_empty(), "off, nothing changes");
}

#[test]
fn a_grant_already_held_from_a_campus_holder_buys_nothing() {
    let mut g = board();
    campus(&mut g, 1);
    campus(&mut g, 2);
    hold(&mut g, "margaret_mead");
    let turn = g.turn;
    g.players[2].open_borders_until.insert(0, turn + 10);

    assert!(!buy(&mut g, true));
    assert_eq!(grants(&g), [2]);
}

#[test]
fn a_campus_city_state_we_lead_buys_nothing() {
    let mut g = board();
    campus(&mut g, 1);
    let minor = city_state(&g);
    campus(&mut g, minor);
    hold(&mut g, "margaret_mead");
    let mut follower = g.clone();

    // Meeting a city-state already files an entry for it; replace it.
    g.players[0].envoys.retain(|(cs, _)| *cs != minor);
    g.players[0].envoys.push((minor, 3));
    assert_eq!(g.suzerain_of(minor), Some(0));
    assert!(!buy(&mut g, true));
    assert!(grants(&g).is_empty());

    // Not its Suzerain, the same city-state's Campus is no site.
    assert!(buy(&mut follower, true));
    assert_eq!(grants(&follower), [1]);
}

#[test]
fn a_used_mead_or_another_person_buys_nothing() {
    let mut g = board();
    campus(&mut g, 1);
    assert!(!buy(&mut g, true), "no person held");

    hold(&mut g, "isaac_newton");
    assert!(!buy(&mut g, true), "a person our own Campus serves");
    assert!(grants(&g).is_empty());
}

#[test]
fn off_a_stranded_mead_buys_nothing() {
    let mut g = board();
    campus(&mut g, 1);
    hold(&mut g, "margaret_mead");
    let gold = g.players[0].gold;

    assert!(!buy(&mut g, false));
    assert!(grants(&g).is_empty());
    assert_eq!(g.players[0].gold, gold);
}

#[test]
fn a_falling_treasury_keeps_its_reserve() {
    let mut g = board();
    campus(&mut g, 1);
    hold(&mut g, "margaret_mead");
    g.players[0].gold = 30.0;
    g.players[0].gold_per_turn = -4.0;

    assert!(!buy(&mut g, true));
    assert_eq!(g.players[0].gold, 30.0);
}

#[test]
fn a_stranded_darwin_buys_passage_to_the_wonder_holder() {
    let mut g = board();
    wonder_inside(&mut g, 2);
    hold(&mut g, "charles_darwin");

    assert!(buy(&mut g, true));
    assert_eq!(grants(&g), [2]);
}

#[test]
fn darwin_beside_open_or_own_ground_buys_nothing() {
    let mut g = board();
    wonder_inside(&mut g, 2);
    hold(&mut g, "charles_darwin");
    let open = g
        .map
        .tiles
        .iter()
        .find(|(pos, tile)| {
            tile.owner_city.is_none()
                && g.nbrs(**pos)
                    .into_iter()
                    .all(|near| g.map.get(near).is_some_and(|t| t.owner_city.is_none()))
        })
        .map(|(pos, _)| *pos)
        .expect("unowned ground");
    let mut unowned = g.clone();
    unowned.map.tiles.get_mut(&open).unwrap().feature = Some(crate::name!("uluru"));
    assert!(!buy(&mut unowned, true), "an unowned wonder is walked to");
    assert!(grants(&unowned).is_empty());

    let mut ours = g.clone();
    wonder_inside(&mut ours, 0);
    assert!(!buy(&mut ours, true), "a wonder in our own land");
    assert!(grants(&ours).is_empty());

    // Unowned but never explored: the live mirror's fog, not open ground.
    let mut fogged = unowned.clone();
    fogged.players[0].explored.remove(&open);
    for near in fogged.nbrs(open) {
        fogged.players[0].explored.remove(&near);
    }
    assert!(buy(&mut fogged, true));
    assert_eq!(grants(&fogged), [2]);

    // Unowned but sealed: a seat whose city we have not seen holds it.
    let mut sealed = unowned;
    let mut closed = vec![open];
    closed.extend(sealed.nbrs(open));
    sealed.closed_borders.extend(closed);
    assert!(buy(&mut sealed, true));
    assert_eq!(grants(&sealed), [2]);
}

/// `passage-waits-past-the-opening` withholds passage from a rival over 1.5
/// times our military: the nine live declarations it answers came from
/// Unfriendly rivals that had just granted it. A declared friend cannot
/// declare war while the friendship stands, so live Deity t132-t150
/// (civvis-t7-20261127T075243Z) left Mead idle beside two friends at 1.9-5.3
/// times our military. The gate still holds a stronger rival that is not one.
#[test]
fn a_stronger_friend_sells_past_the_passage_gate() {
    let mut g = board();
    // Past the gate's opening, which a Standard-speed board holds to t160.
    g.turn = 200;
    campus(&mut g, 1);
    hold(&mut g, "margaret_mead");
    let ours = g.military_power(0).max(1.0);
    std::sync::Arc::make_mut(&mut g.observed_military_power).insert(1, ours * 3.0);
    let mut ai = seat(true);
    ai.enable_passage_waits_past_the_opening();
    assert!(
        matches!(
            ai.passage_hold(&g, 0, 1),
            Some(PassageHold::Stronger { .. })
        ),
        "the gate holds rival 1 for its military"
    );

    let mut stranger = g.clone();
    assert!(!ai.foreign_site_buys_the_open_border(
        &mut stranger,
        0,
        None,
        &BTreeSet::new(),
        &BTreeSet::new()
    ));
    assert!(grants(&stranger).is_empty());

    let until = g.turn + 20;
    g.players[0].friends_until.insert(1, until);
    g.players[1].friends_until.insert(0, until);
    assert!(ai.foreign_site_buys_the_open_border(
        &mut g,
        0,
        None,
        &BTreeSet::new(),
        &BTreeSet::new()
    ));
    assert_eq!(grants(&g), [1]);
}

/// Live 081553Z: Darwin stood idle t102-t151 with no natural wonder on the
/// exported map at all, and the gene had nothing to buy. It still has
/// nothing to buy; it now says so.
#[test]
fn darwin_with_no_known_wonder_buys_nothing() {
    let mut g = board();
    hold(&mut g, "charles_darwin");
    assert!(!buy(&mut g, true));
    assert!(grants(&g).is_empty());
}

/// The alliance desks wait on the turn a stranded person's passage closes:
/// the live bridge holds any purchase sent beside an alliance ask.
#[test]
fn the_purchase_turn_is_known_to_the_alliance_desks() {
    let mut g = board();
    campus(&mut g, 1);
    hold(&mut g, "margaret_mead");
    let ai = seat(true);
    assert!(!ai.foreign_site_passage_bought(&g, 0), "nothing bought yet");
    assert!(!seat(false).foreign_site_passage_bought(&g, 0));

    assert!(buy(&mut g, true));
    assert!(ai.foreign_site_passage_bought(&g, 0));
    assert!(!seat(false).foreign_site_passage_bought(&g, 0), "off");

    g.turn += 1;
    assert!(
        !ai.foreign_site_passage_bought(&g, 0),
        "the next turn asks again"
    );
}
