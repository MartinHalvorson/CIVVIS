//! Import revealed prerequisite facts without rerolling or revealing hidden nodes.

use super::{civvis_node_name, Name, StateSnapshot};
use crate::game::Game;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::Arc;

#[derive(Clone, Debug, Default, Deserialize)]
pub struct StateResearchTreeRow {
    #[serde(default)]
    pub t: String,
    /// Missing is unknown, while an explicit empty list is an observed root.
    #[serde(default)]
    pub requires: Option<Vec<String>>,
    /// An instantaneous host reading for native action eligibility. It does
    /// not constrain prerequisite searches after hypothetical research.
    #[serde(default)]
    pub can_research: Option<bool>,
}

pub(super) fn apply(game: &mut Game, state: &StateSnapshot, unmapped: &mut Vec<String>) {
    let observations = (|| {
        let mut observations = BTreeMap::new();
        let mut eligibility = BTreeMap::new();
        for row in state.research_tree.iter().flatten() {
            let node = civvis_node_name(&game.rules.techs, &row.t, "TECH_")
                .ok_or_else(|| format!("unknown_node:{}", row.t))?;
            let requires = row
                .requires
                .as_ref()
                .ok_or_else(|| format!("missing_requires:{}", row.t))?;
            let mut parents = Vec::new();
            for parent in requires {
                let name = civvis_node_name(&game.rules.techs, parent, "TECH_")
                    .ok_or_else(|| format!("unknown_prerequisite:{parent}"))?;
                parents.push(Name::new(&name));
            }
            if let Some(can) = row.can_research {
                eligibility.insert(Name::new(&node), can);
            }
            if observations.insert(Name::new(&node), parents).is_some() {
                return Err(format!("duplicate_node:{}", row.t));
            }
        }
        Ok((observations, eligibility))
    })();
    let result = match observations {
        Ok((observations, eligibility)) => {
            let result = game.set_host_research_prerequisites(observations);
            if result.is_ok() {
                game.host_research_eligibility = Arc::new(BTreeMap::from([(0, eligibility)]));
            }
            result
        }
        Err(error) => {
            // Invalid observations must not retain an earlier export's graph.
            game.set_host_research_prerequisites(BTreeMap::new())
                .unwrap();
            Err(error)
        }
    };
    if let Err(error) = result {
        let diagnostic = format!("research_tree:{error}");
        if !unmapped.contains(&diagnostic) {
            unmapped.push(diagnostic);
        }
    }
}

#[cfg(test)]
mod tests;
