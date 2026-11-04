//! `trade-quotes-pay-the-lump`: a seat's quick-deal quotes pay one
//! rounded-up Gold lump instead of a floored lump plus a sliver per turn.
//!
//! `Game::quoted_payment` floors the lump and pays the remaining fraction as
//! 0.1 Gold a turn, and every quote then has to clear `items_are_valid`'s
//! `gold_per_turn <= empire_gold_per_turn` — a liquidity proxy of 4 Gold a
//! city plus a hundredth of the treasury, less unit and building upkeep,
//! floored at zero. A native Immortal culture census (4p pangaea, three
//! handicapped rival chairs, 8 games) read that proxy as exactly zero from
//! turn 100 in every game while the seat's real income was 30–200 Gold a
//! turn, and its luxury, strategic and Great Work quotes fell from 1–5 a mark
//! to none: a built-up empire stops trading for the rest of the game.
//!
//! With the gene on, the seat names itself `Game::lump_quote_viewer` each
//! turn, and a quote it asks for is paid as the midpoint rounded up when the
//! payer's treasury covers that above the same reserve `quoted_payment`
//! keeps; otherwise the ordinary quote stands. Both sides are still valued
//! and validated by the engine, and the authoritative game revalidates every
//! harvested trade. Off, `lump_quote_viewer` is `None` and nothing changes.

#[cfg(test)]
mod tests;
