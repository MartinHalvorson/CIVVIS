use super::super::AdvancedAi;
use crate::ai::Ai;
use crate::game::Game;
use crate::reasoning::Journal;
use crate::Pos;
use std::sync::Arc;

fn agent(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_engine_repairs();
    ai.enable_settler_never_idles();
    if gene {
        ai.enable_settler_fallback_takes_seen_ground();
    }
    ai
}

/// A one-player board seen in full, a capital founded where the Settler
/// stands, and the Settler still beside it.
fn board() -> (Game, u32) {
    let mut g = Game::new_full(1, 30, 20, 91_347, 250, 0, false);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| g.units[uid].kind == "settler")
        .unwrap();
    let home = g.units[&settler].pos;
    g.found_city_for(0, home, None);
    let explored: Vec<Pos> = g.map.tiles.keys().copied().collect();
    g.players[0].explored.extend(explored);
    Arc::make_mut(&mut g.rules).enable_unknown_terrain();
    (g, settler)
}

/// Mark `pos` as a plot the seat has never seen, as the live mirror does:
/// unknown terrain the route planner may still cross.
fn fog(g: &mut Game, pos: Pos) {
    let tile = g.map.tiles.get_mut(&pos).unwrap();
    tile.terrain = crate::name!("unknown");
    tile.assumed_traversable = true;
}

#[test]
fn the_gene_ships_off_and_the_toggles_are_twins() {
    assert!(!AdvancedAi::new().settler_fallback_takes_seen_ground);
    assert!(!AdvancedAi::legacy().settler_fallback_takes_seen_ground);
    let mut ai = AdvancedAi::new();
    ai.enable_settler_fallback_takes_seen_ground();
    assert!(ai.settler_fallback_takes_seen_ground);
    ai.disable_settler_fallback_takes_seen_ground();
    assert!(!ai.settler_fallback_takes_seen_ground);
}

/// Off, the ranked list keeps its unseen plots in place and the ranked tier
/// takes the unseen plot that ranks first; on, the unseen plots leave the
/// list in their order and the ranked tier takes the seen site.
#[test]
fn the_ranked_tier_takes_its_seen_site_before_an_unseen_one() {
    let (mut g, settler) = board();
    let off = agent(false);
    let seen = off
        .exhaustion_legal_site(&g, 0, settler, 14, None, false)
        .expect("fixture: the open board offers a legal site");
    let unseen = g
        .wdisk(seen, 3)
        .into_iter()
        .filter(|pos| *pos != seen && off.base.valid_settle_site(&g, 0, *pos))
        .find(|pos| g.route_step(settler, *pos, 0).is_some())
        .expect("fixture: a second reachable legal plot");
    fog(&mut g, unseen);
    assert!(AdvancedAi::plot_is_unseen(&g, unseen));
    assert!(!AdvancedAi::plot_is_unseen(&g, seen));
    let listed = vec![(unseen, 40.0), (seen, 30.0)];

    let mut ranked = listed.clone();
    assert!(off.exhaustion_fog_set_aside(&g, &mut ranked).is_empty());
    assert_eq!(ranked, listed, "off, the list is untouched");
    assert_eq!(
        off.exhaustion_ranked_site(&g, 0, settler, &mut ranked),
        Some((unseen, 40.0)),
        "off, the unseen plot that ranks first is taken"
    );

    let mut on = agent(true);
    let journal = Journal::recording();
    on.attach_journal(journal.handle());
    let mut ranked = listed.clone();
    let fogged = on.exhaustion_fog_set_aside(&g, &mut ranked);
    assert_eq!(fogged, vec![(unseen, 40.0)]);
    assert_eq!(ranked, vec![(seen, 30.0)]);
    let taken = on.exhaustion_ranked_site(&g, 0, settler, &mut ranked);
    assert_eq!(taken, Some((seen, 30.0)));
    on.journal_fog_passed_over(&g, 0, settler, seen, Some(30.0), &fogged);
    let lines: Vec<_> = journal
        .since(0)
        .thoughts
        .into_iter()
        .filter(|t| t.headline == "Settler takes seen ground before the fog")
        .collect();
    assert_eq!(lines.len(), 1, "the passed-over plot is journalled once");
    assert!(lines[0].detail.contains(&format!("{unseen:?}")));
}

/// A seen site that already ranked above every unseen plot passed nothing
/// over, so nothing is journalled; nor is anything with no unseen plot.
#[test]
fn nothing_is_journalled_when_no_unseen_plot_was_passed_over() {
    let (mut g, settler) = board();
    let mut on = agent(true);
    let journal = Journal::recording();
    on.attach_journal(journal.handle());
    let seen = on
        .exhaustion_legal_site(&g, 0, settler, 14, None, true)
        .expect("fixture: a legal site");
    let unseen = g
        .wdisk(seen, 4)
        .into_iter()
        .find(|pos| *pos != seen && on.base.valid_settle_site(&g, 0, *pos))
        .expect("fixture: a second legal plot");
    fog(&mut g, unseen);
    on.journal_fog_passed_over(&g, 0, settler, seen, Some(50.0), &[(unseen, 40.0)]);
    on.journal_fog_passed_over(&g, 0, settler, seen, None, &[]);
    assert!(journal
        .since(0)
        .thoughts
        .iter()
        .all(|t| t.headline != "Settler takes seen ground before the fog"));
}

/// The legal-site tier over seen plots never names an unseen plot, and
/// still names a seen one while one is legal and reachable.
#[test]
fn the_seen_legal_tier_passes_over_unseen_plots() {
    let (mut g, settler) = board();
    let ai = agent(true);
    let first = ai
        .exhaustion_legal_site(&g, 0, settler, 14, None, false)
        .expect("fixture: the open board offers a legal site");
    fog(&mut g, first);
    let seen = ai
        .exhaustion_legal_site(&g, 0, settler, 14, None, true)
        .expect("a seen legal site remains");
    assert_ne!(seen, first);
    assert!(!AdvancedAi::plot_is_unseen(&g, seen));
    assert!(ai.base.valid_settle_site(&g, 0, seen));
}

/// The whole fallback with the gene on, over a board where every other
/// plot five or more tiles from the capital is fog: whatever ranks, the
/// Settler is sent to seen ground while seen ground is legal and reachable.
/// With the gene off the same board is answered by the historical order,
/// which here may or may not reach into the fog; the two differ only where
/// it does.
#[test]
fn the_fallback_sends_the_settler_to_seen_ground_first() {
    let (mut g, settler) = board();
    let home = g.units[&settler].pos;
    let plots: Vec<Pos> = g.map.tiles.keys().copied().collect();
    for pos in plots {
        if g.wdist(home, pos) >= 5 && (pos.0 + pos.1) % 2 == 0 {
            fog(&mut g, pos);
        }
    }
    let on = agent(true);
    let picked = on
        .settler_exhaustion_target(&g, 0, settler)
        .expect("seen ground remains legal and reachable");
    assert!(!AdvancedAi::plot_is_unseen(&g, picked));
    let off = agent(false);
    let historical = off.settler_exhaustion_target(&g, 0, settler);
    if historical.is_some_and(|site| !AdvancedAi::plot_is_unseen(&g, site)) {
        assert_eq!(Some(picked), historical, "seen either way: the same site");
    }
}
