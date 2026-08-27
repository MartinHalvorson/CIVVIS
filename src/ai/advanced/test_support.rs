//! Shared `#[cfg(test)]` helpers for the gene submodules' own `tests` blocks.
//!
//! Nine gene modules each carried a private, byte-identical copy of
//! `opt_in_off_in_both_controllers` (`coalition.rs`, `enemy_of_my_enemy.rs`,
//! `boost_research.rs`, `field_craft.rs`, `chokepoints.rs`,
//! `city_state_quests.rs`, `deity_habits.rs`, `recon_disruption.rs`, and a
//! divergent ninth in `city_campaign.rs`). This is the one copy for the
//! seven that were identical; the two with their own bespoke assertions kept
//! theirs rather than silently changing what they check.

use super::{genes::GENES, AdvancedAi};

/// An opt-in gene must be off in both `AdvancedAi::new()` and
/// `AdvancedAi::legacy()`, published as an opt-in, screenable, non-live row,
/// and its own `enable`/`disable` pair must actually move the field `read`
/// reports.
pub(crate) fn opt_in_off_in_both_controllers(tag: &str, read: fn(&AdvancedAi) -> bool) {
    assert!(!read(&AdvancedAi::new()), "{tag} must be off in new()");
    assert!(
        !read(&AdvancedAi::legacy()),
        "{tag} must be off in legacy()"
    );
    let gene = GENES
        .iter()
        .find(|gene| gene.tag == tag)
        .expect("the gene is published for gene_screen");
    assert!(gene.opt_in() && gene.screenable() && !gene.live());
    let mut ai = AdvancedAi::new();
    (gene.enable)(&mut ai);
    assert!(read(&ai));
    (gene.disable)(&mut ai);
    assert!(!read(&ai));
}
