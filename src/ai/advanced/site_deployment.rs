//! The player civvis.ai seats.
//!
//! Every major on the site plays the deployment genome on the authoritative
//! board ([`AdvancedAi::enable_native_deployment`]) plus the genes named in
//! [`SITE_EXPERIMENT_GENES`].
//!
//! ## Why the site has a list of its own
//!
//! An opt-in gene reaches the deployment genome only once a whole-game
//! screen has priced it into the ledger, and the site seated the ledger
//! genome and nothing else. So a behaviour the operator asked to *watch* never
//! reached the page they watch it on: between 2026-11-09 and 2026-11-11 three
//! sessions measured wider empires and filed opt-in genes for them, and
//! civvis.ai/test went on building ten cities a civ.
//!
//! This list is that route. A tag here is on for every site major and for
//! nothing else: `gene_screen`, the live Civ VI seat and every rated
//! instrument build their own agents from the ledger and never call
//! [`AdvancedAi::enable_site_deployment`]. The `/test` lane follows `main`,
//! so an entry plays there at the next publish; the front page takes it at
//! its next promotion.
//!
//! Add a tag when the operator asks for an experiment on the site. Remove it
//! when the ledger deploys the gene (the entry is then redundant) or the
//! experiment ends.
use super::*;

/// Opt-in genes civvis.ai's majors play on top of the deployment genome, by
/// registry tag.
///
/// - `each-civ-draws-its-width` (operator, 2026-11-11): every major draws a
///   city width of 11 to 18 for the game instead of stopping at the shared
///   ten. See `advanced/each_civ_draws_its_width.rs`.
pub const SITE_EXPERIMENT_GENES: &[&str] = &["each-civ-draws-its-width"];

impl AdvancedAi {
    /// The player civvis.ai seats (`Session::ai_fleet`): the deployment
    /// genome on the authoritative board, then every gene in
    /// [`SITE_EXPERIMENT_GENES`] in list order.
    pub fn enable_site_deployment(&mut self) {
        self.enable_native_deployment();
        for gene in site_experiment_genes() {
            (gene.enable)(self);
        }
    }
}

/// The registry rows [`SITE_EXPERIMENT_GENES`] names. A tag the registry
/// does not carry is a bug in the list, caught by this module's tests rather
/// than skipped on the page.
fn site_experiment_genes() -> impl Iterator<Item = &'static Gene> {
    SITE_EXPERIMENT_GENES.iter().filter_map(|tag| gene(tag))
}

#[cfg(test)]
mod tests;
