use super::*;
use crate::ai::GrandStrategy;
use crate::game::HostBandPromotions;
use std::collections::BTreeSet;
use std::sync::Arc;

fn seat(target: VictoryTarget, offered: &[&str]) -> (Game, AdvancedAi, u32) {
    let mut g = Game::new_full(2, 60, 24, 936_312, 250, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
    }
    g.found_city_for(0, (6, 8), None);
    g.players[0].religion = Some("Buddhism".into());
    g.current = 0;
    g.turn = 80;
    let uid = g.spawn_unit("apostle", 0, (7, 8));
    g.units.get_mut(&uid).unwrap().religion = Some("Buddhism".into());
    let menu = HostBandPromotions {
        held: BTreeSet::new(),
        offered: offered.iter().map(|name| Name::new(name)).collect(),
    };
    Arc::make_mut(&mut g.host_band_promotions).insert(uid, menu);
    (g, AdvancedAi::targeting(target), uid)
}

fn promote(g: &mut Game, ai: &AdvancedAi) {
    ai.advanced_promotions_at(g, 0, GrandStrategy::Religion, None);
}

fn has(g: &Game, uid: u32, promotion: &str) -> bool {
    g.units[&uid]
        .promotions
        .iter()
        .any(|name| *name == Name::new(promotion))
}

#[test]
fn the_gene_takes_debater_over_the_quiet_homeland_pick() {
    let menu = ["debater", "translator", "indulgence_vendor"];
    let (mut off_game, off, uid) = seat(VictoryTarget::Religion, &menu);
    promote(&mut off_game, &off);
    assert!(
        has(&off_game, uid, "translator"),
        "fixture: off, a quiet homeland takes Translator"
    );
    let (mut on_game, mut on, uid) = seat(VictoryTarget::Religion, &menu);
    on.enable_religion_seat_takes_the_debater();
    promote(&mut on_game, &on);
    assert!(has(&on_game, uid, "debater"));
}

#[test]
fn another_victory_target_keeps_its_own_pick() {
    let menu = ["debater", "translator", "indulgence_vendor"];
    let (mut game, mut ai, uid) = seat(VictoryTarget::Science, &menu);
    ai.enable_religion_seat_takes_the_debater();
    promote(&mut game, &ai);
    assert!(!has(&game, uid, "debater"));
}

#[test]
fn a_menu_without_debater_keeps_its_own_pick() {
    let menu = ["orator", "pilgrim", "martyr"];
    let (mut off_game, off, off_uid) = seat(VictoryTarget::Religion, &menu);
    promote(&mut off_game, &off);
    let (mut on_game, mut on, on_uid) = seat(VictoryTarget::Religion, &menu);
    on.enable_religion_seat_takes_the_debater();
    promote(&mut on_game, &on);
    assert!(!on_game.units[&on_uid].promotions.is_empty());
    assert_eq!(
        off_game.units[&off_uid].promotions,
        on_game.units[&on_uid].promotions
    );
}
