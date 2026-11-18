use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::ai::advanced::trade_district_follows_the_campus::TradeClaim;
use crate::ai::industrial_zones::tests::producer_empire;
use crate::rules::Yields;
use std::sync::Arc;

const TAG: &str = "science-universities-pass-the-claims:";

fn unit(name: &str) -> Item {
    Item::Unit {
        unit: Name::new(name),
    }
}

fn building(name: &str) -> Item {
    Item::Building {
        building: Name::new(name),
    }
}

fn university() -> Item {
    building("university")
}

fn head(g: &Game, cid: u32) -> Option<Item> {
    g.cities[&cid].queue.first().cloned()
}

fn calm_plan(g: &Game) -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 9,
        assessed_turn: g.turn,
        rush: false,
    }
}

/// Run `claim` with a recording journal and return its thoughts' details.
fn journalled(
    ai: &mut AdvancedAi,
    g: &mut Game,
    claim: impl FnOnce(&AdvancedAi, &mut Game),
) -> Vec<String> {
    let journal = crate::reasoning::Journal::recording();
    journal.begin_turn(g.turn, 0);
    ai.attach_journal(journal.handle());
    claim(ai, g);
    journal
        .since(0)
        .thoughts
        .into_iter()
        .map(|thought| thought.detail)
        .collect()
}

/// Set `cid`'s Production to `production` a turn.
fn set_production(g: &mut Game, cid: u32, production: f64) {
    let adjustment = Arc::make_mut(&mut g.observed_city_yield_adjustments)
        .remove(&cid)
        .unwrap_or_default();
    let base = g.city_yields(cid).production;
    Arc::make_mut(&mut g.observed_city_yield_adjustments).insert(
        cid,
        Yields {
            production: production - base,
            ..adjustment
        },
    );
}

/// The six-city empire of `producer_empire` with its Libraries standing and
/// Education in hand, every city making `production`, on a Science seat that
/// carries the live claim genes (`campus-building-claims-the-queue`,
/// `campus-and-library-before-the-claims`) and this one when `gene`.
/// `near-done-work-finishes` is off so only the claims decide.
fn board(production: f64, gene: bool) -> (Game, Vec<u32>, AdvancedAi, StrategicPlan) {
    let (mut g, ranked) = producer_empire("SCIENCEUNIVERSITIES", 94_141, 1);
    for tech in ["education", "mathematics"] {
        g.players[0].techs.insert(Name::new(tech));
    }
    for cid in &ranked {
        set_production(&mut g, *cid, production);
        assert_eq!(
            BasicAi::campus_building_item(&g, 0, *cid),
            Some(university()),
            "fixture: the University is buildable"
        );
    }
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.enable_campus_building_claims_the_queue();
    ai.enable_campus_and_library_before_the_claims();
    ai.disable_near_done_work_finishes();
    if gene {
        ai.enable_science_universities_pass_the_claims();
    }
    let plan = calm_plan(&g);
    (g, ranked, ai, plan)
}

/// A district head on a free tile of `cid` with `paid` Production in.
fn district_head(g: &mut Game, cid: u32, family: &str, paid: f64) -> Item {
    let center = g.cities[&cid].pos;
    let site = g.cities[&cid]
        .owned_tiles
        .iter()
        .copied()
        .filter(|pos| *pos != center && g.map.tiles[pos].district.is_none())
        .min()
        .expect("a free tile");
    let item = Item::District {
        district: BasicAi::civ_district(g, 0, family),
        pos: site,
    };
    set_head(g, cid, &item, paid);
    item
}

fn set_head(g: &mut Game, cid: u32, item: &Item, paid: f64) {
    let city = g.cities.get_mut(&cid).unwrap();
    city.queue = vec![item.clone()];
    city.production = paid;
}

/// The heads of the six cities: an Industrial Zone and a Commercial Hub with
/// Production in (no routine claim may defer either), a Builder, a Granary,
/// an Aqueduct and a Settler.
fn held_heads(g: &mut Game, ranked: &[u32]) -> Vec<(u32, Item)> {
    let zone = district_head(g, ranked[0], "industrial_zone", 2.0);
    let hub = district_head(g, ranked[1], "commercial_hub", 2.0);
    let aqueduct = district_head(g, ranked[4], "aqueduct", 0.0);
    let builder = unit("builder");
    let settler = AdvancedAi::early_settler_item();
    set_head(g, ranked[2], &builder, 0.0);
    set_head(g, ranked[3], &building("granary"), 0.0);
    set_head(g, ranked[5], &settler, 0.0);
    let heads = vec![
        (ranked[0], zone),
        (ranked[1], hub),
        (ranked[2], builder),
        (ranked[3], building("granary")),
        (ranked[4], aqueduct),
        (ranked[5], settler),
    ];
    for (cid, item) in &heads {
        assert!(
            g.item_cost_for_city(0, *cid, item) > g.city_yields(*cid).production,
            "fixture: {item:?} is more than a turn from done"
        );
    }
    heads
}

#[test]
fn ships_off() {
    opt_in_off_in_both_controllers("science-universities-pass-the-claims", |ai| {
        ai.science_universities_pass_the_claims
    });
}

/// The live shape: a Science seat's quick University waits behind an
/// Industrial Zone and a Commercial Hub with Production in, because the claim
/// may not defer either and the Domination gene does not reach a Science
/// seat. With the gene both pass; the Builder, Granary, Aqueduct and Settler
/// keep their queue either way.
#[test]
fn the_university_passes_a_zone_and_a_hub_but_not_a_builder_granary_aqueduct_or_settler() {
    for gene in [false, true] {
        let (mut g, ranked, mut ai, plan) = board(15.0, gene);
        let heads = held_heads(&mut g, &ranked);
        let details = journalled(&mut ai, &mut g, |ai, g| {
            ai.claim_campus_building(g, 0, &plan)
        });
        for (cid, item) in &heads[..2] {
            let expected = if gene { university() } else { item.clone() };
            assert_eq!(head(&g, *cid), Some(expected), "gene {gene}");
            assert_eq!(
                g.item_invested_production(*cid, item),
                2.0,
                "the displaced item keeps its progress"
            );
        }
        for (cid, item) in &heads[2..] {
            assert_eq!(
                head(&g, *cid),
                Some(item.clone()),
                "{item:?} keeps its queue, gene {gene}"
            );
        }
        for passed in ["the Industrial Zone", "the Commercial Hub"] {
            assert_eq!(
                details.iter().any(|detail| detail.starts_with(TAG)
                    && detail.contains(&format!("University goes ahead of {passed}"))),
                gene,
                "{details:?}"
            );
        }
    }
}

/// The claim takes one city a turn without the gene; with it every city
/// whose University is quick claims in the same turn.
#[test]
fn every_city_claims_its_university_in_one_turn_only_with_the_gene() {
    for gene in [false, true] {
        let (mut g, ranked, mut ai, plan) = board(15.0, gene);
        for cid in &ranked {
            set_head(&mut g, *cid, &building("market"), 0.0);
        }
        ai.claim_campus_building(&mut g, 0, &plan);
        let claimed = ranked
            .iter()
            .filter(|cid| head(&g, **cid) == Some(university()))
            .count();
        assert_eq!(claimed, if gene { ranked.len() } else { 1 }, "gene {gene}");
    }
}

/// A slow University (125 Production at 7 a turn, about 18 Online turns),
/// another victory target and the plan's threatened city keep the old
/// order: the Industrial Zone with Production in stays first.
#[test]
fn a_slow_university_another_target_or_a_threatened_city_keeps_the_zone() {
    let slow = {
        let (g, ranked, ai, plan) = board(7.0, true);
        let turns = g.item_cost_for_city(0, ranked[0], &university()) / 7.0;
        assert!(
            turns > AdvancedAi::science_university_turns(&g)
                && turns <= crate::ai::campus_buildings::CAMPUS_BUILDING_MAX_TURNS,
            "fixture: the University is {turns} turns away"
        );
        (g, ranked, ai, plan)
    };
    let culture = {
        let (g, ranked, mut ai, plan) = board(15.0, true);
        ai.victory_target = Some(VictoryTarget::Culture);
        (g, ranked, ai, plan)
    };
    let threatened = {
        let (g, ranked, ai, mut plan) = board(15.0, true);
        plan.threatened_city = Some(ranked[0]);
        (g, ranked, ai, plan)
    };
    for (label, (mut g, ranked, ai, plan)) in [
        ("slow", slow),
        ("culture", culture),
        ("threatened", threatened),
    ] {
        let zone = district_head(&mut g, ranked[0], "industrial_zone", 2.0);
        for cid in &ranked[1..] {
            set_head(&mut g, *cid, &AdvancedAi::early_settler_item(), 0.0);
        }
        ai.claim_campus_building(&mut g, 0, &plan);
        assert_eq!(head(&g, ranked[0]), Some(zone), "{label}");
    }
}

/// Once the gene's University holds the head, the trade-district claim's
/// Market or Lighthouse may not pass it, and the writers that read
/// `campus_and_library_holds` leave it alone even with the Domination gene
/// off.
#[test]
fn the_university_holds_against_the_trade_claim_and_the_writers() {
    for gene in [false, true] {
        let (mut g, ranked, mut ai, plan) = board(15.0, gene);
        ai.disable_campus_and_library_before_the_claims();
        let cid = ranked[0];
        set_head(&mut g, cid, &university(), 0.0);
        assert_eq!(
            ai.trade_claim_may_pass(&g, 0, cid, &plan, &university(), TradeClaim::Building),
            !gene,
            "gene {gene}"
        );
        assert_eq!(
            ai.campus_and_library_holds(&g, 0, cid, &plan, &university()),
            gene,
            "gene {gene}"
        );
    }
}

/// The University passes the economy's buildings (a Workshop, a Market, a
/// Lighthouse) and districts, never a Builder, a soldier, Walls, a Shrine or
/// a wonder.
#[test]
fn the_university_passes_the_economy_items_only() {
    let (g, ranked, ai, _) = board(15.0, true);
    let cid = ranked[0];
    for (item, passes) in [
        (building("workshop"), true),
        (building("market"), true),
        (building("lighthouse"), true),
        (building("walls"), false),
        (building("shrine"), false),
        (building("granary"), false),
        (unit("builder"), false),
        (unit("warrior"), false),
    ] {
        assert_eq!(
            ai.science_university_passes(&g, 0, cid, &item).is_some(),
            passes,
            "{item:?}"
        );
    }
}
