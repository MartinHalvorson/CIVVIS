use super::*;

/// Two majors: our founded capital at population 4 with a pillaged Campus
/// beside it and a Warrior at its head, every unit removed.
fn pillaged_capital() -> (Game, u32, Pos) {
    let mut game = Game::new_full(2, 32, 20, 936_221, 200, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .expect("fixture: a settler");
    game.apply(0, &Action::FoundCity { unit: settler })
        .expect("fixture: the capital founds");
    for pid in [0, 1] {
        for uid in game.player_unit_ids(pid) {
            game.remove_unit(uid);
        }
    }
    let capital = game.player_city_ids(0)[0];
    let home = game.cities[&capital].pos;
    let site =
        game.wdisk(home, 1)
            .into_iter()
            .filter(|pos| *pos != home)
            .find(|pos| {
                game.map.tiles.get(pos).is_some_and(|tile| {
                    !game.rules.is_water(tile) && tile.owner_city == Some(capital)
                })
            })
            .expect("fixture: an owned land tile beside the capital");
    let city = game.cities.get_mut(&capital).unwrap();
    city.pop = 4;
    city.queue.clear();
    city.queue.push(Item::Unit {
        unit: crate::name!("warrior"),
    });
    city.districts.insert(crate::name!("campus"), site);
    let tile = game.map.tiles.get_mut(&site).unwrap();
    tile.district = Some(crate::name!("campus"));
    tile.pillaged = true;
    game.turn = 40;
    (game, capital, site)
}

fn seat(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    if gene {
        ai.enable_pillaged_districts_repair_first();
    }
    ai
}

fn is_district_repair(item: Option<&Item>, at: Pos) -> bool {
    matches!(item, Some(Item::Repair { repair, pos }) if *repair == "district" && *pos == at)
}

/// The pillaged Campus is repaired ahead of the Warrior, which keeps its
/// place behind it; off, the queue is untouched.
#[test]
fn a_pillaged_district_is_repaired_first() {
    let (game, capital, site) = pillaged_capital();
    assert!(
        game.producible_items(0, capital)
            .iter()
            .any(|item| is_district_repair(Some(item), site)),
        "fixture: the engine offers the repair"
    );

    let mut on = game.clone();
    seat(true).claim_pillaged_repairs(&mut on, 0);
    assert!(
        is_district_repair(on.cities[&capital].queue.first(), site),
        "the head: {:?}",
        on.cities[&capital].queue.first()
    );

    let mut off = game.clone();
    seat(false).claim_pillaged_repairs(&mut off, 0);
    assert_eq!(off.cities[&capital].queue, game.cities[&capital].queue);
}

/// A barbarian soldier beside the site holds the repair, and a head that is
/// already a repair is left alone.
#[test]
fn a_raider_beside_the_site_holds_the_repair() {
    let (game, capital, site) = pillaged_capital();
    let ai = seat(true);

    let barbarian = game
        .players
        .iter()
        .position(|player| player.is_barbarian)
        .expect("fixture: a barbarian player");
    let mut raided = game.clone();
    let beside = raided
        .wdisk(site, 1)
        .into_iter()
        .find(|pos| {
            *pos != site
                && *pos != raided.cities[&capital].pos
                && raided
                    .map
                    .tiles
                    .get(pos)
                    .is_some_and(|tile| !raided.rules.is_water(tile))
        })
        .expect("fixture: land beside the site");
    raided.spawn_unit("warrior", barbarian, beside);
    assert!(ai.pillaged_repair_item(&raided, 0, capital).is_none());

    let mut repairing = game.clone();
    ai.claim_pillaged_repairs(&mut repairing, 0);
    let head = repairing.cities[&capital].queue.first().cloned();
    assert!(ai.pillaged_repair_item(&repairing, 0, capital).is_none());
    ai.claim_pillaged_repairs(&mut repairing, 0);
    assert_eq!(repairing.cities[&capital].queue.first().cloned(), head);
}

/// `stuck-repair-yields-the-queue`: the host refuses the repair every turn
/// (the head is back to the Warrior after each claim, as on the live Qin board),
/// so after a run of asks the repair stands aside for ten turns and the claim
/// leaves the queue to the city; with the gene off it asks every turn.
#[test]
fn a_repair_the_host_never_starts_gives_the_queue_back() {
    for stuck_gene in [true, false] {
        let (mut game, capital, site) = pillaged_capital();
        let mut ai = seat(true);
        if stuck_gene {
            ai.enable_stuck_repair_yields_the_queue();
        }
        let warrior = Item::Unit {
            unit: crate::name!("warrior"),
        };
        let mut asked = Vec::new();
        for _ in 0..30 {
            ai.claim_pillaged_repairs(&mut game, 0);
            let took = is_district_repair(game.cities[&capital].queue.first(), site);
            asked.push(took);
            // The host refused: the board shows the old head again.
            let city = game.cities.get_mut(&capital).unwrap();
            city.queue.clear();
            city.queue.push(warrior.clone());
            game.turn += 1;
        }
        if stuck_gene {
            let first_gap = asked
                .iter()
                .position(|took| !took)
                .expect("the repair yields");
            assert!(
                first_gap >= STUCK_REPAIR_TURNS as usize - 1,
                "asked {first_gap} turns"
            );
            let gap = asked[first_gap..].iter().take_while(|took| !**took).count();
            assert!(
                gap >= STUCK_REPAIR_YIELD_TURNS as usize,
                "stands aside {gap} turns: {asked:?}"
            );
            assert!(
                asked[first_gap + gap..].iter().any(|took| *took),
                "then asks again"
            );
        } else {
            assert!(
                asked.iter().all(|took| *took),
                "off, the claim asks every turn"
            );
        }
    }
}

/// `stuck-repair-yields-the-queue`: a repair the host is producing reads back
/// as the pillaged district at its plot (live Kangaba, `DISTRICT_CAMPUS` over
/// its pillaged Campus). That head is a repair under way, so the claim keeps
/// asking as it does with the gene off and nothing is withheld; the same holds
/// for a pillaged building read back as the building.
#[test]
fn a_repair_the_host_is_producing_is_never_set_aside() {
    let (mut game, capital, site) = pillaged_capital();
    let mut ai = seat(true);
    ai.enable_stuck_repair_yields_the_queue();
    let running = Item::District {
        district: crate::name!("campus"),
        pos: site,
    };
    let repair = Item::Repair {
        repair: crate::name!("district"),
        pos: site,
    };
    for turn in 0..20 {
        // The board as the mirror rebuilds it: the host's head, the repair
        // under way, read as the Campus at its plot.
        let city = game.cities.get_mut(&capital).unwrap();
        city.queue.clear();
        city.queue.push(running.clone());
        assert!(
            !ai.stuck_repair_withheld(&game, capital, &repair),
            "withheld on turn {turn}"
        );
        ai.claim_pillaged_repairs(&mut game, 0);
        assert!(
            is_district_repair(game.cities[&capital].queue.first(), site),
            "the claim still asks on turn {turn}"
        );
        game.turn += 1;
    }
    assert!(ai.stuck_repairs.borrow().is_empty(), "no run was counted");

    let library = Item::Repair {
        repair: crate::name!("library"),
        pos: site,
    };
    game.cities.get_mut(&capital).unwrap().queue = vec![Item::Building {
        building: crate::name!("library"),
    }];
    assert!(AdvancedAi::host_is_repairing(&game, capital, &library));
    assert!(!AdvancedAi::host_is_repairing(&game, capital, &repair));
}
