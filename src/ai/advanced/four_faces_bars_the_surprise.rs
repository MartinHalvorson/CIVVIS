//! `four-faces-bars-the-surprise`: the peacetime surprise readers pass over a
//! rival that cannot declare a Surprise War on us.
//!
//! **The rule in Civilization VI** (shipped rows, Gathering Storm).
//! `DLC/Expansion2/Data/Expansion2_Civilizations_Major.xml:136-143` gives
//! Canada's `TRAIT_CIVILIZATION_FACES_OF_PEACE` the modifier
//! `TRAIT_NO_SUPRISE_WAR_FOR_CANADA` and every major
//! (`TRAIT_LEADER_MAJOR_CIV`) `TRAIT_NO_SUPRISE_WAR_ON_CANADA`; their
//! arguments (`:593-611`) ban `DIPLOACTION_DECLARE_SURPRISE_WAR`, for Canada
//! and against `CIVILIZATION_CANADA`. Every other declaration is a casus
//! belli, valid only in `DIPLO_STATE_DENOUNCED`
//! (`Base/Assets/Gameplay/Data/DiplomaticActions.xml:207-213`,
//! `DLC/Expansion1/Data/Expansion1_DiplomaticActions.xml:40-42`): the Formal
//! War and most others five turns into it (`DenouncementTurnsRequired="5"`,
//! `DiplomaticActions.xml:76`), the Protectorate and Golden Age wars at once
//! (`"0"`). So with Canada on either side, no war opens between two majors
//! that are not in the denounced state. The engine does not enforce this yet
//! (its `Action::DeclareWar` is legal for Canada), so the gene is written
//! for the live seat, where the host does.
//!
//! **Not established here.** A Joint War (`DIPLOACTION_JOINT_WAR`,
//! `DiplomaticActions.xml:77`) is an agreement two partners may sign in any
//! state from Unfriendly to Allied (`:144-199`), and the ban rows name only
//! the Surprise War. Whether the host lets a Joint War name Canada, or lets
//! Canada sign one, was not tested; if it does, that is a war this gene does
//! not see coming.
//!
//! **The finding (two live Deity games, 6 players, Continents, Online,
//! 2026-11-18).**
//! - `civvis-20261118T045850Z`, Kongo beside Canada, 189 turns: Canada never
//!   denounced us and no war was declared. The journal names Canada on 33
//!   `capital-archers-before-the-surprise` lines (t13-29), 2
//!   `capital-walls-before-the-surprise`, 6
//!   `frontier-walls-before-the-surprise` (t24-104) and 37
//!   `near-rival-deterrence` (t39-78), and 24 of the game's 26 deterrents
//!   were trained against it.
//! - `civvis-20261118T051842Z`, we are Canada, 166 turns: no rival denounced
//!   us and no war was declared. `near-rival-deterrence` trained 11 units
//!   against the Maya (t42-66) and `capital-walls-before-the-surprise` wrote
//!   8 lines (t28-47).
//!
//! **What the gene does.** A rival is spared while all of this holds: both of
//! us are majors, at peace, one of the two plays Canada, and neither holds an
//! active denouncement of the other (`BasicAi::four_faces_bars_surprise_from`).
//! A denouncement in either direction puts the pair in the denounced state
//! and ends the sparing that turn, as does a war. The readers that pass over
//! a spared rival:
//! - `near-rival-deterrence` with its `-opens-early` door and `-holds-late`
//!   phase (`deterrence_threat`);
//! - the capital watch that arms `capital-walls-before-the-surprise`,
//!   `capital-archers-before-the-surprise` and
//!   `capital-watch-arms-the-slinger` (`capital_watched_major_power`), and
//!   the walls gene's door leg, which buys or trains a defender on the
//!   soldiers in sight (`capital_door_threat`);
//! - `frontier-walls-before-the-surprise` (`frontier_walls_threat`);
//! - `surprise-watch-holds-its-ratio`'s latches;
//! - `garrison-before-the-surprise` and `capital-keeps-its-defender-3`
//!   (`BasicAi::surprise_soldiers`).
//!
//! **What it never touches.** A rival at war with us is never spared, so the
//! keeper behind Walls, `surprise-defense` and every wartime rule read as
//! before. Sightings are still recorded while a rival is spared: the turn a
//! denouncement lands, each watch reads its window as if the gene were off.
//!
//! Off: nothing changes. The gene writes no journal line of its own; the
//! readers' lines stop naming the spared rival.

use super::*;

impl AdvancedAi {
    /// Whether the peacetime surprise readers pass over `rival`: the gene is
    /// on and `rival` cannot open a war on `pid` by surprise. See the module
    /// and `BasicAi::four_faces_bars_surprise_from`. False with the gene off.
    pub(super) fn four_faces_spares(&self, g: &Game, pid: usize, rival: usize) -> bool {
        self.base.four_faces_bars_surprise_from(g, pid, rival)
    }
}

#[cfg(test)]
mod tests;
