//! Same-turn custody of a maneuver across a required native observation.
//! Store identity, never actions or predicted damage/ownership.
use super::Order;
use civvis::ai::{AdvancedAi, AirCityAssault};
use civvis::game::Game;
use civvis::mirror::StateSnapshot;

#[derive(Clone, Debug)]
struct Pending {
    turn: u32,
    frame: u32,
    target: (i32, i32),
    owner: usize,
}

#[derive(Default)]
pub(super) struct Continuation {
    pending: Option<Pending>,
}

impl Continuation {
    /// Remember only a barrier actually emitted by this batch. No speculative
    /// captured city, sortie result, move path or model unit ID survives.
    pub(super) fn record(
        &mut self,
        state: &StateSnapshot,
        assault: Option<&AirCityAssault>,
        orders: &[Order],
    ) {
        self.pending = None;
        if !orders
            .iter()
            .any(|order| order.kind == "observe" && order.verb.as_deref() == Some("AIR_ASSAULT"))
        {
            return;
        }
        let Some(assault) = assault else {
            return;
        };
        let target = civvis::hex::axial_to_offset(assault.target.0, assault.target.1);
        let Some(owner) = state.rivals.iter().find(|rival| {
            rival.at_war && rival.cities.iter().any(|city| (city.x, city.y) == target)
        }) else {
            return;
        };
        self.pending = Some(Pending {
            turn: state.turn,
            frame: state.frame,
            target,
            owner: owner.player,
        });
    }

    fn take_target(&mut self, state: &StateSnapshot) -> Option<civvis::Pos> {
        let pending = self.pending.take()?;
        if state.turn != pending.turn {
            return None;
        }
        if state.frame <= pending.frame {
            self.pending = Some(pending);
            return None;
        }
        if state
            .cities
            .iter()
            .any(|city| (city.x, city.y) == pending.target)
            || state.rivals.iter().any(|rival| {
                rival.player != pending.owner
                    && rival
                        .cities
                        .iter()
                        .any(|city| (city.x, city.y) == pending.target)
            })
        {
            return None;
        }
        let rival = state
            .rivals
            .iter()
            .find(|rival| rival.player == pending.owner)?;
        if !rival.at_war
            || !rival
                .cities
                .iter()
                .any(|city| (city.x, city.y) == pending.target)
        {
            return None;
        }
        Some(civvis::hex::offset_to_axial(
            pending.target.0,
            pending.target.1,
        ))
    }

    /// Keep the shared immediate-kill prepass and its civilian guards. Only a
    /// viable continuation substitutes for this frame's general deliberation;
    /// a changed board falls back to the same production planner immediately.
    pub(super) fn plan_frame(
        &mut self,
        ai: &mut AdvancedAi,
        game: &mut Game,
        state: &StateSnapshot,
        mapped: &std::collections::BTreeMap<u32, i64>,
    ) -> (civvis::ai::finishing::WarFinishingVolley, usize, bool) {
        let Some(target) = self.take_target(state) else {
            let (finishing, begin) = civvis::ai::player::plan_frame(ai, game, 0, mapped);
            return (finishing, begin, false);
        };
        civvis::ai::player::native_air_assault::plan_frame(ai, game, 0, mapped, target)
    }
}

#[cfg(test)]
#[path = "air_assault_continuation/tests.rs"]
mod tests;
