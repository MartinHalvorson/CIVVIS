//! A bounded native continuation with the shared player prepass and fallback.
use super::{aid, AdvancedAi, Game};
use crate::ai::finishing::{begin_player_turn, WarFinishingVolley};

pub fn plan_frame(
    ai: &mut AdvancedAi,
    view: &mut Game,
    pid: usize,
    mapped: &std::collections::BTreeMap<u32, i64>,
    target: crate::Pos,
) -> (WarFinishingVolley, usize, bool) {
    let finishing = begin_player_turn(ai, view, pid, mapped);
    let begin = view.log.len();
    let resumed = ai.resume_observed_air_city_assault(view, pid, target);
    if !resumed {
        aid::plan_native(view, pid);
        ai.plan_observed_turn(view, pid);
    }
    (finishing, begin, resumed)
}
