//! Which rivals keep our Rock Bands out of their land.
//!
//! Music Censorship (`POLICY_MUSIC_CENSORSHIP`, "Other civilizations' Rock
//! Bands cannot enter your territory"; `block_foreign_rock_bands` in
//! `data/policies.json`) is a rival's policy, and the export never shows a
//! rival's policies. The host does say where each of our bands could play:
//! `concert_plots`, the RockBand component's activation highlights
//! (`SelectedUnit_Expansion2.lua:65-70`, read by `CivvisRockBandConcertPlots`).
//! A finished World Wonder is a stage any band may play, so a rival holding
//! wonders we have seen, none of whose land the host offers any of our bands,
//! has closed that land to them.
//!
//! Live 20261110T164254Z (Kristina, Deity): from t148 to t153 the host offered
//! 20-23 stages in Ethiopia's land; from t154, the turn after our first two
//! concerts there, it offered none, though Ethiopia still held eight wonders,
//! and every walk into Ethiopia was refused for the rest of the game. Rome
//! (eleven wonders) dropped out of the list from t150 to t157 and t165 to t169
//! and came back between. The board, not knowing, planned every route to
//! Rome's stages through Ethiopia: 65 of 102 band-turns stood still.
//!
//! Only a band with at least one offered plot is evidence. A band the host has
//! not yet named offers none (`CivvisNameRockBands`), and a turn on which no
//! rival is open offers none to anyone; then nothing is inferred, and with no
//! stage on offer there is nowhere to route to anyway. A rival with a single
//! wonder plot is not judged, because `Plot:GetWonderType` names a wonder from
//! the turn it is placed and an unfinished one is no stage.
//!
//! Written fresh every frame into [`Game::rock_band_barred_hosts`], so a rival
//! that drops the policy is open again the next turn the host offers its land.

use super::Snapshot;
use crate::game::{Game, MIRRORED_SEAT};
use crate::hex::offset_to_axial;
use std::collections::{BTreeMap, BTreeSet};

/// Wonder plots a rival must hold before an empty offer reads as closed land.
const WONDERS_TO_JUDGE: usize = 2;

pub(super) fn apply(game: &mut Game, snapshot: &Snapshot) {
    game.rock_band_barred_hosts = barred_hosts(game, snapshot);
}

fn land_owner(game: &Game, pos: crate::Pos) -> Option<usize> {
    game.map
        .get(pos)
        .and_then(|tile| tile.owner_city)
        .and_then(|city| game.cities.get(&city))
        .map(|city| city.owner)
}

fn barred_hosts(game: &Game, snapshot: &Snapshot) -> BTreeSet<(usize, usize)> {
    let mut offered = BTreeSet::new();
    let mut read = false;
    for unit in game
        .units
        .values()
        .filter(|unit| unit.owner == MIRRORED_SEAT && unit.kind == "rock_band")
    {
        let Some(plots) = game
            .host_unit_facts
            .get(&unit.id)
            .and_then(|facts| facts.concert_plots.as_ref())
            .filter(|plots| !plots.is_empty())
        else {
            continue;
        };
        read = true;
        offered.extend(plots.iter().filter_map(|plot| land_owner(game, *plot)));
    }
    if !read {
        return BTreeSet::new();
    }
    let mut wonders: BTreeMap<usize, usize> = BTreeMap::new();
    for plot in snapshot.revealed.values().filter(|plot| plot.wo.is_some()) {
        if let Some(owner) = land_owner(game, offset_to_axial(plot.x, plot.y)) {
            *wonders.entry(owner).or_default() += 1;
        }
    }
    wonders
        .into_iter()
        .filter(|(host, count)| {
            let player = &game.players[*host];
            *host != MIRRORED_SEAT
                && *count >= WONDERS_TO_JUDGE
                && player.alive
                && !player.is_minor
                && !player.is_barbarian
                && !offered.contains(host)
        })
        .map(|(host, _)| (MIRRORED_SEAT, host))
        .collect()
}

#[cfg(test)]
mod tests;
