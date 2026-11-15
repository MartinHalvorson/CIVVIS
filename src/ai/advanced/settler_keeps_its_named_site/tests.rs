use super::super::{AdvancedAi, Game, SETTLER_STALL_LIMIT};
use super::FOUNDING_SPACING;
use crate::ai::Ai;
use crate::game::HostUnitFacts;
use crate::reasoning::{Journal, Thought};
use crate::Pos;
use std::sync::Arc;

const CIV6_ID: i64 = 393_217;

fn agent(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    // As on the live bridge (`civvis_orders`).
    ai.settler_commit = true;
    ai.enable_settler_stall_counts_turns();
    if gene {
        ai.enable_settler_keeps_its_named_site();
    }
    ai
}

/// A Settler beside its capital, carrying a host id, before any step.
fn board() -> (Game, u32, Pos) {
    let mut g = Game::new_full(2, 30, 18, 9_204, 120, 0, false);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| g.units[uid].kind == "settler")
        .unwrap();
    let home = g.units[&settler].pos;
    g.found_city_for(0, home, None);
    let start = land_neighbour(&g, home);
    g.units.get_mut(&settler).unwrap().pos = start;
    Arc::make_mut(&mut g.host_unit_facts).insert(
        settler,
        HostUnitFacts {
            civ6_id: Some(CIV6_ID),
            ..HostUnitFacts::default()
        },
    );
    (g, settler, start)
}

fn land_neighbour(g: &Game, pos: Pos) -> Pos {
    g.nbrs(pos)
        .into_iter()
        .find(|p| {
            g.map
                .get(*p)
                .is_some_and(|tile| !g.rules.is_water(tile) && g.rules.is_passable(tile))
        })
        .expect("a land neighbour")
}

/// One decision frame with the Settler's movement restored.
fn frame(g: &mut Game, ai: &mut AdvancedAi, settler: u32) {
    let unit = g.units.get_mut(&settler).unwrap();
    unit.moves_left = 4.0;
    unit.acted = false;
    ai.advanced_settler_step(g, 0, settler);
}

/// A board, an agent and the site the Settler named on its first step.
fn committed(gene: bool) -> (Game, AdvancedAi, u32, Pos, Journal) {
    let (mut g, settler, _) = board();
    let mut ai = agent(gene);
    let journal = Journal::recording();
    ai.attach_journal(journal.handle());
    frame(&mut g, &mut ai, settler);
    let site = *ai
        .settler_targets
        .get(&settler)
        .expect("the Settler names a site");
    (g, ai, settler, site, journal)
}

fn changes(journal: &Journal) -> Vec<Thought> {
    journal
        .since(0)
        .thoughts
        .into_iter()
        .filter(|t| t.headline.starts_with("Settler changes its destination"))
        .collect()
}

#[test]
fn off_is_a_no_op() {
    let (mut g, mut ai, settler, site, journal) = committed(false);
    assert_eq!(ai.named_site_key(&g, settler), None);
    std::sync::Arc::make_mut(&mut g.rules).enable_unknown_terrain();
    let near = land_neighbour(&g, site);
    g.map.tiles.get_mut(&near).unwrap().terrain = crate::name!("unknown");
    g.map.tiles.get_mut(&near).unwrap().assumed_traversable = true;
    assert!(!ai.named_site_is_unkeepable(&g, 0, settler, near));
    // Even a dropped site changes nothing the gene owns.
    g.map.tiles.get_mut(&site).unwrap().terrain = crate::name!("coast");
    g.turn += 1;
    frame(&mut g, &mut ai, settler);
    assert_ne!(ai.settler_targets.get(&settler), Some(&site));
    assert!(ai.settler_named_sites.is_empty());
    assert!(changes(&journal).is_empty());
}

#[test]
fn the_site_is_kept_when_a_better_one_appears() {
    let (mut g, mut ai, settler, site, journal) = committed(true);
    assert_eq!(ai.settler_named_sites.get(&CIV6_ID), Some(&site));
    // Strip the named site's ring to bare desert, so another site now ranks
    // above it while the named one stays legal and safe.
    for pos in g.wdisk(site, 2) {
        if pos == site {
            continue;
        }
        if let Some(tile) = g.map.tiles.get_mut(&pos) {
            if !g.rules.is_water(tile) && g.rules.is_passable(tile) {
                tile.terrain = crate::name!("desert");
                tile.feature = None;
                tile.resource = None;
            }
        }
    }
    let (now_best, _) = ai.best_settler_target(&g, 0, settler, 8, None).unwrap();
    assert_ne!(now_best, site, "precondition: another site now ranks first");
    g.turn += 1;
    frame(&mut g, &mut ai, settler);
    assert_eq!(ai.settler_targets.get(&settler), Some(&site));
    assert!(changes(&journal).is_empty());
}

fn change_line_for(journal: &Journal, old: Pos) -> Thought {
    let lines = changes(journal);
    assert_eq!(lines.len(), 1, "one change, one line: {lines:?}");
    let line = lines.into_iter().next().unwrap();
    assert!(line
        .headline
        .starts_with(&format!("Settler changes its destination from {old:?}")));
    assert!(line.detail.starts_with("settler-keeps-its-named-site: "));
    assert!(line.detail.ends_with(&format!("; settler {CIV6_ID}")));
    line
}

#[test]
fn a_site_revealed_as_water_is_dropped_and_named() {
    let (mut g, mut ai, settler, site, journal) = committed(true);
    g.map.tiles.get_mut(&site).unwrap().terrain = crate::name!("coast");
    g.turn += 1;
    frame(&mut g, &mut ai, settler);
    let new = *ai.settler_targets.get(&settler).expect("a new site");
    assert_ne!(new, site);
    let line = change_line_for(&journal, site);
    assert!(
        line.detail.contains("the old site is water"),
        "{}",
        line.detail
    );
    assert_eq!(ai.settler_named_sites.get(&CIV6_ID), Some(&new));
}

#[test]
fn a_rival_city_within_the_spacing_drops_the_site_and_names_the_city() {
    let (mut g, mut ai, settler, site, journal) = committed(true);
    let beside = land_neighbour(&g, site);
    g.found_city_for(1, beside, None);
    g.turn += 1;
    frame(&mut g, &mut ai, settler);
    assert_ne!(ai.settler_targets.get(&settler), Some(&site));
    let line = change_line_for(&journal, site);
    let civ = &g.players[1].civ;
    assert!(
        line.detail
            .contains(&format!("{civ}'s city at {beside:?} stands 1 tiles")),
        "{}",
        line.detail
    );
}

#[test]
fn a_retired_site_is_dropped_and_named() {
    // The arrival Loyalty check, the cannot-found check and the retreat limit
    // all retire a site through `settler_dead_sites`.
    let (mut g, mut ai, settler, site, journal) = committed(true);
    ai.settler_dead_sites
        .entry(settler)
        .or_default()
        .insert(site, g.turn + 30);
    g.turn += 1;
    frame(&mut g, &mut ai, settler);
    assert_ne!(ai.settler_targets.get(&settler), Some(&site));
    let line = change_line_for(&journal, site);
    assert!(
        line.detail.contains("was retired for this Settler"),
        "{}",
        line.detail
    );
}

#[test]
fn a_threat_on_the_approach_is_named() {
    let (mut g, mut ai, settler, site, journal) = committed(true);
    ai.settler_threat_detour = true;
    ai.commitment_patience = false;
    ai.settler_threat_deferrals.insert(site, g.turn + 10);
    g.turn += 1;
    frame(&mut g, &mut ai, settler);
    assert_ne!(ai.settler_targets.get(&settler), Some(&site));
    let line = change_line_for(&journal, site);
    assert!(
        line.detail.contains("deferred for a visible threat"),
        "{}",
        line.detail
    );
}

#[test]
fn the_stall_limit_drop_is_named() {
    let (mut g, settler, start) = board();
    let mut ai = agent(true);
    let journal = Journal::recording();
    ai.attach_journal(journal.handle());
    // The host puts the Settler back where the turn found it, as a rebuilt
    // live board does for a held unit, so it never gets closer.
    let held = |g: &mut Game, ai: &mut AdvancedAi| {
        g.units.get_mut(&settler).unwrap().pos = start;
        frame(g, ai, settler);
    };
    held(&mut g, &mut ai);
    let site = ai.settler_targets[&settler];
    for _ in 0..SETTLER_STALL_LIMIT {
        g.turn += 1;
        held(&mut g, &mut ai);
    }
    assert_eq!(
        ai.settler_avoid.get(&settler).map(|(pos, _)| *pos),
        Some(site),
        "precondition: the stall limit set the site aside"
    );
    // The drop leaves the Settler without a site; the change is journalled
    // on the step that names the next one, whichever rule names it.
    assert!(!ai.settler_targets.contains_key(&settler));
    let next = land_neighbour(&g, start);
    ai.settler_targets.insert(settler, next);
    let key = ai.named_site_key(&g, settler);
    ai.note_named_site(&g, 0, settler, key);
    let new = ai.settler_targets[&settler];
    assert_ne!(new, site);
    let line = change_line_for(&journal, site);
    assert!(line.detail.contains("was set aside"), "{}", line.detail);
}

#[test]
fn an_unseen_plot_is_not_named() {
    let (mut g, settler, _) = board();
    Arc::make_mut(&mut g.rules).enable_unknown_terrain();
    // Mark the site the plain agent would name unseen, the way the live
    // mirror carries a frontier plot; its ring stays revealed.
    let off = agent(false);
    let first = off.best_settler_target(&g, 0, settler, 8, None).unwrap().0;
    let tile = g.map.tiles.get_mut(&first).unwrap();
    tile.terrain = crate::name!("unknown");
    tile.feature = None;
    tile.resource = None;
    tile.assumed_traversable = true;
    assert!(!off.named_site_is_unkeepable(&g, 0, settler, first));
    let on = agent(true);
    assert!(on.named_site_is_unkeepable(&g, 0, settler, first));
    let (site, _) = on
        .best_settler_target(&g, 0, settler, 8, None)
        .expect("a seen site");
    assert_ne!(site, first);
    assert!(!g.rules.is_unknown(&g.map.tiles[&site]));
}

#[test]
fn a_second_settler_names_a_site_outside_the_firsts_spacing() {
    let pick = |gene: bool| -> i32 {
        let (mut g, settler, start) = board();
        let second = g.spawn_unit("settler", 0, start);
        let mut ai = agent(gene);
        frame(&mut g, &mut ai, settler);
        frame(&mut g, &mut ai, second);
        g.wdist(ai.settler_targets[&settler], ai.settler_targets[&second])
    };
    assert!(pick(true) >= FOUNDING_SPACING, "both sites can be founded");
    assert!(
        pick(false) < FOUNDING_SPACING,
        "precondition: off, the second Settler names a site beside the first's"
    );
}
