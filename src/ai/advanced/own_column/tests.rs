use super::*;
use crate::Pos;

/// Open grassland, no units but the ones a test places, every flag the live
/// bridge ships for these paths: the move-refusal watch, recorded steps and
/// the whole-turn reversal guard.
fn fixture(gene: bool) -> (AdvancedAi, Game) {
    let mut g = Game::new_full(2, 28, 18, 41, 30, 0, false);
    for tile in g.map.tiles.values_mut() {
        tile.terrain = "grassland".into();
        tile.feature = None;
        tile.hills = false;
    }
    g.units.clear();
    g.current = 0;
    let mut ai = AdvancedAi::new();
    ai.enable_live_move_refusal_break();
    ai.base.recorded_tactical_step = true;
    ai.base.whole_turn_backtrack_guard = true;
    if gene {
        ai.enable_own_column_is_not_a_refusal();
    }
    (ai, g)
}

fn warrior(g: &mut Game, pos: Pos) -> u32 {
    let uid = g.spawn_unit("warrior", 0, pos);
    g.units.get_mut(&uid).unwrap().moves_left = 8.0;
    uid
}

/// Plan a two-step walk on a disposable view, then report it to the bridge
/// as withheld (the air-assault barrier's frame 0). Returns the rollback
/// count.
fn plan_withheld_walk(ai: &mut AdvancedAi, g: &Game, uid: u32, steps: [Pos; 2]) -> usize {
    let before = ai.host_frame_movement();
    let mut view = g.speculative_clone();
    for to in steps {
        assert!(ai.base.path_move(&mut view, 0, uid, to), "the plan walks {to:?}");
    }
    let unsent: BTreeSet<u32> = [uid].into();
    ai.note_host_moves(g, before, &BTreeSet::new(), &unsent)
}

#[test]
fn the_gene_is_an_independently_reversible_opt_in() {
    assert!(!AdvancedAi::new().own_column_is_not_a_refusal_enabled());
    let mut ai = AdvancedAi::new();
    ai.enable_own_column_is_not_a_refusal();
    assert!(ai.own_column_is_not_a_refusal_enabled());
    assert!(ai.base.own_column_is_not_a_refusal);
    ai.disable_own_column_is_not_a_refusal();
    assert!(!ai.own_column_is_not_a_refusal_enabled());
    assert!(!ai.base.own_column_is_not_a_refusal);
    assert!(ai.host_frame_movement().is_none(), "the off arm takes no snapshot");
}

/// G104 t216-217: frame 0's ground orders were withheld behind the sortie,
/// but the plan's path trail survived, so the next frame's replan found the
/// unit's own first step "already walked" and gave it nothing.
#[test]
fn a_withheld_walk_does_not_block_the_next_frames_same_step() {
    let (mut ai, mut g) = fixture(true);
    let uid = warrior(&mut g, (5, 5));
    assert_eq!(plan_withheld_walk(&mut ai, &g, uid, [(6, 5), (7, 5)]), 1);
    assert!(
        ai.base.path_step_allowed(&g, uid, (5, 5), (6, 5), false),
        "the withheld walk left no trail: the replan may take the same first step"
    );
    assert!(ai.base.move_refusal_watch.borrow().get(&uid).is_none());

    // The legacy arm keeps the plan's trail, and the reversal guard refuses
    // the very step the host never saw.
    let (mut legacy, mut g) = fixture(false);
    let uid = warrior(&mut g, (5, 5));
    assert_eq!(plan_withheld_walk(&mut legacy, &g, uid, [(6, 5), (7, 5)]), 0);
    assert!(!legacy.base.path_step_allowed(&g, uid, (5, 5), (6, 5), false));
}

/// A withheld step two turns running is not a host refusal: no bar. The
/// legacy arm bars it after the second turn, exactly as G104's t218 bar.
#[test]
fn a_step_the_host_was_never_sent_is_never_struck() {
    for gene in [true, false] {
        let (mut ai, mut g) = fixture(gene);
        let uid = warrior(&mut g, (5, 5));
        for _ in 0..2 {
            plan_withheld_walk(&mut ai, &g, uid, [(6, 5), (7, 5)]);
            g.turn += 1;
            ai.base.judge_move_refusals(&g, 0);
        }
        assert_eq!(
            ai.base.move_refusal_blocked(&g, uid),
            !gene,
            "gene {gene}: a withheld step barred only on the legacy arm"
        );
    }
}

/// A move that crossed keeps its evidence: two sent, untaken turns are still
/// the host's refusal and still barred under the gene.
#[test]
fn a_sent_untaken_step_is_still_a_refusal() {
    let (mut ai, mut g) = fixture(true);
    let uid = warrior(&mut g, (5, 5));
    for _ in 0..2 {
        let before = ai.host_frame_movement();
        let mut view = g.speculative_clone();
        assert!(ai.base.path_move(&mut view, 0, uid, (6, 5)));
        let sent: BTreeSet<u32> = [uid].into();
        assert_eq!(ai.note_host_moves(&g, before, &sent, &BTreeSet::new()), 0);
        g.turn += 1;
        ai.base.judge_move_refusals(&g, 0);
    }
    assert!(ai.base.move_refusal_blocked(&g, uid));
}

/// A unit sent a move earlier this turn keeps that frame's evidence when a
/// later frame's replan of it is withheld: the rollback restores the earlier
/// frame's record, it does not erase it.
#[test]
fn a_later_withheld_frame_restores_the_earlier_sent_frame() {
    let (mut ai, mut g) = fixture(true);
    let uid = warrior(&mut g, (5, 5));
    let before = ai.host_frame_movement();
    let mut view = g.speculative_clone();
    assert!(ai.base.path_move(&mut view, 0, uid, (6, 5)));
    let sent: BTreeSet<u32> = [uid].into();
    ai.note_host_moves(&g, before, &sent, &BTreeSet::new());
    let watched = *ai.base.move_refusal_watch.borrow().get(&uid).unwrap();

    plan_withheld_walk(&mut ai, &g, uid, [(5, 6), (5, 7)]);
    assert_eq!(ai.base.move_refusal_watch.borrow().get(&uid), Some(&watched));
}

/// Firaxis checks stacking where a move ends: a step onto a tile a column
/// mate held when the frame began is congestion, not ground the host will not
/// let us walk. Two such turns bar nothing under the gene.
#[test]
fn a_step_our_own_column_held_is_not_a_strike() {
    for gene in [true, false] {
        let (mut ai, mut g) = fixture(gene);
        let uid = warrior(&mut g, (5, 5));
        let mate = warrior(&mut g, (6, 5));
        for _ in 0..2 {
            ai.base.begin_movement_turn(&g, 0);
            // The planning board walks the mate off first; the host may not.
            let mut view = g.speculative_clone();
            assert!(ai.base.path_move(&mut view, 0, mate, (7, 5)));
            assert!(ai.base.path_move(&mut view, 0, uid, (6, 5)));
            g.turn += 1;
            ai.base.judge_move_refusals(&g, 0);
        }
        assert_eq!(
            ai.base.move_refusal_blocked(&g, uid),
            !gene,
            "gene {gene}: a column-held step barred only on the legacy arm"
        );
    }
}

/// A different layer is not congestion: a Builder on the tile does not stop
/// a Warrior ending its move there, so a refusal there still counts.
#[test]
fn a_civilian_on_the_tile_is_not_congestion_for_a_warrior() {
    let (mut ai, mut g) = fixture(true);
    let uid = warrior(&mut g, (5, 5));
    g.spawn_unit("builder", 0, (6, 5));
    for _ in 0..2 {
        ai.base.begin_movement_turn(&g, 0);
        let before = ai.host_frame_movement();
        let mut view = g.speculative_clone();
        assert!(ai.base.path_move(&mut view, 0, uid, (6, 5)));
        let sent: BTreeSet<u32> = [uid].into();
        ai.note_host_moves(&g, before, &sent, &BTreeSet::new());
        g.turn += 1;
        ai.base.judge_move_refusals(&g, 0);
    }
    assert!(ai.base.move_refusal_blocked(&g, uid));
}

/// A turn on which no move of the unit reached the host is not a lap of a
/// circle: once the bridge reports, it neither counts as fruitless nor enters
/// the livelock window. The legacy arm counts every turn.
#[test]
fn a_turn_without_a_sent_move_is_not_a_fruitless_lap() {
    for gene in [true, false] {
        let (mut ai, mut g) = fixture(gene);
        let uid = warrior(&mut g, (5, 5));
        let other = warrior(&mut g, (9, 9));
        for _ in 0..4 {
            // The bridge reports a frame in which only `other` moved.
            let before = ai.host_frame_movement();
            let sent: BTreeSet<u32> = [other].into();
            ai.note_host_moves(&g, before, &sent, &BTreeSet::new());
            g.turn += 1;
            ai.base.observe_unit_motion(&g, 0);
        }
        let fruitless = ai.base.unit_motion.get(&uid).map_or(0, |m| m.fruitless);
        let window = ai.base.unit_motion.get(&uid).map_or(0, |m| m.tiles.len());
        if gene {
            assert_eq!((fruitless, window), (0, 0), "an unsent turn is inert");
            let sent_fruitless = ai.base.unit_motion.get(&other).map_or(0, |m| m.fruitless);
            assert!(sent_fruitless >= 3, "a sent mover is still judged: {sent_fruitless}");
        } else {
            assert!(fruitless >= 3, "the legacy arm counts every turn: {fruitless}");
        }
    }
}

/// Off the bridge nothing reports, and the livelock detector is unchanged
/// even with the gene on.
#[test]
fn without_a_report_the_livelock_detector_is_unchanged() {
    let (mut ai, mut g) = fixture(true);
    let uid = warrior(&mut g, (5, 5));
    for _ in 0..4 {
        g.turn += 1;
        ai.base.observe_unit_motion(&g, 0);
    }
    assert!(ai.base.unit_motion.get(&uid).is_some_and(|m| m.fruitless >= 3));
}
