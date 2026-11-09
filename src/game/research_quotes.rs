//! Research eligibility, prerequisites and remaining cost as the live host
//! quotes them (`TechTree.lua`), used instead of the model's estimate when
//! observed.

use super::*;

/// Native TechTree.lua:1112/1114 quotes, already adjusted by the host. Progress
/// includes earned boosts and partial research even for a non-current node.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HostResearchQuote {
    #[serde(default)]
    pub cost: Option<f64>,
    #[serde(default)]
    pub progress: Option<f64>,
}

impl HostResearchQuote {
    pub(crate) fn remaining(self) -> Option<f64> {
        let (cost, progress) = (self.cost?, self.progress?);
        (cost.is_finite() && progress.is_finite() && cost >= 0.0 && progress >= 0.0)
            .then(|| (cost - progress).max(0.0))
    }
}

impl Game {
    /// Whether this frame observed the complete prerequisite list, including
    /// lists that happen to agree with our generated layout.
    pub fn host_research_prerequisites_revealed(&self, technology: Name) -> bool {
        self.host_revealed_research.contains(&technology)
    }

    /// Current host eligibility, distinct from hypothetical prerequisites.
    /// Missing observations stay unknown rather than becoming a refusal.
    pub fn host_can_research(&self, pid: usize, technology: Name) -> Option<bool> {
        self.host_research_eligibility
            .get(&pid)?
            .get(&technology)
            .copied()
    }

    /// Switching the host's research path banks the previous node's work.
    /// TechTree.lua:238-245 sends VALUE_EXCLUSIVE while another node is active;
    /// :1114 reads saved progress for every node. Use that measured bank, not a
    /// fresh eureka or the previous node's progress. Ordinary games retain their
    /// existing action contract until per-node research banks are modeled there.
    pub(crate) fn switch_host_research(
        &mut self,
        pid: usize,
        technology: &str,
    ) -> Result<(), String> {
        let current = self.players[pid]
            .research
            .as_deref()
            .ok_or("no current research")?;
        if current == technology {
            return Err("already researching this technology".into());
        }
        let quotes = self
            .host_research_quotes
            .get(&pid)
            .ok_or("already researching")?;
        let old = quotes
            .get(&Name::new(current))
            .filter(|quote| quote.remaining().is_some())
            .ok_or("no observed current research bank")?;
        let new = quotes
            .get(&Name::new(technology))
            .filter(|quote| quote.remaining().is_some())
            .ok_or("no observed destination research bank")?;
        let progress = self.players[pid].research_progress;
        if !progress.is_finite() || progress < 0.0 {
            return Err("invalid current research progress".into());
        }
        if !self
            .available_techs(pid)
            .iter()
            .any(|name| name == technology)
        {
            return Err("tech unavailable".into());
        }
        let current = Name::new(current);
        let saved = HostResearchQuote {
            cost: old.cost,
            progress: Some(progress),
        };
        let restored = new.progress.unwrap();
        Arc::make_mut(&mut self.host_research_quotes)
            .get_mut(&pid)
            .unwrap()
            .insert(current, saved);
        self.players[pid].research = Some(technology.to_string());
        self.players[pid].research_progress = restored;
        Ok(())
    }

    /// Remaining beakers in the observed host snapshot, without another speed
    /// or boost discount. Missing/invalid quotes leave approximation to callers.
    pub fn host_remaining_research_cost(&self, pid: usize, technology: Name) -> Option<f64> {
        self.host_research_quotes
            .get(&pid)?
            .get(&technology)?
            .remaining()
    }
}

#[cfg(test)]
mod tests;
