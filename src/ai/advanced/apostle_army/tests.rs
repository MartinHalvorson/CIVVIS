use super::*;
use crate::game::install_test_district;

const OURS: &str = "Buddhism";
const THEIRS: &str = "Hinduism";

/// Our founder on flat open grassland with one Holy Site city of its own
/// faith (Shrine built, so the ring heals) and a rival city on another faith.
/// Returns the game, our city, the Holy Site tile and the rival city.
fn field() -> (Game, u32, Pos, u32) {
    let mut g = Game::new_full(2, 60, 24, 936_312, 250, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
    }
    g.found_city_for(0, (6, 8), None);
    let home = g.player_city_ids(0)[0];
    let site = install_test_district(&mut g, home, "holy_site");
    g.cities
        .get_mut(&home)
        .unwrap()
        .buildings
        .push(crate::name::Name::new("shrine"));
    g.players[0].religion = Some(OURS.into());
    g.players[1].religion = Some(THEIRS.into());
    g.cities
        .get_mut(&home)
        .unwrap()
        .pressure
        .insert(OURS.into(), 500.0);
    g.found_city_for(1, (30, 8), None);
    let abroad = g.player_city_ids(1)[0];
    g.cities
        .get_mut(&abroad)
        .unwrap()
        .pressure
        .insert(THEIRS.into(), 500.0);
    g.current = 0;
    g.turn = 80;
    (g, home, site, abroad)
}

fn spawn(g: &mut Game, kind: &str, owner: usize, at: Pos, hp: i32, charges: i32) -> u32 {
    let unit = g.spawn_unit(kind, owner, at);
    let faith = if owner == 0 { OURS } else { THEIRS };
    let body = g.units.get_mut(&unit).unwrap();
    body.religion = Some(faith.into());
    body.hp = hp;
    body.charges = charges;
    unit
}

fn home_alive() -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Religion);
    ai.enable_corps_comes_home_alive();
    ai
}

/// The live Religion seat's spreading genes, without the doctrine: the
/// control that spends the last charge.
fn spender() -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Religion);
    ai.enable_religion_lane_spreads_from_founding();
    ai.enable_apostle_evangelizes_unspent();
    ai
}

fn army() -> AdvancedAi {
    let mut ai = spender();
    ai.enable_last_charge_joins_the_army();
    ai
}

#[test]
fn both_genes_are_off_by_default() {
    let (mut g, _, _, _) = field();
    let wounded = spawn(&mut g, "missionary", 0, (16, 8), 40, 3);
    let last = spawn(&mut g, "apostle", 0, (17, 8), 100, 1);
    let ai = AdvancedAi::targeting(VictoryTarget::Religion);
    assert_eq!(ai.wounded_preacher_goes_home(&mut g, 0, wounded), None);
    assert!(!ai.corps_declines_the_fight(&g, wounded, (10.0, 90.0, 100)));
    assert!(!ai.keeps_its_last_charge(&g, 0, last));
    assert_eq!(ai.army_apostle_step(&mut g, 0, last), None);
}

#[test]
fn a_wounded_missionary_walks_toward_our_holy_site() {
    let (mut g, _, site, _) = field();
    let wounded = spawn(&mut g, "missionary", 0, (16, 8), 50, 3);
    let before = g.wdist(g.units[&wounded].pos, site);
    assert_eq!(
        home_alive().wounded_preacher_goes_home(&mut g, 0, wounded),
        Some(true)
    );
    assert!(g.wdist(g.units[&wounded].pos, site) < before);
    assert_eq!(
        g.units[&wounded].charges, 3,
        "no charge is spent on the way"
    );
}

#[test]
fn a_lightly_hurt_unit_in_the_field_carries_on() {
    let (mut g, _, _, _) = field();
    let hurt = spawn(&mut g, "missionary", 0, (16, 8), 75, 3);
    assert_eq!(
        home_alive().wounded_preacher_goes_home(&mut g, 0, hurt),
        None
    );
}

#[test]
fn a_unit_in_the_ring_finishes_healing_before_it_leaves() {
    let (mut g, _, site, _) = field();
    let healing = spawn(&mut g, "apostle", 0, site, 75, 3);
    assert!(g.unit_heal_rate(healing) > 0, "fixture: the ring heals");
    assert_eq!(
        home_alive().wounded_preacher_goes_home(&mut g, 0, healing),
        Some(false)
    );
    g.units.get_mut(&healing).unwrap().hp = 95;
    assert_eq!(
        home_alive().wounded_preacher_goes_home(&mut g, 0, healing),
        None
    );
}

#[test]
fn an_inquisitor_in_the_ring_keeps_working_above_the_floor() {
    let (mut g, _, site, _) = field();
    let inquisitor = spawn(&mut g, "inquisitor", 0, site, 75, 3);
    let ai = home_alive();
    assert_eq!(ai.wounded_preacher_goes_home(&mut g, 0, inquisitor), None);
    g.units.get_mut(&inquisitor).unwrap().hp = 50;
    assert_eq!(
        ai.wounded_preacher_goes_home(&mut g, 0, inquisitor),
        Some(false)
    );
}

#[test]
fn a_guru_with_a_heal_left_heals_itself_instead() {
    let (mut g, _, _, _) = field();
    let guru = spawn(&mut g, "guru", 0, (16, 8), 40, 2);
    assert_eq!(
        home_alive().wounded_preacher_goes_home(&mut g, 0, guru),
        None
    );
}

#[test]
fn a_fight_is_declined_when_the_blow_back_could_kill() {
    let (mut g, _, _, _) = field();
    let healthy = spawn(&mut g, "apostle", 0, (16, 8), 100, 3);
    let ai = home_alive();
    assert!(!ai.corps_declines_the_fight(&g, healthy, (45.0, 45.0, 100)));
    assert!(
        ai.corps_declines_the_fight(&g, healthy, (45.0, 85.0, 100)),
        "85 at the mean is 106 at the top of the roll"
    );
}

#[test]
fn a_wounded_apostle_takes_only_the_kill() {
    let (mut g, _, _, _) = field();
    let wounded = spawn(&mut g, "apostle", 0, (16, 8), 55, 3);
    let ai = home_alive();
    assert!(ai.corps_declines_the_fight(&g, wounded, (30.0, 20.0, 100)));
    assert!(!ai.corps_declines_the_fight(&g, wounded, (30.0, 20.0, 25)));
}

#[test]
fn two_rivals_in_reach_make_a_tile_unsafe_together() {
    let (mut g, _, _, _) = field();
    let ours = spawn(&mut g, "missionary", 0, (16, 8), 100, 3);
    spawn(&mut g, "apostle", 1, (18, 8), 100, 3);
    let reaching = |ai: &AdvancedAi, g: &Game| {
        let threats = ai.theological_threats(g, 0, ours);
        AdvancedAi::theological_threats_reaching(g, ours, g.units[&ours].pos, &threats)
    };
    let mut pressing = AdvancedAi::targeting(VictoryTarget::Religion);
    pressing.enable_spreader_steps_out_of_the_apostle();
    pressing.enable_religion_seat_presses_the_campaign();
    let mut careful = pressing.clone();
    careful.enable_corps_comes_home_alive();
    assert_eq!(reaching(&careful, &g), 0, "one blow does not kill at 100");
    spawn(&mut g, "apostle", 1, (18, 9), 100, 3);
    assert_eq!(reaching(&pressing, &g), 0, "neither blow is lethal alone");
    assert_eq!(reaching(&careful, &g), 2);
}

#[test]
fn an_apostle_on_its_last_charge_is_kept_and_hunts() {
    let (mut g, _, _, _) = field();
    let hunter = spawn(&mut g, "apostle", 0, (16, 8), 100, 1);
    let prey = spawn(&mut g, "missionary", 1, (19, 8), 100, 3);
    let ai = army();
    assert!(ai.keeps_its_last_charge(&g, 0, hunter));
    let before = g.wdist(g.units[&hunter].pos, g.units[&prey].pos);
    assert_eq!(ai.army_apostle_step(&mut g, 0, hunter), Some(true));
    assert!(g.wdist(g.units[&hunter].pos, g.units[&prey].pos) < before);
    assert_eq!(g.units[&hunter].charges, 1);
}

#[test]
fn a_hunter_beside_its_prey_strikes() {
    let (mut g, _, _, _) = field();
    let hunter = spawn(&mut g, "apostle", 0, (16, 8), 100, 1);
    let at = g.nbrs((16, 8))[0];
    let prey = spawn(&mut g, "missionary", 1, at, 100, 3);
    assert_eq!(army().army_apostle_step(&mut g, 0, hunter), Some(true));
    assert!(g.units.get(&prey).is_none_or(|unit| unit.hp < 100));
    assert!(g.units.contains_key(&hunter));
}

#[test]
fn with_no_prey_the_kept_apostle_never_spends_the_charge() {
    let (mut g, _, _, abroad) = field();
    let beside = g.nbrs(g.cities[&abroad].pos)[0];
    let kept = spawn(&mut g, "apostle", 0, beside, 100, 1);
    let ai = army();
    for _ in 0..4 {
        ai.advanced_religious_step(&mut g, 0, kept, true);
    }
    assert_eq!(g.units[&kept].charges, 1);
    let (mut g, _, _, abroad) = field();
    let beside = g.nbrs(g.cities[&abroad].pos)[0];
    let spent = spawn(&mut g, "apostle", 0, beside, 100, 1);
    let spends = spender();
    for _ in 0..4 {
        if g.units.contains_key(&spent) {
            spends.advanced_religious_step(&mut g, 0, spent, true);
        }
    }
    assert!(
        g.units.get(&spent).is_none_or(|unit| unit.charges == 0),
        "control: off, the charge is spent"
    );
}

#[test]
fn a_debater_hunts_with_prey_and_spreads_without() {
    let (mut g, _, _, _) = field();
    let debater = spawn(&mut g, "apostle", 0, (16, 8), 100, 3);
    g.units
        .get_mut(&debater)
        .unwrap()
        .promotions
        .insert(crate::name!("debater"));
    let ai = army();
    assert_eq!(ai.army_apostle_step(&mut g, 0, debater), None);
    spawn(&mut g, "apostle", 1, (19, 8), 100, 3);
    assert_eq!(ai.army_apostle_step(&mut g, 0, debater), Some(true));
}

#[test]
fn a_plain_apostle_with_charges_is_not_held() {
    let (mut g, _, _, _) = field();
    let plain = spawn(&mut g, "apostle", 0, (16, 8), 100, 3);
    spawn(&mut g, "missionary", 1, (19, 8), 100, 3);
    assert_eq!(army().army_apostle_step(&mut g, 0, plain), None);
}

#[test]
fn the_kept_charge_does_not_count_against_the_caps() {
    let (mut g, _, _, _) = field();
    spawn(&mut g, "apostle", 0, (16, 8), 100, 1);
    spawn(&mut g, "apostle", 0, (17, 8), 100, 3);
    assert_eq!(army().founder_corps_count(&g, 0, "apostle", OURS), 1);
    let off = AdvancedAi::targeting(VictoryTarget::Religion);
    assert_eq!(off.founder_corps_count(&g, 0, "apostle", OURS), 2);
}

#[test]
fn a_city_of_ours_in_need_nearby_releases_the_charge() {
    let (mut g, home, _, _) = field();
    let near = g.nbrs(g.cities[&home].pos)[0];
    let apostle = spawn(&mut g, "apostle", 0, near, 100, 1);
    let ai = army();
    assert!(ai.keeps_its_last_charge(&g, 0, apostle));
    g.cities
        .get_mut(&home)
        .unwrap()
        .pressure
        .insert(THEIRS.into(), 900.0);
    assert!(!ai.keeps_its_last_charge(&g, 0, apostle));
}

#[test]
fn the_army_has_a_ceiling() {
    let (mut g, _, _, _) = field();
    let apostles: Vec<u32> = (0..=ARMY_CEILING as i32)
        .map(|index| spawn(&mut g, "apostle", 0, (12 + index, 12), 100, 1))
        .collect();
    let ai = army();
    assert!(ai.keeps_its_last_charge(&g, 0, apostles[ARMY_CEILING - 1]));
    assert!(!ai.keeps_its_last_charge(&g, 0, apostles[ARMY_CEILING]));
}

#[test]
fn another_lane_keeps_no_charge() {
    let (mut g, _, _, _) = field();
    let apostle = spawn(&mut g, "apostle", 0, (16, 8), 100, 1);
    let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
    ai.enable_last_charge_joins_the_army();
    assert!(!ai.keeps_its_last_charge(&g, 0, apostle));
}

#[test]
fn a_last_charge_missionary_scouts_instead_of_spreading() {
    let (mut g, _, _, abroad) = field();
    let beside = g.nbrs(g.cities[&abroad].pos)[0];
    let scout = spawn(&mut g, "missionary", 0, beside, 100, 1);
    let ai = army();
    assert!(ai.keeps_its_last_charge(&g, 0, scout));
    for _ in 0..4 {
        ai.advanced_missionary_step(&mut g, 0, scout, true);
    }
    assert_eq!(g.units[&scout].charges, 1);
    let (mut g, _, _, abroad) = field();
    let beside = g.nbrs(g.cities[&abroad].pos)[0];
    let spent = spawn(&mut g, "missionary", 0, beside, 100, 1);
    let spends = spender();
    for _ in 0..4 {
        if g.units.contains_key(&spent) {
            spends.advanced_missionary_step(&mut g, 0, spent, true);
        }
    }
    assert!(
        g.units.get(&spent).is_none_or(|unit| unit.charges == 0),
        "control: off, the charge is spent"
    );
}
