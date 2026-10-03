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
