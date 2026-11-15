//! A museum swap waits out the art lock.
//!
//! Civilization VI will not move a Great Work of art for ten turns after it
//! is created (`GREATWORK_ART_LOCK_TIME`, Value 10,
//! Base/Assets/Gameplay/Data/GlobalParameters.xml:342; the shipped screen
//! reads it at GreatWorksOverview.lua:782-799), and the mod refuses such a
//! swap as `museum_art_locked`: either the work to be moved or the one in the
//! slot it would take is still inside its lock. The decider did not know the
//! turn a work was created, so it proposed the same swap every frame until
//! the lock ran out: 69 `museum_art_locked` refusals in live
//! civvis-20261115T123222Z, against one swap the host carried out.
//!
//! The state export now carries the turn each of a city's works was created
//! (`great_work_turns`, from `blds:GetTurnFromIndex`, the call the screen
//! makes), and a swap whose
//! work, or whose destination slot's work, is inside the lock is not sent. A
//! state without the turn (an older mod) holds nothing back.

use civvis::mirror::StateSnapshot;

/// `GREATWORK_ART_LOCK_TIME`.
const ART_LOCK_TURNS: i64 = 10;

/// Whether the host would refuse this swap for the art lock: `work` in the
/// Art Museum of city `source`, or what stands in `slot` of the Art Museum of
/// city `destination`, was created fewer than ten turns ago.
pub(super) fn locked(
    state: &StateSnapshot,
    source: i64,
    work: &str,
    destination: i64,
    slot: i32,
) -> bool {
    let city = |id: i64| state.cities.iter().find(|candidate| candidate.id == id);
    // Whether `kind`, housed in city `id`, is inside its lock.
    let inside = |id: i64, kind: &str| {
        city(id)
            .and_then(|found| found.great_work_turns.as_ref())
            .and_then(|turns| turns.get(kind))
            .is_some_and(|made| made + ART_LOCK_TURNS > i64::from(state.turn))
    };
    let in_the_slot = city(destination)
        .and_then(|found| found.great_works.as_deref())
        .and_then(|works| {
            works
                .iter()
                .find(|held| held.building == "BUILDING_MUSEUM_ART" && held.slot == slot)
        });
    inside(source, work) || in_the_slot.is_some_and(|held| inside(destination, &held.kind))
}

#[cfg(test)]
#[path = "locked_art/tests.rs"]
mod tests;
