//! `stage-reads-the-muster-breakers`: under `stage-musters-out-of-reach`,
//! once a siege's muster has closed (`stage_muster_ready`), the Stage step
//! reads the breakers it holds out to [`MUSTER_FAR`] instead of
//! [`STAGING_FAR`]. `no_breaker`, `opens_walls` and the Stage arm then agree
//! with the reading that closed the muster, so the train invests and the
//! guns take their firing posts.
//!
//! **Why (live Emperor, the 42 pin-era games of 2026-10-18).** The muster
//! closes on a breach read within [`MUSTER_BREACH_FAR`], but Stage reads
//! one within [`STAGING_FAR`], and staging guns stop at their danger line
//! six to eight tiles out. Of 1,271 walls-up "0 gun(s) fit" siege turns, the
//! nearest healthy gun stood at exactly six tiles on 202 and at seven to
//! twelve on 488; on Stage turns with the muster already closed it stood
//! within eight tiles on 140 (57 sieges, a median of three turns stuck; 27
//! ever invested). Madurai (civvis-20261018T090059Z, the domination win) held
//! in Stage from turn 166 to 181 with two or three healthy guns six to seven
//! tiles out; Patna (092536Z) held from 157 with four Bombards six to eight
//! tiles out and never invested before the Technology loss at 191.
//!
//! Off, the Stage step reads [`STAGING_FAR`] as before.

use super::*;

impl AdvancedAi {
    /// The radius the Stage step reads breakers within for `cid`: see the
    /// module. [`STAGING_FAR`] with the gene off.
    pub(in crate::ai::advanced) fn stage_breach_radius(&self, cid: u32) -> i32 {
        if self.stage_reads_the_muster_breakers
            && self.stage_musters_out_of_reach
            && self.stage_muster_ready.get(&cid).copied().unwrap_or(false)
        {
            MUSTER_FAR
        } else {
            STAGING_FAR
        }
    }
}
