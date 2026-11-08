//! Native Art Museum inventory proposals. Firaxis checks move cooldowns and
//! the native bridge checks city-specific Tourism before issuing a swap.
use super::*;
use crate::mirror::{StateGreatWork, StateSnapshot};

fn regular_art(work: &StateGreatWork) -> bool {
    matches!(
        work.object.as_str(),
        "GREATWORKOBJECT_SCULPTURE" | "GREATWORKOBJECT_LANDSCAPE" | "GREATWORKOBJECT_PORTRAIT"
    )
}

fn score(slots: &[Option<&StateGreatWork>]) -> i32 {
    let works: Vec<_> = slots.iter().flatten().collect();
    let artists: BTreeSet<_> = works.iter().map(|w| &w.creator).collect();
    // GreatWorks.xml:329,334: Art Tourism is 2. Buildings.xml:469: nonunique art is 1,
    // full same-object/unique-artist museums double their Great Work Tourism.
    let base = works.len() as i32 + artists.len() as i32;
    if works.len() == 3 && artists.len() == 3 && works.iter().all(|w| w.object == works[0].object) {
        base * 2
    } else {
        base
    }
}

impl AdvancedAi {
    /// Ordered, strictly improving inventory proposals, not accepted moves.
    /// Native city multipliers and creation locks are read again by the host.
    /// Religious art has a separate Tourism channel and is left untouched.
    pub fn native_art_museum_swaps(
        &self,
        g: &Game,
        pid: usize,
        state: &StateSnapshot,
    ) -> Vec<(i64, String, i64, i32)> {
        if g.players.get(pid).is_none()
            || self.lane_lost
            || self.active_victory_target(g) != Some(VictoryTarget::Culture)
        {
            return Vec::new();
        }
        let mut museums = Vec::new();
        for city in &state.cities {
            if !city.buildings.iter().any(|b| b == "BUILDING_MUSEUM_ART") {
                continue;
            }
            let Some(works) = &city.great_works else {
                continue;
            };
            let mut slots = vec![None; 3];
            let mut valid = true;
            for work in works.iter().filter(|w| w.building == "BUILDING_MUSEUM_ART") {
                if !regular_art(work)
                    || work.creator.is_empty()
                    || work.kind.is_empty()
                    || !(0..3).contains(&work.slot)
                    || slots[work.slot as usize].is_some()
                {
                    valid = false;
                    break;
                }
                slots[work.slot as usize] = Some(work);
            }
            if valid && slots.iter().any(Option::is_some) {
                museums.push((city.id, slots));
            }
        }
        museums.sort_by_key(|m| m.0);
        let mut proposals = Vec::new();
        for a in 0..museums.len() {
            for b in a + 1..museums.len() {
                for i in 0..3 {
                    for j in 0..3 {
                        let mut left = museums[a].1.clone();
                        let mut right = museums[b].1.clone();
                        let before = score(&left) + score(&right);
                        let x = left[i];
                        let y = right[j];
                        std::mem::swap(&mut left[i], &mut right[j]);
                        let gain = score(&left) + score(&right) - before;
                        if gain <= 0 {
                            continue;
                        }
                        let (src, work, dst, slot) = if let Some(work) = x {
                            (museums[a].0, work, museums[b].0, j)
                        } else if let Some(work) = y {
                            (museums[b].0, work, museums[a].0, i)
                        } else {
                            continue;
                        };
                        proposals.push((gain, src, work.kind.clone(), dst, slot as i32));
                    }
                }
            }
        }
        proposals.sort_by(|a, b| {
            b.0.cmp(&a.0)
                .then_with(|| (&a.1, &a.2, &a.3, &a.4).cmp(&(&b.1, &b.2, &b.3, &b.4)))
        });
        proposals
            .into_iter()
            .take(16)
            .map(|(_, src, work, dst, slot)| (src, work, dst, slot))
            .collect()
    }
}

#[cfg(test)]
mod tests;
