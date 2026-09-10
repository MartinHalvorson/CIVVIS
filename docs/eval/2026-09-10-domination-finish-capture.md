# Only a terminal domination capture can waive occupation safety

The capture planner skips its rapid-loyalty-flip safeguard when a capture
would complete Domination. Its former check ignored the player's own original
capital and treated every missing rival capital as already satisfied. It also
ignored the victory checkboxes and the Require-N victory setting. Consequently
a last foreign capital could be treated as a winning capture while our own
capital was still enemy-held, or when Domination could not end the game at all.

The replacement predicate follows `Game::check_domination` and
`Game::set_winner`: every required original capital includes our own; a living
unfounded opponent remains unresolved; Domination must be an effective enabled
lane; and its new milestone must satisfy the remaining victory-type count.
An already banked Domination type cannot be counted twice. Team games use the
engine's separate rule that each member retains its own capital and every
opponent has lost theirs; Require-N uses the first qualifying team member,
as the engine does.
Its domination check returns after banking that member's milestone, even if
a later teammate already has another victory type banked. Finished and
played-on games do not borrow a terminal shortcut.

The ordinary loyalty threshold, support-city choices, liberation and last-city
exceptions remain in place. A real final capture can still win immediately
even when the city would be unholdable next turn. A nonterminal capture must
pass the existing occupation check.

Tests compare the AI predicate with actual `Attack` then `KeepCity` actions,
which run the engine's own victory check. They cover the true last capital,
our lost capital, disabled Domination, Require-N banked and unbanked cases,
missing living/dead opponents, and team capital ownership. After merging
current main, the full Rust suite passed 3,585 tests (52 ignored), including
all six capture regressions. Clippy produced no diagnostics; formatting and
`git diff --check` passed. This changes AI capture judgment, not engine rules;
actual engine capture transitions provide the targeted semantic check.
