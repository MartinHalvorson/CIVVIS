//! `settler-stall-counts-turns`: a Settler's stall limit counts game turns, not decision frames.
//!
//! `SETTLER_STALL_LIMIT` is documented as "consecutive turns without getting
//! closer", but `settler_stalls` grows on every call that does not advance the
//! Settler. The live bridge replans each turn in two or three decision frames
//! (783 sampled turns of 11-10: 96 one frame, 366 two, 321 three), and a Settler
//! that keeps its movement is stepped again in each. One held turn therefore
//! spent the whole limit, and a Settler that walked one tile and "ends its move
//! short" (one movement left, the next tile costs two) was stalled by the next
//! frames of the same turn. The site was then avoided for eight turns and
//! replaced without a journal line: civvis-20261111T000134Z t20, three "ends its
//! move short of (48, 7)" lines, then a new expedition to (49, 18), eleven
//! tiles from the old site. Across 28 Deity games (t2-80) about two thirds of
//! the unexplained mid-walk site changes on single-Settler turns carried that
//! signature.
//!
//! With the gene on, a stall is counted at most once per game turn, a turn on
//! which the Settler got closer is never a stall, the limit's site drop is
//! journaled, and the march's journal lines carry the Settler's stable id.
//! Off, every call counts as before and no line changes.
use super::{hostile_memory_key, AdvancedAi, Game, SETTLER_STALL_LIMIT};
use crate::think;
use crate::Pos;

impl AdvancedAi {
    /// The Settler's id across the live mirror's per-frame board rebuilds:
    /// the host's Civ VI id when the board carries one, else the board id.
    fn settler_stall_key(g: &Game, uid: u32) -> i64 {
        g.units
            .get(&uid)
            .map_or(i64::from(uid), |unit| hostile_memory_key(g, unit))
    }

    /// Count a stall for this Settler and return its stall count. Off, every
    /// call counts. On, a game turn already spoken for (a stall counted, or
    /// progress made) does not count again.
    pub(super) fn count_settler_stall(&mut self, g: &Game, uid: u32) -> u32 {
        let counts = !self.settler_stall_counts_turns
            || self
                .settler_stall_turn
                .insert(Self::settler_stall_key(g, uid), g.turn)
                != Some(g.turn);
        let stalls = self.settler_stalls.entry(uid).or_insert(0);
        if counts {
            *stalls += 1;
        }
        *stalls
    }

    /// The Settler got closer this turn, so a later frame that finds it out of
    /// movement short of a costly tile is not a stall. A no-op with the gene off.
    pub(super) fn note_settler_progress_turn(&mut self, g: &Game, uid: u32) {
        if self.settler_stall_counts_turns {
            self.settler_stall_turn
                .insert(Self::settler_stall_key(g, uid), g.turn);
        }
    }

    /// "; settler N" for the march's journal lines while the gene is on, so a
    /// line can be joined to one Settler when two are walking; empty when off.
    pub(super) fn settler_stall_id_note(&self, g: &Game, uid: u32) -> String {
        if self.settler_stall_counts_turns {
            format!("; settler {}", Self::settler_stall_key(g, uid))
        } else {
            String::new()
        }
    }

    /// Journal the stall limit's site drop, the one silent drop in the march.
    pub(super) fn journal_settler_stall_drop(&self, g: &Game, uid: u32, target: Pos) {
        if !self.settler_stall_counts_turns {
            return;
        }
        let id = Self::settler_stall_key(g, uid);
        think!(self.journal(), Expansion, Detail,
               "Settler sets {target:?} aside after {SETTLER_STALL_LIMIT} stalled turns";
               "settler-stall-counts-turns: {SETTLER_STALL_LIMIT} separate game turns without \
                getting closer, the last t{}; the site is avoided for 8 standard turns and \
                the next best is asked; settler {id}",
               g.turn; target);
    }
}

#[cfg(test)]
mod tests {
    use super::super::{AdvancedAi, Game, SETTLER_STALL_LIMIT};
    use crate::ai::Ai;
    use crate::game::HostUnitFacts;
    use crate::Pos;
    use std::sync::Arc;

    /// A committed Settler beside its capital, stepped once so it holds a site.
    fn committed_settler(gene: bool) -> (Game, AdvancedAi, u32, Pos, Pos) {
        let mut g = Game::new_full(2, 30, 18, 9_204, 120, 0, false);
        let settler = g
            .player_unit_ids(0)
            .into_iter()
            .find(|uid| g.units[uid].kind == "settler")
            .unwrap();
        let home = g.units[&settler].pos;
        g.found_city_for(0, home, None);
        let start = g
            .nbrs(home)
            .into_iter()
            .find(|position| {
                g.map
                    .get(*position)
                    .is_some_and(|tile| !g.rules.is_water(tile) && g.rules.is_passable(tile))
            })
            .expect("the capital has a land neighbour");
        g.units.get_mut(&settler).unwrap().pos = start;
        let mut ai = AdvancedAi::new();
        ai.settler_commit = true;
        if gene {
            ai.enable_settler_stall_counts_turns();
        }
        frame(&mut g, &mut ai, settler, start);
        let target = *ai
            .settler_targets
            .get(&settler)
            .expect("the settler commits to a site");
        (g, ai, settler, start, target)
    }

    /// One decision frame: the host put the Settler back where the turn found
    /// it, with its movement, as a rebuilt live board does for a held unit.
    fn frame(g: &mut Game, ai: &mut AdvancedAi, settler: u32, start: Pos) {
        let unit = g.units.get_mut(&settler).unwrap();
        unit.pos = start;
        unit.moves_left = 4.0;
        unit.acted = false;
        ai.advanced_settler_step(g, 0, settler);
    }

    #[test]
    fn three_frames_of_one_turn_count_one_stall_and_keep_the_site() {
        let (mut g, mut ai, settler, start, target) = committed_settler(true);
        // The commitment frame got closer, so the rest of that turn is spoken for.
        for _ in 0..SETTLER_STALL_LIMIT {
            frame(&mut g, &mut ai, settler, start);
        }
        assert_eq!(ai.settler_stalls.get(&settler).copied().unwrap_or(0), 0);
        g.turn += 1;
        for _ in 0..SETTLER_STALL_LIMIT {
            frame(&mut g, &mut ai, settler, start);
        }
        assert_eq!(ai.settler_stalls.get(&settler), Some(&1));
        assert_eq!(ai.settler_targets.get(&settler), Some(&target));
    }

    #[test]
    fn the_limit_still_releases_after_that_many_turns_and_says_so() {
        let (mut g, mut ai, settler, start, target) = committed_settler(true);
        Arc::make_mut(&mut g.host_unit_facts).insert(
            settler,
            HostUnitFacts {
                civ6_id: Some(327_682),
                ..HostUnitFacts::default()
            },
        );
        let journal = crate::reasoning::Journal::recording();
        ai.attach_journal(journal.handle());
        for turn in 1..=SETTLER_STALL_LIMIT {
            g.turn += 1;
            frame(&mut g, &mut ai, settler, start);
            frame(&mut g, &mut ai, settler, start);
            if turn < SETTLER_STALL_LIMIT {
                assert_eq!(ai.settler_targets.get(&settler), Some(&target));
            }
        }
        assert_ne!(ai.settler_targets.get(&settler), Some(&target));
        assert_eq!(
            ai.settler_avoid.get(&settler).map(|(pos, _)| *pos),
            Some(target)
        );
        let thoughts = journal.since(0).thoughts;
        assert!(thoughts.iter().any(|thought| {
            thought.headline.starts_with("Settler sets")
                && thought.detail.starts_with("settler-stall-counts-turns:")
                && thought.detail.ends_with("settler 327682")
        }));
        assert!(thoughts
            .iter()
            .filter(|thought| thought.headline.starts_with("Settler marching to"))
            .all(|thought| thought.detail.ends_with("; settler 327682")));
    }

    #[test]
    fn off_every_frame_counts_as_before() {
        let (mut g, mut ai, settler, start, target) = committed_settler(false);
        g.turn += 1;
        for _ in 0..SETTLER_STALL_LIMIT {
            frame(&mut g, &mut ai, settler, start);
        }
        assert_ne!(
            ai.settler_targets.get(&settler),
            Some(&target),
            "off, three frames of one turn spend the whole limit"
        );
        assert!(ai.settler_stall_turn.is_empty());
    }
}
