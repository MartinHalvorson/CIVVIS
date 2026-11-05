use super::*;
use crate::game::Game;

/// The shipped promotion rule is a CONSTANT ORDER, and this is the fact the
/// treatment exists to change. Scoring by the sum of absolute effect values
/// ranks the nine Apostle promotions identically for every empire, on every
/// board, in every situation — because 200 percent outranks 20 strength as
/// a bare number, and 100 gold outranks both.
///
/// Asserted off the SHIPPED ruleset rather than off a hand-written list.
/// This test has already earned that: the first draft of the treatment was
/// written against `data/promotions.json` read through a filter that
/// returned five of the nine, and this assertion is what said so.
#[test]
fn the_shipped_apostle_ranking_is_a_constant_and_it_is_a_units_mismatch() {
    let rules = crate::rules::Rules::embedded();
    let magnitude = |name: &str| {
        rules.promotions[name]
            .effects
            .values()
            .map(|effect| effect.abs())
            .sum::<f64>()
    };
    let apostle: Vec<&str> = rules
        .promotions
        .iter()
        .filter(|(_, spec)| spec.class == "religious_apostle")
        .map(|(name, _)| name.as_str())
        .collect();
    assert_eq!(
        apostle.len(),
        9,
        "the Apostle promotion set moved; re-read the treatment's table"
    );
    // Exactly the comparator `prepare_unit_formations` uses, ties included.
    let mut ranked = apostle.clone();
    ranked.sort_by(|a, b| {
        magnitude(b)
            .partial_cmp(&magnitude(a))
            .unwrap()
            .then_with(|| a.cmp(b))
    });
    assert_eq!(
            ranked,
            [
                "translator",
                "indulgence_vendor",
                "proselytizer",
                "chaplain",
                "debater",
                "pilgrim",
                "orator",
                "heathen_conversion",
                "martyr",
            ],
            "the shipped magnitude rule no longer produces the order this treatment was written against"
        );
    // The mismatch itself: a percentage, a gold figure and a combat
    // strength ranked against each other as bare numbers — with the GOLD
    // promotion outranking both defensive ones.
    assert_eq!(magnitude("translator"), 200.0);
    assert!(rules.promotions["translator"]
        .effects
        .contains_key("foreign_spread_pct"));
    assert_eq!(magnitude("indulgence_vendor"), 100.0);
    assert!(rules.promotions["indulgence_vendor"]
        .effects
        .contains_key("gold_per_conversion"));
    assert_eq!(magnitude("debater"), 20.0);
    assert!(rules.promotions["debater"]
        .effects
        .contains_key("religious_strength"));
    // Proselytizer is the defensive one: it is what raises `do_spread`'s
    // eviction of rival pressure from 0.25 to 0.75.
    assert_eq!(
        rules.promotions["proselytizer"].effects["rival_pressure_removed_pct"],
        50.0
    );
}

/// The treatment's whole content: the same empire asks for a different
/// promotion depending on what is happening to it — and asks for nothing at
/// all when nothing is.
#[test]
fn the_apostle_promotion_follows_the_situation() {
    let mut ai = BasicAi::new();
    ai.apostle_promotion_by_role = true;
    let mut game = Game::new(2, 32, 24, 9_311, 250, 0);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .expect("starting settler");
    game.apply(0, &crate::game::Action::FoundCity { unit: settler })
        .expect("found city");
    let cid = game.player_city_ids(0)[0];

    // 1. A quiet homeland: our own faith holds the city outright, and the
    //    treatment declines to have an opinion, which is what leaves the
    //    shipped rule in charge.
    game.players[0].religion = Some("ours".to_string());
    {
        let city = game.cities.get_mut(&cid).unwrap();
        city.pressure.clear();
        city.pressure.insert("ours".to_string(), 400.0);
        city.atheist_pressure = 0.0;
    }
    assert_eq!(
        game.city_religion(&game.cities[&cid]),
        Some("ours"),
        "the fixture must actually seat our own faith"
    );
    assert_eq!(
        ai.apostle_promotion_preference(&game, 0)[0],
        "translator",
        "a quiet homeland spreads abroad"
    );
    assert_eq!(
        ai.apostle_promotion_preference(&game, 0)[1],
        "proselytizer",
        "and its SECOND choice is a conversion promotion, not the gold one — \
             this is the branch the census showed being got wrong 6 times in 24"
    );

    // 2. A rival faith has reached half our pressure — still ours, already
    //    being lost. This is the branch a majority test cannot see.
    game.cities
        .get_mut(&cid)
        .unwrap()
        .pressure
        .insert("theirs".to_string(), 200.0);
    assert_eq!(
        game.city_religion(&game.cities[&cid]),
        Some("ours"),
        "the city has NOT flipped yet — that is the point of this branch"
    );
    assert_eq!(
        ai.apostle_promotion_preference(&game, 0)[0],
        "proselytizer",
        "a city being lost asks for the promotion that evicts rival pressure"
    );

    // And once it has flipped outright, still reconversion.
    {
        let city = game.cities.get_mut(&cid).unwrap();
        city.pressure.clear();
        city.pressure.insert("theirs".to_string(), 400.0);
    }
    assert_eq!(game.city_religion(&game.cities[&cid]), Some("theirs"));
    assert_eq!(
        ai.apostle_promotion_preference(&game, 0)[0],
        "proselytizer",
        "a city already lost is the same job"
    );

    // 3. Homeland quiet again, but a foreign religious unit is inside our
    //    reach: the fight is happening whether we chose it or not.
    {
        let city = game.cities.get_mut(&cid).unwrap();
        city.pressure.clear();
        city.pressure.insert("ours".to_string(), 400.0);
    }
    assert_eq!(
        ai.apostle_promotion_preference(&game, 0)[0],
        "translator",
        "the fixture must be back to quiet before the unit is placed"
    );
    let home = game.cities[&cid].pos;
    let intruder = game.spawn_unit("missionary", 1, home);
    game.units.get_mut(&intruder).unwrap().religion = Some("theirs".to_string());
    assert_eq!(
        ai.apostle_promotion_preference(&game, 0)[0],
        "debater",
        "a foreign religious unit at the gates asks for the theological-combat promotion"
    );

    // A unit of OUR OWN faith at the same tile is not a threat, so the
    // detector must not fire on our own Apostles standing at home.
    game.units.get_mut(&intruder).unwrap().religion = Some("ours".to_string());
    assert_eq!(
        ai.apostle_promotion_preference(&game, 0)[0],
        "translator",
        "a co-religionist is not an intruder"
    );

    // And the flag decides whether any of this is consulted at all.
    assert!(
        !BasicAi::new().apostle_promotion_by_role,
        "the treatment must ship off"
    );
}

/// ★★★★ THE APOSTLE IS THE ONLY PROMOTABLE RELIGIOUS UNIT, AND THAT IS WHY
/// A PROMOTION CANNOT BE A DEFENSIVE REPAIR.
///
/// Missionary, Guru and Inquisitor carry no `promotion_class` at all, so
/// they can never take a promotion — and those are exactly the units a
/// defending empire buys: `AdvancedAi::religious_spending_with_reserve`
/// asks for `["missionary", "inquisitor"]` when it is not on the offensive,
/// and puts the Inquisitor first when the home cities are under pressure.
/// So promotion choice can only ever reach an empire that is SPREADING,
/// and the two situational branches of
/// [`BasicAi::apostle_promotion_by_role`] are nearly unreachable by
/// construction. The reachability census below measured them at exactly
/// zero over four whole games.
///
/// This is a fact about the shipped roster, not about the flag, and it is
/// pinned here so the next person to reach for a promotion as a religious
/// defence learns it from a failing test instead of from a measurement.
#[test]
fn the_apostle_is_the_only_religious_unit_that_can_be_promoted() {
    let rules = crate::rules::Rules::embedded();
    let religious: Vec<(&str, &str)> = rules
        .units
        .iter()
        .filter(|(_, spec)| spec.class == "religious")
        .map(|(name, spec)| (name.as_str(), spec.promotion_class.as_str()))
        .collect();
    assert!(
        religious.len() >= 4,
        "the religious roster shrank; re-read this test's premise"
    );
    for (unit, promotion_class) in &religious {
        if *unit == "apostle" {
            assert_eq!(
                *promotion_class, "religious_apostle",
                "the Apostle must still be promotable or the treatment has no subject"
            );
        } else {
            assert!(
                promotion_class.is_empty(),
                "{unit} became promotable — a promotion can now reach a defending \
                     empire, so the treatment's defensive branches are no longer \
                     unreachable and its census should be re-run"
            );
        }
    }
}

/// ★★★ IS THE TREATMENT REACHABLE AT ALL? This is the cheap question, and
/// it is the precondition for a win rate meaning anything —
/// `src/bin/gene_census.rs` states the doctrine: *does this control change
/// anything at all* is far cheaper to establish than a win rate, and a gene
/// that cannot change an outcome cannot be selected on.
///
/// Plays whole games with the flag off and on and counts the promotion each
/// Apostle actually took. Three ways this can come back, and only one of
/// them justifies spending hours on a win-rate screen:
///
/// - **no Apostles at all** — the treatment is unreachable at this profile
///   and the screen would measure nothing;
/// - **Apostles, but the same promotions** — the situational branches never
///   fire, so the flag is a no-op;
/// - **different promotions** — the treatment is live, and the census says
///   how often and in which direction.
///
/// Ignored because it plays whole games. Run it with:
/// `CIVVIS_APOSTLE_AB_SEEDS=8 cargo test --profile ci --lib \
///  the_apostle_treatment_changes_the_promotions_actually_taken -- --ignored --nocapture`
#[test]
#[ignore]
fn the_apostle_treatment_changes_the_promotions_actually_taken() {
    use std::collections::BTreeMap;
    let count: u64 = std::env::var("CIVVIS_APOSTLE_AB_SEEDS")
        .ok()
        .and_then(|n| n.parse().ok())
        .unwrap_or(4);
    let seeds: Vec<u64> = (1..=count).map(|n| 26_081_900 + n).collect();
    let mut census: Vec<BTreeMap<String, usize>> = Vec::new();
    for treated in [false, true] {
        let mut taken: BTreeMap<String, usize> = BTreeMap::new();
        for seed in &seeds {
            let mut game = Game::new_with(crate::game::GameOptions {
                speed: "online".to_string(),
                ..crate::game::GameOptions::new(4, 60, 38, *seed, 250, 6)
            });
            let mut ais: Vec<crate::ai::AdvancedAi> = (0..game.players.len())
                .map(|_| {
                    let mut ai = crate::ai::AdvancedAi::new();
                    if treated {
                        ai.enable_apostle_promotion_by_role();
                    }
                    ai
                })
                .collect();
            crate::ai::run_game(&mut game, &mut ais);
            for unit in game.units.values() {
                if game.rules.units[unit.kind].promotion_class != "religious_apostle" {
                    continue;
                }
                for promotion in &unit.promotions {
                    *taken.entry(promotion.to_string()).or_default() += 1;
                }
            }
        }
        census.push(taken);
    }
    println!(
        "\napostle promotions taken over {} games per arm (4p 60x38 online-250):",
        seeds.len()
    );
    let mut names: Vec<&String> = census[0].keys().chain(census[1].keys()).collect();
    names.sort();
    names.dedup();
    println!("{:<22} {:>8} {:>8}", "promotion", "shipped", "by-role");
    for name in &names {
        println!(
            "{:<22} {:>8} {:>8}",
            name,
            census[0].get(*name).copied().unwrap_or(0),
            census[1].get(*name).copied().unwrap_or(0)
        );
    }
    let shipped_total: usize = census[0].values().sum();
    let treated_total: usize = census[1].values().sum();
    println!("{:<22} {shipped_total:>8} {treated_total:>8}", "total");
    // The assertion is only that the census RAN and found Apostles to
    // count. Whether the distribution moved is the thing being reported,
    // and a single small sweep cannot make that a pass/fail.
    assert!(
        shipped_total > 0 || treated_total > 0,
        "no Apostle survived to the end of any game at this profile, so this \
             census cannot say whether the treatment is reachable — raise \
             CIVVIS_APOSTLE_AB_SEEDS or count promotions as they are taken"
    );
}

/// Every returned branch names all nine promotions, so whichever three of
/// the nine the offer happens to hold, the chooser finds one and never
/// falls back to the magnitude rule by accident.
#[test]
fn every_apostle_preference_covers_the_whole_offer() {
    let rules = crate::rules::Rules::embedded();
    let apostle: std::collections::BTreeSet<&str> = rules
        .promotions
        .iter()
        .filter(|(_, spec)| spec.class == "religious_apostle")
        .map(|(name, _)| name.as_str())
        .collect();
    // The two situational branches, taken from the function itself by
    // driving it into each one rather than by copying its lists.
    let mut ai = BasicAi::new();
    ai.apostle_promotion_by_role = true;
    let mut game = Game::new(2, 32, 24, 9_313, 250, 0);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .expect("starting settler");
    game.apply(0, &crate::game::Action::FoundCity { unit: settler })
        .expect("found city");
    let cid = game.player_city_ids(0)[0];
    game.players[0].religion = Some("ours".to_string());

    let mut branches = Vec::new();
    {
        let city = game.cities.get_mut(&cid).unwrap();
        city.pressure.clear();
        city.pressure.insert("ours".to_string(), 400.0);
        city.pressure.insert("theirs".to_string(), 400.0);
        city.atheist_pressure = 0.0;
    }
    branches.push(ai.apostle_promotion_preference(&game, 0));
    {
        let city = game.cities.get_mut(&cid).unwrap();
        city.pressure.clear();
        city.pressure.insert("ours".to_string(), 400.0);
    }
    let home = game.cities[&cid].pos;
    let intruder = game.spawn_unit("missionary", 1, home);
    game.units.get_mut(&intruder).unwrap().religion = Some("theirs".to_string());
    branches.push(ai.apostle_promotion_preference(&game, 0));
    // And the offensive branch, which is the reachable one.
    game.units.get_mut(&intruder).unwrap().religion = Some("ours".to_string());
    branches.push(ai.apostle_promotion_preference(&game, 0));
    assert_eq!(branches.len(), 3);
    for branch in branches {
        let listed: std::collections::BTreeSet<&str> = branch.iter().copied().collect();
        assert_eq!(
            listed, apostle,
            "a preference branch does not name every Apostle promotion, so an \
                 offer of three could contain none of it"
        );
        assert_eq!(
            branch.len(),
            apostle.len(),
            "a preference branch repeats a promotion"
        );
    }
}
