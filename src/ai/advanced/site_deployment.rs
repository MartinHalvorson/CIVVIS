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
/// - `no-city-limit` (operator, 2026-11-11: "Remove the civvis city limit
///   ... Try to build 15-20 cities per game"): no city target of its own;
///   every major settles each seat it can hold while the city founded there
///   out-earns its Settler, several Settlers at a time. See
///   `advanced/no_city_limit.rs`.
///
/// It replaced `each-civ-draws-its-width`, the list's first entry (a drawn
/// width of 11 to 18), on the measurement that decided between them: all four
/// majors on one list, the stock site world, 8 seeds a list (2026-11-11).
///
/// | list | peak cities | majors past 15 | Science t150 | Production t150 | wall time |
/// |---|---|---|---|---|---|
/// | none (the shared ten) | 11.2 | 6% | par | par | par |
/// | the draw | 14.9 | 41% | +5% (se 5) | +10% (se 4) | x1.08 |
/// | `no-city-limit` | 18.3 | 81% | +13% (se 3) | +18% (se 3) | x1.19 |
/// | both | 19.3 | 84% | +9% (se 3) | +21% (se 4) | x1.31 |
///
/// The price is early: Science at turn 100 reads 22% (se 3) under the shared
/// ten's, against 3% for the draw, and is back to par near turn 130; Gold is
/// 9% (se 3) lower at turn 150. The draw on top of it only keeps the plan in
/// its expansion half longer, which costs Science and wall time, so it came
/// off the list and stays in the registry as an opt-in.
pub const SITE_EXPERIMENT_GENES: &[&str] = &["no-city-limit"];

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
