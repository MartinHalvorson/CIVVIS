use super::*;

fn campaign() -> (Game, u32) {
    let mut g = Game::new_full(2, 24, 16, 5_150, 200, 0, false);
    for pid in 0..2 {
        let settler = g
            .player_unit_ids(pid)
            .into_iter()
            .find(|id| g.units[id].kind == "settler")
            .unwrap();
        g.found_city_for(pid, g.units[&settler].pos, None);
    }
    let first = g.cities[&g.player_city_ids(1)[0]].pos;
    let second = g
        .wdisk(first, 7)
        .into_iter()
        .find(|pos| {
            g.wdist(*pos, first) >= 5
                && !g.rules.is_water(&g.map.tiles[pos])
                && g.units_at(*pos).is_empty()
                && g.city_at(*pos).is_none()
        })
        .unwrap();
    g.found_city_for(1, second, None);
    g.current = 0;
    g.at_war.insert((0, 1));
    g.at_war.insert((1, 0));
    g.record_contact(0, 1);
    let home = g.cities[&g.player_city_ids(0)[0]].pos;
    let target = g
        .player_city_ids(1)
        .into_iter()
        .max_by_key(|cid| g.wdist(g.cities[cid].pos, home))
        .unwrap();
    (g, target)
}

#[test]
fn either_capture_standdown_version_overrides_a_cached_siege_until_expiry() {
    for version_two in [false, true] {
        let (mut g, target) = campaign();
        let mut ai = AdvancedAi::new();
        ai.disable_capture_go_or_stand_down();
        ai.disable_capture_go_or_stand_down_2();
        ai.enable_siege_commitment();
        if version_two {
            ai.enable_capture_go_or_stand_down_2();
        } else {
            ai.enable_capture_go_or_stand_down();
        }
        ai.plan = Some(StrategicPlan {
            strategy: GrandStrategy::Conquest,
            target_player: Some(1),
            target_city: Some(target),
            threatened_city: None,
            desired_cities: 4,
            assessed_turn: g.turn,
            rush: false,
        });
        assert_eq!(
            ai.assess(&g, 0).target_city,
            Some(target),
            "an active siege stays committed"
        );
        let until = g.turn + 5;
        ai.capture_stood_down.insert(target, until);
        assert!(ai.plan_stale(&g, 0));
        let reassessed = ai.assess(&g, 0);
        assert_eq!(reassessed.target_player, Some(1));
        assert_ne!(
            reassessed.target_city,
            Some(target),
            "version_two={version_two}: re-assessment must honor the stand-down"
        );
        g.turn = until;
        assert!(!ai.capture_stood_down_holds(&g, target));
        assert_eq!(
            ai.assess(&g, 0).target_city,
            Some(target),
            "an expired stand-down no longer excludes the commitment"
        );
    }
}
