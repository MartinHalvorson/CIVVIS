use super::*;

/// City IDs are host identities here; never retain a board ID from an earlier
/// fresh observation. Unknown fields on old hosts are not evidence of success.
pub(super) fn apply(ctx: &mut HostStepCtx<'_>) {
    ctx.game.players[0].holy_city = ctx
        .state
        .holy_city_id
        .and_then(|host| ctx.known_city_ids.get(&host).copied());
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
