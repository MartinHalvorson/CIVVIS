//! Default-off, single forward-field preparation project.
use super::*;

#[derive(Clone)]
pub(super) struct FieldProject {
    pub city: u32,
    pub capital: u32,
    pub field: Pos,
    pub started: u32,
}

impl AdvancedAi {
    pub(super) fn claim_forward_airfield_project(
        &mut self,
        _g: &mut Game,
        _pid: usize,
        _plan: &StrategicPlan,
    ) {
    }

    pub(super) fn forward_airfield_project_holds(
        &self,
        _g: &Game,
        _pid: usize,
        _cid: u32,
        _plan: &StrategicPlan,
        _item: &Item,
    ) -> bool {
        false
    }

    pub(super) fn forward_airfield_engineer_step(
        &mut self,
        _g: &mut Game,
        _pid: usize,
        _uid: u32,
        _plan: &StrategicPlan,
    ) -> Option<bool> {
        None
    }
}

#[cfg(test)]
mod tests;
