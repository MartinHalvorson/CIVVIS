//! `science-heads-keep-their-queue`: the backlog Builder never displaces
//! science in progress.
//!
//! **The finding (87 live Deity Online Korea games, 2026-11-04/05).** From
//! t40 to t120, `backlog-builder-takes-the-queue` claimed a queue 1,075
//! times. 288 of those claims (27%, 3.3 a game) put the Builder ahead of a
//! science head: a Library 137 times, a University 115 and a Seowon 36. The
//! displaced item keeps its progress, but it waits the two to five turns the
//! Builder takes. That cost lands on the axis the seat loses on: 11 of the
//! last 13 live Deity games were rival Technology victories (t173-203).
//!
//! **The gene.** A city whose queue opens with a Campus-family district
//! (Korea's Seowon, matched by family and not by name) or with a building
//! that stands in one (Library, University, Research Lab, Madrasa) keeps its
//! queue against the claim. The claim then goes to the next-fastest city with
//! an idle queue or a non-science head. If every candidate city is building
//! science, no claim is made that turn and the next turn asks again. The
//! guard never falls back to displacing a science head. The idle-queue claims
//! and the backlog step in `pick_item` are untouched, so a city still trains
//! the Builder when it next chooses. Off, nothing changes.

use super::*;

impl AdvancedAi {
    /// Whether `item` is science: a Campus-family district or a building of
    /// one.
    pub(super) fn science_head(g: &Game, item: &Item) -> bool {
        match item {
            Item::District { district, .. } => g.district_family(*district) == "campus",
            Item::Building { building } => g
                .rules
                .buildings
                .get(building)
                .and_then(|spec| spec.district)
                .is_some_and(|district| g.district_family(district) == "campus"),
            _ => false,
        }
    }

    /// `science-heads-keep-their-queue`: whether `cid`'s queue opens with
    /// science the backlog claim must not displace. False with the gene off.
    pub(super) fn science_head_keeps_its_queue(&self, g: &Game, cid: u32) -> bool {
        self.science_heads_keep_their_queue
            && g.cities
                .get(&cid)
                .and_then(|city| city.queue.first())
                .is_some_and(|item| Self::science_head(g, item))
    }
}
