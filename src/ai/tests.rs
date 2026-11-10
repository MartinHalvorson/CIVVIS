use super::*;
use crate::ai::advanced::{AdvancedAi, PEACETIME_UPGRADE_FLOOR};
use crate::parallel::WorkPool;

/// ⚠⚠ THE SHIPPED PANTHEON CHOICE IS A CONSTANT, AND THIS IS THE PROOF.
///
/// A pantheon is exclusive and the shipped order is fixed, so on the
/// deployment profile's six seats the same six pantheons are handed out in
/// the same order every game, on every map. The first half of this test
/// asserts that: the same board, salted with six Deer, still produces the
/// same choice, because nothing in the chooser reads a tile.
///
/// The second half is the treatment. `advanced_pantheon_board` prices each
/// candidate on the land the empire holds, so a board of Deer and Furs buys
/// Goddess of the Hunt — which pays two yields on every one of them —
/// instead of the first name on a list.
#[test]
fn the_pantheon_is_a_constant_until_it_is_priced_against_the_land() {
    let camp_board = |seed: u64| {
        let mut game = Game::new_full(2, 30, 18, seed, 200, 0, false);
        game.current = 0;
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        game.players[0].faith = 200.0;
        game
    };
    // ⚠ The bar a board read has to clear is the ROSTER, not a constant:
    // one place on the order is worth `PANTHEON_PRIOR_STEP`, so the prior
    // spans every name. This test asked for six Deer against an
    // eleven-name roster; the class was completed to twenty-three on
    // 2026-08-24 and six Deer (two yields each, twelve) no longer clear
    // twenty-three. It now asks for however many Deer the roster takes,
    // so completing a class does not silently retune the assertion —
    // and a city opens owning only its first ring, so the borders have to
    // grow before there is anywhere to put them.
    let salt_with_camps = |game: &mut Game| {
        let city = game.player_city_ids(0)[0];
        let centre = game.cities[&city].pos;
        for position in game.wdisk(centre, 2) {
            let owned = game.map.tiles[&position].owner_city;
            if owned.is_none() {
                game.map.tiles.get_mut(&position).unwrap().owner_city = Some(city);
                game.cities
                    .get_mut(&city)
                    .unwrap()
                    .owned_tiles
                    .push(position);
            }
        }
        // Goddess of the Hunt pays a Deer two yields — `camp_food` and
        // `camp_production` — so this is the spread divided by two, plus
        // one to be over rather than level with it.
        let per_deer = 2.0;
        let count = (game.rules.beliefs.pantheon.len() as f64 * PANTHEON_PRIOR_STEP / per_deer)
            .ceil() as usize
            + 1;
        let tiles: Vec<_> = game.cities[&city]
            .owned_tiles
            .iter()
            .copied()
            .filter(|position| *position != centre)
            .take(count)
            .collect();
        assert_eq!(
            tiles.len(),
            count,
            "the capital must own enough tiles to outweigh the order"
        );
        for position in tiles {
            let tile = game.map.tiles.get_mut(&position).unwrap();
            tile.resource = Some(crate::name!("deer"));
            tile.pillaged = false;
        }
    };

    // Shipped: the land is irrelevant. Bare board and Deer board agree.
    let mut shipped = BasicAi::new();
    shipped.pursue_religion = true;
    let mut bare = camp_board(6_101);
    shipped.research_with_government(&mut bare, 0, false, None);
    let mut salted = camp_board(6_101);
    salt_with_camps(&mut salted);
    shipped.research_with_government(&mut salted, 0, false, None);
    assert_eq!(bare.players[0].pantheon, salted.players[0].pantheon);
    assert_eq!(bare.players[0].pantheon.as_deref(), Some("divine_spark"));

    // Treated: the same Deer board buys the pantheon that pays on it.
    let mut treated_ai = BasicAi::new();
    treated_ai.pursue_religion = true;
    treated_ai.pantheon_reads_the_board = true;
    let mut treated = camp_board(6_101);
    salt_with_camps(&mut treated);
    treated_ai.research_with_government(&mut treated, 0, false, None);
    assert_eq!(
        treated.players[0].pantheon.as_deref(),
        Some("goddess_of_the_hunt"),
        "enough Deer to pay more than the whole spread of the order"
    );

    // And on a board with nothing to work it still chooses what it always
    // chose, so the treatment adds a read rather than replacing a policy.
    let mut treated_bare = camp_board(6_101);
    treated_ai.research_with_government(&mut treated_bare, 0, false, None);
    assert_eq!(
        treated_bare.players[0].pantheon.as_deref(),
        Some("divine_spark")
    );
}

/// ★★★★ THE LIVE SEAT TAKES THE PANTHEON THAT FOUNDS A CITY. See
/// `expansion_pantheon`: Religious Settlements grants a Settler in the
/// capital and Fertility Rites a Builder, and the shipped prefix — Divine
/// Spark first — took Divine Spark in 40 of 40 recorded live runs. With
/// the flag on the founding pantheons lead; a Religious Settlements a
/// rival already holds (`Game::blocked_pantheons`, the mirror's read of a
/// `taken_BELIEF_*` refusal) falls to Fertility Rites; the shipped order
/// is untouched with the flag off.
#[test]
fn the_expansion_pantheon_founds_a_city_and_falls_to_the_next_when_taken() {
    let board = |seed: u64| {
        let mut game = Game::new_full(2, 30, 18, seed, 200, 0, false);
        game.current = 0;
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        game.players[0].faith = 200.0;
        game
    };
    let settlers = |game: &Game| {
        game.player_unit_ids(0)
            .into_iter()
            .filter(|unit| game.units[unit].kind == "settler")
            .count()
    };

    // Off: the shipped first choice, and no unit granted for it.
    let mut shipped = BasicAi::new();
    shipped.pursue_religion = true;
    let mut bare = board(6_101);
    shipped.research_with_government(&mut bare, 0, false, None);
    assert_eq!(bare.players[0].pantheon.as_deref(), Some("divine_spark"));
    assert_eq!(settlers(&bare), 0);

    // On: the pantheon that founds a city, and the Settler it grants.
    let mut live = BasicAi::new();
    live.pursue_religion = true;
    live.enable_expansion_pantheon();
    let mut treated = board(6_101);
    live.research_with_government(&mut treated, 0, false, None);
    assert_eq!(
        treated.players[0].pantheon.as_deref(),
        Some("religious_settlements")
    );
    assert_eq!(
        settlers(&treated),
        1,
        "the host grants a Settler in the capital"
    );

    // On, first choice held by a rival: the next founding pantheon, not
    // the shipped list's first name.
    let mut contested = board(6_101);
    std::sync::Arc::make_mut(&mut contested.blocked_pantheons)
        .insert(crate::name!("religious_settlements"));
    live.research_with_government(&mut contested, 0, false, None);
    assert_eq!(
        contested.players[0].pantheon.as_deref(),
        Some("fertility_rites")
    );
    assert_eq!(settlers(&contested), 0);
    assert_eq!(
        contested
            .player_unit_ids(0)
            .into_iter()
            .filter(|unit| contested.units[unit].kind == "builder")
            .count(),
        1,
        "Fertility Rites grants a Builder"
    );
}

/// ★★★★ THE BOOK'S SETTLER SLOT IS HELD FOR THE HOST'S FLOOR, NOT BURNED.
/// See `opening_settler_waits`: with a Settler at the head of the book and
/// the capital still at population 1, the shipped book fails the
/// `g.apply`, advances past the slot, and plays the next opener — the
/// Settler is gone until `pick_item` finds its own reason. With the flag
/// on the next opener plays now, and the turn the capital reaches
/// population 2 the Settler takes the queue ahead of it, the filler's
/// progress banked (`City::production_progress`) exactly as the host
/// keeps it. Ordinary and frozen controllers keep the burning slot.
#[test]
fn the_opening_books_settler_waits_for_the_hosts_population_floor() {
    let board = |seed: u64| {
        let mut game = Game::new_full(2, 30, 18, seed, 200, 0, false);
        game.current = 0;
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        game
    };
    let capital = |game: &Game| game.player_city_ids(0)[0];
    // Settler first, then Builder, so the slot in question is the very
    // first decision and its filler is unambiguous.
    let book_first_settler = |ai: &mut BasicAi| {
        ai.w.open0 = 3.0; // OPENING_MENU[3] is Settler.
        ai.w.open1 = 2.0; // OPENING_MENU[2] is Builder.
        ai.w.open2 = 1.0;
        ai.w.open3 = 5.0;
    };
    let head = |game: &Game| game.cities[&capital(game)].queue.first().cloned();
    let settler = Item::Unit {
        unit: crate::name!("settler"),
    };
    let builder = Item::Unit {
        unit: crate::name!("builder"),
    };

    // Shipped: the slot burns. The Builder plays at population 1, and the
    // capital growing to 2 changes nothing.
    let mut shipped = BasicAi::new();
    book_first_settler(&mut shipped);
    let mut bare = board(6_101);
    assert_eq!(bare.cities[&capital(&bare)].pop, 1);
    assert!(
        shipped.has_practical_settle_site(&bare, 0),
        "the board must offer a site"
    );
    shipped.cities(&mut bare, 0);
    assert_eq!(
        head(&bare),
        Some(builder.clone()),
        "the burned slot's filler"
    );
    assert!(!shipped.book_settler_pending());
    bare.cities.get_mut(&capital(&bare)).unwrap().pop = 2;
    shipped.cities(&mut bare, 0);
    assert_eq!(
        head(&bare),
        Some(builder.clone()),
        "shipped: nothing revisits the slot"
    );

    // Live: the slot is held; the Builder plays now, and the Settler takes
    // the queue the turn the capital reaches the floor.
    let mut live = BasicAi::new();
    book_first_settler(&mut live);
    live.enable_opening_settler_waits();
    let mut treated = board(6_101);
    live.cities(&mut treated, 0);
    assert_eq!(
        head(&treated),
        Some(builder.clone()),
        "the next opener plays now"
    );
    assert!(
        live.book_settler_pending(),
        "and the Settler slot is held, not burned"
    );
    // Not yet: still population 1.
    live.cities(&mut treated, 0);
    assert_eq!(head(&treated), Some(builder.clone()));
    assert!(live.book_settler_pending());
    // A turn of Builder progress, then the city grows.
    {
        let city = treated.cities.get_mut(&capital(&treated)).unwrap();
        city.production = 7.0;
        city.pop = 2;
    }
    live.cities(&mut treated, 0);
    assert_eq!(
        head(&treated),
        Some(settler.clone()),
        "the held Settler takes the queue"
    );
    assert!(!live.book_settler_pending(), "played once, then released");
    let city = &treated.cities[&capital(&treated)];
    assert_eq!(
        city.production_progress.get("unit:builder").copied(),
        Some(7.0),
        "the filler's progress is banked, as the host banks it"
    );
    // And nothing plays it twice.
    live.cities(&mut treated, 0);
    assert_eq!(head(&treated), Some(settler));
    assert!(!live.book_settler_pending());

    // Frozen and ordinary controllers never carry the flag.
    assert!(!BasicAi::new().opening_settler_waits);
    assert!(!AdvancedAi::new().opening_settler_waits);
    assert!(!AdvancedAi::legacy().opening_settler_waits);
}

/// ⚠⚠ AN EIGHT-PLAYER GAME LEFT TWO EMPIRES WITH NO PANTHEON AT ALL.
///
/// The chooser tried a hand-written list of exactly the six pantheons
/// `beliefs.json` carried, in order, and stopped at the first that took. A
/// pantheon is exclusive, so once six empires had founded one the seventh
/// and eighth ran the loop, found every name taken, and founded nothing —
/// while holding the faith to pay for it, for the rest of the game. The
/// `audit` and `soak` defaults are eight players.
///
/// The list is now a preference *prefix* over a roster read from the rules,
/// which is what the follower and founder choosers twenty lines below it
/// have always done. `AGENTS.md`: discover, never list.
#[test]
fn every_major_can_found_a_pantheon_when_there_are_more_majors_than_favourites() {
    let mut game = Game::new_full(8, 40, 26, 5_811, 200, 0, false);
    let majors: Vec<usize> = (0..8).collect();
    for pid in &majors {
        game.current = *pid;
        let settler = game
            .player_unit_ids(*pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(*pid, &Action::FoundCity { unit: settler })
            .unwrap();
        game.players[*pid].faith = 200.0;
    }
    game.current = 0;
    let mut ai = BasicAi::new();
    ai.pursue_religion = true;
    for pid in &majors {
        game.current = *pid;
        ai.research_with_government(&mut game, *pid, false, None);
    }
    let founded: Vec<Option<String>> = majors
        .iter()
        .map(|pid| game.players[*pid].pantheon.clone())
        .collect();
    assert!(
        founded.iter().all(Option::is_some),
        "an empire with the faith and no pantheon: {founded:?}"
    );
    let distinct: std::collections::BTreeSet<&String> = founded.iter().flatten().collect();
    assert_eq!(
        distinct.len(),
        majors.len(),
        "a pantheon was taken twice: {founded:?}"
    );
    // Non-vacuous: more majors than the six the old list could name, so at
    // least two of these come from the discovered part of the roster.
    assert!(majors.len() > 6);
    assert!(game.rules.beliefs.pantheon.len() >= majors.len());
}

#[test]
fn live_policy_counterfactuals_replay_serially_on_workers() {
    let mut initial = Game::new_full(1, 20, 14, 91_481, 120, 0, false);
    initial.players[0].civics.extend(
        [
            "code_of_laws",
            "craftsmanship",
            "foreign_trade",
            "early_empire",
            "state_workforce",
            "military_tradition",
            "political_philosophy",
        ]
        .into_iter()
        .map(Name::new),
    );
    initial.players[0].government = Some("classical_republic".to_string());
    initial.turn = POLICY_REVIEW_EVERY;
    assert!(
        initial.available_policies(0).len() >= 4,
        "the fixture needs enough independent cards to use every worker"
    );
    let weights = Weights {
        policy_deck: PolicyDeck::Live,
        ..Weights::default()
    };

    let mut serial = initial.clone();
    revise_policy_deck(&mut serial, 0, &weights, None, false, false, false);
    for threads in [4, 5] {
        let mut parallel = initial.clone();
        let pool = WorkPool::new(threads);
        revise_policy_deck(&mut parallel, 0, &weights, Some(&pool), false, false, false);

        assert_eq!(serial.log, parallel.log, "threads={threads}");
        assert_eq!(
            serde_json::to_value(&serial).unwrap(),
            serde_json::to_value(&parallel).unwrap(),
            "worker completion order must not change the authoritative game (threads={threads})"
        );
    }
}

/// The whole claim the sweep skip rests on, checked card by card against
/// the sweep itself on a played-forward board.
///
/// `PolicyReadSet::card_is_inert` says a card cannot move
/// [`empire_reading`]. This asserts the implication over **every card in
/// the ruleset**, on both sides of the counterfactual the review runs
/// (removal for a card the seat holds, slotting for one it does not) and
/// under both `net_maintenance` arms, and it compares raw bits rather than
/// an epsilon: the skip reuses a number, so anything short of bit equality
/// is a defect.
///
/// The board is a real one — four majors and six city-states playing
/// seventy turns of the Basic agent — because a hand-built fixture only
/// exercises the yield paths its author thought of, and the read set is
/// exactly the set of paths the board happens to open.
#[test]
fn a_reading_inert_policy_card_moves_the_empire_reading_by_exactly_zero() {
    let mut game = Game::new_full(4, 44, 30, 7_320_017, 300, 6, true);
    game.set_fog_memory(false);
    let mut ais: Vec<AdvancedAi> = game.players.iter().map(|_| AdvancedAi::new()).collect();
    while game.winner.is_none() && game.turn <= 110 {
        let pid = game.current;
        ais[pid].take_turn(&mut game, pid);
        if game.winner.is_none() && game.current == pid {
            let _ = game.apply(pid, &Action::EndTurn);
        }
    }
    let cards: Vec<Name> = game.rules.policies.keys().copied().collect();
    let weights = Weights::default();
    let mut ever_live: std::collections::BTreeSet<&str> = Default::default();
    let mut ever_inert: std::collections::BTreeSet<&str> = Default::default();
    let mut checked = 0usize;
    // Every major, because the read set is a property of the board: a seat
    // whose capital is queueing a Settler asks `item_prod_mult` for
    // `settler_production_pct` and a seat queueing a Granary does not, so
    // one seat exercises only the paths its own position opens.
    for pid in 0..4 {
        let cities = game.player_city_ids(pid);
        let districts: usize = cities
            .iter()
            .map(|cid| game.cities[cid].districts.len())
            .sum();
        let buildings: usize = cities
            .iter()
            .map(|cid| game.cities[cid].buildings.len())
            .sum();
        let routes = game
            .routes
            .iter()
            .filter(|route| route.owner == pid)
            .count();
        println!(
            "fixture seat {pid}: {} cities, {districts} districts, {buildings} buildings, \
                 {routes} trade routes, {} units, religion {:?}, pantheon {:?}, {} cards slotted",
            cities.len(),
            game.player_unit_ids(pid).len(),
            game.players[pid].religion,
            game.players[pid].pantheon,
            game.players[pid].policies.len(),
        );
        assert!(!cities.is_empty(), "a seat with no cities reads nothing");
        for net_maintenance in [false, true] {
            let (base, reads) =
                reading_with_policy_trace(&game, pid, &weights, net_maintenance, false);
            assert!(
                !reads.is_opaque(),
                "an ordinary seat carries no runtime attachment"
            );
            let mut inert: Vec<&str> = Vec::new();
            let mut live: Vec<&str> = Vec::new();
            for card in cards.iter().copied() {
                let verdict = reads.card_is_inert(&game.rules, card);
                let held = game.players[pid].policies.contains(&card);
                if held {
                    game.players[pid].policies.remove(&card);
                } else {
                    game.players[pid].policies.insert(card);
                }
                let other = empire_reading(&game, pid, &weights, net_maintenance, false);
                if held {
                    game.players[pid].policies.insert(card);
                } else {
                    game.players[pid].policies.remove(&card);
                }
                checked += 1;
                if verdict {
                    assert_eq!(
                        other.to_bits(),
                        base.to_bits(),
                        "{} is classified reading-inert but the sweep moved: {other} against \
                             {base} (seat={pid}, held={held}, net_maintenance={net_maintenance})",
                        card.as_str()
                    );
                    inert.push(card.as_str());
                    ever_inert.insert(card.as_str());
                } else {
                    live.push(card.as_str());
                    ever_live.insert(card.as_str());
                }
            }
            println!(
                "  seat {pid} net_maintenance={net_maintenance}: {} of {} cards \
                     reading-inert, so the review skips that many whole-empire sweeps",
                inert.len(),
                cards.len()
            );
            if pid == 0 && !net_maintenance {
                println!("  inert on seat 0: {}", inert.join(" "));
            }
            // Non-vacuous in both directions on every seat: the classifier
            // has to rule cards out as well as in, or it measures nothing.
            assert!(
                inert.len() >= 20,
                "expected the Great-Person, envoy and espionage families to be inert on \
                     seat {pid}, got {}",
                inert.len()
            );
            assert!(!live.is_empty(), "the yield cards must not be skipped");
            // Two named controls, one each way. A Great-Person card moves
            // points `empire_reading` has no term for; an influence card
            // moves the one non-yield rate it does read.
            assert!(
                inert.contains(&"inspiration"),
                "a card whose whole effect is Great Scientist points is inert"
            );
            assert!(
                live.contains(&"charismatic_leader"),
                "influence_per_turn is a term of the reading, so its cards are not inert"
            );
        }
    }
    println!(
        "{checked} (seat, card, maintenance-arm) verdicts checked against the sweep; \
             {} cards were live somewhere and {} inert somewhere",
        ever_live.len(),
        ever_inert.len()
    );
    // The verdict is a property of the board, not of the card: some cards
    // are inert on one seat and live on another, which is the whole reason
    // this is traced per review rather than tabulated once.
    assert!(
        ever_live.intersection(&ever_inert).next().is_some(),
        "a classifier that answered the same on every board would not need the trace"
    );
}

#[test]
fn the_barbarian_responder_matches_a_galley_to_an_offshore_raider() {
    let mut g = Game::new_full(2, 24, 16, 91_484, 60, 0, true);
    for unit in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(unit);
    }
    g.barb_camps.clear();
    let barb = g.barb_pid.expect("a barbarian seat");
    g.map.clear_rivers();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
    }
    // A shoreline three tiles long beside the capital, with the raider on
    // it and our own hull one tile along.
    let home = *g.map.tiles.keys().min().expect("the map has tiles");
    g.found_city_for(0, home, None);
    let mut water: Vec<Pos> = g
        .wdisk(home, 3)
        .into_iter()
        .filter(|pos| *pos != home && g.wdist(home, *pos) >= 2)
        .take(3)
        .collect();
    water.sort();
    for pos in &water {
        g.map.tiles.get_mut(pos).unwrap().terrain = crate::name!("coast");
    }
    // Two tiles out: adjacent land would be claimed as the garrison and
    // never reach the responder pool at all.
    let land_spot = g
        .wdisk(home, 2)
        .into_iter()
        .filter(|pos| g.wdist(home, *pos) == 2)
        .find(|pos| {
            g.map.get(*pos).is_some_and(|t| !g.rules.is_water(t)) && g.unit_ids_at(*pos).is_empty()
        })
        .expect("open land two tiles from the capital");
    let our_galley = g.spawn_test_unit("galley", 0, water[0]);
    // A warrior already holds the capital, so the garrison mechanism is
    // satisfied and the spearman genuinely belongs to the field pool.
    g.spawn_test_unit("warrior", 0, home);
    let spearman = g.spawn_test_unit("spearman", 0, land_spot);
    let raider = g.spawn_test_unit("galley", barb, water[2]);
    let raider_pos = g.units[&raider].pos;
    let enemies = [barb];

    let ai = BasicAi::new();
    assert_eq!(
        ai.barbarian_response_objective(&g, 0, our_galley, &enemies),
        Some(raider_pos)
    );
    assert!(ai
        .barbarian_response_objective(&g, 0, spearman, &enemies)
        .is_none());
}

#[test]
fn the_maintenance_discount_scores_only_when_the_deck_sees_the_bill() {
    let mut game = Game::new_full(1, 20, 14, 91_483, 120, 0, false);
    game.players[0]
        .civics
        .insert(crate::name!("state_workforce"));
    game.players[0].government = Some("chiefdom".to_string());
    // An army whose bill the discount would cut: three Spearmen at one
    // Gold of maintenance each (a Warrior costs nothing to keep).
    let spots: Vec<Pos> = game
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| {
            game.rules.is_passable(tile)
                && !game.rules.is_water(tile)
                && game.unit_ids_at(**pos).is_empty()
        })
        .map(|(pos, _)| *pos)
        .take(3)
        .collect();
    for pos in spots {
        game.spawn_test_unit("spearman", 0, pos);
    }
    assert!(game.unit_gold_maintenance(0) > 0.0);
    let card = crate::name!("conscription");
    let spec = &game.rules.policies[&card];
    let candidate = (0usize, spec.slot.clone(), card);
    let weights = Weights {
        policy_deck: PolicyDeck::Live,
        ..Weights::default()
    };

    // Blind (production today): the discount is an empire-level payment
    // no city yield carries, so the counterfactual reads 0.0 exactly.
    // It is also the classifier's own boundary: with the bill unread the
    // sweep never asks for `unit_maintenance_discount`, so the card is
    // reading-inert and the skip returns the same exact 0.0.
    let (current, reads) = reading_with_policy_trace(&game, 0, &weights, false, false);
    assert!(reads.card_is_inert(&game.rules, card));
    let (blind, ..) = policy_card_score(
        &mut game, 0, &weights, &candidate, current, &reads, false, false,
    );
    assert_eq!(blind, 0.0, "the defect under test: the card is invisible");

    // Aware: the with-side bill is lower by one Gold per unit, so the
    // gain is the discount at the gold weight — strictly positive. The
    // same card is now live to the classifier, because the same sweep now
    // asks the deck for the discount.
    let (current, reads) = reading_with_policy_trace(&game, 0, &weights, true, false);
    assert!(!reads.card_is_inert(&game.rules, card));
    let (aware, ..) = policy_card_score(
        &mut game, 0, &weights, &candidate, current, &reads, true, false,
    );
    assert!(
        aware > 0.0,
        "the maintenance discount must score its own relief, got {aware}"
    );
}

/// `merchant-confederation-earns-its-slot`: Merchant Confederation's Gold
/// per Envoy is paid to the player, not to a city, so the shipped
/// reading never asks for it, the card is reading-inert and scores
/// exactly 0.0 at any number of Envoys. Reading the player's policy
/// income prices it at the gold weight per Envoy.
#[test]
fn merchant_confederation_scores_its_envoys_only_when_player_income_reads() {
    let mut game = Game::new_full(1, 20, 14, 91_483, 120, 0, false);
    game.players[0].envoys = vec![(5, 7), (6, 5)];
    let card = crate::name!("merchant_confederation");
    let spec = &game.rules.policies[&card];
    let candidate = (0usize, spec.slot.clone(), card);
    let weights = Weights {
        policy_deck: PolicyDeck::Live,
        ..Weights::default()
    };

    let (current, reads) = reading_with_policy_trace(&game, 0, &weights, false, false);
    assert!(reads.card_is_inert(&game.rules, card));
    let (blind, ..) = policy_card_score(
        &mut game, 0, &weights, &candidate, current, &reads, false, false,
    );
    assert_eq!(
        blind, 0.0,
        "the defect under test: twelve Envoys' Gold is invisible"
    );

    let (current, reads) = reading_with_policy_trace(&game, 0, &weights, false, true);
    assert!(!reads.card_is_inert(&game.rules, card));
    let (priced, ..) = policy_card_score(
        &mut game, 0, &weights, &candidate, current, &reads, false, true,
    );
    assert!(
        (priced - 12.0 * weights.pol_gold).abs() < 1e-9,
        "twelve Envoys at one Gold each, at the gold weight: {priced}"
    );

    // With no Envoy placed the card is worth nothing either way.
    game.players[0].envoys.clear();
    let (current, reads) = reading_with_policy_trace(&game, 0, &weights, false, true);
    let (empty, ..) = policy_card_score(
        &mut game, 0, &weights, &candidate, current, &reads, false, true,
    );
    assert_eq!(empty, 0.0);
}

#[test]
fn incremental_policy_score_matches_both_counterfactual_sides() {
    let mut game = Game::new_full(1, 20, 14, 91_482, 120, 0, false);
    game.players[0].civics.extend(
        [
            "code_of_laws",
            "craftsmanship",
            "foreign_trade",
            "early_empire",
            "state_workforce",
            "military_tradition",
            "political_philosophy",
        ]
        .into_iter()
        .map(Name::new),
    );
    game.players[0].government = Some("classical_republic".to_string());
    let card = game
        .available_policies(0)
        .into_iter()
        .next()
        .expect("the fixture has an available policy");
    let spec = &game.rules.policies[&card];
    let candidate = (POLICY_PRIORITY.len(), spec.slot.clone(), card);
    let weights = Weights {
        policy_deck: PolicyDeck::Live,
        ..Weights::default()
    };

    // Challenger: the current slate is the exact `without` side, so the
    // incremental scorer must agree with the old two-sweep reading.
    let (current, reads) = reading_with_policy_trace(&game, 0, &weights, false, false);
    let mut full = game.clone();
    let without = empire_reading(&full, 0, &weights, false, false);
    full.players[0].policies.insert(card);
    let with = empire_reading(&full, 0, &weights, false, false);
    let expected_gain = with - without;
    let expected = (expected_gain, candidate.0, candidate.1.clone(), candidate.2);
    let actual = policy_card_score(
        &mut game, 0, &weights, &candidate, current, &reads, false, false,
    );
    assert_eq!(actual, expected);

    // Incumbent: after putting the card on the slate, the current reading
    // is the exact `with` side and only removal needs a fresh sweep.
    game.players[0].policies.insert(card);
    let (current, reads) = reading_with_policy_trace(&game, 0, &weights, false, false);
    let mut full = game.clone();
    full.players[0].policies.remove(&card);
    let without = empire_reading(&full, 0, &weights, false, false);
    let gain = current - without;
    let expected = (
        gain + weights.pol_swap_margin * gain.abs(),
        candidate.0,
        candidate.1.clone(),
        candidate.2,
    );
    let actual = policy_card_score(
        &mut game, 0, &weights, &candidate, current, &reads, false, false,
    );
    assert_eq!(actual, expected);
    assert!(game.players[0].policies.contains(&card));
}

/// A quiet one-player world with a lone Scout, so nothing else on the map
/// can move the unit or attack it while its whereabouts are recorded.
fn scouted_world() -> (Game, Vec<Pos>, u32) {
    let mut g = Game::new_full(1, 20, 12, 5, 300, 0, true);
    // Nothing else on the map, so the only thing the record can be
    // measuring is the Scout's own itinerary.
    g.units.clear();
    let mut ground: Vec<Pos> = g
        .map
        .tiles
        .iter()
        .filter(|(_, tile)| !g.rules.is_water(tile) && g.rules.is_passable(tile))
        .map(|(pos, _)| *pos)
        .collect();
    ground.sort();
    assert!(ground.len() > 8, "the case needs somewhere to walk");
    let scout = g.spawn_test_unit("scout", 0, ground[0]);
    (g, ground, scout)
}

/// Walk one unit through a scripted sequence of tiles, letting the agent
/// take down where the unit stood at the start of each turn exactly as a
/// real movement phase would.
fn observe_walk(tiles: &[usize], spend_charge_on: Option<usize>) -> (BasicAi, Game, Vec<Pos>, u32) {
    let (mut g, ground, scout) = scouted_world();
    let mut ai = BasicAi::new();
    for (turn, tile) in tiles.iter().enumerate() {
        g.units.get_mut(&scout).unwrap().pos = ground[*tile];
        if spend_charge_on == Some(turn) {
            g.units.get_mut(&scout).unwrap().charges += 1;
        }
        ai.begin_movement_turn(&g, 0);
        g.turn += 1;
    }
    (ai, g, ground, scout)
}

#[test]
fn live_motion_turn_accounting_counts_replan_frames_once() {
    let (mut game, ground, scout) = scouted_world();
    game.turn = 42;

    // A normal controller retains the historical per-decision accounting.
    // Four tiles keep the unit outside the three-tile loop bound, so the
    // assertion observes the counter rather than a later stand-down reset.
    let mut native = BasicAi::new();
    for frame in 0..20 {
        game.units.get_mut(&scout).unwrap().pos = ground[frame % 4];
        native.begin_movement_turn(&game, 0);
    }
    assert_eq!(native.unit_motion[&scout].fruitless, 20);

    let mut live = BasicAi::new();
    live.enable_live_motion_turn_accounting();
    for frame in 0..20 {
        game.units.get_mut(&scout).unwrap().pos = ground[frame % 4];
        live.begin_movement_turn(&game, 0);
    }
    let first_turn = &live.unit_motion[&scout];
    assert_eq!(
        first_turn.fruitless, 1,
        "twenty replan frames are one host turn"
    );
    assert_eq!(first_turn.tiles.len(), 1);
    assert_eq!(first_turn.observed_turn, Some(game.turn));

    game.turn += 1;
    game.units.get_mut(&scout).unwrap().pos = ground[1];
    live.begin_movement_turn(&game, 0);
    let next_turn = &live.unit_motion[&scout];
    assert_eq!(
        next_turn.fruitless, 2,
        "the next host turn still advances motion"
    );
    assert_eq!(next_turn.tiles.len(), 2);
    assert_eq!(next_turn.observed_turn, Some(game.turn));
}

#[test]
fn a_live_replan_remaps_the_same_turn_path_guard() {
    let (mut game, _, scout) = scouted_world();
    game.turn = 42;
    let home = game.units[&scout].pos;
    let next = game
        .nbrs(home)
        .into_iter()
        .find(|pos| game.can_move(scout, *pos))
        .expect("the lone Scout has an open neighboring tile");
    let mut ai = BasicAi::new();

    assert!(ai.path_move(&mut game, 0, scout, next));
    game.remove_unit(scout);
    let remapped = game.spawn_test_unit("scout", 0, next);
    ai.remap_unit_memory(&std::collections::BTreeMap::from([(scout, remapped)]));

    assert!(
        !ai.path_move(&mut game, 0, remapped, home),
        "a same-turn fresh-board replan must not undo the step it just observed"
    );
    assert_eq!(game.units[&remapped].pos, next);

    game.turn += 1;
    assert!(
        ai.path_move(&mut game, 0, remapped, home),
        "the remapped trail expires with its host turn"
    );
}

#[test]
fn a_live_motion_stand_down_is_not_restarted_by_its_replan_frame() {
    let (mut game, ground, scout) = scouted_world();
    let mut live = BasicAi::new();
    live.enable_live_motion_turn_accounting();

    for elapsed in 0..LIVELOCK_STAND_DOWN_AFTER {
        game.turn = 100 + elapsed;
        game.units.get_mut(&scout).unwrap().pos = ground[elapsed as usize % 2];
        live.begin_movement_turn(&game, 0);
    }
    let stood_down = &live.unit_motion[&scout];
    let resume_turn = stood_down.resume_turn;
    assert!(
        resume_turn > game.turn,
        "the two-tile loop stood down after real turns"
    );
    assert_eq!(
        stood_down.fruitless, 0,
        "the retry starts with a clean record"
    );
    assert_eq!(stood_down.observed_turn, Some(game.turn));

    // `observe_unit_motion` clears the record on a stand-down. Preserve
    // its same-turn marker too, or this replan would immediately start a
    // new artificial fruitless turn.
    live.begin_movement_turn(&game, 0);
    let replanned = &live.unit_motion[&scout];
    assert_eq!(replanned.resume_turn, resume_turn);
    assert_eq!(replanned.fruitless, 0);
    assert!(replanned.tiles.is_empty());
    assert_eq!(replanned.observed_turn, Some(game.turn));
}

#[test]
fn unit_memory_keeps_a_campaign_and_warning_across_turns_and_id_remap() {
    let mut game = Game::new_full(2, 24, 16, 91_482, 80, 0, false);
    let enemy_settler = game
        .player_unit_ids(1)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .expect("fixture starts player one with a Settler");
    let city_pos = game.units[&enemy_settler].pos;
    let city = game.found_city_for(1, city_pos, None);
    game.remove_unit(enemy_settler);
    let land: Vec<Pos> = game
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| {
            game.rules.is_passable(tile)
                && !game.rules.is_water(tile)
                && game.city_at(**pos).is_none()
        })
        .map(|(pos, _)| *pos)
        .collect();
    let unit = game.spawn_test_unit("warrior", 0, land[0]);
    let replacement = game.spawn_test_unit("warrior", 0, land[1]);
    let mut ai = BasicAi::new();
    ai.unit_objective_memory = true;

    ai.remember_capture_objective(&game, 0, unit, city);
    assert!(
        !ai.remember_dangerous_approach(&game, unit, land[2], 20.0),
        "a merely risky tile is retained without forcing a full retreat"
    );
    let first = ai.unit_memory(unit).expect("the unit owns a memory");
    assert!(matches!(
        first.objective,
        Some(UnitObjective::CaptureCity {
            city: remembered,
            target,
            started_turn,
            last_confirmed_turn,
        }) if remembered == city
            && target == city_pos
            && started_turn == game.turn
            && last_confirmed_turn == game.turn
    ));
    assert_eq!(first.danger.map(|danger| danger.position), Some(land[2]));

    game.turn += 1;
    ai.remember_capture_objective(&game, 0, unit, city);
    let refreshed = ai
        .unit_memory(unit)
        .expect("memory persists into the next turn");
    assert!(matches!(
        refreshed.objective,
        Some(UnitObjective::CaptureCity {
            started_turn,
            last_confirmed_turn,
            ..
        }) if started_turn + 1 == last_confirmed_turn
    ));

    let planned = ai.unit_plan_state(unit);
    let mut committed = BasicAi::new();
    committed.unit_objective_memory = true;
    committed.merge_unit_plan_state(replacement, planned);
    assert_eq!(
        committed.unit_memory(replacement),
        Some(refreshed.clone()),
        "a parallel unit plan returns only its own durable memory to the serial board"
    );

    ai.remap_unit_memory(&std::collections::BTreeMap::from([(unit, replacement)]));
    assert!(ai.unit_memory(unit).is_none(), "the old mirror ID is gone");
    let remapped = ai
        .unit_memory(replacement)
        .expect("the new mirror ID inherits the same campaign");
    assert_eq!(remapped.objective, refreshed.objective);
    assert_eq!(remapped.danger, refreshed.danger);

    game.cities.get_mut(&city).unwrap().owner = 0;
    ai.begin_movement_turn(&game, 0);
    let after_capture = ai
        .unit_memory(replacement)
        .expect("the warning remains until its short expiry");
    assert!(
        after_capture.objective.is_none(),
        "capturing the city resolves the job"
    );
}

#[test]
fn a_dangerous_approach_causes_a_short_retreat_without_losing_the_memory() {
    let mut game = Game::new_full(2, 24, 16, 91_483, 80, 0, false);
    for unit in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(unit);
    }
    game.at_war.insert((0, 1));
    let positions: Vec<Pos> = game
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| {
            game.rules.is_passable(tile)
                && !game.rules.is_water(tile)
                && game.city_at(**pos).is_none()
        })
        .map(|(pos, _)| *pos)
        .collect();
    let (threat, origin, refuge) = positions
        .iter()
        .copied()
        .find_map(|threat| {
            game.nbrs(threat).into_iter().find_map(|origin| {
                if !positions.contains(&origin) {
                    return None;
                }
                game.nbrs(origin)
                    .into_iter()
                    .find(|refuge| {
                        positions.contains(refuge)
                            && game.wdist(*refuge, threat) > game.wdist(origin, threat)
                    })
                    .map(|refuge| (threat, origin, refuge))
            })
        })
        .expect("fixture needs a land tile with a retreat route");
    let unit = game.spawn_test_unit("warrior", 0, origin);
    let enemy = game.spawn_test_unit("warrior", 1, threat);
    game.units.get_mut(&unit).unwrap().hp = 60;
    let mut ai = BasicAi::new();
    ai.unit_objective_memory = true;

    let expected = ai.projected_counter_damage(&game, unit, threat, &[enemy]);
    assert!(
        expected > 0.0,
        "the route must have a real hostile counterattack"
    );
    assert!(
        ai.remember_dangerous_approach(&game, unit, threat, expected),
        "damage that crosses the withdrawal floor becomes a retreat"
    );
    let before = game.units[&unit].pos;
    assert_eq!(ai.retreat_step(&mut game, 0, unit), Some(true));
    let after = game.units[&unit].pos;
    assert!(
        game.wdist(after, threat) > game.wdist(before, threat),
        "retreat moves away from the exact dangerous approach"
    );
    assert!(
        game.wdist(after, threat) >= game.wdist(refuge, threat),
        "the unit selects a route at least as far from danger as the known escape"
    );
    let memory = ai.unit_memory(unit).expect("retreat retains the warning");
    assert_eq!(memory.danger.map(|danger| danger.position), Some(threat));
    assert_eq!(memory.retreat_until, Some(game.turn + UNIT_RETREAT_TURNS));

    // Retreat is deliberately shorter than the warning: the unit can resume its
    // campaign once it has backed off, but it must not immediately forget why.
    game.turn += UNIT_RETREAT_TURNS;
    ai.begin_movement_turn(&game, 0);
    let cooling = ai
        .unit_memory(unit)
        .expect("danger survives the retreat window");
    assert!(cooling.danger.is_some());
    assert_eq!(cooling.retreat_until, None);

    game.turn += 1;
    ai.begin_movement_turn(&game, 0);
    assert!(
        ai.unit_memory(unit).is_none(),
        "short-lived danger is removed once its observation expires"
    );
}

#[test]
fn a_lethal_pool_evacuates_to_a_city_the_enemy_cannot_reach() {
    let mut game = Game::new_full(2, 20, 14, 91_484, 80, 0, false);
    for unit in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(unit);
    }
    game.map.clear_rivers();
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.district_foundation = None;
        tile.wonder = None;
        tile.owner_city = None;
        tile.hills = false;
        tile.road = 0;
    }
    game.at_war.insert((0, 1));
    game.current = 0;

    let front = game
        .map
        .tiles
        .keys()
        .copied()
        .find(|position| game.wdisk(*position, 2).len() == 19)
        .expect("fixture needs an interior plains tile");
    let ring = game.nbrs(front);
    let enemies = [ring[0], ring[1], ring[2]];
    let refuge = game
        .wdisk(front, 2)
        .into_iter()
        .find(|position| {
            game.wdist(*position, front) == 2
                && enemies
                    .iter()
                    .all(|enemy| game.wdist(*position, *enemy) >= 3)
        })
        .expect("fixture needs a two-step refuge outside every warrior envelope");
    game.found_city_for(0, refuge, Some("Refuge".to_string()));
    let defender = game.spawn_test_unit("warrior", 0, front);
    game.units.get_mut(&defender).unwrap().hp = 55;
    let enemy_units: Vec<u32> = enemies
        .into_iter()
        .map(|position| game.spawn_test_unit("warrior", 1, position))
        .collect();

    for enemy in enemy_units {
        assert!(
            game.attack_reach(enemy).contains(&front),
            "each nearby warrior can attack the exposed front tile next turn"
        );
        assert!(
            !game.attack_reach(enemy).contains(&refuge),
            "the City Center is outside that warrior's exact attack envelope"
        );
    }
    assert!(
        game.reachable(defender).contains(&refuge),
        "the wounded warrior can reach the City Center this turn"
    );

    let mut ai = BasicAi::new();
    assert!(
        !ai.can_survive_by_attacking(&game, 0, defender, 0),
        "a counterattack cannot make this three-warrior pool survivable"
    );
    assert_eq!(ai.healing_step(&mut game, 0, defender), Some(true));
    assert_eq!(game.units[&defender].pos, refuge);
    assert!(ai.recovering_units.contains(&defender));
    assert_eq!(game.unit_heal_rate(defender), 20);
}

#[test]
fn baseline_envoys_skip_an_unmet_city_state_and_reach_a_known_one() {
    let mut game = Game::new_full(1, 24, 16, 90_732, 120, 2, false);
    let city_states: Vec<usize> = game
        .players
        .iter()
        .filter(|player| player.is_minor && !player.is_barbarian)
        .map(|player| player.id)
        .collect();
    assert_eq!(city_states.len(), 2);
    let hidden = city_states[0];
    let known = city_states[1];
    for player in 0..game.players.len() {
        game.players[player].met.clear();
    }
    game.record_contact(0, known);
    // This fixture isolates the baseline's choice of destination from the
    // first-discovery reward granted by contact itself.
    game.players[0].envoys.clear();
    game.players[0].envoys_free = 1;

    // With equal influence, the baseline's stable tie-break would prefer
    // the lower-id hidden state. It must discard that candidate rather
    // than fail its first send and leave the known court unfunded.
    BasicAi::new().research_without_government_with_pool(&mut game, 0, None);

    assert_eq!(game.players[0].envoys_free, 0);
    assert_eq!(game.envoys_at(0, hidden), 0);
    assert_eq!(game.envoys_at(0, known), 1);
}

#[test]
fn baseline_diplomacy_skips_an_unmet_major_and_reaches_a_known_one() {
    let mut game = Game::new_full(3, 24, 16, 90_734, 120, 0, false);
    for player in 0..game.players.len() {
        game.players[player].met.clear();
    }
    game.record_contact(0, 2);
    game.turn = 0;
    game.current = 0;

    // Player 1 wins the stable id tie but is unknown. A rejected proposal
    // must not prevent the same diplomatic pass from reaching player 2.
    BasicAi::new().diplomacy(&mut game, 0);

    assert_eq!(game.pending_deals.len(), 1);
    assert_eq!(game.pending_deals[0].from, 0);
    assert_eq!(game.pending_deals[0].to, 2);
    assert!(game.pending_deals[0].friendship);
}

/// The single-turn reversal ban cannot see a round trip that takes two
/// turns to complete, and every step of one is individually the best move
/// available. Only the unit's own recent history gives the loop away.
#[test]
fn a_unit_shuttling_between_two_tiles_is_priced_out_of_both() {
    let shuttle: Vec<usize> = (0..LIVELOCK_WINDOW + 2).map(|turn| turn % 2).collect();
    let (ai, _g, ground, scout) = observe_walk(&shuttle, None);

    assert!(ai.livelock_penalty(scout, ground[0]) < 0.0);
    assert!(ai.livelock_penalty(scout, ground[1]) < 0.0);
    assert_eq!(
        ai.livelock_penalty(scout, ground[7]),
        0.0,
        "anywhere it has not already been is worth going"
    );
    assert!(ai.retreads_a_loop(scout, ground[1]));
    assert!(!ai.retreads_a_loop(scout, ground[7]));
}

#[test]
fn a_proven_loop_can_use_the_coordinated_route_escape() {
    let shuttle: Vec<usize> = (0..LIVELOCK_WINDOW + 2).map(|turn| turn % 2).collect();
    let (ai, _g, _ground, scout) = observe_walk(&shuttle, None);

    assert!(
        !ai.recorded_tactical_step,
        "the coordinated production path does not enable live movement recording"
    );
    assert!(ai.unit_motion[&scout].looping);
    assert!(ai.live_livelock_route_escape(scout));
}

/// The host can move a Scout around an obstacle without ever taking the
/// committed route toward new ground. That is a distinct failure from an
/// outright refused move: it must retire the goal when the unit's own
/// history proves the loop, rather than wait forever for the same-tile
/// refusal counter.
#[test]
fn a_live_scout_loop_retires_its_committed_goal() {
    let (mut game, ground, scout) = scouted_world();
    let home = ground[0];
    let other = game
        .nbrs(home)
        .into_iter()
        .find(|pos| {
            game.map
                .get(*pos)
                .is_some_and(|tile| !game.rules.is_water(tile) && game.rules.is_passable(tile))
        })
        .expect("the scout has a second land tile for the loop");
    game.players[0].explored.clear();
    game.turn = 10;

    let mut live = BasicAi::new();
    live.enable_explore_dead_targets();
    live.enable_explore_commit();
    let goal = live
        .exploration_goal(&game, 0, scout, false)
        .expect("the board has unexplored ground");
    assert_eq!(
        live.explore_goal
            .borrow()
            .get(&scout)
            .map(|(goal, _)| *goal),
        Some(goal)
    );

    // The route order changes the Scout's position, but it returns to the
    // same two tiles for a whole livelock window and reveals nothing.
    for turn in 0..LIVELOCK_WINDOW {
        game.turn = 10 + turn as u32;
        game.units.get_mut(&scout).unwrap().pos = if turn.is_multiple_of(2) { home } else { other };
        live.begin_movement_turn(&game, 0);
    }

    let dead = live.explore_dead.borrow();
    let retired = dead.get(&scout).expect("the loop retires its held goal");
    assert_eq!(
        retired.get(&goal),
        Some(&(game.turn + EXPLORE_DEAD_TARGET_TURNS))
    );
    assert!(
        game.nbrs(goal)
            .iter()
            .all(|neighbour| retired.contains_key(neighbour)),
        "the blocked goal's ring is retired with it"
    );
    drop(dead);
    assert!(
        !live.explore_goal.borrow().contains_key(&scout),
        "the next step must choose a new destination rather than keep the looped one"
    );
    let rerouted = live
        .exploration_goal(&game, 0, scout, false)
        .expect("there is another route to discover");
    assert_ne!(rerouted, goal, "the Scout is sent toward fresh ground");

    let plain = BasicAi::new();
    assert!(
        !plain.explore_dead_targets,
        "the retirement remains live-bridge only"
    );
}

#[test]
fn a_unit_that_is_getting_somewhere_is_left_alone() {
    let march: Vec<usize> = (0..LIVELOCK_WINDOW + 2).collect();
    let (ai, _g, ground, scout) = observe_walk(&march, None);
    for tile in &ground[..8] {
        assert_eq!(ai.livelock_penalty(scout, *tile), 0.0);
    }
}

/// A Builder working two tiles beside a city occupies the same footprint
/// as a Builder stuck between them. Spending the charge is the difference,
/// and it is what the work fingerprint exists to see.
#[test]
fn a_unit_that_accomplishes_something_keeps_its_ground() {
    let shuttle: Vec<usize> = (0..LIVELOCK_WINDOW + 2).map(|turn| turn % 2).collect();
    let halfway = Some(LIVELOCK_WINDOW / 2);
    let (working, _g, ground, scout) = observe_walk(&shuttle, halfway);
    assert_eq!(working.livelock_penalty(scout, ground[0]), 0.0);

    let (idle, _g, ground, scout) = observe_walk(&shuttle, None);
    assert!(idle.livelock_penalty(scout, ground[0]) < 0.0);
}

/// A three-tile shuffle is still a loop; a four-tile circuit is a unit
/// covering ground, and pricing that would punish ordinary movement. Both
/// walks stop short of `LIVELOCK_STAND_DOWN_AFTER`, which would wipe the
/// record being examined.
#[test]
fn the_footprint_bound_separates_a_loop_from_a_march() {
    let turns = LIVELOCK_WINDOW + 2;
    let (looping, _g, ground, scout) =
        observe_walk(&(0..turns).map(|turn| turn % 3).collect::<Vec<_>>(), None);
    assert!(looping.livelock_penalty(scout, ground[0]) < 0.0);

    let (marching, _g, ground, scout) =
        observe_walk(&(0..turns).map(|turn| turn % 4).collect::<Vec<_>>(), None);
    assert_eq!(marching.livelock_penalty(scout, ground[0]), 0.0);
}

/// The tabu redirects a unit that has somewhere else to go. A unit with
/// nowhere else to go keeps circling anyway; for that one the record is
/// wiped so the retry re-plans against a world that has moved on, and a
/// unit that then still finds nothing to do digs in rather than standing
/// in the open.
#[test]
fn a_unit_still_looping_after_a_second_window_starts_over_and_digs_in() {
    let shuttle: Vec<usize> = (0..LIVELOCK_STAND_DOWN_AFTER as usize + 1)
        .map(|turn| turn % 2)
        .collect();
    let (ai, mut g, ground, scout) = observe_walk(&shuttle, None);

    assert_eq!(
        ai.livelock_penalty(scout, ground[0]),
        0.0,
        "the stand-down clears the record, so the retry is unencumbered"
    );
    ai.hold_stood_down_unit(&mut g, 0, scout);
    assert!(g.units[&scout].fortified);

    // And it ends on its own: a unit past its stand-down is left alone.
    g.turn += LIVELOCK_STAND_DOWN_TURNS;
    let (later, mut g, _ground, other) = observe_walk(&[0, 0], None);
    later.hold_stood_down_unit(&mut g, 0, other);
    assert!(!g.units[&other].fortified);
}

/// Digging in must never take the place of a turn the unit could have
/// spent. An earlier version ran *before* the unit's own step and guessed
/// at what it might have wanted; measured over six games it cost more
/// productive turns than the loops it broke, which is exactly what the
/// `picket` column in `audit` exists to expose.
#[test]
fn a_stood_down_unit_still_takes_a_turn_it_can_use() {
    let shuttle: Vec<usize> = (0..LIVELOCK_STAND_DOWN_AFTER as usize + 1)
        .map(|turn| turn % 2)
        .collect();
    let (mut ai, mut g, _ground, scout) = observe_walk(&shuttle, None);
    assert!(
        ai.unit_motion[&scout].resume_turn > g.turn,
        "the case needs a unit that is serving out a stand-down"
    );

    let before = g.units[&scout].pos;
    ai.units(&mut g, 0);

    assert_ne!(
        g.units[&scout].pos, before,
        "a Scout with a whole world left to look at goes and looks at it"
    );
    assert!(!g.units[&scout].fortified);
}

/// The observer sees the start of every turn exactly once, in order,
/// and the game it watched is the game `run_game` plays.
#[test]
fn the_observed_loop_visits_every_turn_once_and_plays_the_same_game() {
    let mut g = Game::new_full(2, 20, 14, 90_210, 12, 0, false);
    let first = g.turn;
    let mut ais = AdvancedAi::fleet(&g);
    let mut seen = Vec::new();
    run_game_observed(&mut g, &mut ais, |world| seen.push(world.turn));
    let last = *seen.last().expect("at least one turn was observed");
    assert_eq!(
        seen,
        (first..=last).collect::<Vec<_>>(),
        "one visit per turn, in order, with no turn skipped or repeated"
    );
    assert!(g.winner.is_some() || g.turn > g.max_turns);
    let mut plain = Game::new_full(2, 20, 14, 90_210, 12, 0, false);
    let mut plain_ais = AdvancedAi::fleet(&plain);
    run_game(&mut plain, &mut plain_ais);
    assert_eq!((plain.turn, plain.winner), (g.turn, g.winner));
}

#[test]
fn a_game_no_enabled_victory_can_end_still_stops_at_its_turn_limit() {
    // A lobby is free to pin `--victories` without `score`, which removes
    // the turn-limit tiebreak that is the only thing guaranteeing a winner.
    // `set_winner` then refuses every path, and before the turn bound this
    // loop ran past the limit forever.
    let mut g = Game::new_full(2, 20, 14, 90_210, 12, 0, false);
    g.victory_conditions = crate::game::VictoryConditions {
        science: false,
        culture: false,
        religious: false,
        diplomatic: false,
        domination: false,
        score: false,
    };
    let mut ais = AdvancedAi::fleet(&g);
    run_game(&mut g, &mut ais);
    assert_eq!(g.winner, None, "no enabled path could be awarded");
    assert!(
        g.turn > g.max_turns,
        "the game ran to its limit: turn {} of {}",
        g.turn,
        g.max_turns
    );
}

fn walled_war_game(seed: u64) -> (Game, u32, u32) {
    let mut g = Game::new_full(2, 20, 14, seed, 40, 0, false);
    let settler0 = g
        .player_unit_ids(0)
        .into_iter()
        .find(|id| g.units[id].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler0 }).unwrap();
    g.apply(0, &Action::EndTurn).unwrap();
    let settler1 = g
        .player_unit_ids(1)
        .into_iter()
        .find(|id| g.units[id].kind == "settler")
        .unwrap();
    g.apply(1, &Action::FoundCity { unit: settler1 }).unwrap();
    g.apply(1, &Action::EndTurn).unwrap();
    let home = g.player_city_ids(0)[0];
    let enemy = g.player_city_ids(1)[0];
    g.cities
        .get_mut(&enemy)
        .unwrap()
        .buildings
        .push(crate::name!("walls"));
    g.record_contact(0, 1);
    g.apply(0, &Action::DeclareWar { player: 1 }).unwrap();
    (g, home, enemy)
}

fn island_colony_game(players: usize) -> (Game, Pos, Pos) {
    let mut g = Game::new_full(players, 18, 10, 91, 120, 0, false);
    let founding_settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| g.units[uid].kind == "settler")
        .unwrap();
    let source = g.units[&founding_settler].pos;
    let target = g
        .map
        .tiles
        .keys()
        .copied()
        .max_by_key(|pos| (g.wdist(source, *pos), *pos))
        .expect("map has a tile");
    assert!(g.wdist(source, target) > 6);
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("coast");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
        tile.owner_city = None;
        tile.cliff_edges = [false; 6];
    }
    g.map.tiles.get_mut(&source).unwrap().terrain = crate::name!("plains");
    g.map.tiles.get_mut(&target).unwrap().terrain = crate::name!("grassland");
    g.apply(
        0,
        &Action::FoundCity {
            unit: founding_settler,
        },
    )
    .unwrap();
    (g, source, target)
}

fn grant_tech_with_prerequisites(g: &mut Game, pid: usize, tech: &str) {
    let prerequisites = g.rules.techs[tech].requires.clone();
    for prerequisite in prerequisites {
        grant_tech_with_prerequisites(g, pid, &prerequisite);
    }
    g.players[pid].techs.insert(Name::new(tech));
}

/// The production picker named base districts, so for a civilization with
/// a unique replacement it kept proposing a district the engine refuses.
/// A refused proposal ends the city's turn, so those cities queued nothing
/// at all - permanently, since the same choice came back every turn.
#[test]
fn civilizations_queue_the_unique_district_they_can_actually_build() {
    let g = Game::new_full(8, 40, 24, 3, 60, 0, false);
    let greece = g
        .players
        .iter()
        .position(|player| player.civ == "Greece")
        .unwrap();
    let rome = g
        .players
        .iter()
        .position(|player| player.civ == "Rome")
        .unwrap();
    assert_eq!(
        BasicAi::civ_district(&g, greece, "theater_square"),
        "acropolis"
    );
    assert_eq!(BasicAi::civ_district(&g, rome, "aqueduct"), "bath");
    // Civilizations without a replacement keep the stock district, and a
    // rival's unique district is never proposed.
    assert_eq!(
        BasicAi::civ_district(&g, rome, "theater_square"),
        "theater_square"
    );
    assert_eq!(BasicAi::civ_district(&g, greece, "campus"), "campus");
}

/// Buildings carry replacements too - a secret society swaps the Monument
/// for an Old God Obelisk - and the Monument is the first thing every city
/// considers, so proposing a blocked one would strand it from turn one.
/// Whatever comes back must always be something the engine accepts.
#[test]
fn building_choices_are_always_producible() {
    let mut g = Game::new_full(8, 40, 24, 3, 60, 0, false);
    for pid in 0..8 {
        let settler = g
            .player_unit_ids(pid)
            .into_iter()
            .find(|id| g.units[id].kind == "settler")
            .unwrap();
        while g.current != pid {
            let current = g.current;
            g.apply(current, &Action::EndTurn).unwrap();
        }
        g.apply(pid, &Action::FoundCity { unit: settler }).unwrap();
        let cid = g.player_city_ids(pid)[0];
        for family in ["monument", "amphitheater", "arena"] {
            if let Some(item) = BasicAi::civ_building(&g, pid, cid, family) {
                assert!(
                    g.can_produce(pid, cid, &item),
                    "{} was offered {item:?} for {family} and cannot build it",
                    g.players[pid].civ
                );
            }
        }
        // Rome starts every city with a free Monument, so it must fall
        // through rather than proposing one it already has.
        if g.players[pid].civ == "Rome" {
            assert!(BasicAi::civ_building(&g, pid, cid, "monument").is_none());
        }
    }
}

/// Builders were produced to a flat quota per city whether or not the
/// empire had a tile left to improve, so a built-out empire kept paying
/// ⚠⚠ A BUILDER WOULD NOT WALK TO A PILLAGED TILE.
///
/// Repair was handled for the tile the Builder already stood on and
/// nowhere else: the empire-wide target sweep tested only
/// `valid_improvements`, which a pillaged-but-improved tile fails. So a
/// razed farm was never a destination — a Builder had to wander onto it.
///
/// Meanwhile `has_builder_work`, the gate that decides whether to *train* a
/// Builder, counts exactly that tile. Two definitions of "work" that
/// disagree, with the wider one deciding to spend the production and the
/// narrower one deciding where to go.
///
/// Counted over three 250-turn six-player games before the repair:
/// Builders reached a decision with no target 3,704 times, and on **508**
/// of those `has_builder_work` said there was work.
#[test]
fn a_builder_walks_to_a_pillaged_improvement_across_the_empire() {
    let mut g = Game::new_full(1, 20, 14, 37, 60, 0, false);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|id| g.units[id].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    g.current = 0;
    let cid = g.player_city_ids(0)[0];
    let centre = g.cities[&cid].pos;

    // Improve every owned tile, so the only work left is a repair — and
    // put the pillaged one as far from the Builder as the city reaches.
    let owned: Vec<Pos> = g.cities[&cid]
        .owned_tiles
        .iter()
        .copied()
        .filter(|p| *p != centre)
        .collect();
    for pos in &owned {
        let tile = g.map.tiles.get_mut(pos).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
        tile.improvement = Some(crate::name!("farm"));
        tile.pillaged = false;
    }
    // Two owned tiles as far apart as the capital reaches: the Builder
    // starts on one and the only work in the empire is on the other.
    let (start, razed) = owned
        .iter()
        .flat_map(|a| owned.iter().map(move |b| (*a, *b)))
        .max_by_key(|(a, b)| g.wdist(*a, *b))
        .expect("the capital owns tiles");
    g.map.tiles.get_mut(&razed).unwrap().pillaged = true;

    // The gate can see this work.
    assert!(BasicAi::has_builder_work(&g, 0));

    let builder = g.spawn_test_unit("builder", 0, start);
    let before = g.wdist(g.units[&builder].pos, razed);
    assert!(before > 1, "the fixture must put the repair out of reach");

    let ai = BasicAi::new();
    let acted = ai.builder_step(&mut g, 0, builder);
    assert!(acted, "a Builder with a repair to reach must do something");
    let after = g.wdist(g.units[&builder].pos, razed);
    assert!(
        after < before,
        "the Builder stayed at distance {before} from the only work on the board"
    );
}

/// for Builders that then stood still for the rest of the game.
#[test]
fn builders_are_only_built_when_there_is_ground_to_work() {
    let mut g = Game::new_full(1, 20, 14, 29, 40, 0, false);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|id| g.units[id].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let cid = g.player_city_ids(0)[0];

    // The test is about the Builder-work predicate, not the random
    // resource that happened to land in a generated capital ring.
    let open = g.cities[&cid]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != g.cities[&cid].pos)
        .expect("a founded city owns a workable non-centre tile");
    {
        let tile = g.map.tiles.get_mut(&open).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
        tile.improvement = None;
        tile.pillaged = false;
    }
    assert!(
        BasicAi::has_builder_work(&g, 0),
        "a fresh city has tiles worth improving"
    );

    // Improve everything the city owns; nothing is left for a Builder.
    for pos in g.cities[&cid].owned_tiles.clone() {
        let tile = g.map.tiles.get_mut(&pos).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
        tile.improvement = Some(crate::name!("farm"));
        tile.pillaged = false;
    }
    assert!(!BasicAi::has_builder_work(&g, 0));

    // Pillaging one of them is work again: it can be repaired.
    let pos = g.cities[&cid].owned_tiles[1];
    g.map.tiles.get_mut(&pos).unwrap().pillaged = true;
    assert!(BasicAi::has_builder_work(&g, 0));
}

/// Builders used to take whichever legal improvement sorted first by
/// name, so a Farm was laid over Iron, Stone and Wine - forfeiting the
/// strategic resource or luxury on that tile for the rest of the game.
#[test]
fn builders_improve_the_resource_rather_than_the_alphabet() {
    let mut g = Game::new_full(1, 20, 14, 29, 40, 0, false);
    let pos = *g.map.tiles.keys().next().unwrap();
    {
        let tile = g.map.tiles.get_mut(&pos).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
    }

    for (resource, expected) in [
        ("iron", "mine"),
        ("stone", "quarry"),
        ("wine", "plantation"),
    ] {
        g.map.tiles.get_mut(&pos).unwrap().resource = Some(Name::new(resource));
        let options = vec![
            crate::name!("farm"),
            Name::new(expected),
            crate::name!("camp"),
        ];
        assert_eq!(
            BasicAi::best_improvement(&g, pos, &options).map(|name| name.to_string()),
            Some(expected.to_string()),
            "{resource} should be worked, not farmed over"
        );
    }

    // With nothing on the tile the choice falls back to yield, and a
    // Lumber Mill's two Production beats a Farm's one Food.
    g.map.tiles.get_mut(&pos).unwrap().resource = None;
    let options = vec![crate::name!("farm"), crate::name!("lumber_mill")];
    assert_eq!(
        BasicAi::best_improvement(&g, pos, &options),
        Some(crate::name!("lumber_mill"))
    );
    assert_eq!(BasicAi::best_improvement(&g, pos, &[]), None);
}

/// `veterans-withdraw-early`: a promoted unit leaves the line with hit
/// points in hand where a green unit of the same health fights on.
#[test]
fn a_veteran_withdraws_with_hit_points_in_hand() {
    let (mut game, ours, front, _home) = a_warrior_under_one_archer();
    {
        let unit = game.units.get_mut(&ours).unwrap();
        // Above the 45 withdrawal line, below it plus two promotions.
        unit.hp = 60;
        unit.promotions.insert(crate::name!("battlecry"));
        unit.promotions.insert(crate::name!("tortoise"));
    }
    let mut control = game.clone();
    let mut green = BasicAi::new();
    assert_eq!(green.veteran_hp_margin(&control, ours), 0);
    assert_eq!(
        green.healing_step(&mut control, 0, ours),
        None,
        "60 hp is healthy for a controller that keeps no margin"
    );
    assert_eq!(control.units[&ours].pos, front);
    assert!(!green.recovering_units.contains(&ours));

    let mut ai = BasicAi::new();
    ai.veteran_retreat_margin = true;
    assert_eq!(
        ai.veteran_hp_margin(&game, ours),
        2 * VETERAN_HP_PER_PROMOTION
    );
    assert!(ai.healing_step(&mut game, 0, ours).is_some());
    assert!(
        ai.recovering_units.contains(&ours),
        "the veteran is recovering"
    );

    let unit = game.units.get_mut(&ours).unwrap();
    unit.promotions.insert(crate::name!("ambush"));
    unit.promotions.insert(crate::name!("zweihander"));
    assert_eq!(ai.veteran_hp_margin(&game, ours), VETERAN_HP_MARGIN_CAP);
}

/// `ranged-hp-reserve`: our archer stands where an enemy archer can shoot
/// it next turn. Below the lethal pool both controllers leave; above it a
/// controller with no reserve stays, and the reserve controller keeps
/// leaving for another `RANGED_HP_RESERVE` hit points — the band in which
/// a hit would not kill it but would leave it too hurt to keep shooting.
fn an_archer_under_one_archer() -> (Game, u32, Pos) {
    let (mut game, warrior, front, _home) = a_warrior_under_one_archer();
    game.remove_unit(warrior);
    let ours = game.spawn_test_unit("archer", 0, front);
    (game, ours, front)
}

#[test]
fn a_ranged_unit_keeps_a_reserve_against_the_lethal_pool() {
    let (game, ours, _front) = an_archer_under_one_archer();
    let controller = |reserve: bool| {
        let mut ai = BasicAi::new();
        ai.ranged_hp_reserve = reserve;
        ai
    };
    assert_eq!(controller(false).ranged_hp_reserve(&game, ours), 0);
    assert_eq!(
        controller(true).ranged_hp_reserve(&game, ours),
        RANGED_HP_RESERVE
    );

    // A fresh controller per reading: `retreat_step` remembers danger it has
    // seen, and a memory from a low-hp reading would make a healthy one leave.
    let leaves = |reserve: bool, hp: i32| {
        let mut g = game.clone();
        g.units.get_mut(&ours).unwrap().hp = hp;
        controller(reserve).retreat_step(&mut g, 0, ours).is_some()
    };
    // The pool is fixed by the fixture (~45 at full health, ~55 when hurt);
    // find where each controller stops leaving as hit points rise, rather
    // than restating the damage table.
    let last_leave = |reserve: bool| (1..=100).rev().find(|hp| leaves(reserve, *hp));
    let plain_edge = last_leave(false).expect("a low-hp archer under fire leaves");
    let reserve_edge = last_leave(true).expect("the reserve controller leaves too");
    assert!(
        reserve_edge > plain_edge,
        "reserve leaves from {reserve_edge} hp, plain from {plain_edge}: the archer \
             must leave while it would still be left standing"
    );
    // At full health it holds its ground and takes the duel: the reserve is
    // for breaking off early, not for refusing to fight.
    assert!(
        !leaves(true, 100),
        "a full-health archer must still stand and shoot"
    );
}

#[test]
fn melee_and_siege_read_no_ranged_reserve() {
    let (mut game, ours, front, _home) = a_warrior_under_one_archer();
    let mut ai = BasicAi::new();
    ai.ranged_hp_reserve = true;
    assert_eq!(
        ai.ranged_hp_reserve(&game, ours),
        0,
        "a warrior trades blows"
    );
    game.remove_unit(ours);
    let catapult = game.spawn_test_unit("catapult", 0, front);
    assert!(game.rules.units["catapult"].siege && game.rules.units["catapult"].has_ranged_attack());
    assert_eq!(
        ai.ranged_hp_reserve(&game, catapult),
        0,
        "a siege piece must sit under the walls"
    );
}

/// `modernize-before-spending`: the same Gold goes to the promoted unit
/// first, and the floor holds against the second upgrade.
#[test]
fn modernize_army_upgrades_the_veteran_first_and_keeps_the_floor() {
    let (mut g, source, _) = island_colony_game(1);
    g.players[0].civ = "Egypt".to_string();
    grant_tech_with_prerequisites(&mut g, 0, "iron_working");
    g.players[0]
        .strategic_resources
        .insert(crate::name!("iron"), 400.0);
    for uid in g.player_unit_ids(0) {
        if g.units[&uid].kind == "warrior" {
            g.remove_unit(uid);
        }
    }
    let green = g.spawn_test_unit("warrior", 0, source);
    let veteran = g.spawn_test_unit("warrior", 0, source);
    g.units
        .get_mut(&veteran)
        .unwrap()
        .promotions
        .insert(crate::name!("battlecry"));
    // 110 Gold per upgrade: one clears a 120 floor, a second would not.
    g.players[0].gold = 235.0;

    let mut by_strength = g.clone();
    assert_eq!(BasicAi::modernize_army(&mut by_strength, 0, 120.0, 0.0), 1);
    assert_eq!(
        by_strength.units[&green].kind, "swordsman",
        "equal gain per Gold: the lower id goes first"
    );
    assert_eq!(by_strength.units[&veteran].kind, "warrior");

    let weight = crate::ai::advanced::VETERAN_UPGRADE_WEIGHT;
    assert_eq!(BasicAi::modernize_army(&mut g, 0, 120.0, weight), 1);
    assert_eq!(
        g.units[&veteran].kind, "swordsman",
        "the promotion carries into the successor, so the veteran goes first"
    );
    assert_eq!(g.units[&green].kind, "warrior");
    assert_eq!(g.players[0].gold, 125.0);
}

/// `modernize-before-spending`: a one-era breakthrough modernizes the
/// army before the purchase pass, keeping a fixed purchase buffer. A
/// treasury that cannot clear that floor waits; withheld, nothing moves.
#[test]
fn a_breakthrough_modernizes_the_army_with_a_fixed_peacetime_reserve() {
    let (mut g, source, _) = island_colony_game(1);
    g.players[0].civ = "Egypt".to_string();
    grant_tech_with_prerequisites(&mut g, 0, "iron_working");
    g.players[0]
        .strategic_resources
        .insert(crate::name!("iron"), 400.0);
    for uid in g.player_unit_ids(0) {
        if g.units[&uid].kind == "warrior" {
            g.remove_unit(uid);
        }
    }
    let warrior = g.spawn_test_unit("warrior", 0, source);
    let floor = PEACETIME_UPGRADE_FLOOR;
    assert_eq!(floor, 100.0);
    g.players[0].gold = floor + 110.0;

    let mut withheld = g.clone();
    let mut control = AdvancedAi::new();
    control.disable_modernize_before_spending();
    assert_eq!(
        control.modernize_before_the_purchase_pass(&mut withheld, 0),
        0
    );
    assert_eq!(withheld.units[&warrior].kind, "warrior");

    let mut ai = AdvancedAi::new();
    assert_eq!(ai.modernize_before_the_purchase_pass(&mut g, 0), 1);
    assert_eq!(g.units[&warrior].kind, "swordsman");
    assert_eq!(g.players[0].gold, floor);

    // The next Warrior waits: at peace the purchase pass keeps its floor.
    let straggler = g.spawn_test_unit("warrior", 0, source);
    assert_eq!(ai.modernize_before_the_purchase_pass(&mut g, 0), 0);
    assert_eq!(g.units[&straggler].kind, "warrior");
}

/// Production only ever replaces losses, so without this pass the units
/// standing in a city on turn 30 are still standing there in the
/// Information era. The AI has to spend Gold to modernize them.
#[test]
fn the_ai_spends_gold_to_modernize_the_garrison_it_already_has() {
    let (mut g, source, _) = island_colony_game(1);
    g.players[0].civ = "Egypt".to_string();
    grant_tech_with_prerequisites(&mut g, 0, "iron_working");
    g.players[0]
        .strategic_resources
        .insert(crate::name!("iron"), 400.0);
    g.players[0].gold = 900.0;
    let veterans: Vec<u32> = (0..3)
        .map(|_| g.spawn_test_unit("warrior", 0, source))
        .collect();
    let obsolete = g
        .units
        .values()
        .filter(|unit| unit.owner == 0 && unit.kind == "warrior")
        .count();

    BasicAi::upgrade_units(&mut g, 0);

    assert!(
        veterans.iter().all(|uid| g.units[uid].kind == "swordsman"),
        "kinds={:?}",
        veterans
            .iter()
            .map(|uid| g.units[uid].kind)
            .collect::<Vec<_>>()
    );
    // Every Warrior in the empire modernized, at 110 Gold each.
    assert_eq!(g.players[0].gold, 900.0 - 110.0 * obsolete as f64);
    assert!(!g
        .units
        .values()
        .any(|unit| unit.owner == 0 && unit.kind == "warrior"));

    // A treasury that cannot clear the floor buys nothing at all.
    g.players[0].gold = 150.0;
    let straggler = g.spawn_test_unit("warrior", 0, source);
    BasicAi::upgrade_units(&mut g, 0);
    assert_eq!(g.units[&straggler].kind, "warrior");
}

#[test]
fn a_nearby_advanced_barbarian_redirects_research_to_the_army_upgrade_path() {
    let (mut g, city, raider) = barbarian_at_the_gates_game(91_501);
    g.units.get_mut(&raider).unwrap().kind = crate::name!("crossbowman");
    let center = g.cities[&city].pos;
    for _ in 0..3 {
        g.spawn_test_unit("archer", 0, center);
    }

    assert!(BasicAi::barbarian_military_gap(&g, 0));
    assert_eq!(
        BasicAi::barbarian_military_research_goal(&g, 0).as_deref(),
        Some("machinery")
    );

    g.players[0].research = None;
    BasicAi::new().research_with_government(&mut g, 0, false, None);
    let selected = g.players[0]
        .research
        .as_deref()
        .expect("the barbarian gap selects a research step");
    assert!(
        BasicAi::tech_leads_to(&g, selected, "machinery"),
        "selected {selected} should be on the path to Machinery"
    );
}

#[test]
fn a_barbarian_quality_gap_spends_the_upgrade_budget_before_the_peacetime_reserve() {
    let (mut g, city, raider) = barbarian_at_the_gates_game(91_502);
    g.units.get_mut(&raider).unwrap().kind = crate::name!("crossbowman");
    g.players[0].techs.insert(crate::name!("machinery"));
    let center = g.cities[&city].pos;
    let archers: Vec<u32> = (0..2)
        .map(|_| g.spawn_test_unit("archer", 0, center))
        .collect();
    g.players[0].gold = 10_000.0;
    let cost = g
        .unit_gold_upgrade_detail(0, archers[0])
        .unwrap_or_else(|why| panic!("Machinery should expose an Archer upgrade: {why}"))
        .1;
    g.players[0].gold = cost * 2.0 + 40.0;

    BasicAi::upgrade_units(&mut g, 0);

    assert!(
        archers
            .iter()
            .all(|unit| g.units[unit].kind == "crossbowman"),
        "both affordable upgrades should be taken under the local gap"
    );
    assert_eq!(g.players[0].gold, 40.0);
}

/// A Gold upgrade is only offered inside the borders, so an army that
/// spends its life on frontier patrol or in no-man's-land can never take
/// one however rich its owner is - the modernization pass simply never
/// sees those units. Measured over a full 6-player game, a third of all
/// military unit-turns were spent on unowned tiles.
#[test]
fn a_unit_stranded_outside_the_borders_walks_home_to_modernize() {
    let (mut g, source, target) = island_colony_game(1);
    grant_tech_with_prerequisites(&mut g, 0, "iron_working");
    g.players[0]
        .strategic_resources
        .insert(crate::name!("iron"), 400.0);
    g.players[0].gold = 900.0;
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("plains");
    }
    let stranded = g.spawn_test_unit("warrior", 0, target);
    assert!(g.map.tiles[&target].owner_city.is_none());
    let before = g.wdist(target, source);

    let mut ai = BasicAi::new();
    ai.peacetime_step(&mut g, 0, stranded, false);

    let after = g.wdist(g.units[&stranded].pos, source);
    assert!(after < before, "before={before} after={after}");

    // A unit already standing at home has nothing to walk toward, and one
    // whose successor is out of reach is left to patrol as before.
    let garrison = g.spawn_test_unit("warrior", 0, source);
    assert!(!ai.modernization_step(&mut g, 0, garrison));
    g.players[0].gold = 0.0;
    let broke = g.spawn_test_unit("warrior", 0, target);
    assert!(!ai.modernization_step(&mut g, 0, broke));
}

/// A Recon unit explores for as long as any tile is unseen, which on an
/// ocean map is forever, so Scouts and Skirmishers walked out across the
/// water and stayed there: 11% of every empire's land army ended its games
/// embarked, where no upgrade is offered at all.
#[test]
fn an_embarked_unit_comes_ashore_before_it_explores_any_further() {
    let (mut g, source, _) = island_colony_game(1);
    grant_tech_with_prerequisites(&mut g, 0, "iron_working");
    // Without embarkation a castaway cannot cross open water at all, so
    // grant the technology that put it out there in the first place.
    grant_tech_with_prerequisites(&mut g, 0, "shipbuilding");
    g.players[0]
        .strategic_resources
        .insert(crate::name!("iron"), 400.0);
    g.players[0].gold = 900.0;
    let at_sea = g
        .map
        .tiles
        .keys()
        .copied()
        .filter(|pos| g.rules.is_water(&g.map.tiles[pos]))
        // Far enough out that the only way home is not the city tile
        // itself, which the starting garrison already occupies.
        .filter(|pos| g.wdist(*pos, source) >= 3)
        .min_by_key(|pos| (g.wdist(*pos, source), *pos))
        .expect("the colony sits in an ocean");
    let castaway = g.spawn_test_unit("warrior", 0, at_sea);
    assert!(
        g.is_embarked(&g.units[&castaway]),
        "the case needs a unit at sea"
    );
    let before = g.wdist(at_sea, source);

    let mut ai = BasicAi::new();
    ai.peacetime_step(&mut g, 0, castaway, false);

    assert!(
        g.wdist(g.units[&castaway].pos, source) < before,
        "an embarked unit heads for shore instead of exploring on"
    );
}

/// The come-ashore rule above only reaches units that have an upgrade
/// waiting: `modernization_step` returns before it does anything when
/// `unit_upgrade_target` is `None`. Everything else stayed at sea, where
/// it cannot attack at all and defends at `embarked_strength`. Measured
/// across 133 live runs, land combat units spend a mean 15% of their
/// unit-turns embarked and 21.7% while one of our own cities is taking
/// damage; on run `civvis-20260803T130831Z` a Crossbowman left the capital
/// tile and paced between two sea hexes for 47 turns while that capital
/// sat at 179/200 damage.
#[test]
fn a_castaway_with_no_upgrade_waiting_still_comes_ashore() {
    let (mut g, source, _target) = island_colony_game(1);
    grant_tech_with_prerequisites(&mut g, 0, "shipbuilding");
    // No iron, no gold, no unlocked successor: the modernization path
    // cannot fire, which is the whole point of the case.
    g.players[0].gold = 0.0;
    let at_sea = g
        .map
        .tiles
        .keys()
        .copied()
        .filter(|pos| g.rules.is_water(&g.map.tiles[pos]))
        .filter(|pos| g.wdist(*pos, source) >= 3)
        .min_by_key(|pos| (g.wdist(*pos, source), *pos))
        .expect("the colony sits in an ocean");
    // The claim is "heads for shore", so measure distance to the nearest
    // dry tile — not to the city, which a unit rounding a headland can
    // move away from while still doing exactly the right thing.
    let dry_tiles: Vec<Pos> = g
        .map
        .tiles
        .iter()
        .filter(|(_, tile)| !g.rules.is_water(tile) && g.rules.is_passable(tile))
        .map(|(pos, _)| *pos)
        .collect();
    assert!(!dry_tiles.is_empty(), "the island has land on it");
    let to_shore = |g: &Game, pos: Pos| {
        dry_tiles
            .iter()
            .map(|dry| g.wdist(pos, *dry))
            .min()
            .expect("dry_tiles is non-empty")
    };
    let before = to_shore(&g, at_sea);

    // The two arms get their own unit: a step spends `moves_left`, so
    // replaying one unit through both would measure an exhausted turn
    // rather than the flag.
    let frozen_unit = g.spawn_test_unit("warrior", 0, at_sea);
    assert!(
        g.is_embarked(&g.units[&frozen_unit]),
        "the case needs a unit at sea"
    );
    let mut frozen = BasicAi::new();
    // The pre-existing come-ashore path is unreachable here — that is the
    // defect, not an artifact of the setup.
    assert!(!frozen.modernization_step(&mut g, 0, frozen_unit));

    let castaway = g.spawn_test_unit("warrior", 0, at_sea);
    let mut ai = BasicAi::new();
    ai.come_ashore = true;

    // Walk both arms the same number of turns. One step is not the claim —
    // rounding a headland can leave shore distance flat for a turn — so
    // give each the same budget and compare where they end up.
    for _ in 0..12 {
        g.turn += 1;
        for uid in [frozen_unit, castaway] {
            let unit = g.units.get_mut(&uid).expect("arm alive");
            unit.moves_left = 2.0;
            unit.moved = false;
            unit.acted = false;
            unit.fortified = false;
        }
        frozen.peacetime_step(&mut g, 0, frozen_unit, false);
        ai.peacetime_step(&mut g, 0, castaway, false);
    }

    let swam = g.units[&frozen_unit].pos;
    let landed = g.units[&castaway].pos;
    assert!(
        g.is_embarked(&g.units[&frozen_unit]),
        "frozen behaviour keeps the castaway at sea; it reached {swam:?}"
    );
    assert!(
        !g.is_embarked(&g.units[&castaway]),
        "a castaway with nothing to upgrade into still comes ashore: \
             went {at_sea:?} -> {landed:?} (shore distance was {before}, now {}); \
             its frozen twin swam to {swam:?}",
        to_shore(&g, landed)
    );
}

/// The standing-army target used to count heads, so a single Warrior that
/// outlived its era filled a city's whole military allowance and the
/// empire stopped building anything better. Weighting each unit by the
/// fraction of a front-line unit it can still field is what keeps late
/// production and Gold flowing into modern units.
#[test]
fn an_obsolete_garrison_no_longer_fills_the_standing_army_target() {
    let (g, _, _) = island_colony_game(1);
    // Nothing unlocked yet: every unit is as modern as the empire gets.
    assert_eq!(BasicAi::force_weight(&g, "warrior", (0.0, 0.0)), 1.0);

    let front_line = (55.0, 0.0); // Musketman
    let warrior = BasicAi::force_weight(&g, "warrior", front_line);
    let musketman = BasicAi::force_weight(&g, "musketman", front_line);
    assert_eq!(musketman, 1.0);
    assert!(warrior < 0.4, "warrior={warrior}");
    // Even a relic holds a tile, so it never counts for nothing at all.
    assert!(BasicAi::force_weight(&g, "warrior", (140.0, 0.0)) >= 0.2);
    // Naval units are measured against the fleet, not against the army.
    assert_eq!(BasicAi::force_weight(&g, "quadrireme", (55.0, 0.0)), 1.0);
}

#[test]
fn basic_ai_modernizes_affordable_obsolete_units() {
    let mut game = Game::new_full(1, 20, 14, 41_005, 40, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = game.player_city_ids(0)[0];
    let home = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| game.unit_ids_at(*position).is_empty())
        .unwrap();
    game.players[0].techs.insert(crate::name!("archery"));
    game.players[0].gold = 180.0;
    let slinger = game.spawn_test_unit("slinger", 0, home);

    BasicAi::upgrade_units(&mut game, 0);

    assert_eq!(game.units[&slinger].kind, "archer");
    assert_eq!(game.players[0].gold, 120.0);
}

#[test]
fn ai_claims_an_earned_great_person_instead_of_leaving_it_pending() {
    let mut game = Game::new_full(1, 20, 14, 41_006, 80, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = game.player_city_ids(0)[0];
    let campus = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != game.cities[&city].pos)
        .unwrap();
    game.map.tiles.get_mut(&campus).unwrap().district = Some(crate::name!("campus"));
    game.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(crate::name!("campus"), campus);
    let cost = game.gp_cost(0, "scientist");
    game.players[0].gpp.insert("scientist".to_string(), cost);
    let offered_scientist = game
        .current_great_person("scientist")
        .expect("the roster offers a Great Scientist")
        .0
        .to_string();
    assert!(game.legal_actions(0).iter().any(
        |action| matches!(action, Action::RecruitGreatPerson { kind } if kind == "scientist")
    ));

    // Firaxis may have a Merchant (or any other class) on its actual
    // Great People screen while CIVVIS's native roster still has
    // Hypatia. The free-claim sweep must not emit the latter order merely
    // because it has enough local points: the host will refuse it and the
    // unspent resource never reaches an offered person.
    let mut different_live_offer = game.clone();
    different_live_offer.players[0].live_great_person_offers =
        Some(["merchant".to_string()].into_iter().collect());
    assert!(!different_live_offer.great_person_class_offered_now(0, "scientist"));
    assert!(!different_live_offer.legal_actions(0).iter().any(
        |action| matches!(action, Action::RecruitGreatPerson { kind } if kind == "scientist")
    ));
    assert_eq!(
        BasicAi::claim_free_great_people(&mut different_live_offer, 0),
        0
    );
    assert_eq!(
        different_live_offer.players[0]
            .gp_claimed
            .get("scientist")
            .copied()
            .unwrap_or(0),
        0
    );
    assert!(different_live_offer
        .apply(
            0,
            &Action::RecruitGreatPerson {
                kind: "scientist".to_string(),
            },
        )
        .is_err());

    assert_eq!(BasicAi::claim_free_great_people(&mut game, 0), 1);
    assert_eq!(game.players[0].gp_claimed["scientist"], 1);
    // Whoever the market is offering, not a name: #2377 put Gathering
    // Storm's other Classical Scientists in front of Hypatia, and the
    // claim this test is about is "the offered person", not "her".
    assert_eq!(
        game.players[0].great_people.last().map(String::as_str),
        Some(offered_scientist.as_str())
    );
}

#[test]
fn a_stranded_live_scientist_unlocks_and_queues_a_campus() {
    let mut game = Game::new_full(1, 20, 14, 41_106, 80, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    game.players[0].live_great_person_activation_needs.push(
        crate::game::LiveGreatPersonActivationNeed {
            kind: "scientist".to_string(),
            individual: Some("hypatia".to_string()),
            required_district: Some("campus".to_string()),
            ..crate::game::LiveGreatPersonActivationNeed::default()
        },
    );

    assert_eq!(
        BasicAi::live_great_person_tech_goal(&game, 0).as_deref(),
        Some("writing"),
        "research must open the activation district before generic priorities"
    );
    grant_tech_with_prerequisites(&mut game, 0, "writing");
    let ai = BasicAi::new();
    assert!(ai.prioritize_live_great_person_activation(&mut game, 0));
    let city = game.player_city_ids(0)[0];
    assert!(matches!(
        game.cities[&city].queue.first(),
        Some(Item::District { district, .. })
            if game.district_family(*district) == "campus"
    ));
    assert!(
        !ai.prioritize_live_great_person_activation(&mut game, 0),
        "a queued Campus satisfies every waiting Scientist without duplication"
    );
}

#[test]
fn science_expansion_defers_low_impact_new_great_person_paths() {
    let mut game = Game::new_full(1, 20, 14, 41_116, 80, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = game.player_city_ids(0)[0];
    grant_tech_with_prerequisites(&mut game, 0, "astrology");
    game.players[0].live_great_person_activation_needs.push(
        crate::game::LiveGreatPersonActivationNeed {
            kind: "scientist".to_string(),
            individual: Some("hildegard_of_bingen".to_string()),
            required_district: Some("holy_site".to_string()),
            ..crate::game::LiveGreatPersonActivationNeed::default()
        },
    );

    let mut ai = BasicAi::new();
    ai.set_defer_low_impact_science_activation_paths(true);
    assert!(
        !ai.prioritize_live_great_person_activation(&mut game, 0),
        "a frontier Science empire must not open a new Holy Site solely for Hildegard"
    );
    assert!(game.cities[&city].queue.is_empty());

    let mut ordinary = game.clone();
    ai.set_defer_low_impact_science_activation_paths(false);
    assert!(ai.prioritize_live_great_person_activation(&mut ordinary, 0));
    assert!(matches!(
        ordinary.cities[&city].queue.first(),
        Some(Item::District { district, .. })
            if ordinary.district_family(*district) == "holy_site"
    ));

    let need = game.players[0]
        .live_great_person_activation_needs
        .first_mut()
        .unwrap();
    need.individual = Some("hypatia".to_string());
    need.required_district = Some("campus".to_string());
    grant_tech_with_prerequisites(&mut game, 0, "writing");
    ai.set_defer_low_impact_science_activation_paths(true);
    assert!(
        ai.prioritize_live_great_person_activation(&mut game, 0),
        "a high-impact Scientist remains worth a new Campus during expansion"
    );
    assert!(matches!(
        game.cities[&city].queue.first(),
        Some(Item::District { district, .. })
            if game.district_family(*district) == "campus"
    ));
}

/// See `activation_resume_waits`: a paused Holy Site foundation does not
/// resume over a Campus that already holds production.
#[test]
fn an_activation_path_waits_for_a_district_under_way() {
    let mut game = Game::new_full(1, 20, 14, 41_109, 80, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = game.player_city_ids(0)[0];
    game.cities.get_mut(&city).unwrap().pop = 5;
    for position in game.cities[&city].owned_tiles.clone() {
        if position == game.cities[&city].pos {
            continue;
        }
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
    }
    grant_tech_with_prerequisites(&mut game, 0, "astrology");
    grant_tech_with_prerequisites(&mut game, 0, "writing");
    for family in ["holy_site", "campus"] {
        let pos = game
            .district_sites(city, Name::new(family))
            .into_iter()
            .next()
            .unwrap();
        game.apply(
            0,
            &Action::Produce {
                city,
                item: Item::District {
                    district: Name::new(family),
                    pos,
                },
            },
        )
        .unwrap();
    }
    game.cities.get_mut(&city).unwrap().production = 17.0;
    game.players[0].live_great_person_activation_needs.push(
        crate::game::LiveGreatPersonActivationNeed {
            kind: "scientist".to_string(),
            individual: Some("hildegard_of_bingen".to_string()),
            required_district: Some("holy_site".to_string()),
            ..crate::game::LiveGreatPersonActivationNeed::default()
        },
    );
    assert!(
        BasicAi::new()
            .live_great_person_activation_resume_item(&game, 0, city)
            .is_some(),
        "stock resumes the Holy Site over the Campus"
    );
    let mut waits = BasicAi::new();
    waits.activation_resume_waits = true;
    assert_eq!(
        waits.live_great_person_activation_resume_item(&game, 0, city),
        None
    );
    assert!(!waits.prioritize_live_great_person_activation(&mut game, 0));
    assert!(matches!(game.cities[&city].queue.first(),
            Some(Item::District { district, .. }) if *district == "campus"));
    game.cities.get_mut(&city).unwrap().production = 0.0;
    assert!(
        waits
            .live_great_person_activation_resume_item(&game, 0, city)
            .is_some(),
        "a queued district with nothing invested still yields"
    );
}

/// See `activation_keeps_its_building`: a Great Artist and Mary Leakey
/// share one Theater Square, and the Art Museum under way is not swapped
/// for the Archaeological Museum that excludes it.
#[test]
fn an_activation_path_keeps_a_museum_under_way() {
    let mut game = Game::new_full(1, 20, 14, 41_109, 80, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = game.player_city_ids(0)[0];
    let theater = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != game.cities[&city].pos)
        .unwrap();
    game.map.tiles.get_mut(&theater).unwrap().district = Some(crate::name!("theater_square"));
    game.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(crate::name!("theater_square"), theater);
    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("amphitheater"));
    game.players[0].civics.insert(crate::name!("drama_poetry"));
    game.players[0].civics.insert(crate::name!("humanism"));
    for (kind, individual, work) in [
        ("artist", "andrei_rublev", "art"),
        ("scientist", "mary_leakey", "artifact"),
    ] {
        game.players[0].live_great_person_activation_needs.push(
            crate::game::LiveGreatPersonActivationNeed {
                kind: kind.to_string(),
                individual: Some(individual.to_string()),
                required_district: Some("theater_square".to_string()),
                required_great_work: Some(work.to_string()),
                ..crate::game::LiveGreatPersonActivationNeed::default()
            },
        );
    }
    game.apply(
        0,
        &Action::Produce {
            city,
            item: Item::Building {
                building: crate::name!("art_museum"),
            },
        },
    )
    .unwrap();
    game.cities.get_mut(&city).unwrap().production = 46.0;
    assert!(
        matches!(BasicAi::new().live_great_person_activation_resume_item(&game, 0, city),
                Some(Item::Building { building }) if building == "archaeological_museum"),
        "stock swaps the Art Museum for the Archaeological Museum"
    );
    let mut keeps = BasicAi::new();
    keeps.activation_keeps_its_building = true;
    assert_eq!(
        keeps.live_great_person_activation_resume_item(&game, 0, city),
        None
    );
    assert!(!keeps.prioritize_live_great_person_activation(&mut game, 0));
    assert!(matches!(game.cities[&city].queue.first(),
            Some(Item::Building { building }) if *building == "art_museum"));
}

#[test]
fn a_live_great_person_resumes_a_foundation_without_displacing_another_activation_path() {
    let mut game = Game::new_full(1, 20, 14, 41_109, 80, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = game.player_city_ids(0)[0];
    // Two specialty foundations are legal at this population. This is the
    // shape the live mirror observed in Lugdunum: the required Holy Site
    // remains on the map while a later Campus owns the active queue.
    game.cities.get_mut(&city).unwrap().pop = 5;
    for position in game.cities[&city].owned_tiles.clone() {
        if position == game.cities[&city].pos {
            continue;
        }
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
    }
    grant_tech_with_prerequisites(&mut game, 0, "astrology");
    grant_tech_with_prerequisites(&mut game, 0, "writing");
    let holy_site = game
        .district_sites(city, crate::name!("holy_site"))
        .into_iter()
        .next()
        .unwrap();
    game.apply(
        0,
        &Action::Produce {
            city,
            item: Item::District {
                district: crate::name!("holy_site"),
                pos: holy_site,
            },
        },
    )
    .unwrap();
    let campus = game
        .district_sites(city, crate::name!("campus"))
        .into_iter()
        .next()
        .unwrap();
    game.apply(
        0,
        &Action::Produce {
            city,
            item: Item::District {
                district: crate::name!("campus"),
                pos: campus,
            },
        },
    )
    .unwrap();
    game.cities.get_mut(&city).unwrap().production = 17.0;
    game.players[0].live_great_person_activation_needs.push(
        crate::game::LiveGreatPersonActivationNeed {
            kind: "scientist".to_string(),
            individual: Some("hildegard_of_bingen".to_string()),
            required_district: Some("holy_site".to_string()),
            required_missing_building: None,
            required_great_work: None,
        },
    );

    let ai = BasicAi::new();
    let waiting_scientist = crate::game::LiveGreatPersonActivationNeed {
        kind: "scientist".to_string(),
        individual: Some("isaac_newton".to_string()),
        required_district: Some("campus".to_string()),
        ..crate::game::LiveGreatPersonActivationNeed::default()
    };
    game.players[0]
        .live_great_person_activation_needs
        .push(waiting_scientist.clone());
    // Native turn 95/frame 1 alternated these two foundations forever.
    // Check the candidate first so the old implementation fails promptly
    // instead of hanging the entire test suite inside the planner loop.
    assert_eq!(
        ai.live_great_person_activation_resume_item(&game, 0, city),
        None,
        "the Campus already serves another waiting person"
    );
    assert!(!ai.prioritize_live_great_person_activation(&mut game, 0));
    assert_eq!(game.cities[&city].production, 17.0);
    assert!(matches!(game.cities[&city].queue.first(),
            Some(Item::District { district, .. }) if *district == "campus"));

    // Once that person no longer needs the Campus, the Holy Site can
    // resume normally and bank the Campus's progress.
    game.players[0].live_great_person_activation_needs.pop();
    assert!(ai.prioritize_live_great_person_activation(&mut game, 0));
    assert!(matches!(
        game.cities[&city].queue.first(),
        Some(Item::District { district, pos })
            if game.district_family(*district) == "holy_site" && *pos == holy_site
    ));
    assert_eq!(
        game.cities[&city]
            .production_progress
            .get(&format!("district:campus:{},{}", campus.0, campus.1)),
        Some(&17.0),
        "switching back to the placed Holy Site must bank the active Campus"
    );
    assert!(
        !ai.prioritize_live_great_person_activation(&mut game, 0),
        "the active foundation must not be re-applied in a loop"
    );
    game.players[0]
        .live_great_person_activation_needs
        .push(waiting_scientist);
    assert_eq!(
        ai.live_great_person_activation_resume_item(&game, 0, city),
        None,
        "the Holy Site is also protected when it is the active path"
    );
    assert!(!ai.prioritize_live_great_person_activation(&mut game, 0));
}

#[test]
fn a_live_great_person_does_not_preempt_a_military_activation_queue() {
    let mut game = Game::new_full(1, 20, 14, 41_110, 80, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = game.player_city_ids(0)[0];
    game.cities.get_mut(&city).unwrap().pop = 5;
    for position in game.cities[&city].owned_tiles.clone() {
        if position == game.cities[&city].pos {
            continue;
        }
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
    }
    grant_tech_with_prerequisites(&mut game, 0, "astrology");
    grant_tech_with_prerequisites(&mut game, 0, "writing");
    let holy_site = game
        .district_sites(city, crate::name!("holy_site"))
        .into_iter()
        .next()
        .unwrap();
    game.apply(
        0,
        &Action::Produce {
            city,
            item: Item::District {
                district: crate::name!("holy_site"),
                pos: holy_site,
            },
        },
    )
    .unwrap();
    game.apply(
        0,
        &Action::Produce {
            city,
            item: Item::Unit {
                unit: crate::name!("warrior"),
            },
        },
    )
    .unwrap();
    game.players[0].live_great_person_activation_needs.push(
        crate::game::LiveGreatPersonActivationNeed {
            kind: "scientist".to_string(),
            individual: Some("hildegard_of_bingen".to_string()),
            required_district: Some("holy_site".to_string()),
            required_missing_building: None,
            required_great_work: None,
        },
    );

    assert!(
        !BasicAi::new().prioritize_live_great_person_activation(&mut game, 0),
        "a military queue remains an active survival commitment"
    );
    assert!(matches!(
        game.cities[&city].queue.first(),
        Some(Item::Unit { unit }) if unit == "warrior"
    ));
}

#[test]
fn a_stranded_live_writer_can_bank_a_safe_queue_for_a_work_slot() {
    let mut game = Game::new_full(1, 20, 14, 41_111, 80, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = game.player_city_ids(0)[0];
    let theater = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != game.cities[&city].pos)
        .unwrap();
    game.map.tiles.get_mut(&theater).unwrap().district = Some(crate::name!("theater_square"));
    game.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(crate::name!("theater_square"), theater);
    game.players[0].civics.insert(crate::name!("drama_poetry"));

    let repeatable_project = Item::Project {
        project: crate::name!("theater_square_festival"),
    };
    assert!(
        game.can_produce(0, city, &repeatable_project),
        "the fixture must expose a safe repeatable queue"
    );
    game.apply(
        0,
        &Action::Produce {
            city,
            item: repeatable_project,
        },
    )
    .unwrap();
    game.cities.get_mut(&city).unwrap().production = 11.0;
    game.players[0].live_great_person_activation_needs.push(
        crate::game::LiveGreatPersonActivationNeed {
            kind: "writer".to_string(),
            individual: Some("homer".to_string()),
            required_district: Some("theater_square".to_string()),
            ..crate::game::LiveGreatPersonActivationNeed::default()
        },
    );

    let ai = BasicAi::new();
    assert!(ai.prioritize_live_great_person_activation(&mut game, 0));
    assert!(matches!(
        game.cities[&city].queue.first(),
        Some(Item::Building { building })
            if game.building_is_family(building, crate::name!("amphitheater"))
    ));
}

#[test]
fn a_stranded_live_general_builds_a_body_after_encampment() {
    let mut game = Game::new_full(1, 20, 14, 41_112, 80, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = game.player_city_ids(0)[0];
    let encampment = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != game.cities[&city].pos)
        .unwrap();
    game.map.tiles.get_mut(&encampment).unwrap().district = Some(crate::name!("encampment"));
    game.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(crate::name!("encampment"), encampment);
    game.players[0].live_great_person_activation_needs.push(
        crate::game::LiveGreatPersonActivationNeed {
            kind: "general".to_string(),
            individual: Some("sun_tzu".to_string()),
            required_district: Some("encampment".to_string()),
            ..crate::game::LiveGreatPersonActivationNeed::default()
        },
    );

    let ai = BasicAi::new();
    assert!(ai.prioritize_live_great_person_activation(&mut game, 0));
    assert!(matches!(
        game.cities[&city].queue.first(),
        Some(Item::Unit { unit })
            if game.rules.units[unit].class == "military"
                && game.rules.units[unit].domain.as_deref() != Some("sea")
    ));
}

#[test]
fn a_stranded_live_general_can_bank_a_safe_queue_for_a_body() {
    let mut game = Game::new_full(1, 20, 14, 41_113, 80, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = game.player_city_ids(0)[0];
    let mut specialty_tiles = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .filter(|position| *position != game.cities[&city].pos);
    let encampment = specialty_tiles.next().unwrap();
    let theater = specialty_tiles.next().unwrap();
    game.map.tiles.get_mut(&encampment).unwrap().district = Some(crate::name!("encampment"));
    game.map.tiles.get_mut(&theater).unwrap().district = Some(crate::name!("theater_square"));
    game.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(crate::name!("encampment"), encampment);
    game.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(crate::name!("theater_square"), theater);
    game.players[0].civics.insert(crate::name!("drama_poetry"));

    let repeatable_project = Item::Project {
        project: crate::name!("theater_square_festival"),
    };
    assert!(
        game.can_produce(0, city, &repeatable_project),
        "the fixture must expose a safe repeatable queue"
    );
    game.apply(
        0,
        &Action::Produce {
            city,
            item: repeatable_project,
        },
    )
    .unwrap();
    game.cities.get_mut(&city).unwrap().production = 11.0;
    game.players[0].live_great_person_activation_needs.push(
        crate::game::LiveGreatPersonActivationNeed {
            kind: "general".to_string(),
            individual: Some("sun_tzu".to_string()),
            required_district: Some("encampment".to_string()),
            ..crate::game::LiveGreatPersonActivationNeed::default()
        },
    );

    let ai = BasicAi::new();
    assert!(ai.prioritize_live_great_person_activation(&mut game, 0));
    assert!(matches!(
        game.cities[&city].queue.first(),
        Some(Item::Unit { unit })
            if game.rules.units[unit].class == "military"
                && game.rules.units[unit].domain.as_deref() != Some("sea")
    ));
}

#[test]
fn a_stranded_live_merchant_can_bank_a_safe_queue_for_a_commercial_hub() {
    let mut game = Game::new_full(1, 20, 14, 41_114, 80, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = game.player_city_ids(0)[0];
    game.cities.get_mut(&city).unwrap().pop = 5;
    for position in game.cities[&city].owned_tiles.clone() {
        if position == game.cities[&city].pos {
            continue;
        }
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
    }
    grant_tech_with_prerequisites(&mut game, 0, "currency");
    let theater = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != game.cities[&city].pos)
        .unwrap();
    game.map.tiles.get_mut(&theater).unwrap().district = Some(crate::name!("theater_square"));
    game.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(crate::name!("theater_square"), theater);
    game.players[0].civics.insert(crate::name!("drama_poetry"));

    let repeatable_project = Item::Project {
        project: crate::name!("theater_square_festival"),
    };
    assert!(
        game.can_produce(0, city, &repeatable_project),
        "the fixture must expose a safe repeatable queue"
    );
    game.apply(
        0,
        &Action::Produce {
            city,
            item: repeatable_project,
        },
    )
    .unwrap();
    game.cities.get_mut(&city).unwrap().production = 11.0;
    game.players[0].live_great_person_activation_needs.push(
        crate::game::LiveGreatPersonActivationNeed {
            kind: "merchant".to_string(),
            individual: Some("marco_polo".to_string()),
            required_district: Some("commercial_hub".to_string()),
            ..crate::game::LiveGreatPersonActivationNeed::default()
        },
    );

    let ai = BasicAi::new();
    assert!(ai.prioritize_live_great_person_activation(&mut game, 0));
    assert!(matches!(
        game.cities[&city].queue.first(),
        Some(Item::District { district, .. })
            if game.district_family(*district) == "commercial_hub"
    ));
}

#[test]
fn an_observed_unfinished_activation_district_reserves_its_family() {
    let mut game = Game::new_full(1, 20, 14, 41_108, 80, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    grant_tech_with_prerequisites(&mut game, 0, "rocketry");
    let city = game.player_city_ids(0)[0];
    let spaceport = crate::name!("spaceport");
    let site = game
        .district_sites(city, spaceport)
        .into_iter()
        .next()
        .expect("a legal Spaceport site");
    let item = Item::District {
        district: spaceport,
        pos: site,
    };
    let cost = game.item_cost_for_city(0, city, &item);
    let foundation = crate::world::DistrictFoundation {
        district: spaceport,
        cost,
    };
    game.map.tiles.get_mut(&site).unwrap().district_foundation = Some(foundation);
    game.players[0].live_great_person_activation_needs.push(
        crate::game::LiveGreatPersonActivationNeed {
            kind: "scientist".to_string(),
            individual: Some("stephanie_kwolek".to_string()),
            required_district: Some("spaceport".to_string()),
            ..crate::game::LiveGreatPersonActivationNeed::default()
        },
    );

    assert!(
        BasicAi::empire_district_family_ready_or_queued(&game, 0, "spaceport"),
        "an incomplete district from the host reserves its family just like a queue"
    );
    assert!(
        BasicAi::new()
            .live_great_person_activation_item(&game, 0, city)
            .is_none(),
        "the activation pass must not start a second Spaceport while the first is unfinished"
    );
}

#[test]
fn a_stranded_live_artist_builds_the_great_work_slot_chain() {
    let mut game = Game::new_full(1, 20, 14, 41_107, 80, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = game.player_city_ids(0)[0];
    let theater = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != game.cities[&city].pos)
        .unwrap();
    game.map.tiles.get_mut(&theater).unwrap().district = Some(crate::name!("theater_square"));
    game.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(crate::name!("theater_square"), theater);
    game.players[0].civics.insert(crate::name!("drama_poetry"));
    game.players[0].live_great_person_activation_needs.push(
        crate::game::LiveGreatPersonActivationNeed {
            kind: "artist".to_string(),
            individual: Some("donatello".to_string()),
            required_district: Some("theater_square".to_string()),
            ..crate::game::LiveGreatPersonActivationNeed::default()
        },
    );

    assert_eq!(
        BasicAi::live_great_person_civic_goal(&game, 0).as_deref(),
        Some("humanism"),
        "the planner must unlock the Museum rather than stopping at a Theater"
    );
    let ai = BasicAi::new();
    assert!(ai.prioritize_live_great_person_activation(&mut game, 0));
    assert!(matches!(
        game.cities[&city].queue.first(),
        Some(Item::Building { building })
            if game.building_is_family(building, crate::name!("amphitheater"))
    ));
    assert!(
        !ai.prioritize_live_great_person_activation(&mut game, 0),
        "the queued prerequisite must not be cloned by a second city"
    );
}

#[test]
fn a_stranded_live_artifact_scientist_builds_the_exact_museum() {
    let mut game = Game::new_full(1, 20, 14, 41_109, 80, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = game.player_city_ids(0)[0];
    let theater = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != game.cities[&city].pos)
        .unwrap();
    game.map.tiles.get_mut(&theater).unwrap().district = Some(crate::name!("theater_square"));
    game.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(crate::name!("theater_square"), theater);
    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("amphitheater"));
    game.players[0].civics.insert(crate::name!("drama_poetry"));
    game.players[0].live_great_person_activation_needs.push(
        crate::game::LiveGreatPersonActivationNeed {
            kind: "scientist".to_string(),
            individual: Some("mary_leakey".to_string()),
            required_district: Some("theater_square".to_string()),
            required_great_work: Some("artifact".to_string()),
            ..crate::game::LiveGreatPersonActivationNeed::default()
        },
    );

    assert_eq!(
        BasicAi::live_great_person_civic_goal(&game, 0).as_deref(),
        Some("humanism"),
        "the named Artifact prerequisite must lead to Humanism"
    );
    game.players[0].civics.insert(crate::name!("humanism"));
    let ai = BasicAi::new();
    assert!(ai.prioritize_live_great_person_activation(&mut game, 0));
    assert!(matches!(
        game.cities[&city].queue.first(),
        Some(Item::Building { building })
            if game.building_is_family(building, crate::name!("archaeological_museum"))
    ));
    assert!(
        !ai.prioritize_live_great_person_activation(&mut game, 0),
        "the exact Museum prerequisite must not be cloned while queued"
    );
}

#[test]
fn a_live_wonder_engineer_does_not_cycle_through_host_rejected_wonders() {
    let activation_board = || {
        let mut game = Game::new_full(1, 24, 16, 91_772, 120, 0, false);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let city = game.player_city_ids(0)[0];
        let techs: Vec<Name> = game.rules.techs.keys().cloned().collect();
        let civics: Vec<Name> = game.rules.civics.keys().cloned().collect();
        game.players[0].techs.extend(techs);
        game.players[0].civics.extend(civics);
        for position in game.cities[&city].owned_tiles.clone() {
            if position == game.cities[&city].pos {
                continue;
            }
            let tile = game.map.tiles.get_mut(&position).unwrap();
            tile.terrain = crate::name!("desert");
            tile.feature = None;
            tile.hills = false;
            tile.resource = None;
            tile.improvement = None;
            tile.district = None;
            tile.wonder = None;
        }
        game.players[0].live_great_person_activation_needs.push(
            crate::game::LiveGreatPersonActivationNeed {
                kind: "engineer".to_string(),
                individual: Some("imhotep".to_string()),
                required_district: None,
                ..crate::game::LiveGreatPersonActivationNeed::default()
            },
        );
        (game, city)
    };

    let (mut available, city) = activation_board();
    let ai = BasicAi::new();
    assert!(ai.prioritize_live_great_person_activation(&mut available, 0));
    assert!(matches!(
        available.cities[&city].queue.first(),
        Some(Item::Wonder { .. })
    ));

    let (mut refused, city) = activation_board();
    std::sync::Arc::make_mut(&mut refused.blocked_wonders)
        .entry(city)
        .or_default()
        .insert(crate::name!("pyramids"));
    assert!(
            !ai.prioritize_live_great_person_activation(&mut refused, 0),
            "one host rejection must return this city to ordinary production instead of trying every other wonder"
        );
    assert!(refused.cities[&city].queue.is_empty());
}

/// The 36 cities that vanished at FULL loyalty, in one assertion: a city on
/// 100 losing ground fast is in more danger than a city on 60 that is
/// stable, and the level-only rule ranked them exactly the other way round.
#[test]
fn a_city_bleeding_loyalty_outranks_a_lower_but_stable_one() {
    let (mut game, source, target) = island_colony_game(1);
    let second_settler = game.spawn_test_unit("settler", 0, target);
    let bleeding = game.found_city_for(0, game.units[&second_settler].pos, None);
    let stable = game.city_at(source).unwrap();

    // Full loyalty, but the engine's rate says it is going fast.
    game.cities.get_mut(&bleeding).unwrap().loyalty = 100.0;
    game.cities.get_mut(&stable).unwrap().loyalty = 60.0;

    let mut live = BasicAi::new();
    live.loyalty_rate_alarm = true;
    let frozen = BasicAi::new();

    // ⚠ Force the rates rather than hoping the map generates a falling city.
    // `observed_city_loyalty_per_turn` is the map the MIRROR writes from
    // Civilization VI's own export, so this is the live representation and
    // not a test-only back door — and it makes the case unconditional
    // instead of a test that quietly passes when nothing is falling.
    std::sync::Arc::make_mut(&mut game.observed_city_loyalty_per_turn).insert(bleeding, -12.0);
    std::sync::Arc::make_mut(&mut game.observed_city_loyalty_per_turn).insert(stable, 1.0);
    let bleed_rate = game.city_loyalty_per_turn(&game.cities[&bleeding]);
    let stable_rate = game.city_loyalty_per_turn(&game.cities[&stable]);
    assert_eq!(
        bleed_rate, -12.0,
        "the injected rate must be what the AI reads"
    );

    let live_bleeding = live.loyalty_emergency(&game, bleeding);
    let live_stable = live.loyalty_emergency(&game, stable);
    assert!(
        live_bleeding.is_some(),
        "a city losing {bleed_rate}/turn must register as an emergency at ANY level"
    );
    assert!(
        live_stable.is_none() || live_bleeding.unwrap() < live_stable.unwrap(),
        "the bleeding city must rank ahead: bleeding={live_bleeding:?} stable={live_stable:?} \
             (rates {bleed_rate} vs {stable_rate})"
    );

    // The frozen controller keeps the old level-only rule, so the city on
    // 100 is invisible to it however fast it is falling.
    assert_eq!(
        frozen.loyalty_emergency(&game, bleeding),
        None,
        "a tournament controller must not gain the rate reading"
    );
    assert!(
        frozen.loyalty_emergency(&game, stable).is_some(),
        "and must still see the low-level city exactly as before"
    );
}

/// The ordering rule on explicit rates, independent of what any map happens
/// to generate.
#[test]
fn turns_to_flip_orders_loyalty_emergencies() {
    let (mut game, _, _) = island_colony_game(1);
    let city = game.player_city_ids(0)[0];
    let mut ai = BasicAi::new();
    ai.loyalty_rate_alarm = true;

    // A city that is not falling and is comfortably loyal is not an
    // emergency at all.
    game.cities.get_mut(&city).unwrap().loyalty = 100.0;
    let quiet = ai.loyalty_emergency(&game, city);
    let rate = game.city_loyalty_per_turn(&game.cities[&city]);
    assert!(
        rate >= 0.0,
        "test precondition changed: this map's lone city is now falling ({rate}/turn)"
    );
    assert_eq!(
        quiet, None,
        "a stable city on 100 loyalty is not an emergency"
    );

    // Below the old alarm level it is flagged whatever the rate, and always
    // behind anything actually falling.
    game.cities.get_mut(&city).unwrap().loyalty = LOYALTY_LEVEL_ALARM - 1.0;
    let low = ai
        .loyalty_emergency(&game, city)
        .expect("below the alarm level");
    assert!(
        low > 1_000.0,
        "a stable-but-low city must rank behind any falling city, got {low}"
    );
}

#[test]
fn ai_reassigns_a_governor_from_a_safe_city_to_a_loyalty_emergency() {
    let (mut game, source, target) = island_colony_game(1);
    let second_settler = game.spawn_test_unit("settler", 0, target);
    let second = game.found_city_for(0, game.units[&second_settler].pos, None);
    let first = game.city_at(source).unwrap();
    game.players[0]
        .counters
        .insert("district_governor_titles".to_string(), 1);
    game.apply(
        0,
        &Action::AppointGovernor {
            governor: crate::name!("victor"),
            city: first,
        },
    )
    .unwrap();
    game.cities.get_mut(&first).unwrap().loyalty = 100.0;
    game.cities.get_mut(&second).unwrap().loyalty = 35.0;

    assert!(BasicAi::new().reassign_governor_for_loyalty(&mut game, 0));
    assert_eq!(game.players[0].governor_roster["victor"].city, Some(second));
}

/// `victor_first_for_a_short_runway`: a city about to flip takes Victor,
/// who establishes in three turns, ahead of an unassigned Governor; a
/// longer runway keeps the unassigned Governor first.
#[test]
fn a_short_loyalty_runway_takes_victor_before_an_unassigned_governor() {
    let run = |flag: bool, loyalty: f64| {
        let (mut game, source, target) = island_colony_game(1);
        let second_settler = game.spawn_test_unit("settler", 0, target);
        let second = game.found_city_for(0, game.units[&second_settler].pos, None);
        let first = game.city_at(source).unwrap();
        game.players[0]
            .counters
            .insert("district_governor_titles".to_string(), 1);
        game.apply(
            0,
            &Action::AppointGovernor {
                governor: crate::name!("victor"),
                city: first,
            },
        )
        .unwrap();
        game.players[0].governor_roster.insert(
            "pingala".to_string(),
            crate::game::GovernorState {
                city: None,
                assigned_turn: 0,
                disabled_until: 0,
                promotions: std::collections::BTreeSet::new(),
            },
        );
        game.cities.get_mut(&first).unwrap().loyalty = 100.0;
        game.cities.get_mut(&second).unwrap().loyalty = loyalty;
        let mut ai = BasicAi::new();
        ai.victor_first_for_a_short_runway = flag;
        assert!(ai.reassign_governor_for_loyalty(&mut game, 0));
        game.players[0].governor_roster["victor"].city == Some(second)
    };
    assert!(!run(false, 3.0), "off: the unassigned Governor goes");
    assert!(run(true, 3.0), "a short runway takes Victor");
    assert!(!run(true, 35.0), "a long runway keeps the unassigned first");
}

#[test]
fn ai_uses_heathen_conversion_before_the_apostle_moves() {
    let mut game = Game::new_full(1, 20, 14, 41_007, 80, 0, true);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let center = game.cities[&game.player_city_ids(0)[0]].pos;
    let adjacent = game
        .nbrs(center)
        .into_iter()
        .find(|position| {
            game.map
                .get(*position)
                .is_some_and(|tile| game.rules.is_passable(tile) && !game.rules.is_water(tile))
        })
        .unwrap();
    let apostle = game.spawn_test_unit("apostle", 0, center);
    game.units
        .get_mut(&apostle)
        .unwrap()
        .promotions
        .insert(crate::name!("heathen_conversion"));
    let barbarian = game.barb_pid.unwrap();
    let converted = game.spawn_test_unit("warrior", barbarian, adjacent);

    assert_eq!(BasicAi::use_opportunistic_unit_tools(&mut game, 0), 1);
    assert_eq!(game.units[&converted].owner, 0);
    assert_eq!(game.units[&apostle].moves_left, 0.0);
}

#[test]
fn coastal_empires_research_navigation_before_generic_land_unlocks() {
    let (mut g, _, _) = island_colony_game(1);
    g.players[0].research = None;
    let ai = BasicAi::new();
    ai.research(&mut g, 0);
    assert_eq!(g.players[0].research.as_deref(), Some("sailing"));
}

#[test]
fn market_city_states_finish_the_banking_branch_before_late_era_fallbacks() {
    let mut g = Game::new_full(1, 20, 14, 18878401, 250, 0, false);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|id| g.units[id].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let cid = g.player_city_ids(0)[0];
    g.players[0].is_minor = true;
    g.players[0].civ = "Zanzibar".to_string();
    g.cities
        .get_mut(&cid)
        .unwrap()
        .buildings
        .extend([crate::name!("market"), crate::name!("walls")]);
    grant_tech_with_prerequisites(&mut g, 0, "education");
    g.players[0]
        .techs
        .extend([crate::name!("sailing"), crate::name!("currency")]);
    let completed_eras: Vec<Name> = g
        .rules
        .techs
        .iter()
        .filter(|(_, spec)| spec.era < 2)
        .map(|(tech, _)| *tech)
        .collect();
    g.players[0].techs.extend(completed_eras);
    g.players[0].research = None;
    let mut ai = BasicAi::new();
    ai.minor = true;

    assert_eq!(BasicAi::economic_research_goal(&g, 0), Some("banking"));
    assert!(g.available_techs(0).iter().any(|tech| tech == "stirrups"));
    ai.minor_research(&mut g, 0);
    assert_eq!(g.players[0].research.as_deref(), Some("stirrups"));

    g.players[0].techs.insert(crate::name!("stirrups"));
    let completed_eras: Vec<Name> = g
        .rules
        .techs
        .iter()
        .filter(|(_, spec)| spec.era < 3)
        .map(|(tech, _)| *tech)
        .collect();
    g.players[0].techs.extend(completed_eras);
    g.players[0].research = None;
    ai.minor_research(&mut g, 0);
    assert_eq!(g.players[0].research.as_deref(), Some("banking"));
}

#[test]
fn naval_wars_prioritize_the_next_fleet_upgrade() {
    let (mut g, source, _) = island_colony_game(2);
    grant_tech_with_prerequisites(&mut g, 0, "cartography");
    grant_tech_with_prerequisites(&mut g, 0, "celestial_navigation");
    let completed_eras: Vec<Name> = g
        .rules
        .techs
        .iter()
        .filter(|(_, spec)| spec.era < 3)
        .map(|(tech, _)| *tech)
        .collect();
    g.players[0].techs.extend(completed_eras);
    g.at_war.insert((0, 1));
    let contact = g
        .nbrs(source)
        .into_iter()
        .find(|pos| g.map.get(*pos).is_some_and(|tile| g.rules.is_water(tile)))
        .unwrap();
    g.spawn_test_unit("galley", 1, contact);
    g.players[0].research = None;
    assert_eq!(
        BasicAi::water_research_goal(&g, 0),
        Some("square_rigging"),
        "available={:?}, war={}, enemy_alive={}",
        g.available_techs(0),
        g.is_at_war(0, 1),
        g.players[1].alive
    );
    assert!(
        g.available_techs(0)
            .iter()
            .any(|tech| tech == "square_rigging"),
        "available={:?}",
        g.available_techs(0)
    );
    let available = g.available_techs(0);
    BasicAi::new().research(&mut g, 0);
    assert_eq!(
        g.players[0].research.as_deref(),
        Some("square_rigging"),
        "available before selection: {available:?}"
    );
}

#[test]
fn coastal_cities_build_a_melee_ship_for_exploration_and_capture() {
    let (mut g, _, _) = island_colony_game(1);
    g.players[0].techs.insert(crate::name!("sailing"));
    let cid = g.player_city_ids(0)[0];
    let ai = BasicAi::new();
    let item = ai
        .pick_item(&g, 0, cid, 1, 0, 2, 1, 0, 4, 2, 2)
        .expect("coastal city has a production choice");
    assert!(matches!(item, Item::Unit { unit } if unit == "galley"));
}

#[test]
fn coastal_cities_add_ranged_firepower_after_the_melee_screen() {
    let (mut g, source, _) = island_colony_game(2);
    g.players[0]
        .techs
        .extend([crate::name!("sailing"), crate::name!("shipbuilding")]);
    g.at_war.insert((0, 1));
    let water = g
        .nbrs(source)
        .into_iter()
        .find(|pos| g.map.get(*pos).is_some_and(|tile| g.rules.is_water(tile)))
        .unwrap();
    g.spawn_test_unit("galley", 0, water);
    let enemy_water = g
        .nbrs(water)
        .into_iter()
        .find(|pos| g.map.get(*pos).is_some_and(|tile| g.rules.is_water(tile)))
        .unwrap();
    g.spawn_test_unit("galley", 1, enemy_water);
    let cid = g.player_city_ids(0)[0];
    let item = BasicAi::new()
        .pick_item(&g, 0, cid, 1, 0, 2, 1, 0, 5, 3, 2)
        .expect("coastal city has a production choice");
    assert!(matches!(item, Item::Unit { unit } if unit == "quadrireme"));
}

#[test]
fn city_states_keep_a_bounded_force_that_scales_with_local_threat() {
    let mut g = Game::new_full(2, 24, 16, 97, 120, 1, false);
    let minor = g
        .players
        .iter()
        .find(|player| player.is_minor && !player.is_barbarian)
        .map(|player| player.id)
        .unwrap();
    assert_eq!(BasicAi::minor_military_budget(&g, minor), 3);
    g.world_era = 6;
    assert_eq!(BasicAi::minor_military_budget(&g, minor), 3);

    let major_units = g.player_unit_ids(0);
    for unit in major_units {
        g.remove_unit(unit);
    }
    g.at_war.insert((0, minor));
    assert_eq!(BasicAi::minor_military_budget(&g, minor), 4);

    let city = g.player_city_ids(minor)[0];
    let front = g
        .nbrs(g.cities[&city].pos)
        .into_iter()
        .find(|position| {
            g.map
                .get(*position)
                .is_some_and(|tile| !g.rules.is_water(tile))
        })
        .unwrap();
    for _ in 0..8 {
        g.spawn_test_unit("warrior", 0, front);
    }
    assert_eq!(BasicAi::minor_military_budget(&g, minor), 7);

    g.at_war.clear();
    let mut ai = BasicAi::new();
    ai.minor = true;
    let choice = ai.pick_item(&g, minor, city, 1, 0, 1, 0, 0, 3, 2, 1);
    assert!(
        !matches!(choice, Some(Item::Unit { ref unit }) if g.rules.units[unit].class == "military"),
        "a peaceful city-state at its force budget must prefer infrastructure or idle"
    );
}

/// `plaza-in-the-district-list`: the capital wants the empire's one
/// Government Plaza until a city holds it; the Plaza's city then builds
/// the Warlord's Throne, then the Grand Master's Chapel.
#[test]
fn plaza_in_the_district_list_wants_one_plaza_then_its_tier_buildings() {
    let mut g = Game::new_full(1, 24, 16, 91_770, 120, 0, false);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| g.units[unit].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = g.player_city_ids(0)[0];
    g.players[0].civics.insert(crate::name!("state_workforce"));
    assert!(
        BasicAi::plaza_wanted_here(&g, 0, city),
        "the capital, no Plaza"
    );
    assert_eq!(
        BasicAi::plaza_building_item(&g, 0, city),
        None,
        "no Plaza yet"
    );
    assert!(
        matches!(
            BasicAi::plaza_item(&g, 0, city),
            Some(Item::District { district, .. }) if district == "government_plaza"
        ),
        "the capital's Plaza"
    );
    let site = g.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| {
            let tile = &g.map.tiles[position];
            *position != g.cities[&city].pos
                && tile.district.is_none()
                && !g.rules.is_water(tile)
                && g.rules.is_passable(tile)
        })
        .unwrap();
    g.map.tiles.get_mut(&site).unwrap().district = Some(crate::name!("government_plaza"));
    g.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(crate::name!("government_plaza"), site);
    assert!(!BasicAi::plaza_wanted_here(&g, 0, city), "one per empire");
    assert!(
        matches!(
            BasicAi::plaza_building_item(&g, 0, city),
            Some(Item::Building { building }) if building == "warlords_throne"
        ),
        "the conquest tier first"
    );
    g.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("warlords_throne"));
    assert!(
        matches!(
            BasicAi::plaza_building_item(&g, 0, city),
            Some(Item::Building { building }) if building == "grand_masters_chapel"
        ),
        "then the Chapel"
    );
}

/// `plaza-serves-the-scientist`: with the science flag the Plaza's city builds
/// the Ancestral Hall, then the Intelligence Agency; a Plaza already holding
/// the Warlord's Throne goes straight to the Agency; without the flag the
/// stock list is unchanged.
#[test]
fn plaza_serves_the_scientist_takes_the_hall_then_the_agency() {
    let mut g = Game::new_full(1, 24, 16, 91_770, 120, 0, false);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| g.units[unit].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = g.player_city_ids(0)[0];
    g.players[0].civics.insert(crate::name!("state_workforce"));
    let site = g.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| {
            let tile = &g.map.tiles[position];
            *position != g.cities[&city].pos
                && tile.district.is_none()
                && !g.rules.is_water(tile)
                && g.rules.is_passable(tile)
        })
        .unwrap();
    g.map.tiles.get_mut(&site).unwrap().district = Some(crate::name!("government_plaza"));
    g.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(crate::name!("government_plaza"), site);
    let building =
        |g: &Game, science: bool| match BasicAi::plaza_building_item_for(g, 0, city, science) {
            Some(Item::Building { building }) => Some(building),
            _ => None,
        };
    assert_eq!(
        building(&g, false),
        Some(crate::name!("warlords_throne")),
        "stock"
    );
    assert_eq!(
        building(&g, true),
        Some(crate::name!("ancestral_hall")),
        "science tier one"
    );
    let mut hall = g.clone();
    hall.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("ancestral_hall"));
    assert_eq!(
        building(&hall, true),
        Some(crate::name!("intelligence_agency")),
        "then the Agency"
    );
    let mut throne = g.clone();
    throne
        .cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("warlords_throne"));
    assert_eq!(
        building(&throne, true),
        Some(crate::name!("intelligence_agency")),
        "a Throne already standing goes straight to the Agency"
    );
    assert_eq!(
        BasicAi::plaza_building_item(&g, 0, city),
        BasicAi::plaza_building_item_for(&g, 0, city, false),
        "the stock entry point is the flag-off list"
    );
}

#[test]
fn city_state_governors_repair_pillaged_districts_before_new_production() {
    let mut g = Game::new_full(1, 24, 16, 91_769, 120, 0, false);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| g.units[unit].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    g.players[0].is_minor = true;
    g.players[0].techs.insert(crate::name!("writing"));
    let city = g.player_city_ids(0)[0];
    let center = g.cities[&city].pos;
    let campus = g.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != center)
        .unwrap();
    {
        let tile = g.map.tiles.get_mut(&campus).unwrap();
        tile.district = Some(crate::name!("campus"));
        tile.pillaged = true;
    }
    let developed = g.cities.get_mut(&city).unwrap();
    developed.districts.insert(crate::name!("campus"), campus);
    developed.buildings.push(crate::name!("library"));
    developed.pillaged_buildings.insert(crate::name!("library"));

    let mut ai = BasicAi::new();
    ai.minor = true;
    let choice = ai
        .pick_item(&g, 0, city, 1, 0, 1, 0, 0, 3, 2, 1)
        .expect("a damaged city-state has a repair to queue");
    assert!(matches!(
        choice,
        Item::Repair { repair, pos } if repair == "district" && pos == campus
    ));
}

#[test]
fn city_states_research_and_build_ancient_walls_first() {
    let mut g = Game::new_full(1, 24, 16, 91_772, 120, 0, false);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| g.units[unit].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = g.player_city_ids(0)[0];
    g.players[0].is_minor = true;
    g.players[0].civ = "Geneva".to_string();
    let mut ai = BasicAi::new();
    ai.minor = true;

    ai.minor_research(&mut g, 0);
    assert_eq!(g.players[0].research.as_deref(), Some("mining"));
    g.players[0].research = None;
    g.players[0].techs.insert(crate::name!("mining"));
    ai.minor_research(&mut g, 0);
    assert_eq!(g.players[0].research.as_deref(), Some("masonry"));

    g.players[0].research = None;
    g.players[0].techs.insert(crate::name!("masonry"));
    let item = ai.pick_item(&g, 0, city, 1, 0, 10, 0, 0, 3, 2, 1).unwrap();
    assert_eq!(
        item,
        Item::Building {
            building: crate::name!("walls")
        }
    );
}

#[test]
fn every_city_state_type_prioritizes_its_matching_district() {
    for (civilization, family) in [
        ("Geneva", "campus"),
        ("Mohenjo-Daro", "theater_square"),
        ("Yerevan", "holy_site"),
        ("Kabul", "encampment"),
        ("Auckland", "industrial_zone"),
        ("Zanzibar", "commercial_hub"),
    ] {
        let mut g = Game::new_full(1, 24, 16, 91_800, 120, 0, false);
        let settler = g
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| g.units[unit].kind == "settler")
            .unwrap();
        g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        let city = g.player_city_ids(0)[0];
        g.players[0].is_minor = true;
        g.players[0].civ = civilization.to_string();
        g.players[0].techs = g.rules.techs.keys().cloned().collect();
        g.players[0].civics = g.rules.civics.keys().cloned().collect();
        let center = g.cities[&city].pos;
        for position in g.wdisk(center, 3) {
            if g.map.tiles[&position].owner_city.is_none() {
                g.map.tiles.get_mut(&position).unwrap().owner_city = Some(city);
                g.cities.get_mut(&city).unwrap().owned_tiles.push(position);
            }
        }
        {
            let state = g.cities.get_mut(&city).unwrap();
            state.pop = 10;
            state.buildings.extend([
                crate::name!("walls"),
                crate::name!("medieval_walls"),
                crate::name!("renaissance_walls"),
                crate::name!("monument"),
            ]);
            state.wall_hp = 300;
        }
        let mut ai = BasicAi::new();
        ai.minor = true;
        let item = ai
            .pick_item(&g, 0, city, 1, 0, 10, 10, 10, 99, 99, 99)
            .unwrap_or_else(|| panic!("{civilization} found no specialty district"));
        assert!(
            matches!(&item, Item::District { district, .. }
                    if g.district_family(district) == family),
            "{civilization} selected {item:?} instead of {family}"
        );
    }
}

#[test]
fn fully_developed_city_states_can_idle() {
    let mut g = Game::new_full(1, 24, 16, 91_770, 120, 0, false);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| g.units[unit].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = g.player_city_ids(0)[0];
    let project_pos = g.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != g.cities[&city].pos)
        .unwrap();
    let techs = g.rules.techs.keys().cloned().collect();
    let civics = g.rules.civics.keys().cloned().collect();
    let buildings = g.rules.buildings.keys().cloned().collect();
    let districts: Vec<Name> = g.rules.districts.keys().cloned().collect();
    let wonders: Vec<Name> = g.rules.wonders.keys().cloned().collect();
    g.players[0].is_minor = true;
    g.players[0].techs = techs;
    g.players[0].civics = civics;
    {
        let tile = g.map.tiles.get_mut(&project_pos).unwrap();
        tile.district = Some(crate::name!("campus"));
        tile.pillaged = false;
    }
    {
        let developed = g.cities.get_mut(&city).unwrap();
        developed.buildings = buildings;
        for district in districts {
            developed
                .districts
                .insert(Name::new(&district), project_pos);
        }
        for wonder in wonders {
            developed.wonders.insert(Name::new(&wonder), project_pos);
        }
    }
    let mut ai = BasicAi::new();
    ai.minor = true;

    assert_eq!(
        ai.pick_item(&g, 0, city, 1, 0, 10, 10, 10, 99, 99, 99),
        None
    );
}

#[test]
fn repeatable_district_projects_do_not_preempt_basic_infrastructure() {
    let mut g = Game::new_full(1, 24, 16, 91_771, 120, 0, false);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| g.units[unit].kind == "settler")
        .unwrap();
    g.found_city_for(0, g.units[&settler].pos, None);
    let city = g.player_city_ids(0)[0];
    g.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .retain(|building| building != "monument");
    for position in g.nbrs(g.cities[&city].pos) {
        g.map.tiles.get_mut(&position).unwrap().terrain = crate::name!("plains");
    }
    let campus = g.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != g.cities[&city].pos)
        .unwrap();
    g.map.tiles.get_mut(&campus).unwrap().district = Some(crate::name!("campus"));
    g.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(crate::name!("campus"), campus);
    let grants = Item::Project {
        project: crate::name!("campus_research_grants"),
    };
    assert!(g.can_produce(0, city, &grants));

    let item = BasicAi::new()
        .pick_item(&g, 0, city, 8, 2, 20, 10, 0, 20, 10, 10)
        .expect("developing city has a production choice");
    assert!(
        matches!(item, Item::Building { ref building } if building == "monument"),
        "repeatable project displaced {item:?}"
    );
}

#[test]
fn developed_city_wonder_fallback_uses_a_legal_placed_item() {
    let mut game = Game::new_full(1, 24, 16, 91_772, 120, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = game.player_city_ids(0)[0];
    let techs: Vec<Name> = game.rules.techs.keys().cloned().collect();
    let civics: Vec<Name> = game.rules.civics.keys().cloned().collect();
    game.players[0].techs.extend(techs);
    game.players[0].civics.extend(civics);
    for position in game.cities[&city].owned_tiles.clone() {
        if position == game.cities[&city].pos {
            continue;
        }
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("desert");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
    }
    assert!(game
        .producible_items(0, city)
        .iter()
        .any(|item| matches!(item, Item::Wonder { wonder, .. } if wonder == "pyramids")));

    let wonder = BasicAi::fallback_wonder(&game, 0, city)
        .expect("the developed city has a placed wonder fallback");
    assert!(matches!(wonder, Item::Wonder { .. }));
    assert!(game.can_produce(0, city, &wonder));

    // The live mirror records `build_no_plot` as an exact Wonder block.
    // After one such response, generic fallback production must return to
    // its non-Wonder choices instead of trying the next catalog entry.
    std::sync::Arc::make_mut(&mut game.blocked_wonders)
        .entry(city)
        .or_default()
        .insert(crate::name!("pyramids"));
    assert!(
        BasicAi::fallback_wonder(&game, 0, city).is_none(),
        "a host-refused Wonder must stop the generic fallback from walking the catalogue"
    );
}

/// A settler standing still does not hold the expansion gate shut.
///
/// This is the whole repair: `stranded_settlers` must reach 1 only after
/// `STRANDED_SETTLER_TURNS` consecutive motionless turns, and any real
/// move must reset the streak so a genuinely walking settler keeps the
/// gate closed exactly as before.
/// The brake's own idle count runs without the discount, trips at six
/// turns on one tile, and leaves the discount's twelve-turn stranded count
/// alone. Same board as the stranded test below.
#[test]
fn a_parked_settler_counts_for_the_brake_at_six_turns() {
    let mut game = Game::new_full(
        1,
        24,
        16,
        crate::rng::fixture_seed("STRANDED", 91_774),
        250,
        0,
        false,
    );
    let first = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: first }).unwrap();
    let capital = game.cities[&game.player_city_ids(0)[0]].pos;
    let parked = game
        .map
        .tiles
        .iter()
        .filter(|(_, tile)| game.rules.is_passable(tile) && !game.rules.is_water(tile))
        .map(|(position, _)| *position)
        .find(|position| game.wdist(capital, *position) == 3)
        .unwrap();
    game.spawn_unit("settler", 0, parked);

    let mut ai = BasicAi::new();
    assert!(!ai.settler_backlog_brake, "the brake ships off");
    ai.settler_backlog_brake = true;
    ai.settler_strand_discount = false;
    for turn in 1..=BasicAi::SETTLER_BACKLOG_IDLE_TURNS {
        game.turn = turn;
        ai.refresh_settler_idle(&game, 0);
        assert_eq!(
            ai.parked_settlers(&game, 0, BasicAi::SETTLER_BACKLOG_IDLE_TURNS),
            0,
            "turn {turn} is inside the brake's patience"
        );
    }
    game.turn = BasicAi::SETTLER_BACKLOG_IDLE_TURNS + 1;
    ai.refresh_settler_idle(&game, 0);
    assert_eq!(
        ai.parked_settlers(&game, 0, BasicAi::SETTLER_BACKLOG_IDLE_TURNS),
        1,
        "six turns on one tile is parked"
    );
    assert_eq!(
        ai.stranded_settlers(&game, 0),
        0,
        "the discount is off, so nothing is stranded for it"
    );
}

/// V2 reaches the baseline governor that owns most city queues: a parked
/// Settler cannot stop a three-city growth wave, but it does stop new
/// Settlers after the empire matures to six cities. V1 still stops at
/// three.
#[test]
fn settler_backlog_v2_waits_for_a_mature_baseline_empire() {
    let mut game = Game::new_full(
        1,
        24,
        16,
        crate::rng::fixture_seed("BACKLOG_RESCUE", 91_774),
        250,
        0,
        false,
    );
    let first = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .expect("a starting settler");
    game.apply(0, &Action::FoundCity { unit: first }).unwrap();
    let city = game.player_city_ids(0)[0];
    game.cities.get_mut(&city).expect("capital exists").pop = 5;
    let capital = game.cities[&city].pos;
    let parked = game
        .map
        .tiles
        .iter()
        .filter(|(_, tile)| game.rules.is_passable(tile) && !game.rules.is_water(tile))
        .map(|(position, _)| *position)
        .find(|position| game.wdist(capital, *position) == 3)
        .expect("a parked tile");
    game.spawn_unit("settler", 0, parked);

    let mut ai = BasicAi::new();
    ai.land_grab = true;
    ai.w.city_target = 16.0;
    ai.w.settler_stop_turn = 250.0;
    ai.settler_backlog_brake_2 = true;
    for turn in 1..=BasicAi::SETTLER_BACKLOG_IDLE_TURNS + 1 {
        game.turn = turn;
        ai.refresh_settler_idle(&game, 0);
    }
    let pick =
        |ai: &BasicAi, settlers| ai.pick_item(&game, 0, city, 3, settlers, 99, 99, 99, 99, 99, 99);
    assert!(
        matches!(pick(&ai, 1), Some(Item::Unit { unit }) if unit == "settler"),
        "a parked walker does not stop the three-city growth wave"
    );
    let mature_pick = ai.pick_item(&game, 0, city, 6, 1, 99, 99, 99, 99, 99, 99);
    assert!(
        !matches!(mature_pick, Some(Item::Unit { unit }) if unit == "settler"),
        "the same parked walker stops a mature empire's replacement cascade"
    );
    ai.settler_backlog_brake_2 = false;
    ai.settler_backlog_brake = true;
    assert!(
        !matches!(pick(&ai, 1), Some(Item::Unit { unit }) if unit == "settler"),
        "V1 still closes the whole pipeline"
    );
}

#[test]
fn a_settler_that_stops_walking_stops_blocking_the_next_one() {
    let mut game = Game::new_full(
        1,
        24,
        16,
        crate::rng::fixture_seed("STRANDED", 91_774),
        250,
        0,
        false,
    );
    let first = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: first }).unwrap();
    let capital = game.cities[&game.player_city_ids(0)[0]].pos;
    let parked = game
        .map
        .tiles
        .iter()
        .filter(|(_, tile)| game.rules.is_passable(tile) && !game.rules.is_water(tile))
        .map(|(position, _)| *position)
        .find(|position| game.wdist(capital, *position) == 3)
        .unwrap();
    let settler = game.spawn_unit("settler", 0, parked);

    let mut ai = BasicAi::new();
    ai.settler_strand_discount = true;
    // One motionless turn short of the threshold the settler is still
    // "in flight" and the empire must not order another one.
    // The first sighting only records where the settler is, so the streak
    // reaches the threshold one turn after that: sightings 1..=N leave it
    // at N-1 idle turns, all of them inside the patience window.
    for turn in 1..=BasicAi::STRANDED_SETTLER_TURNS {
        game.turn = turn;
        ai.refresh_settler_idle(&game, 0);
        assert_eq!(
            ai.stranded_settlers(&game, 0),
            0,
            "turn {turn} is inside the patience window"
        );
        assert_eq!(ai.walking_settlers(&game, 0, 1), 1);
    }

    game.turn = BasicAi::STRANDED_SETTLER_TURNS + 1;
    ai.refresh_settler_idle(&game, 0);
    assert_eq!(
        ai.stranded_settlers(&game, 0),
        1,
        "parked long enough to be stranded"
    );
    // The gate below reads exactly this: one settler, none of it walking.
    assert_eq!(ai.walking_settlers(&game, 0, 1), 0);

    // Moving resets the streak: a settler that is genuinely walking must
    // still serialise expansion, which is what the cap is for.
    let stepped =
        game.map
            .tiles
            .iter()
            .map(|(position, _)| *position)
            .find(|position| {
                *position != parked
                    && game.wdist(parked, *position) == 1
                    && game.map.tiles.get(position).is_some_and(|tile| {
                        game.rules.is_passable(tile) && !game.rules.is_water(tile)
                    })
            })
            .unwrap();
    game.units.get_mut(&settler).unwrap().pos = stepped;
    game.turn = BasicAi::STRANDED_SETTLER_TURNS + 2;
    ai.refresh_settler_idle(&game, 0);
    assert_eq!(
        ai.stranded_settlers(&game, 0),
        0,
        "a moving settler is not stranded"
    );
    assert_eq!(ai.walking_settlers(&game, 0, 1), 1);

    // A founded or killed settler must not leave its streak behind for a
    // recycled unit id to inherit.
    game.units.remove(&settler);
    game.turn = BasicAi::STRANDED_SETTLER_TURNS + 3;
    ai.refresh_settler_idle(&game, 0);
    assert_eq!(ai.stranded_settlers(&game, 0), 0);
    assert!(ai.settler_idle.is_empty(), "dead settlers are pruned");
}

/// The early Cities route: a two-city empire with a Settler walking and pop
/// to spare starts a second one only under `parallel_settlers`, and never
/// past the city target.
#[test]
fn parallel_settlers_let_a_second_settler_start_while_one_walks() {
    let mut game = Game::new_full(
        1,
        24,
        16,
        crate::rng::fixture_seed("PIPELINE", 91_777),
        250,
        0,
        false,
    );
    let first = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: first }).unwrap();
    let capital = game.player_city_ids(0)[0];
    game.cities.get_mut(&capital).unwrap().pop = 5;
    game.turn = 30;
    // A scout on the board and a siege arm in the counts, so neither the
    // recon nor the siege capability gap pre-empts the settler gate.
    let home = game.cities[&capital].pos;
    game.spawn_unit("scout", 0, home);
    game.spawn_unit("warrior", 0, home);
    let ask = |ai: &BasicAi| ai.pick_item(&game, 0, capital, 2, 1, 6, 2, 1, 20, 10, 10);

    // The league genome the live seat plays wants ~8 cities; the default
    // weights want 4, which would cap the pipeline before it is tested.
    let mut stock = BasicAi::new();
    stock.w.city_target = 8.0;
    assert!(!stock.parallel_settlers, "off unless the bridge asks");
    assert!(
        !matches!(ask(&stock), Some(Item::Unit { unit }) if unit == "settler"),
        "stock: a settler is already in flight"
    );

    let mut treated = BasicAi::new();
    treated.w.city_target = 8.0;
    treated.parallel_settlers = true;
    assert!(
        matches!(ask(&treated), Some(Item::Unit { unit }) if unit == "settler"),
        "the live pipeline lets the capital start the next settler while one walks: {:?}",
        ask(&treated)
    );
    // Still capped by the city target: two cities plus one walker plus this
    // one must stay below it.
    treated.w.city_target = 4.0;
    assert!(
        !matches!(ask(&treated), Some(Item::Unit { unit }) if unit == "settler"),
        "the target is the hard cap"
    );
}

/// Civilization VI starts a Settler at population 2; the live genome's
/// `settler_min_pop` 2.456 waited for population 3 and held a food-poor
/// capital at one city for forty turns (civvis-20260816T021044Z). See
/// `host_settler_pop`.
#[test]
fn the_live_seat_builds_a_settler_at_the_hosts_population_floor() {
    let mut game = Game::new_full(
        1,
        24,
        16,
        crate::rng::fixture_seed("HOSTPOP", 91_778),
        250,
        0,
        false,
    );
    let first = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: first }).unwrap();
    let capital = game.player_city_ids(0)[0];
    game.cities.get_mut(&capital).unwrap().pop = 2;
    game.turn = 20;
    let home = game.cities[&capital].pos;
    game.spawn_unit("scout", 0, home);
    game.spawn_unit("warrior", 0, home);
    // No settler in flight, room to expand, inside the window: only the
    // population gate can refuse.
    let ask = |game: &Game, ai: &BasicAi| ai.pick_item(game, 0, capital, 2, 0, 6, 2, 1, 20, 10, 10);

    // The genome's figure, as the live seat plays it.
    let mut stock = BasicAi::new();
    stock.w.city_target = 8.0;
    stock.w.settler_min_pop = 2.456;
    assert!(!stock.host_settler_pop, "off unless the bridge asks");
    assert!(
        !matches!(ask(&game, &stock), Some(Item::Unit { unit }) if unit == "settler"),
        "stock: population 2 is below the genome's 2.456"
    );

    let mut treated = BasicAi::new();
    treated.w.city_target = 8.0;
    treated.w.settler_min_pop = 2.456;
    treated.enable_host_settler_pop();
    assert!(
        matches!(ask(&game, &treated), Some(Item::Unit { unit }) if unit == "settler"),
        "the live seat starts the Settler at the host's floor of 2: {:?}",
        ask(&game, &treated)
    );
    // Population 1 still cannot: the host's floor is a floor.
    game.cities.get_mut(&capital).unwrap().pop = 1;
    assert!(
        !matches!(ask(&game, &treated), Some(Item::Unit { unit }) if unit == "settler"),
        "population 1 cannot start a Settler on the host either"
    );
    // A genome that already asks for less than the host's floor keeps its
    // own figure: the floor never raises the bar.
    game.cities.get_mut(&capital).unwrap().pop = 2;
    treated.w.settler_min_pop = 1.5;
    assert!(matches!(ask(&game, &treated), Some(Item::Unit { unit }) if unit == "settler"));
}

/// The capital-settler-after-completion gene acts only after the capital
/// has finished its queued work. It bypasses the ordinary one-walker
/// serialisation, but does not interrupt a current build or outgrow the
/// normal city target.
#[test]
fn capital_settler_after_completion_waits_for_an_empty_capital_queue() {
    let mut game = Game::new_full(
        1,
        24,
        16,
        crate::rng::fixture_seed("CAPITALSETTLER", 91_779),
        250,
        0,
        false,
    );
    let founding = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: founding })
        .unwrap();
    let capital = game.player_city_ids(0)[0];
    let home = game.cities[&capital].pos;
    game.cities.get_mut(&capital).unwrap().pop = 2;
    game.turn = 20;
    // One walker fills the ordinary one-at-a-time pipeline. The Scout and
    // Warrior keep unrelated early capability gaps out of this fixture.
    game.spawn_unit("settler", 0, home);
    game.spawn_unit("scout", 0, home);
    game.spawn_unit("warrior", 0, home);

    let settler = Item::Unit {
        unit: crate::name!("settler"),
    };
    let builder = Item::Unit {
        unit: crate::name!("builder"),
    };
    let ask = |game: &Game, ai: &BasicAi| ai.pick_item(game, 0, capital, 1, 1, 6, 2, 1, 20, 10, 10);

    let mut stock = BasicAi::new();
    stock.book_pos = 1;
    stock.w.city_target = 4.0;
    assert!(stock.has_practical_settle_site(&game, 0));
    assert!(
        !matches!(ask(&game, &stock), Some(Item::Unit { unit }) if unit == "settler"),
        "the ordinary picker leaves the second walker to its one-at-a-time gate"
    );

    let mut treated = BasicAi::new();
    treated.book_pos = 1;
    treated.w.city_target = 4.0;
    treated.enable_capital_settler_after_completion();
    assert!(treated.capital_settler_after_completion);

    // A queued Builder is left alone, even after the capital reaches the
    // population floor: this is a next-build reservation, not preemption.
    game.cities.get_mut(&capital).unwrap().queue = vec![builder.clone()];
    assert!(
        !matches!(ask(&game, &treated), Some(Item::Unit { unit }) if unit == "settler"),
        "the reservation requires a fully empty capital queue"
    );
    treated.cities(&mut game, 0);
    assert_eq!(game.cities[&capital].queue.first(), Some(&builder));

    // Once the current item has completed and the queue is empty, the
    // legal population-two capital starts the next Settler immediately.
    game.cities.get_mut(&capital).unwrap().queue.clear();
    treated.cities(&mut game, 0);
    assert_eq!(game.cities[&capital].queue.first(), Some(&settler));
    assert_eq!(
        treated.book_pos, 1,
        "the reservation takes one empty queue without consuming the rest of the opening book"
    );

    // The screenable wrapper remains off in every normal constructor until
    // explicitly enabled; the registry invokes its toggle pair.
    let mut strategic = AdvancedAi::new();
    assert!(!strategic.capital_settler_after_completion);
    assert!(!AdvancedAi::legacy().capital_settler_after_completion);
    strategic.enable_capital_settler_after_completion();
    assert!(strategic.capital_settler_after_completion);
    strategic.disable_capital_settler_after_completion();
    assert!(!strategic.capital_settler_after_completion);
}

/// The land grab's early Cities route: the pipeline is two walkers from
/// the first city and one more per three cities, bounded by the seats
/// still short; the window stays open until a settler can no longer
/// repay, not until the genome's `settler_stop_turn`. Off unless the
/// bridge asks. See `land_grab`.
#[test]
fn the_land_grab_widens_the_pipeline_and_outlives_the_genomes_stop_turn() {
    let mut game = Game::new_full(
        1,
        24,
        16,
        crate::rng::fixture_seed("LANDGRAB", 91_779),
        250,
        0,
        false,
    );
    let first = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: first }).unwrap();
    let capital = game.player_city_ids(0)[0];
    game.cities.get_mut(&capital).unwrap().pop = 5;
    game.turn = 30;
    let home = game.cities[&capital].pos;
    game.spawn_unit("scout", 0, home);
    game.spawn_unit("warrior", 0, home);
    let ask = |game: &Game, ai: &BasicAi, cities: usize, settlers: usize| {
        ai.pick_item(game, 0, capital, cities, settlers, 6, 2, 1, 20, 10, 10)
    };
    let is_settler =
        |item: Option<Item>| matches!(item, Some(Item::Unit { unit }) if unit == "settler");

    // The wide target the bridge threads through `delegated_cities`.
    let mut treated = BasicAi::new();
    treated.w.city_target = 12.0;
    assert!(!treated.land_grab, "off unless the bridge asks");
    assert!(
        !is_settler(ask(&game, &treated, 1, 1)),
        "stock: one city, one walker — a settler is already in flight"
    );
    treated.enable_land_grab();
    assert!(
        is_settler(ask(&game, &treated, 1, 1)),
        "the land grab lets the capital queue its next settler while the first walks"
    );
    assert!(
        !is_settler(ask(&game, &treated, 1, 2)),
        "two walkers from one city is the base width"
    );
    assert!(
        is_settler(ask(&game, &treated, 3, 2)),
        "three cities: a third walker"
    );
    assert!(!is_settler(ask(&game, &treated, 3, 3)));
    assert!(
        is_settler(ask(&game, &treated, 6, 3)),
        "six cities: a fourth"
    );
    // Never more than the seats still short.
    assert!(
        !is_settler(ask(&game, &treated, 11, 1)),
        "one seat short: one walker covers it"
    );
    assert!(
        !is_settler(ask(&game, &treated, 12, 0)),
        "at the target: no settler"
    );

    // The window: the genome says stop at 150; the land grab keeps
    // settling while a settler can still repay before the turn limit.
    treated.w.settler_stop_turn = 150.0;
    game.turn = 200;
    let mut stock = BasicAi::new();
    stock.w.city_target = 12.0;
    stock.w.settler_stop_turn = 150.0;
    assert!(
        !is_settler(ask(&game, &stock, 3, 0)),
        "stock: past settler_stop_turn"
    );
    assert!(
        is_settler(ask(&game, &treated, 3, 0)),
        "the land grab settles at t200 of 250"
    );
    game.turn = 250 - game.standard_duration(LAND_GRAB_SETTLE_HORIZON);
    assert!(
        !is_settler(ask(&game, &treated, 3, 0)),
        "and stops when a settler can no longer repay"
    );
}

/// The queue revalidation keeps a queued Settler while the pipeline
/// `pick_item` would open for it has room. It used to keep one Settler in
/// all, walker or queued, so a capital building the second Settler the
/// land grab had just authorised swapped it for a Monument, then took it
/// back the next turn, every turn (live King `civvis-20261001T000033Z`
/// t27-t30).
#[test]
fn a_queued_second_settler_survives_the_revalidation_inside_the_pipeline() {
    let mut game = Game::new_full(
        1,
        24,
        16,
        crate::rng::fixture_seed("LANDGRAB", 91_779),
        250,
        0,
        false,
    );
    let first = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: first }).unwrap();
    let capital = game.player_city_ids(0)[0];
    game.cities.get_mut(&capital).unwrap().pop = 4;
    let home = game.cities[&capital].pos;
    game.spawn_unit("scout", 0, home);
    game.spawn_unit("warrior", 0, home);
    game.spawn_unit("settler", 0, home);
    game.turn = 30;
    let settler = Item::Unit {
        unit: crate::name!("settler"),
    };
    let queue_settler = |game: &mut Game| {
        game.cities.get_mut(&capital).unwrap().queue.clear();
        game.apply(
            0,
            &Action::Produce {
                city: capital,
                item: settler.clone(),
            },
        )
        .unwrap();
    };
    let mut ai = BasicAi::new();
    ai.w.city_target = 8.0;
    ai.enable_land_grab();
    assert!(
        ai.has_practical_settle_site(&game, 0),
        "the fixture has land"
    );
    queue_settler(&mut game);
    ai.cities(&mut game, 0);
    assert_eq!(
        game.cities[&capital].queue.first(),
        Some(&settler),
        "one walker and one queued Settler fit the land grab's two"
    );

    // Without a widened pipeline the old one-Settler rule still holds.
    let mut single = BasicAi::new();
    single.w.city_target = 8.0;
    queue_settler(&mut game);
    single.cities(&mut game, 0);
    assert_ne!(game.cities[&capital].queue.first(), Some(&settler));
}

/// `rapid-city-expansion-2` widens the pipeline only while the empire
/// trails the opening pace. Once its band closes it must hand the
/// question back to the land grab, not pin the delegated governor to one
/// walker for the rest of the game.
#[test]
fn a_closed_rapid_expansion_band_falls_through_to_the_land_grab_width() {
    let mut game = Game::new_full(
        1,
        24,
        16,
        crate::rng::fixture_seed("LANDGRAB", 91_779),
        250,
        0,
        false,
    );
    let first = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: first }).unwrap();
    let capital = game.player_city_ids(0)[0];
    game.cities.get_mut(&capital).unwrap().pop = 5;
    let home = game.cities[&capital].pos;
    game.spawn_unit("scout", 0, home);
    game.spawn_unit("warrior", 0, home);
    game.turn = 150;
    let ask = |game: &Game, ai: &BasicAi, cities: usize, settlers: usize| {
        ai.pick_item(game, 0, capital, cities, settlers, 6, 2, 1, 20, 10, 10)
    };
    let is_settler =
        |item: Option<Item>| matches!(item, Some(Item::Unit { unit }) if unit == "settler");
    let mut treated = BasicAi::new();
    treated.w.city_target = 10.0;
    treated.enable_land_grab();
    assert!(
        is_settler(ask(&game, &treated, 4, 1)),
        "land grab alone: four cities, one walker, a second may start"
    );
    treated.enable_rapid_city_expansion_2();
    assert_eq!(
        advanced::rapid_city_expansion::pipeline_width(&game, 10, 4, 1),
        None,
        "the opening band has closed"
    );
    assert!(
        is_settler(ask(&game, &treated, 4, 1)),
        "a closed band falls through to the land grab's width"
    );
    assert!(
        !is_settler(ask(&game, &treated, 4, 3)),
        "and the land grab's own width still binds"
    );
}

/// The live land-grab already allows two Settlers from one city, but the
/// scripted opening previously made its next Builder wait for the first
/// walker to found. Keep the book for ordinary controllers and unsafe
/// cases, while letting the live opening fill its authorized second slot.
#[test]
fn land_grab_fills_the_opening_settler_pipeline_before_book_filler() {
    let board = || {
        let mut game = Game::new_full(
            2,
            24,
            16,
            crate::rng::fixture_seed("OPENINGPIPE", 91_780),
            250,
            0,
            false,
        );
        let founding = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(0, &Action::FoundCity { unit: founding })
            .unwrap();
        let capital = game.player_city_ids(0)[0];
        let home = game.cities[&capital].pos;
        game.cities.get_mut(&capital).unwrap().pop = 2;
        game.turn = 14;
        // One Settler is already travelling; the Warrior and Scout keep
        // the ordinary governor's military and recon checks satisfied.
        game.spawn_unit("settler", 0, home);
        game.spawn_unit("warrior", 0, home);
        game.spawn_unit("scout", 0, home);
        game
    };
    let capital = |game: &Game| game.player_city_ids(0)[0];
    let head = |game: &Game| game.cities[&capital(game)].queue.first().cloned();
    let builder = Item::Unit {
        unit: crate::name!("builder"),
    };
    let settler = Item::Unit {
        unit: crate::name!("settler"),
    };
    let opening = |ai: &mut BasicAi| {
        // The next original book slot is a Builder. A high city target
        // keeps the live two-Settler pipeline materially open.
        ai.book_pos = 2;
        ai.w.open2 = 2.0;
        ai.w.city_target = 8.0;
    };

    let mut stock = BasicAi::new();
    opening(&mut stock);
    let mut ordinary = board();
    stock.cities(&mut ordinary, 0);
    assert_eq!(
        head(&ordinary),
        Some(builder.clone()),
        "without the live land-grab policy, the opening book stays unchanged"
    );

    let mut live = BasicAi::new();
    opening(&mut live);
    live.enable_land_grab();
    let mut treated = board();
    live.cities(&mut treated, 0);
    assert_eq!(
        head(&treated),
        Some(settler),
        "the first walking Settler opens the already-authorized second slot"
    );
    assert_eq!(
        live.book_pos, 2,
        "the deferred Builder remains the next book slot"
    );

    // The bypass does not force a Settler below the host's legal
    // population floor: in that case the original book continues.
    let mut low_pop = board();
    low_pop.cities.get_mut(&capital(&low_pop)).unwrap().pop = 1;
    let mut guarded = BasicAi::new();
    opening(&mut guarded);
    guarded.enable_land_grab();
    guarded.enable_host_settler_pop();
    guarded.cities(&mut low_pop, 0);
    assert_eq!(
        head(&low_pop),
        Some(builder.clone()),
        "the normal Settler population safety gate remains authoritative"
    );

    // An opening at war never jumps ahead of its defensive book slot.
    let mut wartime = board();
    wartime.record_contact(0, 1);
    wartime.current = 0;
    wartime
        .apply(0, &Action::DeclareWar { player: 1 })
        .expect("the fixture majors can declare war");
    let mut defended = BasicAi::new();
    opening(&mut defended);
    defended.enable_land_grab();
    defended.cities(&mut wartime, 0);
    assert_eq!(
        head(&wartime),
        Some(builder),
        "a major war keeps the defensive opening book in control"
    );
}

/// A scout the host will not move gives up its target: civvis-20260816T040537Z
/// ordered its only Scout at one plot for 34 turns and revealed nothing.
/// See `explore_dead_targets`.
#[test]
fn a_scout_the_host_will_not_move_gives_up_its_exploration_target() {
    let mut game = Game::new_full(
        1,
        24,
        16,
        crate::rng::fixture_seed("DEADGOAL", 91_779),
        250,
        0,
        false,
    );
    let first = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: first }).unwrap();
    let home = game.cities[&game.player_city_ids(0)[0]].pos;
    let scout = game.spawn_unit("scout", 0, home);
    game.turn = 10;

    let mut ai = BasicAi::new();
    ai.enable_explore_dead_targets();
    let goal = ai
        .exploration_goal(&game, 0, scout, false)
        .expect("a fresh map has unexplored ground to aim at");
    // The host "accepts" the order and never moves the unit: model that by
    // leaving it no movement, so the step fails and it stands still.
    for turn in 0..EXPLORE_STUCK_TURNS {
        game.units.get_mut(&scout).unwrap().moves_left = 0.0;
        game.turn = 10 + turn;
        let _ = ai.explore_step(&mut game, 0, scout);
        assert_eq!(game.units[&scout].pos, home, "the unit stands where it was");
        assert!(
            ai.explore_dead
                .borrow()
                .get(&scout)
                .is_none_or(|dead| dead.is_empty()),
            "nothing is written off before {EXPLORE_STUCK_TURNS} unmoved turns"
        );
    }
    game.units.get_mut(&scout).unwrap().moves_left = 0.0;
    game.turn = 10 + EXPLORE_STUCK_TURNS;
    let _ = ai.explore_step(&mut game, 0, scout);
    let dead = ai.explore_dead.borrow();
    let retired = dead.get(&scout).expect("the goal is written off");
    assert!(
        retired.contains_key(&goal),
        "the unreachable goal itself is retired"
    );
    assert!(
        game.nbrs(goal).iter().all(|n| retired.contains_key(n)),
        "and its ring with it"
    );
    assert_eq!(
        retired[&goal],
        game.turn + EXPLORE_DEAD_TARGET_TURNS,
        "for {EXPLORE_DEAD_TARGET_TURNS} turns"
    );
    drop(dead);
    // The next goal is somewhere else.
    let next = ai.exploration_goal(&game, 0, scout, false);
    assert_ne!(next, Some(goal), "the unit aims elsewhere: {next:?}");
    // And once the retirement lapses the ground is a candidate again.
    game.turn += EXPLORE_DEAD_TARGET_TURNS + 1;
    let later = ai.exploration_goal(&game, 0, scout, false);
    assert!(later.is_some());

    // Without the bridge flag nothing is ever written off: the native
    // engine moves what it accepts, and the frozen anchor keeps its goal.
    let plain = BasicAi::new();
    assert!(!plain.explore_dead_targets);
    game.turn = 10;
    for turn in 0..=EXPLORE_STUCK_TURNS {
        game.units.get_mut(&scout).unwrap().moves_left = 0.0;
        game.turn = 10 + turn;
        let _ = plain.explore_step(&mut game, 0, scout);
    }
    assert!(plain.explore_dead.borrow().is_empty());
}

/// See `campus_before_harbor`: the first Campus is offered once Writing
/// is in, never a second one, and never in a city that would take longer
/// than `FIRST_CAMPUS_MAX_TURNS`.
#[test]
fn the_first_campus_is_offered_once_writing_is_in() {
    let mut game = Game::new_full(
        1,
        24,
        16,
        crate::rng::fixture_seed("FIRSTCAMPUS", 91_801),
        250,
        0,
        false,
    );
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let cid = game.player_city_ids(0)[0];
    game.cities.get_mut(&cid).unwrap().pop = 3;
    assert_eq!(
        BasicAi::first_campus_item(&game, 0, cid),
        None,
        "no Campus before Writing"
    );
    game.players[0].techs.insert(crate::name!("writing"));
    std::sync::Arc::make_mut(&mut game.observed_city_yield_adjustments).insert(
        cid,
        crate::rules::Yields {
            production: 10.0,
            ..Default::default()
        },
    );
    let Some(Item::District { district, .. }) = BasicAi::first_campus_item(&game, 0, cid) else {
        panic!("with Writing a producing city is offered its Campus");
    };
    assert_eq!(game.district_family(district), "campus");

    std::sync::Arc::make_mut(&mut game.observed_city_yield_adjustments).insert(
        cid,
        crate::rules::Yields {
            // The adjustment is added to the modelled yield; this drives
            // it to the 0.5 floor the gate divides by.
            production: -100.0,
            ..Default::default()
        },
    );
    assert_eq!(
        BasicAi::first_campus_item(&game, 0, cid),
        None,
        "a city that would take longer than the cap is not asked"
    );
}

/// See `capital_campus_first`: a three-city capital with Writing starts
/// its Campus where the stock governor starts something else, and a
/// two-city capital keeps the stock choice.
#[test]
fn the_capital_opens_its_campus_before_the_next_settler() {
    let mut game = Game::new_full(
        1,
        24,
        16,
        crate::rng::fixture_seed("CAPITALCAMPUS", 91_802),
        250,
        0,
        false,
    );
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let cid = game.player_city_ids(0)[0];
    assert!(game.cities[&cid].is_capital);
    game.cities.get_mut(&cid).unwrap().pop = 3;
    game.players[0].techs.insert(crate::name!("writing"));
    std::sync::Arc::make_mut(&mut game.observed_city_yield_adjustments).insert(
        cid,
        crate::rules::Yields {
            production: 10.0,
            ..Default::default()
        },
    );
    let pick = |first: bool, n_cities: usize| {
        let mut ai = BasicAi::new();
        ai.capital_campus_first = first;
        ai.pick_item(&game, 0, cid, n_cities, 0, 3, 1, 0, 6, 3, 3)
    };
    let is_campus = |item: &Option<Item>| {
        matches!(item, Some(Item::District { district, .. })
                if game.district_family(*district) == "campus")
    };
    let stock = pick(false, 3);
    assert!(
        !is_campus(&stock),
        "the fixture's stock pick is not the Campus: {stock:?}"
    );
    assert!(
        is_campus(&pick(true, 3)),
        "a three-city capital opens its Campus"
    );
    assert_eq!(
        pick(true, 2),
        pick(false, 2),
        "two cities keep the stock choice"
    );
}

/// One founded capital with ten Production a turn, the shared fixture of
/// the `monument_first` and `district_buildings_first` tests below.
fn founded_capital_fixture(tag: &str, seed: u64) -> (Game, u32) {
    let mut game = Game::new_full(
        1,
        24,
        16,
        crate::rng::fixture_seed(tag, seed),
        250,
        0,
        false,
    );
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let cid = game.player_city_ids(0)[0];
    std::sync::Arc::make_mut(&mut game.observed_city_yield_adjustments).insert(
        cid,
        crate::rules::Yields {
            production: 10.0,
            ..Default::default()
        },
    );
    (game, cid)
}

/// See `front_weighted_floor`: the city far from the campaign's target
/// leaves the floor's unit to an equally productive city beside it, which
/// keeps it; a slow colony beside the front does not take it.
#[test]
fn the_floor_builds_beside_the_front() {
    let (mut game, capital) = founded_capital_fixture("FRONTFLOOR", 91_871);
    let home = game.cities[&capital].pos;
    let candidates: Vec<Pos> = game
        .map
        .tiles
        .keys()
        .copied()
        .filter(|pos| game.wdist(home, *pos) >= 9 && game.wdist(home, *pos) <= 12)
        .collect();
    let far_site = candidates
        .into_iter()
        .find(|pos| {
            let settler = game.spawn_test_unit("settler", 0, *pos);
            let ok = game.can_found_city(settler);
            game.remove_unit(settler);
            ok
        })
        .expect("a foundable site nine to twelve tiles out");
    let settler = game.spawn_test_unit("settler", 0, far_site);
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let front = game.city_at(far_site).expect("the second city");
    let objective = game
        .nbrs(far_site)
        .into_iter()
        .find(|pos| game.wdist(*pos, home) > game.wdist(far_site, home))
        .unwrap_or(far_site);
    assert!(game.can_produce(
        0,
        front,
        &Item::Unit {
            unit: crate::name!("warrior")
        }
    ));
    // A pop-1 colony is too slow to beat the capital's build and walk.
    assert!(
        !BasicAi::another_city_arrives_sooner(&game, 0, capital, "warrior", objective),
        "a slow city beside the front does not take the floor from a fast capital"
    );
    std::sync::Arc::make_mut(&mut game.observed_city_yield_adjustments).insert(
        front,
        crate::rules::Yields {
            production: 10.0,
            ..Default::default()
        },
    );
    assert!(
        BasicAi::another_city_arrives_sooner(&game, 0, capital, "warrior", objective),
        "the capital leaves the warrior to the city beside the front"
    );
    assert!(
        !BasicAi::another_city_arrives_sooner(&game, 0, front, "warrior", objective),
        "the city beside the front keeps it"
    );
    // Version 2: the city beside the front is a front city, the capital
    // nine to twelve tiles back is not.
    assert!(BasicAi::is_front_city_for(
        &game, 0, front, "warrior", objective
    ));
    assert!(!BasicAi::is_front_city_for(
        &game, 0, capital, "warrior", objective
    ));
    let mut second = BasicAi::new();
    second.front_weighted_floor_2 = true;
    second.front_objective = Some(objective);
    let pick = second.pick_item(&game, 0, front, 2, 1, 3, 1, 0, 0, 0, 0);
    assert!(
        matches!(&pick, Some(Item::Unit { unit }) if game.rules.units[unit].class == "military"),
        "the front city builds the floor's unit: {pick:?}"
    );
}

/// See `granary_before_the_army`: a city at its housing builds its Granary
/// where the stock governor fills the military floor.
#[test]
fn a_housing_bound_city_builds_its_granary_before_the_army() {
    let (mut game, cid) = founded_capital_fixture("GRANARYFIRST", 91_861);
    game.players[0].gold = 500.0;
    game.players[0].gold_per_turn = 5.0;
    game.players[0].techs.insert(crate::name!("pottery"));
    let housing = game.city_housing(&game.cities[&cid]);
    game.cities.get_mut(&cid).unwrap().pop = housing.floor() as i32;
    let granary = BasicAi::housing_reserve_item(&game, 0, cid);
    assert!(
        matches!(&granary, Some(Item::Building { building }) if *building == "granary"),
        "the fixture is housing-bound: {granary:?}"
    );
    let pick = |on: bool, settlers: usize| {
        let mut ai = BasicAi::new();
        ai.granary_before_the_army = on;
        ai.pick_item(&game, 0, cid, 3, settlers, 3, 1, 0, 0, 0, 0)
    };
    let stock = pick(false, 1);
    assert_ne!(
        stock, granary,
        "the fixture's stock pick fills the military floor"
    );
    assert_eq!(pick(true, 1), granary, "the Granary comes first");
    // Version 2 takes the same Granary from behind the Campus step, and
    // yields to a due Settler.
    let second = |settlers: usize| {
        let mut ai = BasicAi::new();
        ai.granary_before_the_army_2 = true;
        ai.pick_item(&game, 0, cid, 3, settlers, 3, 1, 0, 0, 0, 0)
    };
    assert_eq!(second(1), granary, "version 2 takes the floor's build");
    assert!(
        BasicAi::new().settler_due(&game, 0, cid, 3, 0),
        "the fixture is due a Settler"
    );
    assert_ne!(second(0), granary, "a due Settler comes first");
}

/// The Emperor shape of `housing_bound_city_builds_its_granary`: a capital
/// at its housing with Pottery and no Granary, a Settler due, the live
/// lineage's Monument, Campus and Builder steps armed, and a lent war
/// target of two units a city over an empty army.
fn emperor_housing_bound_capital(tag: &str, seed: u64) -> (Game, u32) {
    let (mut game, cid) = founded_capital_fixture(tag, seed);
    game.difficulty = "emperor".to_string();
    game.players[0].gold = 500.0;
    game.players[0].gold_per_turn = 5.0;
    for tech in ["pottery", "writing", "mining"] {
        game.players[0].techs.insert(Name::new(tech));
    }
    let housing = game.city_housing(&game.cities[&cid]);
    game.cities.get_mut(&cid).unwrap().pop = housing.floor() as i32;
    (game, cid)
}

fn emperor_lineage_governor(gene: bool) -> BasicAi {
    let mut ai = BasicAi::new();
    ai.monument_first = true;
    ai.campus_before_the_army_2 = true;
    ai.builder_before_the_army_3 = true;
    ai.granary_before_the_army_2 = true;
    ai.housing_reserve = true;
    ai.w.mil_per_city = 2.0;
    ai.lent_military_floor_base = Some(1.0);
    ai.housing_bound_city_builds_its_granary = gene;
    ai
}

/// See `housing_bound_city_builds_its_granary`: on the Emperor shape the
/// stock governor spends the housing-bound capital on its lent army or
/// its due Settler; under the gene the Granary is the next build, Settler
/// due or not.
#[test]
fn a_housing_bound_city_builds_its_granary_first_under_the_gene() {
    let (game, cid) = emperor_housing_bound_capital("HOUSINGGRANARY", 91_901);
    let granary = Some(Item::Building {
        building: crate::name!("granary"),
    });
    assert!(
        BasicAi::new().settler_due(&game, 0, cid, 3, 0),
        "the fixture is due a Settler"
    );
    for settlers in [0, 1] {
        let pick = |gene: bool| {
            emperor_lineage_governor(gene).pick_item(&game, 0, cid, 3, settlers, 1, 1, 0, 0, 0, 0)
        };
        let stock = pick(false);
        assert!(stock.is_some(), "the stock governor builds something");
        assert_ne!(
            stock, granary,
            "{settlers} settlers: the stock pick is not the Granary"
        );
        assert_eq!(
            pick(true),
            granary,
            "{settlers} settlers: the Granary comes first"
        );
    }
    // A city with room to grow keeps the stock order.
    let mut roomy = game.clone();
    roomy.cities.get_mut(&cid).unwrap().pop = 1;
    assert!(
        1.0 + 1.0 < roomy.city_housing(&roomy.cities[&cid]),
        "fixture: room to grow"
    );
    assert_eq!(
        emperor_lineage_governor(true).pick_item(&roomy, 0, cid, 3, 1, 1, 1, 0, 0, 0, 0),
        emperor_lineage_governor(false).pick_item(&roomy, 0, cid, 3, 1, 1, 1, 0, 0, 0, 0),
        "room to grow: the stock pick"
    );
}

/// The `housing-cap-builds-the-aqueduct` shape: the Emperor capital at
/// its housing with its Granary built, Engineering researched and a river
/// edge beside a city-centre neighbour, so its Aqueduct is legal.
fn housing_capped_capital_with_an_aqueduct_site(tag: &str, seed: u64) -> (Game, u32, Item) {
    let (mut game, cid) = emperor_housing_bound_capital(tag, seed);
    for tech in ["masonry", "bronze_working", "engineering"] {
        game.players[0].techs.insert(Name::new(tech));
    }
    game.cities
        .get_mut(&cid)
        .unwrap()
        .buildings
        .push(crate::name!("granary"));
    let center = game.cities[&cid].pos;
    let site = game
        .nbrs(center)
        .into_iter()
        .find(|pos| {
            game.map
                .tiles
                .get(pos)
                .is_some_and(|tile| !game.rules.is_water(tile))
                && game
                    .nbrs(*pos)
                    .into_iter()
                    .any(|other| other != center && game.map.tiles.get(&other).is_some())
        })
        .expect("a land neighbour of the centre");
    let source = game
        .nbrs(site)
        .into_iter()
        .find(|pos| *pos != center && game.map.tiles.get(pos).is_some())
        .expect("a river partner");
    assert!(
        game.map.set_river_edge(site, source, true),
        "river edge set"
    );
    let housing = game.city_housing(&game.cities[&cid]);
    game.cities.get_mut(&cid).unwrap().pop = housing.floor() as i32;
    let aqueduct = BasicAi::housing_reserve_item(&game, 0, cid)
        .expect("fixture: the housing reserve names the Aqueduct");
    assert!(
        matches!(&aqueduct, Item::District { district, .. } if game.district_family(*district) == "aqueduct"),
        "fixture: the reserve is the Aqueduct, not a Granary: {aqueduct:?}"
    );
    (game, cid, aqueduct)
}

/// See `housing_cap_builds_the_aqueduct`: the stock governor spends the
/// housing-bound capital whose Granary stands on something else; under
/// the gene its Aqueduct is the next build. A city with room to grow and
/// a city under the population floor keep the stock pick.
#[test]
fn a_housing_capped_city_builds_its_aqueduct_under_the_gene() {
    let (game, cid, aqueduct) =
        housing_capped_capital_with_an_aqueduct_site("HOUSINGAQUEDUCT", 91_921);
    let pick = |game: &Game, gene: bool| {
        let mut ai = emperor_lineage_governor(true);
        ai.granary_before_the_army_2 = false;
        ai.housing_cap_builds_the_aqueduct = gene;
        ai.pick_item(game, 0, cid, 3, 1, 1, 1, 0, 0, 0, 0)
    };
    let stock = pick(&game, false);
    assert!(stock.is_some(), "the stock governor builds something");
    assert_ne!(
        stock.as_ref(),
        Some(&aqueduct),
        "the stock pick is not the Aqueduct"
    );
    assert_eq!(
        pick(&game, true),
        Some(aqueduct.clone()),
        "the Aqueduct comes first"
    );

    let mut roomy = game.clone();
    roomy.cities.get_mut(&cid).unwrap().pop = 1;
    assert_eq!(
        pick(&roomy, true),
        pick(&roomy, false),
        "room to grow: the stock pick"
    );

    // Still at its housing, but too small for the district.
    let mut tiny = game.clone();
    let housing = tiny.city_housing(&tiny.cities[&cid]);
    if housing >= 3.0 {
        tiny.cities.get_mut(&cid).unwrap().pop = 2;
        if 2.0 + 1.0 >= housing {
            assert_eq!(
                pick(&tiny, true),
                pick(&tiny, false),
                "under the population floor"
            );
        }
    }

    // A slow city keeps the stock pick: past the turn cap.
    let mut slow = game.clone();
    std::sync::Arc::make_mut(&mut slow.observed_city_yield_adjustments).insert(
        cid,
        crate::rules::Yields {
            production: 0.1,
            ..Default::default()
        },
    );
    let turns = slow
        .host_production_turns(cid, &aqueduct)
        .unwrap_or_else(|| {
            slow.item_cost_for(0, &aqueduct) / slow.city_yields(cid).production.max(0.5)
        });
    if turns > BasicAi::HOUSING_CAP_AQUEDUCT_MAX_TURNS {
        assert_ne!(
            pick(&slow, true),
            Some(aqueduct.clone()),
            "past the turn cap"
        );
    }
}

/// See `housing_cap_aqueduct_step`: a city the plan names threatened and
/// a city attacked within four turns keep the stock pick; a city still
/// short of its Granary takes the Granary through the granary gene.
#[test]
fn a_threatened_city_keeps_the_stock_pick_under_the_aqueduct_gene() {
    let (game, cid, aqueduct) =
        housing_capped_capital_with_an_aqueduct_site("HOUSINGAQUEDUCTTHREAT", 91_923);
    let governor = |gene: bool| {
        let mut ai = emperor_lineage_governor(true);
        ai.granary_before_the_army_2 = false;
        ai.housing_cap_builds_the_aqueduct = gene;
        ai
    };
    let ask = |game: &Game, ai: &BasicAi| ai.pick_item(game, 0, cid, 3, 1, 1, 1, 0, 0, 0, 0);
    assert_eq!(
        ask(&game, &governor(true)),
        Some(aqueduct.clone()),
        "fixture: the gene fires"
    );

    let mut named = governor(true);
    named.plan_threatened_city = Some(cid);
    let mut named_stock = governor(false);
    named_stock.plan_threatened_city = Some(cid);
    assert_eq!(
        ask(&game, &named),
        ask(&game, &named_stock),
        "the plan's threatened city"
    );

    let mut attacked = game.clone();
    attacked.turn = 40;
    attacked.cities.get_mut(&cid).unwrap().last_attacked = 37;
    assert_eq!(
        ask(&attacked, &governor(true)),
        ask(&attacked, &governor(false)),
        "attacked three turns ago"
    );

    let mut no_granary = game.clone();
    no_granary
        .cities
        .get_mut(&cid)
        .unwrap()
        .buildings
        .retain(|building| *building != "granary");
    assert_eq!(
        ask(&no_granary, &governor(true)),
        Some(Item::Building {
            building: crate::name!("granary"),
        }),
        "the Granary before the Aqueduct"
    );
}

/// See `housing_bound_granary_step`: a city the plan names threatened, a
/// city attacked within four turns, and a city holding a Settler more
/// than half built keep the stock pick under the gene.
#[test]
fn a_threatened_or_settling_city_keeps_the_stock_pick_under_the_granary_gene() {
    let (game, cid) = emperor_housing_bound_capital("HOUSINGGRANARYTHREAT", 91_903);
    let ask = |game: &Game, ai: &BasicAi| ai.pick_item(game, 0, cid, 3, 1, 1, 1, 0, 0, 0, 0);
    let stock = ask(&game, &emperor_lineage_governor(false));
    let granary = Some(Item::Building {
        building: crate::name!("granary"),
    });
    assert_eq!(
        ask(&game, &emperor_lineage_governor(true)),
        granary,
        "fixture: the gene fires"
    );

    let mut named = emperor_lineage_governor(true);
    named.plan_threatened_city = Some(cid);
    assert_eq!(ask(&game, &named), stock, "the plan's threatened city");

    let mut attacked = game.clone();
    attacked.turn = 40;
    attacked.cities.get_mut(&cid).unwrap().last_attacked = 37;
    assert_eq!(
        ask(&attacked, &emperor_lineage_governor(true)),
        ask(&attacked, &emperor_lineage_governor(false)),
        "attacked three turns ago"
    );
    attacked.turn = 45;
    assert_eq!(
        ask(&attacked, &emperor_lineage_governor(true)),
        granary,
        "eight turns ago"
    );

    let mut settling = game.clone();
    let settler = Item::Unit {
        unit: crate::name!("settler"),
    };
    let cost = settling.item_cost_for_city(0, cid, &settler);
    settling
        .cities
        .get_mut(&cid)
        .unwrap()
        .production_progress
        .insert(Game::item_progress_key(&settler), 0.6 * cost);
    assert_eq!(
        ask(&settling, &emperor_lineage_governor(true)),
        ask(&settling, &emperor_lineage_governor(false)),
        "a Settler 60% built"
    );
    settling
        .cities
        .get_mut(&cid)
        .unwrap()
        .production_progress
        .insert(Game::item_progress_key(&settler), 0.4 * cost);
    assert_eq!(
        ask(&settling, &emperor_lineage_governor(true)),
        granary,
        "40% built"
    );
}

/// See `luxury_builder_item`: a city short of Amenities, with no Granary
/// to build, trains a Builder for a luxury the empire lacks that lies
/// unimproved on its ground -- not once the empire holds a Builder for it,
/// and not for a luxury it already has.
#[test]
fn a_city_short_of_amenities_trains_a_builder_for_a_missing_luxury() {
    let (mut game, cid) = emperor_housing_bound_capital("HOUSINGGRANARYLUX", 91_907);
    game.players[0].techs.remove(&crate::name!("pottery"));
    let center = game.cities[&cid].pos;
    let site = game.cities[&cid]
        .owned_tiles
        .iter()
        .copied()
        .find(|pos| *pos != center && !game.rules.is_water(&game.map.tiles[pos]))
        .expect("an owned land tile");
    let tile = game.map.tiles.get_mut(&site).unwrap();
    tile.terrain = crate::name!("plains");
    tile.feature = None;
    tile.hills = false;
    tile.improvement = None;
    tile.resource = Some(crate::name!("salt"));
    assert!(
        game.valid_improvements(0, site)
            .contains(&crate::name!("mine")),
        "fixture: the salt takes a Mine"
    );
    assert_eq!(game.resource_access_count(0, "salt"), 0);
    std::sync::Arc::make_mut(&mut game.observed_city_amenity_adjustments).insert(cid, -4);
    assert!(
        game.city_amenity_surplus(&game.cities[&cid]) < 0,
        "fixture: short"
    );
    let builder = Some(Item::Unit {
        unit: crate::name!("builder"),
    });
    let ask = |game: &Game, gene: bool, builders: usize| {
        emperor_lineage_governor(gene).pick_item(game, 0, cid, 3, 1, builders, 1, 0, 0, 0, 0)
    };
    assert_ne!(
        ask(&game, false, 1),
        builder,
        "the stock pick is not a Builder"
    );
    assert_eq!(
        ask(&game, true, 0),
        builder,
        "no Builder for the salt: train one"
    );
    assert_eq!(
        ask(&game, true, 1),
        ask(&game, false, 1),
        "one Builder for one luxury"
    );
    let mut mined = game.clone();
    mined.map.tiles.get_mut(&site).unwrap().improvement = Some(crate::name!("mine"));
    assert!(
        mined.resource_access_count(0, "salt") > 0,
        "fixture: the salt is connected"
    );
    assert_eq!(
        ask(&mined, true, 0),
        ask(&mined, false, 0),
        "the luxury is held"
    );
}

/// See `settler_before_the_navy`: a coastal city due a Settler trains it
/// where the stock navy step would launch a ship.
#[test]
fn a_due_settler_comes_before_the_navy() {
    let (mut game, cid) = founded_capital_fixture("SETTLERNAVY", 91_851);
    game.cities.get_mut(&cid).unwrap().pop = 4;
    game.players[0].gold = 500.0;
    game.players[0].gold_per_turn = 5.0;
    game.players[0].techs.insert(crate::name!("sailing"));
    let shore = game.nbrs(game.cities[&cid].pos)[0];
    let tile = game.map.tiles.get_mut(&shore).unwrap();
    tile.terrain = crate::name!("coast");
    tile.feature = None;
    assert!(BasicAi::city_is_coastal(&game, cid));
    assert!(
        BasicAi::new().desired_navy(&game, 0) > 0,
        "the fixture wants a ship"
    );
    let pick = |on: bool, settlers: usize| {
        let mut ai = BasicAi::new();
        ai.settler_before_the_navy = on;
        // A standing army well over the floor, so the navy step is reached.
        ai.pick_item(&game, 0, cid, 3, settlers, 3, 1, 0, 30, 15, 15)
    };
    let stock = pick(false, 0);
    assert!(
        BasicAi::new().settler_due(&game, 0, cid, 3, 0),
        "the fixture is due a Settler"
    );
    let is_ship = |item: &Option<Item>| {
        matches!(item, Some(Item::Unit { unit })
            if game.rules.units.get(unit).is_some_and(|spec| spec.domain.as_deref() == Some("sea")))
    };
    assert!(
        is_ship(&stock),
        "the fixture's stock pick launches a ship: {stock:?}"
    );
    assert_eq!(
        pick(true, 0),
        Some(Item::Unit {
            unit: crate::name!("settler")
        }),
        "the due Settler comes first"
    );
    assert_eq!(
        pick(true, 1),
        pick(false, 1),
        "a Settler on the road leaves the navy step alone"
    );
}

/// See `campus_before_the_army`: a city without a Campus opens one where
/// the stock governor fills the military floor, from three cities on.
#[test]
fn a_campus_comes_before_the_army() {
    let (mut game, cid) = founded_capital_fixture("CAMPUSFIRST", 91_841);
    game.cities.get_mut(&cid).unwrap().pop = 4;
    for position in game.nbrs(game.cities[&cid].pos) {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
    }
    game.players[0].gold = 500.0;
    game.players[0].gold_per_turn = 5.0;
    game.players[0].techs.insert(crate::name!("writing"));
    assert!(
        BasicAi::first_campus_item(&game, 0, cid).is_some(),
        "the fixture can open a Campus"
    );
    let pick = |game: &Game, on: bool, n_cities: usize| {
        let mut ai = BasicAi::new();
        ai.campus_before_the_army = on;
        ai.pick_item(game, 0, cid, n_cities, 0, 3, 1, 0, 0, 0, 0)
    };
    let is_campus = |item: &Option<Item>| matches!(item, Some(Item::District { district, .. }) if *district == "campus");
    let stock = pick(&game, false, 3);
    assert!(
        !is_campus(&stock),
        "the fixture's stock pick is not the Campus: {stock:?}"
    );
    assert!(
        is_campus(&pick(&game, true, 3)),
        "{:?}",
        pick(&game, true, 3)
    );
    assert_eq!(
        pick(&game, true, 2),
        pick(&game, false, 2),
        "two cities keep the stock choice"
    );
    let second = |game: &Game, settlers: usize| {
        let mut ai = BasicAi::new();
        ai.campus_before_the_army_2 = true;
        ai.pick_item(game, 0, cid, 3, settlers, 3, 1, 0, 0, 0, 0)
    };
    let stock_ai = BasicAi::new();
    assert!(
        !stock_ai.settler_due(&game, 0, cid, 3, 1),
        "a Settler on the road fills the pipeline"
    );
    assert!(
        is_campus(&second(&game, 1)),
        "version 2 takes the floor's build"
    );
    assert!(
        stock_ai.settler_due(&game, 0, cid, 3, 0),
        "the fixture is due a Settler"
    );
    assert!(
        !is_campus(&second(&game, 0)),
        "version 2 yields to a due Settler"
    );
    // With the Campus standing, the Library comes next.
    let site = match BasicAi::first_campus_item(&game, 0, cid) {
        Some(Item::District { pos, .. }) => pos,
        other => panic!("{other:?}"),
    };
    game.map.tiles.get_mut(&site).unwrap().district = Some(crate::name!("campus"));
    game.cities
        .get_mut(&cid)
        .unwrap()
        .districts
        .insert(crate::name!("campus"), site);
    let library = pick(&game, true, 3);
    assert!(
        matches!(&library, Some(Item::Building { building }) if *building == "library"),
        "{library:?}"
    );
    // Version 3 goes on to the University once the Library stands.
    game.cities
        .get_mut(&cid)
        .unwrap()
        .buildings
        .push(crate::name!("library"));
    game.players[0].techs.insert(crate::name!("education"));
    let third = |game: &Game, on: bool| {
        let mut ai = BasicAi::new();
        ai.campus_before_the_army_2 = !on;
        ai.campus_before_the_army_3 = on;
        ai.pick_item(game, 0, cid, 3, 1, 3, 1, 0, 0, 0, 0)
    };
    let university = third(&game, true);
    assert!(
        matches!(&university, Some(Item::Building { building }) if *building == "university"),
        "{university:?}"
    );
    assert_ne!(
        third(&game, false),
        university,
        "version 2 stops at the Library"
    );
}

/// See `industry_before_the_army`: a city holding its Campus opens an
/// Industrial Zone where the stock governor fills the military floor,
/// then its Workshop, then its Factory; two cities, a due Settler, or a
/// third of the empire already on the chain keep the stock choice.
#[test]
fn a_campus_city_builds_its_industry_before_the_army() {
    let (mut game, cid) = campus_city_fixture();
    game.players[0].gold = 500.0;
    game.players[0].gold_per_turn = 5.0;
    game.players[0].techs.insert(crate::name!("apprenticeship"));
    // Mines on the second ring give every first-ring site adjacency.
    let center = game.cities[&cid].pos;
    let owned: Vec<Pos> = game.cities[&cid].owned_tiles.to_vec();
    for position in owned {
        if game.wdist(center, position) == 2 && game.map.tiles[&position].district.is_none() {
            let tile = game.map.tiles.get_mut(&position).unwrap();
            tile.terrain = crate::name!("plains");
            tile.hills = true;
            tile.feature = None;
            tile.resource = None;
            tile.improvement = Some(crate::name!("mine"));
        }
    }
    let pick = |game: &Game, on: bool, n_cities: usize, settlers: usize| {
        let mut ai = BasicAi::new();
        ai.industry_before_the_army = on;
        ai.pick_item(game, 0, cid, n_cities, settlers, 3, 1, 0, 0, 0, 0)
    };
    let is_zone = |item: &Option<Item>| matches!(item, Some(Item::District { district, .. }) if *district == "industrial_zone");
    let stock = pick(&game, false, 3, 1);
    assert!(
        !is_zone(&stock),
        "the fixture's stock pick is not the zone: {stock:?}"
    );
    let zone = pick(&game, true, 3, 1);
    assert!(is_zone(&zone), "the zone comes first: {zone:?}");
    assert_eq!(
        pick(&game, true, 2, 1),
        pick(&game, false, 2, 1),
        "two cities keep the stock choice"
    );
    assert!(
        BasicAi::new().settler_due(&game, 0, cid, 3, 0),
        "the fixture is due a Settler"
    );
    assert!(
        !is_zone(&pick(&game, true, 3, 0)),
        "a due Settler comes first"
    );
    // One city in three already on the chain leaves this one to stock.
    let mut busy = game.clone();
    busy.cities
        .get_mut(&cid)
        .unwrap()
        .queue
        .insert(0, zone.clone().unwrap());
    assert!(BasicAi::industry_before_the_army_item(&busy, 0, cid, 3, false).is_none());
    // With the zone standing, its Workshop, then its Factory.
    let site = match zone {
        Some(Item::District { pos, .. }) => pos,
        other => panic!("{other:?}"),
    };
    game.map.tiles.get_mut(&site).unwrap().district = Some(crate::name!("industrial_zone"));
    game.cities
        .get_mut(&cid)
        .unwrap()
        .districts
        .insert(crate::name!("industrial_zone"), site);
    let workshop = pick(&game, true, 3, 1);
    assert!(
        matches!(&workshop, Some(Item::Building { building }) if *building == "workshop"),
        "{workshop:?}"
    );
    game.cities
        .get_mut(&cid)
        .unwrap()
        .buildings
        .push(crate::name!("workshop"));
    game.players[0]
        .techs
        .insert(crate::name!("industrialization"));
    let factory = pick(&game, true, 3, 1);
    assert!(
        matches!(&factory, Some(Item::Building { building }) if *building == "factory"),
        "{factory:?}"
    );
}

/// See `prophet_race_takes_a_district_slot`: while the race is open, the
/// campus-first step's Campus becomes the empire's first Holy Site, and
/// only the first.
#[test]
fn an_open_prophet_race_takes_a_district_slot() {
    let (mut game, cid) = founded_capital_fixture("RACESLOT", 91_843);
    game.cities.get_mut(&cid).unwrap().pop = 4;
    for position in game.nbrs(game.cities[&cid].pos) {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
    }
    game.players[0].gold = 500.0;
    game.players[0].gold_per_turn = 5.0;
    game.players[0].techs.insert(crate::name!("writing"));
    game.players[0].techs.insert(crate::name!("astrology"));
    assert!(
        BasicAi::first_campus_item(&game, 0, cid).is_some(),
        "the fixture can open a Campus"
    );
    let pick = |game: &Game, gene: bool, open: bool| {
        let mut ai = BasicAi::new();
        ai.campus_before_the_army = true;
        ai.enter_prophet_race = open;
        ai.prophet_race_takes_a_district_slot = gene;
        ai.pick_item(game, 0, cid, 3, 0, 3, 1, 0, 0, 0, 0)
    };
    let family = |item: &Option<Item>| match item {
        Some(Item::District { district, .. }) => district.as_str().to_string(),
        other => format!("{other:?}"),
    };
    assert_eq!(
        family(&pick(&game, false, true)),
        "campus",
        "off, the Campus keeps its slot"
    );
    assert_eq!(
        family(&pick(&game, true, false)),
        "campus",
        "a closed race keeps the Campus"
    );
    assert_eq!(
        family(&pick(&game, true, true)),
        "holy_site",
        "{:?}",
        pick(&game, true, true)
    );
    // One Holy Site reserves the race: a second city keeps its Campus.
    let site = match pick(&game, true, true) {
        Some(Item::District { pos, .. }) => pos,
        other => panic!("{other:?}"),
    };
    game.map.tiles.get_mut(&site).unwrap().district = Some(crate::name!("holy_site"));
    game.cities
        .get_mut(&cid)
        .unwrap()
        .districts
        .insert(crate::name!("holy_site"), site);
    assert_eq!(
        family(&pick(&game, true, true)),
        "campus",
        "one Holy Site is enough"
    );
}

/// See `prophet_race_takes_a_district_slot_2`: the race's Holy Site goes
/// to the city that builds it soonest, ahead of its own campus step, and
/// a slower city keeps its Campus. Live King civvis-20261005T035848Z
/// (game 97) opened the site in a 3.6-production city.
#[test]
fn the_prophet_race_site_goes_to_the_fastest_city() {
    let (mut game, capital) = founded_capital_fixture("RACESITE2", 91_844);
    game.cities.get_mut(&capital).unwrap().pop = 4;
    let (x, y) = game.cities[&capital].pos;
    let second = game.found_city_for(0, (x + 7, y), None);
    game.cities.get_mut(&second).unwrap().pop = 4;
    std::sync::Arc::make_mut(&mut game.observed_city_yield_adjustments).insert(
        second,
        crate::rules::Yields {
            production: 4.0,
            ..Default::default()
        },
    );
    for city in [capital, second] {
        for position in game.nbrs(game.cities[&city].pos) {
            let tile = game.map.tiles.get_mut(&position).unwrap();
            tile.terrain = crate::name!("plains");
            tile.feature = None;
        }
    }
    game.players[0].gold = 500.0;
    game.players[0].gold_per_turn = 5.0;
    game.players[0].techs.insert(crate::name!("writing"));
    game.players[0].techs.insert(crate::name!("astrology"));
    assert!(
        game.city_yields(capital).production > game.city_yields(second).production,
        "fixture: the capital is the faster city"
    );
    let pick = |game: &Game, cid: u32, v1: bool, v2: bool| {
        let mut ai = BasicAi::new();
        ai.campus_before_the_army = true;
        ai.enter_prophet_race = true;
        ai.prophet_race_takes_a_district_slot = v1;
        ai.prophet_race_takes_a_district_slot_2 = v2;
        ai.pick_item(game, 0, cid, 2, 0, 3, 1, 0, 0, 0, 0)
    };
    let family = |item: &Option<Item>| match item {
        Some(Item::District { district, .. }) => district.as_str().to_string(),
        other => format!("{other:?}"),
    };
    assert_eq!(
        family(&pick(&game, capital, false, true)),
        "holy_site",
        "the faster city opens the site"
    );
    assert_ne!(
        family(&pick(&game, second, false, true)),
        "holy_site",
        "the slower city keeps its own pick"
    );
    // Closed race: nothing changes.
    let mut ai = BasicAi::new();
    ai.prophet_race_takes_a_district_slot_2 = true;
    assert!(
        ai.race_site_city(&game, 0).is_none(),
        "a closed race names no city"
    );
    ai.enter_prophet_race = true;
    assert_eq!(
        ai.race_site_city(&game, 0).map(|(city, _)| city),
        Some(capital)
    );
}

/// See `builder_before_the_army`: an empire with no Builder trains one
/// where the stock governor fills the military floor, and keeps the
/// stock choice once its quota is met or while it holds one city.
#[test]
fn a_builderless_empire_trains_a_builder_before_the_army() {
    let (mut game, cid) = founded_capital_fixture("BUILDERFIRST", 91_831);
    game.cities.get_mut(&cid).unwrap().pop = 3;
    game.players[0].gold = 500.0;
    game.players[0].gold_per_turn = 5.0;
    assert!(
        BasicAi::has_builder_work(&game, 0),
        "the fixture has land to improve"
    );
    let pick = |on: bool, n_cities: usize, builders: usize| {
        let mut ai = BasicAi::new();
        ai.builder_before_the_army = on;
        ai.pick_item(&game, 0, cid, n_cities, 0, builders, 1, 0, 0, 0, 0)
    };
    let is_builder =
        |item: &Option<Item>| matches!(item, Some(Item::Unit { unit }) if *unit == "builder");
    let stock = pick(false, 3, 0);
    assert!(
        matches!(stock, Some(Item::Unit { .. })) && !is_builder(&stock),
        "the fixture's stock pick fills the military floor: {stock:?}"
    );
    assert!(is_builder(&pick(true, 3, 0)), "{:?}", pick(true, 3, 0));
    assert_eq!(
        pick(true, 3, 2),
        pick(false, 3, 2),
        "the quota of two is met"
    );
    assert_eq!(
        pick(true, 1, 0),
        pick(false, 1, 0),
        "one city keeps the stock choice"
    );
    let second = |builders: usize| {
        let mut ai = BasicAi::new();
        ai.builder_before_the_army_2 = true;
        ai.pick_item(&game, 0, cid, 3, 0, builders, 1, 0, 0, 0, 0)
    };
    assert!(is_builder(&second(0)), "version 2 answers the drought");
    assert_eq!(
        second(1),
        pick(false, 3, 1),
        "version 2 leaves one Builder to stock"
    );
}

/// See `industrial_hub`: of three cities in one Factory's reach, the
/// most productive opens the zone and the others keep the stock choice;
/// the hub then takes its Workshop, Factory and a fuelled power plant,
/// and the Factory waits while the empire's Gold runs negative.
#[test]
fn the_industrial_hub_opens_where_its_factory_reaches_most() {
    let (mut game, capital) = founded_capital_fixture("INDUSTRIALHUB", 91_833);
    let center = game.cities[&capital].pos;
    let mut others = Vec::new();
    for pos in game.map.tiles.keys().copied().collect::<Vec<_>>() {
        if others.len() == 2 {
            break;
        }
        let tile = &game.map.tiles[&pos];
        let land = !game.rules.terrains[&tile.terrain].water && tile.terrain != "mountain";
        let spaced =
            game.wdist(center, pos) == 4 && others.iter().all(|other| game.wdist(*other, pos) >= 4);
        if land && spaced && tile.owner_city.is_none() {
            others.push(pos);
        }
    }
    assert_eq!(others.len(), 2, "the fixture places two more cities");
    let others: Vec<u32> = others
        .into_iter()
        .map(|pos| game.found_city_for(0, pos, None))
        .collect();
    game.players[0].gold = 500.0;
    game.players[0].gold_per_turn = 5.0;
    game.players[0].techs.insert(crate::name!("apprenticeship"));
    game.cities.get_mut(&capital).unwrap().pop = 7;
    let zone = BasicAi::industrial_hub_item(&game, 0, capital);
    let site = match zone {
        Some(Item::District { district, pos }) if district == "industrial_zone" => pos,
        other => panic!("the capital opens the hub's zone: {other:?}"),
    };
    for city in &others {
        assert_eq!(
            BasicAi::industrial_hub_item(&game, 0, *city),
            None,
            "one hub"
        );
    }
    game.map.tiles.get_mut(&site).unwrap().district = Some(crate::name!("industrial_zone"));
    game.cities
        .get_mut(&capital)
        .unwrap()
        .districts
        .insert(crate::name!("industrial_zone"), site);
    let next = |game: &Game| BasicAi::industrial_hub_item(game, 0, capital);
    assert_eq!(
        next(&game),
        Some(Item::Building {
            building: crate::name!("workshop")
        })
    );
    game.cities
        .get_mut(&capital)
        .unwrap()
        .buildings
        .push(crate::name!("workshop"));
    game.players[0]
        .techs
        .insert(crate::name!("industrialization"));
    assert_eq!(
        next(&game),
        Some(Item::Building {
            building: crate::name!("factory")
        })
    );
    game.players[0].gold_per_turn = -3.0;
    assert_eq!(next(&game), None, "the Factory waits on a negative budget");
    game.players[0].gold_per_turn = 5.0;
    game.cities
        .get_mut(&capital)
        .unwrap()
        .buildings
        .push(crate::name!("factory"));
    assert_eq!(next(&game), None, "no fuel, no plant");
    game.players[0]
        .strategic_resources
        .insert(crate::name!("coal"), 20.0);
    assert_eq!(
        next(&game),
        Some(Item::Building {
            building: crate::name!("coal_power_plant")
        })
    );

    // `exclude_fossil_power` (a Diplomatic seat): the governor that
    // fills the delegated queues takes neither fossil plant, from the hub
    // chain or from the cheapest-building fallback.
    let coal = Item::Building {
        building: crate::name!("coal_power_plant"),
    };
    let oil = Item::Building {
        building: crate::name!("oil_power_plant"),
    };
    let nuclear = Item::Building {
        building: crate::name!("nuclear_power_plant"),
    };
    let mut stock = BasicAi::new();
    stock.industrial_hub = true;
    assert!(!stock.fossil_power_excluded(&coal), "off by default");
    let mut diplomatic = stock.clone();
    diplomatic.exclude_fossil_power = true;
    assert!(diplomatic.fossil_power_excluded(&coal));
    assert!(diplomatic.fossil_power_excluded(&oil));
    assert!(!diplomatic.fossil_power_excluded(&nuclear));
    let pick = |ai: &BasicAi| ai.pick_item(&game, 0, capital, 3, 1, 3, 1, 0, 3, 2, 1);
    assert_eq!(
        pick(&stock),
        Some(coal.clone()),
        "the hub's stock pick is the coal plant"
    );
    let refused = pick(&diplomatic);
    assert!(
        refused != Some(coal.clone()) && refused != Some(oil.clone()),
        "the diplomatic governor picks something else: {refused:?}"
    );
}

/// See `builder_before_the_army_3`: past version 2's first Builder, an
/// empire working unimproved hills trains another where the stock
/// governor fills the military floor, and a due Settler comes first.
#[test]
fn unimproved_worked_hills_call_another_builder_before_the_army() {
    let (mut game, cid) = founded_capital_fixture("BUILDERBACKLOG", 91_832);
    game.cities.get_mut(&cid).unwrap().pop = 6;
    game.players[0].gold = 500.0;
    game.players[0].gold_per_turn = 5.0;
    game.players[0].techs.insert(crate::name!("mining"));
    let center = game.cities[&cid].pos;
    let owned: Vec<Pos> = game.cities[&cid].owned_tiles.to_vec();
    for position in owned {
        if position != center && game.map.tiles[&position].district.is_none() {
            let tile = game.map.tiles.get_mut(&position).unwrap();
            tile.terrain = crate::name!("grassland");
            tile.hills = true;
            tile.feature = None;
            tile.resource = None;
            tile.improvement = None;
        }
    }
    assert!(
        BasicAi::unimproved_worked_tiles(&game, 0) >= 4,
        "the fixture works unimproved hills: {}",
        BasicAi::unimproved_worked_tiles(&game, 0)
    );
    let pick = |version: u8, settlers: usize, builders: usize| {
        let mut ai = BasicAi::new();
        ai.builder_before_the_army_2 = version == 2;
        ai.builder_before_the_army_3 = version == 3;
        ai.pick_item(&game, 0, cid, 3, settlers, builders, 1, 0, 0, 0, 0)
    };
    let is_builder =
        |item: &Option<Item>| matches!(item, Some(Item::Unit { unit }) if *unit == "builder");
    assert!(
        !is_builder(&pick(2, 1, 1)),
        "version 2 leaves a second Builder to stock"
    );
    assert!(is_builder(&pick(3, 1, 1)), "{:?}", pick(3, 1, 1));
    assert!(is_builder(&pick(3, 1, 0)), "the first Builder as version 2");
    assert_eq!(
        pick(3, 1, 2),
        pick(2, 1, 2),
        "the one-per-two-cities cap is met"
    );
    assert!(
        BasicAi::new().settler_due(&game, 0, cid, 3, 0),
        "the fixture is due a Settler"
    );
    assert!(!is_builder(&pick(3, 0, 1)), "a due Settler comes first");
}

/// See `builders_cover_the_worked_backlog`: past `builder-before-the-army-3`'s
/// one-per-two-cities cap, an empire whose Builder charges cover too little
/// of its unimproved worked hills trains another; charges in hand that
/// cover the backlog, one Builder per city, a due Settler and a city just
/// attacked keep the stock pick.
#[test]
fn builders_cover_the_worked_backlog_under_the_gene() {
    let (mut game, cid) = founded_capital_fixture("BUILDERCOVER", 91_841);
    game.cities.get_mut(&cid).unwrap().pop = 6;
    game.players[0].gold = 500.0;
    game.players[0].gold_per_turn = 5.0;
    game.players[0].techs.insert(crate::name!("mining"));
    let center = game.cities[&cid].pos;
    let owned: Vec<Pos> = game.cities[&cid].owned_tiles.to_vec();
    for position in owned {
        if position != center && game.map.tiles[&position].district.is_none() {
            let tile = game.map.tiles.get_mut(&position).unwrap();
            tile.terrain = crate::name!("grassland");
            tile.hills = true;
            tile.feature = None;
            tile.resource = None;
            tile.improvement = None;
        }
    }
    let backlog = BasicAi::unimproved_worked_tiles(&game, 0);
    assert!(
        backlog >= BasicAi::WORKED_BACKLOG_MIN_TILES,
        "fixture backlog {backlog}"
    );
    assert_eq!(
        BasicAi::builder_charge_cover(&game, 0),
        0,
        "fixture: no Builder charges"
    );
    let pick = |game: &Game, gene: bool, settlers: usize, builders: usize| {
        let mut ai = BasicAi::new();
        ai.builder_before_the_army_3 = true;
        ai.builders_cover_the_worked_backlog = gene;
        ai.pick_item(game, 0, cid, 3, settlers, builders, 1, 0, 0, 0, 0)
    };
    let is_builder =
        |item: &Option<Item>| matches!(item, Some(Item::Unit { unit }) if *unit == "builder");
    assert!(
        !is_builder(&pick(&game, false, 1, 2)),
        "version 3's one-per-two-cities cap is met: {:?}",
        pick(&game, false, 1, 2)
    );
    assert!(
        is_builder(&pick(&game, true, 1, 2)),
        "{:?}",
        pick(&game, true, 1, 2)
    );
    assert_eq!(
        pick(&game, true, 1, 3),
        pick(&game, false, 1, 3),
        "one Builder per city"
    );
    assert!(
        BasicAi::new().settler_due(&game, 0, cid, 3, 0),
        "the fixture is due a Settler"
    );
    assert_eq!(
        pick(&game, true, 0, 2),
        pick(&game, false, 0, 2),
        "a due Settler keeps the stock pick"
    );
    let mut attacked = game.clone();
    attacked.cities.get_mut(&cid).unwrap().last_attacked = attacked.turn.max(1);
    assert_eq!(
        pick(&attacked, true, 1, 2),
        pick(&attacked, false, 1, 2),
        "a city just attacked keeps the stock pick"
    );
    // Charges in hand that cover the backlog: no further Builder.
    let mut covered = game.clone();
    let uid = covered.spawn_unit("builder", 0, center);
    covered.units.get_mut(&uid).unwrap().charges = backlog as i32;
    assert!(BasicAi::builder_charge_cover(&covered, 0) >= backlog as i32);
    assert_eq!(
        pick(&covered, true, 1, 2),
        pick(&covered, false, 1, 2),
        "charges in hand cover the backlog"
    );
    // A Builder already queued counts its charges.
    let mut queued = game.clone();
    queued.cities.get_mut(&cid).unwrap().queue.insert(
        0,
        Item::Unit {
            unit: crate::name!("builder"),
        },
    );
    assert_eq!(
        BasicAi::builder_charge_cover(&queued, 0),
        queued.builder_charges(0),
        "a queued Builder counts the charges it will carry"
    );
}

/// See `monument_first`: a city without a Monument builds it where the
/// stock governor fills the military floor, and a capital that has not
/// yet sent out two Settlers keeps the stock choice.
#[test]
fn the_monument_comes_before_the_military_floor() {
    let (mut game, cid) = founded_capital_fixture("MONUMENTFIRST", 91_811);
    let city = game.cities.get_mut(&cid).unwrap();
    city.pop = 3;
    // The fixture's start grants a Monument; this city has none.
    city.buildings.retain(|building| building != "monument");
    // A treasury, so the economic-recovery branch ahead of both steps
    // stays shut.
    game.players[0].gold = 500.0;
    game.players[0].gold_per_turn = 5.0;
    let pick = |game: &Game, first: bool, n_cities: usize, settlers: usize| {
        let mut ai = BasicAi::new();
        ai.monument_first = first;
        ai.pick_item(game, 0, cid, n_cities, settlers, 3, 1, 0, 0, 0, 0)
    };
    let is_monument = |item: &Option<Item>| matches!(item, Some(Item::Building { building }) if *building == "monument");
    assert!(
        BasicAi::civ_building(&game, 0, cid, "monument").is_some(),
        "the fixture can build a Monument"
    );
    let ai = BasicAi::new();
    assert_eq!(
        ai.barbarian_defense_item(&game, 0, cid),
        None,
        "no barbarian alarm"
    );
    assert_eq!(ai.besieged_city_item(&game, 0, cid), None, "no siege");
    let stock = pick(&game, false, 3, 0);
    assert!(
        matches!(stock, Some(Item::Unit { .. })),
        "the fixture's stock pick fills the military floor: {stock:?}"
    );
    let first = pick(&game, true, 3, 0);
    assert!(is_monument(&first), "the Monument comes first: {first:?}");
    assert_eq!(
        pick(&game, true, 2, 0),
        pick(&game, false, 2, 0),
        "a capital that has sent one Settler keeps the stock choice"
    );
    assert!(
        is_monument(&pick(&game, true, 2, 1)),
        "a Settler on the road counts as sent"
    );
    game.cities
        .get_mut(&cid)
        .unwrap()
        .buildings
        .push(crate::name!("monument"));
    assert_eq!(
        pick(&game, true, 3, 0),
        stock,
        "a city holding a Monument is not asked again"
    );
}

/// A capital holding a Monument and a Campus, with the Writing, Currency
/// and Astrology it needs for a Library and two more districts.
fn campus_city_fixture() -> (Game, u32) {
    let (mut game, cid) = founded_capital_fixture("DISTRICTBUILDINGS", 91_812);
    let site = game.cities[&cid]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != game.cities[&cid].pos)
        .unwrap();
    game.map.tiles.get_mut(&site).unwrap().district = Some(crate::name!("campus"));
    let city = game.cities.get_mut(&cid).unwrap();
    city.pop = 7;
    city.districts.insert(crate::name!("campus"), site);
    if !city.buildings.contains(&crate::name!("monument")) {
        city.buildings.push(crate::name!("monument"));
    }
    for tech in ["writing", "currency", "astrology", "pottery"] {
        game.players[0].techs.insert(Name::new(tech));
    }
    (game, cid)
}

/// See `district_buildings_first`: the stock governor opens another
/// district beside a Campus that has no Library; the gene builds the
/// Library first.
#[test]
fn a_standing_campus_takes_its_library_before_another_district() {
    let (game, cid) = campus_city_fixture();
    let pick = |first: bool| {
        let mut ai = BasicAi::new();
        ai.district_buildings_first = first;
        ai.pick_item(&game, 0, cid, 12, 0, 6, 1, 0, 12, 6, 6)
    };
    let stock = pick(false);
    assert!(
        matches!(stock, Some(Item::District { .. })),
        "the fixture's stock pick opens another district: {stock:?}"
    );
    assert_eq!(
        pick(true),
        Some(Item::Building {
            building: crate::name!("library")
        }),
        "the Campus takes its Library first"
    );
}

/// See `culture_defense_theater`: while a rival out-makes the empire's
/// Culture, the city opens a Theater Square the stock governor does not;
/// with no rival Culture to trail, the stock choice stands.
#[test]
fn a_trailing_empire_opens_a_theater_square() {
    let mut game = Game::new_full(
        2,
        24,
        16,
        crate::rng::fixture_seed("CULTUREDEFENSE", 91_813),
        250,
        0,
        false,
    );
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let cid = game.player_city_ids(0)[0];
    // The rival is only a Culture figure here; its starting units on a
    // small map would otherwise raise the local-defense steps.
    for unit in game.player_unit_ids(1) {
        game.remove_unit(unit);
    }
    std::sync::Arc::make_mut(&mut game.observed_city_yield_adjustments).insert(
        cid,
        crate::rules::Yields {
            production: 10.0,
            ..Default::default()
        },
    );
    game.players[0].gold = 500.0;
    game.players[0].gold_per_turn = 5.0;
    game.players[0].civics.insert(crate::name!("drama_poetry"));
    for tech in ["writing", "currency"] {
        game.players[0].techs.insert(Name::new(tech));
    }
    for position in game.nbrs(game.cities[&cid].pos) {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
    }
    let city = game.cities.get_mut(&cid).unwrap();
    city.pop = 7;
    if !city.buildings.contains(&crate::name!("monument")) {
        city.buildings.push(crate::name!("monument"));
    }
    let pick = |game: &Game, on: bool| {
        let mut ai = BasicAi::new();
        ai.culture_defense_theater = on;
        ai.pick_item(game, 0, cid, 12, 0, 6, 1, 0, 12, 6, 6)
    };
    let is_theater = |item: &Option<Item>, game: &Game| {
        matches!(item, Some(Item::District { district, .. })
                if game.district_family(*district) == "theater_square")
    };
    assert_eq!(
        pick(&game, true),
        pick(&game, false),
        "no rival Culture, no answer"
    );
    std::sync::Arc::make_mut(&mut game.observed_yield_adjustments).insert(
        1,
        crate::rules::Yields {
            culture: 100.0,
            ..Default::default()
        },
    );
    assert!(BasicAi::culture_trails_the_field(&game, 0));
    let stock = pick(&game, false);
    assert!(
        !is_theater(&stock, &game),
        "the stock pick is not a Theater: {stock:?}"
    );
    let theater = BasicAi::civ_district(&game, 0, "theater_square");
    let sites = game.district_sites(cid, theater);
    let producible: Vec<bool> = sites
        .iter()
        .map(|pos| {
            game.can_produce(
                0,
                cid,
                &Item::District {
                    district: theater,
                    pos: *pos,
                },
            )
        })
        .collect();
    assert!(
        BasicAi::culture_defense_theater_item(&game, 0, cid, 12).is_some(),
        "the city has a Theater site: {theater} sites {sites:?} producible {producible:?}"
    );
    let answer = pick(&game, true);
    assert!(
        is_theater(&answer, &game),
        "a trailing empire opens one: {answer:?}"
    );
}

/// See `capital_library_first`: a three-city capital holding a Campus
/// takes its Library where the stock governor trains the next Settler.
#[test]
fn the_capital_takes_its_library_before_the_next_settler() {
    let (mut game, cid) = campus_city_fixture();
    game.players[0].gold = 500.0;
    game.players[0].gold_per_turn = 5.0;
    let pick = |game: &Game, on: bool, n_cities: usize| {
        let mut ai = BasicAi::new();
        ai.capital_library_first = on;
        ai.pick_item(game, 0, cid, n_cities, 0, 3, 1, 0, 6, 3, 3)
    };
    let library = Some(Item::Building {
        building: crate::name!("library"),
    });
    let stock = pick(&game, false, 3);
    assert_ne!(
        stock, library,
        "the fixture's stock pick is not the Library"
    );
    assert_eq!(
        pick(&game, true, 3),
        library,
        "the Library rides with its Campus"
    );
    assert_eq!(
        pick(&game, true, 2),
        pick(&game, false, 2),
        "two cities keep the stock choice"
    );
}

/// See `district_buildings_first`: only a district's first building is
/// raised. A Campus holding its Library does not pull the University
/// ahead of a new district.
#[test]
fn a_district_buildings_first_city_does_not_raise_the_second_tier() {
    let (mut game, cid) = campus_city_fixture();
    game.players[0].techs.insert(crate::name!("education"));
    game.cities
        .get_mut(&cid)
        .unwrap()
        .buildings
        .push(crate::name!("library"));
    assert_eq!(BasicAi::district_building_item(&game, 0, cid), None);
    let pick = |first: bool| {
        let mut ai = BasicAi::new();
        ai.district_buildings_first = first;
        ai.pick_item(&game, 0, cid, 12, 0, 6, 1, 0, 12, 6, 6)
    };
    assert_eq!(pick(true), pick(false));
}

/// A Scout the host bounces between two tiles toward the same goal is as
/// stuck as one it never moves: live King 2026-09-30T225143Z paced its
/// Scout between two tiles for fifteen turns.
#[test]
fn a_scout_the_host_bounces_between_two_tiles_gives_up_its_exploration_target() {
    let mut game = Game::new_full(
        1,
        24,
        16,
        crate::rng::fixture_seed("DEADGOAL", 91_779),
        250,
        0,
        false,
    );
    let first = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: first }).unwrap();
    let home = game.cities[&game.player_city_ids(0)[0]].pos;
    let scout = game.spawn_unit("scout", 0, home);
    game.turn = 10;

    let mut ai = BasicAi::new();
    ai.enable_explore_commit();
    ai.enable_explore_dead_targets();
    let goal = ai
        .exploration_goal(&game, 0, scout, false)
        .expect("a fresh map has unexplored ground to aim at");
    let away = game
        .nbrs(home)
        .into_iter()
        .filter(|pos| game.map.tiles.contains_key(pos) && game.unit_can_traverse(scout, *pos))
        .max_by_key(|pos| game.wdist(*pos, goal))
        .expect("a passable neighbour");
    // The host moves the unit, but only ever back and forth.
    let mut retired_on = None;
    for (step, tile) in [home, away, home, away, home, away].into_iter().enumerate() {
        game.relocate(scout, tile);
        game.units.get_mut(&scout).unwrap().moves_left = 0.0;
        game.turn = 10 + step as u32;
        let _ = ai.explore_step(&mut game, 0, scout);
        if ai
            .explore_dead
            .borrow()
            .get(&scout)
            .is_some_and(|dead| dead.contains_key(&goal))
        {
            retired_on = Some(step);
            break;
        }
    }
    assert_eq!(
        retired_on,
        Some(EXPLORE_NO_PROGRESS_TURNS as usize),
        "turns without coming closer retire the goal"
    );
    let dead = ai.explore_dead.borrow();
    let retired = dead.get(&scout).expect("the goal is written off");
    assert!(
        game.wdisk(goal, EXPLORE_NO_PROGRESS_RING)
            .iter()
            .all(|pos| retired.contains_key(pos)),
        "with the wider ring, so the next goal is not behind the same obstacle"
    );
}

/// ★★★★ See `explore_commit`: a goal is held across turns until it is
/// revealed or aged out, a second explorer keeps out of the first one's
/// ground, the pick reaches deeper than the stock rule and prefers the
/// ground farthest from home, and the plain controllers keep re-deriving
/// theirs every turn.
#[test]
fn a_committed_explorer_holds_its_goal_and_sweeps_outward() {
    let mut game = Game::new_full(
        1,
        24,
        16,
        crate::rng::fixture_seed("COMMITGOAL", 91_781),
        250,
        0,
        false,
    );
    let first = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: first }).unwrap();
    let home = game.cities[&game.player_city_ids(0)[0]].pos;
    let scout = game.spawn_unit("scout", 0, home);
    game.turn = 10;

    // The plain controllers hold nothing.
    let plain = BasicAi::new();
    assert!(!plain.explore_commit);
    let _ = plain.exploration_goal(&game, 0, scout, false);
    assert!(plain.explore_goal.borrow().is_empty());

    let mut ai = BasicAi::new();
    ai.enable_explore_commit();
    let goal = ai
        .exploration_goal(&game, 0, scout, false)
        .expect("a fresh map has unexplored ground to aim at");
    assert_eq!(
        ai.explore_goal.borrow().get(&scout).copied(),
        Some((goal, 10))
    );
    // The committed goal lies beyond the stock lookahead's reach: at least
    // as far as the stock rule's own pick, and never nearer to home than it.
    let stock = plain.exploration_goal(&game, 0, scout, false).unwrap();
    assert!(
            game.wdist(home, goal) >= game.wdist(home, stock),
            "the committed pick is the farther-from-home one: commit {goal:?} at {} vs stock {stock:?} at {}",
            game.wdist(home, goal), game.wdist(home, stock)
        );

    // Held on the following turns, whatever the nearest fringe now is.
    game.turn = 11;
    assert_eq!(
        ai.exploration_goal(&game, 0, scout, false),
        Some(goal),
        "held on turn 11"
    );
    game.turn = 10 + EXPLORE_COMMIT_TURNS;
    assert_eq!(
        ai.exploration_goal(&game, 0, scout, false),
        Some(goal),
        "held to its age limit"
    );
    // Aged out: re-chosen (and re-stamped) rather than kept forever.
    game.turn = 11 + EXPLORE_COMMIT_TURNS;
    let again = ai
        .exploration_goal(&game, 0, scout, false)
        .expect("still ground to aim at");
    assert_eq!(
        ai.explore_goal
            .borrow()
            .get(&scout)
            .map(|(_, since)| *since),
        Some(game.turn)
    );
    // Revealed: dropped and re-chosen.
    game.players[0].explored.insert(again);
    let after = ai
        .exploration_goal(&game, 0, scout, false)
        .expect("still ground to aim at");
    assert_ne!(after, again, "a revealed goal is not held");

    // A second explorer keeps out of the first one's ground.
    let second = game.spawn_unit("scout", 0, home);
    let other = ai
        .exploration_goal(&game, 0, second, false)
        .expect("the second scout has ground of its own");
    assert!(
        game.wdist(other, after) > EXPLORE_COMMIT_SEPARATION,
        "the second scout's goal {other:?} lies clear of the first's {after:?}"
    );

    // A dead board (`explore_dead_targets`) drops the held goal too.
    let mut bridge = BasicAi::new();
    bridge.enable_explore_commit();
    bridge.enable_explore_dead_targets();
    game.turn = 40;
    let held = bridge.exploration_goal(&game, 0, scout, false).unwrap();
    bridge
        .explore_dead
        .borrow_mut()
        .entry(scout)
        .or_default()
        .insert(held, game.turn + EXPLORE_DEAD_TARGET_TURNS);
    let replaced = bridge.exploration_goal(&game, 0, scout, false).unwrap();
    assert_ne!(replaced, held, "a written-off goal is not held");

    // And the memory rides a rebuilt board like the other unit-keyed maps.
    let mut carried = BasicAi::new();
    carried.enable_explore_commit();
    game.turn = 50;
    let before = carried.exploration_goal(&game, 0, scout, false).unwrap();
    let map = std::collections::BTreeMap::from([(scout, 999u32)]);
    carried.remap_unit_memory(&map);
    assert_eq!(
        carried
            .explore_goal
            .borrow()
            .get(&999)
            .map(|(goal, _)| *goal),
        Some(before)
    );
    carried.forget_unit_memory();
    assert!(carried.explore_goal.borrow().is_empty());
}

/// An unmet city-state is enough reason to cross the continent, but not
/// enough information to know which direction it lies. This deliberately
/// leaves only one long fog corridor: the destination selector can see
/// terrain and its own fog, while the contact remains hidden until the
/// Scout's sight reaches it.
#[test]
fn an_unmet_city_state_sends_a_scout_on_a_long_contact_sweep() {
    const CONTACT_DISTANCE: i32 = 20;
    let mut game = Game::new_full(
        1,
        74,
        46,
        crate::rng::fixture_seed("CONTACTSWEEP", 91_782),
        250,
        1,
        false,
    );
    let minor = game
        .players
        .iter()
        .find(|player| player.is_minor && !player.is_barbarian)
        .map(|player| player.id)
        .expect("fixture has one city-state");
    let minor_city = game.player_city_ids(minor)[0];
    let ray_from = |origin: Pos| {
        let mut current = origin;
        let mut ray = Vec::new();
        for distance in 1..=CONTACT_DISTANCE {
            current = game.nbrs(current).into_iter().find(|position| {
                game.map.tiles.contains_key(position) && game.wdist(origin, *position) == distance
            })?;
            ray.push(current);
        }
        Some(ray)
    };
    let (home, ray) = game
        .map
        .tiles
        .keys()
        .copied()
        .find_map(|position| ray_from(position).map(|ray| (position, ray)))
        .expect("the standard board has a twenty-tile land corridor");

    // A flat continent makes the movement question deterministic. The
    // city-state itself is placed at the far end only after the map is
    // made, and stays in fog until the Scout sees it.
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
        tile.owner_city = None;
        tile.cliff_edges = [false; 6];
    }
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .expect("the major opens with a Settler");
    game.relocate(settler, home);
    game.current = 0;
    game.apply(0, &Action::FoundCity { unit: settler })
        .expect("the clear home tile is a legal capital");
    let contact = *ray.last().expect("twenty-tile ray");
    game.cities.get_mut(&minor_city).unwrap().pos = contact;
    game.map.tiles.get_mut(&contact).unwrap().owner_city = Some(minor_city);
    for player in game.players.iter_mut() {
        player.met.clear();
    }

    let scout = game.spawn_test_unit("scout", 0, home);
    let charted: std::collections::BTreeSet<Pos> = game.map.tiles.keys().copied().collect();
    game.players[0].explored = charted;
    for position in &ray {
        game.players[0].explored.remove(position);
    }
    assert!(
        !game.has_met(0, minor),
        "the city-state must stay unknown until the Scout's own sight finds it"
    );

    let mut ai = BasicAi::new();
    ai.enable_explore_commit();
    ai.enable_explore_dead_targets();
    let goal = ai
        .exploration_goal(&game, 0, scout, true)
        .expect("the fog corridor has a destination");
    assert!(
        game.wdist(home, goal) > EXPLORE_COMMIT_LOOKAHEAD + 1,
        "an unmet city-state warrants a continent-scale goal, got {goal:?} at {}",
        game.wdist(home, goal)
    );
    assert!(
        game.route_step(scout, goal, 0).is_some(),
        "the held long horizon has an actual route"
    );

    // The continent sweep is an expedition assignment for a recon unit,
    // not a map-wide pathfinding policy for every unit that happens to
    // fall through to generic exploration.
    let mut non_recon = game.clone();
    let warrior = non_recon.spawn_test_unit("warrior", 0, home);
    let mut ordinary_ai = BasicAi::new();
    ordinary_ai.enable_explore_commit();
    ordinary_ai.enable_explore_dead_targets();
    let ordinary_goal = ordinary_ai
        .exploration_goal(&non_recon, 0, warrior, true)
        .expect("the ordinary explorer has a local frontier");
    let first_local_frontier = non_recon
        .map
        .tiles
        .keys()
        .filter(|position| !non_recon.players[0].explored.contains(position))
        .filter(|position| {
            non_recon.map.get(**position).is_some_and(|tile| {
                non_recon.rules.is_passable(tile) && !non_recon.rules.is_water(tile)
            })
        })
        .map(|position| non_recon.wdist(home, *position))
        .min()
        .expect("the fog corridor has a first local frontier");
    assert!(
            non_recon.wdist(home, ordinary_goal)
                <= first_local_frontier + EXPLORE_COMMIT_LOOKAHEAD,
            "only a recon unit gets the continent sweep, got {ordinary_goal:?} at {} from a first frontier at {first_local_frontier}",
            non_recon.wdist(home, ordinary_goal),
        );

    // The expensive long appointment is deliberately for the Civ VI
    // verification bridge, whose target-retirement guard is on. Native
    // tournament controllers retain the established local commitment.
    let mut native_ai = BasicAi::new();
    native_ai.enable_explore_commit();
    let native_goal = native_ai
        .exploration_goal(&game, 0, scout, true)
        .expect("the native Scout has a local frontier");
    assert!(
            game.wdist(home, native_goal) <= first_local_frontier + EXPLORE_COMMIT_LOOKAHEAD,
            "without the bridge guard a Scout stays local, got {native_goal:?} at {} from a first frontier at {first_local_frontier}",
            game.wdist(home, native_goal),
        );

    // Once every city-state is known the same fog uses the ordinary,
    // short committed horizon again.
    let mut known = game.clone();
    known.record_contact(0, minor);
    let mut known_ai = BasicAi::new();
    known_ai.enable_explore_commit();
    known_ai.enable_explore_dead_targets();
    let local_goal = known_ai
        .exploration_goal(&known, 0, scout, true)
        .expect("the local fog still has a destination");
    assert!(
        known.wdist(home, local_goal) <= EXPLORE_COMMIT_LOOKAHEAD + 1,
        "once contact is complete the Scout returns to the local horizon: {local_goal:?}"
    );

    // Seven clear Scout turns are enough to reach the far side of the
    // corridor. A short-horizon picker wastes its first commitment on the
    // near section; this one opens the city-state before that second
    // appointment is necessary.
    for _ in 0..7 {
        let moves = game.unit_max_moves(scout);
        game.units.get_mut(&scout).unwrap().moves_left = moves;
        ai.begin_movement_turn(&game, 0);
        for _ in 0..8 {
            if game.units[&scout].moves_left <= 0.0 || !ai.explore_step(&mut game, 0, scout) {
                break;
            }
        }
        if game.has_met(0, minor) {
            break;
        }
        game.turn += 1;
    }
    assert!(
        game.has_met(0, minor),
        "the long sweep discovers the city-state at the far end of its own fog"
    );
}

/// The frozen `advanced_v1` anchor and every native tournament game keep
/// the historical gate: without the treatment a parked settler is still
/// "in flight", so their recorded ladders stay comparable.
#[test]
fn without_the_treatment_a_parked_settler_still_blocks_expansion() {
    let mut game = Game::new_full(
        1,
        24,
        16,
        crate::rng::fixture_seed("STRANDED", 91_774),
        250,
        0,
        false,
    );
    let first = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: first }).unwrap();
    let capital = game.cities[&game.player_city_ids(0)[0]].pos;
    let parked = game
        .map
        .tiles
        .iter()
        .filter(|(_, tile)| game.rules.is_passable(tile) && !game.rules.is_water(tile))
        .map(|(position, _)| *position)
        .find(|position| game.wdist(capital, *position) == 3)
        .unwrap();
    game.spawn_unit("settler", 0, parked);

    let mut ai = BasicAi::new();
    assert!(!ai.settler_strand_discount, "off unless the bridge asks");
    for turn in 1..=(BasicAi::STRANDED_SETTLER_TURNS * 3) {
        game.turn = turn;
        ai.refresh_settler_idle(&game, 0);
        assert_eq!(ai.stranded_settlers(&game, 0), 0);
        assert_eq!(
            ai.walking_settlers(&game, 0, 1),
            1,
            "still in flight at turn {turn}"
        );
    }
    assert!(
        ai.settler_idle.is_empty(),
        "no bookkeeping when the treatment is off"
    );
}

#[test]
fn unfounded_empire_reserves_only_one_holy_site_for_the_prophet_race() {
    let mut game = Game::new_full(
        1,
        24,
        16,
        crate::rng::fixture_seed("HOLYSITE", 91_774),
        120,
        0,
        false,
    );
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let capital = game.player_city_ids(0)[0];
    let capital_pos = game.cities[&capital].pos;
    let second_pos = game
        .map
        .tiles
        .iter()
        .filter(|(_, tile)| {
            tile.owner_city.is_none() && game.rules.is_passable(tile) && !game.rules.is_water(tile)
        })
        .map(|(position, _)| *position)
        .find(|position| (4..=10).contains(&game.wdist(capital_pos, *position)))
        .unwrap();
    let second = game.found_city_for(0, second_pos, None);
    game.players[0].techs.insert(crate::name!("astrology"));
    for city in [capital, second] {
        game.cities
            .get_mut(&city)
            .unwrap()
            .buildings
            .push(crate::name!("monument"));
    }

    let ai = BasicAi::new();
    let choose = |game: &Game, city| {
        ai.pick_item(game, 0, city, 2, 2, 2, 2, 10, 5, 5, 5)
            .expect("city has a production choice")
    };
    let first = choose(&game, capital);
    assert!(matches!(
        first,
        Item::District { ref district, .. }
            if game.district_family(district) == "holy_site"
    ));
    game.apply(
        0,
        &Action::Produce {
            city: capital,
            item: first,
        },
    )
    .unwrap();

    let reserved = choose(&game, second);
    assert!(
        !matches!(
            reserved,
            Item::District { ref district, .. }
                if game.district_family(district) == "holy_site"
        ),
        "a second opening Holy Site displaced development: {reserved:?}"
    );

    game.players[0].religion = Some("Test Faith".to_string());
    let founded = choose(&game, second);
    assert!(matches!(
        founded,
        Item::District { ref district, .. }
            if game.district_family(district) == "holy_site"
    ));
}

#[test]
fn live_missionary_buyer_never_strengthens_a_rival_majority() {
    let mut game = Game::new_full(2, 24, 16, 91_775, 120, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = game.player_city_ids(0)[0];
    let center = game.cities[&city].pos;
    let holy_site = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != center)
        .unwrap();
    game.map.tiles.get_mut(&holy_site).unwrap().district = Some(crate::name!("holy_site"));
    {
        let city = game.cities.get_mut(&city).unwrap();
        city.districts.insert(crate::name!("holy_site"), holy_site);
        city.buildings.push(crate::name!("shrine"));
        city.atheist_pressure = 0.0;
        city.pressure.insert("Rival Faith".to_string(), 1_000.0);
        // Keep this test on the post-production Faith-spending path.
        city.queue.push(Item::Project {
            project: crate::name!("holy_site_prayers"),
        });
    }
    game.players[0].religion = Some("Home Faith".to_string());
    game.players[1].religion = Some("Rival Faith".to_string());
    game.players[0].techs.insert(crate::name!("astrology"));
    game.players[0].faith = 1_000.0;
    let mut historical = game.clone();
    BasicAi::new().cities(&mut historical, 0);
    assert!(
        historical.units.values().any(|unit| {
            unit.owner == 0
                && unit.kind == "missionary"
                && unit.religion.as_deref() == Some("Rival Faith")
        }),
        "the default-off gate preserves the frozen native controller"
    );

    let mut ai = BasicAi::new();
    ai.w.faith_builder = f64::INFINITY;
    ai.live_religious_purchase_guard = true;

    let before = game.log.len();
    ai.cities(&mut game, 0);
    assert!(
        !game.log.since(before).any(|(_, action)| matches!(
            action,
            Action::Buy { unit, currency, .. }
                if unit == "missionary" && currency == "faith"
        )),
        "a converted Holy Site must not buy the rival's Missionary"
    );

    let city_state = game.cities.get_mut(&city).unwrap();
    city_state.pressure.clear();
    city_state
        .pressure
        .insert("Home Faith".to_string(), 1_000.0);
    ai.cities(&mut game, 0);
    assert!(game.log.iter().any(|(_, action)| matches!(
        action,
        Action::Buy { city: bought_at, unit, currency, .. }
            if *bought_at == city && unit == "missionary" && currency == "faith"
    )));
    let missionary = game
        .units
        .values()
        .find(|unit| unit.owner == 0 && unit.kind == "missionary")
        .expect("the same Holy Site can buy once its majority is ours");
    assert_eq!(missionary.religion.as_deref(), Some("Home Faith"));
}

#[test]
fn the_science_building_partition_keeps_cheapest_first_inside_each_half() {
    // The gene must move exactly one boundary. Everything else about the
    // order — cheapest-first among science buildings, cheapest-first among
    // the rest — has to survive, because that policy was priced elsewhere
    // and this gene is not the place to relitigate it.
    let rules = crate::rules::Rules::shipped();
    let science: Vec<Name> = rules
        .buildings
        .iter()
        .filter(|(_, s)| !s.wonder && s.yields.science > 0.0)
        .map(|(b, _)| *b)
        .collect();
    assert!(
        !science.is_empty(),
        "the partition is vacuous if no building makes science"
    );

    // Order a mixed set the way the sort does, with the gene on and off.
    let order = |on: bool| -> Vec<(i64, Name)> {
        let mut v: Vec<(i64, Name)> = rules
            .buildings
            .iter()
            .filter(|(_, s)| !s.wonder)
            .map(|(b, s)| (s.cost as i64, *b))
            .collect();
        let makes = |b: &Name| on && rules.buildings[b].yields.science > 0.0;
        v.sort_by(|a, b| {
            makes(&b.1)
                .cmp(&makes(&a.1))
                .then_with(|| a.0.cmp(&b.0))
                .then_with(|| a.1.cmp(&b.1))
        });
        v
    };

    let off = order(false);
    let on = order(true);

    // 1. Same multiset — the gene adds and removes nothing.
    let mut a: Vec<Name> = off.iter().map(|e| e.1).collect();
    let mut b: Vec<Name> = on.iter().map(|e| e.1).collect();
    a.sort();
    b.sort();
    assert_eq!(a, b, "the partition must not add or drop a candidate");

    // 2. Every science building precedes every non-science building.
    let first_plain = on
        .iter()
        .position(|(_, b)| rules.buildings[b].yields.science <= 0.0)
        .expect("some building makes no science");
    assert!(
        on[first_plain..]
            .iter()
            .all(|(_, b)| rules.buildings[b].yields.science <= 0.0),
        "science buildings must form one unbroken prefix"
    );

    // 3. Cheapest-first still holds INSIDE each half.
    for half in [&on[..first_plain], &on[first_plain..]] {
        assert!(
            half.windows(2).all(|w| w[0].0 <= w[1].0),
            "cheapest-first must survive inside each half"
        );
    }
}

#[test]
fn skipping_the_prophet_race_refuses_the_reservation_the_prize_gate_leaves_open() {
    // `pursue_religion` gates missionary purchases and the secret-society
    // lean. It does NOT gate the Holy Site reservation, so clearing only
    // pursuit still pays the entry fee. This pins
    // the other half: `skip_prophet_race` must refuse the district itself.
    let mut game = Game::new_full(
        1,
        24,
        16,
        crate::rng::fixture_seed("HOLYSITE", 91_773),
        120,
        0,
        false,
    );
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let capital = game.player_city_ids(0)[0];
    game.players[0].techs.insert(crate::name!("astrology"));
    game.cities
        .get_mut(&capital)
        .unwrap()
        .buildings
        .push(crate::name!("monument"));

    let holy = |ai: &BasicAi, game: &Game| {
        matches!(
            ai.pick_item(game, 0, capital, 2, 2, 2, 2, 10, 5, 5, 5)
                .expect("city has a production choice"),
            Item::District { ref district, .. }
                if game.district_family(district) == "holy_site"
        )
    };

    let shipped = BasicAi::new();
    assert!(
        !shipped.skip_prophet_race,
        "off unless the advanced bridge asks for it"
    );
    assert!(
        holy(&shipped, &game),
        "the frozen native controller opens the prophet race here"
    );

    let mut treated = BasicAi::new();
    treated.skip_prophet_race = true;
    assert!(
        !holy(&treated, &game),
        "the treated empire must not spend the slot contesting a race it \
             has already decided to sit out"
    );
}

#[test]
fn entering_the_prophet_race_puts_the_first_holy_site_ahead_of_the_lane() {
    let mut game = Game::new_full(
        1,
        24,
        16,
        crate::rng::fixture_seed("HOLYSITE", 91_775),
        120,
        0,
        false,
    );
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let capital = game.player_city_ids(0)[0];
    for tech in ["astrology", "writing", "currency"] {
        game.players[0].techs.insert(Name::new(tech));
    }
    game.cities
        .get_mut(&capital)
        .unwrap()
        .buildings
        .push(crate::name!("monument"));

    let picks = |ai: &BasicAi, game: &Game, family: &str| {
        matches!(
            ai.pick_item(game, 0, capital, 2, 2, 2, 2, 10, 5, 5, 5)
                .expect("city has a production choice"),
            Item::District { ref district, .. }
                if game.district_family(district) == family
        )
    };

    let shipped = BasicAi::new();
    assert!(
        !shipped.enter_prophet_race,
        "off unless the advanced bridge asks for it"
    );
    assert!(
        picks(&shipped, &game, "campus"),
        "the shipped order opens with the Campus the genome weights first"
    );

    let mut treated = BasicAi::new();
    treated.enter_prophet_race = true;
    assert!(
        picks(&treated, &game, "holy_site"),
        "while the race is open the first Holy Site outranks the lane"
    );

    // The skip flag is the operator's word, and it still wins.
    let mut skipped = BasicAi::new();
    skipped.enter_prophet_race = true;
    skipped.skip_prophet_race = true;
    assert!(
        !picks(&skipped, &game, "holy_site"),
        "a seat told to sit the race out does not enter it"
    );
}

#[test]
fn religionless_empire_skips_holy_site_after_prophet_slots_close() {
    let mut game = Game::new_full(4, 30, 18, 91_774, 120, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = game.player_city_ids(0)[0];
    game.players[0].techs.insert(crate::name!("astrology"));
    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("monument"));
    for player in 1..=game.max_religions() {
        game.players[player].religion = Some(format!("Claimed Faith {player}"));
    }
    assert_eq!(game.religions_founded(), game.max_religions());

    let choice = BasicAi::new()
        .pick_item(&game, 0, city, 1, 1, 1, 1, 10, 5, 5, 5)
        .expect("the city has non-religious development available");
    assert!(
        !matches!(
            choice,
            Item::District { ref district, .. }
                if game.district_family(district) == "holy_site"
        ),
        "a closed Prophet race must not consume a district slot: {choice:?}"
    );
}

fn pending_prophet_after_preferred_beliefs_are_taken() -> Game {
    let mut game = Game::new_full(3, 26, 16, 91_773, 120, 0, false);
    for pid in 0..3 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        let position = game.units[&settler].pos;
        game.found_city_for(pid, position, None);
    }
    game.players[0].religion = Some("First Faith".to_string());
    game.players[0].religion_beliefs = vec!["work_ethic".to_string(), "tithe".to_string()];
    game.players[1].religion = Some("Second Faith".to_string());
    game.players[1].religion_beliefs = vec!["choral_music".to_string(), "world_church".to_string()];

    let holy_city = game.player_city_ids(2)[0];
    let center = game.cities[&holy_city].pos;
    let holy_site = game.cities[&holy_city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != center)
        .unwrap();
    game.map.tiles.get_mut(&holy_site).unwrap().district = Some(crate::name!("holy_site"));
    game.cities
        .get_mut(&holy_city)
        .unwrap()
        .districts
        .insert(crate::name!("holy_site"), holy_site);
    game.players[2].prophet_pending = true;
    game.current = 2;
    game
}

#[test]
fn prophet_uses_remaining_data_backed_beliefs_after_preferred_pairs_are_taken() {
    let mut game = pending_prophet_after_preferred_beliefs_are_taken();

    BasicAi::new().research(&mut game, 2);

    assert!(game.players[2].religion.is_some());
    assert!(!game.players[2].prophet_pending);
    assert!(game.players[2]
        .religion_beliefs
        .iter()
        .any(|belief| belief == "cross_cultural_dialogue"));
}

#[test]
fn earned_prophet_founds_without_pursuing_the_religious_race() {
    let mut game = pending_prophet_after_preferred_beliefs_are_taken();
    let mut ai = BasicAi::new();
    ai.pursue_religion = false;
    ai.skip_prophet_race = true;
    ai.research(&mut game, 2);

    assert!(game.players[2].religion.is_some());
    assert!(!game.players[2].prophet_pending);
    assert!(!ai.pursue_religion);
    assert!(ai.skip_prophet_race);
}

#[test]
fn nonreligious_seat_without_a_prophet_does_not_found() {
    let mut game = pending_prophet_after_preferred_beliefs_are_taken();
    game.players[2].prophet_pending = false;
    let mut ai = BasicAi::new();
    ai.pursue_religion = false;
    ai.research(&mut game, 2);

    assert!(game.players[2].religion.is_none());
}

#[test]
fn settler_keeps_a_reachable_colony_target_across_water() {
    let (mut g, source, target) = island_colony_game(1);
    g.players[0]
        .techs
        .extend([crate::name!("sailing"), crate::name!("shipbuilding")]);
    let settler = g.spawn_test_unit("settler", 0, source);
    let mut ai = BasicAi::new();

    assert!(ai.settler_step(&mut g, 0, settler));
    assert_eq!(ai.settler_targets.get(&settler), Some(&target));
    assert!(g
        .map
        .get(g.units[&settler].pos)
        .is_some_and(|tile| g.rules.is_water(tile)));
}

#[test]
fn island_exploration_prefers_the_known_foreign_landfall() {
    let (mut g, source, target) = island_colony_game(1);
    g.players[0].techs.insert(crate::name!("sailing"));
    let landfall = g
        .nbrs(target)
        .into_iter()
        .find(|pos| g.map.get(*pos).is_some_and(|tile| g.rules.is_water(tile)))
        .expect("the island has a coastal approach");
    let start = g
        .nbrs(landfall)
        .into_iter()
        .find(|pos| {
            g.map.get(*pos).is_some_and(|tile| g.rules.is_water(tile)) && g.wdist(*pos, target) == 2
        })
        .expect("the approach has an outer water tile");
    let galley = g.spawn_test_unit("galley", 0, start);
    let empty_water = g
        .nbrs(start)
        .into_iter()
        .find(|pos| {
            *pos != landfall
                && g.map.get(*pos).is_some_and(|tile| g.rules.is_water(tile))
                && g.wdist(*pos, target) > g.unit_sight(galley)
        })
        .expect("the approach has an equally close empty-water choice");
    let all_tiles: Vec<Pos> = g.map.tiles.keys().copied().collect();
    g.players[0].explored.extend(all_tiles);
    g.players[0].explored.remove(&landfall);
    g.players[0].explored.remove(&empty_water);
    let mut ai = BasicAi::new();
    ai.island_exploration = true;

    assert!(ai.island_exploration_active(&g, 0));
    assert_eq!(ai.exploration_goal(&g, 0, galley, false), Some(landfall));
    assert_ne!(source, target, "the fixture has distinct landmasses");
}

#[test]
fn settler_routes_to_distant_land_before_embarkation() {
    let mut game = Game::new_full(1, 18, 10, 91_002, 120, 0, false);
    let founding_settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    let source = game.units[&founding_settler].pos;
    let target = game
        .map
        .tiles
        .keys()
        .copied()
        .max_by_key(|position| (game.wdist(source, *position), *position))
        .unwrap();
    assert!(game.wdist(source, target) > 8);
    game.apply(
        0,
        &Action::FoundCity {
            unit: founding_settler,
        },
    )
    .unwrap();
    for (position, tile) in &mut game.map.tiles {
        tile.feature = None;
        tile.resource = None;
        tile.owner_city = None;
        if position != source && position != target {
            tile.terrain = crate::name!("mountain");
            tile.improvement = Some(crate::name!("mountain_tunnel"));
        }
    }
    game.map.tiles.get_mut(&source).unwrap().terrain = crate::name!("plains");
    game.map.tiles.get_mut(&target).unwrap().terrain = crate::name!("grassland");
    let settler = game.spawn_test_unit("settler", 0, source);
    let mut ai = BasicAi::new();

    assert!(!ai.has_practical_settle_site(&game, 0));
    assert!(ai.settler_step(&mut game, 0, settler));
    assert_eq!(ai.settler_targets.get(&settler), Some(&target));
    assert_ne!(game.units[&settler].pos, source);
}

#[test]
fn settler_search_looks_past_an_unreachable_high_value_prefix() {
    let mut game = Game::new_full(1, 30, 18, 91_005, 120, 0, false);
    let founding_settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    let source = game.units[&founding_settler].pos;
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("coast");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
        tile.owner_city = None;
        tile.cliff_edges = [false; 6];
    }
    game.map.tiles.get_mut(&source).unwrap().terrain = crate::name!("desert");
    game.apply(
        0,
        &Action::FoundCity {
            unit: founding_settler,
        },
    )
    .unwrap();

    let target = game
        .wdisk(source, 4)
        .into_iter()
        .filter(|position| game.wdist(source, *position) == 4)
        .min()
        .expect("the source has a site four tiles away");
    let mut corridor = vec![source];
    let mut cursor = source;
    while cursor != target {
        cursor = game
            .nbrs(cursor)
            .into_iter()
            .filter(|next| game.wdist(*next, target) < game.wdist(cursor, target))
            .min()
            .expect("a direct corridor reaches the target");
        corridor.push(cursor);
    }
    for position in &corridor {
        game.map.tiles.get_mut(position).unwrap().terrain = crate::name!("desert");
    }

    let island_center = game
        .map
        .tiles
        .keys()
        .copied()
        .max_by_key(|position| (game.wdist(source, *position), *position))
        .expect("the map has a distant island center");
    let island: Vec<Pos> = game
        .wdisk(island_center, 5)
        .into_iter()
        .filter(|position| {
            game.wdist(source, *position) >= 9
                && corridor
                    .iter()
                    .all(|land| game.wdist(*land, *position) >= 2)
        })
        .collect();
    assert!(island.len() > 40);
    for position in island {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.feature = Some(crate::name!("forest"));
        tile.hills = true;
    }

    let settler = game.spawn_test_unit("settler", 0, source);
    let ai = BasicAi::new();
    let mut ranked: Vec<(Pos, f64)> = game
        .wdisk(source, game.map.width + game.map.height)
        .into_iter()
        .filter(|position| ai.valid_settle_site(&game, 0, *position))
        .map(|position| {
            let score = ai.settle_value(&game, position)
                - ai.w.settle_dist * game.wdist(source, position) as f64;
            (position, score)
        })
        .collect();
    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap().then(a.0.cmp(&b.0)));
    assert!(ranked
        .iter()
        .take(40)
        .all(|(position, _)| game.route_step(settler, *position, 0).is_none()));

    assert_eq!(
        ai.best_reachable_settle_site(&game, 0, settler, game.map.width + game.map.height,)
            .map(|(position, _)| position),
        Some(target),
    );
}

#[test]
fn settler_retargets_a_natural_wonder_for_an_open_island_site() {
    let (mut game, _source, wonder) = island_colony_game(1);
    let alternate = game
        .nbrs(wonder)
        .into_iter()
        .find(|position| game.map.get(*position).is_some())
        .expect("the island has a neighboring shore tile");
    game.map.tiles.get_mut(&alternate).unwrap().terrain = crate::name!("grassland");
    game.map.tiles.get_mut(&wonder).unwrap().feature = Some(crate::name!("pantanal"));

    let settler = game.spawn_test_unit("settler", 0, wonder);
    let mut ai = BasicAi::new();
    ai.settler_targets.insert(settler, wonder);

    assert!(
        !ai.valid_settle_site(&game, 0, wonder),
        "a natural wonder looks like land but may not become a city"
    );
    assert!(!game.can_found_city(settler));
    assert!(ai.settler_step(&mut game, 0, settler));
    assert_eq!(ai.settler_targets.get(&settler), Some(&alternate));
    assert_eq!(game.units[&settler].pos, alternate);
}

#[test]
fn advanced_settle_search_rejects_a_natural_wonder() {
    let (mut game, _source, wonder) = island_colony_game(1);
    game.map.tiles.get_mut(&wonder).unwrap().feature = Some(crate::name!("pantanal"));

    assert!(
        !AdvancedAi::new().any_settle_site(&game, 0),
        "a natural wonder cannot keep the strategic settler gate open"
    );
}

#[test]
fn settler_follows_its_route_past_a_closer_cul_de_sac() {
    let mut game = Game::new_full(1, 20, 14, 91_003, 120, 0, false);
    for unit in game.player_unit_ids(0) {
        game.remove_unit(unit);
    }
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
        tile.owner_city = None;
    }
    let start = game
        .map
        .tiles
        .keys()
        .copied()
        .find(|position| game.wdisk(*position, 4).len() == 61)
        .expect("test map has an interior tile");
    let target = game
        .wdisk(start, 4)
        .into_iter()
        .filter(|position| game.wdist(start, *position) == 4)
        .min()
        .expect("test map has a target four tiles away");
    let trap = game
        .nbrs(start)
        .into_iter()
        .filter(|position| game.wdist(*position, target) < game.wdist(start, target))
        .min_by_key(|position| (game.wdist(*position, target), *position))
        .expect("target has a geometrically closer neighbor");
    for position in game.nbrs(trap) {
        if position != start {
            game.map.tiles.get_mut(&position).unwrap().terrain = crate::name!("mountain");
        }
    }
    let settler = game.spawn_test_unit("settler", 0, start);
    let routed = game
        .route_step(settler, target, 0)
        .expect("the target remains reachable around the cul-de-sac");
    assert_ne!(routed, trap);

    let mut ai = BasicAi::new();
    ai.settler_targets.insert(settler, target);
    assert!(ai.settler_step(&mut game, 0, settler));
    assert!(ai.settler_step(&mut game, 0, settler));
    assert_ne!(
        game.units[&settler].pos, start,
        "the settler must not spend both movement points entering and leaving the trap"
    );
    assert_ne!(game.units[&settler].pos, trap);
}

#[test]
fn generic_pathing_does_not_reverse_its_last_step_in_the_same_turn() {
    let mut game = Game::new_full(1, 20, 14, 91_004, 120, 0, false);
    for unit in game.player_unit_ids(0) {
        game.remove_unit(unit);
    }
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
        tile.owner_city = None;
    }
    let start = game
        .map
        .tiles
        .keys()
        .copied()
        .find(|position| game.wdisk(*position, 4).len() == 61)
        .expect("test map has an interior tile");
    let target = game
        .wdisk(start, 4)
        .into_iter()
        .filter(|position| game.wdist(start, *position) == 4)
        .min()
        .expect("test map has a target four tiles away");
    let trap = game
        .nbrs(start)
        .into_iter()
        .filter(|position| game.wdist(*position, target) < game.wdist(start, target))
        .min_by_key(|position| (game.wdist(*position, target), *position))
        .expect("target has a geometrically closer neighbor");
    for position in game.nbrs(trap) {
        if position != start {
            game.map.tiles.get_mut(&position).unwrap().terrain = crate::name!("mountain");
        }
    }
    let unit = game.spawn_test_unit("warrior", 0, start);
    let ai = BasicAi::new();

    assert!(ai.step_toward(&mut game, 0, unit, target));
    assert_eq!(game.units[&unit].pos, trap);
    assert!(
        !ai.step_toward(&mut game, 0, unit, target),
        "the unit should wait instead of spending its next point returning to its source"
    );
    assert_eq!(game.units[&unit].pos, trap);
}

#[test]
fn naval_escorts_link_to_embarked_settlers() {
    let (mut g, source, _) = island_colony_game(1);
    g.players[0]
        .techs
        .extend([crate::name!("sailing"), crate::name!("shipbuilding")]);
    let water = g
        .nbrs(source)
        .into_iter()
        .find(|pos| g.map.get(*pos).is_some_and(|tile| g.rules.is_water(tile)))
        .unwrap();
    let settler = g.spawn_test_unit("settler", 0, water);
    let galley = g.spawn_test_unit("galley", 0, water);
    BasicAi::new().prepare_unit_formations(&mut g, 0);
    assert_eq!(g.units[&galley].linked_to, Some(settler));
    assert_eq!(g.units[&settler].linked_to, Some(galley));
}

#[test]
fn linked_ship_leads_settler_toward_the_persistent_colony_target() {
    let (mut g, source, target) = island_colony_game(1);
    g.players[0]
        .techs
        .extend([crate::name!("sailing"), crate::name!("shipbuilding")]);
    let settler = g.spawn_test_unit("settler", 0, source);
    let galley = g.spawn_test_unit("galley", 0, source);
    let mut ai = BasicAi::new();
    ai.prepare_unit_formations(&mut g, 0);

    assert!(!ai.settler_step(&mut g, 0, settler));
    assert_eq!(ai.settler_targets.get(&settler), Some(&target));
    assert!(ai.military_step(&mut g, 0, galley));
    assert_eq!(g.units[&galley].pos, g.units[&settler].pos);
    assert!(g
        .map
        .get(g.units[&galley].pos)
        .is_some_and(|tile| g.rules.is_water(tile)));
}

#[test]
fn escorted_settler_unlinks_at_the_destination_coast_and_founds_the_colony() {
    let (mut g, source, target) = island_colony_game(1);
    g.players[0]
        .techs
        .extend([crate::name!("sailing"), crate::name!("shipbuilding")]);
    let settler = g.spawn_test_unit("settler", 0, source);
    let galley = g.spawn_test_unit("galley", 0, source);
    let mut ai = BasicAi::new();
    ai.prepare_unit_formations(&mut g, 0);

    for _ in 0..12 {
        for uid in [settler, galley] {
            if let Some(unit) = g.units.get_mut(&uid) {
                unit.moves_left = 4.0;
                unit.attacks_left = 1;
                unit.acted = false;
                unit.moved = false;
                unit.fortified = false;
            }
        }
        for _ in 0..8 {
            if !g.units.contains_key(&settler)
                || g.units[&settler].moves_left <= 0.0
                || !ai.settler_step(&mut g, 0, settler)
            {
                break;
            }
        }
        for _ in 0..8 {
            if !g.units.contains_key(&galley)
                || g.units[&galley].moves_left <= 0.0
                || !ai.military_step(&mut g, 0, galley)
            {
                break;
            }
        }
        if !g.units.contains_key(&settler) {
            break;
        }
    }

    assert!(!g.units.contains_key(&settler));
    assert!(g
        .city_at(target)
        .is_some_and(|cid| g.cities[&cid].owner == 0));
}

#[test]
fn ships_intercept_embarked_enemies_instead_of_chasing_inland_targets() {
    let (mut g, source, target) = island_colony_game(2);
    g.at_war.insert((0, 1));
    // The rival's own starting units sit wherever mapgen dropped them, so
    // clear them: this test is about choosing between the two threats it
    // places, not about whichever spawn happens to be nearest.
    for uid in g.player_unit_ids(1) {
        g.units.remove(&uid);
    }
    let water = g
        .nbrs(source)
        .into_iter()
        .find(|pos| g.map.get(*pos).is_some_and(|tile| g.rules.is_water(tile)))
        .unwrap();
    let galley = g.spawn_test_unit("galley", 0, water);
    let enemy_water = g
        .nbrs(water)
        .into_iter()
        .find(|pos| g.map.get(*pos).is_some_and(|tile| g.rules.is_water(tile)))
        .unwrap();
    let embarked = g.spawn_test_unit("settler", 1, enemy_water);
    g.spawn_test_unit("warrior", 1, target);
    let ai = BasicAi::new();
    assert_eq!(
        ai.nearest_enemy_for_unit(&g, 0, galley, &[1]),
        Some(g.units[&embarked].pos)
    );
}

#[test]
fn wounded_units_withdraw_and_finish_recovering_before_rejoining() {
    let mut g = Game::new_full(2, 20, 14, 30, 30, 0, false);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| g.units[uid].kind == "settler")
        .unwrap();
    let warrior = g
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| g.units[uid].kind == "warrior")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();

    let neutral = g
        .map
        .tiles
        .iter()
        .filter(|(_, tile)| {
            tile.owner_city.is_none() && g.rules.is_passable(tile) && !g.rules.is_water(tile)
        })
        .map(|(pos, _)| *pos)
        .find(|pos| {
            g.nbrs(*pos).into_iter().any(|neighbor| {
                let tile = &g.map.tiles[&neighbor];
                tile.owner_city.is_some() && g.rules.is_passable(tile) && !g.rules.is_water(tile)
            })
        })
        .expect("map has neutral land adjacent to the capital's territory");
    {
        let unit = g.units.get_mut(&warrior).unwrap();
        unit.pos = neutral;
        unit.hp = 45;
        unit.moves_left = 2.0;
        unit.acted = false;
        unit.fortified = false;
    }
    // Rebuild occupancy after placing the unit in this controlled setup.
    let snapshot = serde_json::to_value(&g).unwrap();
    let mut g: Game = serde_json::from_value(snapshot).unwrap();
    let mut ai = BasicAi::new();

    assert_eq!(ai.healing_step(&mut g, 0, warrior), Some(true));
    assert!(
        g.unit_heal_rate(warrior) >= 15,
        "unit should seek friendly borders"
    );
    assert!(ai.recovering_units.contains(&warrior));

    // Once safe, it waits instead of immediately marching back out.
    assert_eq!(ai.healing_step(&mut g, 0, warrior), Some(false));
    assert!(g.units[&warrior].fortified);
    g.units.get_mut(&warrior).unwrap().hp = 79;
    assert_eq!(ai.healing_step(&mut g, 0, warrior), Some(false));

    // Recovery mode has hysteresis and releases the unit at 80 HP.
    g.units.get_mut(&warrior).unwrap().hp = 80;
    assert_eq!(ai.healing_step(&mut g, 0, warrior), None);
    assert!(!ai.recovering_units.contains(&warrior));
}

/// A flat, empty two-player board at war, holding exactly three things:
/// our City Center, a Warrior of ours standing two steps outside it, and
/// one enemy Archer two steps beyond that Warrior on the far side.
///
/// The Archer is the whole point. Its shot at a Warrior on 50 hit points
/// is worth about 45 on the average — survivable, and so not a lethal
/// pool — while `game::damage` rolls it at `uniform(0.8, 1.2)`, which
/// kills outright. Returns `(game, our Warrior, its tile, the city)`.
fn a_warrior_under_one_archer() -> (Game, u32, Pos, Pos) {
    let mut game = Game::new_full(2, 20, 14, 91_484, 80, 0, false);
    for unit in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(unit);
    }
    game.map.clear_rivers();
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.district_foundation = None;
        tile.wonder = None;
        tile.owner_city = None;
        tile.hills = false;
        tile.road = 0;
    }
    game.at_war.insert((0, 1));
    game.current = 0;

    let front = game
        .map
        .tiles
        .keys()
        .copied()
        .find(|position| game.wdisk(*position, 4).len() == 61)
        .expect("fixture needs an interior tile with four clear rings");
    let home = game
        .wdisk(front, 2)
        .into_iter()
        .find(|position| game.wdist(*position, front) == 2)
        .expect("fixture needs a tile two steps from the front");
    game.found_city_for(0, home, Some("Refuge".to_string()));
    let shooter = game
        .wdisk(front, 2)
        .into_iter()
        .filter(|position| game.wdist(*position, front) == 2)
        .max_by_key(|position| game.wdist(*position, home))
        .expect("fixture needs a firing position on the far side");

    let ours = game.spawn_test_unit("warrior", 0, front);
    let archer = game.spawn_test_unit("archer", 1, shooter);
    assert!(
        game.attack_reach(archer).contains(&front),
        "the fixture's whole subject is a shot the Archer can take next turn"
    );
    assert!(
        game.reachable(ours).contains(&home),
        "and a City Center the Warrior can reach this turn"
    );
    (game, ours, front, home)
}

/// A Quadrireme has one tile of ranged reach, not two, and must retain a
/// movement point to fire. A wounded land unit that has actually entered
/// that full envelope must recover away from the shore, rather than
/// fortifying for another naval volley.
#[test]
fn a_wounded_land_unit_leaves_a_coast_under_quadrireme_fire() {
    let mut game = Game::new_full(2, 20, 14, 91_485, 80, 0, false);
    for unit in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(unit);
    }
    game.map.clear_rivers();
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.district_foundation = None;
        tile.wonder = None;
        tile.owner_city = None;
        tile.hills = false;
        tile.road = 0;
    }
    game.at_war.insert((0, 1));
    game.current = 0;

    let shore = game
        .map
        .tiles
        .keys()
        .copied()
        .find(|position| game.wdisk(*position, 4).len() == 61)
        .expect("fixture needs an interior shore");
    let water = game.nbrs(shore)[0];
    let water_after_one_move = game
        .nbrs(water)
        .into_iter()
        .find(|position| game.wdist(*position, shore) == 2)
        .expect("fixture needs a coast one move out");
    let water_after_two_moves = game
        .nbrs(water_after_one_move)
        .into_iter()
        .find(|position| game.wdist(*position, shore) == 3)
        .expect("fixture needs a coast two moves out");
    for position in [water, water_after_one_move, water_after_two_moves] {
        game.map.tiles.get_mut(&position).unwrap().terrain = crate::name!("coast");
    }
    let inland = game
        .nbrs(shore)
        .into_iter()
        .find(|position| {
            *position != water
                && game
                    .nbrs(*position)
                    .into_iter()
                    .all(|neighbor| !game.rules.is_water(&game.map.tiles[&neighbor]))
        })
        .expect("fixture needs a land step behind the shore");

    let defender = game.spawn_test_unit("swordsman", 0, shore);
    game.units.get_mut(&defender).unwrap().hp = 45;
    // A friendly observer gives the controller sight of the ship even
    // though its target is two sea steps beyond the wounded defender.
    let observer = game
        .nbrs(water_after_two_moves)
        .into_iter()
        .find(|position| {
            *position != water_after_one_move && !game.rules.is_water(&game.map.tiles[position])
        })
        .expect("fixture needs a land observer next to the ship");
    game.spawn_test_unit("scout", 0, observer);
    let quadrireme = game.spawn_test_unit("quadrireme", 1, water_after_two_moves);
    assert_eq!(
        game.unit_attack_range(quadrireme),
        1,
        "the shipped Quadrireme record has Range=1"
    );
    assert!(
        game.attack_reach(quadrireme).contains(&shore),
        "two sea moves leave the one point needed for its adjacent shot"
    );
    assert!(
        !game.attack_reach(quadrireme).contains(&inland),
        "the tile behind that shore is outside its actual firing envelope"
    );

    let probe = BasicAi::new();
    let envelopes = probe.enemy_attack_envelopes(&game, 0);
    let incoming = BasicAi::evacuation_incoming_damage(&game, 0, defender, shore, &envelopes);
    assert!(
        incoming > 0.0 && incoming < f64::from(game.units[&defender].hp),
        "the current volley is harmful but not an average lethal pool: {incoming:.1}"
    );

    let mut ai = BasicAi::new();
    assert_eq!(ai.healing_step(&mut game, 0, defender), Some(true));
    let retreat = game.units[&defender].pos;
    assert_ne!(
        retreat, shore,
        "the wounded unit does not fortify on the shore"
    );
    assert!(
        game.nbrs(retreat)
            .into_iter()
            .all(|position| !game.rules.is_water(&game.map.tiles[&position])),
        "the recovery step leaves the exposed coast"
    );
    let envelopes = ai.enemy_attack_envelopes(&game, 0);
    assert!(
        envelopes.iter().all(|(_, reach)| !reach.contains(&retreat)),
        "the recovery tile is outside every observed next-turn attack envelope"
    );
}

/// ★★★ THE CASE BOTH EXISTING TESTS ABOVE MISS. `withdraw_hp` is 45 and
/// the pool this tile invites is 45, so a Warrior on 50 is healthy by the
/// floor and safe by the lethal-pool test — and dies to the next shot
/// whenever the engine rolls above its average, which is half the time.
/// See `one_shot_recovery`.
#[test]
fn a_unit_one_blow_from_death_withdraws_though_the_static_floor_calls_it_healthy() {
    let (mut game, ours, front, home) = a_warrior_under_one_archer();
    game.units.get_mut(&ours).unwrap().hp = 50;

    let probe = BasicAi::new();
    let envelopes = probe.enemy_attack_envelopes(&game, 0);
    let pool = BasicAi::evacuation_incoming_damage(&game, 0, ours, front, &envelopes);
    let blow = BasicAi::killing_blow(&game, 0, ours, front, &envelopes);
    assert!(
        pool < 50.0,
        "the expected pool is {pool:.1}: a survivable tile"
    );
    assert!(
        blow >= 50.0,
        "the top of that same roll is {blow:.1}: a dead Warrior"
    );

    // Untreated, no part of the controller has anything to say about it.
    let mut control = game.clone();
    let mut untreated = BasicAi::new();
    assert_eq!(
        untreated.healing_step(&mut control, 0, ours),
        None,
        "50 is above the withdrawal floor and the pool is not lethal"
    );
    assert_eq!(control.units[&ours].pos, front);
    assert!(!untreated.recovering_units.contains(&ours));

    let mut ai = BasicAi::new();
    ai.one_shot_recovery = true;
    assert_eq!(ai.healing_step(&mut game, 0, ours), Some(true));
    assert!(ai.recovering_units.contains(&ours));
    assert_eq!(
        game.units[&ours].pos, home,
        "the Warrior spends the turn getting inside the City Center"
    );
    let envelopes = ai.enemy_attack_envelopes(&game, 0);
    assert_eq!(
        BasicAi::killing_blow(&game, 0, ours, home, &envelopes),
        0.0,
        "a garrison is not a target: the district takes the shot"
    );
}

/// The other half: a unit part-way through recovery, on ground an Archer
/// has since come within reach of.
///
/// Nothing above answers this. The pool is not lethal, so no retreat is
/// created; the tile is threatened, so the stationary hold refuses it; and
/// the route hop is one tile toward distant healing ground, chosen without
/// asking whether the same shooter still covers where it lands. What is
/// left is the fortify-and-heal fallback, under the Archer.
/// See `one_shot_recovery`.
#[test]
fn a_healing_unit_leaves_ground_the_enemy_can_still_reach() {
    let (mut game, ours, front, home) = a_warrior_under_one_archer();
    // Withdrawn at 45 several turns ago and healing since: still in
    // recovery, and now well clear of one Archer shot.
    game.units.get_mut(&ours).unwrap().hp = 60;

    let probe = BasicAi::new();
    let envelopes = probe.enemy_attack_envelopes(&game, 0);
    let pool = BasicAi::evacuation_incoming_damage(&game, 0, ours, front, &envelopes);
    assert!(
        pool > 0.0 && pool < 60.0,
        "a threatened but survivable tile: {pool:.1}"
    );
    assert!(
        BasicAi::killing_blow(&game, 0, ours, front, &envelopes) < 60.0,
        "and not one blow from death either: this half stands on its own"
    );

    let mut control = game.clone();
    let mut untreated = BasicAi::new();
    untreated.recovering_units.insert(ours);
    assert_eq!(untreated.healing_step(&mut control, 0, ours), Some(true));
    let control_envelopes = untreated.enemy_attack_envelopes(&control, 0);
    let rested = control.units[&ours].pos;
    assert!(
        BasicAi::killing_blow(&control, 0, ours, rested, &control_envelopes) > 0.0,
        "untreated, the Warrior heals on where the Archer can still shoot it"
    );

    let mut ai = BasicAi::new();
    ai.one_shot_recovery = true;
    ai.recovering_units.insert(ours);
    assert_eq!(ai.healing_step(&mut game, 0, ours), Some(true));
    assert_eq!(
        game.units[&ours].pos, home,
        "treated, it finishes healing on ground the Archer cannot reach"
    );
    assert!(
        ai.recovering_units.contains(&ours),
        "and it is still recovering: leaving the tile is not rejoining the line"
    );
}

/// ⚠⚠ THE STAMP CANNOT TELL TWO DIVERGENT BOARDS APART, WHICH IS WHY THE
/// CACHE MUST NOT BE SHARED ACROSS THEM.
///
/// `Tiles::epoch` documents "two reads of the same epoch saw the same map".
/// That holds for one map's history and NOT across clones: two siblings that
/// each take one write carry the same counter and different content, as this
/// pins. `healing_target_stamp` is built from four such counters.
///
/// So the safe-healing entry is deliberately **per AI instance and reset on
/// clone** (see `FreshOnClone`). A shared entry keyed on these counters could
/// hand one board's target sets to another board that merely happens to have
/// taken the same number of writes.
#[test]
fn sibling_clones_share_a_healing_stamp_so_the_cache_may_not_cross_boards() {
    let game = Game::new_full(3, 24, 16, 91_485, 120, 1, false);
    let mut a = game.clone();
    let mut b = game.clone();
    let positions: Vec<Pos> = game.map.tiles.keys().copied().take(2).collect();
    // One mutable open each, on DIFFERENT tiles: same counter, different map.
    a.map.tiles.get_mut(&positions[0]);
    b.map.tiles.get_mut(&positions[1]);
    assert_eq!(
        a.healing_target_stamp(),
        b.healing_target_stamp(),
        "these boards differ; if the stamp ever starts distinguishing them, \
             the FreshOnClone reset below may be relaxed"
    );
    // And the reset is what keeps that harmless: a cloned AI starts empty.
    let ai = BasicAi::new();
    let envelopes = std::sync::Arc::new(Vec::new());
    let first = ai.safe_healing_targets(&a, 0, &envelopes);
    let cloned_ai = ai.clone();
    assert!(
        !std::sync::Arc::ptr_eq(&first, &cloned_ai.safe_healing_targets(&b, 0, &envelopes)),
        "a cloned AI must not answer board B from board A's entry"
    );
}

/// The target scan is deliberately shared by the cloned boards a frontier
/// planner creates, but no state it reads may cross that boundary stale.
#[test]
fn safe_healing_targets_share_clones_and_refresh_every_board_input() {
    let mut game = Game::new_full(3, 24, 16, 91_485, 120, 1, false);
    for pid in [0, 1] {
        game.current = pid;
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|uid| game.units[uid].kind == "settler")
            .expect("each major begins with a settler");
        game.apply(pid, &Action::FoundCity { unit: settler })
            .expect("found the fixture capital");
    }
    assert!(game.at_war.insert((0, 1)), "the fixture begins at peace");
    let hostile_city = game
        .cities
        .values()
        .find(|city| city.owner == 1)
        .map(|city| city.id)
        .expect("the second major begins with a city");
    let minor = game
        .players
        .iter()
        .find(|player| player.is_minor && !player.is_barbarian)
        .map(|player| player.id)
        .expect("the fixture seats one city-state");
    // Keep the identical table on every board below. That makes every
    // refresh prove the board stamp rather than merely a new envelope.
    let envelopes = std::sync::Arc::new(Vec::new());
    let ai = BasicAi::new();
    let first = ai.safe_healing_targets(&game, 0, &envelopes);
    assert!(std::sync::Arc::ptr_eq(
        &first,
        &ai.safe_healing_targets(&game, 0, &envelopes)
    ));
    let clone = game.clone();
    assert!(std::sync::Arc::ptr_eq(
        &first,
        &ai.safe_healing_targets(&clone, 0, &envelopes)
    ));

    let assert_refresh = |label: &str, board: &Game| {
        let before = ai.safe_healing_targets(&game, 0, &envelopes);
        let warm = ai.safe_healing_targets(board, 0, &envelopes);
        assert!(
            !std::sync::Arc::ptr_eq(&before, &warm),
            "{label}: the changed board reused the original target set"
        );
        let cold = BasicAi::new().safe_healing_targets(board, 0, &envelopes);
        assert_eq!(
            warm.as_ref(),
            cold.as_ref(),
            "{label}: the warm cache differs from a fresh scan"
        );
    };

    let mut map_changed = game.clone();
    let owned_tile = map_changed
        .map
        .tiles
        .keys()
        .copied()
        .find(|position| {
            map_changed.city_at(*position).is_none()
                && map_changed.map.tiles[position].owner_city == Some(hostile_city)
        })
        .expect("the hostile capital owns at least one non-centre tile");
    map_changed
        .map
        .tiles
        .get_mut(&owned_tile)
        .unwrap()
        .owner_city = None;
    assert_refresh("tile ownership", &map_changed);

    let mut city_changed = game.clone();
    city_changed.cities.get_mut(&hostile_city).unwrap().wall_hp += 1;
    assert_refresh("city strike state", &city_changed);

    let mut player_changed = game.clone();
    player_changed
        .players
        .get_mut(0)
        .unwrap()
        .envoys
        .push((minor, 3));
    assert_refresh("suzerain input", &player_changed);

    let mut diplomacy_changed = game.clone();
    assert!(
        diplomacy_changed.at_war.insert((0, 2)),
        "the added war must change diplomacy"
    );
    assert_refresh("war ledger", &diplomacy_changed);

    let different_envelopes = std::sync::Arc::new(Vec::new());
    let original = ai.safe_healing_targets(&game, 0, &envelopes);
    let different = ai.safe_healing_targets(&game, 0, &different_envelopes);
    assert!(
        !std::sync::Arc::ptr_eq(&original, &different),
        "a different hostile coverage table reused its target set"
    );
}

/// The pool and the largest blow are different questions, and the pool
/// answers the survival one wrongly in both directions: three Warriors
/// that cannot kill anything add up to a lethal tile, while the single
/// Archer that can kill outright is a fraction of their sum.
#[test]
fn the_incoming_pool_and_the_killing_blow_are_different_readings() {
    let (mut game, ours, front, _home) = a_warrior_under_one_archer();
    game.units.get_mut(&ours).unwrap().hp = 60;
    for position in game.nbrs(front) {
        if game.unit_ids_at(position).is_empty() && game.city_at(position).is_none() {
            game.spawn_test_unit("warrior", 1, position);
        }
    }

    let envelopes = BasicAi::new().enemy_attack_envelopes(&game, 0);
    let pool = BasicAi::evacuation_incoming_damage(&game, 0, ours, front, &envelopes);
    let blow = BasicAi::killing_blow(&game, 0, ours, front, &envelopes);
    assert!(
        pool > blow,
        "a pool of {pool:.1} is not one blow of {blow:.1}"
    );
    assert!(
        blow <= MAX_SINGLE_BLOW,
        "no single blow exceeds what the engine can deal: {blow:.1}"
    );
}

/// The gene ships off, and it is registered where a native screen can
/// price it. `production_and_opt_in_rows_are_real` guards the row itself.
#[test]
fn one_shot_recovery_is_an_off_by_default_native_gene() {
    assert!(!BasicAi::new().one_shot_recovery);
    let gene = GENES
        .iter()
        .find(|gene| gene.field == "one_shot_recovery")
        .expect("the gene is registered as a native opt-in");
    assert_eq!(gene.tag, "one-shot-recovery");
    assert!(gene.opt_in());
}

/// One major with a capital, plus a fabricated barbarian warrior on an
/// open tile adjacent to the major's warrior. Returns (game, warrior,
/// barb warrior).
/// One city, one warrior, and a single Barbarian raider standing next to
/// the city — the shape every settler lost on the live ladder was built
/// into.
fn barbarian_at_the_gates_game(seed: u64) -> (Game, u32, u32) {
    let mut g = Game::new_full(1, 20, 14, seed, 60, 0, true);
    let barb_pid = g.barb_pid.unwrap();
    // Organic camps and their garrisons would add besiegers this test does
    // not control; the staged raider must be the only one in reach.
    for unit in g
        .units
        .values()
        .filter(|unit| unit.owner == barb_pid)
        .map(|unit| unit.id)
        .collect::<Vec<_>>()
    {
        g.remove_unit(unit);
    }
    let camps: Vec<Pos> = g.barb_camps.keys().copied().collect();
    for camp in camps {
        g.barb_camps.remove(&camp);
        g.barb_naval_camps.remove(&camp);
        g.barb_camp_guards.remove(&camp);
        let tile = g.map.tiles.get_mut(&camp).unwrap();
        if tile.improvement.as_deref() == Some("barbarian_camp") {
            tile.improvement = None;
        }
    }
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|id| g.units[id].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = g.player_city_ids(0)[0];
    let cpos = g.cities[&city].pos;
    let open = g
        .nbrs(cpos)
        .into_iter()
        .find(|p| {
            let t = &g.map.tiles[p];
            g.rules.is_passable(t)
                && !g.rules.is_water(t)
                && g.unit_ids_at(*p).is_empty()
                && g.city_at(*p).is_none()
        })
        .expect("open land tile next to the city");
    let template = g
        .player_unit_ids(0)
        .into_iter()
        .find(|id| g.units[id].kind == "warrior")
        .unwrap();
    let mut barb = g.units[&template].clone();
    barb.id = g.next_id;
    g.next_id += 1;
    barb.owner = barb_pid;
    barb.pos = open;
    let bid = barb.id;
    g.units.insert(bid, barb);
    // Round-trip to rebuild occupancy after the manual insert.
    let snapshot = serde_json::to_value(&g).unwrap();
    let g: Game = serde_json::from_value(snapshot).unwrap();
    (g, city, bid)
}

#[test]
fn barbarian_alert_suppresses_an_available_trader() {
    let (mut g, city, _raider) = barbarian_at_the_gates_game(82);
    for uid in g.player_unit_ids(0) {
        g.remove_unit(uid);
    }
    let home = g.cities[&city].pos;
    let second_center = g
        .map
        .tiles
        .iter()
        .find(|(pos, tile)| {
            (4..=15).contains(&g.wdist(home, **pos))
                && tile.owner_city.is_none()
                && g.rules.is_passable(tile)
                && !g.rules.is_water(tile)
                && g.unit_ids_at(**pos).is_empty()
        })
        .map(|(pos, _)| *pos)
        .expect("the test map has a second city site");
    let settler = g.spawn_test_unit("settler", 0, second_center);
    g.current = 0;
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    g.players[0].civics.insert(crate::name!("foreign_trade"));
    // One local guard is enough to make this a trade-suppression test
    // rather than a second production-floor test.
    g.spawn_test_unit("warrior", 0, home);

    assert!(BasicAi::should_add_trader(&g, 0, 0));
    assert!(BasicAi::barbarian_trade_risk(&g, 0));
    assert!(!BasicAi::should_add_trader_safely(&g, 0, 0));
    let trader = Item::Unit {
        unit: crate::name!("trader"),
    };
    let choice = BasicAi::new().pick_item(&g, 0, city, 2, 1, 4, 0, 0, 6, 3, 3);
    assert_ne!(
        choice,
        Some(trader),
        "a nearby raider must displace the Trader"
    );
}

/// The live bridge imports a visible Firaxis Scout without the native
/// camp-scout record, because the host does not expose that relationship.
/// It therefore reaches the normal local-raider alarm. The regression was
/// not threat recognition: the opening book simply skipped the alarm and
/// left a nearly-finished Settler in place while civilians waited out the
/// Scout. A first defender must take that queue.
/// On a live board the opening's defense displaces a queued Settler at
/// most `OPENING_DEFENSE_SETTLER_PREEMPTS` times; then the Settler keeps
/// the queue. A native board keeps yielding.
#[test]
fn a_live_opening_keeps_its_settler_after_the_defense_cap() {
    let settler = Item::Unit {
        unit: crate::name!("settler"),
    };
    for (live, preempts, yields) in [(true, 0, true), (true, 2, false), (false, 2, true)] {
        let (mut g, city, scout) = barbarian_at_the_gates_game(91_503);
        for uid in g.player_unit_ids(0) {
            g.remove_unit(uid);
        }
        g.units.get_mut(&scout).unwrap().kind = crate::name!("scout");
        g.cities.get_mut(&city).unwrap().pop = 2;
        if live {
            let pos = g.cities[&city].pos;
            std::sync::Arc::make_mut(&mut g.host_observed).insert(pos);
        }
        g.apply(
            0,
            &Action::Produce {
                city,
                item: settler.clone(),
            },
        )
        .unwrap();
        let mut ai = BasicAi::new();
        ai.opening_defense_preempts = preempts;
        ai.cities(&mut g, 0);
        let kept = g.cities[&city].queue.first() == Some(&settler);
        assert_eq!(!kept, yields, "live {live}, preempts {preempts}");
        if live && yields {
            assert_eq!(ai.opening_defense_preempts, preempts + 1);
        }
    }
}

/// `opening-defense-preempts-once`: with the gene a live opening keeps its
/// Settler after one displacement; without it the second still yields.
#[test]
fn a_live_opening_keeps_its_settler_after_one_preempt_with_the_gene() {
    let settler = Item::Unit {
        unit: crate::name!("settler"),
    };
    for (gene, preempts, yields) in [(true, 0, true), (true, 1, false), (false, 1, true)] {
        let (mut g, city, scout) = barbarian_at_the_gates_game(91_503);
        for uid in g.player_unit_ids(0) {
            g.remove_unit(uid);
        }
        g.units.get_mut(&scout).unwrap().kind = crate::name!("scout");
        g.cities.get_mut(&city).unwrap().pop = 2;
        let pos = g.cities[&city].pos;
        std::sync::Arc::make_mut(&mut g.host_observed).insert(pos);
        g.apply(
            0,
            &Action::Produce {
                city,
                item: settler.clone(),
            },
        )
        .unwrap();
        let mut ai = BasicAi::new();
        ai.opening_defense_preempts_once = gene;
        ai.opening_defense_preempts = preempts;
        ai.cities(&mut g, 0);
        let kept = g.cities[&city].queue.first() == Some(&settler);
        assert_eq!(!kept, yields, "gene {gene}, preempts {preempts}");
    }
}

#[test]
fn opening_queue_yields_to_an_unmarked_live_barbarian_scout() {
    let (mut native, native_city, native_scout) = barbarian_at_the_gates_game(91_502);
    for uid in native.player_unit_ids(0) {
        native.remove_unit(uid);
    }
    native.units.get_mut(&native_scout).unwrap().kind = crate::name!("scout");
    let native_home = native.cities[&native_city].pos;
    native.barb_scout_homes.insert(native_scout, native_home);
    native.cities.get_mut(&native_city).unwrap().pop = 2;
    let settler = Item::Unit {
        unit: crate::name!("settler"),
    };
    native
        .apply(
            0,
            &Action::Produce {
                city: native_city,
                item: settler.clone(),
            },
        )
        .unwrap();
    let mut native_ai = BasicAi::new();
    assert!(
        !native_ai.barbarian_local_alarm_for_controller(&native, 0, native_city),
        "the native engine-managed Scout remains outside the defense ring"
    );
    native_ai.cities(&mut native, 0);
    assert_eq!(
        native.cities[&native_city].queue.first(),
        Some(&settler),
        "the native Scout does not interrupt the opening"
    );

    let (mut g, city, scout) = barbarian_at_the_gates_game(91_503);
    for uid in g.player_unit_ids(0) {
        g.remove_unit(uid);
    }
    g.units.get_mut(&scout).unwrap().kind = crate::name!("scout");
    assert!(
        !g.barb_scout_homes.contains_key(&scout),
        "the host-imported Scout has no native camp assignment"
    );
    g.cities.get_mut(&city).unwrap().pop = 2;
    assert!(g.can_produce(0, city, &settler));
    g.apply(
        0,
        &Action::Produce {
            city,
            item: settler.clone(),
        },
    )
    .unwrap();
    g.cities.get_mut(&city).unwrap().production = 9.0;

    let mut ai = BasicAi::new();
    assert!(ai.book_pos < 4, "the scripted opening is still active");
    assert!(ai.barbarian_local_alarm_for_controller(&g, 0, city));
    assert_eq!(ai.barbarian_defense_gap(&g, 0, city), 1);

    ai.cities(&mut g, 0);

    let head = g.cities[&city].queue.first().expect("a defense queue");
    assert!(
        matches!(
            head,
            Item::Unit { unit }
                if g.rules.units[unit].class == "military"
                    && g.rules.units[unit].promotion_class != "recon"
        ),
        "the opening Settler yields to a local defender, got {head:?}"
    );
    assert!(
        g.cities[&city]
            .production_progress
            .values()
            .any(|progress| (*progress - 9.0).abs() < f64::EPSILON),
        "the delayed Settler keeps its accumulated production"
    );
}

#[test]
fn live_opening_trains_a_shooter_before_more_settlers_despite_a_scout() {
    for (archery, expected) in [(false, "slinger"), (true, "archer")] {
        let (mut g, city, _) = barbarian_at_the_gates_game(91_504);
        for uid in g.player_unit_ids(0) {
            g.remove_unit(uid);
        }
        let home = g.cities[&city].pos;
        g.spawn_test_unit("scout", 0, home);
        g.cities.get_mut(&city).unwrap().pop = 2;
        if archery {
            g.players[0].techs.insert(crate::name!("archery"));
        }
        let settler = Item::Unit {
            unit: crate::name!("settler"),
        };
        g.apply(
            0,
            &Action::Produce {
                city,
                item: settler,
            },
        )
        .unwrap();
        g.cities.get_mut(&city).unwrap().production = 9.0;
        let mut ai = BasicAi::new();
        ai.garrison_under_fire = true;
        assert_eq!(ai.barbarian_local_defenders_for_controller(&g, 0, city), 0);
        ai.cities(&mut g, 0);
        assert_eq!(
            g.cities[&city].queue.first(),
            Some(&Item::Unit {
                unit: Name::new(expected)
            })
        );
        assert!(g.cities[&city]
            .production_progress
            .values()
            .any(|p| (*p - 9.0).abs() < f64::EPSILON));
    }
}

#[test]
fn live_shooter_request_is_local_bounded_and_not_a_siege_piece() {
    let (mut g, city, _) = barbarian_at_the_gates_game(91_505);
    g.players[0].techs.insert(crate::name!("archery"));
    g.players[0].techs.insert(crate::name!("engineering"));
    let mut ai = BasicAi::new();
    assert!(ai.early_local_shooter_item(&g, 0, city).is_none());
    ai.garrison_under_fire = true;
    let archer = Item::Unit {
        unit: crate::name!("archer"),
    };
    assert_eq!(ai.barbarian_defense_item(&g, 0, city), Some(archer.clone()));
    g.cities.get_mut(&city).unwrap().hp = 100;
    assert_eq!(ai.besieged_city_item(&g, 0, city), Some(archer));
    let home = g.cities[&city].pos;
    let uid = g.spawn_test_unit("archer", 0, home);
    assert!(ai.early_local_shooter_item(&g, 0, city).is_none());
    g.remove_unit(uid);
    g.players[0].techs.insert(crate::name!("machinery"));
    assert!(ai.early_local_shooter_item(&g, 0, city).is_none());
}

#[test]
fn barbarian_raider_uses_pillage_when_no_attack_is_available() {
    let (mut g, city, raider) = barbarian_at_the_gates_game(83);
    let barb = g.units[&raider].owner;
    for uid in g.player_unit_ids(0) {
        g.remove_unit(uid);
    }
    let home = g.cities[&city].pos;
    let raid_tile = g
        .map
        .tiles
        .iter()
        .find(|(pos, tile)| {
            g.wdist(home, **pos) >= 3
                && g.rules.is_passable(tile)
                && !g.rules.is_water(tile)
                && g.city_at(**pos).is_none()
                && g.unit_ids_at(**pos).is_empty()
        })
        .map(|(pos, _)| *pos)
        .expect("the city has an open tile outside immediate attack range");
    {
        let tile = g.map.tiles.get_mut(&raid_tile).unwrap();
        tile.owner_city = Some(city);
        tile.improvement = Some(crate::name!("farm"));
    }
    g.units.get_mut(&raider).unwrap().pos = raid_tile;
    let snapshot = serde_json::to_value(&g).unwrap();
    let mut g: Game = serde_json::from_value(snapshot).unwrap();
    g.current = barb;
    let mut ai = BasicAi::new();
    ai.barb = true;

    assert!(matches!(
        ai.barbarian_raid_action(&g, barb, raider),
        Some(Action::Pillage { unit }) if unit == raider
    ));
    assert!(ai.military_step(&mut g, barb, raider));
    assert!(matches!(
        g.log.last(),
        Some((pid, Action::Pillage { unit })) if *pid == barb && *unit == raider
    ));
    assert!(g.map.tiles[&raid_tile].pillaged);
}

/// ★★★★★ THE BARBARIANS DID NOT TAKE SETTLERS AND THE LIVE SEAT KEPT
/// LOSING THEM.
///
/// MEASURED before this change, `ai_eval live live_without_camp_reach`,
/// 36 seat-games an arm, 6 players, 150 turns, online speed: **0.31 and
/// 0.25 civilians lost to barbarians per game**. The live Civilization VI
/// seat lost **eight Settlers and two Builders in 104 turns** on run
/// civvis-20260821T130446Z. A simulation thirty times gentler than the
/// thing it models cannot price a settler-protection treatment at all —
/// the effect is smaller than the noise by construction.
///
/// `capture_adjacent_civilian` was ungated, so a raider took a Settler it
/// happened to end its turn beside. The PURSUIT — walking to one two or
/// three tiles off, which is what actually costs an empire its walkers —
/// sat behind `civilian_rescue`, and the barbarian seat is a `BasicAi`
/// that never carries it.
#[test]
fn a_barbarian_walks_to_an_undefended_settler_inside_its_raid_ring() {
    let take = |tactics: bool| -> bool {
        let (mut g, city, raider) = barbarian_at_the_gates_game(84);
        let barb = g.units[&raider].owner;
        let cpos = g.cities[&city].pos;
        let home = g.units[&raider].pos;
        // The camp that owns this raider, so the leash below is satisfied
        // and the walk is a raid rather than an all-map chase.
        g.barb_camps.insert(home, g.turn + 1_000);
        g.barb_raider_homes.insert(raider, home);
        let road = g
            .map
            .tiles
            .keys()
            .copied()
            .filter(|pos| {
                g.wdist(*pos, home) == 2
                    && g.wdist(*pos, cpos) > 1
                    && g.map
                        .get(*pos)
                        .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
                    && g.unit_ids_at(*pos).is_empty()
                    && g.city_at(*pos).is_none()
            })
            .min()
            .expect("open ground two tiles from the raider");
        let settler = g.spawn_test_unit("settler", 0, road);
        let mut ai = BasicAi::new();
        ai.barb = true;
        ai.barbarian_tactics = tactics;
        // Seat-turns, not forced `current` writes: the walk is one step per
        // call and a unit only gets its movement back when the turn really
        // comes round to it. Six is three world turns for a two-seat board,
        // ample for a two-tile approach.
        for _ in 0..6 {
            let pid = g.current;
            if pid == barb {
                ai.take_turn(&mut g, barb);
                if g.units.get(&settler).is_some_and(|unit| unit.owner == barb) {
                    return true;
                }
            }
            if g.winner.is_none() && g.current == pid {
                let _ = g.apply(pid, &Action::EndTurn);
            }
        }
        false
    };
    assert!(
        take(true),
        "a barbarian two tiles from an undefended Settler must walk onto it"
    );
    assert!(
        !take(false),
        "and it must still be the barbarian tactics that decide it, so the \
             frozen no-tactics controller plays the game it always did"
    );
}

/// The religious corps is prey too: a raider two tiles from a Missionary
/// walks onto it, and condemns it with the movement it has on arrival or
/// at the start of its next turn. The frozen no-tactics controller, and a
/// tactics controller with the hunt withheld, leave the Missionary alone.
#[test]
fn a_barbarian_walks_onto_a_missionary_and_condemns_it() {
    let condemned = |tactics: bool, hunt: bool| -> bool {
        let (mut g, city, raider) = barbarian_at_the_gates_game(84);
        let barb = g.units[&raider].owner;
        let cpos = g.cities[&city].pos;
        let home = g.units[&raider].pos;
        g.barb_camps.insert(home, g.turn + 1_000);
        g.barb_raider_homes.insert(raider, home);
        let road = g
            .map
            .tiles
            .keys()
            .copied()
            .filter(|pos| {
                g.wdist(*pos, home) == 2
                    && g.wdist(*pos, cpos) > 1
                    && g.map
                        .get(*pos)
                        .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
                    && g.unit_ids_at(*pos).is_empty()
                    && g.city_at(*pos).is_none()
            })
            .min()
            .expect("open ground two tiles from the raider");
        g.players[0].religion = Some("Faith".to_string());
        let missionary = g.spawn_test_unit("missionary", 0, road);
        g.units.get_mut(&missionary).unwrap().religion = Some("Faith".to_string());
        let mut ai = BasicAi::new();
        ai.barb = true;
        ai.barbarian_tactics = tactics;
        ai.barbarian_heretic_hunt = hunt;
        // The Missionary itself is never moved: the question is whether
        // the raider comes for it. Eight seat-turns is four world turns,
        // the walk plus the condemnation on the turn after.
        for _ in 0..8 {
            let pid = g.current;
            if pid == barb {
                ai.take_turn(&mut g, barb);
                if !g.units.contains_key(&missionary) {
                    return true;
                }
            }
            if g.winner.is_none() && g.current == pid {
                let _ = g.apply(pid, &Action::EndTurn);
            }
        }
        false
    };
    assert!(
        condemned(true, true),
        "a barbarian two tiles from a Missionary must walk onto it and condemn it"
    );
    assert!(
        !condemned(true, false),
        "the hunt withheld leaves the Missionary standing"
    );
    assert!(
        !condemned(false, true),
        "and the frozen no-tactics controller plays the game it always did"
    );
    assert!(
        BasicAi::new().barbarian_heretic_hunt,
        "the hunt is on in every current controller"
    );
}

#[test]
fn barbarian_raider_returns_to_its_camp_after_a_long_chase() {
    let (mut g, _city, raider) = barbarian_at_the_gates_game(84);
    let barb = g.units[&raider].owner;
    let open: Vec<Pos> = g
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| {
            g.rules.is_passable(tile)
                && !g.rules.is_water(tile)
                && g.city_at(**pos).is_none()
                && g.unit_ids_at(**pos).is_empty()
        })
        .map(|(pos, _)| *pos)
        .collect();
    let (raider_at, camp) = open
        .iter()
        .find_map(|raider_at| {
            open.iter()
                .find(|camp| g.wdist(*raider_at, **camp) > BARBARIAN_RETURN_RADIUS)
                .map(|camp| (*raider_at, *camp))
        })
        .expect("the map has two open sites beyond the barbarian leash");
    g.units.get_mut(&raider).unwrap().pos = raider_at;
    g.barb_camps.insert(camp, g.turn + 1_000);
    let mut ai = BasicAi::new();
    ai.barb = true;
    assert_eq!(
        ai.nearest_enemy(&g, barb, raider, &[0]),
        Some(camp),
        "a raider beyond its leash must return to the nearest camp"
    );
}

#[test]
fn barbarian_raider_keeps_the_camp_that_raised_it_when_another_is_nearer() {
    let (mut g, _city, raider) = barbarian_at_the_gates_game(85);
    let open: Vec<Pos> = g
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| {
            g.rules.is_passable(tile)
                && !g.rules.is_water(tile)
                && g.city_at(**pos).is_none()
                && g.unit_ids_at(**pos).is_empty()
        })
        .map(|(pos, _)| *pos)
        .collect();
    let (raider_at, home, nearer_camp) = open
        .iter()
        .find_map(|raider_at| {
            let home = open
                .iter()
                .copied()
                .find(|camp| g.wdist(*raider_at, *camp) > BARBARIAN_RETURN_RADIUS)?;
            open.iter()
                .copied()
                .filter(|camp| *camp != *raider_at && *camp != home)
                .min_by_key(|camp| g.wdist(*raider_at, *camp))
                .filter(|camp| g.wdist(*raider_at, *camp) < g.wdist(*raider_at, home))
                .map(|nearer_camp| (*raider_at, home, nearer_camp))
        })
        .expect("the map has one distant and one nearer camp site");
    g.units.get_mut(&raider).unwrap().pos = raider_at;
    g.barb_camps.insert(home, g.turn + 1_000);
    g.barb_camps.insert(nearer_camp, g.turn + 1_000);
    g.barb_raider_homes.insert(raider, home);

    assert_eq!(
        BasicAi::barbarian_home(&g, raider),
        Some(home),
        "a raider must retain the camp that raised it rather than switch to a closer outpost"
    );
}

fn barb_skirmish_game(seed: u64) -> (Game, u32, u32) {
    let mut g = Game::new_full(1, 20, 14, seed, 60, 0, true);
    let barb_pid = g.barb_pid.unwrap();
    for unit in g
        .units
        .values()
        .filter(|unit| unit.owner == barb_pid)
        .map(|unit| unit.id)
        .collect::<Vec<_>>()
    {
        g.remove_unit(unit);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    g.barb_camp_guards.clear();
    g.barb_scout_homes.clear();
    g.barb_scout_targets.clear();
    g.barb_camp_targets.clear();
    g.barb_alerted_until.clear();
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|id| g.units[id].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let warrior = g
        .player_unit_ids(0)
        .into_iter()
        .find(|id| g.units[id].kind == "warrior")
        .unwrap();
    let wpos = g.units[&warrior].pos;
    let open = g
        .nbrs(wpos)
        .into_iter()
        .find(|p| {
            let t = &g.map.tiles[p];
            g.rules.is_passable(t)
                && !g.rules.is_water(t)
                && g.unit_ids_at(*p).is_empty()
                && g.city_at(*p).is_none()
        })
        .expect("open land tile next to the warrior");
    let mut barb = g.units[&warrior].clone();
    barb.id = g.next_id;
    g.next_id += 1;
    barb.owner = barb_pid;
    barb.pos = open;
    let bid = barb.id;
    g.units.insert(bid, barb);
    // The staged raider must be the only Barbarian in reach: organic
    // camp garrisons on the generated map would steal the target pick.
    let strays: Vec<u32> = g
        .units
        .values()
        .filter(|unit| unit.owner == g.barb_pid.unwrap() && unit.id != bid)
        .map(|unit| unit.id)
        .collect();
    for stray in strays {
        g.units.remove(&stray);
    }
    // Camps are pursuit targets too; the staged fight must be the only one.
    let camps: Vec<Pos> = g.barb_camps.keys().copied().collect();
    for camp in camps {
        g.barb_camps.remove(&camp);
        g.barb_naval_camps.remove(&camp);
        g.barb_camp_guards.remove(&camp);
        let tile = g.map.tiles.get_mut(&camp).unwrap();
        if tile.improvement.as_deref() == Some("barbarian_camp") {
            tile.improvement = None;
        }
    }
    // Round-trip to rebuild occupancy after the manual inserts.
    let snapshot = serde_json::to_value(&g).unwrap();
    let g: Game = serde_json::from_value(snapshot).unwrap();
    (g, warrior, bid)
}

/// Build a one-player world whose seat owns a city and has charted only
/// part of the map — the shape every empire is in for most of a game.
fn blind_empire(seed: u64) -> (Game, BasicAi) {
    let mut g = Game::new_full(1, 30, 18, seed, 300, 0, false);
    g.current = 0;
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| g.units[uid].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    for uid in g.player_unit_ids(0) {
        g.remove_unit(uid);
    }
    let mut ai = BasicAi::new();
    ai.recon_replacement = true;
    (g, ai)
}

fn dry_map(g: &mut Game) {
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
    }
}

/// Lay a straight, four-or-more-hex coastal route from a city. The final
/// tile is deliberately kept unseen, giving a native-test fixture the
/// same fresh-water objective the live mirror supplies at its fog edge.
fn coastal_waterway(g: &mut Game, origin: Pos, length: usize) -> Vec<Pos> {
    let mut waterway = Vec::new();
    let mut current = origin;
    for distance in 1..=length {
        current = g
            .nbrs(current)
            .into_iter()
            .find(|pos| g.map.tiles.contains_key(pos) && g.wdist(origin, *pos) == distance as i32)
            .expect("an interior city has an outward coastal path");
        g.map.tiles.get_mut(&current).unwrap().terrain = crate::name!("coast");
        waterway.push(current);
    }
    g.players[0].explored.remove(waterway.last().unwrap());
    waterway
}

/// Cut a two-hex lake into otherwise dry ground. The caller chooses a
/// centre away from cities, so this is a small, deterministic stand-in for
/// the live Durocortorum lake that trapped a Galley.
fn two_tile_lake(g: &mut Game, centre: Pos) -> [Pos; 2] {
    for pos in g.wdisk(centre, 2) {
        let Some(tile) = g.map.tiles.get_mut(&pos) else {
            continue;
        };
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
    }
    let neighbors = g.nbrs(centre);
    let first = neighbors[0];
    let second = neighbors
        .into_iter()
        .find(|pos| *pos != first && g.wdist(first, *pos) == 1)
        .expect("a hex has adjacent neighbors");
    for pos in [first, second] {
        g.map.tiles.get_mut(&pos).unwrap().terrain = crate::name!("coast");
    }
    [first, second]
}

/// The empire owns no recon and there is ground it has never walked, so the
/// arm is missing. See `recon_is_the_missing_arm`: this is the state live
/// run `civvis-20260808T142724Z` sat in for 150 turns while its army grew
/// to 22 units and 77% of the map stayed dark.
/// The measurement behind `BasicAi::explore_commit`, kept runnable:
/// `CIVVIS_EXPLORE_AB_SEEDS=16 cargo test --profile ci --lib explore_commit_sweeps_more_ground -- --ignored --nocapture`.
/// Six-seat Continents boards at the live seat's size, seat 0 the deployed
/// bridge controller with and without the treatment, everyone else stock;
/// prints revealed plots and contacts at t30/t50/t70 per arm. Ignored
/// because it plays whole early games (~2 min per 16 seeds); the assertion
/// is the mean, so a single map cannot pass or fail it. 2026-08-16, 16
/// seeds: stock 181/284/384, treated 208/333/474 revealed; city-states met
/// 1.69/2.25/3.06 against 1.62/2.56/3.31.
#[test]
#[ignore]
fn explore_commit_sweeps_more_ground() {
    let arms = ["stock-bridge", "explore-commit"];
    let count: u64 = std::env::var("CIVVIS_EXPLORE_AB_SEEDS")
        .ok()
        .and_then(|n| n.parse().ok())
        .unwrap_or(8);
    let seeds: Vec<u64> = (1..=count).map(|n| 26_081_600 + n).collect();
    type ExploreCommitSummary = (String, [f64; 3], [f64; 3], [f64; 3]);
    let mut summary: Vec<ExploreCommitSummary> = Vec::new();
    for (arm, commit) in arms.iter().zip([false, true]) {
        let mut revealed = [0.0f64; 3];
        let mut minors_met = [0.0f64; 3];
        let mut majors_met = [0.0f64; 3];
        for seed in &seeds {
            let mut game = Game::new_with(crate::game::GameOptions {
                speed: "online".to_string(),
                map_script: crate::setup::MapScript::Continents,
                ..crate::game::GameOptions::new(6, 74, 46, *seed, 70, 9)
            });
            let mut ais: Vec<AdvancedAi> = (0..game.players.len())
                .map(|seat| {
                    let mut ai = AdvancedAi::new();
                    if seat == 0 {
                        ai.enable_live_bridge();
                        if commit {
                            ai.enable_explore_commit();
                        }
                    }
                    ai
                })
                .collect();
            game.set_fog_memory(false);
            let mut marks = [30u32, 50, 70].into_iter().peekable();
            while game.winner.is_none() && game.turn <= game.max_turns {
                let pid = game.current;
                ais[pid].take_turn(&mut game, pid);
                if game.winner.is_none() && game.current == pid {
                    let _ = game.apply(pid, &Action::EndTurn);
                }
                if pid == game.players.len() - 1 {
                    if let Some(mark) = marks.peek().copied() {
                        if game.turn >= mark {
                            let slot = [30u32, 50, 70].iter().position(|m| *m == mark).unwrap();
                            revealed[slot] += game.players[0].explored.len() as f64;
                            minors_met[slot] += game
                                .players
                                .iter()
                                .filter(|p| p.is_minor && !p.is_barbarian && game.has_met(0, p.id))
                                .count() as f64;
                            majors_met[slot] += game
                                .players
                                .iter()
                                .filter(|p| !p.is_minor && p.id != 0 && game.has_met(0, p.id))
                                .count() as f64;
                            marks.next();
                        }
                    }
                }
            }
        }
        let n = seeds.len() as f64;
        for slot in 0..3 {
            revealed[slot] /= n;
            minors_met[slot] /= n;
            majors_met[slot] /= n;
        }
        eprintln!(
                "{arm}: revealed t30/t50/t70 = {:.0}/{:.0}/{:.0}  city-states met = {:.2}/{:.2}/{:.2}  majors met = {:.2}/{:.2}/{:.2}",
                revealed[0], revealed[1], revealed[2],
                minors_met[0], minors_met[1], minors_met[2],
                majors_met[0], majors_met[1], majors_met[2]
            );
        summary.push((arm.to_string(), revealed, minors_met, majors_met));
    }
    assert!(
        summary[1].1[1] > summary[0].1[1],
        "the committed sweep must reveal more ground by t50 on average: {:?}",
        summary
    );
}

#[test]
fn an_empire_with_no_eyes_and_ground_left_to_walk_is_missing_the_recon_arm() {
    let (g, ai) = blind_empire(4_411);
    assert!(
        g.map
            .tiles
            .iter()
            .any(|(pos, tile)| !g.players[0].explored.contains(pos)
                && g.rules.is_passable(tile)
                && !g.rules.is_water(tile)),
        "the fixture must leave real ground unexplored"
    );
    assert!(ai.recon_is_the_missing_arm(&g, 0));

    // One scout answers it before expansion. The arm is repaired, not
    // stockpiled through the opening.
    let mut with_scout = g.clone();
    let home = with_scout.player_city_ids(0)[0];
    let pos = with_scout.cities[&home].pos;
    with_scout.spawn_test_unit("scout", 0, pos);
    assert!(!ai.recon_is_the_missing_arm(&with_scout, 0));

    // City two earns an independent second eye. The single Scout may be
    // trapped or killed; it must no longer mask the frontier from an
    // expanded empire.
    let mut expanded = with_scout.clone();
    let capital = expanded.player_city_ids(0)[0];
    let capital_pos = expanded.cities[&capital].pos;
    let second_site = expanded
        .map
        .tiles
        .iter()
        .filter(|(_, tile)| {
            tile.owner_city.is_none()
                && expanded.rules.is_passable(tile)
                && !expanded.rules.is_water(tile)
        })
        .map(|(position, _)| *position)
        .find(|position| (4..=10).contains(&expanded.wdist(capital_pos, *position)))
        .expect("the fixture has a legal second-city site");
    let second_city = expanded.found_city_for(0, second_site, None);
    assert_eq!(
        expanded.player_city_ids(0).len(),
        RECON_SECOND_EYE_CITY_FLOOR
    );
    assert!(
        ai.recon_is_the_missing_arm(&expanded, 0),
        "one Scout does not leave a two-city empire with a redundant eye"
    );

    // ★★★★ A queued Scout also answers the second-eye gap. With only
    // units on the map counted, every city that came to `pick_item` on the
    // same turn read "no eyes" and answered it — seven Rangers in seven
    // cities on run civvis-20260816T213447Z, t140. A soldier in the queue
    // is not eyes.
    let mut queued = expanded.clone();
    queued
        .cities
        .get_mut(&home)
        .unwrap()
        .queue
        .push(Item::Unit {
            unit: Name::new("warrior"),
        });
    assert!(
        ai.recon_is_the_missing_arm(&queued, 0),
        "a queued warrior is no eye"
    );
    queued.cities.get_mut(&home).unwrap().queue.clear();
    queued
        .cities
        .get_mut(&home)
        .unwrap()
        .queue
        .push(Item::Unit {
            unit: Name::new("scout"),
        });
    assert!(
        !ai.recon_is_the_missing_arm(&queued, 0),
        "a Scout already in a queue is the expanded arm being rebuilt"
    );

    expanded.spawn_test_unit("scout", 0, expanded.cities[&second_city].pos);
    assert!(
        !ai.recon_is_the_missing_arm(&expanded, 0),
        "two live Scouts satisfy the complete, bounded recon arm"
    );
}

/// The current live Rome game reached turn 121 with nine cities, two
/// Scouts, and only four of roughly ten city-states met. Two eyes are the
/// ordinary bounded arm, but a mature empire that has missed most of the
/// map's city-states needs one fresh route without turning its opening or
/// a rebuild into a three-Scout burst.
#[test]
fn a_developed_empire_with_most_city_states_unmet_adds_exactly_one_third_scout() {
    let mut game = Game::new_full(1, 42, 30, 4_415, 200, 6, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .expect("the major opens with a Settler");
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    for uid in game.player_unit_ids(0) {
        game.remove_unit(uid);
    }

    while game.player_city_ids(0).len() < RECON_CITY_STATE_SWEEP_CITY_FLOOR {
        let site = game
            .map
            .tiles
            .iter()
            .filter(|(position, tile)| {
                tile.owner_city.is_none()
                    && game.rules.is_passable(tile)
                    && !game.rules.is_water(tile)
                    && game.city_at(**position).is_none()
            })
            .map(|(position, _)| *position)
            .find(|position| {
                game.cities
                    .values()
                    .all(|city| game.wdist(city.pos, *position) >= 4)
            })
            .expect("the large fixture has room for the developed empire");
        game.found_city_for(0, site, None);
    }
    let home = game.cities[&game.player_city_ids(0)[0]].pos;
    game.spawn_test_unit("scout", 0, home);
    game.spawn_test_unit("scout", 0, home);

    let minors = game
        .players
        .iter()
        .filter(|player| {
            player.alive && player.is_minor && !player.is_barbarian && !player.is_free_city
        })
        .map(|player| player.id)
        .collect::<Vec<_>>();
    assert_eq!(minors.len(), 6, "the fixture needs six city-states");

    let mut ai = BasicAi::new();
    ai.recon_replacement = true;
    assert!(
        ai.recon_is_the_missing_arm(&game, 0),
        "two ordinary eyes leave one late contact-sweep eye missing"
    );

    // The live production pass starts only one third Scout: once a city
    // has it queued, the other five empty cities see the complete arm.
    ai.book_pos = 4;
    ai.cities(&mut game, 0);
    let queued_scouts = game
        .player_city_ids(0)
        .into_iter()
        .flat_map(|city| game.cities[&city].queue.iter())
        .filter(|item| matches!(item, Item::Unit { unit } if unit == "scout"))
        .count();
    assert_eq!(queued_scouts, 1, "the contact sweep adds exactly one Scout");
    assert!(
        !ai.recon_is_the_missing_arm(&game, 0),
        "the queued third Scout completes the bounded contact sweep"
    );

    // Meeting half the city-states releases the extra eye; the ordinary
    // two-Scout arm remains enough even after its pending Scout is cleared.
    for city in game.player_city_ids(0) {
        game.cities.get_mut(&city).unwrap().queue.clear();
    }
    game.players[0].met.extend(minors.iter().take(3).copied());
    assert!(
        !ai.recon_is_the_missing_arm(&game, 0),
        "three met versus three unseen city-states is no longer a majority gap"
    );
}

#[test]
fn the_live_recon_repair_spends_the_first_opening_slot_on_a_scout() {
    let (mut game, mut ai) = blind_empire(4_413);

    ai.cities(&mut game, 0);

    let capital = game.player_city_ids(0)[0];
    assert!(matches!(
        game.cities[&capital].queue.first(),
        Some(Item::Unit { unit }) if unit == "scout"
    ));
    assert_eq!(
        ai.book_pos, 1,
        "the Scout replaces the first scripted build instead of adding a fifth opener"
    );
}

#[test]
fn the_live_recon_repair_starts_one_second_scout_after_city_two() {
    let (mut game, mut ai) = blind_empire(4_414);
    let capital = game.player_city_ids(0)[0];
    let capital_pos = game.cities[&capital].pos;
    game.spawn_test_unit("scout", 0, capital_pos);
    let second_site = game
        .map
        .tiles
        .iter()
        .filter(|(_, tile)| {
            tile.owner_city.is_none() && game.rules.is_passable(tile) && !game.rules.is_water(tile)
        })
        .map(|(position, _)| *position)
        .find(|position| (4..=10).contains(&game.wdist(capital_pos, *position)))
        .expect("the fixture has a legal second-city site");
    game.found_city_for(0, second_site, None);
    ai.book_pos = 4;

    ai.cities(&mut game, 0);

    let queued_scouts = game
        .player_city_ids(0)
        .into_iter()
        .flat_map(|city| game.cities[&city].queue.iter())
        .filter(|item| matches!(item, Item::Unit { unit } if unit == "scout"))
        .count();
    assert_eq!(
        queued_scouts, 1,
        "city two replaces the lone Scout with exactly one independent eye"
    );
}

/// An army is not eyes, and a charted world needs none.
#[test]
fn the_recon_arm_is_not_missing_for_a_soldier_or_a_finished_map() {
    let (g, ai) = blind_empire(4_412);

    // A stack of soldiers does not explore, so it does not satisfy this.
    let mut army = g.clone();
    let pos = army.cities[&army.player_city_ids(0)[0]].pos;
    for _ in 0..12 {
        army.spawn_test_unit("warrior", 0, pos);
    }
    assert!(
        ai.recon_is_the_missing_arm(&army, 0),
        "twelve warriors are twelve bodies and no eyes — exactly the \
             headcount the military floor already reads as finished"
    );

    // Nothing left to find: the trigger releases.
    let mut charted = g.clone();
    let all: Vec<Pos> = charted.map.tiles.keys().copied().collect();
    charted.players[0].explored.extend(all);
    assert!(!ai.recon_is_the_missing_arm(&charted, 0));

    // And it is off unless the live bridge turns it on.
    let mut shipped = ai.clone();
    shipped.recon_replacement = false;
    assert!(!shipped.recon_is_the_missing_arm(&g, 0));
}

/// ★★★★ One of five rivals met by turn 142 and 13% of the map seen: the
/// land recon arm charted its continent and stopped at the shore, and in
/// 142 turns the empire built one Galley (civvis-20260816T110555Z). See
/// `naval_recon`. An empire with no ship and unexplored water is missing
/// the sea's arm; one Galley with an open route satisfies it; a charted
/// sea releases it; and a viable coastal city with Sailing buys the
/// cheapest hull for it.
#[test]
fn an_empire_with_no_ship_and_water_left_to_chart_is_missing_the_naval_arm() {
    let (mut g, mut ai) = blind_empire(4_430);
    ai.naval_recon = true;
    let cid = g.player_city_ids(0)[0];
    let city_pos = g.cities[&cid].pos;
    dry_map(&mut g);
    let waterway = coastal_waterway(&mut g, city_pos, NAVAL_RECON_MIN_WATERWAY_TILES);
    assert!(
        BasicAi::city_has_naval_recon_launch(&g, 0, cid),
        "fixture precondition: unexplored water exists"
    );
    assert!(
        ai.naval_recon_is_the_missing_arm(&g, 0),
        "no ship, water unseen: missing"
    );

    // Nothing to buy without Sailing; the cheapest hull once it is known.
    assert_eq!(ai.best_naval_recon(&g, 0, cid), None);
    let mut sailing = g.clone();
    sailing.players[0].techs.insert(crate::name!("sailing"));
    assert_eq!(
        ai.best_naval_recon(&sailing, 0, cid).as_deref(),
        Some("galley"),
        "the cheapest ship the city can lay down"
    );

    // A soldier does not satisfy the sea's arm; one Galley does.
    let mut army = g.clone();
    let pos = army.cities[&cid].pos;
    for _ in 0..6 {
        army.spawn_test_unit("warrior", 0, pos);
    }
    assert!(ai.naval_recon_is_the_missing_arm(&army, 0));
    let mut fleet = g.clone();
    let free_galley = fleet.spawn_test_unit("galley", 0, waterway[0]);
    assert!(
        BasicAi::naval_recon_ship_can_chart(&fleet, 0, free_galley),
        "the Galley can reach the uncharted open water"
    );
    assert!(
        !ai.naval_recon_is_the_missing_arm(&fleet, 0),
        "one hull is the whole arm"
    );

    // A Galley that is physically present but caught in a two-hex lake
    // must not stop the empire from repairing its actual naval eye.
    let mut stranded = sailing.clone();
    let city_pos = stranded.cities[&cid].pos;
    let lake_centre = stranded
        .map
        .tiles
        .values()
        .find(|tile| {
            stranded.wdist(city_pos, tile.pos) >= 5
                && !stranded.rules.is_water(tile)
                && stranded.wdisk(tile.pos, 2).len() == 19
        })
        .map(|tile| tile.pos)
        .expect("a dry patch well away from the coastal capital");
    let lake = two_tile_lake(&mut stranded, lake_centre);
    let trapped_galley = stranded.spawn_test_unit("galley", 0, lake[0]);
    assert!(
        !BasicAi::naval_recon_ship_can_chart(&stranded, 0, trapped_galley),
        "a two-hex lake is not a route to new contacts"
    );
    assert!(
        ai.naval_recon_is_the_missing_arm(&stranded, 0),
        "the stranded Galley cannot mask the missing sea explorer"
    );

    // A charted sea releases it; and it is off unless turned on.
    let mut charted = g.clone();
    let all: Vec<Pos> = charted.map.tiles.keys().copied().collect();
    charted.players[0].explored.extend(all);
    assert!(!ai.naval_recon_is_the_missing_arm(&charted, 0));
    let mut off = ai.clone();
    off.naval_recon = false;
    assert!(!off.naval_recon_is_the_missing_arm(&g, 0));
}

#[test]
fn a_major_naval_war_keeps_a_second_recon_eye() {
    let mut game = Game::new_full(2, 30, 18, 4_432, 300, 0, false);
    game.current = 0;
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .expect("the first major starts with a Settler");
    game.apply(0, &Action::FoundCity { unit: settler })
        .expect("the fixture founds a capital");
    for uid in game.player_unit_ids(0) {
        game.remove_unit(uid);
    }
    let city = game.player_city_ids(0)[0];
    let city_pos = game.cities[&city].pos;
    dry_map(&mut game);
    let waterway = coastal_waterway(&mut game, city_pos, NAVAL_RECON_MIN_WATERWAY_TILES + 3);
    game.players[0].techs.insert(crate::name!("sailing"));
    game.at_war.insert((0, 1));
    game.spawn_test_unit("galley", 1, *waterway.last().unwrap());
    let first = game.spawn_test_unit("galley", 0, waterway[0]);

    let mut ai = BasicAi::new();
    ai.enable_naval_recon();
    assert!(BasicAi::naval_recon_ship_can_chart(&game, 0, first));
    assert!(
        ai.naval_recon_is_the_missing_arm(&game, 0),
        "the sole Galley can fight the major naval war or chart, but not reliably do both"
    );
    let mut delayed_v3 = BasicAi::new();
    delayed_v3.naval_recon_3 = true;
    delayed_v3.recon_replacement = true;
    assert!(
        delayed_v3.naval_recon_is_the_missing_arm(&game, 0),
        "v3 delays only peacetime production; a one-city naval war still keeps its second eye"
    );
    assert!(
        !delayed_v3.naval_recon_yields_to_land_recon(&game, 0),
        "v3's land-first queue rule never weakens the wartime second-eye exception"
    );

    // A viable sea unit already queued is the second eye being built, not
    // permission for every other city to queue another copy.
    game.cities.get_mut(&city).unwrap().queue.push(Item::Unit {
        unit: crate::name!("galley"),
    });
    assert!(
        !ai.naval_recon_is_the_missing_arm(&game, 0),
        "one live eye plus one queued eye completes the bounded wartime arm"
    );
    game.cities.get_mut(&city).unwrap().queue.clear();

    let second = game.spawn_test_unit("galley", 0, waterway[1]);
    assert!(BasicAi::naval_recon_ship_can_chart(&game, 0, second));
    assert!(
        !ai.naval_recon_is_the_missing_arm(&game, 0),
        "two live eyes complete the wartime arm"
    );

    let mut city_state_war = game.clone();
    city_state_war.remove_unit(second);
    city_state_war.players[1].is_minor = true;
    assert!(
        !ai.naval_recon_is_the_missing_arm(&city_state_war, 0),
        "a city-state war keeps the normal one-ship exploration arm"
    );
}

/// `naval-recon-3` gives a simultaneously missing land eye the next idle
/// production queue, so the sea scout cannot crowd out the broader
/// continental information gap. It still keeps the normal one-hull
/// peacetime arm and the existing two-eye wartime exception.
#[test]
fn a_v3_naval_recon_yields_to_a_missing_land_eye() {
    let (mut game, mut ai) = blind_empire(4_433);
    ai.naval_recon = false;
    ai.naval_recon_3 = true;
    let capital = game.player_city_ids(0)[0];
    let capital_pos = game.cities[&capital].pos;
    dry_map(&mut game);
    coastal_waterway(&mut game, capital_pos, NAVAL_RECON_MIN_WATERWAY_TILES + 3);
    game.players[0].techs.insert(crate::name!("sailing"));

    assert!(
        ai.naval_recon_is_the_missing_arm(&game, 0),
        "the v3 sea arm remains a normal one-hull peacetime need"
    );
    assert!(
        ai.recon_is_the_missing_arm(&game, 0),
        "the fixture also leaves the empire without a land eye"
    );
    assert!(
        ai.naval_recon_yields_to_land_recon(&game, 0),
        "v3 makes the sea arm yield to the broader land frontier"
    );

    let mut v1 = ai.clone();
    v1.naval_recon_3 = false;
    v1.naval_recon = true;
    assert!(
        !v1.naval_recon_yields_to_land_recon(&game, 0),
        "the existing one-eye version retains its independent sea reservation"
    );

    game.spawn_test_unit("scout", 0, capital_pos);
    assert!(
        !ai.naval_recon_yields_to_land_recon(&game, 0),
        "once the land eye exists, v3 may reserve its one sea scout"
    );
}

/// The early exit answers exactly what the full walk answers.
///
/// ⚠⚠ THIS IS THE WHOLE SAFETY ARGUMENT FOR THE OPTIMIZATION, AND IT IS AN
/// ASSERTION RATHER THAN A PARAGRAPH. `naval_recon_can_chart_from` stops
/// the flood fill as soon as `size >= 4`, "reaches open water" and "still
/// charts something" are all settled; `naval_recon_waterway` +
/// `naval_recon_waterway_can_chart` build the whole component and then
/// ask. They must agree on every start, on every map, at every stage of
/// exploration — including the cases where the answer is `false`, which
/// are the ones the early exit cannot short-circuit.
///
/// The counts at the end are the census guard: a comparison that never
/// saw a `true`, never saw a `false`, or never ran is a broken test that
/// passes, and this repository has shipped that shape before.
#[test]
fn naval_recon_early_exit_agrees_with_the_full_walk() {
    let mut compared = 0usize;
    let mut agreed_true = 0usize;
    let mut agreed_false = 0usize;
    for seed in [11_u64, 4_531, 90_210, 7_310_002] {
        let mut g = Game::new_full(2, 30, 18, seed, 300, 0, false);
        g.current = 0;
        // Three exploration stages, because the frontier half of the
        // predicate is the only one that can flip from more knowledge:
        // blind, partly charted, and fully charted (where every walk must
        // run to exhaustion and return `false`).
        let all: Vec<Pos> = g.map.tiles.keys().copied().collect();
        for stage in 0..3 {
            g.players[0].explored.clear();
            match stage {
                1 => {
                    for (index, pos) in all.iter().enumerate() {
                        if index % 3 == 0 {
                            g.players[0].explored.insert(*pos);
                        }
                    }
                }
                2 => {
                    for pos in &all {
                        g.players[0].explored.insert(*pos);
                    }
                    for tile in g.map.tiles.values_mut() {
                        if g.rules.is_unknown(tile) {
                            tile.terrain = crate::name!("grassland");
                        }
                    }
                }
                _ => {}
            }
            for (index, pos) in all.iter().enumerate() {
                // Every ninth tile, so the test covers land starts (which
                // seed nothing) and water starts alike without walking a
                // 540-tile map three times per seed.
                if index % 9 != 0 {
                    continue;
                }
                let starts = std::iter::once(*pos).chain(g.nbrs(*pos));
                let fused = BasicAi::naval_recon_can_chart_from(&g, 0, starts.clone());
                let waterway = BasicAi::naval_recon_waterway(&g, 0, starts);
                let full = BasicAi::naval_recon_waterway_can_chart(&g, 0, &waterway);
                assert_eq!(
                    fused, full,
                    "seed {seed} stage {stage} start {pos:?}: the early exit said \
                         {fused} and the full walk said {full}"
                );
                compared += 1;
                if fused {
                    agreed_true += 1;
                } else {
                    agreed_false += 1;
                }
            }
        }
    }
    assert!(
        compared > 500,
        "only {compared} comparisons ran; the sweep collapsed"
    );
    assert!(
        agreed_true > 0,
        "no start ever charted; only the false branch was compared"
    );
    assert!(
        agreed_false > 0,
        "every start charted; the exhausting walk was never compared"
    );
}

#[test]
fn naval_recon_never_launches_from_a_two_tile_lake() {
    let (mut lake, mut ai) = blind_empire(4_531);
    ai.naval_recon = true;
    let cid = lake.player_city_ids(0)[0];
    let city_pos = lake.cities[&cid].pos;
    dry_map(&mut lake);
    let shore = two_tile_lake(&mut lake, city_pos);
    lake.players[0].techs.insert(crate::name!("sailing"));

    assert!(
        BasicAi::city_is_coastal(&lake, cid),
        "the lake still makes the city coastal"
    );
    assert!(
        !BasicAi::city_has_naval_recon_launch(&lake, 0, cid),
        "coastal yields do not imply a viable sea-scout launch"
    );
    assert_eq!(
        ai.best_naval_recon(&lake, 0, cid),
        None,
        "do not spend a Galley on a lake-bound reconnaissance job"
    );
    let galley = lake.spawn_test_unit("galley", 0, shore[0]);
    assert!(
        !BasicAi::naval_recon_ship_can_chart(&lake, 0, galley),
        "a lake-bound hull is not a naval reconnaissance arm"
    );
    assert!(
        !ai.naval_recon_is_the_missing_arm(&lake, 0),
        "with no viable launch, do not keep forcing a naval build"
    );
}

#[test]
fn even_barbarian_trades_are_taken_not_shadowed() {
    let (mut g, warrior, barb) = barb_skirmish_game(33);
    let mut ai = BasicAi::new();
    assert!(ai.military_step(&mut g, 0, warrior));
    assert!(
        g.units.get(&barb).map(|b| b.hp < 100).unwrap_or(true),
        "adjacent equal-strength barbarian should be attacked, not shadowed"
    );
}

#[test]
fn outmatched_units_stop_chasing_barbarians() {
    let (mut g, uid, barb) = barb_skirmish_game(34);
    let ai = BasicAi::new();
    let bp = g.units[&barb].owner;
    let bpos = g.units[&barb].pos;
    // A warrior takes the even fight, so the raider is a valid target...
    assert_eq!(ai.nearest_enemy(&g, 0, uid, &[bp]), Some(bpos));
    // ...but a scout would decline the attack, so it must not pick the
    // raider as a pursuit target either (the chase-without-striking bug).
    g.units.get_mut(&uid).unwrap().kind = crate::name!("scout");
    assert_ne!(ai.nearest_enemy(&g, 0, uid, &[bp]), Some(bpos));
}

#[test]
fn first_step_ties_favor_movement_but_real_positional_losses_do_not() {
    let g = Game::new_full(1, 20, 14, 34, 30, 0, false);
    let warrior = g
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| g.units[uid].kind == "warrior")
        .unwrap();
    let ai = BasicAi::new();

    assert!(ai.move_beats_holding(&g, warrior, 10.0, 10.0));
    assert!(!ai.move_beats_holding(&g, warrior, 5.5, 10.0));

    let mut already_moved = g;
    already_moved.units.get_mut(&warrior).unwrap().moved = true;
    assert!(!ai.move_beats_holding(&already_moved, warrior, 10.0, 10.0));
}

#[test]
fn most_idle_peacetime_troops_patrol_instead_of_fortifying() {
    let mut g = Game::new_full(1, 24, 16, 35, 30, 0, false);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| g.units[uid].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let capital = g.cities[&g.player_city_ids(0)[0]].pos;
    let staging: Vec<Pos> = g
        .wdisk(capital, 5)
        .into_iter()
        .filter(|pos| {
            g.map.get(*pos).is_some_and(|tile| {
                g.rules.is_passable(tile)
                    && !g.rules.is_water(tile)
                    && g.unit_ids_at(*pos).is_empty()
            })
        })
        .take(5)
        .collect();
    assert_eq!(
        staging.len(),
        5,
        "test map needs open land near the capital"
    );
    for pos in staging {
        g.spawn_test_unit("warrior", 0, pos);
    }
    g.players[0].explored.extend(g.map.tiles.keys().copied());

    let military: Vec<u32> = g
        .player_unit_ids(0)
        .into_iter()
        .filter(|uid| g.rules.units[g.units[uid].kind].class == "military")
        .collect();
    let mut ai = BasicAi::new();
    ai.units(&mut g, 0);
    let moved = military.iter().filter(|uid| g.units[uid].moved).count();

    assert_eq!(
        ai.patrol_posts.len(),
        1,
        "same-domain troops should share one frontier scan per turn"
    );
    assert!(
        moved * 2 > military.len(),
        "expected most idle troops to patrol; moved {moved}/{}",
        military.len()
    );
}

/// The open stretch of land `most_wartime_troops_advance_when_a_campaign_route_exists`
/// needs: a target well clear of every starting unit, with six tiles three
/// to six hexes out that troops can stage on and march off.
fn arena(g: &Game) -> Option<(Pos, Vec<Pos>)> {
    g.map
        .tiles
        .iter()
        .filter(|(pos, tile)| {
            g.rules.is_passable(tile)
                    && !g.rules.is_water(tile)
                    && g.unit_ids_at(**pos).is_empty()
                    // The campaign must march, not brawl: keep the arena away
                    // from everyone's starting units.
                    && g.units.values().all(|unit| g.wdist(unit.pos, **pos) > 8)
        })
        .find_map(|(target, _)| {
            let staging: Vec<Pos> = g
                .wdisk(*target, 6)
                .into_iter()
                .filter(|pos| {
                    (3..=6).contains(&g.wdist(*target, *pos))
                            && g.map.get(*pos).is_some_and(|tile| {
                                g.rules.is_passable(tile)
                                    && !g.rules.is_water(tile)
                                    && g.unit_ids_at(*pos).is_empty()
                            })
                            // Troops staged here must be able to march out.
                            && g.nbrs(*pos)
                                .iter()
                                .filter(|neighbour| {
                                    g.map.get(**neighbour).is_some_and(|tile| {
                                        g.rules.is_passable(tile) && !g.rules.is_water(tile)
                                    })
                                })
                                .count()
                                >= 4
                })
                .take(6)
                .collect();
            (staging.len() == 6).then_some((*target, staging))
        })
}

#[test]
fn most_wartime_troops_advance_when_a_campaign_route_exists() {
    // The fixture needs an arena: a stretch of open land well clear of
    // everybody's starting units, with room to stage six warriors around
    // one target. That is a fact about the map rather than the thing under
    // test, so take the first seed that offers one instead of naming a seed
    // and trusting that starts never move again.
    let (mut g, target, staging) = (36..96u64)
        .find_map(|seed| {
            let mut g = Game::new_full(2, 24, 16, seed, 30, 0, false);
            g.at_war.insert((0, 1));
            let found = arena(&g)?;
            Some((g, found.0, found.1))
        })
        .expect("no seed offered a map with an open land campaign");
    g.spawn_test_unit("warrior", 1, target);
    let army: Vec<u32> = staging
        .into_iter()
        .map(|pos| g.spawn_test_unit("warrior", 0, pos))
        .collect();

    let mut ai = BasicAi::new();
    for uid in &army {
        for _ in 0..8 {
            if !g.units.contains_key(uid)
                || g.units[uid].moves_left <= 0.0
                || !ai.military_step(&mut g, 0, *uid)
            {
                break;
            }
        }
    }
    let moved = army
        .iter()
        .filter(|uid| g.units.get(uid).is_some_and(|unit| unit.moved))
        .count();
    assert!(
        moved * 2 > army.len(),
        "expected most campaign troops to advance; moved {moved}/{}",
        army.len()
    );
}

#[test]
fn military_roster_maps_to_distinct_strategic_doctrines() {
    let mut g = Game::new_full(1, 24, 16, 37, 30, 0, false);
    let positions: Vec<Pos> = g
        .map
        .tiles
        .keys()
        .copied()
        .filter(|pos| g.unit_ids_at(*pos).is_empty())
        .take(9)
        .collect();
    let cases = [
        ("scout", UnitDoctrine::Recon),
        ("swordsman", UnitDoctrine::Assault),
        ("horseman", UnitDoctrine::Mobile),
        ("archer", UnitDoctrine::Ranged),
        ("catapult", UnitDoctrine::Siege),
        ("battering_ram", UnitDoctrine::Support),
        ("biplane", UnitDoctrine::AirDefense),
        ("bomber", UnitDoctrine::AirStrike),
        ("aircraft_carrier", UnitDoctrine::Carrier),
    ];
    for ((kind, expected), pos) in cases.into_iter().zip(positions) {
        let uid = g.spawn_test_unit(kind, 0, pos);
        assert_eq!(BasicAi::unit_doctrine(&g, uid), expected, "{kind}");
    }
}

#[test]
fn projected_counter_damage_prices_move_and_attack_return_fire() {
    let mut g = Game::new_full(2, 24, 16, 37_104, 30, 0, false);
    for unit in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(unit);
    }
    g.at_war.insert((0, 1));
    let enemy_pos = *g
        .map
        .tiles
        .iter()
        .find(|(_, tile)| g.rules.is_passable(tile) && !g.rules.is_water(tile))
        .map(|(position, _)| position)
        .unwrap();
    let exposed = g
        .wdisk(enemy_pos, 2)
        .into_iter()
        .find(|position| g.wdist(*position, enemy_pos) == 2)
        .unwrap();
    let safe = g
        .wdisk(enemy_pos, 3)
        .into_iter()
        .find(|position| g.wdist(*position, enemy_pos) == 3)
        .unwrap();
    let archer = g.spawn_test_unit("archer", 0, safe);
    let warrior = g.spawn_test_unit("warrior", 1, enemy_pos);
    g.units.get_mut(&warrior).unwrap().moves_left = 0.0;
    let ai = BasicAi::new();

    assert!(ai.projected_counter_damage(&g, archer, exposed, &[warrior]) > 0.0);
    assert_eq!(
        ai.projected_counter_damage(&g, archer, safe, &[warrior]),
        0.0
    );
}

/// Flatten a disk to bare passable plains so a scenario's visibility,
/// line of sight, and movement costs are exactly what it placed there.
fn flatten_disk(g: &mut Game, center: Pos, radius: i32) {
    for pos in g.wdisk(center, radius) {
        let Some(tile) = g.map.tiles.get_mut(&pos) else {
            continue;
        };
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
    }
}

/// A tile whose whole radius-`radius` disk is on the map, so a scenario
/// built around it never loses a ring tile to the map edge.
fn open_anchor(g: &Game, radius: i32) -> Pos {
    let full = (1 + 3 * radius * (radius + 1)) as usize;
    g.map
        .tiles
        .keys()
        .copied()
        .find(|pos| g.wdisk(*pos, radius).len() == full)
        .expect("a tile with a full disk exists")
}

/// The candidate loop scores statically, so before the legality screen a
/// refused shot could win the argmax and the legal runner-up was never
/// taken — the unit did nothing offensive at all. Census: 519 authoritative
/// combat-order refusals in one 150-turn deployment game, all from that
/// loop. Both halves are pinned: the frozen identities keep the historical
/// shadowing, production drops the refused candidate and takes the shot.
#[test]
fn a_refused_shot_no_longer_shadows_the_legal_one() {
    let build = || {
        let mut g = Game::new_full(2, 24, 16, 37_106, 30, 0, false);
        for unit in g.units.keys().copied().collect::<Vec<_>>() {
            g.remove_unit(unit);
        }
        g.at_war.insert((0, 1));
        let anchor = open_anchor(&g, 3);
        flatten_disk(&mut g, anchor, 3);
        let ring2: Vec<Pos> = g
            .wdisk(anchor, 2)
            .into_iter()
            .filter(|pos| g.wdist(*pos, anchor) == 2)
            .collect();
        let blocked_pos = ring2[0];
        // The far side of the ring, so walling off one corridor leaves
        // the other target's corridor untouched.
        let clear_pos = *ring2
            .iter()
            .max_by_key(|pos| (g.wdist(**pos, blocked_pos), pos.0, pos.1))
            .unwrap();
        let corridor: Vec<Pos> = g
            .nbrs(anchor)
            .into_iter()
            .filter(|pos| g.wdist(*pos, blocked_pos) == 1)
            .collect();
        for middle in corridor {
            g.map.tiles.get_mut(&middle).unwrap().terrain = crate::name!("mountain");
        }
        let archer = g.spawn_test_unit("archer", 0, anchor);
        let blocked = g.spawn_test_unit("warrior", 1, blocked_pos);
        let clear = g.spawn_test_unit("warrior", 1, clear_pos);
        // A near-dead target outscores a full-health one, so the refused
        // shot is the one the argmax wants.
        g.units.get_mut(&blocked).unwrap().hp = 5;
        g.units.get_mut(&blocked).unwrap().moves_left = 0.0;
        g.units.get_mut(&clear).unwrap().moves_left = 0.0;
        (g, archer, blocked, clear, blocked_pos, clear_pos)
    };

    // Engine preconditions: the wall makes one shot illegal, not both.
    let (g, archer, _, _, blocked_pos, clear_pos) = build();
    let frames = (g.player_vision_frame(0), g.visibility_viewers(0));
    assert!(
        !g.ranged_order_is_legal(0, archer, blocked_pos, &frames.0, &frames.1),
        "the walled corridor must refuse the shot"
    );
    assert!(
        g.ranged_order_is_legal(0, archer, clear_pos, &frames.0, &frames.1),
        "the open corridor must allow the shot"
    );
    // Scoring precondition: the refused candidate wins the argmax, so the
    // scenario really exercises shadowing rather than a lucky ordering.
    let ai = BasicAi::new();
    let utility =
        |pos: Pos| ai.exchange_score(&g, archer, pos, true) - ai.attack_threshold(&g, archer, pos);
    assert!(
        utility(blocked_pos) > utility(clear_pos) && utility(clear_pos) > 0.0,
        "the blocked kill must outscore the clear shot, and the clear \
             shot must clear the threshold on its own"
    );

    // The frozen candidate set: the refused winner shadows the legal shot
    // and nobody is hit. This is the recorded ladders' behavior.
    let (mut g, archer, blocked, clear, _, _) = build();
    let mut ai = BasicAi::new();
    ai.military_step(&mut g, 0, archer);
    assert_eq!(
        g.units[&blocked].hp, 5,
        "no shot can reach the walled target"
    );
    assert_eq!(g.units[&clear].hp, 100, "the legal shot was shadowed");

    // The screened candidate set: the doomed candidate never enters the
    // argmax, and the legal shot is taken the same turn.
    let (mut g, archer, blocked, clear, _, _) = build();
    let mut ai = BasicAi::new();
    ai.legal_tactical_candidates = true;
    assert!(ai.military_step(&mut g, 0, archer));
    assert_eq!(
        g.units[&blocked].hp, 5,
        "the walled target still cannot be hit"
    );
    assert!(
        g.units[&clear].hp < 100,
        "the legal shot is taken instead of being shadowed"
    );
}

/// The per-unit action loop is the whole mechanism behind same-turn
/// multi-step play: each `military_step` call performs one action, and
/// the `0..8` loop in `units` re-invokes it while movement remains. An
/// archer with two movement points closes one hex and fires the same
/// turn.
#[test]
fn an_archer_steps_once_and_shoots_in_the_same_turn() {
    let mut g = Game::new_full(2, 24, 16, crate::rng::fixture_seed("AUTO_36", 37_107), 30, 0, false);
    for unit in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(unit);
    }
    g.at_war.insert((0, 1));
    let anchor = open_anchor(&g, 4);
    flatten_disk(&mut g, anchor, 4);
    let start = g
        .wdisk(anchor, 3)
        .into_iter()
        .find(|pos| g.wdist(*pos, anchor) == 3)
        .unwrap();
    let warrior = g.spawn_test_unit("warrior", 1, anchor);
    g.units.get_mut(&warrior).unwrap().hp = 50;
    g.units.get_mut(&warrior).unwrap().moves_left = 0.0;
    let archer = g.spawn_test_unit("archer", 0, start);
    let mut ai = BasicAi::new();
    ai.legal_tactical_candidates = true;
    ai.units(&mut g, 0);
    assert_ne!(g.units[&archer].pos, start, "the archer closed the gap");
    assert!(
        g.units[&warrior].hp < 50,
        "and fired in the same turn from the tile the step opened"
    );
}

/// A settler with movement to spare founds on arrival rather than
/// standing on its site until the next turn.
#[test]
fn a_settler_steps_once_and_founds_in_the_same_turn() {
    let mut g = Game::new_full(2, 24, 16, 37_108, 30, 0, false);
    for unit in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(unit);
    }
    let anchor = open_anchor(&g, 3);
    flatten_disk(&mut g, anchor, 3);
    let target = g.nbrs(anchor)[0];
    let settler = g.spawn_test_unit("settler", 0, anchor);
    let mut ai = BasicAi::new();
    assert!(ai.valid_settle_site(&g, 0, target));
    ai.settler_targets.insert(settler, target);
    ai.units(&mut g, 0);
    assert!(
        g.player_city_ids(0)
            .iter()
            .any(|city| g.cities[city].pos == target),
        "the settler stepped onto its site and founded the same turn"
    );
}

/// A builder with movement to spare improves the tile it walked to rather
/// than standing on the job until the next turn.
#[test]
fn a_builder_steps_once_and_improves_in_the_same_turn() {
    let mut g = Game::new_full(1, 20, 14, 37_109, 40, 0, false);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|id| g.units[id].kind == "settler")
        .unwrap();
    let start = g.units[&settler].pos;
    flatten_disk(&mut g, start, 1);
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = g.player_city_ids(0)[0];
    let center = g.cities[&city].pos;
    for unit in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(unit);
    }
    let builder = g.spawn_test_unit("builder", 0, center);
    g.units.get_mut(&builder).unwrap().charges = 3;
    let mut ai = BasicAi::new();
    ai.units(&mut g, 0);
    assert_ne!(
        g.units[&builder].pos, center,
        "the builder stepped off the city center toward its job"
    );
    assert_eq!(
        g.units[&builder].charges, 2,
        "and spent a charge the same turn"
    );
    assert!(
        g.map.tiles[&g.units[&builder].pos].improvement.is_some(),
        "the tile it stands on carries the new improvement"
    );
}

#[test]
fn light_cavalry_raids_first_while_an_attack_opportunity_takes_priority() {
    let mut g = Game::new_full(2, 24, 16, 37_106, 30, 0, false);
    for unit in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(unit);
    }
    g.at_war.insert((0, 1));
    let (heavy_position, enemy_position, light_position) = g
        .map
        .tiles
        .iter()
        .filter(|(_, tile)| g.rules.is_passable(tile) && !g.rules.is_water(tile))
        .find_map(|(position, _)| {
            let neighbors: Vec<Pos> = g
                .nbrs(*position)
                .into_iter()
                .filter(|neighbor| {
                    g.map
                        .get(*neighbor)
                        .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
                })
                .take(2)
                .collect();
            (neighbors.len() == 2).then_some((*position, neighbors[0], neighbors[1]))
        })
        .expect("test map needs two open neighbors");
    let light = g.spawn_test_unit("horseman", 0, light_position);
    let heavy = g.spawn_test_unit("heavy_chariot", 0, heavy_position);
    let enemy = g.spawn_test_unit("warrior", 1, enemy_position);
    g.units.get_mut(&enemy).unwrap().hp = 1;
    for position in [light_position, heavy_position] {
        g.map.tiles.get_mut(&position).unwrap().improvement = Some(crate::name!("barbarian_camp"));
    }
    let mut ai = BasicAi::new();

    assert!(matches!(
        ai.doctrine_action(&g, 0, light),
        Some(Action::Pillage { unit }) if unit == light
    ));
    assert_eq!(ai.doctrine_action(&g, 0, heavy), None);
    assert!(ai.military_step(&mut g, 0, heavy));
    assert!(matches!(
        g.log.last(),
        Some((0, Action::Attack { unit, target }))
            if *unit == heavy && *target == enemy_position
    ));
    assert_eq!(
        g.map.tiles[&heavy_position].improvement.as_deref(),
        Some("barbarian_camp"),
        "heavy cavalry should leave pillaging until after an attack opportunity"
    );
}

#[test]
fn scout_avoids_damage_while_strong_assault_unit_attacks() {
    let mut g = Game::new_full(2, 24, 16, 38, 30, 0, false);
    g.at_war.insert((0, 1));
    let (enemy_pos, scout_pos, assault_pos, hidden) = g
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| {
            g.rules.is_passable(tile) && !g.rules.is_water(tile) && g.unit_ids_at(**pos).is_empty()
        })
        .find_map(|(center, _)| {
            let ring: Vec<Pos> = g
                .nbrs(*center)
                .into_iter()
                .filter(|pos| {
                    g.map.get(*pos).is_some_and(|tile| {
                        g.rules.is_passable(tile)
                            && !g.rules.is_water(tile)
                            && g.unit_ids_at(*pos).is_empty()
                    })
                })
                .collect();
            if ring.len() < 3 {
                return None;
            }
            let scout = ring[0];
            let hidden = ring
                .iter()
                .copied()
                .skip(1)
                .find(|pos| g.wdist(scout, *pos) == 1)?;
            let assault = ring
                .iter()
                .copied()
                .find(|pos| *pos != scout && *pos != hidden)?;
            Some((*center, scout, assault, hidden))
        })
        .expect("test map needs an open tactical ring");
    // The scout is meant to demonstrate exploration choice, not ignore a
    // lethal ambush. A Warrior still gives the assault unit a real attack
    // target without making this full-health Scout evacuate first.
    let enemy = g.spawn_test_unit("warrior", 1, enemy_pos);
    let scout = g.spawn_test_unit("scout", 0, scout_pos);
    let assault = g.spawn_test_unit("giant_death_robot", 0, assault_pos);
    g.players[0].explored.extend(g.map.tiles.keys().copied());
    g.players[0].explored.remove(&hidden);

    let mut ai = BasicAi::new();
    assert!(ai.military_step(&mut g, 0, scout));
    assert!(matches!(g.log.last(), Some((0, Action::Move { unit, .. })) if *unit == scout));
    let envelopes = ai.enemy_attack_envelopes(&g, 0);
    let selected = BasicAi::incoming_damage(&g, 0, scout, g.units[&scout].pos, &envelopes).total;
    let exposed = BasicAi::incoming_damage(&g, 0, scout, hidden, &envelopes).total;
    assert!(
        selected <= exposed,
        "exploration must not override a safer route"
    );
    assert!(g.units.contains_key(&enemy));

    assert!(
        ai.attack_threshold(&g, assault, enemy_pos) < ai.attack_threshold(&g, scout, enemy_pos),
        "strong assault units should have a more aggressive attack threshold"
    );
    assert!(ai.military_step(&mut g, 0, assault));
    assert!(
        matches!(
            g.log.last(),
            Some((0, Action::Attack { unit, target } | Action::Ranged { unit, target }))
                if *unit == assault && *target == enemy_pos
        ),
        "unexpected assault decision: {:?}",
        g.log.last()
    );
}

#[test]
fn production_scout_collects_a_visible_reachable_goody_hut_before_exploring_fog() {
    let mut g = Game::new_full(1, 24, 16, 38_001, 30, 0, false);
    for tile in g.map.tiles.values_mut() {
        if tile.improvement.as_deref() == Some("goody_hut") {
            tile.improvement = None;
        }
    }
    let (origin, hidden, hut) = g
        .map
        .tiles
        .iter()
        .filter(|(origin, tile)| {
            g.rules.is_passable(tile)
                && !g.rules.is_water(tile)
                && g.unit_ids_at(**origin).is_empty()
                && g.city_at(**origin).is_none()
        })
        .find_map(|(origin, _)| {
            let mut neighbors: Vec<Pos> = g
                .nbrs(*origin)
                .into_iter()
                .filter(|position| {
                    g.map.get(*position).is_some_and(|tile| {
                        g.rules.is_passable(tile)
                            && !g.rules.is_water(tile)
                            && g.unit_ids_at(*position).is_empty()
                            && g.city_at(*position).is_none()
                    })
                })
                .collect();
            neighbors.sort();
            (neighbors.len() >= 2).then_some((*origin, neighbors[0], neighbors[1]))
        })
        .expect("test map needs two open tiles beside a scout");
    for position in [origin, hidden, hut] {
        let tile = g.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
    }
    g.map.tiles.get_mut(&hut).unwrap().improvement = Some(crate::name!("goody_hut"));
    let scout = g.spawn_test_unit("scout", 0, origin);
    g.players[0].explored.extend(g.map.tiles.keys().copied());
    g.players[0].explored.remove(&hidden);

    assert!(
        g.player_can_see(0, hut),
        "the tribal village needs to be in the scout's current sight"
    );

    // This is a production improvement, not a rewrite of the frozen
    // Basic/advanced_v1 route: their scout keeps pursuing the unseen tile.
    assert!(!BasicAi::new().hut_collection);
    let mut frozen = g.clone();
    let mut frozen_ai = BasicAi::new();
    assert!(frozen_ai.military_step(&mut frozen, 0, scout));
    assert_eq!(frozen.units[&scout].pos, hidden);
    assert_eq!(
        frozen.map.tiles[&hut].improvement.as_deref(),
        Some("goody_hut")
    );

    // `hut_collection` carries the pickup on its own.
    let mut collected = g.clone();
    let mut collector = BasicAi::new();
    collector.hut_collection = true;
    assert!(collector.military_step(&mut collected, 0, scout));
    assert_eq!(collected.units[&scout].pos, hut);
    assert!(collected.map.tiles[&hut].improvement.is_none());

    // A crashed meteor is the same expiring prize with a richer table:
    // the pickup collects it through the identical branch.
    let mut meteor_board = frozen.clone();
    meteor_board.map.tiles.get_mut(&hut).unwrap().improvement = Some(crate::name!("meteor_goody"));
    let mut meteor_ai = BasicAi::new();
    meteor_ai.hut_collection = true;
    assert!(meteor_ai.military_step(&mut meteor_board, 0, scout));
    assert_eq!(meteor_board.units[&scout].pos, hut);
    assert!(meteor_board.map.tiles[&hut].improvement.is_none());
}

#[test]
fn a_visible_adjacent_empty_barbarian_camp_is_cleared_before_peacetime_movement() {
    let mut g = Game::new_full(1, 24, 16, 38_006, 30, 0, true);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    for camp in g.barb_camps.keys().copied().collect::<Vec<_>>() {
        g.barb_camps.remove(&camp);
        if g.map.tiles[&camp].improvement.as_deref() == Some("barbarian_camp") {
            g.map.tiles.get_mut(&camp).unwrap().improvement = None;
        }
    }
    g.barb_naval_camps.clear();
    g.barb_camp_guards.clear();
    g.barb_scout_homes.clear();
    g.barb_scout_targets.clear();
    g.barb_camp_targets.clear();
    let (origin, camp) = g
        .map
        .tiles
        .iter()
        .filter(|(origin, tile)| {
            g.rules.is_passable(tile) && !g.rules.is_water(tile) && g.city_at(**origin).is_none()
        })
        .find_map(|(origin, _)| {
            g.nbrs(*origin)
                .into_iter()
                .find(|camp| {
                    g.map.get(*camp).is_some_and(|tile| {
                        g.rules.is_passable(tile)
                            && !g.rules.is_water(tile)
                            && g.city_at(*camp).is_none()
                    })
                })
                .map(|camp| (*origin, camp))
        })
        .expect("fixture needs a pair of adjacent open land tiles");
    let warrior = g.spawn_test_unit("warrior", 0, origin);
    g.barb_camps.insert(camp, g.turn + 1_000);
    g.map.tiles.get_mut(&camp).unwrap().improvement = Some(crate::name!("barbarian_camp"));

    assert!(
        g.player_can_see(0, camp),
        "the adjacent camp is a real sighting"
    );
    assert!(g.can_move(warrior, camp), "the clear is a legal move");
    let mut ai = BasicAi::new();
    assert!(ai.military_step(&mut g, 0, warrior));
    assert_eq!(g.units[&warrior].pos, camp);
    assert!(
        !g.barb_camps.contains_key(&camp),
        "entering an empty camp must clear it rather than leave the unit beside it"
    );
    assert_eq!(g.players[0].counters.get("camps"), Some(&1));
    assert!(matches!(
        g.log.last(),
        Some((0, Action::Move { unit, to })) if *unit == warrior && *to == camp
    ));
}

#[test]
fn a_seeking_scout_walks_to_a_charted_village_beyond_this_turns_reach() {
    let mut g = Game::new_full(1, 24, 16, 38_003, 30, 0, false);
    for unit in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(unit);
    }
    g.map.clear_rivers();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
    }
    let origin = *g.map.tiles.keys().min().expect("the map has tiles");
    let village = g
        .wdisk(origin, VILLAGE_SEEK_RADIUS)
        .into_iter()
        .filter(|pos| g.wdist(origin, *pos) == 5)
        .min()
        .expect("a tile five steps out exists");
    g.map.tiles.get_mut(&village).unwrap().improvement = Some(crate::name!("goody_hut"));
    let scout = g.spawn_test_unit("scout", 0, origin);
    // The whole board is charted: no fog competes with the village, and
    // the village is far outside this turn's reach (three movement).
    g.players[0].explored.extend(g.map.tiles.keys().copied());
    assert!(!g.reachable(scout).contains(&village));

    // Without the flag nothing moves — no fog, no this-turn village.
    let mut frozen = g.clone();
    assert!(!BasicAi::new().village_seeking);
    assert!(!BasicAi::new().explore_step(&mut frozen, 0, scout));
    assert_eq!(frozen.units[&scout].pos, origin);

    // With it, the scout closes on the charted village.
    let mut seeker = BasicAi::new();
    seeker.village_seeking = true;
    let mut sought = g.clone();
    assert!(seeker.explore_step(&mut sought, 0, scout));
    assert!(
        g.wdist(sought.units[&scout].pos, village) < g.wdist(origin, village),
        "the seeking step must close the distance to the village"
    );

    // One chaser per village: a strictly closer fellow scout releases
    // this one back to the frontier.
    let mut crowded = g.clone();
    let closer = crowded
        .nbrs(village)
        .into_iter()
        .min()
        .expect("the village has a neighbour");
    crowded.spawn_test_unit("scout", 0, closer);
    let mut yielding = BasicAi::new();
    yielding.village_seeking = true;
    assert!(!yielding.explore_step(&mut crowded, 0, scout));
    assert_eq!(crowded.units[&scout].pos, origin);
}

#[test]
fn a_meteor_ruin_outranks_a_nearer_village_and_uses_the_closest_collector() {
    let mut g = Game::new_full(1, 48, 32, 38_007, 30, 0, false);
    for unit in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(unit);
    }
    g.map.clear_rivers();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
    }
    let (meteor, scout_origin, hut, warrior_origin) = g
        .map
        .tiles
        .keys()
        .copied()
        .find_map(|meteor| {
            let scout_origin = g
                .wdisk(meteor, METEOR_SEEK_RADIUS)
                .into_iter()
                .filter(|pos| g.wdist(meteor, *pos) == METEOR_SEEK_RADIUS)
                .min()?;
            let warrior_origin = g
                .wdisk(meteor, METEOR_MILITARY_SEEK_RADIUS)
                .into_iter()
                .filter(|pos| g.wdist(meteor, *pos) == METEOR_MILITARY_SEEK_RADIUS)
                .min()?;
            let hut = g
                .nbrs(scout_origin)
                .into_iter()
                .filter(|pos| *pos != meteor && *pos != warrior_origin)
                .min()?;
            Some((meteor, scout_origin, hut, warrior_origin))
        })
        .expect("test map needs a meteor ring wider than an ordinary village chase");
    g.map.tiles.get_mut(&meteor).unwrap().improvement = Some(crate::name!("meteor_goody"));
    g.map.tiles.get_mut(&hut).unwrap().improvement = Some(crate::name!("goody_hut"));
    let scout = g.spawn_test_unit("scout", 0, scout_origin);
    g.players[0].explored.extend(g.map.tiles.keys().copied());

    assert!(g.reachable(scout).contains(&hut));
    assert!(!g.reachable(scout).contains(&meteor));
    assert!(
        g.wdist(scout_origin, meteor) > VILLAGE_SEEK_RADIUS,
        "the meteor must be beyond the ordinary village horizon"
    );

    let mut ai = BasicAi::new();
    ai.village_seeking = true;
    assert_eq!(
        ai.village_collection_target(&g, 0, scout),
        Some(meteor),
        "the guaranteed free Heavy Cavalry should outrank the adjacent random village"
    );
    let mut scout_board = g.clone();
    assert!(ai.village_collection_step(&mut scout_board, 0, scout));
    assert!(
        scout_board.wdist(scout_board.units[&scout].pos, meteor)
            < scout_board.wdist(scout_origin, meteor),
        "the Scout must close on the distant meteor instead of taking the nearer hut"
    );

    let warrior = g.spawn_test_unit("warrior", 0, warrior_origin);
    assert_eq!(
        ai.village_collection_target(&g, 0, warrior),
        Some(meteor),
        "a nearby field unit should chase the meteor within its extended horizon"
    );
    assert_eq!(
        ai.village_collection_target(&g, 0, scout),
        Some(hut),
        "the farther Scout should release the meteor to the closer Warrior"
    );
    let before = g.wdist(warrior_origin, meteor);
    assert!(ai.village_collection_step(&mut g, 0, warrior));
    assert!(
        g.wdist(g.units[&warrior].pos, meteor) < before,
        "the field unit must close on the meteor"
    );
}

#[test]
fn a_nearby_military_unit_claims_a_charted_village_without_retasking_civilians() {
    let mut g = Game::new_full(1, 24, 16, 38_004, 30, 0, false);
    for unit in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(unit);
    }
    g.map.clear_rivers();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
    }
    let origin = *g.map.tiles.keys().min().expect("the map has tiles");
    let village = g
        .wdisk(origin, VILLAGE_MILITARY_SEEK_RADIUS)
        .into_iter()
        .find(|pos| g.wdist(origin, *pos) == VILLAGE_MILITARY_SEEK_RADIUS)
        .expect("a tile four steps out exists");
    let civilians: Vec<Pos> = g
        .map
        .tiles
        .keys()
        .copied()
        .filter(|pos| g.wdist(origin, *pos) > VILLAGE_MILITARY_SEEK_RADIUS + 2)
        .take(2)
        .collect();
    assert_eq!(civilians.len(), 2, "the map has room away from the errand");
    g.map.tiles.get_mut(&village).unwrap().improvement = Some(crate::name!("goody_hut"));
    let warrior = g.spawn_test_unit("warrior", 0, origin);
    let builder = g.spawn_test_unit("builder", 0, civilians[0]);
    let settler = g.spawn_test_unit("settler", 0, civilians[1]);
    g.players[0].explored.extend(g.map.tiles.keys().copied());

    let mut ai = BasicAi::new();
    ai.village_seeking = true;
    let builder_before = g.units[&builder].pos;
    let settler_before = g.units[&settler].pos;
    assert!(
        !ai.village_collection_step(&mut g, 0, builder)
            && !ai.village_collection_step(&mut g, 0, settler),
        "the village collector must reject civilian unit roles"
    );
    assert_eq!(g.units[&builder].pos, builder_before);
    assert_eq!(g.units[&settler].pos, settler_before);

    assert!(ai.military_step(&mut g, 0, warrior));
    assert!(
        g.wdist(g.units[&warrior].pos, village) < g.wdist(origin, village),
        "with no Scout able to claim it, the nearby Warrior should close on the village"
    );
}

#[test]
fn a_scout_keeps_a_long_village_errand_over_the_military_fallback() {
    let mut g = Game::new_full(1, 24, 16, 38_005, 30, 0, false);
    for unit in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(unit);
    }
    g.map.clear_rivers();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
    }
    let scout_origin = *g.map.tiles.keys().min().expect("the map has tiles");
    let village = g
        .wdisk(scout_origin, VILLAGE_SEEK_RADIUS)
        .into_iter()
        .find(|pos| g.wdist(scout_origin, *pos) == 5)
        .expect("a tile five steps from the Scout exists");
    let warrior_origin = g
        .wdisk(village, VILLAGE_MILITARY_SEEK_RADIUS)
        .into_iter()
        .find(|pos| g.wdist(village, *pos) == VILLAGE_MILITARY_SEEK_RADIUS && *pos != scout_origin)
        .expect("a tile four steps from the village exists");
    g.map.tiles.get_mut(&village).unwrap().improvement = Some(crate::name!("goody_hut"));
    let scout = g.spawn_test_unit("scout", 0, scout_origin);
    let warrior = g.spawn_test_unit("warrior", 0, warrior_origin);
    g.players[0].explored.extend(g.map.tiles.keys().copied());

    let mut ai = BasicAi::new();
    ai.village_seeking = true;
    assert_eq!(
        ai.village_collection_target(&g, 0, warrior),
        None,
        "a long military detour yields when a Scout can collect the village"
    );
    assert_eq!(ai.village_collection_target(&g, 0, scout), Some(village));
    assert!(ai.village_collection_step(&mut g, 0, scout));
    assert!(
        g.wdist(g.units[&scout].pos, village) < g.wdist(scout_origin, village),
        "the Scout should be sent toward the village immediately"
    );
}

#[test]
fn raiders_and_aircraft_use_their_specialized_actions() {
    let mut g = Game::new_full(2, 24, 16, 39, 30, 0, false);
    g.at_war.insert((0, 1));
    for unit in g.player_unit_ids(1) {
        g.units.get_mut(&unit).unwrap().owner = 0;
    }
    let positions: Vec<Pos> = g
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| {
            g.rules.is_passable(tile) && !g.rules.is_water(tile) && g.unit_ids_at(**pos).is_empty()
        })
        // Demand elbow room rather than taking whichever tiles the map
        // lists first: the air base needs a free land tile beside it that
        // the other two staged units are not already standing on.
        .filter(|(pos, _)| {
            g.nbrs(**pos)
                .into_iter()
                .filter(|neighbor| {
                    g.map.get(*neighbor).is_some_and(|tile| {
                        g.rules.is_passable(tile)
                            && !g.rules.is_water(tile)
                            && g.unit_ids_at(*neighbor).is_empty()
                    })
                })
                .count()
                >= 3
        })
        .map(|(pos, _)| *pos)
        .take(3)
        .collect();
    let air_target = g
        .nbrs(positions[2])
        .into_iter()
        .find(|pos| {
            !positions.contains(pos)
                && g.map.get(*pos).is_some_and(|tile| {
                    g.rules.is_passable(tile)
                        && !g.rules.is_water(tile)
                        && g.unit_ids_at(*pos).is_empty()
                })
        })
        .expect("test map needs a land target beside the air base");
    let raider = g.spawn_test_unit("horseman", 0, positions[0]);
    let assault = g.spawn_test_unit("swordsman", 0, positions[1]);
    g.map.tiles.get_mut(&positions[0]).unwrap().improvement = Some(crate::name!("barbarian_camp"));
    g.map.tiles.get_mut(&positions[1]).unwrap().improvement = Some(crate::name!("barbarian_camp"));
    let fighter = g.spawn_test_unit("biplane", 0, positions[2]);
    let bomber = g.spawn_test_unit("bomber", 0, positions[2]);
    g.spawn_test_unit("modern_armor", 1, air_target);
    let ai = BasicAi::new();

    let full_legal = g.legal_actions(0);
    for uid in [raider, assault, fighter, bomber] {
        let expected: Vec<Action> = full_legal
            .iter()
            .filter(|action| match action {
                Action::Pillage { unit }
                | Action::AirRebase { unit, .. }
                | Action::AirStrike { unit, .. }
                | Action::AirPillage { unit, .. }
                | Action::PriorityTarget { unit, .. }
                | Action::AirPatrol { unit, .. }
                | Action::CoastalRaid { unit, .. } => *unit == uid,
                _ => false,
            })
            .cloned()
            .collect();
        assert_eq!(g.legal_doctrine_actions(0, uid), expected);
    }

    assert!(matches!(
        ai.doctrine_action(&g, 0, raider),
        Some(Action::Pillage { unit }) if unit == raider
    ));
    assert_eq!(ai.doctrine_action(&g, 0, assault), None);
    assert!(matches!(
        ai.doctrine_action(&g, 0, fighter),
        Some(Action::AirPatrol { unit, .. }) if unit == fighter
    ));
    let bomber_action = ai.doctrine_action(&g, 0, bomber);
    assert!(
        matches!(
            bomber_action,
            Some(Action::AirStrike { unit, target })
                if unit == bomber && target == air_target
        ),
        "unexpected bomber action: {bomber_action:?}"
    );
}

#[test]
fn spec_ops_bypass_an_escort_to_priority_target_air_defense() {
    let mut game = Game::new_full(2, 24, 16, 43_015, 80, 0, false);
    game.at_war.insert((0, 1));
    for unit in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(unit);
    }
    let origin = game
        .map
        .tiles
        .iter()
        .find(|(_, tile)| game.rules.is_passable(tile) && !game.rules.is_water(tile))
        .map(|(position, _)| *position)
        .unwrap();
    let target =
        game.wdisk(origin, 2)
            .into_iter()
            .find(|position| {
                game.wdist(origin, *position) == 2
                    && game.map.get(*position).is_some_and(|tile| {
                        game.rules.is_passable(tile) && !game.rules.is_water(tile)
                    })
            })
            .unwrap();
    // Keep this doctrine fixture inside the Spec Ops unit's real sight
    // corridor. Priority Target cannot select an escorted support unit
    // hidden behind terrain at range two.
    for position in game.wdisk(origin, 2) {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
    }
    let spec_ops = game.spawn_test_unit("spec_ops", 0, origin);
    // The escort demonstrates bypass targeting. It should not itself be
    // a guaranteed one-turn kill that correctly triggers the evacuation
    // policy before the Spec Ops mission can be considered.
    game.spawn_test_unit("warrior", 1, target);
    let sam = game.spawn_test_unit("mobile_sam", 1, target);
    let mut ai = BasicAi::new();

    assert!(game.player_visibility(0).contains(&target));
    assert!(ai.military_step(&mut game, 0, spec_ops));
    assert_eq!(game.units[&sam].hp, 35);
    assert!(matches!(
        game.log.last(),
        Some((0, Action::PriorityTarget { unit, target: action_target }))
            if *unit == spec_ops && *action_target == target
    ));
}

#[test]
fn ranged_holds_firing_depth_while_mobile_unit_closes() {
    let mut g = Game::new_full(1, 24, 16, 40, 30, 0, false);
    let (target, ranged_pos, mobile_pos) = g
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| {
            g.rules.is_passable(tile) && !g.rules.is_water(tile) && g.unit_ids_at(**pos).is_empty()
        })
        .find_map(|(target, _)| {
            let ranged = g.wdisk(*target, 2).into_iter().find(|pos| {
                g.wdist(*target, *pos) == 2
                    && g.map.get(*pos).is_some_and(|tile| {
                        g.rules.is_passable(tile)
                            && !g.rules.is_water(tile)
                            && g.unit_ids_at(*pos).is_empty()
                    })
            })?;
            let mobile = g.wdisk(*target, 4).into_iter().find(|pos| {
                g.wdist(*target, *pos) == 4
                    && *pos != ranged
                    && g.map.get(*pos).is_some_and(|tile| {
                        g.rules.is_passable(tile)
                            && !g.rules.is_water(tile)
                            && g.unit_ids_at(*pos).is_empty()
                    })
            })?;
            Some((*target, ranged, mobile))
        })
        .expect("test map needs open role-spacing positions");
    let archer = g.spawn_test_unit("archer", 0, ranged_pos);
    let ai = BasicAi::new();
    ai.tactical_step(&mut g, 0, archer, target, &[], 2);
    assert_eq!(g.wdist(g.units[&archer].pos, target), 2);

    let horseman = g.spawn_test_unit("horseman", 0, mobile_pos);
    assert!(ai.tactical_step(&mut g, 0, horseman, target, &[], 1));
    assert!(g.wdist(g.units[&horseman].pos, target) < g.wdist(mobile_pos, target));
}

#[test]
fn military_picker_preserves_city_capturing_melee() {
    let mut g = Game::new_full(1, 20, 14, 31, 30, 0, false);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|id| g.units[id].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    g.players[0].techs.extend([
        crate::name!("archery"),
        crate::name!("iron_working"),
        crate::name!("machinery"),
    ]);
    let cid = g.player_city_ids(0)[0];
    let ai = BasicAi::new();

    let ranged = ai.combined_arms_unit(&g, 0, cid, 2, 0).unwrap();
    assert!(g.rules.units[ranged].has_ranged_attack());

    let melee = ai.combined_arms_unit(&g, 0, cid, 2, 2).unwrap();
    assert!(!g.rules.units[melee].has_ranged_attack());
}

/// `domination-spends-the-stock`: with the flag lent and the stock to pay for
/// it, the combined-arms pick takes a resource-priced melee unit where the
/// alternation would take a shooter. Without the stock it does not.
#[test]
fn a_lent_flag_spends_the_stock_on_priced_melee() {
    let mut g = Game::new_full(2, 20, 14, 91_770, 120, 0, false);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|id| g.units[id].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    g.players[0]
        .techs
        .extend([crate::name!("archery"), crate::name!("iron_working")]);
    let cid = g.player_city_ids(0)[0];
    let mut ai = BasicAi::new();
    ai.prefers_priced_melee = true;

    let without_stock = ai.combined_arms_unit(&g, 0, cid, 2, 0).unwrap();
    assert!(g.rules.units[without_stock].has_ranged_attack());

    g.players[0]
        .strategic_resources
        .insert(crate::name!("iron"), 30.0);
    let with_stock = ai.combined_arms_unit(&g, 0, cid, 2, 0).unwrap();
    let spec = &g.rules.units[with_stock];
    assert!(!spec.has_ranged_attack());
    assert!(spec.resource_cost > 0.0);

    ai.prefers_priced_melee = false;
    let flag_off = ai.combined_arms_unit(&g, 0, cid, 2, 0).unwrap();
    assert!(g.rules.units[flag_off].has_ranged_attack());
}

#[test]
fn military_units_capture_adjacent_enemy_civilians_by_moving() {
    let mut game = Game::new_full(2, 20, 14, 91_769, 120, 0, false);
    for pid in 0..2 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.found_city_for(pid, game.units[&settler].pos, None);
    }
    // This is a local-defense routing test. Normalize the board so a
    // resource-placement correction cannot turn its route into an
    // incidental terrain fixture.
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.pillaged = false;
        tile.district = None;
        tile.district_foundation = None;
        tile.wonder = None;
    }
    for unit in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(unit);
    }
    let origin = game.cities[&game.player_city_ids(0)[0]].pos;
    let target =
        game.nbrs(origin)
            .into_iter()
            .find(|position| {
                game.city_at(*position).is_none()
                    && game.map.get(*position).is_some_and(|tile| {
                        game.rules.is_passable(tile) && !game.rules.is_water(tile)
                    })
            })
            .unwrap();
    game.at_war.insert((0, 1));
    let warrior = game.spawn_test_unit("warrior", 0, origin);
    let builder = game.spawn_test_unit("builder", 1, target);
    let mut ai = BasicAi::new();

    assert!(ai.military_step(&mut game, 0, warrior));
    assert_eq!(game.units[&builder].owner, 0);
    assert!(matches!(
        game.log.last(),
        Some((0, Action::Move { unit, to })) if *unit == warrior && *to == target
    ));
}

/// A flattened two-city board with every stray unit removed: the shared
/// prelude of the civilian-capture tests. Returns the game and the tile
/// beside player 0's capital that a capture target stands on.
fn capture_test_board(seed: u64) -> (Game, Pos) {
    let mut game = Game::new_full(2, 20, 14, seed, 120, 0, true);
    for pid in 0..2 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.found_city_for(pid, game.units[&settler].pos, None);
    }
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.pillaged = false;
        tile.district = None;
        tile.district_foundation = None;
        tile.wonder = None;
    }
    for unit in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(unit);
    }
    let origin = game.cities[&game.player_city_ids(0)[0]].pos;
    let target =
        game.nbrs(origin)
            .into_iter()
            .find(|position| {
                game.city_at(*position).is_none()
                    && game.map.get(*position).is_some_and(|tile| {
                        game.rules.is_passable(tile) && !game.rules.is_water(tile)
                    })
            })
            .unwrap();
    (game, target)
}

/// The frozen controllers keep their recorded behavior: with the rescue
/// off, the duplicate-settler guard still declines a barbarian-held
/// settler and holds the unit.
#[test]
fn without_the_rescue_a_duplicate_settler_is_still_declined() {
    let (mut game, target) = capture_test_board(91_773);
    let origin = game.cities[&game.player_city_ids(0)[0]].pos;
    let barb = game.barb_pid.unwrap();
    let warrior = game.spawn_test_unit("warrior", 0, origin);
    let far = game.cities[&game.player_city_ids(1)[0]].pos;
    game.spawn_test_unit("settler", 0, far);
    let captured = game.spawn_test_unit("settler", barb, target);
    let mut ai = BasicAi::new();

    ai.military_step(&mut game, 0, warrior);
    assert_eq!(game.units[&captured].owner, barb);
}

/// A settler in the barbarians' hands is never declined. Live run
/// `civvis-20260826T194422Z`: our own settler, taken at t65, walked
/// unguarded beside Heavy Chariots with full movement at t76, t82, t87
/// and t88, and each time the duplicate-settler guard ordered a fortify,
/// because another settler of ours was alive or queued.
#[test]
fn a_barbarian_held_settler_is_rescued_despite_a_duplicate() {
    let (mut game, target) = capture_test_board(91_773);
    let origin = game.cities[&game.player_city_ids(0)[0]].pos;
    let barb = game.barb_pid.unwrap();
    let warrior = game.spawn_test_unit("warrior", 0, origin);
    // The duplicate that made the old guard decline: our own settler,
    // alive but far from the rescuer.
    let far = game.cities[&game.player_city_ids(1)[0]].pos;
    let own = game.spawn_test_unit("settler", 0, far);
    let captured = game.spawn_test_unit("settler", barb, target);
    let mut ai = BasicAi::new();
    ai.barbarian_settler_capture = true;

    assert!(ai.military_step(&mut game, 0, warrior));
    assert_eq!(game.units[&captured].owner, 0);
    assert_eq!(game.units[&own].owner, 0);
}

/// The frozen controllers keep their recorded behaviour: with the gene
/// off, the duplicate-settler guard still declines a barbarian-held
/// settler and holds the unit.
#[test]
fn without_the_gene_a_duplicate_settler_is_still_declined() {
    let (mut game, target) = capture_test_board(91_773);
    let origin = game.cities[&game.player_city_ids(0)[0]].pos;
    let barb = game.barb_pid.unwrap();
    let warrior = game.spawn_test_unit("warrior", 0, origin);
    let far = game.cities[&game.player_city_ids(1)[0]].pos;
    game.spawn_test_unit("settler", 0, far);
    let captured = game.spawn_test_unit("settler", barb, target);
    let mut ai = BasicAi::new();

    ai.military_step(&mut game, 0, warrior);
    assert_eq!(game.units[&captured].owner, barb);
}

/// A major rival's settler is unchanged by the gene: the
/// duplicate/unusable guard still declines it.
#[test]
fn a_duplicate_settler_from_a_major_is_still_declined_under_the_gene() {
    let (mut game, target) = capture_test_board(91_772);
    let origin = game.cities[&game.player_city_ids(0)[0]].pos;
    game.at_war.insert((0, 1));
    let warrior = game.spawn_test_unit("warrior", 0, origin);
    let far = game.cities[&game.player_city_ids(1)[0]].pos;
    game.spawn_test_unit("settler", 0, far);
    let enemy_settler = game.spawn_test_unit("settler", 1, target);
    let mut ai = BasicAi::new();
    ai.barbarian_settler_capture = true;

    ai.military_step(&mut game, 0, warrior);
    assert_eq!(game.units[&enemy_settler].owner, 1);
}

/// Capture reach is movement, not adjacency: a barbarian-held settler
/// two tiles out is walked onto within the same turn. Adjacent-only
/// capture is how the live pursuer parked *beside* the settler every
/// turn while the barbarians walked it home.
#[test]
fn a_barbarian_held_settler_two_tiles_away_is_walked_onto() {
    let (mut game, mid) = capture_test_board(91_773);
    let origin = game.cities[&game.player_city_ids(0)[0]].pos;
    let barb = game.barb_pid.unwrap();
    let target =
        game.nbrs(mid)
            .into_iter()
            .find(|position| {
                game.wdist(origin, *position) == 2
                    && game.city_at(*position).is_none()
                    && game.map.get(*position).is_some_and(|tile| {
                        game.rules.is_passable(tile) && !game.rules.is_water(tile)
                    })
            })
            .unwrap();
    let warrior = game.spawn_test_unit("warrior", 0, origin);
    let captured = game.spawn_test_unit("settler", barb, target);
    let mut ai = BasicAi::new();
    ai.barbarian_settler_capture = true;

    // The unit loop's shape: step until the unit declines or runs dry.
    for _ in 0..8 {
        if game.units[&warrior].moves_left <= 0.0 || !ai.military_step(&mut game, 0, warrior) {
            break;
        }
    }
    assert_eq!(
        game.units[&captured].owner, 0,
        "two tiles and two movement points are a capture, not a vigil"
    );
}

/// A tile at hex distance `distance` from `origin` on the flattened board,
/// with no city on it.
fn open_tile_at(game: &Game, origin: Pos, distance: i32) -> Pos {
    let mut tiles: Vec<Pos> = game
        .map
        .tiles
        .keys()
        .copied()
        .filter(|position| {
            game.wdist(origin, *position) == distance
                && game.city_at(*position).is_none()
                && game.unit_ids_at(*position).is_empty()
        })
        .collect();
    tiles.sort_unstable();
    tiles[0]
}

/// The longer walk: a barbarian-held settler four tiles out — two turns
/// for a warrior — is approached, while any other capturable civilian
/// stays within this turn's reach.
#[test]
fn a_barbarian_held_settler_four_tiles_away_is_approached() {
    let (mut game, _) = capture_test_board(91_773);
    let origin = game.cities[&game.player_city_ids(0)[0]].pos;
    let barb = game.barb_pid.unwrap();
    let target = open_tile_at(&game, origin, BARBARIAN_SETTLER_PURSUIT_RADIUS);
    // A second soldier keeps the city, so the chaser is not its only
    // garrison.
    game.spawn_test_unit("warrior", 0, origin);
    let warrior = game.spawn_test_unit("warrior", 0, origin);
    let captured = game.spawn_test_unit("settler", barb, target);
    let mut ai = BasicAi::new();

    assert!(
        !ai.pursue_capturable_civilian(&mut game, 0, warrior, true),
        "without the gene a major never pursues"
    );
    ai.barbarian_settler_capture = true;
    assert!(ai.pursue_capturable_civilian(&mut game, 0, warrior, true));
    assert!(
        game.wdist(game.units[&warrior].pos, target) < BARBARIAN_SETTLER_PURSUIT_RADIUS,
        "the first step of the walk closes on the settler"
    );
    assert_eq!(
        game.units[&captured].owner, barb,
        "not yet: the capture is next turn's"
    );

    // The same settler owned by a major at war is not worth the walk.
    let (mut game, _) = capture_test_board(91_773);
    game.at_war.insert((0, 1));
    let origin = game.cities[&game.player_city_ids(0)[0]].pos;
    let target = open_tile_at(&game, origin, BARBARIAN_SETTLER_PURSUIT_RADIUS);
    game.spawn_test_unit("warrior", 0, origin);
    let warrior = game.spawn_test_unit("warrior", 0, origin);
    game.spawn_test_unit("settler", 1, target);
    assert!(!ai.pursue_capturable_civilian(&mut game, 0, warrior, false));
}

/// The only soldier in one of our cities chases only what it can take
/// this turn: a settler beyond its movement stays where it is, and the
/// city keeps its garrison.
#[test]
fn a_lone_city_garrison_chases_only_what_it_can_take_this_turn() {
    let (mut game, _) = capture_test_board(91_773);
    let origin = game.cities[&game.player_city_ids(0)[0]].pos;
    let barb = game.barb_pid.unwrap();
    let far = open_tile_at(&game, origin, 3);
    let warrior = game.spawn_test_unit("warrior", 0, origin);
    game.spawn_test_unit("settler", barb, far);
    let mut ai = BasicAi::new();
    ai.barbarian_settler_capture = true;

    assert!(!ai.pursue_capturable_civilian(&mut game, 0, warrior, true));
    assert_eq!(game.units[&warrior].pos, origin);

    // Within reach, the same garrison takes it.
    let (mut game, _) = capture_test_board(91_773);
    let origin = game.cities[&game.player_city_ids(0)[0]].pos;
    let near = open_tile_at(&game, origin, 2);
    let warrior = game.spawn_test_unit("warrior", 0, origin);
    let captured = game.spawn_test_unit("settler", barb, near);
    assert!(ai.pursue_capturable_civilian(&mut game, 0, warrior, true));
    for _ in 0..8 {
        if game.units[&warrior].moves_left <= 0.0 || !ai.military_step(&mut game, 0, warrior) {
            break;
        }
    }
    assert_eq!(game.units[&captured].owner, 0);
}

#[test]
fn city_state_armies_decline_settlers_they_cannot_use() {
    let mut game = Game::new_full(2, 20, 14, 91_768, 120, 0, false);
    for pid in 0..2 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.found_city_for(pid, game.units[&settler].pos, None);
    }
    for unit in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(unit);
    }
    let origin = game.cities[&game.player_city_ids(0)[0]].pos;
    let target =
        game.nbrs(origin)
            .into_iter()
            .find(|position| {
                game.city_at(*position).is_none()
                    && game.map.get(*position).is_some_and(|tile| {
                        game.rules.is_passable(tile) && !game.rules.is_water(tile)
                    })
            })
            .unwrap();
    game.players[0].is_minor = true;
    game.at_war.insert((0, 1));
    let warrior = game.spawn_test_unit("warrior", 0, origin);
    let settler = game.spawn_test_unit("settler", 1, target);
    let mut ai = BasicAi::new();
    ai.minor = true;

    let _ = ai.military_step(&mut game, 0, warrior);
    assert_eq!(game.units[&settler].owner, 1);
    assert_ne!(game.units[&warrior].pos, target);
}

#[test]
fn unlevied_city_state_forces_return_to_the_local_defense_area() {
    let mut game = Game::new_full(2, 28, 18, 91_769, 120, 0, false);
    for pid in 0..2 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.found_city_for(pid, game.units[&settler].pos, None);
    }
    for unit in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(unit);
    }
    game.players[0].is_minor = true;
    game.at_war.insert((0, 1));
    let home = game.cities[&game.player_city_ids(0)[0]].pos;
    let candidates = game
        .map
        .tiles
        .values()
        .filter(|tile| {
            game.wdist(home, tile.pos) == MINOR_DEFENSE_RADIUS + 1
                && game.rules.is_passable(tile)
                && !game.rules.is_water(tile)
                && game.city_at(tile.pos).is_none()
                && tile.owner_city.is_none()
        })
        .map(|tile| tile.pos)
        .collect::<Vec<_>>();
    let warrior = candidates
        .into_iter()
        .find_map(|position| {
            let unit = game.spawn_test_unit("warrior", 0, position);
            if game
                .route_step(unit, home, 0)
                .is_some_and(|next| game.can_move(unit, next))
            {
                Some(unit)
            } else {
                game.remove_unit(unit);
                None
            }
        })
        .expect("test map has a remote land route home");
    let start = game.units[&warrior].pos;
    let mut ai = BasicAi::new();
    ai.minor = true;

    game.turn += 1;
    let unit = game.units.get_mut(&warrior).unwrap();
    unit.moves_left = 10.0;
    unit.moved = false;
    assert!(ai.military_step(&mut game, 0, warrior));
    assert!(
        game.wdist(home, game.units[&warrior].pos) <= MINOR_DEFENSE_RADIUS,
        "remote defender did not return from {start:?}; stopped at {:?}; last action {:?}",
        game.units[&warrior].pos,
        game.log.last()
    );

    let (boundary, outside) = game
        .map
        .tiles
        .values()
        .filter(|tile| {
            game.wdist(home, tile.pos) == MINOR_DEFENSE_RADIUS
                && game.rules.is_passable(tile)
                && !game.rules.is_water(tile)
                && game.city_at(tile.pos).is_none()
        })
        .find_map(|tile| {
            game.nbrs(tile.pos).into_iter().find_map(|outside| {
                game.map
                    .get(outside)
                    .is_some_and(|candidate| {
                        game.wdist(home, outside) > MINOR_DEFENSE_RADIUS
                            && game.rules.is_passable(candidate)
                            && !game.rules.is_water(candidate)
                            && game.city_at(outside).is_none()
                    })
                    .then_some((tile.pos, outside))
            })
        })
        .expect("test map has a passable defense boundary");
    {
        let unit = game.units.get_mut(&warrior).unwrap();
        unit.pos = boundary;
        unit.moves_left = 10.0;
        unit.moved = false;
    }
    assert!(game.can_move(warrior, outside));
    assert!(!ai.path_move(&mut game, 0, warrior, outside));
    assert_eq!(game.units[&warrior].pos, boundary);
}

#[test]
fn gold_spending_fills_worker_gap_but_keeps_reserve() {
    let mut g = Game::new_full(1, 20, 14, 32, 30, 0, false);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|id| g.units[id].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let cid = g.player_city_ids(0)[0];
    let ai = BasicAi::new();

    // This is a treasury test, so give it explicit Builder work instead of
    // depending on which features a seeded map happens to draw nearby.
    let work = g.cities[&cid]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != g.cities[&cid].pos)
        .unwrap();
    {
        let tile = g.map.tiles.get_mut(&work).unwrap();
        tile.terrain = crate::name!("plains");
        tile.hills = false;
        tile.feature = None;
        tile.resource = None;
        tile.improvement = None;
    }

    // One city keeps 125 gold and spends 200 on its missing builder.
    g.players[0].gold = 325.0;
    assert!(ai.spend_gold(&mut g, 0, &[cid], 0, 0, 0, 1, 1, 0));
    assert_eq!(g.players[0].gold, 125.0);
    assert!(g
        .units
        .values()
        .any(|u| u.owner == 0 && u.kind == "builder"));

    let builders = g
        .units
        .values()
        .filter(|u| u.owner == 0 && u.kind == "builder")
        .count();
    g.players[0].gold = 324.0;
    assert!(!ai.spend_gold(&mut g, 0, &[cid], 0, 0, 0, 1, 1, 0));
    assert_eq!(g.players[0].gold, 324.0);
    assert_eq!(
        g.units
            .values()
            .filter(|u| u.owner == 0 && u.kind == "builder")
            .count(),
        builders
    );
}

#[test]
fn gold_spending_converts_surplus_into_city_infrastructure() {
    let mut g = Game::new_full(1, 20, 14, 320, 30, 0, false);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|id| g.units[id].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let cid = g.player_city_ids(0)[0];
    g.cities
        .get_mut(&cid)
        .unwrap()
        .buildings
        .retain(|building| building != "monument");
    let ai = BasicAi::new();

    // With its unit needs already covered, the AI buys a Monument but
    // keeps the full one-city peacetime reserve.
    g.players[0].gold = 365.0;
    assert!(g.legal_actions(0).iter().any(|action| matches!(
        action,
        Action::BuyBuilding { building, currency, .. }
            if building == "monument" && currency == "gold"
    )));
    assert!(ai.spend_gold(&mut g, 0, &[cid], 1, 1, 1, 2, 1, 1));
    assert_eq!(g.players[0].gold, 125.0);
    assert!(g.cities[&cid].buildings.iter().any(|b| b == "monument"));

    // The same purchase is exposed through the public action protocol.
    assert!(!g.legal_actions(0).iter().any(|action| matches!(
        action,
        Action::BuyBuilding { building, currency, .. }
            if building == "monument" && currency == "gold"
    )));
}

/// `threatened-city-reserve`: the floor the deployed controller sets is
/// money the baseline buyer cannot see. Same board as the surplus test
/// above; with the floor raised past the bank, the Monument stays on the
/// shelf and the treasury is untouched.
#[test]
fn gold_spending_keeps_the_reserve_floor_it_was_handed() {
    let mut g = Game::new_full(1, 20, 14, 320, 30, 0, false);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|id| g.units[id].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let cid = g.player_city_ids(0)[0];
    g.cities
        .get_mut(&cid)
        .unwrap()
        .buildings
        .retain(|building| building != "monument");
    let mut ai = BasicAi::new();
    assert_eq!(ai.reserve_floor, 0.0, "the floor ships at zero");
    g.players[0].gold = 365.0;
    ai.reserve_floor = 360.0;
    assert!(
        !ai.spend_gold(&mut g, 0, &[cid], 1, 1, 1, 2, 1, 1),
        "a 240 Monument cannot come out of a bank of 365 with 360 held back"
    );
    assert_eq!(g.players[0].gold, 365.0);
    assert!(!g.cities[&cid].buildings.iter().any(|b| b == "monument"));
    ai.reserve_floor = 0.0;
    assert!(ai.spend_gold(&mut g, 0, &[cid], 1, 1, 1, 2, 1, 1));
    assert_eq!(g.players[0].gold, 125.0);
}

/// ⚠⚠ A HOST PURCHASE REFUSAL MUST REACH THE BUYERS THAT NEVER ENUMERATE.
///
/// `purchase_action_is_blocked` was applied only where legal actions are
/// enumerated, but `buy_gold_infrastructure`, `buy_gold_unit` and
/// `buy_gold_military` each build an `Action::Buy*` themselves and call
/// `apply` directly. They gated on `can_produce`, which reads
/// `blocked_production` — a DIFFERENT set meaning "cannot build here", not
/// "the host will not sell this here".
///
/// Live run `civvis-20260804T091315Z` re-bought the same Granary in the same
/// city on turns 114, 117, 121, 122, 123 and 128; 8 of that game's 9
/// purchases were refused. Replaying those turns after this change, the
/// Granary is gone and the gold goes to a Shrine instead.
#[test]
fn a_refused_purchase_is_not_re_offered_to_the_direct_buyers() {
    let mut g = Game::new_full(1, 20, 14, 320, 30, 0, false);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|id| g.units[id].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let cid = g.player_city_ids(0)[0];
    g.cities
        .get_mut(&cid)
        .unwrap()
        .buildings
        .retain(|building| building != "monument");
    let ai = BasicAi::new();
    g.players[0].gold = 365.0;

    // Baseline: the Monument is both priced and reachable by the direct buyer.
    assert!(
        g.building_gold_purchase_cost(0, cid, "monument").is_some(),
        "precondition: the Monument is purchasable before the host refuses it"
    );

    // Now the host refuses to sell a Monument in this city.
    g.replace_blocked_purchases(std::collections::BTreeMap::from([(
        cid,
        std::collections::BTreeSet::from(["building:monument".to_string()]),
    )]));

    assert_eq!(
        g.building_gold_purchase_cost(0, cid, "monument"),
        None,
        "a refused purchase must stop being priced — this is the choke point \
             every buyer reaches the offer through"
    );
    assert!(
        !g.legal_actions(0).iter().any(|action| matches!(
            action,
            Action::BuyBuilding { building, .. } if building == "monument"
        )),
        "and it must leave the enumeration too"
    );

    // The direct buyer must not spend gold on it either — this is the path
    // that bypassed the enumeration and re-proposed it every turn.
    let before = g.players[0].gold;
    ai.spend_gold(&mut g, 0, &[cid], 1, 1, 1, 2, 1, 1);
    assert!(
        !g.cities[&cid].buildings.iter().any(|b| b == "monument"),
        "the direct gold buyer must respect a host purchase refusal"
    );
    assert!(
        g.players[0].gold <= before,
        "and must not have paid for the refused building"
    );
}

#[test]
fn gold_spending_annexes_a_luxury_without_breaking_its_reserve() {
    let mut game = Game::new_full(1, 20, 14, 321, 30, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = game.player_city_ids(0)[0];
    let center = game.cities[&city].pos;
    for position in game.wdisk(center, 3) {
        if game.map.tiles[&position].owner_city.is_none() {
            let tile = game.map.tiles.get_mut(&position).unwrap();
            tile.terrain = crate::name!("plains");
            tile.hills = false;
            tile.feature = None;
            tile.resource = None;
        }
    }
    let target = game
        .wdisk(center, 2)
        .into_iter()
        .find(|position| {
            game.wdist(*position, center) == 2
                && game.map.tiles[position].owner_city.is_none()
                && game
                    .nbrs(*position)
                    .into_iter()
                    .any(|neighbor| game.map.tiles[&neighbor].owner_city == Some(city))
        })
        .unwrap();
    game.map.tiles.get_mut(&target).unwrap().resource = Some(crate::name!("diamonds"));
    game.players[0]
        .explored
        .extend(game.map.tiles.keys().copied());
    game.players[0].gold = 175.0;

    assert!(BasicAi::new().buy_gold_plot(&mut game, 0, 125.0));
    assert_eq!(game.map.tiles[&target].owner_city, Some(city));
    assert_eq!(game.players[0].gold, 125.0);
}

#[test]
fn crowded_world_does_not_produce_or_buy_a_stranded_settler() {
    let mut game = Game::new_full(1, 20, 14, 320_001, 80, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = game.player_city_ids(0)[0];
    let center = game.cities[&city].pos;
    game.cities.get_mut(&city).unwrap().pop = 4;
    for (position, tile) in &mut game.map.tiles {
        if position != center {
            tile.terrain = crate::name!("ocean");
            tile.feature = None;
        }
    }
    let settler_item = Item::Unit {
        unit: crate::name!("settler"),
    };
    assert!(game.can_produce(0, city, &settler_item));
    let mut ai = BasicAi::new();

    let production = ai.pick_item(&game, 0, city, 1, 0, 1, 1, 0, 2, 1, 1);
    assert_ne!(
        production,
        Some(settler_item.clone()),
        "the city must not turn Population and Production into a settler with nowhere to settle"
    );

    game.players[0].gold = 10_000.0;
    let _ = ai.spend_gold(&mut game, 0, &[city], 0, 1, 1, 2, 1, 1);
    assert!(game
        .player_unit_ids(0)
        .into_iter()
        .all(|unit| game.units[&unit].kind != "settler"));

    game.apply(
        0,
        &Action::Produce {
            city,
            item: settler_item.clone(),
        },
    )
    .unwrap();
    game.cities.get_mut(&city).unwrap().production = 42.0;
    ai.cities(&mut game, 0);
    assert!(!matches!(
        game.cities[&city].queue.first(),
        Some(Item::Unit { unit }) if unit == "settler"
    ));
    assert_eq!(
        game.cities[&city].production_progress.get("unit:settler"),
        Some(&42.0),
        "the invested Production should remain banked when the queue is redirected"
    );
}

#[test]
fn existing_settler_redirects_a_queued_duplicate() {
    let mut game = Game::new_full(1, 20, 14, 320_002, 80, 0, false);
    let founding_settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(
        0,
        &Action::FoundCity {
            unit: founding_settler,
        },
    )
    .unwrap();
    let city = game.player_city_ids(0)[0];
    game.cities.get_mut(&city).unwrap().pop = 4;
    let settler_item = Item::Unit {
        unit: crate::name!("settler"),
    };
    game.apply(
        0,
        &Action::Produce {
            city,
            item: settler_item,
        },
    )
    .unwrap();
    game.cities.get_mut(&city).unwrap().production = 42.0;
    let captured = game.spawn_test_unit("settler", 0, game.cities[&city].pos);
    let mut ai = BasicAi::new();
    assert!(ai.has_practical_settle_site(&game, 0));

    ai.cities(&mut game, 0);

    assert!(game.units.contains_key(&captured));
    assert!(!matches!(
        game.cities[&city].queue.first(),
        Some(Item::Unit { unit }) if unit == "settler"
    ));
    assert_eq!(
        game.cities[&city].production_progress.get("unit:settler"),
        Some(&42.0)
    );
}

#[test]
fn deficit_empire_builds_its_way_back_to_positive_gpt() {
    let mut g = Game::new_full(1, 20, 14, 323, 60, 0, false);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|id| g.units[id].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    g.players[0].techs.insert(crate::name!("currency"));
    g.players[0].gold = 0.0;
    g.players[0].gold_per_turn = -9.2;
    let cid = g.player_city_ids(0)[0];
    // The assertion is about what a broke empire chooses to build, not
    // about whether the map left it anywhere to build: level the ring.
    let center = g.cities[&cid].pos;
    let ring: Vec<Pos> = g.cities[&cid]
        .owned_tiles
        .iter()
        .copied()
        .filter(|position| *position != center)
        .collect();
    for position in ring {
        let tile = g.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
    }
    let ai = BasicAi::new();

    let district = ai
        .pick_item(&g, 0, cid, 1, 0, 0, 0, 0, 4, 2, 2)
        .expect("a deficit city should establish gold infrastructure");
    let Item::District {
        district,
        pos: commercial_hub,
    } = district
    else {
        panic!("expected a Commercial Hub, got {district:?}");
    };
    assert_eq!(district, "commercial_hub");
    g.map.tiles.get_mut(&commercial_hub).unwrap().district = Some(district);
    g.cities
        .get_mut(&cid)
        .unwrap()
        .districts
        .insert(district, commercial_hub);

    assert_eq!(
        ai.pick_item(&g, 0, cid, 1, 0, 0, 0, 0, 4, 2, 2),
        Some(Item::Building {
            building: crate::name!("market")
        })
    );
    g.cities
        .get_mut(&cid)
        .unwrap()
        .buildings
        .push(crate::name!("market"));
    let second_center = g
        .map
        .tiles
        .iter()
        .find(|(position, tile)| {
            let distance = g.wdist(**position, center);
            (4..=15).contains(&distance)
                && g.rules.is_passable(tile)
                && !g.rules.is_water(tile)
                && g.unit_ids_at(**position).is_empty()
        })
        .map(|(position, _)| *position)
        .expect("the recovery Trader needs a reachable destination");
    let second_settler = g.spawn_test_unit("settler", 0, second_center);
    g.apply(
        0,
        &Action::FoundCity {
            unit: second_settler,
        },
    )
    .unwrap();
    g.players[0].civics.insert(crate::name!("foreign_trade"));
    assert_eq!(
        ai.pick_item(&g, 0, cid, 1, 0, 0, 0, 0, 4, 2, 2),
        Some(Item::Unit {
            unit: crate::name!("trader")
        })
    );
}

#[test]
fn trade_capacity_without_an_open_destination_does_not_create_a_trader() {
    let mut g = Game::new_full(1, 20, 14, 323_001, 60, 0, false);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|id| g.units[id].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    g.players[0].civics.insert(crate::name!("foreign_trade"));
    let city = g.player_city_ids(0)[0];
    let trader = Item::Unit {
        unit: crate::name!("trader"),
    };
    assert!(g.trade_capacity(0) > 0);
    assert!(g.can_produce(0, city, &trader));
    assert_eq!(BasicAi::open_trade_destinations(&g, 0), 0);
    assert!(!BasicAi::should_add_trader(&g, 0, 0));

    let ai = BasicAi::new();
    let choice = ai.pick_item(&g, 0, city, 1, 1, 1, 0, 0, 3, 2, 1);
    assert_ne!(choice, Some(trader));

    g.players[0].gold = g.rules.units["trader"].cost * 4.0 + 125.0;
    let _ = ai.spend_gold(&mut g, 0, &[city], 1, 1, 0, 3, 2, 1);
    assert!(g
        .player_unit_ids(0)
        .into_iter()
        .all(|unit| g.units[&unit].kind != "trader"));
}

#[test]
fn deficit_empire_without_a_gold_build_keeps_producing_without_new_upkeep() {
    let mut g = Game::new_full(1, 20, 14, 324, 60, 0, false);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|id| g.units[id].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    g.players[0].techs.insert(crate::name!("pottery"));
    g.players[0].gold = 0.0;
    g.players[0].gold_per_turn = -4.0;
    let cid = g.player_city_ids(0)[0];

    let item = BasicAi::new()
        .pick_item(&g, 0, cid, 1, 0, 0, 0, 0, 6, 3, 3)
        .expect("a deficit city must not leave its Production idle");
    match &item {
        Item::Building { building } => {
            assert!(g.rules.buildings[building].maintenance <= f64::EPSILON)
        }
        Item::Project { project } => assert!(g.rules.projects[project].repeatable),
        other => panic!("recovery fallback added upkeep: {other:?}"),
    }
    g.apply(0, &Action::Produce { city: cid, item }).unwrap();
    assert!(!g.cities[&cid].queue.is_empty());
}

#[test]
fn one_queued_spaceport_reserves_the_empire_launch_site() {
    let mut game = Game::new_full(
        1,
        30,
        20,
        crate::rng::fixture_seed("SPACEPORT", 324_006),
        100,
        0,
        false,
    );
    let first_settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(
        0,
        &Action::FoundCity {
            unit: first_settler,
        },
    )
    .unwrap();
    let first_center = game.cities[&game.player_city_ids(0)[0]].pos;
    let second_center = game
        .map
        .tiles
        .iter()
        .filter(|(position, tile)| {
            game.wdist(**position, first_center) >= 4
                && game.rules.is_passable(tile)
                && !game.rules.is_water(tile)
                && game.unit_ids_at(**position).is_empty()
        })
        .map(|(position, _)| *position)
        .next()
        .expect("map has a second legal city site");
    let second_settler = game.spawn_test_unit("settler", 0, second_center);
    game.apply(
        0,
        &Action::FoundCity {
            unit: second_settler,
        },
    )
    .unwrap();
    game.players[0].techs.insert(crate::name!("rocketry"));
    let cities = game.player_city_ids(0);
    for city in &cities {
        game.cities.get_mut(city).unwrap().pop = 10;
        assert!(
            !game
                .district_sites(*city, crate::name!("spaceport"))
                .is_empty(),
            "both cities must be able to repeat the archived overbuild"
        );
    }

    let ai = BasicAi::new();
    let launch_city = cities[0];
    let first = ai
        .pick_item(&game, 0, launch_city, 2, 2, 2, 2, 1, 10, 5, 5)
        .expect("the empire needs its first launch site");
    assert!(matches!(
        &first,
        Item::District { district, .. } if district == "spaceport"
    ));
    let mut domination = BasicAi::new();
    domination.exclude_space_race = true;
    let conquest_choice = domination.pick_item(&game, 0, launch_city, 2, 2, 2, 2, 1, 10, 5, 5);
    assert!(
        !matches!(conquest_choice, Some(Item::District { ref district, .. }) if district == "spaceport"),
        "the delegated domination governor must leave the launch site alone: {conquest_choice:?}"
    );
    game.apply(
        0,
        &Action::Produce {
            city: launch_city,
            item: first,
        },
    )
    .unwrap();

    let other = cities[1];
    let next = ai.pick_item(&game, 0, other, 2, 2, 2, 2, 1, 10, 5, 5);
    assert!(
        !matches!(next, Some(Item::District { ref district, .. }) if district == "spaceport"),
        "a queued Spaceport must stop every other city reserving another one: {next:?}"
    );

    let launch_site = match game.cities[&launch_city].queue.first().unwrap() {
        Item::District { pos, .. } => *pos,
        other => panic!("expected the queued Spaceport, got {other:?}"),
    };
    game.cities.get_mut(&launch_city).unwrap().queue.clear();
    game.cities
        .get_mut(&launch_city)
        .unwrap()
        .districts
        .insert(crate::name!("spaceport"), launch_site);
    game.map.tiles.get_mut(&launch_site).unwrap().district = Some(crate::name!("spaceport"));
    let launch = Item::Project {
        project: crate::name!("launch_earth_satellite"),
    };
    assert!(game.can_produce(0, launch_city, &launch));
    assert_eq!(
        ai.pick_item(&game, 0, launch_city, 2, 2, 2, 2, 1, 10, 5, 5),
        Some(launch),
        "the ordinary governor would start the launch chain"
    );
    let conquest_choice = domination.pick_item(&game, 0, launch_city, 2, 2, 2, 2, 1, 10, 5, 5);
    assert!(
        !matches!(conquest_choice, Some(Item::Project { ref project }) if project == "launch_earth_satellite"),
        "the domination governor must spend this queue outside the space race: {conquest_choice:?}"
    );
}

#[test]
fn buying_a_queued_building_finishes_it_without_a_duplicate() {
    let mut g = Game::new_full(1, 20, 14, 322, 30, 0, false);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|id| g.units[id].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let cid = g.player_city_ids(0)[0];
    g.cities
        .get_mut(&cid)
        .unwrap()
        .buildings
        .retain(|building| building != "monument");
    g.apply(
        0,
        &Action::Produce {
            city: cid,
            item: Item::Building {
                building: crate::name!("monument"),
            },
        },
    )
    .unwrap();
    g.players[0].gold = 1_000.0;

    let purchase = Action::BuyBuilding {
        city: cid,
        building: crate::name!("monument"),
        currency: "gold".to_string(),
    };
    let cost = g
        .building_gold_purchase_cost(0, cid, "monument")
        .expect("a queued ordinary building remains purchasable");
    assert!(g.legal_actions(0).iter().any(|action| matches!(
        action,
        Action::BuyBuilding { city, building, .. }
            if *city == cid && building == "monument"
    )));
    g.apply(0, &purchase).unwrap();
    assert_eq!(g.players[0].gold, 1_000.0 - cost);
    assert!(g.cities[&cid].queue.is_empty());
    assert_eq!(
        g.cities[&cid]
            .buildings
            .iter()
            .filter(|building| building.as_str() == "monument")
            .count(),
        1
    );
    assert!(g.apply(0, &purchase).is_err());
}

#[test]
fn city_states_invest_surplus_gold_without_abandoning_their_reserve() {
    let mut g = Game::new_full(1, 20, 14, 321, 30, 0, false);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|id| g.units[id].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let cid = g.player_city_ids(0)[0];
    g.players[0].is_minor = true;
    g.cities
        .get_mut(&cid)
        .unwrap()
        .buildings
        .retain(|building| building != "monument");
    let mut ai = BasicAi::new();
    ai.minor = true;

    // Expansion and trade purchases remain major-only, but a city-state
    // with its local worker/defense needs met should convert excess Gold
    // into its city instead of accumulating an inert four-figure balance.
    g.players[0].gold = 365.0;
    assert!(ai.spend_gold(&mut g, 0, &[cid], 1, 1, 0, 3, 2, 1));
    assert_eq!(g.players[0].gold, 125.0);
    assert!(g.cities[&cid].buildings.iter().any(|b| b == "monument"));
}

#[test]
fn headless_ai_spends_promotions_and_forms_unlocked_corps() {
    let mut g = Game::new_full(1, 20, 14, 33, 30, 0, false);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|id| g.units[id].kind == "settler")
        .unwrap();
    let veteran = g
        .player_unit_ids(0)
        .into_iter()
        .find(|id| g.units[id].kind == "warrior")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    g.units.get_mut(&veteran).unwrap().xp = 15;
    let ai = BasicAi::new();
    ai.prepare_unit_formations(&mut g, 0);
    assert_eq!(g.units[&veteran].level, 2);
    assert_eq!(g.units[&veteran].promotions.len(), 1);

    let pos = g
        .map
        .tiles
        .iter()
        .find(|(pos, tile)| {
            g.rules.is_passable(tile) && !g.rules.is_water(tile) && g.unit_ids_at(**pos).is_empty()
        })
        .map(|(pos, _)| *pos)
        .unwrap();
    for _ in 0..6 {
        g.spawn_test_unit("warrior", 0, pos);
    }
    g.players[0].civics.insert(crate::name!("nationalism"));
    ai.prepare_unit_formations(&mut g, 0);
    let warriors: Vec<_> = g
        .units
        .values()
        .filter(|unit| unit.owner == 0 && unit.kind == "warrior")
        .collect();
    assert_eq!(warriors.len(), 5, "the AI keeps a five-unit reserve");
    assert!(warriors.iter().any(|unit| unit.formation == 1));
    assert!(
        warriors.iter().any(|unit| unit.promotions.len() == 1),
        "the veteran remains in the force"
    );
}

#[test]
fn production_adds_one_support_unit_for_walled_wars() {
    let (mut g, home, enemy) = walled_war_game(33);
    g.cities.get_mut(&enemy).unwrap().wall_hp = 100;
    let ai = BasicAi::new();
    g.players[0].techs.insert(crate::name!("masonry"));

    let ram = ai.pick_item(&g, 0, home, 1, 0, 1, 0, 0, 2, 2, 0).unwrap();
    assert_eq!(
        ram,
        Item::Unit {
            unit: crate::name!("battering_ram")
        }
    );

    let tower_tech = g.rules.units["siege_tower"].tech.unwrap();
    g.players[0].techs.insert(tower_tech);
    let tower = ai.pick_item(&g, 0, home, 1, 0, 1, 0, 0, 2, 2, 0).unwrap();
    assert_eq!(
        tower,
        Item::Unit {
            unit: crate::name!("siege_tower")
        }
    );

    let next = ai.pick_item(&g, 0, home, 1, 0, 1, 0, 1, 2, 2, 0).unwrap();
    assert!(!matches!(next, Item::Unit { unit }
            if unit == "battering_ram" || unit == "siege_tower"));
}

#[test]
fn headless_ai_resolves_mandatory_capture_choices() {
    let mut g = Game::new_full(2, 20, 14, 34, 30, 0, false);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|id| g.units[id].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = g.player_city_ids(0)[0];
    {
        let captured = g.cities.get_mut(&city).unwrap();
        // This is an unresolved capture of player 1's original city;
        // resolving a genuine recapture of player 0's own original city
        // correctly produces no grievance.
        captured.original_owner = 1;
        captured.captured_from = Some(1);
    }
    assert!(matches!(
        g.legal_actions(0).as_slice(),
        [Action::KeepCity { city: pending }] if *pending == city
    ));

    let mut ai = BasicAi::new();
    ai.resolve_city_dispositions(&mut g, 0, false, false);

    assert_eq!(g.cities[&city].captured_from, None);
    assert_eq!(g.players[1].grievances.get(&0), Some(&50.0));
}

#[test]
fn builder_never_paces_between_tiles_it_cannot_work() {
    // A project target the Builder cannot stand on (the game places
    // districts on land, but a mod or a captured layout can leave one
    // unreachable) must not leave it walking back and forth forever.
    let mut g = Game::new_full(1, 20, 14, 35, 40, 0, false);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|id| g.units[id].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = g.player_city_ids(0)[0];
    let spaceport = g.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != g.cities[&city].pos)
        .unwrap();
    {
        let tile = g.map.tiles.get_mut(&spaceport).unwrap();
        tile.terrain = crate::name!("mountain");
        tile.district = Some(crate::name!("spaceport"));
    }
    g.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(crate::name!("spaceport"), spaceport);
    g.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("royal_society"));
    g.cities.get_mut(&city).unwrap().queue = vec![Item::Project {
        project: crate::name!("launch_earth_satellite"),
    }];
    let builder = g.spawn_test_unit("builder", 0, g.cities[&city].pos);
    g.units.get_mut(&builder).unwrap().charges = 3;

    let ai = BasicAi::new();
    let mut visited = Vec::new();
    for _ in 0..12 {
        if !g.units.contains_key(&builder) {
            break;
        }
        visited.push(g.units[&builder].pos);
        if !ai.builder_step(&mut g, 0, builder) {
            break;
        }
        let movement = g.rules.units["builder"].moves;
        // Spending the last charge consumes the Builder mid-loop.
        let Some(unit) = g.units.get_mut(&builder) else {
            break;
        };
        unit.moves_left = movement;
        unit.moved = false;
        unit.acted = false;
    }
    let charges_spent = g
        .units
        .get(&builder)
        .map(|unit| 3 - unit.charges)
        .unwrap_or(3);
    assert!(
        charges_spent > 0
            || visited
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                == visited.len(),
        "Builder paced without working: {visited:?}"
    );
}

#[test]
fn builder_routes_to_a_royal_society_project_and_contributes() {
    let mut g = Game::new_full(1, 20, 14, 35, 40, 0, false);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|id| g.units[id].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = g.player_city_ids(0)[0];
    // Districts have to sit on land a Builder can walk onto; picking the
    // first owned tile can land the Spaceport on water or a mountain, and
    // the routing under test then has no legal way to reach it.
    let buildable = |g: &Game, position: &Pos| {
        let tile = &g.map.tiles[position];
        !g.rules.is_water(tile) && g.rules.is_passable(tile)
    };
    let spaceport = g.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != g.cities[&city].pos && buildable(&g, position))
        .unwrap();
    g.map.tiles.get_mut(&spaceport).unwrap().district = Some(crate::name!("spaceport"));
    g.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(crate::name!("spaceport"), spaceport);
    let government_plaza = g.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| {
            *position != g.cities[&city].pos
                && *position != spaceport
                && g.map.tiles[position].district.is_none()
                && buildable(&g, position)
        })
        .unwrap();
    g.map.tiles.get_mut(&government_plaza).unwrap().district =
        Some(crate::name!("government_plaza"));
    g.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(crate::name!("government_plaza"), government_plaza);
    g.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("royal_society"));
    g.cities.get_mut(&city).unwrap().queue = vec![Item::Project {
        project: crate::name!("launch_earth_satellite"),
    }];
    let builder = g.spawn_test_unit("builder", 0, g.cities[&city].pos);
    g.units.get_mut(&builder).unwrap().charges = 3;

    let ai = BasicAi::new();
    // Reaching the Spaceport can take more than one turn's movement, and
    // entering a wooded or hilled district tile can spend a whole
    // allowance on its own, so drive the Builder the way the AI game loop
    // does — a step per turn — rather than assuming it arrives at once.
    let mut turns = 0;
    while g.units[&builder].pos != spaceport {
        assert!(ai.builder_step(&mut g, 0, builder));
        turns += 1;
        assert!(turns < 8, "Builder never reached the Spaceport");
        let movement = g.rules.units["builder"].moves;
        let unit = g.units.get_mut(&builder).unwrap();
        unit.moves_left = movement;
        unit.moved = false;
        unit.acted = false;
    }
    assert!(ai.builder_step(&mut g, 0, builder));
    assert!(!g.units.contains_key(&builder));
    assert_eq!(g.cities[&city].production, 54.0);
}

#[test]
fn headless_naturalist_routes_to_and_establishes_a_complete_park() {
    let mut g = Game::new_full(1, 20, 14, 36, 40, 0, false);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|id| g.units[id].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = g.player_city_ids(0)[0];
    let center = g.cities[&city].pos;
    let positions = g
        .map
        .tiles
        .keys()
        .copied()
        .filter(|top| g.wdist(center, *top) > 4)
        .find_map(|top| {
            let positions = [
                top,
                crate::hex::canon((top.0 - 1, top.1 + 1), g.map.width),
                crate::hex::canon((top.0, top.1 + 1), g.map.width),
                crate::hex::canon((top.0 - 1, top.1 + 2), g.map.width),
            ];
            positions
                .iter()
                .all(|position| g.map.tiles.contains_key(position))
                .then_some(positions)
        })
        .unwrap();

    let old_owned = g.cities[&city].owned_tiles.clone();
    for position in old_owned {
        g.map.tiles.get_mut(&position).unwrap().owner_city = None;
    }
    g.cities.get_mut(&city).unwrap().owned_tiles = positions.to_vec();
    for position in positions
        .iter()
        .flat_map(|position| g.nbrs(*position))
        .chain(positions)
        .collect::<std::collections::BTreeSet<_>>()
    {
        let tile = g.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.pillaged = false;
        tile.district = None;
        tile.wonder = None;
        tile.flooded = false;
        tile.submerged = false;
    }
    for position in positions {
        let tile = g.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("mountain");
        tile.owner_city = Some(city);
    }
    g.map.tiles.get_mut(&positions[0]).unwrap().terrain = crate::name!("grassland");
    g.players[0].civics.insert(crate::name!("conservation"));
    assert_eq!(g.national_park_sites(0), vec![positions]);

    let start = g
        .nbrs(positions[0])
        .into_iter()
        .find(|position| !positions.contains(position))
        .unwrap();
    let naturalist = g.spawn_test_unit("naturalist", 0, start);
    let ai = BasicAi::new();
    assert!(ai.naturalist_step(&mut g, 0, naturalist));
    assert_eq!(g.units[&naturalist].pos, positions[0]);
    assert!(ai.naturalist_step(&mut g, 0, naturalist));
    assert!(!g.units.contains_key(&naturalist));
    assert!(positions
        .iter()
        .all(|position| { g.map.tiles[position].improvement.as_deref() == Some("national_park") }));
}

#[test]
fn headless_military_engineer_routes_to_and_accelerates_an_aqueduct() {
    let mut game = Game::new_full(1, 20, 14, 36_001, 80, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = game.player_city_ids(0)[0];
    let center = game.cities[&city].pos;
    let site = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != center && game.unit_ids_at(*position).is_empty())
        .unwrap();
    let center_edge = game.map.direction_to(site, center).unwrap();
    {
        let tile = game.map.tiles.get_mut(&site).unwrap();
        tile.terrain = crate::name!("plains");
        tile.hills = false;
        tile.feature = None;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.river_edges[(center_edge + 1) % 6] = true;
    }
    game.players[0].techs.insert(crate::name!("engineering"));
    let aqueduct = Item::District {
        district: crate::name!("aqueduct"),
        pos: site,
    };
    game.cities.get_mut(&city).unwrap().queue = vec![aqueduct.clone()];
    let district_cost = game.item_cost_for_city(0, city, &aqueduct);
    let engineer = game.spawn_test_unit("military_engineer", 0, center);
    let mut ai = BasicAi::new();

    assert!(ai.military_engineer_step(&mut game, 0, engineer));
    assert_eq!(game.units[&engineer].pos, site);
    // The river-adjacent construction tile can consume the Engineer's
    // full movement; the contribution is made after movement refreshes.
    game.units.get_mut(&engineer).unwrap().moves_left = 2.0;
    assert!(game.can_contribute_district(0, engineer, city));
    assert!(ai.military_engineer_step(&mut game, 0, engineer));
    assert!(
        (game.cities[&city].production - district_cost * 0.2).abs() < 1e-9,
        "production was {}",
        game.cities[&city].production
    );
    assert_eq!(game.units[&engineer].charges, 1);
    assert_eq!(game.units[&engineer].moves_left, 0.0);
}

#[test]
fn idle_military_engineer_starts_a_stockpile_safe_railroad() {
    let mut game = Game::new_full(1, 20, 14, 36_002, 80, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = game.player_city_ids(0)[0];
    let center = game.cities[&city].pos;
    game.players[0].techs.insert(crate::name!("steam_power"));
    game.players[0]
        .strategic_resources
        .insert(crate::name!("iron"), 8.0);
    game.players[0]
        .strategic_resources
        .insert(crate::name!("coal"), 8.0);
    let engineer = game.spawn_test_unit("military_engineer", 0, center);
    let mut ai = BasicAi::new();

    assert!(ai.military_engineer_step(&mut game, 0, engineer));
    assert_eq!(game.map.tiles[&center].road, 5);
    assert_eq!(game.strategic_stockpile(0, crate::name!("iron")), 7.0);
    assert_eq!(game.strategic_stockpile(0, crate::name!("coal")), 7.0);

    // Once either reserve reaches the floor, the Engineer stops spending
    // and remains available for a Dam/Aqueduct/Canal contribution.
    game.map.tiles.get_mut(&center).unwrap().road = 1;
    game.players[0]
        .strategic_resources
        .insert(crate::name!("coal"), RAILROAD_RESOURCE_RESERVE);
    game.units.get_mut(&engineer).unwrap().moves_left = 2.0;
    let _ = ai.military_engineer_step(&mut game, 0, engineer);
    assert_ne!(game.map.tiles[&center].road, 5);
    assert_eq!(
        game.strategic_stockpile(0, crate::name!("coal")),
        RAILROAD_RESOURCE_RESERVE
    );
}

#[test]
fn headless_archaeologist_routes_to_and_extracts_an_artifact() {
    let mut g = Game::new_full(1, 20, 14, 37, 40, 0, false);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|id| g.units[id].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = g.player_city_ids(0)[0];
    g.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("archaeological_museum"));
    g.players[0].civics.insert(crate::name!("natural_history"));
    // The generator now lays down the shipped six dig sites per
    // civilization, so this fixture has to be the only Artifact on the map
    // or the routing assertion is really asserting which site is nearest.
    for position in g.map.tiles.keys().copied().collect::<Vec<_>>() {
        let is_artifact = g.map.tiles[&position]
            .resource
            .as_deref()
            .is_some_and(|resource| g.rules.resources[resource].class == "artifact");
        if is_artifact {
            g.map.tiles.get_mut(&position).unwrap().resource = None;
        }
    }
    // One `archaeologist_step` is one movement step, so the dig has to
    // border the city for the route to finish inside it. Taking the first
    // owned tile instead left that to the shape of the generated map.
    let site = g.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| {
            g.wdist(*position, g.cities[&city].pos) == 1 && g.unit_ids_at(*position).is_empty()
        })
        .unwrap();
    let tile = g.map.tiles.get_mut(&site).unwrap();
    tile.terrain = crate::name!("plains");
    tile.feature = None;
    tile.resource = Some(crate::name!("antiquity_site"));
    tile.improvement = None;
    tile.district = None;
    tile.wonder = None;
    let archaeologist = g.spawn_test_unit("archaeologist", 0, g.cities[&city].pos);

    let ai = BasicAi::new();
    assert!(ai.archaeologist_step(&mut g, 0, archaeologist));
    assert_eq!(g.units[&archaeologist].pos, site);
    assert!(ai.archaeologist_step(&mut g, 0, archaeologist));
    assert!(g.map.tiles[&site].resource.is_none());
    assert_eq!(g.players[0].counters["great_work:artifact"], 1);
    assert_eq!(g.units[&archaeologist].charges, 2);
}

#[test]
fn basic_ai_establishes_sources_then_runs_its_best_spy_operation() {
    let mut game = Game::new_full(2, 24, 16, 38, 80, 0, false);
    let cities: Vec<u32> = (0..2)
        .map(|pid| {
            let settler = game
                .player_unit_ids(pid)
                .into_iter()
                .find(|unit| game.units[unit].kind == "settler")
                .unwrap();
            game.found_city_for(pid, game.units[&settler].pos, None)
        })
        .collect();
    let target = cities[1];
    let commercial = game.cities[&target]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != game.cities[&target].pos)
        .unwrap();
    game.map.tiles.get_mut(&commercial).unwrap().district = Some(crate::name!("commercial_hub"));
    game.cities
        .get_mut(&target)
        .unwrap()
        .districts
        .insert(crate::name!("commercial_hub"), commercial);
    game.players[0].explored.insert(game.cities[&target].pos);
    let spy = game.next_id;
    game.next_id += 1;
    game.spies.insert(
        spy,
        crate::game::Spy {
            id: spy,
            owner: 0,
            level: 0,
            promotions: std::collections::BTreeSet::new(),
            city: Some(cities[0]),
            ready_turn: game.turn,
            mission: None,
            sources_city: None,
            sources_until: 0,
            captured_by: None,
        },
    );

    let ai = BasicAi::new();
    ai.spies(&mut game, 0);
    assert_eq!(game.spies[&spy].city, Some(target));
    game.turn = game.spies[&spy].ready_turn;
    ai.spies(&mut game, 0);
    assert_eq!(
        game.spies[&spy]
            .mission
            .as_ref()
            .map(|mission| mission.kind.as_str()),
        Some("gain_sources")
    );
    let ends = game.spies[&spy].mission.as_ref().unwrap().ends;
    game.turn = ends;
    game.process_spies(0);
    ai.spies(&mut game, 0);
    assert_eq!(
        game.spies[&spy]
            .mission
            .as_ref()
            .map(|mission| mission.kind.as_str()),
        Some("siphon_funds")
    );
}

/// Fires-check for `pol_influence`: does making influence visible actually
/// get the card slotted?
///
/// `empire_reading` scores a card by the counterfactual difference it makes
/// to the empire. Influence is neither a city yield nor unit strength, so
/// `charismatic_leader` — whose entire effect is `influence_per_turn: 2` —
/// read bit-identical either side and scored exactly 0.0, then lost every
/// tie because `POLICY_PRIORITY` does not name it. `envoy_income_census`
/// (#612) measured the consequence: unlocked on 63.7% of turns with an open
/// diplomatic slot on 49.1%, and slotted on 0.0%.
///
/// The criterion is the outcome — the card in a slot — not a score.
///
/// Run with `cargo test --release influence_visible_fires -- --ignored --nocapture`.
#[test]
#[ignore = "census, not an assertion; run explicitly with --nocapture"]
fn influence_visible_fires() {
    for weight in [0.0f64, 1.0, 4.0] {
        let mut slotted = 0u64;
        let mut turns = 0u64;
        let mut envoys_placed = 0.0f64;
        let maps = 6u64;
        for map in 0..maps {
            let mut game = crate::game::Game::new_full(6, 74, 46, 480_000 + map, 200, 12, false);
            let mut ais: Vec<AdvancedAi> = (0..game.players.len())
                .map(|_| {
                    // ⚠ `Weights::default()` is `PolicyDeck::Legacy`, which
                    // returns before the counterfactual scoring ever runs.
                    // The shipped agent loads the evolved champion, and
                    // that artifact deserializes to `Live` — forcing
                    // champions to Legacy was measured to lose (12 map
                    // directions for, 26 against, p=0.0336). Testing on the
                    // default would measure a path deployment never takes.
                    let mut w = crate::evolve::load_champion("evolved").unwrap_or_default();
                    w.policy_deck = PolicyDeck::Live;
                    w.pol_influence = weight;
                    AdvancedAi::with_weights(w)
                })
                .collect();
            game.set_fog_memory(false);
            while game.winner.is_none() && game.turn <= game.max_turns {
                let pid = game.current;
                ais[pid].take_turn(&mut game, pid);
                if game.winner.is_none() && game.current == pid {
                    let _ = game.apply(pid, &crate::game::Action::EndTurn);
                }
                if pid != 0 {
                    continue;
                }
                turns += 1;
                if game.players[0]
                    .policies
                    .iter()
                    .any(|p| p.as_str() == "charismatic_leader")
                {
                    slotted += 1;
                }
                envoys_placed += game.players[0].envoys.iter().map(|(_, n)| *n).sum::<i64>() as f64;
            }
        }
        println!(
                "  pol_influence={weight:<4}  charismatic_leader slotted {:>5.1}% of turns   envoys placed mean {:.2}",
                slotted as f64 / turns.max(1) as f64 * 100.0,
                envoys_placed / turns.max(1) as f64
            );
    }
    println!();
}

/// ⚠ The two Museums are identical except for the slot KIND, both cost 290, and
/// `sort()` fell through to `Name::cmp` — which compares text, so
/// `archaeological_museum` won on the letter 'c'. That alphabetical accident
/// decided which Museum every empire built, and the one it picked needs a
/// 400-production Archaeologist that no live run has ever built.
#[test]
fn a_cost_tie_prefers_the_slots_a_great_person_can_fill() {
    let g = Game::new_full(1, 20, 14, 37, 40, 0, false);
    let art = &g.rules.buildings["art_museum"];
    let dig = &g.rules.buildings["archaeological_museum"];

    // Fixture preconditions: the tie is real, and the alphabet did decide it.
    assert_eq!(art.cost, dig.cost, "same cost — this is why it is a tie");
    assert_eq!(art.yields.culture, dig.yields.culture);
    assert_eq!(
        art.great_work_slots.values().sum::<i32>(),
        dig.great_work_slots.values().sum::<i32>(),
        "same NUMBER of slots, so counting them cannot break the tie either"
    );
    assert!(
        crate::name!("archaeological_museum") < crate::name!("art_museum"),
        "fixture precondition: the alphabet really does put artifact first"
    );

    let slot_worth = |name: &str| -> f64 {
        g.rules.buildings[&Name::new(name)]
            .great_work_slots
            .iter()
            .map(|(kind, count)| {
                let count = (*count).max(0) as f64;
                if kind == "artifact" {
                    count * 0.5
                } else {
                    count
                }
            })
            .sum()
    };
    assert!(
        slot_worth("art_museum") > slot_worth("archaeological_museum"),
        "the tie must now break toward the Museum whose slots a Great Person fills"
    );
    // ⚠ Everything without an artifact slot keeps its exact weight, so this
    // changes tied pairs only and never reorders on cost.
    assert_eq!(slot_worth("amphitheater"), 2.0);
    assert_eq!(slot_worth("library"), 0.0);

    // ⚠ SCOPE. The rule fires only where BOTH candidates carry slots. A first
    // version compared across every cost tie and reordered six groups —
    // old_god_obelisk over monument, temple over market, cathedral over every
    // other worship building, broadcast_center over research_lab — and priced at
    // -14 Elo with culture LOWER (151.7 vs 153.8). These assertions pin the
    // narrowing: a slotless building must never be displaced by a slotted one.
    for (cost, slotless) in [
        (60, "monument"),
        (120, "market"),
        (150, "arena"),
        (290, "bank"),
        (440, "research_lab"),
    ] {
        let spec = &g.rules.buildings[&Name::new(slotless)];
        assert_eq!(
            spec.cost as i64, cost,
            "fixture: {slotless} still costs {cost}"
        );
        assert_eq!(
            slot_worth(slotless),
            0.0,
            "{slotless} has no slots, so the tiebreak must not touch its position"
        );
    }
}
