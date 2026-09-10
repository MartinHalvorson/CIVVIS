//! A single finish-line Aid Request policy; adapters supply observed facts.
use crate::game::{Action, DealItems, Game, Item};

pub const FINISH_WINDOW: i64 = 3;
const GOLD_RESERVE: f64 = 100.0;
const SCORE_MAX: f64 = 200.0;

pub struct Opportunity<'a> {
    pub kind: &'a str,
    pub target: i64,
    pub turns_left: i64,
    pub begun: bool,
    pub member: bool,
    pub ours: Option<f64>,
    pub leader: Option<f64>,
    pub peaceful_met_target: bool,
    pub project_finishes: bool,
}

pub fn is_aid(kind: &str) -> bool {
    matches!(
        kind.trim(),
        "EMERGENCY_SEND_AID" | "EMERGENCY_SEND_MILITARY_AID"
    )
}

/// Preserve 100 Gold, spend at most one Send Aid project's score, and choose
/// the earliest deadline, cheapest winning gap, then recipient deterministically.
/// Missing/non-finite scoreboard facts are not a zero-score rival.
pub fn choose<'a, 'b>(
    gold: f64,
    opportunities: &'a [Opportunity<'b>],
) -> Option<(&'a Opportunity<'b>, i64)> {
    if !gold.is_finite() {
        return None;
    }
    opportunities
        .iter()
        .filter_map(|op| {
            if !is_aid(op.kind)
                || !op.begun
                || !op.member
                || op.target < 0
                || !(0..=FINISH_WINDOW).contains(&op.turns_left)
                || !op.peaceful_met_target
                || op.project_finishes
            {
                return None;
            }
            let ours = op.ours.filter(|v| v.is_finite() && *v >= 0.0)?;
            let leader = op.leader.filter(|v| v.is_finite() && *v >= 0.0)?;
            if ours > leader {
                return None;
            }
            let amount = (leader - ours).floor() + 1.0;
            (amount <= SCORE_MAX && amount <= gold - GOLD_RESERVE).then_some((op, amount as i64))
        })
        .min_by_key(|(op, amount)| (op.turns_left, *amount, op.target))
}

/// Native facts use the same public tracker and own-city production estimate
/// represented by the live export. No opponent production or treasury is read.
pub(super) fn plan_native(game: &mut Game, pid: usize) {
    let Some(running) = game.competition.as_ref() else {
        return;
    };
    let Some(target) = running.target else {
        return;
    };
    if running.ends <= game.turn {
        return;
    }
    let turns_left = i64::from(running.ends - game.turn);
    let project_finishes = game.player_city_ids(pid).into_iter().any(|cid| {
        let Some(item @ Item::Project { project }) = game.cities[&cid].queue.first() else {
            return false;
        };
        if project != "send_aid" {
            return false;
        }
        let production =
            game.city_yields(cid).production * game.item_prod_mult(pid, cid, Some(item));
        let remaining = game.item_remaining_cost_for_city(pid, cid, item);
        production.is_finite()
            && production > 0.0
            && remaining.is_finite()
            && (remaining / production).ceil() <= turns_left as f64
    });
    let opportunities = [Opportunity {
        kind: &running.kind,
        target: target as i64,
        turns_left,
        begun: true,
        member: target != pid
            && game.players[pid].alive
            && !game.players[pid].is_minor
            && !game.players[pid].is_barbarian,
        ours: Some(running.scores.get(&pid).copied().unwrap_or(0.0)),
        leader: Some(
            running
                .scores
                .iter()
                .filter(|(seat, _)| **seat != pid)
                .map(|(_, score)| *score)
                .fold(0.0, f64::max),
        ),
        peaceful_met_target: game
            .players
            .get(target)
            .is_some_and(|p| p.alive && !p.is_minor && !p.is_barbarian)
            && game.has_met(pid, target)
            && !game.is_at_war(pid, target),
        project_finishes,
    }];
    let Some((_, amount)) = choose(game.players[pid].gold, &opportunities) else {
        return;
    };
    // Log an ordinary action, not a direct treasury mutation. Native execution
    // and live transport own acceptance and the resulting competition score.
    let _ = game.apply(
        pid,
        &Action::Trade {
            player: target,
            offer: Box::new(DealItems {
                gold: amount as f64,
                ..Default::default()
            }),
            request: Box::default(),
        },
    );
}

#[cfg(test)]
mod tests;
