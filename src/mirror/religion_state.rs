use super::*;

/// Native city IDs are player-local, so match the observed coordinates after
/// every retained city has been placed. Never carry a prior board ID forward.
pub(super) fn apply(ctx: &mut HostStepCtx<'_>) {
    ctx.game.players[0].holy_city = ctx
        .state
        .holy_city
        .and_then(|[x, y]| ctx.game.city_at(crate::hex::offset_to_axial(x, y)));
    if let Some(launched) = ctx.state.inquisition_launched {
        if launched {
            ctx.game.players[0].counters.insert("inquisition".into(), 1);
        } else {
            ctx.game.players[0].counters.remove("inquisition");
        }
    }
}

#[cfg(test)]
mod tests;
