//! `saved-work-is-seen` (opt-in): the host's saved work on paused units,
//! buildings and projects joins the board's saved-work bank.
//!
//! Civilization VI keeps a switched item's progress per city and item, and
//! the native engine models it (`City::production_progress`), so a native
//! board already resumes what it paused (`resume_city_production`) and prices
//! the remaining cost. The live mirror filled that bank for district
//! foundations only (`apply_host_district_progress`), because the menu export
//! carried `pr` for district rows alone. In 38 Emperor/Immortal games on
//! 2026-11-02 the host restored saved work on 382 of 383 resumed Buildings,
//! yet a median 17 paused units, buildings and projects a game were never
//! finished (switched away at a median 55% done), their saved work 4.9-7.9%
//! of all city production, and the resume path fired about once a game.
//!
//! The export now carries `pr` for every unit, building and project row and
//! the mirror keeps the positive readings in `Game::host_saved_work`. Under
//! the gene, before any production writer runs, each of our cities' readings
//! off its queue head overwrite the matching bank entries. A reading the host
//! omits or reports as zero leaves the engine's own entry alone. On a native
//! board the map is empty and the gene does nothing.
use super::*;

impl AdvancedAi {
    pub(super) fn see_host_saved_work(&self, g: &mut Game, pid: usize) {
        if !self.saved_work_is_seen || g.host_saved_work.is_empty() {
            return;
        }
        let saved = std::sync::Arc::clone(&g.host_saved_work);
        let mut seen = 0usize;
        for (cid, rows) in saved.iter() {
            let Some(city) = g.cities.get_mut(cid) else {
                continue;
            };
            if city.owner != pid {
                continue;
            }
            // The head's work is `City::production`; banking it again would
            // count it twice.
            let head = city.queue.first().map(Game::item_progress_key);
            for (key, progress) in rows {
                if head.as_deref() == Some(key.as_str())
                    || !progress.is_finite()
                    || *progress <= 0.0
                {
                    continue;
                }
                let slot = city.production_progress.entry(key.clone()).or_insert(0.0);
                if *slot != *progress {
                    *slot = *progress;
                    seen += 1;
                }
            }
        }
        if seen > 0 && self.journal().wants(crate::reasoning::Level::Detail) {
            think!(self.journal(), Economy, Detail,
                "Saved work is seen on {} paused builds", seen;
                "the host keeps a switched item's progress; the resume path can now finish it");
        }
    }
}

#[cfg(test)]
mod tests;
