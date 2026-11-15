//! Fog the host refused a move onto because entering it would start a war.
//!
//! Before a move goes out the control mod asks the host whether it would
//! start a war the seat never declared (`CombatManager.IsAttackChangeWarState`)
//! and, when the answer names a player, refuses the move and logs
//! `war_refused` with the unit, the verb, the plot and the players. For a
//! plain move the usual case is a plot inside a rival's closed border.
//!
//! The mirror cannot know that about ground the seat has not revealed: it
//! assumes unrevealed plots near the frontier are open so exploration can aim
//! outward. So the same step was planned again. Live game
//! `civvis-20261115T071444Z` (Pericles, Deity) counted 36 orders refused as
//! `throw` over its two sittings, which is how this refusal was counted until
//! the mod could name it (the guard raised while reading the host's answer;
//! the next game's `order_threw` event named the line). One Scout was sent to
//! (14,14) on turns 65 and 66, to (26,15) on turns 89 and 90 and to (22,18)
//! on turns 95, 97 and 98, each of them unrevealed when ordered and inside a
//! rival's border once seen, and it ended six of those seven turns on the
//! plot it began on. The third identical failure strikes an order out for
//! ten turns (`ORDER_REFUSAL_COOLDOWN_TURNS`).
//!
//! This reads those refusals from the run's own log, as `SpyLossLedger` reads
//! losses, and for the same ten turns stops assuming the refused plot is open
//! while it is still unrevealed, the way `HostMoveRefusals::apply` treats
//! ground a move never started across. One refusal is enough: the host has
//! named the reason. Revealed ground is never touched, because its owner and
//! the border rules then answer for it, and after ten turns the plot is
//! assumed open again, so a border that opened is retried.
use super::host_refusals::ORDER_REFUSAL_COOLDOWN_TURNS;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// Plots a `MOVE_TO` was refused onto as a war starter, in host offset
/// coordinates, with every turn each was refused on. Every turn is kept, not
/// the latest, so a replay reading a log that runs past its own turn answers
/// as the live game did.
#[derive(Default)]
pub(super) struct Ledger {
    offset: u64,
    pub(super) refused: BTreeMap<(i32, i32), BTreeSet<u32>>,
}

impl Ledger {
    /// Read the whole lines the log has gained since the last call. A shorter
    /// file is a new run and starts the ledger over.
    pub(super) fn refresh(&mut self, events: &Path) {
        use std::io::{BufRead, Seek, SeekFrom};
        let Ok(mut file) = std::fs::File::open(events) else {
            return;
        };
        let length = file.metadata().map_or(0, |meta| meta.len());
        if length < self.offset {
            *self = Self::default();
        }
        if file.seek(SeekFrom::Start(self.offset)).is_err() {
            return;
        }
        let mut reader = std::io::BufReader::new(file);
        let mut line = Vec::new();
        loop {
            line.clear();
            let Ok(read) = reader.read_until(b'\n', &mut line) else {
                return;
            };
            if read == 0 || line.last() != Some(&b'\n') {
                return;
            }
            self.offset += read as u64;
            if let Some((plot, turn)) = Self::refused_move(&line) {
                self.refused.entry(plot).or_default().insert(turn);
            }
        }
    }

    /// The plot and turn of a `war_refused` line for a plain move, else
    /// `None`. A refused strike is the mirror's (`refused_strikes_on`), and
    /// the `war_refused` a declaration raises carries no plot.
    fn refused_move(line: &[u8]) -> Option<((i32, i32), u32)> {
        let has = |needle: &[u8]| line.windows(needle.len()).any(|window| window == needle);
        if !has(b"war_refused") || !has(b"MOVE_TO") {
            return None;
        }
        let event: serde_json::Value = serde_json::from_slice(line).ok()?;
        if event.get("kind")?.as_str()? != "war_refused"
            || event.get("verb")?.as_str()? != "MOVE_TO"
        {
            return None;
        }
        let whole = |key: &str| {
            event
                .get(key)
                .and_then(serde_json::Value::as_i64)
                .and_then(|value| i32::try_from(value).ok())
        };
        let turn = u32::try_from(event.get("turn")?.as_u64()?).ok()?;
        Some(((whole("x")?, whole("y")?), turn))
    }

    /// Whether one of these refusals still closes its plot on `turn`. A
    /// refusal from later than `turn` is a replay reading a log that runs on.
    fn holds(refused: &BTreeSet<u32>, turn: u32) -> bool {
        refused
            .iter()
            .any(|&on| on <= turn && turn - on < ORDER_REFUSAL_COOLDOWN_TURNS)
    }

    /// How many plots are closed on `turn`.
    pub(super) fn closed_on(&self, turn: u32) -> usize {
        self.refused
            .values()
            .filter(|refused| Self::holds(refused, turn))
            .count()
    }

    /// On a board built for `turn`, stop assuming the refused plots are open
    /// while they are unrevealed.
    pub(super) fn apply(&self, mirror: &mut civvis::mirror::LiveMirror, turn: u32) {
        for (&(x, y), refused) in &self.refused {
            if !Self::holds(refused, turn) {
                continue;
            }
            let pos = civvis::hex::offset_to_axial(x, y);
            if let Some(tile) = mirror.game.map.tiles.get_mut(&pos) {
                if tile.terrain == "unknown" {
                    tile.assumed_traversable = false;
                    tile.assumed_navigable = false;
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "closed_ground/tests.rs"]
mod tests;
