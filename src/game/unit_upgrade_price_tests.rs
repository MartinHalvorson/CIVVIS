use super::*;

/// ★★★ THE UPGRADE BILL IS PINNED TO THE HOST'S OWN `GlobalParameters`.
///
/// Every constant below was read out of the shipped
/// `Cache/DebugGameplay.sqlite`, table `GlobalParameters`, and then confirmed
/// against `Rules::Units::Instance::GetUpgradeCost` inside the shipped
/// `GameCore_XP2_FinalRelease.dll`, which reads exactly these five rows and
/// nothing else:
///
/// ```text
/// UPGRADE_BASE_COST                    10
/// UPGRADE_MINIMUM_COST                 15
/// UPGRADE_NET_PRODUCTION_PERCENT_COST 100
/// GOLD_EQUIVALENT_OTHER_YIELDS          2
/// PURCHASE_DIVISOR                      5
/// ```
///
/// ⚠ The per-Production coefficient reads as "doubled" and is not. The host
/// takes ALL of the net Production difference (`..._PERCENT_COST` is 100) and
/// then converts Production into Gold at `GOLD_EQUIVALENT_OTHER_YIELDS`, which
/// is 2. A coefficient of one would halve every upgrade in the game, so these
/// tests assert against that reading explicitly rather than only for 110 Gold.
const UPGRADE_BASE_COST: f64 = 10.0;
const UPGRADE_MINIMUM_COST: f64 = 15.0;
const UPGRADE_NET_PRODUCTION_PERCENT_COST: f64 = 100.0;
const GOLD_EQUIVALENT_OTHER_YIELDS: f64 = 2.0;
const PURCHASE_DIVISOR: f64 = 5.0;

/// The host's formula, written out from the parameter names alone.
fn shipped_upgrade_gold(from: f64, to: f64) -> f64 {
    UPGRADE_BASE_COST
        + (to - from).max(0.0)
            * (UPGRADE_NET_PRODUCTION_PERCENT_COST / 100.0)
            * GOLD_EQUIVALENT_OTHER_YIELDS
}

/// A settled seat that can quote Warrior -> Swordsman. Egypt has no melee
/// replacement, so the generic upgrade edge is the one under test.
fn settled_seat(seed: u64) -> Game {
    let mut game = Game::new_full(2, 26, 16, seed, 300, 0, false);
    for player in 0..2 {
        let settler = game
            .player_unit_ids(player)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .expect("a starting settler");
        game.found_city_for(player, game.units[&settler].pos, None);
    }
    game.players[0].civ = "Egypt".to_string();
    game.players[0].techs.insert(crate::name!("iron_working"));
    game
}

#[test]
fn the_upgrade_bill_is_the_hosts_base_cost_plus_two_gold_per_production() {
    let game = settled_seat(7_401);

    // The repo's own table must still be the host's table, or the rest of
    // this test is pinning CIVVIS to CIVVIS.
    let warrior = game.rules.units["warrior"].cost;
    let swordsman = game.rules.units["swordsman"].cost;
    assert_eq!(
        (warrior, swordsman),
        (40.0, 90.0),
        "`data/units.json` must still carry the shipped `Units.Cost` rows"
    );

    let (target, gold, _) = game
        .unit_upgrade_price(0, "warrior")
        .expect("Iron Working unlocks the Swordsman");
    assert_eq!(target, "swordsman");
    assert_eq!(gold, shipped_upgrade_gold(warrior, swordsman));
    assert_eq!(gold, 110.0, "10 + 2 x (90 - 40)");

    // ⚠ The reading this guards against: taking the percent row as the whole
    // conversion and dropping `GOLD_EQUIVALENT_OTHER_YIELDS`.
    assert_ne!(
        gold,
        UPGRADE_BASE_COST + (swordsman - warrior),
        "a per-Production coefficient of one quotes 60 Gold and halves every \
         upgrade in the game"
    );
}

#[test]
fn a_bill_under_the_minimum_settles_at_the_shipped_floor() {
    let mut game = settled_seat(7_402);
    // No Production difference at all leaves only `UPGRADE_BASE_COST`, which
    // is below `UPGRADE_MINIMUM_COST`. No shipped pair is this cheap, so the
    // floor is unreachable without moving a cost.
    let warrior = game.rules.units["warrior"].cost;
    std::sync::Arc::make_mut(&mut game.rules)
        .units
        .get_mut("swordsman")
        .expect("the Swordsman row")
        .cost = warrior;

    let (_, gold, _) = game
        .unit_upgrade_price(0, "warrior")
        .expect("the edge still exists");
    assert!(shipped_upgrade_gold(warrior, warrior) < UPGRADE_MINIMUM_COST);
    assert_eq!(gold, UPGRADE_MINIMUM_COST);
}

#[test]
fn game_speed_scales_the_bill_and_purchase_divisor_rounds_it_down() {
    let mut game = settled_seat(7_403);

    // The host speed-scales the base cost and differences two already scaled
    // Production costs, so the whole bill moves with the speed.
    game.game_speed = GameSpeed::Online; // 50%
    assert_eq!(game.unit_upgrade_price(0, "warrior").unwrap().1, 55.0);
    game.game_speed = GameSpeed::Marathon; // 300%
    assert_eq!(game.unit_upgrade_price(0, "warrior").unwrap().1, 330.0);

    // 67% of 110 is 73.7, and the host quotes 70: the final step truncates to
    // whole Gold and rounds DOWN to a multiple of `PURCHASE_DIVISOR`.
    game.game_speed = GameSpeed::Quick;
    let gold = game.unit_upgrade_price(0, "warrior").unwrap().1;
    assert_eq!(gold, 70.0);
    assert_eq!(gold % PURCHASE_DIVISOR, 0.0);
    assert!(gold < GameSpeed::Quick.scale(110.0));
}

#[test]
fn the_upgrade_discount_is_taken_before_the_floor_and_the_rounding() {
    let mut game = settled_seat(7_404);
    game.players[0]
        .policies
        .insert(crate::name!("professional_army"));
    assert_eq!(
        game.policy_effect(0, "upgrade_gold_discount_pct"),
        50.0,
        "Professional Army is the shipped 50% upgrade discount"
    );
    assert_eq!(game.unit_upgrade_price(0, "warrior").unwrap().1, 55.0);

    // Half of 55 is 27.5, and the host cannot quote half a Gold piece: the
    // rounding runs after the discount, not before it.
    game.game_speed = GameSpeed::Online;
    assert_eq!(game.unit_upgrade_price(0, "warrior").unwrap().1, 25.0);
}

#[test]
fn a_corps_and_an_army_pay_for_every_body_they_carry() {
    let mut game = settled_seat(7_405);
    let pos = game.cities[&game.player_city_ids(0)[0]].pos;
    let warrior = game
        .place_new_unit("warrior", 0, pos)
        .expect("the capital tile takes a unit");
    game.players[0].gold = 5_000.0;
    game.players[0]
        .strategic_resources
        .insert(crate::name!("iron"), 500.0);

    for (formation, expected) in [(0u8, 110.0), (1, 220.0), (2, 330.0)] {
        game.units.get_mut(&warrior).unwrap().formation = formation;
        let (_, gold, _) = game
            .unit_gold_upgrade_offer(0, warrior)
            .expect("a garrisoned, unspent Warrior can upgrade at home");
        assert_eq!(
            gold, expected,
            "`GetUpgradeCost` multiplies the bill by 2 for a Corps and 3 for \
             an Army before the minimum and the rounding"
        );
    }
}
