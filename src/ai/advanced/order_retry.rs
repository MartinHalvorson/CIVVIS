//! `order-retry`: when an order the planner already decided on is refused,
//! spend the same turn on the next-best candidate instead of losing the turn.
//!
//! ## The defect
//!
//! `Game::apply` returns `Result<(), String>`, and across
//! `src/ai/advanced.rs` and `src/ai.rs` roughly **137 call sites discard or
//! collapse that error** — 46 of them literally `let _ = g.apply(…)`. Of the
//! ~17 sites in the advanced controller that do branch on `is_err()`, every
//! one `break`s or returns; **not one tries an alternative.** A refused order
//! is not a decision the planner revisits, it is a turn it silently loses.
//!
//! The rate is measured. `docs/AI_GAPS.md` (12 paired games, 2,371 turns,
//! 75,483 planned orders) records 2,301 refusals, and **896 of 2,371 turns —
//! 37.8% — carry at least one**. By kind: `trade` 67.3%, `produce` **24.9%**
//! (619 of 2,490), `levy_military` 19.4%, `found_city` 14.3%, `move` 2.8%.
//! The same document records that the seat's *judgement* is intact at matched
//! states; what it loses, it loses by not landing orders.
//!
//! ## Why this shape, and not the one that was already tried
//!
//! ⚠⚠ `fog-honest-2` attempted the ambitious repair — stop the tape at the
//! first refusal, re-cross the fog boundary and re-plan the rest of the turn
//! — and it **lost**: −5.7 pp wins (z −2.25), −2.44 pp share (z −3.10).
//! `docs/AI_GAPS.md` names the reason and the version worth building: to
//! reach the re-plan it must stop the tape, which "treats the tape as one
//! dependent chain … a refused Settler move says nothing about a Builder's
//! `improve` twelve actions later", and what is wanted instead is to
//! **"skip only the refused actor's remaining actions and keep everyone
//! else's, which needs no early `EndTurn` and no re-plan at all."**
//!
//! This gene is that shape, taken natively rather than on the live tape:
//! nothing is re-planned, no turn is ended early, and one decision's refusal
//! costs only that decision. The next candidate is the one the planner had
//! already scored and ranked, so a retry spends no new valuation.
//!
//! ## Where it acts
//!
//! Only the three places a ranked list of alternatives **already exists**, so
//! the next-best is free:
//!
//! 1. **The city production governor.** `advanced_production` scores every
//!    producible item and applies the argmax; on refusal the city keeps an
//!    empty queue for the turn. This is the biggest share of the refusals
//!    above — 24.9% of `produce` orders.
//! 2. **The gold purchase loop.** It builds a ranked `candidates` vector and
//!    then `if g.apply(…).is_err() { break }` — **one refusal abandons the
//!    entire remaining purchase budget for the turn**, not merely the item.
//! 3. **The builder's improvement.** `worthwhile_improvements` returns a
//!    ranked vector and only `here.first()` is ever attempted.
//!
//! Each is bounded by [`ORDER_RETRY_LIMIT`] alternatives, so a city with a
//! long menu cannot spend the turn walking it, and a refusal that is really a
//! rules-wide veto costs a handful of cheap `can_produce` checks rather than
//! sixty.
//!
//! Byte-identical when off: every site takes the same first candidate it
//! takes today and stops the moment the gene is not on.

use super::AdvancedAi;

/// How many *alternatives* a refused decision may try before giving the turn
/// up. The first attempt is not a retry, so a budget of two means at most
/// three `apply` calls for one decision.
pub const ORDER_RETRY_LIMIT: usize = 2;

impl AdvancedAi {
    /// How many alternatives a refused order may fall through to. Zero while
    /// the gene is off, which is what makes every call site an exact no-op.
    pub(super) fn order_retry_budget(&self) -> usize {
        if self.order_retry {
            ORDER_RETRY_LIMIT
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn off_by_default_and_toggles() {
        let ai = AdvancedAi::new();
        assert!(!ai.order_retry, "an opt-in ships off");
        assert!(!AdvancedAi::legacy().order_retry);
        assert_eq!(ai.order_retry_budget(), 0, "off is a zero budget");
        let mut ai = AdvancedAi::new();
        ai.enable_order_retry();
        assert_eq!(ai.order_retry_budget(), ORDER_RETRY_LIMIT);
        ai.disable_order_retry();
        assert_eq!(ai.order_retry_budget(), 0);
    }
}
