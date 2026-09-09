use std::collections::BTreeMap;

/// A host combat result, keyed by both player and stable unit id.
#[derive(Clone, Debug, PartialEq)]
pub struct HostUnitDeath {
    pub player: usize,
    pub unit: i64,
    pub turn: u32,
}

#[derive(Default)]
pub(super) struct HostDeaths(BTreeMap<(usize, i64), HostUnitDeath>);

impl HostDeaths {
    pub(super) fn observe(&mut self, line: &str) {
        if !line.contains("\"combat\"") {
            return;
        }
        let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else {
            return;
        };
        if event["kind"].as_str() != Some("combat") {
            return;
        }
        let Some(turn) = event["turn"].as_u64().and_then(|t| u32::try_from(t).ok()) else {
            return;
        };
        for (side, killed) in [
            ("attacker", "attacker_killed"),
            ("defender", "defender_killed"),
        ] {
            let participant = &event[side];
            if event[killed].as_bool() != Some(true) || participant["type"].as_str() != Some("unit")
            {
                continue;
            }
            let (Some(player), Some(unit)) = (
                participant["player"]
                    .as_u64()
                    .and_then(|p| usize::try_from(p).ok()),
                participant["id"].as_i64().filter(|id| *id >= 0),
            ) else {
                continue;
            };
            self.0
                .insert((player, unit), HostUnitDeath { player, unit, turn });
        }
    }

    /// Called at the selected state line: later combat results cannot leak
    /// backwards into an earlier turn or an earlier frame of the same turn.
    pub(super) fn through(&self, turn: u32) -> Vec<HostUnitDeath> {
        self.0
            .values()
            .filter(|death| death.turn <= turn)
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests;
