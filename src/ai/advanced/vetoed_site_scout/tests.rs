use super::*;
use crate::game::Game;

/// Open grassland, one city of ours at offset (17, 9), a Scout beside it, and
/// a rival plot at offset (6, 9) the mirror could not attribute to a seen
/// city. The site at offset (10, 9) lies four tiles from that plot, inside the
/// veto's radius. Our sight covers four tiles around home and the site.
struct Board {
    g: Game,
    ai: AdvancedAi,
    scout: u32,
    home: Pos,
    site: Pos,
    plot: Pos,
}

fn board() -> Board {
    let mut g = Game::new_full(2, 28, 18, 717009, 250, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.improvement = None;
        tile.resource = None;
        tile.owner_city = None;
        tile.district = None;
        tile.river_edges = [false; 6];
    }
    g.current = 0;
    let home = crate::hex::offset_to_axial(17, 9);
    g.found_city_for(0, home, None);
    let scout = g.spawn_test_unit("scout", 0, g.nbrs(home)[0]);
    let site = crate::hex::offset_to_axial(10, 9);
    let plot = crate::hex::offset_to_axial(6, 9);
    g.players[0].explored = g
        .wdisk(home, 4)
        .into_iter()
        .chain(g.wdisk(site, 4))
        .collect();
    g.unseen_major_borders.insert(plot);
    let mut ai = AdvancedAi::new();
    ai.frontier_loyalty = true;
    Board {
        g,
        ai,
        scout,
        home,
        site,
        plot,
    }
}

fn explore_around(b: &mut Board, centre: Pos, reach: i32) {
    let seen: Vec<Pos> = b.g.wdisk(centre, reach);
    b.g.players[0].explored.extend(seen);
}

#[test]
fn off_the_veto_is_the_plain_unresolved_border_test() {
    let mut b = board();
    assert!(AdvancedAi::beside_unresolved_major_border(&b.g, b.site));
    assert!(b.ai.unresolved_border_vetoes(&b.g, 0, b.site));
    // Every tile round the plot explored: off, the plot still vetoes.
    let plot = b.plot;
    explore_around(&mut b, plot, VETOED_BORDER_RESOLVE_REACH);
    assert!(b.ai.unresolved_border_vetoes(&b.g, 0, b.site));
    // Off, nothing is remembered and no unit is sent.
    b.ai.note_vetoed_site(&b.g, b.site, 150.0);
    assert!(b.ai.vetoed_sites.borrow().is_empty());
    assert_eq!(b.ai.vetoed_site_scout_goal(&b.g, 0, b.scout), None);
}

#[test]
fn an_explored_border_no_longer_vetoes_the_site() {
    let mut b = board();
    b.ai.enable_vetoed_site_sends_a_scout();
    assert!(b.ai.unresolved_border_vetoes(&b.g, 0, b.site), "fog beside the plot");
    let plot = b.plot;
    explore_around(&mut b, plot, VETOED_BORDER_RESOLVE_REACH - 1);
    assert!(
        b.ai.unresolved_border_vetoes(&b.g, 0, b.site),
        "a city five tiles out could still own the plot"
    );
    explore_around(&mut b, plot, VETOED_BORDER_RESOLVE_REACH);
    assert!(!b.ai.unresolved_border_vetoes(&b.g, 0, b.site));
    assert_eq!(b.ai.exhaustion_site_unpriceable(&b.g, 0, b.site), None);
}

#[test]
fn a_vetoed_good_site_sends_one_scout_toward_the_fog() {
    let mut b = board();
    b.ai.enable_vetoed_site_sends_a_scout();
    b.ai.note_vetoed_site(&b.g, b.site, 120.0);
    let (goal, site) = b
        .ai
        .vetoed_site_scout_goal(&b.g, 0, b.scout)
        .expect("a scout is sent");
    assert_eq!(site, b.site);
    assert!(!b.g.players[0].explored.contains(&goal));
    assert!(b.g.wdist(goal, b.plot) <= VETOED_BORDER_RESOLVE_REACH);
    // With two free scouts only the one nearer the fog looks; the other
    // keeps its ordinary sweep.
    let second = b
        .g
        .spawn_test_unit("scout", 0, crate::hex::offset_to_axial(17, 15));
    let sent: Vec<u32> = [b.scout, second]
        .into_iter()
        .filter(|scout| b.ai.vetoed_site_scout_goal(&b.g, 0, *scout).is_some())
        .collect();
    assert_eq!(sent.len(), 1, "one scout looks");
    let looker = sent[0];
    let (goal, _) = b.ai.vetoed_site_scout_goal(&b.g, 0, looker).unwrap();
    let start = b.g.wdist(b.g.units[&looker].pos, goal);
    for _ in 0..3 {
        b.g.turn += 1;
        let moves = b.g.unit_max_moves(looker);
        let unit = b.g.units.get_mut(&looker).unwrap();
        unit.moves_left = moves;
        unit.moved = false;
        unit.acted = false;
        b.ai.distance_scout_step(&mut b.g, 0, looker);
    }
    assert!(
        b.g.wdist(b.g.units[&looker].pos, goal) < start,
        "the scout heads for the fog beside the vetoing plot"
    );
}

#[test]
fn a_site_under_the_floor_sends_nobody() {
    let mut b = board();
    b.ai.enable_vetoed_site_sends_a_scout();
    b.ai.note_vetoed_site(&b.g, b.site, VETOED_SITE_WORTH_FLOOR - 1.0);
    assert!(b.ai.vetoed_sites.borrow().is_empty());
    assert_eq!(b.ai.vetoed_site_scout_goal(&b.g, 0, b.scout), None);
}

#[test]
fn the_hunt_ends_when_the_border_resolves_or_the_record_expires() {
    let mut b = board();
    b.ai.enable_vetoed_site_sends_a_scout();
    b.ai.note_vetoed_site(&b.g, b.site, 120.0);
    assert_eq!(b.ai.live_vetoed_sites(&b.g, 0).len(), 1);
    // Resolved: all the ground that could hide the city has been seen.
    let plot = b.plot;
    let mut resolved = board();
    resolved.ai.enable_vetoed_site_sends_a_scout();
    resolved.ai.note_vetoed_site(&resolved.g, resolved.site, 120.0);
    explore_around(&mut resolved, plot, VETOED_BORDER_RESOLVE_REACH);
    assert!(resolved.ai.live_vetoed_sites(&resolved.g, 0).is_empty());
    assert_eq!(
        resolved.ai.vetoed_site_scout_goal(&resolved.g, 0, resolved.scout),
        None
    );
    // Expired: no refusal for longer than the memory.
    b.g.turn += b.g.standard_duration(VETOED_SITE_MEMORY_STANDARD) + 1;
    assert!(b.ai.live_vetoed_sites(&b.g, 0).is_empty());
    assert!(b.ai.vetoed_sites.borrow().is_empty(), "the stale record is dropped");
    // Taken: a city within three tiles of the site.
    let mut taken = board();
    taken.ai.enable_vetoed_site_sends_a_scout();
    taken.ai.note_vetoed_site(&taken.g, taken.site, 120.0);
    let beside = taken.g.nbrs(taken.site)[0];
    taken.g.found_city_for(1, beside, None);
    assert!(taken.ai.live_vetoed_sites(&taken.g, 0).is_empty());
}

#[test]
fn with_no_scout_free_the_board_asks_a_land_unit_to_look() {
    let mut b = board();
    b.ai.enable_vetoed_site_sends_a_scout();
    b.ai.note_vetoed_site(&b.g, b.site, 120.0);
    assert!(b.ai.vetoed_border_row(&b.g, 0).is_none(), "a Scout is free");
    b.g.remove_unit(b.scout);
    b.g.spawn_test_unit("warrior", 0, b.g.nbrs(b.home)[0]);
    let row = b.ai.vetoed_border_row(&b.g, 0).expect("a look row");
    assert_eq!(row.kind, ObjectiveKind::Recon);
    assert_eq!(row.key, ObjectiveKey::VetoedBorder(b.site));
    assert!(!b.g.players[0].explored.contains(&row.at));
    assert!(b.g.wdist(row.at, b.plot) <= VETOED_BORDER_RESOLVE_REACH);
    let mut off = board();
    off.ai.note_vetoed_site(&off.g, off.site, 120.0);
    off.g.remove_unit(off.scout);
    assert!(off.ai.vetoed_border_row(&off.g, 0).is_none());
}
