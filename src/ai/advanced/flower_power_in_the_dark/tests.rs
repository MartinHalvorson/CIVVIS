//! `flower-power-in-the-dark`: unit tests.
use super::*;
use crate::ai::advanced::{GrandStrategy, StrategicPlan, VictoryTarget};
use crate::game::Action;
use crate::name::Name;

/// A full Democracy deck (1 military, 3 economic, 2 diplomatic, 2 wildcard)
/// in a Dark Age of the Information era, Cold War known, one Rock Band in
/// the field, at peace. Grand Opera overflows into a wildcard seat beside
/// Symphonies, the Great Person card.
fn board() -> (Game, u32) {
    let mut g = Game::new(2, 40, 16, 936_017, 200, 0);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|id| g.units[id].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let cid = g.player_city_ids(0)[0];
    let pos = g.cities[&cid].pos;
    g.spawn_unit("rock_band", 0, pos);
    g.world_era = 7;
    let player = &mut g.players[0];
    player.age = "dark".to_string();
    player.government = Some("democracy".to_string());
    player.civics.insert(crate::name!("cold_war"));
    for card in [
        "levee_en_masse",
        "heritage_tourism",
        "satellite_broadcasts",
        "online_communities",
        "grand_opera",
        "gunboat_diplomacy",
        "cryptography",
        "symphonies",
    ] {
        player.policies.insert(Name::new(card));
    }
    (g, cid)
}

fn seat(target: VictoryTarget, on: bool, g: &Game) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(target);
    if on {
        ai.enable_flower_power_in_the_dark();
    }
    ai.plan = Some(StrategicPlan {
        strategy: GrandStrategy::Culture,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 1,
        assessed_turn: g.turn,
        rush: false,
    });
    ai.base.journal = crate::reasoning::Journal::recording();
    ai
}

fn slotted(g: &Game, card: &str) -> bool {
    g.players[0].policies.contains(&Name::new(card))
}

fn lines(ai: &AdvancedAi, headline: &str) -> Vec<String> {
    ai.base
        .journal
        .since(0)
        .thoughts
        .iter()
        .filter(|thought| thought.headline.starts_with(headline))
        .map(|thought| format!("{} | {}", thought.headline, thought.detail))
        .collect()
}

#[test]
fn it_is_an_opt_in_off_by_default() {
    assert!(!AdvancedAi::targeting(VictoryTarget::Culture).flower_power_in_the_dark);
}

#[test]
fn a_peaceful_dark_age_with_cold_war_slots_and_keeps_the_card() {
    let (mut g, _) = board();
    assert!(g
        .available_policies(0)
        .contains(&crate::name!("flower_power")));
    let ai = seat(VictoryTarget::Culture, true, &g);
    assert!(matches!(ai.flower_power_verdict(&g, 0), Some(Ok(_))));
    ai.strategic_policies(&mut g, 0, GrandStrategy::Culture);
    assert!(slotted(&g, "flower_power"), "{:?}", g.players[0].policies);
    // The Great Person card in the wildcard seat made way; the plan's
    // tourism cards stayed.
    assert!(!slotted(&g, "symphonies"));
    for card in ["heritage_tourism", "satellite_broadcasts", "grand_opera"] {
        assert!(slotted(&g, card), "{card}");
    }
    let slot = lines(&ai, "Slotted flower power");
    assert_eq!(slot.len(), 1, "{slot:?}");
    assert!(slot[0].contains("flower-power-in-the-dark"), "{slot:?}");
    assert!(slot[0].contains("over symphonies"), "{slot:?}");

    // Held, it is wanted and stays past the situational-card drop.
    ai.strategic_policies(&mut g, 0, GrandStrategy::Culture);
    assert!(slotted(&g, "flower_power"));
    assert!(lines(&ai, "Dropping").is_empty());
}

/// Live 164254Z t178: taken first, the card took an unwanted card's seat
/// and Satellite Broadcasts, ranked above New Deal, lost its own. Taken
/// last, the wanted card seats first and Flower Power takes the seat of the
/// card the plan ranks lowest.
#[test]
fn the_card_seats_last_from_the_lowest_ranked_card() {
    let (mut g, _) = board();
    let player = &mut g.players[0];
    for card in ["grand_opera", "symphonies"] {
        player.policies.remove(&Name::new(card));
    }
    player.policies.insert(crate::name!("town_charters"));
    player.policies.insert(crate::name!("new_deal"));
    player.civics.insert(crate::name!("opera_ballet"));
    let ai = seat(VictoryTarget::Culture, true, &g);
    ai.strategic_policies(&mut g, 0, GrandStrategy::Culture);
    for card in ["flower_power", "grand_opera", "satellite_broadcasts"] {
        assert!(slotted(&g, card), "{card}: {:?}", g.players[0].policies);
    }
    assert!(!slotted(&g, "town_charters"));
    assert!(!slotted(&g, "new_deal"));
    let slot = lines(&ai, "Slotted flower power");
    assert_eq!(slot, ["Slotted flower power over new deal | flower-power-in-the-dark: 1 Rock Band(s) in the field; concerts +50% Tourism, no war near our cities"]);
}

#[test]
fn without_cold_war_the_card_is_refused() {
    let (mut g, _) = board();
    g.players[0].civics.remove(&crate::name!("cold_war"));
    let ai = seat(VictoryTarget::Culture, true, &g);
    assert!(matches!(ai.flower_power_verdict(&g, 0), Some(Err(why)) if why.contains("Cold War")));
    let before = g.players[0].policies.clone();
    ai.strategic_policies(&mut g, 0, GrandStrategy::Culture);
    assert_eq!(g.players[0].policies, before);
}

#[test]
fn without_a_band_the_faith_bank_decides() {
    let (mut g, _) = board();
    let band = g
        .player_unit_ids(0)
        .into_iter()
        .find(|id| g.units[id].kind == "rock_band")
        .unwrap();
    g.remove_unit(band);
    g.players[0].faith = 0.0;
    let ai = seat(VictoryTarget::Culture, true, &g);
    assert!(matches!(ai.flower_power_verdict(&g, 0), Some(Err(_))));
    g.players[0].faith = 5_000.0;
    assert!(matches!(ai.flower_power_verdict(&g, 0), Some(Ok(_))));
}

#[test]
fn a_threatened_city_refuses_the_card() {
    let (mut g, cid) = board();
    let mut ai = seat(VictoryTarget::Culture, true, &g);
    ai.plan.as_mut().unwrap().threatened_city = Some(cid);
    assert!(matches!(ai.flower_power_verdict(&g, 0), Some(Err(why)) if why.contains("threatened")));
    ai.strategic_policies(&mut g, 0, GrandStrategy::Culture);
    assert!(!slotted(&g, "flower_power"));
    assert!(slotted(&g, "symphonies"));
    let waits = lines(&ai, "Flower power waits");
    assert_eq!(waits.len(), 1, "{waits:?}");
    assert!(waits[0].contains("a home city is threatened"), "{waits:?}");
    assert!(lines(&ai, "Dropping").is_empty());
}

#[test]
fn a_war_at_our_gates_drops_the_held_card() {
    let (mut g, cid) = board();
    g.players[0].policies.remove(&crate::name!("symphonies"));
    g.players[0].policies.insert(crate::name!("flower_power"));
    g.at_war.insert((0, 1));
    let near = g
        .map
        .tiles
        .keys()
        .copied()
        .find(|pos| {
            let tile = &g.map.tiles[pos];
            g.wdist(g.cities[&cid].pos, *pos) == 2
                && !g.rules.is_water(tile)
                && g.rules.is_passable(tile)
        })
        .unwrap();
    g.spawn_unit("warrior", 1, near);
    let ai = seat(VictoryTarget::Culture, true, &g);
    assert!(
        matches!(ai.flower_power_verdict(&g, 0), Some(Err(why)) if why.contains("at war with player 1"))
    );
    ai.strategic_policies(&mut g, 0, GrandStrategy::Culture);
    assert!(!slotted(&g, "flower_power"), "{:?}", g.players[0].policies);
    let drop = lines(&ai, "Dropping flower power");
    assert_eq!(drop.len(), 1, "{drop:?}");
    assert!(drop[0].contains("flower-power-in-the-dark"), "{drop:?}");
    assert!(lines(&ai, "Dropping the situational card").is_empty());
}

#[test]
fn another_target_plays_the_same_deck() {
    for target in [VictoryTarget::Science, VictoryTarget::Domination] {
        let (g, _) = board();
        let mut off = g.clone();
        let mut on = g.clone();
        let off_ai = seat(target, false, &g);
        let on_ai = seat(target, true, &g);
        assert_eq!(on_ai.flower_power_verdict(&on, 0), None);
        off_ai.strategic_policies(&mut off, 0, GrandStrategy::Science);
        on_ai.strategic_policies(&mut on, 0, GrandStrategy::Science);
        assert_eq!(on.players[0].policies, off.players[0].policies);
        assert!(!slotted(&on, "flower_power"));
    }
}

#[test]
fn off_the_culture_deck_is_unchanged() {
    let (g, _) = board();
    let ai = seat(VictoryTarget::Culture, false, &g);
    assert_eq!(ai.flower_power_verdict(&g, 0), None);
    let mut off = g.clone();
    ai.strategic_policies(&mut off, 0, GrandStrategy::Culture);
    assert!(!slotted(&off, "flower_power"));
    assert!(lines(&ai, "Slotted flower power").is_empty());

    // A held card goes as any situational card did before the gene.
    let mut held = g.clone();
    held.players[0].policies.remove(&crate::name!("symphonies"));
    held.players[0]
        .policies
        .insert(crate::name!("flower_power"));
    ai.strategic_policies(&mut held, 0, GrandStrategy::Culture);
    assert!(!slotted(&held, "flower_power"));
    assert_eq!(lines(&ai, "Dropping the situational card").len(), 1);
    assert!(lines(&ai, "Dropping flower power").is_empty());
}
