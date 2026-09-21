use super::*;

/// Reconcile gross income, not net stockpile change: existing unit/power demand
/// is charged by its own consumers. The host seat is model player zero.
pub(super) fn apply(
    game: &mut crate::game::Game,
    state: &StateSnapshot,
    unmapped: &mut Vec<String>,
) {
    Arc::make_mut(&mut game.observed_strategic_income_adjustments).remove(&0);
    if game.players.is_empty() {
        return;
    }
    let Some(income) = state.strategic_resource_income.as_ref() else {
        return;
    };
    let mut adjustments = BTreeMap::new();
    for (host, amount) in income {
        let name = Name::new(
            &host
                .strip_prefix("RESOURCE_")
                .unwrap_or(host)
                .to_ascii_lowercase(),
        );
        if !game
            .rules
            .resources
            .get(&name)
            .is_some_and(|resource| resource.class == "strategic")
            || !amount.is_finite()
            || *amount < 0.0
        {
            let issue = format!("strategic_resource_income:{host}");
            if !unmapped.contains(&issue) {
                unmapped.push(issue);
            }
            continue;
        }
        adjustments.insert(
            name,
            amount - game.strategic_resource_rate(0, name.as_str()),
        );
    }
    Arc::make_mut(&mut game.observed_strategic_income_adjustments).insert(0, adjustments);
}

#[cfg(test)]
mod tests;
