"use strict";
// The simulator debugger's bar.
//
// A played game keeps a mark of every turn its seat has opened (the engine's
// `server/debugger.rs`), and every state it sends says where the world sits
// on that timeline (`state.debug`). This script is the hand on it: a header
// box at the top of the window, beside the left panel. The whole game is one
// long timeline across the box: turn 1 at its left end, the last turn under a
// chequered flag at its right, the first turn of each era named on it, and a
// marker that moves right as turns open. A turn that was played is gone back
// to as it stood, and a turn still ahead is played to by the seat's Civ AI.
// That Civ AI is picked above the timeline, and each stretch of the turns
// played is named on the timeline by the Civ AI that played it. Under the
// marker, wherever it stands, is Branch. Under the timeline the open turn
// stands in the middle with the turn buttons either side of it (Turn 1, Era
// Back, ten back, one back; one on, ten on, Era Next, Play to End).
// Auto-play, layers and breakpoints are at the left, and stepping in, undo,
// save points, reveal and the Single player switch at the right.
//
// A game opens on one timeline, with the top-rated Civ AI in its seat.
// Branch asks which Civ AI plays a second timeline from the open turn, and
// when one is picked splits the screen in two, side by side, at once: A is
// the timeline as it was, with its own Civ AI and the turns it
// holds past that turn, and B is the branch, which begins there. The two
// begin locked: they stay on the same turn, and this box is the one control
// panel for both. Clicking the lock opens it, and then each has a panel of
// its own. The timeline is one up to the turn it was branched on and forks
// in two there, as Doctor Strange's Civ 6 draws its worlds parting. It is
// drawn as a tree on its side that grows from the left: the one timeline is
// the trunk, in the middle, with the lock at its root, and from the fork a
// branch slants up to A's row and another down to B's, each named on itself
// by its letter and its Civ AI. Each branch ends at the right in its letter,
// the Civ AI that plays it and an ✕ that closes it, and the one that is left
// is the timeline again: when that is B, a timeline that passes from one Civ
// AI to another on the turn it was branched on.
//
// On one timeline, picking another Civ AI asks first whether the timeline
// plays on with it (Switch) or a branch does (Branch). A page that asks for
// it (`tracks=2`) opens a game on its two tracks, asking nothing.
//
// One screen and two change places in one move. A half is a whole page, and
// takes about a second to be there: the window is held as it stands while
// the two load out of sight, and then the two take the one's place at once,
// the map carried to the half that goes on with it and both halves looking
// where the one screen was looking. Closing a branch is the same move back.
// A game that opens on two tracks waits under a veil and is never shown
// on one. See "one move", below the split's style.
//
// A panel of the box (Layers, Breakpoints, Save points) and what the box
// asks are windows: each is moved by pressing its title and dragging.
//
// A game under the debugger has two views. The Debugger view is how it opens:
// the map with the standings across the top, the victory tracker down the
// right and the seat's governors and policies, as a watched game is shown.
// The bar's Single player switch brings the Civilization VI screen for
// playing the seat by hand: yields, launch bar, unit panel, End Turn.
//
// It is a classic script like the others and loads after `app.js`, whose
// `render`, `fetchJSON` and turn loop it uses. Nothing in `app.js` knows it
// is here: the bar redraws by standing behind `render`, the one door every
// state comes through.

// A split's timeline is drawn as a tree on its side (see the style): `row` is
// how tall each of its rails is, `gap` the dark between its two branches, and
// `run` how far along a branch goes before it is in its own row. `rise` is how
// far up or down that takes it, half a row and half the gap, and `tilt` the
// angle it slants at, in radians.
const DEBUG_TREE = {row: 28, gap: 24, run: 32, rise: 26, tilt: Math.atan2(26, 32).toFixed(4)};

const DEBUG_BAR_STYLE = `
  /* The header box stands at the top of the window beside the left panel:
     it begins where the map area does (measured, and published as
     --dbg-left) and runs to the window's right edge, so the left panel keeps
     the window's whole height. The map area stands beneath the box: the
     height the box takes is measured and published as --dbg-top. */
  html.dbg-docked #maparea { margin-top: var(--dbg-top, 0px); }
  /* What the page fixes to the window at a distance from its top moves down
     by the same. */
  html.dbg-docked #moments, html.dbg-docked #site-pick { margin-top: var(--dbg-top, 0px); }
  @media (max-width: 720px) { html.dbg-docked #side { margin-top: var(--dbg-top, 0px); } }
  /* Auto-play is in the header, so the left panel's own copy is put away. */
  html.dbg-docked #humanfooter { display: none !important; }
  #debugbar {
    position: fixed; z-index: 8000; inset: 0 0 auto var(--dbg-left, 0px);
    display: grid; grid-template-columns: minmax(0, 1fr); gap: 7px; padding: 8px 10px 7px;
    color: var(--text); font-size: 12px; line-height: 1.2; user-select: none;
    border-bottom: 1px solid #4d685f; border-left: 1px solid #35504a; background: #0a1512; box-shadow: 0 6px 18px #0008;
  }
  /* Hidden is hidden, whatever a later rule says the thing is laid out as. */
  #debugbar[hidden], #debugbar [hidden] { display: none !important; }
  /* Edge of Tomorrow has a bar of its own and none of the debugger's tools. */
  html.tomorrow #debugbar { display: none !important; }
  html.moneyball #debugbar { display: none !important; }
  /* While the screen is split the box stays up, over the two halves, and has
     the window's whole width: the halves cover the left panel. */
  html.dbg-split-open #debugbar { z-index: 100001; left: 0; }

  /* Under the timeline, one row of three parts. The turn that is open stands
     in the middle, under the middle of the timeline, with the turn buttons on
     either side of it. Auto-play, layers and breakpoints are at the left
     end, and the rest of the tools at the right end. */
  #debugbar .dbg-deck {
    display: grid; grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr); align-items: center; gap: 6px; min-width: 0;
  }
  #debugbar .dbg-side { display: flex; align-items: center; justify-content: space-between; gap: 6px 12px; min-width: 0; }
  #debugbar .dbg-cluster { display: flex; flex-wrap: wrap; align-items: center; gap: 4px 5px; min-width: 0; }
  #debugbar .dbg-right .dbg-cluster { justify-content: flex-end; gap: 4px 12px; }
  #debugbar .dbg-apart { margin-left: 7px; }
  /* Turn 1 and Play to End are words while there is room, and signs when
     not: they are the first words to go (a snug box), before every tool's. */
  #debugbar .dbg-sign { display: none; }
  #debugbar.dbg-snug .dbg-sign, #debugbar.dbg-tight .dbg-sign { display: inline; }
  #debugbar.dbg-snug .dbg-nav .dbg-word { display: none; }
  #debugbar .dbg-group { flex: none; display: inline-flex; align-items: center; gap: 4px; }
  #debugbar button {
    display: inline-flex; align-items: center; justify-content: center; gap: 4px;
    min-width: 26px; height: 26px; padding: 0 6px; border-radius: 8px; cursor: pointer; white-space: nowrap;
    color: var(--text); background: #14231f; border: 1px solid #35504a; font: inherit; line-height: 1;
  }
  #debugbar button i { font-style: normal; }
  #debugbar button:hover:not(:disabled) { border-color: var(--gold-bright, #e7c873); }
  #debugbar button:disabled { opacity: .36; cursor: default; }
  #debugbar button.on { color: #10140f; background: var(--gold-bright, #e7c873); border-color: transparent; font-weight: 700; }
  /* The turn buttons are the hand on the game, and are the larger for it. */
  #debugbar .dbg-nav button { min-width: 36px; height: 32px; padding: 0 8px; font-size: 13px; font-weight: 700; font-variant-numeric: tabular-nums; }
  /* A box too narrow for the words on its tools keeps the signs. */
  #debugbar.dbg-tight .dbg-word { display: none; }
  #debugbar .dbg-count { color: var(--gold-bright, #e7c873); font-weight: 700; }
  #debugbar .dbg-count:empty { display: none; }
  #debugbar button.on .dbg-count { color: inherit; }
  #debugbar .dbg-dot { color: #e2574c; }
  #debugbar .dbg-turnbox {
    display: inline-flex; align-items: center; gap: 6px; padding: 0 4px; color: var(--muted); white-space: nowrap;
    font-weight: 700; letter-spacing: .12em;
  }
  #debugbar #dbg-turn {
    width: 66px; height: 34px; padding: 0 2px; border-radius: 8px; text-align: center; letter-spacing: 0;
    color: var(--gold-bright, #e7c873); background: #060d0b; border: 1px solid #35504a;
    font: 700 24px Georgia, serif; font-variant-numeric: tabular-nums;
  }
  #debugbar #dbg-limit { letter-spacing: 0; font-variant-numeric: tabular-nums; }
  #debugbar #dbg-turn:disabled { opacity: .5; }
  /* Auto-play: which Civ AI plays the seat, for how many turns, and the
     button that starts it. While it plays, the button counts the turns. */
  #debugbar .dbg-field { display: inline-flex; align-items: center; gap: 5px; color: var(--muted); white-space: nowrap; }
  #debugbar .dbg-field > span { font-size: 10px; font-weight: 700; letter-spacing: .1em; text-transform: uppercase; }
  #debugbar .dbg-deck select, #debugbar select.dbg-agent {
    flex: none; height: 26px; padding: 0 4px; border-radius: 8px; cursor: pointer;
    color: var(--text); background: #060d0b; border: 1px solid #35504a; font: inherit;
  }
  /* A Civ AI picker has no width of its own: it is as wide as its longest
     listing, so the Elo and the whole name are read without opening it. */
  #debugbar #dbg-turns { width: 80px; }
  #debugbar .dbg-deck select:disabled, #debugbar select.dbg-agent:disabled { opacity: .5; cursor: default; }
  /* Wide enough for a count, so the row does not move as it plays. 60%
     deeper than the other tools (26px), at the operator's word. */
  #debugbar [data-dbg="autoplay"] { min-width: 112px; height: 42px; font-weight: 700; font-variant-numeric: tabular-nums; }
  #debugbar.dbg-tight [data-dbg="autoplay"] { min-width: 74px; }
  #debugbar #dbg-run-count:empty { display: none; }
  #debugbar:not(.dbg-tight) #dbg-run-count::before { content: "· "; }
  #debugbar.dbg-tight #dbg-turns { width: 78px; }
  #debugbar.dbg-tight .dbg-nav button { min-width: 32px; padding: 0 6px; }
  /* Each half of a split is seated by the host, at the head of its lane. */
  #debugbar.dbg-pane .dbg-agent-field { display: none; }
  /* A box too narrow for the three parts side by side stacks them: the turn
     buttons are a row of their own, and the tools are under them. */
  #debugbar.dbg-stacked .dbg-deck { display: flex; flex-wrap: wrap; justify-content: center; gap: 5px 4px; }
  #debugbar.dbg-stacked .dbg-side { display: contents; }
  #debugbar.dbg-stacked .dbg-left .dbg-nav { order: 1; }
  #debugbar.dbg-stacked .dbg-turnbox { order: 2; }
  #debugbar.dbg-stacked .dbg-right .dbg-nav { order: 3; }
  #debugbar.dbg-stacked .dbg-deck::before { content: ""; order: 4; flex: 0 0 100%; height: 0; }
  #debugbar.dbg-stacked .dbg-cluster { order: 5; justify-content: center; margin: 0 6px; }
  /* What the box has to say stands in the box, between the turn buttons and
     the tools at the right. Its room is kept whether or not there is
     anything to say, so the tools do not move when it speaks. A stacked box
     has no such room, and says it under itself instead. */
  #debugbar .dbg-note {
    flex: 1 1 120px; min-width: 120px; max-height: 56px; overflow: hidden; align-self: center;
    text-align: center; font-size: 11.5px; line-height: 1.25; color: var(--text);
  }
  #debugbar .dbg-note button { height: 18px; margin-left: 6px; vertical-align: middle; }
  #debugbar.dbg-stacked .dbg-note {
    position: absolute; right: 8px; top: calc(100% + 7px); min-width: 0; max-width: min(560px, calc(100% - 16px));
    max-height: none; padding: 5px 11px; border-radius: 999px; font-size: 12px;
    border: 1px solid #4d685f; background: #0a1512f2; box-shadow: var(--shadow);
  }
  #debugbar.dbg-stacked .dbg-note:empty { display: none; }

  /* The timeline is the top of the box: a rail as long as the game. A split
     keeps the one rail up to the turn it was branched on, and from there it
     forks like a tree, a branch for each half.
     It is bright on purpose, sky behind and the turns played in sun across
     it. */
  #debugbar .dbg-time { display: grid; gap: 4px; min-width: 0; }
  /* The measures of that tree (DEBUG_TREE), and where along a rail it forks:
     --dbg-fork is how far along the last turn the halves hold as one world
     stands, and --dbg-at the same from the left end of a rail. */
  #debugbar {
    --dbg-row: ${DEBUG_TREE.row}px; --dbg-gap: ${DEBUG_TREE.gap}px; --dbg-run: ${DEBUG_TREE.run}px; --dbg-rise: ${DEBUG_TREE.rise}px;
    --dbg-at: calc((100% - 36px) * var(--dbg-fork, 1) + 18px);
  }
  /* Above the timeline: the Civ AI that plays the seat from the open turn.
     It begins where the rail does, past the first turn's number. */
  #debugbar .dbg-who { display: flex; align-items: center; gap: 8px; min-width: 0; height: 24px; padding-left: 64px; }
  #debugbar .dbg-who select.dbg-agent { height: 24px; }
  /* Under the timeline's marker, and moving with it: Branch. The lane keeps
     room for it under its rail. */
  #debugbar:not(.dbg-split-on) .dbg-lane[data-dbg-lane=""] { margin-bottom: 26px; }
  #debugbar button.dbg-branch {
    position: absolute; left: 0; top: calc(100% + 6px); transform: translateX(-50%); pointer-events: auto;
    min-width: 0; height: 22px; margin: 0; padding: 0 10px; border-radius: 999px; font-weight: 700;
    color: #3a1d00; background: #ffd23f; border: 1px solid #fff3b0; box-shadow: 0 2px 6px #0007;
  }
  #debugbar button.dbg-branch::before {
    content: ""; position: absolute; left: 50%; bottom: 100%; width: 2px; height: 7px; margin-left: -1px; background: #ffd23f;
  }
  #debugbar button.dbg-branch:hover:not(:disabled) { border-color: #ffffff; background: #ffe27a; }
  /* While it asks which Civ AI plays the branch. */
  #debugbar button.dbg-branch.on { color: #3a1d00; background: #ffffff; border-color: #ffffff; }
  /* Two branches are the most there are: a split has no Branch. */
  #debugbar.dbg-split-on .dbg-branch, #debugbar.dbg-split-on .dbg-who { display: none; }
  #debugbar .dbg-lane { display: flex; align-items: center; gap: 8px; min-width: 0; --dbg-own: #ff8a1f; }
  #debugbar .dbg-lane[data-dbg-lane="a"] { --dbg-own: #ff6b4a; }
  #debugbar .dbg-lane[data-dbg-lane="b"] { --dbg-own: #b06cff; }
  #debugbar .dbg-lane-name { flex: none; width: 12px; text-align: center; font-weight: 700; color: var(--dbg-own); }
  #debugbar .dbg-lane-name:empty { display: none; }
  /* The two ends of a lane: the turn the game begins on at the left, and at
     the right the turn it ends on, under a chequered flag when the game has a
     last turn. The world's lane gives both ends the same width, so the middle
     of its rail is the middle of the box, where the open turn stands. */
  #debugbar .dbg-start, #debugbar .dbg-finish {
    flex: none; display: inline-flex; align-items: center; gap: 5px;
    font-size: 13px; font-weight: 700; font-variant-numeric: tabular-nums; color: #b7c7c0;
  }
  #debugbar .dbg-start { min-width: 10px; justify-content: flex-end; }
  #debugbar .dbg-lane[data-dbg-lane=""] .dbg-start, #debugbar .dbg-lane[data-dbg-lane=""] .dbg-finish { width: 56px; }
  #debugbar .dbg-finish b { color: var(--text); }
  #debugbar .dbg-flag { display: none; position: relative; width: 20px; height: 22px; }
  #debugbar .dbg-flagged .dbg-flag { display: block; }
  #debugbar .dbg-flag::before {
    content: ""; position: absolute; left: 1px; top: 1px; bottom: 0; width: 2px; border-radius: 1px; background: #d8e2dd;
  }
  #debugbar .dbg-flag::after {
    content: ""; position: absolute; left: 3px; top: 2px; width: 16px; height: 12px; border-radius: 0 2px 2px 0;
    background: repeating-conic-gradient(#10140f 0 25%, #f4f6f2 0 50%) 0 0 / 8px 8px; box-shadow: 0 0 0 1px #10140f;
  }
  /* A rail is where its lane's slider is. The sky is the rail as it is
     seen, and is a layer of its own so that a split can show part of it. */
  #debugbar .dbg-rail { position: relative; flex: 1; min-width: 0; height: 68px; border: 1px solid transparent; }
  #debugbar .dbg-sky {
    position: absolute; inset: -1px; border-radius: 12px; pointer-events: none;
    background: linear-gradient(180deg, #9be2ff 0%, #58bdf7 100%); border: 1px solid #d3f2ff;
    box-shadow: inset 0 1px 0 #ffffffa0, 0 2px 10px #0007;
  }
  /* A split has three lanes in one grid, and they are drawn as a tree on its
     side that grows from the left, one timeline into two (operator
     2026-11-26: "branches should split from 1 to 2... not 2 to 1"). The
     world's lane is the trunk: the one timeline up to the turn it was
     branched on, a rail in the middle of the box, with the lock and the turn
     the game begins on at its root. Each half's lane is a row of its own, A's
     over the trunk and B's under it, with the box's dark between them, and
     from the turn it was branched on the trunk forks into the two. Each
     branch ends in what is its own: its letter, the Civ AI that plays it and
     its ✕. Nothing of the two stands left of the trunk. */
  #debugbar .dbg-half .dbg-rail { height: var(--dbg-row); }
  #debugbar.dbg-split-on .dbg-time {
    grid-template-columns: auto auto minmax(0, 1fr) auto auto auto auto; grid-template-rows: var(--dbg-row) var(--dbg-row);
    column-gap: 8px; row-gap: var(--dbg-gap); align-items: center;
  }
  #debugbar.dbg-split-on .dbg-lane { display: contents; }
  #debugbar.dbg-split-on .dbg-lane > * { grid-row: 1 / span 2; }
  #debugbar.dbg-split-on .dbg-lane[data-dbg-lane="a"] > * { grid-row: 1; }
  #debugbar.dbg-split-on .dbg-lane[data-dbg-lane="b"] > * { grid-row: 2; }
  #debugbar.dbg-split-on .dbg-start { grid-column: 2; }
  #debugbar.dbg-split-on .dbg-rail { grid-column: 3; }
  #debugbar.dbg-split-on .dbg-lane-name { grid-column: 4; margin-right: -4px; }
  #debugbar.dbg-split-on .dbg-lane select.dbg-agent { grid-column: 5; }
  #debugbar.dbg-split-on .dbg-close { grid-column: 6; }
  #debugbar.dbg-split-on .dbg-finish { grid-column: 7; }
  /* The ✕ of a branch closes it, and the other is the timeline. It stands at
     the right, where its branch's row ends (operator 2026-11-22: "move x
     deletes over to the right side please, right after each closable
     timeline ends"), past the branch's Civ AI. */
  #debugbar button.dbg-close {
    min-width: 0; width: 22px; height: 22px; margin: 0; padding: 0; border-radius: 50%;
    color: var(--dbg-own); border-color: var(--dbg-own); font-weight: 700;
  }
  #debugbar button.dbg-close:hover:not(:disabled) { color: #10140f; background: var(--dbg-own); border-color: transparent; }
  /* The two ends of the game are named once, by the trunk. */
  #debugbar .dbg-half .dbg-start, #debugbar .dbg-half .dbg-finish { display: none; }
  #debugbar.dbg-split-on .dbg-lane[data-dbg-lane=""] .dbg-start,
  #debugbar.dbg-split-on .dbg-lane[data-dbg-lane=""] .dbg-finish { width: auto; }
  /* The trunk is drawn and not pressed: what is pressed is a half's slider.
     Each is pressed from its own row to the middle of the tree, so the top
     half of the timeline is the hand on A and the bottom half the hand on B,
     on the trunk as on the branches. */
  #debugbar.dbg-split-on .dbg-lane[data-dbg-lane=""] .dbg-rail { height: var(--dbg-row); align-self: center; pointer-events: none; }
  #debugbar.dbg-split-on .dbg-half .dbg-rail input { height: calc(100% + var(--dbg-gap) / 2 + 1px); }
  #debugbar.dbg-split-on .dbg-lane[data-dbg-lane="a"] .dbg-rail input { bottom: auto; }
  #debugbar.dbg-split-on .dbg-lane[data-dbg-lane="b"] .dbg-rail input { top: auto; }
  #debugbar .dbg-half .dbg-rail input::-webkit-slider-thumb { height: calc(var(--dbg-row) + var(--dbg-gap) / 2); }
  #debugbar .dbg-half .dbg-rail input::-moz-range-thumb { height: calc(var(--dbg-row) + var(--dbg-gap) / 2); }
  /* What a half holds past the turn the two part on is the half's own, and
     is drawn in the half's lane from there: the turns it has played, in its
     colour. (A split that had not forked drew a half's lane where the trunk
     is, its rail shown only when its slider had the keys; a split now forks
     as it opens, on the turn it was branched on.) */
  #debugbar.dbg-split-on .dbg-half .dbg-deco { clip-path: inset(0 0 0 calc(100% * var(--dbg-fork, 1))); }
  #debugbar.dbg-split-on:not(.dbg-forked) .dbg-lane[data-dbg-lane="a"] :is(.dbg-sky, .dbg-deco) { transform: translateY(var(--dbg-rise)); }
  #debugbar.dbg-split-on:not(.dbg-forked) .dbg-lane[data-dbg-lane="b"] :is(.dbg-sky, .dbg-deco) { transform: translateY(calc(-1 * var(--dbg-rise))); }
  #debugbar:not(.dbg-forked) .dbg-half .dbg-sky { background: none; border-color: transparent; box-shadow: none; }
  #debugbar:not(.dbg-forked) .dbg-half .dbg-ruler { visibility: hidden; }
  /* On the turn it was branched on (dbg-forked) the tree forks. The trunk is
     shown up to that turn, and each half's own rail from where its branch is
     in its row, --dbg-run further on. Between the two is the branch on its
     way there (dbg-arm): the half's rail and what is drawn on it over again
     (dbg-bent), slanted from the trunk to the row. Each arm is cut along the
     middle of the tree, so at the fork the trunk's band is half A's colour
     and half B's, and the two part from there. */
  #debugbar.dbg-forked .dbg-lane[data-dbg-lane=""] .dbg-sky {
    right: calc((100% - 36px) * (1 - var(--dbg-fork, 1)) + 18px); border-right: 0; border-radius: 12px 0 0 12px;
  }
  #debugbar.dbg-forked .dbg-lane[data-dbg-lane=""] .dbg-deco { clip-path: inset(0 calc(100% * (1 - var(--dbg-fork, 1))) 0 0); }
  #debugbar.dbg-forked .dbg-half .dbg-sky { left: calc(var(--dbg-at) + var(--dbg-run)); border-left: 0; border-radius: 0 12px 12px 0; }
  #debugbar.dbg-forked .dbg-half .dbg-deco { clip-path: inset(0 0 0 calc(100% * var(--dbg-fork, 1) + var(--dbg-run))); }
  #debugbar .dbg-arm {
    display: none; position: absolute; inset: 0; pointer-events: none;
    transform-origin: calc(var(--dbg-at) + var(--dbg-run)) 0;
  }
  #debugbar.dbg-forked .dbg-arm { display: block; }
  #debugbar .dbg-lane[data-dbg-lane="a"] .dbg-arm {
    transform: skewY(-${DEBUG_TREE.tilt}rad);
    clip-path: polygon(calc(var(--dbg-at) - 1px) -13px, calc(var(--dbg-at) + var(--dbg-run) + 1px) -13px,
      calc(var(--dbg-at) + var(--dbg-run) + 1px) calc(50% + var(--dbg-rise)), calc(var(--dbg-at) - 1px) 50%);
  }
  #debugbar .dbg-lane[data-dbg-lane="b"] .dbg-arm {
    transform: skewY(${DEBUG_TREE.tilt}rad);
    clip-path: polygon(calc(var(--dbg-at) - 1px) 50%, calc(var(--dbg-at) + var(--dbg-run) + 1px) calc(50% - var(--dbg-rise)),
      calc(var(--dbg-at) + var(--dbg-run) + 1px) calc(100% + 13px), calc(var(--dbg-at) - 1px) calc(100% + 13px));
  }
  #debugbar .dbg-arm-sky {
    position: absolute; inset: -1px 0; border: solid #d3f2ff; border-width: 1px 0;
    background: linear-gradient(180deg, #9be2ff 0%, #58bdf7 100%); box-shadow: inset 0 1px 0 #ffffffa0, 0 2px 10px #0007;
  }
  #debugbar .dbg-bent { position: absolute; inset: 0 18px; }
  /* The band runs on through the fork: the trunk's has no end there, and a
     branch's no beginning. */
  #debugbar.dbg-forked .dbg-lane[data-dbg-lane=""] .dbg-band { border-radius: 7px 0 0 7px; }
  #debugbar.dbg-forked .dbg-half .dbg-band.dbg-own { border-radius: 0 7px 7px 0; }
  /* The turn the timeline forks on, said where it does: between the two
     branches, where they have parted. */
  #debugbar .dbg-fork {
    position: absolute; z-index: 2; top: 50%; transform: translate(calc(var(--dbg-run) + 3px), -50%); padding: 2px 7px; border-radius: 999px;
    white-space: nowrap; font-size: 11px; font-weight: 700; font-variant-numeric: tabular-nums;
    color: #ffffff; background: #0b3554; box-shadow: 0 0 0 1px #d3f2ff;
  }
  /* The turns of a tree are numbered once, on the dark beside its rails:
     over the trunk, and between the two branches. */
  #debugbar.dbg-split-on .dbg-ruler b { display: none; }
  #debugbar.dbg-split-on .dbg-ruler i.major { top: 0; }
  #debugbar .dbg-axis { position: absolute; inset: 0 18px; pointer-events: none; }
  #debugbar .dbg-axis b {
    position: absolute; bottom: calc(100% + 7px); transform: translateX(-50%);
    font-size: 11px; font-weight: 700; color: #b7c7c0; font-variant-numeric: tabular-nums;
  }
  #debugbar .dbg-axis b.dbg-between {
    z-index: 4; bottom: auto; top: 50%; transform: translate(-50%, -50%); padding: 1px 4px; background: #0a1512;
  }
  /* The lock that holds the two halves of a split on one turn, at the root
     of the tree, where the trunk begins: bright while the halves are locked
     on the same turn, dim while each keeps its own. */
  #debugbar .dbg-tie { display: none; }
  #debugbar.dbg-split-on .dbg-tie {
    display: block; grid-column: 1; grid-row: 1 / span 2; align-self: stretch; position: relative;
    width: 30px;
  }
  #debugbar button.dbg-lock {
    position: absolute; left: 0; top: 50%; transform: translateY(-50%); min-width: 0; width: 30px; height: 30px;
    padding: 0; border-radius: 50%; color: #b7c7c0; background: #14231f; border: 1px solid #6f8a80;
  }
  #debugbar.dbg-locked button.dbg-lock { color: #3a1d00; background: #ffd23f; border-color: #fff3b0; }
  /* A lock that is shut stays whole while the halves are being played: it
     is still what holds them. An open one that cannot be shut yet is dim. */
  #debugbar.dbg-locked button.dbg-lock:disabled { opacity: 1; }
  #debugbar:not(.dbg-locked) button.dbg-lock:disabled { opacity: .5; }
  #debugbar .dbg-lock svg { display: block; }
  #debugbar .dbg-lock .dbg-shackle { fill: none; stroke: currentColor; stroke-width: 2; stroke-linecap: round; }
  #debugbar.dbg-locked .dbg-lock .dbg-shackle-open, #debugbar:not(.dbg-locked) .dbg-lock .dbg-shackle-shut { display: none; }
  /* Locked and on the same turn, the two markers are one: a link stands
     between them, from the middle of A's to the middle of B's. */
  #debugbar.dbg-joined .dbg-lane[data-dbg-lane="a"] .dbg-cursor::after {
    content: ""; position: absolute; z-index: -1; left: -2px; top: 50%; width: 4px; height: calc(var(--dbg-row) + var(--dbg-gap));
    border-radius: 2px; background: #ffd23f; box-shadow: 0 0 0 1px #3a1d0060;
  }
  /* A half's marker stands in the half's own row. Where the tree is not in
     that row (the trunk, and a branch still on its way to its row) a pin
     runs from the marker to the middle of the tree. */
  #debugbar .dbg-half .dbg-cursor.dbg-pinned::before {
    content: ""; position: absolute; left: -1.5px; width: 3px; height: var(--dbg-rise); border-radius: 2px;
    background: var(--dbg-own); box-shadow: 0 0 0 1px #3a1d0060;
  }
  #debugbar .dbg-lane[data-dbg-lane="a"] .dbg-cursor.dbg-pinned::before { top: 50%; }
  #debugbar .dbg-lane[data-dbg-lane="b"] .dbg-cursor.dbg-pinned::before { bottom: 50%; }
  /* Two markers that are joined have the link between them instead. */
  #debugbar.dbg-joined .dbg-half .dbg-cursor.dbg-pinned::before { display: none; }
  #debugbar .dbg-rail:focus-within .dbg-sky, #debugbar .dbg-half .dbg-rail:focus-within .dbg-sky {
    border-color: #ffffff; box-shadow: inset 0 1px 0 #ffffffa0, 0 0 0 2px #ffd23f, 0 2px 10px #0007;
  }
  /* The slider is set in from each end of its rail, and its thumb stops half
     its own width short of each end of that, so what is drawn on a rail is
     drawn in the thumb's travel: the marker stays whole inside the rail on
     the first turn and on the last. */
  #debugbar .dbg-deco, #debugbar .dbg-hands, #debugbar .dbg-marks { position: absolute; inset: 0 18px; pointer-events: none; }
  #debugbar .dbg-marks { z-index: 2; }
  #debugbar .dbg-rail input {
    position: absolute; z-index: 1; inset: 0 11px; width: calc(100% - 22px); height: 100%; margin: 0; padding: 0;
    opacity: 0; cursor: pointer; -webkit-appearance: none; appearance: none; background: transparent;
  }
  #debugbar .dbg-rail input::-webkit-slider-thumb { -webkit-appearance: none; width: 14px; height: 66px; }
  #debugbar .dbg-rail input::-moz-range-thumb { width: 14px; height: 66px; border: 0; }
  #debugbar .dbg-rail input:disabled { pointer-events: none; }
  /* The ruler's numbers run along the top of a rail, the turns played across
     its middle, and the eras' names along the bottom. */
  #debugbar .dbg-ruler i { position: absolute; bottom: 0; height: 6px; width: 1px; background: #0b355470; }
  #debugbar .dbg-ruler i.major { top: 17px; height: auto; background: #0b355438; }
  #debugbar .dbg-ruler b {
    position: absolute; top: 2px; transform: translateX(-50%);
    font-size: 11px; font-weight: 700; color: #0b3554; font-variant-numeric: tabular-nums;
  }
  /* A game played on past its last turn has the flag's line where that was. */
  #debugbar .dbg-ruler i.dbg-finish-line {
    top: 0; height: auto; width: 4px; margin-left: -2px;
    background: repeating-conic-gradient(#10140f 0 25%, #f4f6f2 0 50%) 0 0 / 4px 4px;
  }
  #debugbar .dbg-band {
    position: absolute; top: 20px; bottom: 20px; min-width: 4px; border-radius: 7px;
    background: linear-gradient(180deg, #fff27a 0%, #ffc61a 100%); box-shadow: 0 0 0 1px #b8770a80, 0 1px 4px #0b355466;
  }
  /* A tree's rails are slim, and the band is in the middle of each, so the
     trunk's band is cut in two down its middle where the tree forks. */
  #debugbar.dbg-split-on .dbg-band { top: 3px; bottom: 3px; }
  #debugbar .dbg-band.dbg-own { background: var(--dbg-own); }
  /* Whose play the turns were: each stretch of the band one Civ AI played is
     named at its start, every other one is a shade darker, and a line stands
     where the seat passed from one to the next. A name is cut short at the
     end of its stretch, but for the last one, which has the rail ahead of it
     to run on over. */
  #debugbar .dbg-by s {
    position: absolute; top: 20px; bottom: 20px; overflow: hidden; text-decoration: none; border-radius: 0 7px 7px 0;
  }
  #debugbar.dbg-split-on .dbg-by s { top: 3px; bottom: 3px; }
  #debugbar .dbg-by s.dbg-alt { background: #3a1d0024; }
  #debugbar .dbg-by s.dbg-next { border-left: 2px solid #3a1d00a0; border-radius: 0; }
  #debugbar .dbg-by s.dbg-free { overflow: visible; }
  /* Turns kept ahead that another Civ AI played are drawn as they are
     (dbg-beyond), and only named here. */
  #debugbar .dbg-by s.dbg-far { z-index: 1; }
  #debugbar .dbg-by b {
    position: absolute; left: 3px; top: 50%; transform: translateY(-50%); max-width: calc(100% - 6px);
    padding: 1px 5px; border-radius: 4px; overflow: hidden; white-space: nowrap; text-overflow: ellipsis;
    font-size: 10.5px; font-weight: 700; color: #3a1d00; background: #ffffffc4;
  }
  #debugbar .dbg-by s.dbg-free b { max-width: none; }
  /* When turns were thinned to make room, the band pales and the turns that
     can still be gone back to stand out on it. */
  #debugbar .dbg-thinned .dbg-band { background: #fff4b8; box-shadow: 0 0 0 1px #b8770a40; }
  #debugbar .dbg-kept-turns i { position: absolute; top: 20px; bottom: 20px; width: 2px; margin-left: -1px; background: #e08a00; }
  /* Where a run is headed. */
  #debugbar .dbg-goal {
    position: absolute; top: 20px; bottom: 20px; border-right: 2px solid var(--dbg-own);
    background: repeating-linear-gradient(-45deg, #ffffffc0 0 5px, transparent 5px 10px);
  }
  #debugbar.dbg-split-on .dbg-goal { top: 3px; bottom: 3px; }
  /* Kept turns ahead that another Civ AI played: not this seat's line, and
     played over by the one picked now. */
  #debugbar .dbg-beyond {
    position: absolute; top: 25px; bottom: 25px; min-width: 4px; border-radius: 0 5px 5px 0;
    background: repeating-linear-gradient(90deg, #3f5f7ad0 0 4px, transparent 4px 8px); box-shadow: 0 0 0 1px #3f5f7a90;
  }
  #debugbar.dbg-split-on .dbg-beyond { top: 8px; bottom: 8px; }
  #debugbar .dbg-ghost { position: absolute; top: 0; bottom: 0; width: 2px; margin-left: -1px; background: #0b3554c0; }
  /* The marker stands over whatever else is on its turn (an era's line, a
     flag), and a press on it still reaches the slider beneath. */
  #debugbar .dbg-cursor { position: absolute; z-index: 3; top: 0; bottom: 0; width: 0; }
  #debugbar .dbg-cursor b {
    position: absolute; left: 0; top: 50%; transform: translate(-50%, -50%); min-width: 42px; padding: 6px 9px;
    border-radius: 999px; text-align: center; font-size: 15px; font-weight: 700; font-variant-numeric: tabular-nums;
    color: #3a1d00; background: #ffffff; box-shadow: 0 0 0 3px var(--dbg-own), 0 3px 9px #0b3554a0;
  }
  #debugbar .dbg-half .dbg-cursor b { min-width: 34px; padding: 4px 6px; font-size: 12px; }
  /* The timeline says how it is used, loud, until it has been used: Click to
     travel, in the open rail beside the marker, calling with a ring (operator
     2026-11-22: "have the initial timeline say click to travel so people
     understand the mechanism. make the click to travel very visible at the
     start and then not appear once they understand"). It takes no press: a
     click on it is the rail's, under it. The marker stands over it. */
  #debugbar .dbg-travel {
    position: absolute; z-index: 3; top: 50%; left: 50%; transform: translate(-50%, -50%);
    display: flex; align-items: center; gap: 9px; padding: 5px 16px; border-radius: 999px;
    pointer-events: none; white-space: nowrap; font-size: 15px; font-weight: 800; letter-spacing: .02em;
    color: #10140f; background: #ffd23f; border: 2px solid #ffffff;
    animation: dbg-travel-call 1.6s ease-out infinite;
  }
  /* On a split it stands between the two branches, over the turn numbers
     that are said there. */
  #debugbar.dbg-split-on .dbg-travel { z-index: 4; padding: 3px 16px; font-size: 14px; }
  #debugbar .dbg-travel i { font-style: normal; font-size: 10px; animation: dbg-travel-nudge 1.6s ease-in-out infinite; }
  #debugbar .dbg-travel i:first-child { --dbg-nudge: -4px; }
  #debugbar .dbg-travel i:last-child { --dbg-nudge: 4px; }
  @keyframes dbg-travel-call {
    from { box-shadow: 0 2px 10px #0009, 0 0 0 0 #ffd23fd0; }
    to { box-shadow: 0 2px 10px #0009, 0 0 0 16px #ffd23f00; }
  }
  @keyframes dbg-travel-nudge { 50% { transform: translateX(var(--dbg-nudge)); } }
  /* Used once, it goes, and is not said again. */
  #debugbar .dbg-travel.dbg-travel-gone { animation: dbg-travel-gone .35s ease-in forwards; }
  @keyframes dbg-travel-gone { to { opacity: 0; transform: translate(-50%, -50%) scale(.8); } }
  @media (prefers-reduced-motion: reduce) {
    #debugbar .dbg-travel, #debugbar .dbg-travel i { animation: none; box-shadow: 0 2px 10px #0009; }
  }
  /* Two halves on one turn of the trunk have one marker, ringed in both
     their colours. */
  #debugbar.dbg-split-on .dbg-lane[data-dbg-lane=""] .dbg-cursor b {
    min-width: 48px; border: 3px solid transparent; box-shadow: 0 3px 9px #0b3554a0;
    background: linear-gradient(#ffffff, #ffffff) padding-box, linear-gradient(180deg, #ff6b4a 50%, #b06cff 50%) border-box;
  }
  /* The line under the pointer runs from a half's row to the middle of the
     tree, and locked, when a place on the timeline is both halves', the
     height of both rows. */
  #debugbar.dbg-split-on .dbg-lane[data-dbg-lane="a"] .dbg-ghost { bottom: calc(var(--dbg-gap) / -2); }
  #debugbar.dbg-split-on .dbg-lane[data-dbg-lane="b"] .dbg-ghost { top: calc(var(--dbg-gap) / -2); }
  #debugbar.dbg-locked .dbg-lane[data-dbg-lane="a"] .dbg-ghost { bottom: calc(-1 * (var(--dbg-row) + var(--dbg-gap))); }
  #debugbar.dbg-locked .dbg-lane[data-dbg-lane="b"] .dbg-ghost { top: calc(-1 * (var(--dbg-row) + var(--dbg-gap))); }
  /* Save points stand on a rail as flags and turn breakpoints as red dots,
     each at its own turn. */
  #debugbar .dbg-marks i {
    position: absolute; top: -5px; width: 11px; height: 11px; margin-left: -6px; cursor: pointer; pointer-events: auto;
    border-radius: 2px 2px 50% 50%; border: 1px solid #10140f; background: var(--gold-bright, #e7c873);
  }
  #debugbar .dbg-marks i.auto { background: #8fb4a8; }
  #debugbar .dbg-marks i.stop { top: auto; bottom: -5px; border-radius: 50%; background: #e2574c; }
  /* On the one timeline a breakpoint is a red line down through the rail,
     as a debugger draws one, named at its head (operator, 2026-11-27:
     "breakpoints should add a vertical red line through the spot on the
     timeline. like a debugger"). A line whose turn is worked out of the
     game's future, not set (the next era, a nuclear war), is dashed. */
  #debugbar:not(.dbg-split-on) .dbg-marks i.stop {
    top: 0; bottom: 0; width: 0; height: auto; margin-left: -1px; border: 0; border-radius: 0;
    border-left: 2px solid #e2574c; background: none; box-shadow: 0 0 0 1px #10140f40;
  }
  #debugbar:not(.dbg-split-on) .dbg-marks i.stop.dbg-ahead { border-left-style: dashed; box-shadow: none; }
  #debugbar .dbg-marks i.stop b {
    position: absolute; top: 2px; left: 3px; padding: 0 5px; border-radius: 3px; white-space: nowrap;
    font: 700 10px/15px system-ui, sans-serif; color: #fff; background: #b8322a; cursor: pointer;
  }
  #debugbar .dbg-marks i.stop.dbg-ahead b { background: #10140fcc; color: #ff8a80; border: 1px dashed #e2574c; line-height: 13px; }
  #debugbar .dbg-marks i.stop.dbg-row2 b { top: 20px; }
  /* A name stands over every line, not under the next line along. */
  #debugbar .dbg-marks i.stop b { z-index: 3; }
  #debugbar.dbg-split-on .dbg-marks i.stop b { display: none; }
  /* On a tree's slim rails a mark keeps closer to its own rail. */
  #debugbar.dbg-split-on .dbg-marks i { top: -2px; }
  #debugbar.dbg-split-on .dbg-marks i.stop { top: auto; bottom: -2px; }
  #debugbar.dbg-split-on .dbg-marks i.stop.dbg-ahead { display: none; }
  /* The first turn of each era the game has been through: a line down the
     rail with the era's name at its foot. The name is the thing to click. */
  #debugbar .dbg-marks u {
    position: absolute; top: 0; bottom: 0; display: flex; align-items: flex-end; text-decoration: none;
    border-left: 2px dotted #0b355490;
  }
  #debugbar .dbg-marks u b {
    margin: 0 0 3px 4px; padding: 1px 6px; border-radius: 999px; cursor: pointer; pointer-events: auto; white-space: nowrap;
    font-size: 10.5px; font-weight: 700; color: #ffffff; background: #0b3554;
  }
  #debugbar .dbg-marks u b:hover { background: #124d78; }
  /* An era both halves began on the same turn is named once, at the foot of
     the trunk (dbg-both). Unlocked, it is neither half's to go to: each has
     its own slider. */
  #debugbar .dbg-marks u.dbg-still b { pointer-events: none; cursor: default; }
  /* What letting go of the timeline here would do, said under the pointer. */
  #debugbar .dbg-hover {
    position: absolute; z-index: 4; top: 136px; transform: translateX(-50%); padding: 4px 10px;
    border-radius: 999px; white-space: nowrap; pointer-events: none;
    border: 1px solid #4d685f; background: #0a1512f5; box-shadow: var(--shadow);
  }
  /* The host of a split shows the two halves' timeline. While the halves are
     unlocked that is all it shows: each half carries the tools for its own
     world, and no timeline. While they are locked the two are one, and the
     host's deck is the one control panel for both: the halves put theirs
     away (dbg-pane-locked). */
  #debugbar.dbg-split-on:not(.dbg-merged) .dbg-deck, #debugbar.dbg-split-on:not(.dbg-merged) .dbg-pop { display: none; }
  #debugbar.dbg-split-on .dbg-hover { top: calc(100% + 5px); }
  #debugbar.dbg-merged .dbg-hover { top: 94px; }
  /* Each half's Civ AI is picked at the head of its lane, and a save is one
     world's, kept from that half's own panel. */
  #debugbar.dbg-split-on .dbg-agent-field, #debugbar.dbg-split-on .dbg-file-saves { display: none; }
  #debugbar.dbg-pane .dbg-time { display: none; }
  html.dbg-pane-locked #debugbar, html.dbg-pane-locked #humanfooter { display: none !important; }

  #debugbar .dbg-pop {
    position: absolute; z-index: 3; left: 8px; top: calc(100% + 6px);
    width: min(360px, calc(100vw - 16px)); max-height: calc(100vh - 90px); overflow-y: auto;
    display: grid; gap: 8px; padding: 12px; white-space: normal;
    border-radius: 14px; border: 1px solid #4d685f; background: #0a1512fa; box-shadow: var(--shadow);
  }
  #debugbar .dbg-pop h4 { margin: 0; font: 600 14px Georgia, serif; color: var(--gold-bright, #e7c873); }
  #debugbar .dbg-pop p { margin: 0; color: var(--muted); line-height: 1.35; }
  #debugbar .dbg-pop label { display: flex; align-items: center; gap: 7px; cursor: pointer; }
  #debugbar .dbg-pop .dbg-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 4px 10px; }
  #debugbar .dbg-pop input[type=text] {
    flex: 1; min-width: 0; height: 26px; padding: 0 7px; border-radius: 7px;
    color: var(--text); background: #060d0b; border: 1px solid #35504a; font: inherit;
  }
  #debugbar .dbg-pop select {
    min-width: 0; max-width: 104px; height: 26px; border-radius: 7px;
    color: var(--text); background: #060d0b; border: 1px solid #35504a; font: inherit;
  }
  #debugbar .dbg-pop .dbg-line { display: flex; align-items: center; gap: 6px; }
  #debugbar .dbg-pop .dbg-line > span { flex: none; width: 58px; color: var(--muted); }
  /* What the box asks (debugAsk): which Civ AI plays the branch that Branch
     is about to make, under Branch, or whether a Civ AI picked on the one
     timeline plays on here or in a branch, under the picker it was picked
     from. It is the box's own and not the deck's. */
  #debugbar .dbg-ask {
    position: absolute; z-index: 5; left: 8px; top: calc(100% + 6px);
    width: min(300px, calc(100vw - 16px)); max-height: calc(100vh - 110px); overflow-y: auto;
    display: grid; gap: 8px; padding: 12px; white-space: normal;
    border-radius: 14px; border: 1px solid #ffd23f; background: #0a1512fa; box-shadow: var(--shadow);
  }
  #debugbar .dbg-ask h4 { margin: 0; font: 600 14px Georgia, serif; color: var(--gold-bright, #e7c873); }
  #debugbar .dbg-ask p { margin: 0; color: var(--muted); line-height: 1.35; }
  #debugbar .dbg-ask p b { color: var(--text); }
  #debugbar .dbg-ask-list { display: grid; gap: 4px; }
  #debugbar .dbg-ask-list button { justify-content: space-between; gap: 10px; padding: 0 9px; font-variant-numeric: tabular-nums; }
  #debugbar .dbg-ask-list small { font-size: 10.5px; opacity: .75; }
  #debugbar .dbg-ask .dbg-line { display: flex; flex-wrap: wrap; align-items: center; gap: 6px; }
  #debugbar .dbg-ask .dbg-line button { padding: 0 11px; font-weight: 700; }
  /* A panel and the box's question are windows: each is moved by its title
     (debugMoveWindows), which is the whole band across its top. */
  #debugbar .dbg-pop h4:first-child, #debugbar .dbg-ask h4, #debugbar .dbg-search > h4:first-child {
    margin: -12px -12px -6px; padding: 12px 12px 6px; border-radius: 14px 14px 0 0;
    cursor: grab; touch-action: none;
  }
  #debugbar .dbg-pop h4:first-child:hover, #debugbar .dbg-ask h4:hover,
  #debugbar .dbg-search > h4:first-child:hover { background: #ffffff0d; }
  #debugbar .dbg-moving > h4:first-child { cursor: grabbing; background: #ffffff14; }

  /* One screen and two change places in one move (debugSwap). While the move
     is made ready the window is held as it stands: the box takes no press,
     and Branch says that it is at work. */
  html.dbg-held #debugbar { pointer-events: none; }
  html.dbg-held body { cursor: progress; }
  @keyframes dbg-at-work { from { box-shadow: 0 2px 6px #0007, 0 0 0 0 #ffd23f66; } to { box-shadow: 0 2px 6px #0007, 0 0 0 7px #ffd23f00; } }
  #debugbar.dbg-branching button.dbg-branch { animation: dbg-at-work .8s ease-out infinite; }
  #debugbar.dbg-branching button.dbg-branch::after { content: "ing…"; margin-left: -4px; }
  /* A game that opens on its two tracks is not shown on one first: the window
     waits, as the page's own curtain has it wait, for the two to be there. */
  #dbg-veil {
    position: fixed; inset: 0; z-index: 100002; display: flex; flex-direction: column; gap: 14px;
    align-items: center; justify-content: center; background: #07110f; color: #d7b66a;
    font-family: Georgia, "Times New Roman", serif;
  }
  #dbg-veil b { font-size: 40px; font-weight: 400; letter-spacing: .34em; text-indent: .34em; }
  #dbg-veil span { font-size: 13px; letter-spacing: .18em; color: #6f8279; text-transform: uppercase; }
  /* The move itself: the box goes from one shape to the other with its right
     end, where the game ends, held still, and the map goes to the half that
     carries it on (or that half to the map) while the rest comes and goes
     around it. Nothing is stretched on the way: each picture keeps its own
     size and is cut to the room it has. And the map does not slide under
     itself: the two pictures show the same ground at the same size, so each
     is carried by as much as puts the middle of its map on the middle of the
     other's (--dbg-carry-x and -y, measured by debugSwap), and what is seen
     is one map with its panels changing round it. */
  ::view-transition-group(dbg-bar), ::view-transition-group(dbg-stage) {
    animation-duration: .28s; animation-timing-function: cubic-bezier(.3, .1, .2, 1);
  }
  ::view-transition-old(dbg-bar), ::view-transition-new(dbg-bar) {
    width: 100%; height: 100%; object-fit: none; object-position: right top; animation-duration: .2s;
  }
  @keyframes dbg-swap-out { to { opacity: 0; } }
  @keyframes dbg-swap-in { from { opacity: 0; } }
  @keyframes dbg-carry-old { to { object-position: var(--dbg-carry-x, 0px) var(--dbg-carry-y, 0px); } }
  @keyframes dbg-carry-new { from { object-position: calc(0px - var(--dbg-carry-x, 0px)) calc(0px - var(--dbg-carry-y, 0px)); } }
  ::view-transition-old(dbg-stage), ::view-transition-new(dbg-stage) {
    width: 100%; height: 100%; object-fit: none; object-position: 0 0; mix-blend-mode: plus-lighter;
  }
  ::view-transition-old(dbg-stage) {
    animation: dbg-swap-out .28s cubic-bezier(.3, .1, .2, 1) both, dbg-carry-old .28s cubic-bezier(.3, .1, .2, 1) both;
  }
  ::view-transition-new(dbg-stage) {
    animation: dbg-swap-in .28s cubic-bezier(.3, .1, .2, 1) both, dbg-carry-new .28s cubic-bezier(.3, .1, .2, 1) both;
  }
  html.dbg-swapping::view-transition-old(root), html.dbg-swapping::view-transition-new(root) { animation-duration: .24s; }

  /* Autosearch stands with Layers and Breakpoints and is a size larger than
     the tools beside it (operator 2026-11-22: "make it a bit bigger than the
     others"): as tall as a turn button, in gold. While a search runs it is
     lit and counts the futures played. */
  #debugbar button.dbg-search-open {
    height: 32px; padding: 0 12px; gap: 6px; border-radius: 9px; font-size: 13.5px; font-weight: 700;
    color: var(--gold-bright, #e7c873); border-color: var(--gold-bright, #e7c873); font-variant-numeric: tabular-nums;
  }
  #debugbar button.dbg-search-open svg { display: block; flex: none; }
  #debugbar button.dbg-search-open:not(.on):hover:not(:disabled) { background: #1d302a; border-color: #fff3b0; }
  #debugbar button.dbg-search-open.on { color: #10140f; background: var(--gold-bright, #e7c873); border-color: transparent; }
  #debugbar button.dbg-search-open.on:hover:not(:disabled) { background: #f3d98c; border-color: transparent; }
  #debugbar button.dbg-search-open.dbg-live .dbg-count { color: inherit; }
  #debugbar.dbg-tight button.dbg-search-open { padding: 0 9px; }
  /* The panel is the box's own and not the deck's, as what the box asks is,
     so a split whose halves are unlocked shows it too. */
  #debugbar .dbg-search {
    position: absolute; z-index: 4; left: 8px; top: calc(100% + 6px);
    width: min(396px, calc(100vw - 16px)); max-height: calc(100vh - 96px); overflow-y: auto; scrollbar-width: thin;
    display: grid; gap: 9px; padding: 12px; white-space: normal;
    border-radius: 14px; border: 1px solid var(--gold-bright, #e7c873); background: #0a1512fa; box-shadow: var(--shadow);
  }
  #debugbar .dbg-search h4 { margin: 0; font: 600 15px Georgia, serif; color: var(--gold-bright, #e7c873); }
  #debugbar .dbg-search p { margin: 0; color: var(--muted); line-height: 1.35; }
  #debugbar .dbg-search p b { color: var(--text); font-weight: 600; }
  #debugbar .dbg-search button.dbg-search-shut {
    position: absolute; z-index: 1; right: 10px; top: 9px; min-width: 0; width: 22px; height: 22px; padding: 0; border-radius: 50%;
  }
  #debugbar .dbg-search-set { display: grid; grid-template-columns: auto minmax(0, 1fr); align-items: center; gap: 6px 10px; }
  #debugbar .dbg-search-set input[type=text], #debugbar .dbg-search-term input {
    min-width: 0; height: 26px; padding: 0 6px; border-radius: 8px; color: var(--text); background: #060d0b;
    border: 1px solid #35504a; font: inherit; font-variant-numeric: tabular-nums;
  }
  #debugbar .dbg-search-alpha { display: flex; align-items: center; gap: 6px; color: var(--text); }
  /* The four lists: a name, and under it a row for each term. */
  #debugbar .dbg-search-lists { display: grid; gap: 6px; }
  #debugbar .dbg-search-list { display: grid; grid-template-columns: 74px minmax(0, 1fr); align-items: start; gap: 8px; }
  #debugbar .dbg-search-list > span {
    padding-top: 6px; color: var(--muted); font-size: 10px; font-weight: 700; letter-spacing: .1em; text-transform: uppercase;
  }
  #debugbar .dbg-search-list > div { display: grid; gap: 4px; }
  #debugbar .dbg-search-list i { color: var(--muted); padding-top: 5px; }
  #debugbar .dbg-search-term { display: flex; align-items: center; gap: 4px; }
  #debugbar .dbg-search-term select { width: auto; flex: 1 1 auto; }
  #debugbar .dbg-search-term select[data-term="key"] { flex: 2 1 96px; }
  #debugbar .dbg-search-term button { flex: none; min-width: 0; width: 22px; height: 22px; padding: 0; border-radius: 50%; }
  #debugbar .dbg-search-times { color: var(--muted); }
  #debugbar .dbg-search button.dbg-search-add { justify-self: start; height: 22px; padding: 0 8px; font-size: 11px; }
  #debugbar .dbg-search-set > span { color: var(--muted); font-size: 10px; font-weight: 700; letter-spacing: .1em; text-transform: uppercase; }
  #debugbar .dbg-search select {
    min-width: 0; width: 100%; height: 26px; padding: 0 4px; border-radius: 8px; cursor: pointer;
    color: var(--text); background: #060d0b; border: 1px solid #35504a; font: inherit;
  }
  #debugbar .dbg-search select:disabled { opacity: .5; cursor: default; }
  #debugbar .dbg-search-line { display: flex; flex-wrap: wrap; align-items: center; gap: 6px; }
  #debugbar .dbg-search-line button { padding: 0 11px; font-weight: 700; }
  #debugbar .dbg-search button.dbg-search-go {
    height: 32px; padding: 0 16px; font-size: 13.5px;
    color: #10140f; background: var(--gold-bright, #e7c873); border-color: transparent;
  }
  #debugbar .dbg-search button.dbg-search-go.on { color: var(--text); background: #14231f; border-color: #e2574c; }
  #debugbar .dbg-search-status { flex: 1 1 120px; min-width: 0; color: var(--text); font-variant-numeric: tabular-nums; }
  /* Each core's place on the turns being searched: a row a core, the turns
     it holds of the line it is on as a bar, walked back and played on again. */
  #debugbar .dbg-search-cores { display: grid; gap: 2px; padding: 5px 6px; border-radius: 8px; background: #060d0b; border: 1px solid #22332e; }
  #debugbar .dbg-search-cores i { display: block; position: relative; height: 4px; border-radius: 2px; background: #1b2a26; }
  #debugbar .dbg-search-cores i b { position: absolute; left: 0; top: 0; bottom: 0; border-radius: 2px; background: #5c7c70; }
  #debugbar .dbg-search-cores i.busy b { background: var(--gold-bright, #e7c873); }
  #debugbar .dbg-search-ends { display: flex; justify-content: space-between; color: var(--muted); font-size: 10.5px; font-variant-numeric: tabular-nums; }
  /* What was found: the best line as the stretches it is made of, then the
     number it was asked for over the turns beside the Civ AI seated alone. */
  #debugbar .dbg-search-found { display: grid; gap: 7px; padding-top: 8px; border-top: 1px solid #22332e; }
  #debugbar .dbg-search-plan { display: flex; flex-wrap: wrap; gap: 4px; }
  #debugbar .dbg-search-plan span {
    padding: 2px 7px; border-radius: 6px; white-space: nowrap; font-variant-numeric: tabular-nums;
    color: var(--text); background: #14231f; border: 1px solid #35504a;
  }
  #debugbar .dbg-search-plan span b { color: var(--gold-bright, #e7c873); }
  #debugbar .dbg-search-chart { position: relative; }
  #debugbar .dbg-search-chart svg { display: block; width: 100%; height: auto; border-radius: 8px; background: #060d0b; border: 1px solid #22332e; }
  #debugbar .dbg-search-chart text { font: 10.5px system-ui, sans-serif; fill: #9fb3ab; font-variant-numeric: tabular-nums; }
  #debugbar .dbg-search-chart text.dbg-value { fill: var(--text, #e8e3d2); font-weight: 700; }
  #debugbar .dbg-search-key { display: flex; flex-wrap: wrap; gap: 4px 14px; color: var(--muted); }
  #debugbar .dbg-search-key span { display: inline-flex; align-items: center; gap: 6px; }
  #debugbar .dbg-search-key i { display: block; width: 14px; height: 0; border-top: 2px solid; border-radius: 1px; }
  #debugbar .dbg-search-tip {
    position: absolute; z-index: 1; top: 4px; padding: 4px 8px; border-radius: 8px; pointer-events: none; white-space: nowrap;
    color: var(--muted); border: 1px solid #4d685f; background: #0a1512f5; font-variant-numeric: tabular-nums;
  }
  #debugbar .dbg-search-tip div { display: flex; align-items: center; gap: 6px; }
  #debugbar .dbg-search-tip div i { display: block; width: 10px; height: 0; border-top: 2px solid; }
  #debugbar .dbg-search-tip b { color: var(--text); }
  #debugbar .dbg-search-table {
    display: grid; grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr) auto; gap: 3px 8px; font-variant-numeric: tabular-nums;
  }
  #debugbar .dbg-search-table > * { overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
  #debugbar .dbg-search-table .dbg-v { text-align: right; color: var(--text); padding-right: 8px; }
  #debugbar .dbg-search-table .dbg-top { color: var(--gold-bright, #e7c873); font-weight: 700; }
`;

// The bar, once built, and each lane of its timeline: what is drawn on it
// (`debugLaneModels`) and the elements it is drawn with (`debugLanes`). The
// lanes are "" for the world, and "a" and "b" for the halves of a split.
let debugBar = null;
const debugLanes = {}, debugLaneModels = {};
// A slider's thumb is 14px wide; its centre stops this far short of each end.
const DEBUG_THUMB_HALF = 7;
// A box no wider than this has no room for the tools beside the turn
// buttons, and stacks them. One half of a split has no timeline to stand
// under, and stacks sooner.
const DEBUG_STACKED_UNDER = 720, DEBUG_PANE_STACKED_UNDER = 1000;
// The tools at each end may wrap to two rows of 26px buttons, one row as
// deep as Auto-play's 42px; taller than this, they have wrapped to three,
// and the words on them go.
const DEBUG_DECK_ROWS = 86;
// What Auto-play offers, in the order the picker lists it: a number of turns,
// Era (until a new era begins) and Indefinite (until it is stopped, a
// breakpoint stops it or the game ends). An era is held open 27 to 40 turns
// on Online, the stock speed, and longer on a slower one (`ERA_MINIMUM_TURNS`
// and `ERA_MAXIMUM_TURNS` in src/game.rs, scaled by the game's speed), so Era
// stands after 20 turns.
const DEBUG_AUTOPLAY_CHOICES = [
  ["1", "1 turn"], ["3", "3 turns"], ["5", "5 turns"], ["10", "10 turns"], ["20", "20 turns"],
  ["era", "Era"], ["100", "100 turns"], ["indefinite", "Indefinite"]];
// What Auto-play is set to when the box is built.
const DEBUG_AUTOPLAY_PICK = "era";
// One request at a time: every debugger operation replaces the world.
let debugBusy = false;
// A run is a loop of single steps; the Auto-play button is also its Stop.
// A run to a turn has a goal, which the timeline shows, and the button
// counts the turns played of the turns asked for (null: until stopped).
let debugRunning = false, debugStopAsked = false, debugGoal = null;
let debugPlayed = 0, debugAsked = null;
let debugPopOpen = null;
let debugNoteText = "", debugNotePos = null;
// The lane whose slider is being dragged, or null. A dragged slider is left
// where the hand has it until the hand lets go.
let debugScrubbing = null, debugSliderHeld = false;
// Which half of a split this page is: "a-…" or "b-…" inside the host's frame,
// and null for an ordinary page. A half's address is `?pane#<name>`
// (`debugPaneAddress`); `?pane=<name>` is read too.
const DEBUG_PANE = (() => {
  try {
    const asked = new URLSearchParams(location.search);
    const pane = asked.get("pane") || (asked.has("pane") ? decodeURIComponent(location.hash.slice(1)) : "");
    return pane && window.parent !== window ? pane : null;
  } catch (_) { return null; }
})();
// What is moving this pane's world right now, so its report to the host can
// say how the turn was reached: walked to, or played after the other pane.
// `debugMirroring` is one thing done because the other half did it.
let debugTravelling = false, debugFollowing = false, debugMirroring = false;
// A half of a split begins locked to the other, as the split does: the host
// says so again when the lock is opened or closed (`pane-lock`). A locked
// half puts its own deck away and tells the host what was on it, and the
// host's deck is the one panel for both.
let debugPaneLocked = true;
// The other half's name, while the two are locked and seat the same Civ AI.
// A request that names it is played once for both when the engine finds the
// two to be the same world (`src/server/panes.rs`), and the answer says
// whether it was (`debug.twinned`).
let debugTwin = null, debugTwinned = false;

function debugTimeline() {
  return state && !SPEC && state.debug ? state.debug : null;
}
function debugSay(text, pos = null) {
  debugNoteText = text || "";
  debugNotePos = pos;
  drawDebugBar();
}
// Standing orders the page keeps for the person — a unit walking somewhere, a
// city's queue — act by themselves when a turn opens. A turn that was walked
// to is shown as it stood, so they are dropped rather than re-issued into it.
function debugForgetStandingOrders(turn) {
  unitOrders.clear();
  cityQueues.clear();
  saveCityQueues();
  noticeLog = [];
  noticeDismissed = new Set();
  noticeSeenTurn = Math.max(0, turn - 1);
  // The banners compare each turn with the one before it (`rivalMoments`,
  // `boostMoments` in app_moments.js). A turn walked to is not the turn
  // after the one that was left, so they take attendance again: going back
  // from a war to before it began used to announce a peace.
  // The banners still up or waiting are of the turn that was left, and
  // go with it.
  if (typeof rivalsWorld !== "undefined") rivalsWorld = null;
  if (typeof boostsWorld !== "undefined") boostsWorld = null;
  if (typeof momentQueue !== "undefined") momentQueue.length = 0;
  if (typeof momentsShowing !== "undefined") {
    for (const card of document.querySelectorAll("#moments .moment")) card.remove();
    momentsShowing = 0;
  }
}
const DEBUG_TRAVEL_OPS = new Set(["back", "forward", "jump", "walk", "restart", "undo", "copy", "goto_save"]);
// `run` is a run's own: the request it has out for its next turn (`ahead`)
// and whether an answer leaves it another turn to play (`more`). A run asks
// for the next turn the moment a turn is answered and draws that turn while
// the engine plays, where it used to draw first and leave the engine with
// nothing to do for as long as drawing took (17 to 30 ms of every turn).
async function debugOp(body, timeoutMs = 20000, run = null) {
  const ahead = run ? run.ahead : null;
  if ((debugBusy && !ahead) || turnAdvancePending || !debugTimeline()) return null;
  debugBusy = true;
  drawDebugBar();
  try {
    const before = state.turn;
    debugTravelling = DEBUG_TRAVEL_OPS.has(body.op);
    // What a half does of its own accord it does for its twin as well. What
    // it does because the other half did is the other half's to have said.
    const led = DEBUG_PANE !== null && !debugFollowing && !debugMirroring;
    const ask = () => fetchJSON("/debug", {
      method: "POST",
      headers: {"Content-Type": "application/json"},
      body: JSON.stringify(led && debugTwin ? {...body, twin: debugTwin} : body),
    }, timeoutMs);
    if (run) run.ahead = null;
    const next = await (ahead || ask());
    if (next.error) { debugSay(next.error); return next; }
    if (run && run.more(next)) run.ahead = ask();
    debugTwinned = led && !!(next.debug && next.debug.twinned);
    if (debugHolding && next.turn !== before) {
      // Shown, a half may be asked in the same breath to play its world on
      // ahead (`debugPlayAhead`): the engine does that while this page draws
      // the turn, where it used to wait for the drawing to be done.
      const ahead = await debugHoldTurn(next.turn);
      if (ahead > 0) debugPlayAhead(ahead, next);
    }
    if (DEBUG_TRAVEL_OPS.has(body.op) || body.op === "step" || body.op === "step_action" ||
        body.op === "step_seat" || body.op === "finish_round") {
      sel = null; selCity = null;
      if ((next.turn !== before || body.op === "copy" || body.op === "goto_save") && DEBUG_TRAVEL_OPS.has(body.op))
        debugForgetStandingOrders(next.turn);
    }
    render(next);
    // A turn that moved has been said (`debugPaneReport`). What left the
    // turn where it was is said here, for a locked other half to do too.
    if (led && next.turn === before)
      window.parent.postMessage({civvis: "pane-did", pane: DEBUG_PANE, body, twinned: debugTwinned}, location.origin);
    return next;
  } catch (error) {
    debugSay(`The debugger could not ${body.op}: ${error.message || error}`);
    return null;
  } finally {
    // A run with a turn asked for is still busy: nothing else is to be
    // asked of the engine in between.
    debugBusy = !!(run && run.ahead);
    debugTravelling = false;
    debugTwinned = false;
    drawDebugBar();
  }
}
// The engine has already made this half a copy of its twin, which was asked
// for both: only the page is behind, and reads the world it now holds.
async function debugRefresh() {
  if (debugBusy || !debugTimeline()) return;
  debugBusy = true;
  try {
    const before = state.turn;
    const next = await fetchJSON("/state", {}, 20000);
    if (!next || next.error) return;
    sel = null; selCity = null;
    if (next.turn !== before) debugForgetStandingOrders(next.turn);
    render(next);
  } catch (error) {
    debugSay(`The debugger could not read this half: ${error.message || error}`);
  } finally {
    debugBusy = false;
    drawDebugBar();
  }
}
// One action of this turn taken back. The note says so: what it said before
// was about an action that may be the one that is gone.
async function debugUndo() {
  const next = await debugOp({op: "undo"});
  if (!next || next.error) return;
  const acted = (next.debug || {}).acted || 0;
  debugSay(`Last action undone · ${acted ? `${acted} more can be undone` : "nothing left to undo"}`);
}
function debugStrategy() {
  return (document.getElementById("autoplaystrategy") || {}).value || undefined;
}
// A Civ AI by the name its picker shows.
function debugAgentName(id) {
  const roster = typeof RULES !== "undefined" && RULES && Array.isArray(RULES.strategies) ? RULES.strategies : [];
  const row = roster.find(entry => entry.name === id);
  return row ? row.username || row.name : id || "another Civ AI";
}
// Kept turns ahead that another Civ AI played (`debug.beyond`), in a line.
// They are not walked to: going on from here plays, with the one picked.
// What happens next is said first, because the box has little room.
function debugBeyondWords(beyond) {
  const turns = beyond.from === beyond.to ? `turn ${beyond.from} is` : `turns ${beyond.from} to ${beyond.to} are`;
  return `${debugAgentName(debugStrategy())} plays on from here · ${turns} by ${debugAgentName(beyond.by)}`;
}
// The page's choice of Civ AI is kept where a page reads it when it loads,
// which is also how the halves of a split start on it.
function debugRememberSeat() {
  const name = debugStrategy();
  if (!name || DEBUG_PANE !== null) return;
  try { localStorage.setItem(AUTOPLAY_STRATEGY_KEY, name); } catch (_) {}
}
// The Civ AI picked for the seat is said to the engine when it is picked, not
// at the next step. The turns the engine counts as played ahead are the ones
// that agent played (`Timeline::line`), so the timeline and every way forward
// on it are the picked agent's from the moment of the pick: a turn another
// Civ AI played is never walked to as if it were this one's.
function debugSeat() {
  debugRememberSeat();
  const name = debugStrategy();
  return name && debugTimeline() ? debugOp({op: "status", strategy: name}) : null;
}
// ---- Breakpoints as lines on the timeline
//
// Operator, 2026-11-27: "breakpoints should add a vertical red line through
// the spot on the timeline. like a debugger. for uncretain lines, like future
// eras, we can make the line dashed. label lines with turn and/or era or
// whatever. a nuclear war breakpoint could have a dashed line too. actually
// we can just compute these out so you should be able to figure out the turn
// from it".
//
// A turn breakpoint is a solid line at its turn, named by the turn and the
// era that turn falls in. The era breakpoint is a solid line at each era the
// line has begun, and a dashed one at each era the game's future begins; the
// nuclear breakpoint a dashed one at the turn the future first sees a device
// go off. The future is the engine's own (`foresee`, `debug_foresee` in
// `src/server/debugger.rs`): a copy of the world at the end of the line,
// played on by the same Civ AI to the end of the game with nothing kept but
// what it saw. The engine is deterministic, so that is the turn the line
// itself reaches it on for as long as it is played the same way; it is
// dashed because a turn played another way, or another Civ AI, makes it
// another future, which is then asked for anew.
//
// The future is asked for only on the one timeline, while the era or the
// nuclear breakpoint is set, and only while nothing else is asked of the
// engine: a stretch of turns at a time, about `DEBUG_SIGHT_MS` of the
// engine's time each, so a click is never kept waiting long behind it.
const DEBUG_SIGHT_MS = 250;
let debugSight = null, debugSightOut = false, debugSightAfter = 0;
function debugSightWanted(timeline) {
  const breaks = (timeline || {}).breakpoints || {};
  return !!timeline && DEBUG_PANE === null && !debugSplit && (breaks.eras === true || breaks.nuclear === true);
}
// Which future a line has: the end of the line, the print of its world there
// and the Civ AI that plays on from it. Another of any is another future.
function debugSightKey(timeline) {
  const worlds = timeline.worlds || [];
  return JSON.stringify([timeline.last ?? null, worlds.length ? worlds[worlds.length - 1] : null,
    timeline.seat ?? null, debugStrategy() || null]);
}
async function debugForesee() {
  const timeline = debugTimeline();
  if (!debugSightWanted(timeline) || debugSightOut || debugBusy || debugRunning || turnAdvancePending) return;
  if (performance.now() < debugSightAfter) return;
  const key = debugSightKey(timeline);
  if (debugSight && debugSight.key !== key) debugSight = null;
  if (debugSight && debugSight.done) return;
  const sight = debugSight, strategy = debugStrategy();
  const turns = !sight ? 4 : Math.max(1, Math.min(40, Math.round(DEBUG_SIGHT_MS / Math.max(1, sight.pace))));
  debugSightOut = true;
  const since = performance.now();
  let told = null;
  try {
    told = await fetchJSON("/debug", {method: "POST", headers: {"Content-Type": "application/json"},
      body: JSON.stringify({op: "foresee", turns, lean: true, ...(strategy ? {strategy} : {}),
        ...(sight ? {known: sight.turn} : {anew: true})})}, 20000);
  } catch (_) { told = null; }
  debugSightOut = false;
  const seen = told && !told.error ? told.foresight : null;
  // An engine that cannot say is asked again in a while; nothing is said.
  if (!seen || typeof seen.turn !== "number") { debugSightAfter = performance.now() + 15000; return; }
  const now = debugTimeline();
  if (!now || debugSightKey(now) !== key) { drawDebugBar(false); return; }
  let next = debugSight;
  if (!next || seen.anew === true || typeof seen.since !== "number") next = {key, seen: [], eras: [], pace: 50};
  else next.seen = next.seen.filter(event => event.turn < seen.since);
  next.seen.push(...(Array.isArray(seen.seen) ? seen.seen : []).filter(event => event && typeof event.turn === "number"));
  Object.assign(next, {turn: seen.turn, from: seen.from, done: seen.done === true,
    eras: Array.isArray(seen.eras) ? seen.eras : next.eras});
  if (seen.played > 0) next.pace = (performance.now() - since) / seen.played;
  debugSight = next;
  drawDebugBar(false);
}
// Two names that would overlap are set one under the other: a line's name
// takes the second row when it would run into the name before it on the
// first. `along` is how far along the lane a turn stands, in pixels; a lane
// not yet measured keeps them all on the first.
function debugLineRows(lines, along) {
  const ends = [-Infinity, -Infinity];
  return lines.map(line => {
    const from = along(line.turn) + 3, wide = 12 + 6.2 * [...line.label].length;
    const row = from >= ends[0] ? 0 : from >= ends[1] ? 1 : 0;
    ends[row] = Math.max(ends[row], from + wide + 4);
    return [line, row];
  });
}
// The lines on the one timeline: {turn, ahead, label, title}, one a turn.
function debugBreakLines(timeline, first, last) {
  const breaks = timeline.breakpoints || {};
  const sight = debugSight && debugSight.key === debugSightKey(timeline) ? debugSight : null;
  const begun = timeline.eras || [];
  const coming = sight ? sight.eras.filter(era => era.turn > last) : [];
  const eraOn = turn => [...begun, ...coming].filter(era => era.turn <= turn).sort((one, other) => one.turn - other.turn).pop() || null;
  const lines = new Map();
  const put = (turn, ahead, label, title) => {
    const had = lines.get(turn);
    if (!had) { lines.set(turn, {turn, ahead, label, title}); return; }
    // A set turn is certain whatever else falls on it.
    had.ahead = had.ahead && ahead;
    if (!had.label.includes(label)) had.label += ` · ${label}`;
    had.title += `; ${title}`;
  };
  for (const turn of breaks.turns || []) {
    const era = eraOn(turn);
    put(turn, false, `T${turn}${era ? ` · ${debugEraName(era)}` : ""}`, `Breakpoint on turn ${turn}`);
  }
  if (breaks.eras === true) {
    for (const era of begun.filter(era => era.turn > first))
      put(era.turn, false, `T${era.turn}`, `The ${debugEraName(era)} era began on turn ${era.turn}`);
    for (const era of coming)
      put(era.turn, true, `${debugEraName(era)} ~T${era.turn}`,
        `The ${debugEraName(era)} era begins on turn ${era.turn} if this line is played on as it is (the game's future, worked out)`);
  }
  if (breaks.nuclear === true && sight) {
    const blast = sight.seen.find(event => event.kind === "nuclear" && event.turn > last);
    if (blast)
      put(blast.turn, true, `☢ ~T${blast.turn}`,
        `${blast.text || "A nuclear device goes off"} on turn ${blast.turn}${blast.era_name ? `, in the ${titleCase(blast.era_name)} era` : ""}, if this line is played on as it is`);
  }
  return [...lines.values()].sort((one, other) => one.turn - other.turn);
}
// Why the engine stopped a run, in a line, with somewhere to look if it has one.
function debugSayStop(stopped) {
  if (!stopped) return;
  if (stopped.kind === "turn") debugSay(`Stopped at turn ${stopped.turn}`);
  else if (stopped.kind === "era") {
    const era = ((debugTimeline() || {}).eras || []).find(began => began.era === stopped.era) || {era: stopped.era};
    debugSay(`Stopped on turn ${stopped.turn}: the ${debugEraName(era)} era began`);
  }
  else if (stopped.kind === "nuclear") debugSay(`Stopped on turn ${stopped.turn}: a nuclear device went off`);
  else if (stopped.kind === "event")
    debugSay(`Stopped on ${stopped.category}: ${stopped.text}`, stopped.pos || null);
  else if (stopped.kind === "condition") {
    const who = state.players.find(player => player.id === stopped.seat) || {};
    const name = titleCase(String(who.civ || who.name || `seat ${stopped.seat}`));
    const read = value => typeof value === "number" ? String(Math.round(value * 100) / 100) : JSON.stringify(value);
    const what = `${name} ${stopped.key.replaceAll("_", " ")}`;
    debugSay(stopped.test === "changes"
      ? `Stopped on turn ${stopped.turn}: ${what} changed from ${read(stopped.was)} to ${read(stopped.now)}`
      : `Stopped on turn ${stopped.turn}: ${what} is ${read(stopped.now)} (${stopped.test} ${read(stopped.value)}, was ${read(stopped.was)})`);
  }
  else debugSay(`The game ended on turn ${stopped.turn}`);
}
async function debugStep() {
  if (autoplaying) return null;
  debugSay("");
  autoplaying = true;
  try {
    const next = await debugOp({op: "step", strategy: debugStrategy()}, AUTOPLAY_BATCH_TIMEOUT_MS);
    if (next && !next.error) debugSayStop((next.debug || {}).stopped);
    return next;
  } finally {
    autoplaying = false;
    drawDebugBar();
    drawTurnLoop();
    currentPolicyLedger();
  }
}
// One action of the agent's turn in a few words, and where on the map it was.
function debugActionWords(action) {
  const words = [titleCase(String(action.type || ""))];
  const unit = action.unit === undefined ? null : state.units.find(held => held.id === action.unit);
  if (unit) words.push(titleCase(String(unit.type || "")));
  const city = action.city === undefined ? null : state.cities.find(held => held.id === action.city);
  if (city) words.push(city.name);
  // What was chosen is a name, sometimes one level down (`item: {unit: …}`).
  const named = value => typeof value === "string" ? [value]
    : value && typeof value === "object" && !Array.isArray(value)
      ? Object.values(value).filter(inner => typeof inner === "string") : [];
  for (const [key, value] of Object.entries(action))
    if (key !== "type") words.push(...named(value).map(titleCase));
  return words.filter(Boolean).join(" · ");
}
function debugActionPlace(action) {
  for (const key of ["to", "target", "pos"])
    if (Array.isArray(action[key]) && action[key].length === 2) return action[key];
  const unit = action.unit === undefined ? null : state.units.find(held => held.id === action.unit);
  if (unit) return unit.pos;
  const city = action.city === undefined ? null : state.cities.find(held => held.id === action.city);
  return city ? city.pos : null;
}
// Step in: one action of the agent's turn, named in the bar and centred on
// the map, so a turn can be watched being put together.
async function debugStepAction() {
  if (autoplaying) return null;
  debugSay("");
  autoplaying = true;
  try {
    const next = await debugOp({op: "step_action", strategy: debugStrategy()}, AUTOPLAY_BATCH_TIMEOUT_MS);
    const stepped = next && !next.error ? (next.debug || {}).stepped : null;
    if (stepped && stepped.seat !== undefined) debugSayRound(stepped);
    else if (stepped) {
      const place = debugActionPlace(stepped.action);
      const left = stepped.left ? ` · ${stepped.left} more this turn` : "";
      debugSay(`Civ AI: ${debugActionWords(stepped.action)}${left}`, place);
      if (place) centerOn(place);
    }
    return next;
  } finally {
    autoplaying = false;
    drawDebugBar();
    drawTurnLoop();
    currentPolicyLedger();
  }
}
// Step one seat: the round, a civilization at a time. On your own turn it
// ends the turn as it stands and stops before any rival has moved; after
// that each step plays whoever is up. It is not the Civ AI playing your
// seat, so a linked half follows it as an ended turn, not as a step.
function debugSeatName(seat) {
  const who = state.players.find(player => player.id === seat);
  return who ? titleCase(String(who.civ || who.name || `seat ${seat}`)) : `seat ${seat}`;
}
// Who is up, as a clause: every seat is one civilization but the Barbarians.
function debugPlaysNext(seat) {
  const who = state.players.find(player => player.id === seat);
  return `${debugSeatName(seat)} ${who && who.is_barbarian ? "play" : "plays"} next`;
}
function debugSayRound(stepped) {
  const did = stepped.seat === 0 ? "Your turn ended"
    : `${debugSeatName(stepped.seat)} played ${stepped.actions} action${stepped.actions === 1 ? "" : "s"}`;
  debugSay(stepped.next === 0 ? `${did} · turn ${state.turn} is yours`
    : `${did} · ${debugPlaysNext(stepped.next)}`);
}
async function debugStepSeat() {
  if (autoplaying) return null;
  debugSay("");
  const next = await debugOp({op: "step_seat"}, AUTOPLAY_BATCH_TIMEOUT_MS);
  const stepped = next && !next.error ? (next.debug || {}).stepped : null;
  if (stepped && stepped.seat !== undefined) debugSayRound(stepped);
  return next;
}
// Played a turn a request, like auto-play, so every turn reaches the glass
// and the Stop button is never more than one turn away. `era` is a world era
// to play out of (Era Next): the run stops on the first turn of a new one.
//
// Turns ahead that the seated Civ AI has already played from the open turn
// are not played again (`debug.played_ahead`, the last of them): playing
// them would only arrive at the turns that are kept. The run first asks the
// engine to `walk` to the latest of them it reaches, which stops where a
// breakpoint would have stopped the run that played them, and the Civ AI
// plays on from there. Forward 50, back 20, forward 30 plays ten turns.
async function debugRun(until = null, era = null) {
  if (debugRunning) { debugStopAsked = true; drawDebugBar(); return; }
  if (autoplaying || !debugTimeline()) return;
  debugRunning = true; debugStopAsked = false; autoplaying = true; debugGoal = until;
  debugPlayed = 0; debugAsked = until === null ? null : Math.max(1, until - state.turn);
  debugSay("");
  let played = 0;
  // Whether the turn just answered leaves the run another to play. The next
  // is asked for on this answer, before the turn is drawn (`debugOp`), so a
  // Stop pressed while a turn is being drawn takes effect a turn later: the
  // engine was already playing it.
  const dawn = next => era !== null && next.world_era !== undefined && next.world_era !== era;
  const run = {ahead: null, more: next => !debugStopAsked && !(next.debug || {}).stopped &&
    !(until !== null && next.turn >= until) && !dawn(next)};
  // Whether the turn just answered is where the run ends, and the bar told why.
  const over = next => {
    const stopped = (next.debug || {}).stopped;
    if (stopped) debugSayStop(stopped);
    else if (until !== null && next.turn >= until) debugSay(`Played to turn ${next.turn}`);
    else if (dawn(next)) debugSayStop({kind: "era", turn: next.turn, era: next.world_era});
    else return false;
    return true;
  };
  try {
    const from = state.turn;
    let on = true;
    if ((debugTimeline().played_ahead ?? from) > from) {
      const next = await debugOp({op: "walk", turn: until, era: era !== null, strategy: debugStrategy()},
        AUTOPLAY_BATCH_TIMEOUT_MS);
      on = !!next && !next.error;
      if (on) {
        played = next.turn - from;
        debugPlayed = played;
        on = !over(next) && !debugStopAsked;
      }
    }
    if (on) do {
      const next = await debugOp({op: "step", strategy: debugStrategy()}, AUTOPLAY_BATCH_TIMEOUT_MS, run);
      if (!next || next.error) break;
      played += 1;
      debugPlayed = played;
      if (over(next)) break;
      await new Promise(drawn => requestAnimationFrame(drawn));
    } while (run.ahead);
    if (debugStopAsked) debugSay(`Stopped by hand after ${played} turn${played === 1 ? "" : "s"}`);
  } finally {
    // A turn asked for and never taken (drawing the one before it failed) has
    // still been played: the page reads the world it left.
    if (run.ahead) {
      const late = await run.ahead.catch(() => null);
      debugBusy = false;
      if (late && !late.error) render(late);
    }
    debugRunning = false; debugStopAsked = false; autoplaying = false; debugGoal = null;
    drawDebugBar();
    drawTurnLoop();
    // The deck is not priced while a turn is being played (`currentPolicyLedger`
    // in app.js); the turn the run ended on is the one to price. A single
    // step asks the same way, above.
    currentPolicyLedger();
    if (DEBUG_PANE !== null) window.parent.postMessage({civvis: "pane-ran", pane: DEBUG_PANE}, location.origin);
  }
}
// Auto-play: the seat's Civ AI plays the number of turns picked from the
// turn that is open, until a new era begins (Era; in the last era, the game
// out), or until it is stopped (Indefinite: every turn the game has left),
// and a breakpoint stops it early. Pressed while anything is being played,
// it stops that.
function debugAutoPlay() {
  if (debugRunning) { debugStopAsked = true; drawDebugBar(); return; }
  if (!debugTimeline()) return;
  const pick = document.getElementById("dbg-turns").value || DEBUG_AUTOPLAY_PICK;
  if (pick === "era") { debugRun(null, state.world_era ?? 0); return; }
  debugRun(pick === "indefinite" ? null : state.turn + Math.max(1, parseInt(pick, 10) || 1));
}
// The turn buttons walk the turns already played, and past the last of them
// the Civ AI plays on. Going on does not play over what was played: it walks
// to the last turn first. (Dragging the timeline ahead from an earlier turn
// plays on from that turn, over what was played after it; it says so under
// the pointer. Auto-play does the same. Neither plays again a turn the
// seated Civ AI already played from there: see `debugRun`.)
//
// The walk: along the turns already played towards `goal` (null, the last of
// them), and whether the Civ AI is to play on from where it stopped, which is
// when it reached the last turn played short of the goal.
async function debugWalkOn(goal) {
  const timeline = debugTimeline(), from = state.turn;
  const last = Math.max(timeline.last ?? from, from);
  if (from >= last) return true;
  const to = goal === null ? last : Math.min(goal, last);
  let next = await debugOp(to === from + 1 ? {op: "forward"} : {op: "jump", turn: to});
  // A turn that is no longer kept answers with the nearest before it,
  // which may be this one: the next one that is kept is the way on.
  if (next && !next.error && next.turn <= from) next = await debugOp({op: "forward"});
  return !!next && !next.error && next.turn >= last && (goal === null || next.turn < goal);
}
async function debugAhead(turns) {
  if (!debugTimeline() || debugBusy || autoplaying) return;
  debugSay("");
  const goal = state.turn + turns;
  if (!await debugWalkOn(goal)) return;
  if (goal - state.turn <= 1) await debugStep();
  else await debugRun(goal);
}
// Play to End: the turns already played, and from the last of them the Civ
// AI plays the game out. A breakpoint stops it, and so does Stop.
async function debugToEnd() {
  if (!debugTimeline() || debugBusy || autoplaying) return;
  debugSay("");
  if (await debugWalkOn(null)) await debugRun(null);
}
// Era Next: the first turn of the next era. One already played is gone to;
// past the last turn played the Civ AI plays on until a new era begins.
async function debugEraOn() {
  const timeline = debugTimeline();
  if (!timeline || debugBusy || autoplaying) return;
  debugSay("");
  const began = debugEraNext(timeline);
  if (began) { await debugGoToTurn(began.turn); return; }
  if (await debugWalkOn(null)) await debugRun(null, state.world_era ?? 0);
}
async function debugBehind(turns) {
  const timeline = debugTimeline();
  if (!timeline || debugBusy || autoplaying) return;
  debugSay("");
  if (turns === 1) { await debugOp({op: "back"}); return; }
  await debugOp({op: "jump", turn: Math.max(timeline.first ?? state.turn, state.turn - turns)});
}
// Era Back: the first turn of the era the open turn is in, and from there
// the first turn of the era before. `eras` is the engine's: the first kept
// turn of each era the timeline passes through.
function debugEraBack(timeline) {
  const began = (timeline.eras || []).filter(era => era.turn < state.turn);
  return began.length ? began[began.length - 1] : null;
}
// Era Next: the first kept turn of the era after the open one, when the
// turns already played reach it.
function debugEraNext(timeline) {
  return (timeline.eras || []).find(era => era.turn > state.turn) || null;
}
// An era's name, as the page writes it.
function debugEraName(era) {
  return titleCase(String(era.name || ERA_NAMES[era.era] || `era ${era.era}`));
}
// Go to a turn, like a debugger's "run to here": a turn that was played is
// put back as it stood, and a turn still ahead is played to.
async function debugGoToTurn(text) {
  const timeline = debugTimeline(), turn = parseInt(text, 10);
  if (!timeline || !Number.isFinite(turn) || turn === state.turn) { drawDebugBar(false); return; }
  const first = timeline.first ?? state.turn, last = Math.max(timeline.last ?? state.turn, state.turn);
  if (turn > last) { debugRun(turn); return; }
  const next = await debugOp({op: "jump", turn: Math.max(turn, first)});
  if (next && !next.error && next.turn !== turn && turn >= first)
    debugSay(`Turn ${turn} is no longer kept. This is turn ${next.turn}, the nearest before it`);
}
// A save point is the world as it stands, kept beside the timeline with the
// turns behind it (`save_point`). Unlike a save it is gone with the page.
async function debugSavePoint() {
  const box = document.querySelector("#dbg-pop-saves [data-dbg-field=pointname]");
  const next = await debugOp({op: "save_point", name: (box.value || "").trim()});
  if (!next || next.error) return;
  box.value = "";
  debugSay(`Save point set on turn ${next.turn}`);
}
// A phone closes a tab long before a desktop does, so the engine is told to
// keep fewer turns there (`room`). Asked again if a new game forgot it.
const DEBUG_SMALL_DEVICE = (navigator.deviceMemory || 8) <= 4 || /iPhone|iPad|Android/.test(navigator.userAgent);
let debugRoomAskedAt = 0;
// A save is the world at one turn, kept under a name. The list is this
// server's or this browser's own (`/saves`), the same one the deck shows.
async function debugSave() {
  const box = document.querySelector("#dbg-pop-saves [data-dbg-field=savename]");
  const stamp = new Date();
  const name = (box.value || "").trim() ||
    `t${state.turn}-${String(stamp.getHours()).padStart(2, "0")}${String(stamp.getMinutes()).padStart(2, "0")}`;
  try {
    const done = await fetchJSON("/save", {method: "POST",
      headers: {"Content-Type": "application/json"}, body: JSON.stringify({name})});
    if (done.error) throw new Error(done.error);
    box.value = "";
    debugSay(`Saved turn ${state.turn} as ${name}`);
    refreshSaves();
    debugDrawSaves();
  } catch (error) {
    debugSay(`Could not save: ${error.message || error}`);
  }
}
async function debugDrawSaves() {
  const list = document.getElementById("dbg-save-list");
  let saves = [];
  try { saves = (await fetchJSON("/saves", {}, 4000)).saves || []; } catch (_) {}
  list.innerHTML = saves.length
    ? saves.slice(0, 10).map(save =>
        `<div class="dbg-line"><span style="flex:1;width:auto;color:inherit"><b>${escapeAttr(save.name)}</b> · turn ${escapeAttr(String(save.turn))}</span>` +
        `<button data-dbg-load="${escapeAttr(save.name)}">Load</button></div>`).join("")
    : `<p>Nothing saved yet.</p>`;
}

function debugBreakpointsFromForm() {
  const pop = document.getElementById("dbg-pop-breaks");
  const list = text => text.split(",").map(part => part.trim()).filter(Boolean);
  return {
    op: "breakpoints",
    turns: [...new Set(list(pop.querySelector("[data-dbg-field=turns]").value)
      .map(Number).filter(turn => Number.isInteger(turn) && turn > 0))],
    categories: [...pop.querySelectorAll("[data-dbg-category]")]
      .filter(box => box.checked).map(box => box.dataset.dbgCategory),
    phrases: list(pop.querySelector("[data-dbg-field=phrases]").value),
    important: pop.querySelector("[data-dbg-field=important]").checked,
    every_seat: pop.querySelector("[data-dbg-field=every_seat]").checked,
    eras: pop.querySelector("[data-dbg-field=eras]").checked,
    nuclear: pop.querySelector("[data-dbg-field=nuclear]").checked,
    // Conditions are added and removed one at a time; the engine holds the set.
    conditions: [...(debugTimeline().breakpoints.conditions || [])],
  };
}
function debugLayersFromForm() {
  return [...document.querySelectorAll("#dbg-pop-layers [data-dbg-layer]")]
    .filter(box => box.checked).map(box => box.dataset.dbgLayer);
}
// The Single player switch. `SOLO_UI` is the page's (app.js reads it in
// `civ6Frame` and `debuggerView`); this keeps the choice for the next visit
// and paints the page again in the view it now has. The Debugger view is for
// reading the game, so going to it puts the selection and the seat's own
// screens away; coming back picks up the next unit waiting for orders, as
// the top of a turn does.
function setSoloUi(on) {
  SOLO_UI = !!on;
  try { localStorage.setItem("civvis-debugger-view-v1", SOLO_UI ? "single" : "debugger"); } catch (_) {}
  if (!state || SPEC) return;
  if (!SOLO_UI) {
    sel = null; selCity = null;
    clearUnitIntel(); closeCityScreen(); closeEmpire(); closeDiplomacy(); closeTrade();
  }
  drawSoloHud(); drawLaunchBar(); drawTurnLoop(); drawPlayerHud();
  draw(); drawSide(); drawUbar();
  if (SOLO_UI && !sel && !selCity) advanceToNextActionUnit(true);
  drawDebugBar();
}
function debugTogglePop(name) {
  debugPopOpen = debugPopOpen === name ? null : name;
  drawDebugBar();
}

// One lane of the timeline: a rail as long as the game, what is drawn on it
// (the sky, and on it the ruler and the turns played), the marker and the
// line under the pointer, and the slider that is the hand on it.
function debugLaneMarkup(key, id, label) {
  return `
      <div class="dbg-lane${key ? " dbg-half" : ""}" data-dbg-lane="${key}">
        <b class="dbg-start"></b>
        <div class="dbg-rail">
          <div class="dbg-sky"></div>
          <div class="dbg-deco"><div class="dbg-ruler"></div><div class="dbg-band"></div><div class="dbg-band dbg-own"></div><div class="dbg-by"></div><div class="dbg-beyond" hidden></div><div class="dbg-kept-turns"></div><div class="dbg-goal"></div></div>${
          key ? `<div class="dbg-arm"><div class="dbg-arm-sky"></div><div class="dbg-bent"></div></div>` : `<div class="dbg-axis"></div>`}${key ? "" : `
          <div class="dbg-travel" aria-hidden="true" hidden><i>◀</i>Click to travel<i>▶</i></div>`}
          <div class="dbg-hands"><div class="dbg-ghost" hidden></div>${key ? "" : `<span class="dbg-fork" hidden></span>`}<div class="dbg-cursor"><b></b>${key ? "" : `
            <button class="dbg-branch" data-dbg="split" title="Branch: pick the Civ AI that plays a second timeline from this turn, beside this one. This one keeps its own Civ AI and the turns it holds past here; play both, and close either with its ✕" aria-label="Branch the timeline on this turn" aria-haspopup="dialog"><i>⑂</i>Branch</button>`}</div></div>
          <input type="range" id="${id}" step="1"${key ? ` data-dbg-half="${key}"` : ""} aria-label="${label}">
          <div class="dbg-marks"></div>
        </div>
        <span class="dbg-lane-name"${key ? ` title="${key === "a" ? "A: the timeline as it was when it was branched" : "B: the branch"}"` : ""}>${key.toUpperCase()}</span>${key ? `
        <select class="dbg-agent" data-dbg-agent="${key}" title="The Civ AI that plays ${key.toUpperCase()}'s seat when it is stepped, run or played to" aria-label="Civ AI for ${key.toUpperCase()}"></select>
        <button class="dbg-close" data-dbg-close="${key}" title="Close branch ${key.toUpperCase()}: ${key === "a" ? "B" : "A"} is the timeline from then on, with every turn it holds" aria-label="Close branch ${key.toUpperCase()}">✕</button>` : ""}
        <span class="dbg-finish"><i class="dbg-flag"></i><b></b></span>
      </div>`;
}
function debugBuildBar() {
  const style = document.createElement("style");
  style.textContent = DEBUG_BAR_STYLE;
  document.head.appendChild(style);
  const bar = document.createElement("div");
  bar.id = "debugbar";
  bar.hidden = true;
  bar.setAttribute("role", "toolbar");
  bar.setAttribute("aria-label", "Simulator debugger");
  bar.innerHTML = `
    <div class="dbg-time">
      <div class="dbg-who">
        <label class="dbg-field dbg-agent-field" title="The Civ AI that plays your seat from the open turn: when it is auto-played, stepped or played to"><span>Civ AI</span>
          <select class="dbg-agent" id="dbg-agent" aria-label="Civ AI for your seat"></select></label>
      </div>
      <div class="dbg-tie"><button class="dbg-lock" data-dbg-lock aria-pressed="true"><svg viewBox="0 0 16 18" width="15" height="17" aria-hidden="true">
        <path class="dbg-shackle dbg-shackle-shut" d="M4.5 8.5V5.5a3.5 3.5 0 0 1 7 0v3"/>
        <path class="dbg-shackle dbg-shackle-open" d="M4.5 8.5V4.5a3.5 3.5 0 0 1 7 0v.5"/>
        <rect x="2" y="8" width="12" height="9" rx="2" fill="currentColor"/></svg></button></div>${
      debugLaneMarkup("", "dbg-slider", "Turn, on the whole game")}${
      debugLaneMarkup("a", "dbg-slider-a", "Turn of simulator A")}${
      debugLaneMarkup("b", "dbg-slider-b", "Turn of simulator B")}
    </div>
    <div class="dbg-deck">
      <div class="dbg-side dbg-left">
        <div class="dbg-cluster">
          <select id="dbg-turns" title="How many turns Auto-play plays before it hands the seat back. Era is until a new era begins (in the last era, to the end of the game), and Indefinite is until it is stopped or the game ends" aria-label="Turns to auto-play">${
            DEBUG_AUTOPLAY_CHOICES.map(([pick, word]) =>
              `<option value="${pick}"${pick === DEBUG_AUTOPLAY_PICK ? " selected" : ""}>${word}</option>`).join("")
          }</select>
          <button data-dbg="autoplay" title="Auto-play: the Civ AI plays the turns picked, and stops early when a breakpoint does. While it plays this counts the turns, and pressing it stops" aria-label="Auto-play"><i id="dbg-run-sign">▶▶</i><span class="dbg-word" id="dbg-run-word">Auto-play</span><span id="dbg-run-count"></span></button>
          <button class="dbg-apart" data-dbg-pop="layers" title="Layers: which decisions the Civ AI takes for you each turn">Layers<span class="dbg-count" id="dbg-layer-count"></span></button>
          <button data-dbg-pop="breaks" title="Breakpoints: what stops Auto-play and a turn played to" aria-label="Breakpoints"><span class="dbg-dot">●</span><span class="dbg-word">Breakpoints</span><span class="dbg-count" id="dbg-break-count"></span></button>
          <button class="dbg-search-open dbg-apart" data-dbg-search="open" aria-expanded="false" title="Autosearch: from the open turn, find which Civ AIs should hold the seat, and when, to make the most of a number you pick. A share of this computer's cores each plays futures up and down the timeline" aria-label="Autosearch"><svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true"><circle cx="6.5" cy="6.5" r="4.5" fill="none" stroke="currentColor" stroke-width="2"/><path d="M10 10l4.5 4.5" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"/></svg><span class="dbg-word">Autosearch</span><span class="dbg-count" id="dbg-search-count"></span></button>
        </div>
        <span class="dbg-group dbg-nav">
          <button data-dbg="first" title="Turn 1: go to where the timeline begins" aria-label="Go to the first turn"><i class="dbg-sign">⏮</i><span class="dbg-word">Turn 1</span></button>
          <button data-dbg="era" title="Era Back: back to the first turn of this era, and from there to the first turn of the era before" aria-label="Era Back"><i>◀◀</i>Era</button>
          <button data-dbg="back10" title="Back 10 turns" aria-label="Back 10 turns"><i>◀◀</i>10</button>
          <button data-dbg="back" title="Back 1 turn: the top of this turn, then the turn before" aria-label="Back 1 turn"><i>◀</i>1</button>
        </span>
      </div>
      <label class="dbg-turnbox" title="The turn that is open. Type a turn to go to it: one already played is put back as it stood, one still ahead is played to by the Civ AI. Up and down walk one played turn"><span class="dbg-word">TURN</span>
        <input type="text" id="dbg-turn" inputmode="numeric" maxlength="4" aria-label="Go to turn"><span id="dbg-limit"></span></label>
      <div class="dbg-side dbg-right">
        <span class="dbg-group dbg-nav">
          <button data-dbg="next" title="Next turn: forward along the turns already played, and past the last of them the Civ AI plays one" aria-label="Next turn">1<i>▶</i></button>
          <button data-dbg="next10" title="10 turns on: forward along the turns already played, and past the last of them the Civ AI plays the rest" aria-label="10 turns on">10<i>▶▶</i></button>
          <button data-dbg="era-next" title="Era Next: on to the first turn of the next era, along the turns already played, and past the last of them the Civ AI plays until a new era begins" aria-label="Era Next">Era<i>▶▶</i></button>
          <button data-dbg="end" title="Play to End: along the turns already played, and from the last of them the Civ AI plays the game out. A breakpoint stops it, and so does Stop" aria-label="Play to the end of the game"><span class="dbg-word">Play to End</span><i class="dbg-sign">⏭</i></button>
        </span>
        <div class="dbg-note" id="dbg-note" role="status"></div>
        <div class="dbg-cluster">
          <span class="dbg-group">
            <button data-dbg="step-action" title="Step in: the Civ AI takes its next action this turn, and ends the turn when it has none left" aria-label="Step one action"><i>▸</i><span class="dbg-word">Action</span></button>
            <button data-dbg="step-seat" title="Step one seat: end your turn as it stands, then one civilization's turn at a time" aria-label="Step one seat"><i>▸|</i><span class="dbg-word">Seat</span></button>
            <button data-dbg="undo" title="Undo your last action this turn (Ctrl+Z)" aria-label="Undo last action">↶</button>
          </span>
          <span class="dbg-group">
            <button data-dbg-pop="saves" title="Save points and saves: keep this world to come back to, or load one you kept" aria-label="Save points and saves"><i>⚑</i><span class="dbg-word">Save points</span><span class="dbg-count" id="dbg-point-count"></span></button>
            <button data-dbg="reveal" title="Reveal: show the whole map and every civilization, not only what yours can see" aria-label="Reveal the whole map"><i>◉</i><span class="dbg-word">Reveal</span></button>
            <button data-dbg="solo-ui" aria-pressed="false" title="Single player: the Civilization VI screen for playing the seat by hand, with its yields, launch bar, notifications, unit panel and End Turn. Off, the Debugger view: standings, victory tracker, governors and policies" aria-label="Single player screen"><i>♟</i><span class="dbg-word">Single player</span></button>
          </span>
        </div>
      </div>
    </div>
    <div class="dbg-hover" id="dbg-hover" hidden></div>
    <div class="dbg-ask" id="dbg-ask" role="dialog" aria-labelledby="dbg-ask-title" hidden></div>
    <div class="dbg-search" id="dbg-search" role="dialog" aria-labelledby="dbg-search-title" hidden>
      <h4 id="dbg-search-title">Autosearch</h4>
      <button class="dbg-search-shut" data-dbg-search="close" title="Put this panel away. A search that is running goes on" aria-label="Close the Autosearch panel">✕</button>
      <p id="dbg-search-about"></p>
      <div class="dbg-search-lists" id="dbg-search-lists" title="What a line is judged by: what the seat Needs and Must Avoid, held to a line, and what it Wants and Doesn't Want, with a weight. Each reads one of the seat's own numbers, as its row of the standings shows it"></div>
      <div class="dbg-search-set">
        <span>By</span><select id="dbg-search-turns" title="How far ahead the search plays: the number is read on that turn" aria-label="Turns to search ahead"></select>
        <span>Cores</span><select id="dbg-search-cores" title="How much of this computer searches. Each core plays lines on a copy of the engine, walking back down the timeline and up it again, so each costs the engine's memory again" aria-label="Cores to search with"></select>
        <span id="dbg-search-half-word">From</span><select id="dbg-search-half" title="Which branch the search begins from, as it stands on its open turn" aria-label="Branch to search from"></select>
        <span>Also</span><label class="dbg-search-alpha" title="Hand the seat as well to the Civ AI seated, aimed at each victory in turn: six more Civ AIs to try"><input type="checkbox" id="dbg-search-targets"> the seated Civ AI aimed at each victory</label>
        <span>Genes</span><input type="text" id="dbg-search-genes" placeholder="gene tags, a comma between" title="Hand the seat as well to the Civ AI seated with each of these genes switched on: one more Civ AI a gene" aria-label="Gene tags to try">
      </div>
      <div class="dbg-search-line"><button class="dbg-search-go" data-dbg-search="go"></button>
        <span class="dbg-search-status" id="dbg-search-status" role="status"></span></div>
      <div id="dbg-search-run" hidden>
        <div class="dbg-search-cores" id="dbg-search-heads" title="Each row is a core: the turns it holds of the future it is playing, walked back down the timeline and played up it again"></div>
        <div class="dbg-search-ends"><span id="dbg-search-from"></span><span id="dbg-search-size"></span><span id="dbg-search-to"></span></div>
      </div>
      <div class="dbg-search-found" id="dbg-search-found" hidden>
        <p id="dbg-search-best"></p>
        <p id="dbg-search-front"></p>
        <div class="dbg-search-plan" id="dbg-search-plan"></div>
        <div class="dbg-search-line" id="dbg-search-play"></div>
        <div class="dbg-search-chart" id="dbg-search-chart"></div>
        <div class="dbg-search-key" id="dbg-search-key"></div>
        <p id="dbg-search-lone">Each Civ AI alone, the one seated in gold:</p>
        <div class="dbg-search-table" id="dbg-search-table"></div>
      </div>
    </div>
    <div class="dbg-pop" id="dbg-pop-layers" hidden>
      <h4>Who decides</h4>
      <p>A layer that is on is played by the Civ AI at the top of each of your turns.
        Everything else, and ending the turn, stays with you.</p>
      <div id="dbg-layer-list" style="display:grid;gap:4px"></div>
      <div class="dbg-line">
        <button data-dbg="layers-none" title="Every decision by hand">By hand</button>
        <button data-dbg="layers-all" title="The agent takes every decision but the end of the turn">Everything</button>
        <button data-dbg="assist" title="Have the agent take the layers that are on in the turn that is open now">Apply to this turn</button>
      </div>
    </div>
    <div class="dbg-pop" id="dbg-pop-saves" hidden>
      <h4>Save points</h4>
      <p>A save point keeps this world to come back to, with the turns behind it, however the game
        is played on from here. Going to one keeps the world you left as a save point too. Save points
        last as long as this page is open; a save, below, is kept on this device.</p>
      <div class="dbg-line">
        <input type="text" data-dbg-field="pointname" maxlength="48" placeholder="a name, or leave empty for the turn" aria-label="Save point name">
        <button data-dbg="point" title="Keep the world as it stands now">Save point</button>
      </div>
      <div id="dbg-point-list" style="display:grid;gap:4px"></div>
      <div class="dbg-file-saves" style="display:grid;gap:8px">
        <h4>Saves</h4>
        <p>A save is the world at one turn, kept on this device. Loading one starts a timeline of its own from there.</p>
        <div class="dbg-line">
          <input type="text" data-dbg-field="savename" maxlength="64" placeholder="a name, or leave empty for the turn" aria-label="Save name">
          <button data-dbg="save" title="Save the turn that is open now">Save this turn</button>
        </div>
        <div id="dbg-save-list" style="display:grid;gap:4px"></div>
      </div>
    </div>
    <div class="dbg-pop" id="dbg-pop-breaks" hidden>
      <h4>Breakpoints</h4>
      <p>Auto-play, and a turn played to on the timeline, stop when one of these happens.</p>
      <label title="Stop on the first turn of each new era. The eras already played are named on the timeline at the turn they began"><input type="checkbox" data-dbg-field="eras"> A new era begins</label>
      <label title="Stop on the turn a nuclear device goes off, anyone's. The turn the game's future says it comes is drawn on the timeline as a dashed line"><input type="checkbox" data-dbg-field="nuclear"> A nuclear weapon is used</label>
      <div class="dbg-line"><span>Turns</span>
        <input type="text" data-dbg-field="turns" inputmode="numeric" placeholder="50, 75" aria-label="Stop when these turns open">
        <button data-dbg="break-plus" title="Add the turn ten from now">+10</button></div>
      <div class="dbg-line"><span>Phrases</span>
        <input type="text" data-dbg-field="phrases" placeholder="declared war, captured" aria-label="Stop when the event log says"></div>
      <div class="dbg-line" style="flex-wrap:wrap" title="Stop when this becomes true of a civilization">
        <span>When</span>
        <select data-dbg-field="cond-seat" aria-label="Civilization"></select>
        <select data-dbg-field="cond-key" aria-label="What to read"></select>
        <select data-dbg-field="cond-test" aria-label="How to hold it"></select>
        <input type="text" data-dbg-field="cond-value" inputmode="decimal" placeholder="value" style="flex:0 0 52px" aria-label="Value">
        <button data-dbg="cond-add" title="Stop a run on the turn this becomes true">Add</button>
      </div>
      <div id="dbg-condition-list" style="display:grid;gap:3px"></div>
      <div class="dbg-grid" id="dbg-category-list"></div>
      <label><input type="checkbox" data-dbg-field="important"> Any important event</label>
      <label><input type="checkbox" data-dbg-field="every_seat"> Every civilization's events, not only mine</label>
      <div class="dbg-line"><button data-dbg="break-clear">Clear all</button></div>
    </div>`;
  document.body.appendChild(bar);
  debugBar = bar;
  debugSearchWire(bar);
  for (const box of bar.querySelectorAll(".dbg-lane")) {
    const part = name => box.querySelector(`.dbg-${name}`);
    debugLanes[box.dataset.dbgLane] = {box, input: box.querySelector("input"), ruler: part("ruler"),
      band: part("band"), own: part("own"), by: part("by"), beyond: part("beyond"), kept: part("kept-turns"), goal: part("goal"), ghost: part("ghost"),
      cursor: part("cursor"), pill: box.querySelector(".dbg-cursor b"), marks: part("marks"), agent: part("agent"),
      start: part("start"), finish: part("finish"), fork: part("fork"), close: part("close"),
      deco: part("deco"), bent: part("bent"), axis: part("axis"), travel: part("travel")};
  }

  bar.addEventListener("click", event => {
    // What the box asked is answered on the box.
    const answered = event.target.closest("[data-dbg-ask]");
    if (answered) { debugAnswer(answered); return; }
    if (event.target.closest("[data-dbg-lock]")) {
      if (debugSplit) debugSplitLock(!debugSplit.locked);
      return;
    }
    // The ✕ of a branch closes it: the other is the timeline.
    const closed = event.target.closest("[data-dbg-close]");
    if (closed) {
      if (debugSplit && !closed.disabled) debugSplitClose(closed.dataset.dbgClose === "a" ? "b" : "a");
      return;
    }
    const pop = event.target.closest("[data-dbg-pop]");
    if (pop) {
      debugTogglePop(pop.dataset.dbgPop);
      if (debugPopOpen === "saves" && !debugSplit) debugDrawSaves();
      return;
    }
    // A mark on a lane of the split is that half's: a breakpoint turn is gone
    // to the way the lane's slider goes there, and a save point is the
    // half's own to go to. Locked, the other half follows either.
    const lane = debugSplit ? event.target.closest(".dbg-lane") : null;
    const mark = lane && lane.dataset.dbgLane ? event.target.closest("[data-dbg-turn], [data-dbg-goto]") : null;
    if (mark) {
      debugTravelLearnt();
      debugSay("");
      if (mark.dataset.dbgTurn) debugSplitGoTo(lane.dataset.dbgLane, mark.dataset.dbgTurn);
      else debugSplitGoToSave(lane.dataset.dbgLane, +mark.dataset.dbgGoto);
      return;
    }
    // The deck of a locked split is the one panel for both halves: a press
    // on it is theirs, not this page's own world's, which waits under them.
    if (debugSplit) { debugRemote("pane-press", event.target); return; }
    const gone = event.target.closest("[data-dbg-cond-remove]");
    if (gone && debugTimeline()) {
      const request = debugBreakpointsFromForm();
      request.conditions.splice(+gone.dataset.dbgCondRemove, 1);
      debugOp(request);
      return;
    }
    const named = event.target.closest("[data-dbg-turn]");
    if (named && debugTimeline()) { debugTravelLearnt(); debugSay(""); debugGoToTurn(named.dataset.dbgTurn); return; }
    const point = event.target.closest("[data-dbg-goto]");
    if (point && debugTimeline()) { if (point.closest(".dbg-marks")) debugTravelLearnt(); debugSay(""); debugOp({op: "goto_save", id: +point.dataset.dbgGoto}); return; }
    const dropped = event.target.closest("[data-dbg-drop]");
    if (dropped && debugTimeline()) { debugOp({op: "drop_save", id: +dropped.dataset.dbgDrop}); return; }
    const load = event.target.closest("[data-dbg-load]");
    if (load) { debugPopOpen = null; loadSave(load.dataset.dbgLoad); return; }
    const button = event.target.closest("[data-dbg]");
    if (!button || button.disabled) return;
    const timeline = debugTimeline();
    if (!timeline) return;
    const action = button.dataset.dbg;
    if (action === "back") debugBehind(1);
    else if (action === "back10") debugBehind(10);
    else if (action === "next") debugAhead(1);
    else if (action === "next10") debugAhead(10);
    else if (action === "undo") debugUndo();
    else if (action === "first") { debugSay(""); debugGoToTurn(timeline.first ?? state.turn); }
    else if (action === "era") {
      const began = debugEraBack(timeline);
      if (began) { debugSay(""); debugGoToTurn(began.turn); }
    }
    else if (action === "era-next") debugEraOn();
    else if (action === "end") debugToEnd();
    else if (action === "step-action") debugStepAction();
    else if (action === "step-seat") debugStepSeat();
    else if (action === "reveal") debugOp({op: "reveal", on: !timeline.reveal});
    else if (action === "solo-ui") setSoloUi(!SOLO_UI);
    else if (action === "autoplay") debugAutoPlay();
    else if (action === "save") debugSave();
    else if (action === "point") debugSavePoint();
    // Branch asks which Civ AI plays the branch, and the branch is made when
    // one is picked (`debugAnswer`). Pressed again, it asks nothing.
    else if (action === "split") {
      debugPopOpen = null;
      debugAsk = debugAsk && debugAsk.kind === "branch" ? null : {kind: "branch"};
      drawDebugBar(false);
    }
    else if (action === "show") { if (debugNotePos) centerOn(debugNotePos); }
    else if (action === "assist") debugOp({op: "assist", strategy: debugStrategy()}, AUTOPLAY_BATCH_TIMEOUT_MS);
    else if (action === "layers-none" || action === "layers-all") {
      debugOp({op: "layers", layers: action === "layers-all" ? timeline.layers.map(layer => layer.id) : []});
    } else if (action === "break-clear") {
      debugOp({op: "breakpoints"});
    } else if (action === "cond-add") {
      const pop = document.getElementById("dbg-pop-breaks");
      const field = name => pop.querySelector(`[data-dbg-field=cond-${name}]`);
      const request = debugBreakpointsFromForm();
      const test = field("test").value;
      request.conditions.push({seat: +field("seat").value, key: field("key").value, test,
        value: test === "changes" ? undefined : parseFloat(field("value").value)});
      debugOp(request);
    } else if (action === "break-plus") {
      const request = debugBreakpointsFromForm();
      request.turns = [...new Set([...request.turns, state.turn + 10])];
      debugOp(request);
    }
  });
  bar.addEventListener("change", event => {
    const target = event.target;
    if (target.id === "dbg-slider" || target.dataset.dbgHalf) {
      // Read first: once nothing is being dragged, drawing the bar puts every
      // slider back on the turn its world is on.
      const turn = target.value;
      debugScrubbing = null;
      // A slider let go of by the pointer gives the keys back to the page.
      if (debugSliderHeld) target.blur();
      debugSliderHeld = false;
      debugTravelLearnt();
      debugSay("");
      if (target.dataset.dbgHalf) debugSplitGoTo(target.dataset.dbgHalf, turn);
      else debugGoToTurn(turn);
    } else if (target.dataset.dbgAgent) {
      target.blur();
      debugSplitSeat(target.dataset.dbgAgent, target.value);
    } else if (debugSplit) {
      // The one panel of a locked split: what is changed on it is the halves'.
      if (target.id === "dbg-turn") target.blur();
      debugRemote("pane-change", target);
    } else if (target.id === "dbg-turn") {
      target.blur();
      debugGoToTurn(target.value);
    } else if (target.id === "dbg-agent") {
      // Another Civ AI picked on the one timeline is not seated yet: the box
      // asks whether this timeline plays on with it or a branch does
      // (operator 2026-11-22: "when edited to a different strategy, should
      // ask to switch or branch"). The one already seated is nothing to ask,
      // and is said to the engine as any pick is.
      target.blur();
      debugAsk = target.value && target.value !== debugStrategy() ? {kind: "switch", name: target.value} : null;
      if (debugAsk) drawDebugBar(false); else debugSwitchSeat(target.value);
    } else if (target.dataset.dbgLayer) {
      debugOp({op: "layers", layers: debugLayersFromForm()});
    } else if (target.closest("#dbg-pop-breaks")) {
      // The fields of a condition being composed are not breakpoints yet.
      if ((target.dataset.dbgField || "").startsWith("cond-")) { drawDebugBar(false); return; }
      debugOp(debugBreakpointsFromForm());
    }
  });
  bar.addEventListener("input", event => {
    const target = event.target;
    if (target.type !== "range") return;
    debugScrubbing = target.dataset.dbgHalf || "";
    const model = debugLaneModels[debugScrubbing];
    debugSay(model ? debugTurnWords(model, +target.value) : "");
  });
  // The map must not read a click on the bar as a click on a tile, and the
  // page's keys must not fire while a breakpoint is being typed.
  for (const kind of ["pointerdown", "mousedown", "wheel", "dblclick", "contextmenu"])
    bar.addEventListener(kind, event => event.stopPropagation());
  bar.addEventListener("pointerdown", event => {
    if (event.target.type === "range") debugSliderHeld = true;
  });
  // A slider put back where it was picked up says nothing when it is let go
  // of: it is not left looking dragged.
  window.addEventListener("pointerup", () => setTimeout(() => {
    if (debugScrubbing === null) return;
    const lane = debugLanes[debugScrubbing], model = debugLaneModels[debugScrubbing];
    // Anywhere else, letting go is a `change`, and that is what reads it.
    if (model && +lane.input.value !== model.turn) return;
    debugScrubbing = null;
    debugSliderHeld = false;
    debugSay("");
  }, 0));
  // What a place on the timeline is, and what letting go there would do,
  // said under the pointer.
  bar.addEventListener("pointermove", event => {
    const on = event.target instanceof Element ? event.target : null;
    debugHover(on && !on.closest(".dbg-marks i, .dbg-marks u b, .dbg-branch") ? on.closest(".dbg-rail") : null, event.clientX);
  });
  bar.addEventListener("pointerleave", () => debugHover(null));
  debugMoveWindows(bar);
  // The wheel over the timeline, and the up and down arrows in the turn box,
  // walk the played turns one at a time.
  const walk = forward => {
    if (debugSplit) {
      // The one panel of a locked split walks both halves: A is asked, and B follows.
      const deck = debugSplitDeck();
      if (deck && deck.idle && (forward ? deck.timeline.can_forward : deck.timeline.can_back))
        debugSplitTell("a", {civvis: "pane-op", body: {op: forward ? "forward" : "back"}, lead: true});
      return;
    }
    const timeline = debugTimeline();
    if (!timeline || debugBusy || autoplaying || turnAdvancePending) return;
    if (!(forward ? timeline.can_forward : timeline.can_back)) return;
    debugSay("");
    debugOp({op: forward ? "forward" : "back"});
  };
  bar.addEventListener("wheel", event => {
    if (!event.target.closest('[data-dbg-lane=""], .dbg-rail, #dbg-turn')) return;
    event.preventDefault();
    const delta = Math.abs(event.deltaX) > Math.abs(event.deltaY) ? event.deltaX : event.deltaY;
    if (delta) walk(delta > 0);
  }, {passive: false});
  bar.addEventListener("keydown", event => {
    if (event.key === "Escape" && debugAsk) {
      event.stopPropagation();
      debugAsk = null;
      drawDebugBar();
      return;
    }
    if (event.key === "Escape" && debugPopOpen) {
      event.stopPropagation();
      debugPopOpen = null;
      if (event.target instanceof HTMLElement) event.target.blur();
      drawDebugBar();
      return;
    }
    // A slider's own keys move it a turn at a time, and a picker's choose from
    // it; the page does not hear them.
    if (event.target.matches("input[type=range], select")) { event.stopPropagation(); return; }
    if (!event.target.matches("input[type=text]")) return;
    event.stopPropagation();
    if (event.target.id === "dbg-turn" && (event.key === "ArrowUp" || event.key === "ArrowDown")) {
      event.preventDefault();
      walk(event.key === "ArrowUp");
    }
  });
  // The left panel is folded away, brought back and dragged wider without
  // the window changing size: the box follows the map area's left edge.
  const area = document.getElementById("maparea");
  if (area && typeof ResizeObserver === "function") {
    new ResizeObserver(() => {
      if (!bar.hidden && debugMapLeft() !== debugDockedLeft) drawDebugBar();
    }).observe(area);
  }
  return bar;
}

// A panel of the box, the box's question and the Autosearch panel are
// windows, and a window is
// moved by its title (operator 2026-11-22: "clicking and dragging on a window
// title allows for repositioning"): pressed there and dragged, it goes where
// the pointer takes it, over the map or over either half of a split. Where it
// was put is kept for as long as the page is open, so it opens there again,
// and a double click on its title puts it back under what it opens from.
// The place is the window's top left corner in the browser window, so it
// holds whichever shape the box is in. Each panel has its own, and so has
// each of the two things the box asks (`data-place`).
const debugWindowPlace = {};
let debugWindowMove = null;
function debugWindowKey(win) {
  return win.dataset.place || win.id;
}
function debugPlaceWindow(win, bar, anchor) {
  const frame = bar.getBoundingClientRect(), put = debugWindowPlace[debugWindowKey(win)];
  if (put) {
    // Never out of the browser window, and never with its title out of reach.
    win.style.left = `${Math.max(4, Math.min(window.innerWidth - win.offsetWidth - 4, put.x)) - frame.left}px`;
    win.style.top = `${Math.max(4, Math.min(window.innerHeight - 44, put.y)) - frame.top}px`;
    return;
  }
  const from = anchor.getBoundingClientRect();
  win.style.left = `${Math.max(8, Math.min(from.left - frame.left, frame.width - win.offsetWidth - 8))}px`;
  win.style.top = `${from.bottom - frame.top + 6}px`;
}
function debugMoveWindows(bar) {
  const titled = target => {
    const title = target instanceof Element ? target.closest("h4") : null;
    const win = title ? title.parentElement : null;
    return win && win.matches(".dbg-pop, .dbg-ask, .dbg-search") && win.firstElementChild === title ? win : null;
  };
  bar.addEventListener("pointerdown", event => {
    const win = titled(event.target);
    if (!win || (event.button !== 0 && event.pointerType !== "touch")) return;
    const box = win.getBoundingClientRect();
    debugWindowMove = {win, pointer: event.pointerId, x: event.clientX, y: event.clientY,
      left: box.left, top: box.top, moved: false};
    // The window keeps the pointer, so the drag goes on over a half's frame.
    try { win.setPointerCapture(event.pointerId); } catch (_) {}
    event.preventDefault();
  });
  window.addEventListener("pointermove", event => {
    const move = debugWindowMove;
    if (!move || event.pointerId !== move.pointer) return;
    const dx = event.clientX - move.x, dy = event.clientY - move.y;
    // A press that does not travel is a click, and moves nothing.
    if (!move.moved && Math.abs(dx) + Math.abs(dy) < 3) return;
    move.moved = true;
    move.win.classList.add("dbg-moving");
    debugWindowPlace[debugWindowKey(move.win)] = {x: move.left + dx, y: move.top + dy};
    debugPlaceWindow(move.win, bar, null);
  });
  for (const kind of ["pointerup", "pointercancel"])
    window.addEventListener(kind, event => {
      const move = debugWindowMove;
      if (!move || event.pointerId !== move.pointer) return;
      debugWindowMove = null;
      move.win.classList.remove("dbg-moving");
    });
  bar.addEventListener("dblclick", event => {
    // The window kept the pointer for the press, so the click is said of the
    // window: what was clicked is read from where the pointer is.
    const win = titled(event.target) || titled(document.elementFromPoint(event.clientX, event.clientY));
    if (!win || !debugWindowPlace[debugWindowKey(win)]) return;
    delete debugWindowPlace[debugWindowKey(win)];
    drawDebugBar(false);
  });
}

// The Civ AIs on offer are the ones the page's own picker lists. A picker
// shows the one that is seated unless it is being chosen from.
function debugOfferAgents(select, seated) {
  const offered = [...((document.getElementById("autoplaystrategy") || {}).options || [])]
    .map(option => `<option value="${escapeAttr(option.value)}">${escapeAttr(option.textContent)}</option>`).join("");
  if (select.dataset.drawn !== offered) { select.dataset.drawn = offered; select.innerHTML = offered; }
  if (seated !== undefined && document.activeElement !== select) select.value = seated;
}
// The whole game, end to end: a timeline is as long as the game may be, 250
// turns when the game does not say, and longer when it is played on past that.
// `finish` is the game's last turn, when it has one: that is where the flag is.
function debugAxis(first, last) {
  const finish = [state.turn_limit, state.max_turns].find(turns => Number.isFinite(turns) && turns > 0) || null;
  const open = Number.isFinite(state.turn_limit) && state.turn_limit > 0 ? 0 : 1;
  // A game still inside its last turn ends the timeline there, however short it is.
  const hi = finish !== null && last <= finish ? finish : Math.max(finish || 250, Math.ceil((last + open) / 50) * 50);
  return {lo: Math.min(1, first), hi, finish};
}
// How far along a lane a turn stands, in hundredths of a percent of it.
function debugAlong(turn, lo, hi) {
  return hi > lo ? Math.round(Math.max(0, Math.min(1, (turn - lo) / (hi - lo))) * 10000) / 100 : 0;
}
// How many turns apart the numbered turns of a timeline are.
function debugRulerStep(lo, hi) {
  const length = hi - lo;
  return length <= 60 ? 10 : length <= 150 ? 25 : length <= 300 ? 50 : length <= 600 ? 100 : 250;
}
// The ruler on a lane: a tick every few turns and a number on the round ones
// between the two ends, which are named beside the rail.
function debugRulerMarkup(lo, hi, finish = null) {
  const major = debugRulerStep(lo, hi), minor = major / 5;
  let ruler = "";
  for (let turn = Math.ceil((lo + 1) / minor) * minor; turn < hi; turn += minor) {
    const at = debugAlong(turn, lo, hi), round = turn % major === 0;
    ruler += `<i${round ? ' class="major"' : ""} style="left:${at}%"></i>`;
    if (round) ruler += `<b style="left:${at}%">${turn}</b>`;
  }
  if (finish !== null && finish > lo && finish < hi)
    ruler += `<i class="dbg-finish-line" style="left:${debugAlong(finish, lo, hi)}%" title="Turn ${finish}: the last turn of the game"></i>`;
  return ruler;
}
// A split's timeline is a tree (`model.tree`: whether it has forked, and on
// which turn), and its turns are numbered once, beside its rails: over the
// trunk, and from where the two branches are in their rows, between them.
// `fork` is how far along the lane the tree forks, in pixels (`travel` wide),
// and no number stands where the turn it forks on is said.
function debugAxisMarkup(lo, hi, tree, at, travel) {
  const major = debugRulerStep(lo, hi), fork = at(tree.turn) / 100 * travel;
  let axis = "";
  for (let turn = Math.ceil((lo + 1) / major) * major; turn < hi; turn += major) {
    const between = tree.forked && turn > tree.turn, px = at(turn) / 100 * travel;
    if (tree.forked && travel > 0 && px > fork - 14 && px < fork + DEBUG_TREE.run + 66) continue;
    axis += `<b${between ? ' class="dbg-between"' : ""} style="left:${at(turn)}%">${turn}</b>`;
  }
  return axis;
}
// A half's lane is drawn in the half's own row, and the tree is not in that
// row everywhere: the trunk is in the middle of the box, and a branch takes
// `DEBUG_TREE.run` pixels to reach its row. How far out of its row the tree
// stands on a turn: all the way on the trunk (1), and not at all once the
// branch is in its row (0). `px` is how far along the lane the turn stands,
// and `fork` the turn the tree forks on, both in pixels.
function debugOffRow(tree, turn, px, fork, travel) {
  if (!tree.forked || turn <= tree.turn) return 1;
  return travel > 0 ? Math.max(0, 1 - (px - fork) / DEBUG_TREE.run) : 0;
}
// Drawn only when it changes: a list rebuilt under the pointer loses the click.
function debugDrawn(box, html) {
  if (box.dataset.drawn !== html) { box.dataset.drawn = html; box.innerHTML = html; }
}
function debugPlace(box, css) {
  if (box.dataset.placed !== css) { box.dataset.placed = css; box.style.cssText = css; }
}
// A place on a lane in a few words: what it is, and what letting go of the
// slider there does.
function debugTurnWords(model, turn) {
  const name = model.name ? `${model.name} · turn ${turn}` : `Turn ${turn}`;
  if (turn > model.last) {
    const theirs = (model.played || []).find(stretch => turn > stretch.from && turn <= stretch.to);
    const beyond = model.beyond && turn >= model.beyond.from && turn <= model.beyond.to
      ? `, over the turns ${debugAgentName((theirs || model.beyond).by)} played` : "";
    return (model.turn < model.last ? `${name} · play on from turn ${model.turn} to here` : `${name} · play to here`) + beyond;
  }
  if (turn < model.first) return `${name} · before the timeline begins, goes to turn ${model.first}`;
  if (model.kept && !model.kept.has(turn)) {
    const near = model.turns.filter(kept => kept <= turn).pop() ?? model.first;
    return `${name} · no longer kept, goes to turn ${near}`;
  }
  const flags = (model.named || model.flags || []).filter(point => point.turn === turn).map(point => ` · ⚑ ${point.name}`).join("");
  // The turn a split branches on: where A and B part.
  const branches = model.branches === turn ? " · A and B part here" : "";
  // Whose play opened the turn, of everything the lane's world has been through.
  const played = (model.played || []).find(stretch => turn > stretch.from && turn <= stretch.to);
  const by = played ? ` · ${debugPlayedBy(played)}` : "";
  return `${name}${turn === model.turn ? " · open now" : ""}${by}${flags}${branches}`;
}
// Who played a stretch of a timeline, in a word or two: the Civ AI by the
// name its picker shows, or the person.
function debugPlayedBy(stretch) {
  return stretch.by ? debugAgentName(stretch.by) : "By hand";
}
// How wide a name stands on a band, measured once a name.
const debugNameWidths = new Map();
let debugNameInk = null;
function debugNameWidth(name) {
  if (!debugNameWidths.has(name)) {
    debugNameInk = debugNameInk || document.createElement("canvas").getContext("2d");
    debugNameInk.font = `700 10.5px ${getComputedStyle(debugBar).fontFamily}`;
    debugNameWidths.set(name, Math.ceil(debugNameInk.measureText(name).width) + 12);
  }
  return debugNameWidths.get(name);
}
// The stretches of a lane's band, each named by who played it (`model.by`:
// from which turn, to which, and whose play). A name stands at the start of
// its stretch. The last stretch has the rail ahead of it, so its name may run
// on past its end (`free`). The turns kept ahead of the lane's line, which
// another Civ AI played, are named the same way where they are drawn (`far`).
//
// A name steps past what stands on it: the lane's marker, and in a half's
// row, which is not tall enough for both, an era's name. `blocks` is where
// those stand, each from and to in pixels along the lane and in their order,
// and `travel` the width the lane's turns are drawn across (`debugTravel`).
// Where there is no rail to step on to, the name is cut short before the
// thing instead, and a name that has stepped on is cut short at the end of
// its stretch.
function debugStretchMarkup(model, at, travel, blocks, free) {
  // A branch is named on itself (operator 2026-11-22: "lets label the
  // branches themselves"): its letter stands before the name on its first
  // stretch, and a branch nothing has been played on yet is named by the Civ
  // AI seated in it, the one that will play it (`model.letter`, a half's
  // lane once the tree has forked; `model.seated`).
  const waits = !!model.letter && !(model.by || []).length;
  const stretches = waits ? [{from: model.fork, to: model.fork, by: model.seated || null}] : model.by || [];
  const px = turn => at(turn) / 100 * travel;
  // A stretch that is all on the branch's way to its row (`dbg-arm`), where
  // no word is drawn, is named nowhere, and the branch's letter goes on the
  // first stretch that reaches the row. The last one's name runs on past its
  // end, so it is always named.
  const arm = model.letter && travel > 0 ? px(model.fork) + DEBUG_TREE.run : -Infinity;
  const bent = (stretch, index) => !(free && index === stretches.length - 1) && px(stretch.to) <= arm;
  const lettered = Math.max(0, stretches.findIndex((stretch, index) => !bent(stretch, index)));
  return stretches.map((stretch, index) => {
    const last = free && index === stretches.length - 1;
    const far = stretch.from >= model.last;
    const kind = [far ? "dbg-far" : index % 2 ? "dbg-alt" : "", index && !far ? "dbg-next" : "", last ? "dbg-free" : ""]
      .filter(Boolean).join(" ");
    const shade = `<s${kind ? ` class="${kind}"` : ""} style="left:${at(stretch.from)}%;width:${Math.max(0, at(stretch.to) - at(stretch.from))}%">`;
    if (bent(stretch, index)) return `${shade}</s>`;
    const who = waits && !stretch.by ? "" : debugPlayedBy(stretch);
    const name = index === lettered && model.letter ? [model.letter, who].filter(Boolean).join(" · ") : who;
    const width = debugNameWidth(name);
    const from = px(stretch.from), to = px(stretch.to);
    // How far the name may reach, and where it begins, both from the start
    // of its stretch.
    const reach = last ? travel + 14 - from : to - from;
    let left = 3, room = null;
    for (const [lo, hi] of travel > 0 ? blocks : []) {
      if (hi - from <= left || lo - from >= left + width) continue;
      const after = hi - from + 2;
      if (after + Math.min(width, last ? width : 40) <= reach) { left = Math.round(after); continue; }
      if (lo - from - left >= 24) room = Math.round(lo - from - left);
      else if (last) left = Math.round(Math.max(3, lo - from - 2 - width));
      break;
    }
    if (room === null && !last && left !== 3) room = Math.max(0, Math.round(reach - left - 3));
    const style = `${left === 3 ? "" : `left:${left}px;`}${room === null ? "" : `max-width:${room}px`}`;
    return `${shade}<b${style ? ` style="${style}"` : ""}>${escapeAttr(name)}</b></s>`;
  }).join("");
}
// One lane, drawn from what it stands for: the turns the game may run to
// (`lo` to `hi`), the turns played (`first` to `last`), the turn that is open,
// and, past `fork`, the part of its history that is one half's own.
//
// A split draws three: the trunk, and a lane for each half. It says itself
// where each marker stands (`marker`, null for none), because two halves on
// one turn of the trunk have one marker between them, and on the trunk where
// the timeline branches (`branches`). Its three lanes are one tree (`tree`):
// what a half's lane holds on the trunk, or on a branch still on its way to
// the half's row, is drawn where the tree is (`debugOffRow`).
function debugDrawLane(key, model) {
  const lane = debugLanes[key];
  debugLaneModels[key] = model;
  lane.box.hidden = !model;
  if (!model) return;
  const {lo, hi, first, last} = model;
  const at = turn => debugAlong(turn, lo, hi);
  const span = (from, to) => `left:${at(from)}%;width:${Math.max(0, at(to) - at(from))}%`;
  lane.input.min = lo; lane.input.max = hi;
  lane.input.disabled = !model.idle;
  if (debugScrubbing !== key) lane.input.value = model.turn;
  const shown = model.marker !== undefined ? model.marker : debugScrubbing === key ? +lane.input.value : model.turn;
  lane.input.setAttribute("aria-valuetext", `Turn ${debugScrubbing === key ? +lane.input.value : model.turn} of ${hi}`);
  const finish = model.finish ?? null;
  const axis = `${lo}:${hi}:${finish}`;
  if (lane.ruler.dataset.axis !== axis) {
    lane.ruler.dataset.axis = axis;
    lane.ruler.innerHTML = debugRulerMarkup(lo, hi, finish);
    lane.start.textContent = lo;
    lane.start.title = `Turn ${lo}: where the game begins`;
    lane.finish.classList.toggle("dbg-flagged", finish === hi);
    lane.finish.lastElementChild.textContent = hi;
    lane.finish.title = finish === hi ? `Turn ${hi}: the last turn of the game` : `Turn ${hi}`;
  }
  const fork = model.fork === null ? last : Math.max(first, Math.min(last, model.fork));
  debugPlace(lane.band, span(first, fork));
  lane.own.hidden = fork >= last;
  debugPlace(lane.own, span(fork, last));
  const beyond = model.beyond && model.beyond.to > last ? model.beyond : null;
  lane.beyond.hidden = !beyond;
  if (beyond) debugPlace(lane.beyond, span(last, beyond.to));
  // A lane's last name runs on over the rail ahead unless something else is
  // drawn there (a trunk's branches). What a name steps past: the marker in
  // the lane's row, which in a half's row may be the trunk's (`over`), the
  // wider of the two; on a tree's slim rails the names of the eras that
  // stand on the lane (`under`: the ones a half's lane draws on the trunk);
  // and a branch on its way to its row, where no name is drawn.
  const travel = debugTravel[key] || 0, along = turn => at(turn) / 100 * travel;
  const tree = model.tree || null, half = tree !== null && !!model.name;
  const over = model.over ?? null, pill = over !== null ? over : shown;
  const wide = over !== null || !model.name ? 30 : 23;
  const blocks = pill === null ? [] : [[along(pill) - wide, along(pill) + wide]];
  for (const era of (model.name ? model.eras : model.under) || [])
    blocks.push([along(era.turn) + 2, along(era.turn) + 6 + debugNameWidth(debugEraName(era))]);
  if (half && tree.forked) blocks.push([along(tree.turn), along(tree.turn) + DEBUG_TREE.run]);
  debugDrawn(lane.by, debugStretchMarkup(model, at, travel, blocks.sort((one, other) => one[0] - other[0]), model.free !== false));
  lane.box.classList.toggle("dbg-thinned", !!model.kept);
  const kept = model.kept ? `${lo}:${hi}:${model.turns.join()}` : "";
  if (lane.kept.dataset.turns !== kept) {
    lane.kept.dataset.turns = kept;
    lane.kept.innerHTML = model.kept ? model.turns.map(turn => `<i style="left:${at(turn)}%"></i>`).join("") : "";
  }
  const goal = model.goal !== null && model.goal > model.turn ? model.goal : null;
  lane.goal.hidden = goal === null;
  if (goal !== null) debugPlace(lane.goal, span(model.turn, goal));
  // A branch on its way to its row: everything drawn on the half's rail over
  // again but its words, for the style to slant (`dbg-arm`).
  if (lane.bent) debugDrawn(lane.bent, half && tree.forked ? lane.deco.innerHTML.replace(/<b\b[^>]*>[^<]*<\/b>| data-drawn="[^"]*"/g, "") : "");
  if (lane.axis) debugDrawn(lane.axis, tree ? debugAxisMarkup(lo, hi, tree, at, travel) : "");
  // How far out of its row a half's lane is drawn on a turn, and in pixels
  // how far a mark there is moved to stand on the tree: A's down, B's up.
  // Unlocked, each half's save points and breakpoints are its own to go to,
  // and on the trunk the two halves' keep a little apart (`lean`).
  const off = turn => half ? debugOffRow(tree, turn, along(turn), along(tree.turn), travel) : 0;
  const moved = (turn, lean = 0) => {
    const by = Math.round(off(turn) * (DEBUG_TREE.rise - lean) * 10) / 10;
    return by ? `;transform:translateY(${key === "a" ? by : -by}px)` : "";
  };
  lane.cursor.hidden = shown === null;
  if (shown !== null) {
    debugPlace(lane.cursor, `left:${at(shown)}%`);
    if (lane.pill.textContent !== String(shown)) lane.pill.textContent = shown;
    // A half's marker keeps to its row, with a pin to the tree where the
    // tree is not there.
    lane.cursor.classList.toggle("dbg-pinned", off(shown) > 0);
  }
  // Click to travel, until the timeline has been clicked to travel: in the
  // open rail ahead of the marker, or behind it once the marker is far
  // along, and only while a click would travel.
  if (lane.travel) {
    const call = !debugTravelKnown && !!model.travel;
    // One that is going is left to go.
    if (call || !lane.travel.classList.contains("dbg-travel-gone")) lane.travel.hidden = !call;
    if (call) {
      const from = at(shown ?? model.turn), centre = from <= 60 ? (from + 100) / 2 : from / 2;
      debugPlace(lane.travel, `left:clamp(120px, ${centre.toFixed(1)}%, calc(100% - 120px))`);
    }
  }
  if (lane.fork) {
    const branches = model.branches ?? null;
    // Two halves standing on that very turn have their marker there instead.
    lane.fork.hidden = branches === null || shown === branches;
    if (branches !== null) {
      debugPlace(lane.fork, `left:${at(branches)}%`);
      if (lane.fork.textContent !== `⑂ ${branches}`) lane.fork.textContent = `⑂ ${branches}`;
    }
  }
  debugDrawn(lane.marks,
    (model.eras || []).filter(era => era.turn >= lo && era.turn <= hi).map(era =>
      `<u${era.both ? ` class="dbg-both${era.still ? " dbg-still" : ""}"` : ""} style="left:${at(era.turn)}%${moved(era.turn)}"><b data-dbg-turn="${era.turn}" title="The ${escapeAttr(titleCase(String(era.name || `era ${era.era}`)))} era began on turn ${era.turn}. Click to go there">${
        escapeAttr(titleCase(String(era.name || `era ${era.era}`)))}</b></u>`).join("") +
    (model.lines
      ? debugLineRows(model.lines.filter(line => line.turn >= lo && line.turn <= hi), along).map(([line, row]) =>
        `<i class="stop${line.ahead ? " dbg-ahead" : ""}${row ? " dbg-row2" : ""}" data-dbg-turn="${line.turn}" style="left:${at(line.turn)}%" title="${escapeAttr(line.title)}. Click to ${
          line.turn > last ? "play to it" : "go there"}"><b>${escapeAttr(line.label)}</b></i>`).join("")
      : (model.stops || []).filter(turn => turn >= lo && turn <= hi).map(turn =>
        `<i class="stop" data-dbg-turn="${turn}" style="left:${at(turn)}%${moved(turn, tree ? tree.lean : 0)}" title="Breakpoint on turn ${turn}. Click to ${
          turn > last ? "play to it" : "go there"}"></i>`).join("")) +
    (model.flags || []).filter(point => point.turn >= lo && point.turn <= hi).map(point =>
      `<i${point.auto ? ' class="auto"' : ""} data-dbg-goto="${point.id}" style="left:${at(point.turn)}%${moved(point.turn, tree ? tree.lean : 0)}"` +
      ` title="${escapeAttr(point.name)}: turn ${point.turn}. Click to go there"></i>`).join(""));
}
function debugHover(rail, x = 0) {
  const tag = document.getElementById("dbg-hover");
  if (!tag) return;
  for (const lane of Object.values(debugLanes)) lane.ghost.hidden = true;
  const key = rail ? rail.closest(".dbg-lane").dataset.dbgLane : null;
  const model = key === null ? null : debugLaneModels[key];
  if (!model) { tag.hidden = true; return; }
  const lane = debugLanes[key], box = lane.input.getBoundingClientRect();
  const along = (x - box.left - DEBUG_THUMB_HALF) / Math.max(1, box.width - 2 * DEBUG_THUMB_HALF);
  const turn = Math.round(model.lo + Math.max(0, Math.min(1, along)) * (model.hi - model.lo));
  lane.ghost.hidden = false;
  debugPlace(lane.ghost, `left:${debugAlong(turn, model.lo, model.hi)}%`);
  tag.textContent = debugTurnWords(model, turn);
  tag.hidden = false;
  const bar = debugBar.getBoundingClientRect(), half = tag.offsetWidth / 2;
  tag.style.left = `${Math.max(half + 4, Math.min(bar.width - half - 4, x - bar.left))}px`;
}
// Whether the timeline has been clicked to travel in this browser. Until it
// has, it says Click to travel (`.dbg-travel`); once it has, that goes and
// is not said again, in this game or the next. A travel by the timeline is a
// slider let go of on another turn, or a mark on a rail clicked (an era, a
// breakpoint turn, a save point).
const DEBUG_TRAVEL_KEY = "civvis-debug-click-to-travel";
let debugTravelKnown = (() => {
  try { return localStorage.getItem(DEBUG_TRAVEL_KEY) === "known"; } catch (_) { return false; }
})();
function debugTravelLearnt() {
  if (debugTravelKnown) return;
  debugTravelKnown = true;
  try { localStorage.setItem(DEBUG_TRAVEL_KEY, "known"); } catch (_) {}
  const call = (debugLanes[""] || {}).travel;
  if (!call || call.hidden) return;
  call.classList.add("dbg-travel-gone");
  setTimeout(() => { call.hidden = true; call.classList.remove("dbg-travel-gone"); }, 400);
}
// The words on the tools are for a box with room for them: when the tools at
// either end wrap past two rows, or have so little room that they stand over
// the turn buttons beside them, the words go and the signs stay, first the
// two on the turn buttons (a snug box) and then every tool's (a tight one).
// They are tried again only when the box's width changes, so a box on the
// edge does not flicker. One half of a split, and a stacked box, have no room
// for words at all.
let debugFitted = 0;
function debugFit(bar) {
  const width = bar.clientWidth;
  const stacked = width <= (DEBUG_PANE !== null ? DEBUG_PANE_STACKED_UNDER : DEBUG_STACKED_UNDER);
  bar.classList.toggle("dbg-stacked", stacked);
  const fresh = width !== debugFitted;
  if (fresh) { debugFitted = width; bar.classList.remove("dbg-snug", "dbg-tight"); }
  const deck = bar.querySelector(".dbg-deck"), has = name => bar.classList.contains(name);
  // A row of tools that cannot shrink any further runs on over what is
  // beside it without making the deck any taller: over the room kept for
  // what the box says, which the turn buttons' words give back, and then
  // over the turn buttons themselves.
  const crowded = beside => [...deck.querySelectorAll(".dbg-side")].some(side => {
    const kept = [...side.querySelectorAll(beside)].map(held => held.getBoundingClientRect());
    return [...side.querySelectorAll(".dbg-cluster > *")].some(tool => {
      const box = tool.getBoundingClientRect();
      return box.width > 0 && kept.some(held => box.left < held.right - 1 && box.right > held.left + 1);
    });
  });
  const tall = () => deck.offsetHeight > DEBUG_DECK_ROWS || crowded(".dbg-nav");
  if (stacked || DEBUG_PANE !== null) bar.classList.add("dbg-tight");
  if (!has("dbg-tight") && !has("dbg-snug") && (tall() || crowded(".dbg-note"))) bar.classList.add("dbg-snug");
  if (!has("dbg-tight") && tall()) bar.classList.add("dbg-tight");
}
// The box begins where the map area does, at the right side of the left
// panel, and the map area stands under it. Where that is and how tall the box
// is are measured, and the page is told when either changes: the map is
// fitted to the room it has. A split covers the left panel, and its box has
// the whole width.
let debugDocked = 0, debugDockedLeft = -1;
// How wide each lane's turns are drawn, in pixels. The names on a lane are
// placed by it (`debugStretchMarkup`), so it is read where the box measures
// itself, and a lane whose width changed is drawn again.
const debugTravel = {};
function debugMapLeft() {
  const area = document.getElementById("maparea");
  return area && !debugSplit ? Math.max(0, Math.round(area.getBoundingClientRect().left)) : 0;
}
// A bar that cannot be seen is not measured. Doctor Strange's Civ 6 keeps
// its bar out of sight under the worlds, and a world of it has none
// (`STRANGE_STYLE`, `STRANGE_PANE_STYLE`); every measure here makes the
// browser lay the whole page out again, and with each world saying a dozen
// things a turn that was a fifth of all the page did while time ran. The
// bar is measured again when it is back in sight. Edge of Tomorrow never
// shows it (`html.tomorrow #debugbar`).
function debugBarUnseen() {
  const page = document.documentElement.classList;
  return page.contains("strange-pane") || page.contains("tomorrow") || page.contains("moneyball") || (page.contains("strange") && debugSplit !== null);
}
function debugDock(bar) {
  if (debugBarUnseen()) return;
  const left = bar.hidden ? 0 : debugMapLeft();
  if (left !== debugDockedLeft) {
    debugDockedLeft = left;
    document.documentElement.style.setProperty("--dbg-left", `${left}px`);
  }
  if (!bar.hidden && (!debugSplit || bar.classList.contains("dbg-merged"))) debugFit(bar);
  let moved = false;
  for (const [key, lane] of Object.entries(debugLanes)) {
    const width = bar.hidden || lane.box.hidden ? 0 : lane.by.clientWidth;
    if (width !== (debugTravel[key] || 0)) { debugTravel[key] = width; moved = true; }
  }
  if (moved && !bar.hidden) drawDebugBar(false);
  const height = bar.hidden ? 0 : Math.ceil(bar.getBoundingClientRect().height);
  if (height === debugDocked) return;
  debugDocked = height;
  document.documentElement.classList.toggle("dbg-docked", height > 0);
  document.documentElement.style.setProperty("--dbg-top", `${height}px`);
  window.dispatchEvent(new Event("resize"));
}
// What the deck is drawn from: the turn that is open, what the engine says of
// its timeline, and what this page is doing with it. A page draws its own
// deck from this. A half of a split that is locked to the other sends it to
// the host instead (`pane-deck`), whose deck is then the one panel for both.
function debugDeckNow() {
  const timeline = debugTimeline();
  const mine = state.players.find(player => player.id === 0) || {};
  const layered = timeline.assisted
    ? `The Civ AI took ${timeline.assisted} decision${timeline.assisted === 1 ? "" : "s"} in its layers this turn` : "";
  // A round being stepped a seat at a time is not the person's turn, and the
  // bar says whose it is for as long as nothing newer is being said.
  const round = timeline.playing
    ? `${debugPlaysNext(timeline.playing)} · ▸| steps it, 1▶ lets the round finish` : "";
  // Turns ahead that another Civ AI played are said for as long as they
  // stand there: it is why going on from here plays instead of walking.
  const beyond = timeline.beyond ? debugBeyondWords(timeline.beyond) : "";
  return {
    turn: state.turn,
    limit: Number.isFinite(state.turn_limit) && state.turn_limit > 0 ? state.turn_limit : null,
    timeline,
    idle: !debugBusy && !autoplaying && !turnAdvancePending,
    running: debugRunning, stopping: debugStopAsked, played: debugPlayed, asked: debugAsked, goal: debugGoal,
    strategy: debugStrategy() || "",
    solo: SOLO_UI,
    eraBack: debugEraBack(timeline),
    eraNext: debugEraNext(timeline),
    // The era the open turn is in, and whether there is one after it.
    era: state.world_era ?? 0,
    lastEra: (state.world_era ?? 0) >= ERA_NAMES.length - 1,
    note: debugNoteText || round || layered || beyond,
    show: !!(debugNoteText && debugNotePos),
    // A condition reads one thing the observation says about one
    // civilization. The readings on offer are the ones this seat's own entry
    // has; the engine reads the same entry for any seat, and refuses one it
    // cannot find.
    seats: state.players.map(player => ({id: player.id, name: debugSeatName(player.id),
      major: !player.is_minor && !player.is_barbarian})),
    readable: Object.keys(mine).filter(key => {
      const value = mine[key];
      return typeof value === "number" || typeof value === "boolean" || Array.isArray(value);
    }).sort(),
  };
}
function drawDebugBar(measure = true) {
  const bar = debugBar || debugBuildBar();
  // A window held for a move is drawn once, by the move (`debugSwap`).
  if (debugHeld) return;
  const timeline = debugTimeline();
  bar.hidden = !timeline;
  document.documentElement.classList.toggle("dbg-pane-locked", DEBUG_PANE !== null && debugPaneLocked);
  if (!timeline) { debugDock(bar); return; }
  bar.classList.toggle("dbg-split-on", debugSplit !== null);
  bar.classList.toggle("dbg-pane", DEBUG_PANE !== null);
  document.documentElement.classList.toggle("dbg-split-open", debugSplit !== null);
  const deck = debugSplit ? debugSplitDeck() : debugDeckNow();
  bar.classList.toggle("dbg-merged", debugSplit !== null && deck !== null);
  bar.classList.toggle("dbg-locked", debugSplit !== null && debugSplit.locked);
  if (debugSplit) {
    // The host of a split: its own world waits under the two halves, and the
    // bar shows the two halves' history: one timeline up to the turn it was
    // branched on (`debugSplitFork`), and from there a branch for each, in
    // the half's own colour.
    const panes = debugSplit.panes, from = debugSplit.from ?? state.turn;
    const fork = debugSplitFork();
    const told = (letter, kind) => {
      const list = (panes[letter].marks || {})[kind];
      return Array.isArray(list) ? list : [];
    };
    const beyond = letter => (panes[letter].marks || {}).beyond || null;
    const kept = letter => Math.max(fork.last[letter], (beyond(letter) || {}).to ?? 0);
    const axis = debugAxis(1, Math.max(kept("a"), kept("b")));
    // A run asked for on the one panel is the half's own, and the half says
    // where it is headed.
    const running = debugSplit.running > 0 || (deck !== null && deck.running);
    const goal = debugSplit.running > 0 ? debugSplit.until : deck !== null && deck.running ? deck.goal : null;
    // Where each half's marker stands: on its turn, or where its slider is
    // being dragged to. A locked half goes where the other is dragged.
    const stands = {};
    for (const letter of ["a", "b"])
      stands[letter] = debugScrubbing === letter ? +debugLanes[letter].input.value : panes[letter].turn ?? from;
    if (debugSplit.locked && debugScrubbing) stands.a = stands.b = stands[debugScrubbing];
    const onTrunk = turn => !fork.forked || turn <= fork.turn;
    // Two halves on one turn of the trunk have one marker: the trunk's.
    const one = stands.a === stands.b && onTrunk(stands.a);
    const along = (debugAlong(fork.turn, axis.lo, axis.hi) / 100).toFixed(4);
    bar.classList.toggle("dbg-forked", fork.forked);
    if (bar.dataset.fork !== along) {
      bar.dataset.fork = along;
      bar.style.setProperty("--dbg-fork", along);
    }
    // The trunk: every turn the halves hold as one world. Until their worlds
    // differ its rail is the whole timeline, and says where a run is headed.
    // Whose play each stretch was: the turns the halves hold as one world
    // are named on the trunk, and what each holds past them on its own row.
    const stretches = (letter, lo, hi) => told(letter, "by")
      .map(stretch => ({...stretch, from: Math.max(stretch.from, lo), to: Math.min(stretch.to, hi)}))
      .filter(stretch => stretch.to > stretch.from);
    const trunkFirst = Math.min(fork.first.a, fork.first.b);
    // The three lanes are one tree: whether it has forked, and on which turn.
    const tree = {forked: fork.forked, turn: fork.turn, lean: debugSplit.locked ? 0 : 5};
    debugDrawLane("", {name: "", ...axis, first: trunkFirst, last: fork.turn,
      turn: Math.min(panes.a.turn ?? from, panes.b.turn ?? from), fork: null, kept: null, turns: null, beyond: null,
      by: stretches(told("a", "by").length ? "a" : "b", trunkFirst, fork.turn),
      free: Math.max(fork.last.a, fork.last.b) <= fork.turn && !beyond("a") && !beyond("b"),
      under: [...told("a", "eras"), ...told("b", "eras")].filter(era => onTrunk(era.turn)), tree,
      travel: panes.a.turn !== null && panes.b.turn !== null && !debugSplit.busy && !running,
      goal, idle: false, marker: one ? stands.a : null, branches: fork.forked ? fork.turn : null});
    // A half has no timeline of its own to show them on, so its save points,
    // its breakpoint turns and the eras it has been through stand on its
    // lane (`pane-marks`): on its branch, and on the trunk for the turns the
    // two share. What both halves hold on the trunk is said once there, as
    // on any timeline. An era is the world's, and is named once at the foot.
    // A save point and a breakpoint turn are a half's own to go to, so they
    // are said once only while the halves are locked and go together: a
    // save point at the top, a breakpoint turn at the foot.
    const same = {
      flags: (mine, theirs) => mine.turn === theirs.turn && mine.name === theirs.name,
      stops: (mine, theirs) => mine === theirs,
      eras: (mine, theirs) => mine.turn === theirs.turn && mine.era === theirs.era,
    };
    const both = (kind, letter, mark) => onTrunk(kind === "stops" ? mark : mark.turn) &&
      told(letter === "a" ? "b" : "a", kind).some(other => same[kind](mark, other));
    const once = (kind, letter, mark) => debugSplit.locked && both(kind, letter, mark);
    const marks = {
      a: {flags: told("a", "flags"), stops: told("a", "stops").filter(turn => !once("stops", "a", turn)),
        eras: told("a", "eras").filter(era => !both("eras", "a", era))},
      b: {flags: told("b", "flags").filter(point => !once("flags", "b", point)), stops: told("b", "stops"),
        eras: told("b", "eras").map(era => both("eras", "b", era) ? {...era, both: true, still: !debugSplit.locked} : era)},
    };
    for (const letter of ["a", "b"]) {
      const pane = panes[letter];
      debugDrawLane(letter, {name: letter.toUpperCase(), ...axis, first: fork.first[letter], last: fork.last[letter],
        turn: pane.turn ?? from, fork: fork.turn, kept: null, turns: null, ...marks[letter], named: told(letter, "flags"),
        by: stretches(letter, fork.turn, kept(letter)), played: told(letter, "by"),
        letter: fork.forked ? letter.toUpperCase() : null, seated: pane.strategy,
        beyond: beyond(letter), goal: fork.forked ? goal : null, idle: pane.turn !== null && !debugSplit.busy && !running,
        marker: one ? null : stands[letter], over: one ? stands.a : null,
        branches: fork.forked ? fork.turn : null, tree});
      const agent = debugLanes[letter].agent;
      debugOfferAgents(agent, pane.strategy);
      agent.disabled = debugSplit.busy || running;
      debugLanes[letter].close.disabled = debugSplit.busy || running;
    }
    // The lock that joins the two halves. It closes only on two halves that
    // stand on the same turn, and keeps them there.
    const together = panes.a.turn !== null && panes.a.turn === panes.b.turn;
    bar.classList.toggle("dbg-joined", debugSplit.locked && together);
    const lock = bar.querySelector("[data-dbg-lock]");
    lock.disabled = debugSplit.busy || running || (!debugSplit.locked && !together);
    lock.setAttribute("aria-pressed", debugSplit.locked ? "true" : "false");
    const lockTitle = debugSplit.locked
      ? "Locked: the two simulators stay on the same turn, and this one panel plays both. Click to unlock them, and each keeps a turn and a panel of its own"
      : together ? "Unlocked: each simulator keeps its own turn and its own panel. Click to lock them on this turn"
      : "Unlocked: each simulator keeps its own turn and its own panel. They can be locked when both stand on the same turn";
    if (lock.title !== lockTitle) { lock.title = lockTitle; lock.setAttribute("aria-label", lockTitle.split(".")[0]); }
    if (deck === null) { debugDrawAsk(bar); debugDrawSearch(bar); if (measure) debugDock(bar); return; }
  } else {
    bar.classList.remove("dbg-joined", "dbg-forked");
    // The timeline: the whole game, the turns played on it, the save points
    // and the breakpoint turns, and where a run is headed.
    const first = timeline.first ?? state.turn, last = Math.max(timeline.last ?? state.turn, state.turn);
    const kept = timeline.turns || [], thinned = kept.length < last - first + 1;
    debugDrawLane("a", null);
    debugDrawLane("b", null);
    const beyond = timeline.beyond || null;
    debugDrawLane("", {name: "", ...debugAxis(first, Math.max(last, beyond ? beyond.to : 0)), first, last, turn: state.turn,
      fork: null, beyond, kept: thinned ? new Set(kept) : null, turns: thinned ? kept : null,
      flags: [...(timeline.saves || [])].sort((a, b) => a.turn - b.turn || a.id - b.id), stops: timeline.breakpoints.turns,
      lines: debugBreakLines(timeline, first, last),
      by: timeline.played_by || [], played: timeline.played_by || [],
      travel: deck.idle && !debugRunning,
      eras: timeline.eras || [], goal: debugRunning ? debugGoal : null, idle: deck.idle});
  }
  debugDrawDeck(bar, deck);
  debugDrawAsk(bar);
  debugDrawSearch(bar);
  if (DEBUG_PANE !== null) { debugPaneDeck(deck); debugPaneMarks(timeline); }
  if (measure) debugDock(bar);
  if (debugSightWanted(timeline)) setTimeout(debugForesee, 0);
}
// What the box asks, or null. Both are asked on the one timeline, and
// nothing is done until one is answered. `branch`: Branch was pressed, and
// the branch is made when the Civ AI that plays it is picked (operator
// 2026-11-24: "players can branch if they want (they should choose the new
// strategy when they branch)"). `switch`: another Civ AI was picked on the one
// timeline, and the timeline either plays on with it or a branch does.
// A question stays until it is answered, the box is pressed anywhere else,
// or Escape: unanswered, there is no branch and the timeline keeps its Civ AI.
let debugAsk = null;
function debugDrawAsk(bar) {
  const box = document.getElementById("dbg-ask");
  // A question about the one timeline is gone when the timeline has branched.
  if (debugAsk && debugSplit !== null) debugAsk = null;
  const branch = bar.querySelector('[data-dbg="split"]');
  branch.classList.toggle("on", debugAsk !== null && debugAsk.kind === "branch");
  branch.setAttribute("aria-expanded", branch.classList.contains("on") ? "true" : "false");
  box.hidden = debugAsk === null;
  if (!debugAsk) return;
  const offered = [...((document.getElementById("autoplaystrategy") || {}).options || [])];
  let anchor, html;
  if (debugAsk.kind === "branch") {
    // Every Civ AI the page offers, as its picker lists them, the best
    // first. The one this timeline has may play the branch as well.
    const seated = debugStrategy(), busy = branch.disabled;
    anchor = branch;
    html = `<h4 id="dbg-ask-title">Branch on turn ${state.turn}: which Civ AI plays it?</h4>` +
      `<p>A second timeline begins on turn ${state.turn}, beside this one, and the Civ AI picked plays it from there. This one keeps ${escapeAttr(debugAgentName(seated))}.</p>` +
      `<div class="dbg-ask-list">${offered.map(option =>
        `<button data-dbg-ask="seat" value="${escapeAttr(option.value)}"${busy ? " disabled" : ""}>` +
        `<span>${escapeAttr(option.textContent)}</span>${option.value === seated ? "<small>plays this timeline</small>" : ""}</button>`).join("")}</div>` +
      `<div class="dbg-line"><button data-dbg-ask="close" title="No branch: the timeline stays one">Cancel</button></div>`;
  } else {
    const picked = escapeAttr(debugAgentName(debugAsk.name)), seated = escapeAttr(debugAgentName(debugStrategy()));
    anchor = document.getElementById("dbg-agent");
    html = `<h4 id="dbg-ask-title">${picked} from turn ${state.turn}: switch or branch?</h4>` +
      `<p><b>Switch</b>: this timeline plays on with ${picked} from here.</p>` +
      `<p><b>Branch</b>: a second timeline begins here with ${picked}, beside this one, which keeps ${seated}.</p>` +
      `<div class="dbg-line"><button data-dbg-ask="switch" title="This timeline plays on with ${picked}">Switch</button>` +
      `<button data-dbg-ask="branch" title="A second timeline from this turn, played by ${picked}">⑂ Branch</button>` +
      `<button data-dbg-ask="close" title="Keep ${seated}">Cancel</button></div>`;
  }
  if (box.dataset.drawn !== html) { box.dataset.drawn = html; box.innerHTML = html; }
  // Under the picker it is about, and inside the window, unless it was put
  // somewhere else: each of the two questions keeps the place it was given.
  box.dataset.place = `dbg-ask-${debugAsk.kind}`;
  debugPlaceWindow(box, bar, anchor);
}
// The Civ AI picked for the one timeline is seated: the page's own picker is
// the one every step reads, and the engine is told at once (`debugSeat`,
// whose asking of the engine is what this gives back, or null).
function debugSwitchSeat(name) {
  const page = document.getElementById("autoplaystrategy");
  if (page) { page.value = name; page.dispatchEvent(new Event("change", {bubbles: true})); }
  drawDebugBar(false);
  return debugSeat();
}
function debugAnswer(button) {
  const ask = debugAsk, how = button.dataset.dbgAsk;
  if (!ask || button.disabled) return;
  debugAsk = null;
  if (how === "switch" && ask.kind === "switch") debugSwitchSeat(ask.name);
  else if ((how === "seat" && ask.kind === "branch") || (how === "branch" && ask.kind === "switch")) {
    // Side by side, at once: the strip between the halves stacks them
    // instead, and brings up head to head.
    debugPopOpen = null;
    debugSplitOpen("row", null, ask.kind === "branch" ? button.value : ask.name);
    // A timeline that could not branch just now is still being asked about.
    if (!debugSplit) debugAsk = ask;
  }
  drawDebugBar();
}
// A press anywhere but on the question lets it go. It is heard on its way
// down, before the bar keeps it from the map. Branch lets go of its own
// question when it is pressed again, which is its click's to do.
document.addEventListener("pointerdown", event => {
  const pressed = event.target instanceof Element ? event.target : null;
  if (!debugAsk || (pressed && pressed.closest("#dbg-ask"))) return;
  if (pressed && debugAsk.kind === "branch" && pressed.closest('[data-dbg="split"]')) return;
  debugAsk = null;
  drawDebugBar(false);
}, true);
// The game opens on its one timeline, with one Civ AI, the best there is
// (operator 2026-11-24: "ai debugger should start with 1 strategy (maybe the
// best strategy). players can branch if they want (they should choose the new
// strategy when they branch)"). A second timeline is Branch's to make.
//
// The best is the top-rated Civ AI on offer: the highest Elo anything has
// measured, which is the first one the page's picker lists
// (`strategiesByElo`). With nothing rated, the seat is left as it is.
function debugBestAgent() {
  const roster = typeof RULES !== "undefined" && RULES && Array.isArray(RULES.strategies) ? RULES.strategies : [];
  const offered = [...((document.getElementById("autoplaystrategy") || {}).options || [])].map(option => option.value);
  let best = null;
  for (const entry of roster) {
    if (entry.elo === null || entry.elo === undefined || !offered.includes(entry.name)) continue;
    if (best === null || Number(entry.elo) > Number(best.elo)) best = entry;
  }
  return best ? best.name : null;
}
// A game may still be opened on two tracks, as every game was from 2026-11-22
// until then, its timeline branched where it stands the first time it is
// drawn: a page asked for with `tracks=2` opens so, and Doctor Strange's
// Civ 6, which is two worlds, unless it is asked for with `tracks=1`. With
// no room for two halves (a small device, a narrow window), or with the seat
// played by hand in the Single player view, a game opens on the one timeline
// whatever was asked.
const DEBUG_TRACKS_ASKED = (() => {
  try { return {1: 1, 2: 2}[new URLSearchParams(location.search).get("tracks")] || null; } catch (_) { return null; }
})();
const DEBUG_TWO_TRACKS_FROM = 900;
// The game that was opened, by its seed, and whether the page has seated the
// best Civ AI. That is done once a page, for its first game: after it the
// seat is whoever was picked for it, through every new game.
let debugOpenedGame = null, debugOpenedOnBest = false;
function debugOpenTheGame() {
  if (DEBUG_PANE !== null || debugSplit || !debugTimeline()) return;
  const game = String(state.seed ?? "");
  if (debugOpenedGame === game) return;
  // A game in the middle of something is opened when it is next drawn idle.
  if (debugBusy || debugRunning || autoplaying || turnAdvancePending) return;
  // Doctor Strange's Civ 6 seats its own worlds, and in Edge of Tomorrow the
  // seat is the person's, with the Civ AI they last picked beside it.
  if (!debugOpenedOnBest && !STRANGE && !TOMORROW) {
    debugOpenedOnBest = true;
    const best = debugBestAgent();
    // The engine is told, and the game is opened when it has answered.
    const told = best && best !== debugStrategy() ? debugSwitchSeat(best) : null;
    if (told) { told.then(() => debugOpenTheGame(), () => {}); return; }
  }
  debugOpenedGame = game;
  const two = DEBUG_TRACKS_ASKED === null ? STRANGE : DEBUG_TRACKS_ASKED === 2;
  if (!two || DEBUG_SMALL_DEVICE || SOLO_UI || window.innerWidth < DEBUG_TWO_TRACKS_FROM) return;
  debugSplitOpen("row", null, null, true);
}
// The deck under the timeline, and the panels that open from it, drawn from
// one account of a world (`debugDeckNow`): this page's own, or, on the host
// of a locked split, the one half A sent.
function debugDrawDeck(bar, deck) {
  const timeline = deck.timeline;
  const idle = deck.idle && !(debugSplit && debugSplit.busy);
  const set = (action, enabled) => {
    const button = bar.querySelector(`[data-dbg="${action}"]`);
    if (button) button.disabled = !enabled;
  };
  const first = timeline.first ?? deck.turn, last = Math.max(timeline.last ?? deck.turn, deck.turn);
  set("back", idle && timeline.can_back);
  set("back10", idle && deck.turn > first);
  set("undo", idle && timeline.can_undo);
  set("first", idle && deck.turn > first);
  set("next", idle);
  set("next10", idle);
  set("end", idle);
  const {eraBack, eraNext} = deck;
  set("era", idle && eraBack !== null);
  set("era-next", idle && (eraNext !== null || !deck.lastEra));
  const titled = (action, title) => {
    const button = bar.querySelector(`[data-dbg="${action}"]`);
    if (button.title !== title) button.title = title;
  };
  titled("era", eraBack
    ? `Era Back: back to turn ${eraBack.turn}, the first turn of the ${debugEraName(eraBack)} era`
    : "Era Back: this is the first turn of the first era on the timeline");
  titled("era-next", eraNext
    ? `Era Next: on to turn ${eraNext.turn}, the first turn of the ${debugEraName(eraNext)} era`
    : deck.lastEra ? "Era Next: this is the last era"
    : `Era Next: the Civ AI plays on until the ${debugEraName({era: deck.era + 1})} era begins. A breakpoint stops it sooner, and so does Stop`);
  set("step-action", idle);
  set("step-seat", idle);
  set("reveal", idle);
  bar.querySelector('[data-dbg="reveal"]').classList.toggle("on", timeline.reveal === true);
  const soloUi = bar.querySelector('[data-dbg="solo-ui"]');
  soloUi.classList.toggle("on", deck.solo);
  soloUi.setAttribute("aria-pressed", deck.solo ? "true" : "false");
  set("save", idle);
  set("point", idle);
  set("assist", idle);
  // Auto-play is also the Stop of whatever is being played, and counts its
  // turns: of the turns asked for, or so far when it plays until stopped.
  const run = bar.querySelector('[data-dbg="autoplay"]');
  run.disabled = !(deck.running || idle);
  const runSign = deck.running ? (deck.stopping ? "…" : "⏹") : "▶▶";
  const runCount = !deck.running ? "" : deck.asked === null ? `${deck.played}` : `${deck.played}/${deck.asked}`;
  for (const [id, text] of [["dbg-run-sign", runSign], ["dbg-run-word", deck.running ? "Stop" : "Auto-play"], ["dbg-run-count", runCount]]) {
    const part = document.getElementById(id);
    if (part.textContent !== text) part.textContent = text;
  }
  run.classList.toggle("on", deck.running);
  const agent = document.getElementById("dbg-agent");
  // While the box asks what to do with another Civ AI, the picker shows it.
  debugOfferAgents(agent, debugAsk && debugAsk.kind === "switch" ? debugAsk.name : deck.strategy);
  agent.disabled = !idle;
  agent.closest(".dbg-field").hidden = agent.options.length === 0;
  document.getElementById("dbg-turns").disabled = !idle;

  const turnBox = document.getElementById("dbg-turn");
  turnBox.disabled = !idle;
  // A number being typed is left alone, unless the turn moved under it.
  if (document.activeElement !== turnBox || turnBox.dataset.turn !== String(deck.turn)) turnBox.value = deck.turn;
  turnBox.dataset.turn = String(deck.turn);
  document.getElementById("dbg-limit").textContent = deck.limit === null ? "" : `/ ${deck.limit}`;
  const points = [...(timeline.saves || [])].sort((a, b) => a.turn - b.turn || a.id - b.id);
  const breaks = timeline.breakpoints;
  const kept = timeline.turns || [], thinned = kept.length < last - first + 1;
  // How much of the game can be gone back to, said by the turn box.
  const played = first === last ? `Turn ${first} is kept`
    : `Turns ${first} to ${last} played, ${thinned ? `${kept.length} kept` : "every one kept"}`;
  const turnLabel = turnBox.closest(".dbg-turnbox");
  const turnTitle = `${played}. Type a turn to go to it: one already played is put back as it stood` +
    `${thinned ? " (a turn no longer kept answers with the nearest before it)" : ""}, one still ahead is played to by the Civ AI`;
  if (turnLabel.title !== turnTitle) { turnLabel.title = turnTitle; turnLabel.dataset.played = played; }
  document.getElementById("dbg-point-count").textContent = points.length ? `${points.length}` : "";
  debugDrawn(document.getElementById("dbg-point-list"), points.length ? points.map(point =>
    `<div class="dbg-line"><span style="flex:1;width:auto;color:inherit">${point.auto ? "" : "<b>"}${escapeAttr(point.name)}${point.auto ? "" : "</b>"}` +
    ` · turn ${point.turn}${point.acted ? `, ${point.acted} action${point.acted === 1 ? "" : "s"} in` : ""}</span>` +
    `<button data-dbg-goto="${point.id}" title="Put this world back on the table">Go</button>` +
    `<button data-dbg-drop="${point.id}" title="Remove this save point" aria-label="Remove this save point">×</button></div>`).join("")
    : `<p>No save points yet.</p>`);
  // The host of a split asks nothing of its own world, which waits. A page
  // of its own keeps its turns under the tab's ceiling (`debugRoomMB`).
  if (DEBUG_PANE === null && !debugSplit && idle) debugRoomAsk(timeline);
  // A world of Doctor Strange's Civ 6 keeps its share of the room instead,
  // and a half of the AI Debugger's split its engine's line.
  if (DEBUG_PANE !== null && idle) strangePaneRoom(timeline);

  const layersOn = timeline.layers.filter(layer => layer.on).length;
  document.getElementById("dbg-layer-count").textContent = layersOn ? `${layersOn}` : "";
  const conditions = breaks.conditions || [];
  const breakCount = breaks.turns.length + breaks.categories.length + breaks.phrases.length +
    conditions.length + (breaks.important ? 1 : 0) + (breaks.eras ? 1 : 0) + (breaks.nuclear ? 1 : 0);
  document.getElementById("dbg-break-count").textContent = breakCount ? `${breakCount}` : "";
  // A pane is already one half of a split, and a split does not split again.
  const split = bar.querySelector('[data-dbg="split"]');
  split.hidden = DEBUG_PANE !== null || debugSplit !== null;
  split.disabled = !idle;
  for (const name of ["layers", "breaks", "saves"]) {
    const pop = document.getElementById(`dbg-pop-${name}`), button = bar.querySelector(`[data-dbg-pop="${name}"]`);
    pop.hidden = debugPopOpen !== name;
    button.classList.toggle("on", debugPopOpen === name);
    if (pop.hidden) continue;
    // A panel opens under its own button, and stays inside the window.
    debugPlaceWindow(pop, bar, button);
  }

  // The lists are the engine's: a layer or an event category added there
  // appears here without this file being told.
  const layerList = document.getElementById("dbg-layer-list");
  if (layerList.childElementCount !== timeline.layers.length) {
    layerList.innerHTML = timeline.layers.map(layer =>
      `<label><input type="checkbox" data-dbg-layer="${escapeAttr(layer.id)}"> ${escapeAttr(layer.label)}</label>`).join("");
  }
  for (const layer of timeline.layers) {
    const box = layerList.querySelector(`[data-dbg-layer="${layer.id}"]`);
    if (box) box.checked = layer.on;
  }
  const categoryList = document.getElementById("dbg-category-list");
  const known = breaks.known_categories || [];
  if (categoryList.childElementCount !== known.length) {
    categoryList.innerHTML = known.map(category =>
      `<label><input type="checkbox" data-dbg-category="${escapeAttr(category)}"> ${escapeAttr(category)}</label>`).join("");
  }
  for (const box of categoryList.querySelectorAll("[data-dbg-category]"))
    box.checked = breaks.categories.includes(box.dataset.dbgCategory);
  const pop = document.getElementById("dbg-pop-breaks");
  const field = name => pop.querySelector(`[data-dbg-field=${name}]`);
  const offer = (name, options) => {
    const select = field(`cond-${name}`), drawn = JSON.stringify(options);
    if (select.dataset.drawn === drawn) return;
    const held = select.value;
    select.dataset.drawn = drawn;
    select.innerHTML = options.map(([value, label]) =>
      `<option value="${escapeAttr(String(value))}">${escapeAttr(label)}</option>`).join("");
    if (options.some(([value]) => String(value) === held)) select.value = held;
  };
  offer("seat", deck.seats.filter(seat => seat.major).map(seat => [seat.id, seat.name]));
  offer("key", deck.readable.map(key => [key, key.replaceAll("_", " ")]));
  offer("test", (breaks.known_tests || []).map(test => [test, test]));
  field("cond-value").hidden = field("cond-test").value === "changes";
  const conditionList = document.getElementById("dbg-condition-list");
  const conditionRows = conditions.map((condition, index) => {
    const who = deck.seats.find(seat => seat.id === condition.seat);
    const line = condition.test === "changes" ? "changes" : `${condition.test} ${condition.value}`;
    return `<div class="dbg-line"><span style="flex:1;width:auto;color:inherit">` +
      `${escapeAttr(who ? who.name : `seat ${condition.seat}`)} · ${escapeAttr(condition.key.replaceAll("_", " "))} ${escapeAttr(line)}</span>` +
      `<button data-dbg-cond-remove="${index}" title="Remove this condition" aria-label="Remove this condition">×</button></div>`;
  }).join("");
  if (conditionList.innerHTML !== conditionRows) conditionList.innerHTML = conditionRows;
  if (document.activeElement !== field("turns")) field("turns").value = breaks.turns.join(", ");
  if (document.activeElement !== field("phrases")) field("phrases").value = breaks.phrases.join(", ");
  field("important").checked = breaks.important;
  field("every_seat").checked = breaks.every_seat;
  field("eras").checked = breaks.eras === true;
  field("nuclear").checked = breaks.nuclear === true;

  const note = document.getElementById("dbg-note");
  const html = escapeAttr(deck.note) +
    (deck.show ? `<button data-dbg="show" title="Centre the map there">Show</button>` : "");
  if (note.innerHTML !== html) { note.innerHTML = html; note.title = deck.note; }
}

// ------------------------------------------------------------------ split
//
// Two simulators in one window. The page that was playing becomes the host:
// it covers itself with two frames, and each frame is this same page again,
// playing a session of its own that the engine forks from the host's
// (`src/server/panes.rs`, named by `?pane=`). Both halves start on the host's
// world as it stood when the screen was split, with every turn behind it
// still there to walk back to, and from there each plays its own history.
//
// The split opens locked. Locked, the two stay on the same turn: whatever
// moves one — ending the turn, a step, a walk along the timeline — is
// repeated in the other the same way, and what one is asked that leaves its
// turn where it was (a layer, a breakpoint, one action stepped) the other is
// asked too. A lock joins the two halves in the host's bar to say so. Clicking
// it opens it, and then neither half knows about the other. It closes again
// only on two halves that stand on the same turn. The view can be linked
// too, so that moving the map in one half moves it in the other.
//
// Locked, the two halves are one thing to play, so there is one control
// panel: the host's deck. Each half puts its own deck away and sends the host
// what was on it, the host draws its deck from half A's, and a press on the
// host's deck is passed to A's own deck, which does what it would have done
// had it been pressed there. B follows. Unlocked, each half shows its own.
//
// Two locked halves that are the same world, seated with the same Civ AI,
// are played once: A names B as its twin in what it asks the engine, the
// engine makes B a copy of the answer when the two really are the same
// (`src/server/panes.rs`), and B's page only reads its state.
//
// Host and panes talk by `postMessage` on their own origin:
//   pane → host  pane-turn    the turn it is on, how it got there, and whether its twin was played with it
//   host → pane  pane-follow  go to this turn the same way, or read the state its twin's play left
//   pane → host  pane-did     what it did that left its turn where it was
//   host → pane  pane-op      do this too, or (from the host's own deck) do this
//   host → pane  pane-lock    whether the halves are locked, and who its twin is
//   pane → host  pane-deck    what is on its deck, while it is put away
//   host → pane  pane-press   a press on the host's deck, for its own to take
//   host → pane  pane-change  a field changed on the host's deck
//   host → pane  pane-solo    which of the two views to show
//   host → pane  pane-go      go to this turn of its own accord
//   host → pane  pane-copy    become a copy of the other half
//   pane → host  pane-camera  where its map is looking
//   host → pane  pane-look    look there too; and, to a half not yet shown, where to open looking
//   pane → host  pane-shown   it has painted itself looking there
//   host → pane  pane-seen    the two are shown: its map is its own to move
// Closing the split is the host's own request to the engine: the half that
// is kept becomes the host's session (`op: "adopt"`), timeline and all.
//
// The host's bar stays up across the top of the window while the screen is
// split, and its timeline is the two halves': one timeline up to the turn it
// was branched on, and a branch for each from there (`debugSplitFork`). Each half has a slider the length of the game,
// A's along the top of the timeline and B's along the bottom. It is the hand
// on its half, and on both halves while they are locked.

const DEBUG_SPLIT_STYLE = `
  #debugsplit {
    position: fixed; inset: var(--dbg-top, 0px) 0 0 0; z-index: 100000; display: grid; background: #050b0a;
    grid-template-columns: minmax(0, var(--dbg-a, 1fr)) auto minmax(0, var(--dbg-b, 1fr));
    grid-template-rows: minmax(0, 1fr);
  }
  #debugsplit.dbg-col {
    grid-template-columns: minmax(0, 1fr);
    grid-template-rows: minmax(0, var(--dbg-a, 1fr)) auto minmax(0, var(--dbg-b, 1fr));
  }
  #debugsplit iframe { width: 100%; height: 100%; border: 0; display: block; background: #07110f; }
  #debugsplit.dbg-dragging iframe { pointer-events: none; }
  /* The halves load where they will stand, unseen: the window shows what it
     showed until both are there (debugSplitReveal). Until then, and while a
     branch is being closed, nothing in the split takes a press. */
  #debugsplit.dbg-unseen { opacity: 0; }
  #debugsplit.dbg-unseen > *, #debugsplit.dbg-closing > * { pointer-events: none; }
  #debugsplit .dbg-divider {
    display: flex; flex-direction: column; align-items: stretch; justify-content: center; gap: 6px;
    width: 68px; padding: 6px 5px; background: #0a1512; border-inline: 1px solid #35504a;
    color: var(--text, #e8e3d2); font: 11px/1.2 system-ui, sans-serif; user-select: none;
    cursor: col-resize; touch-action: none;
  }
  #debugsplit.dbg-col .dbg-divider {
    flex-direction: row; align-items: center; width: auto; height: 36px; padding: 4px 8px;
    border-inline: 0; border-block: 1px solid #35504a; cursor: row-resize;
  }
  #debugsplit .dbg-divider button {
    min-height: 26px; padding: 3px 5px; border-radius: 7px; cursor: pointer; font: inherit;
    color: inherit; background: #14231f; border: 1px solid #35504a; white-space: normal;
  }
  #debugsplit.dbg-col .dbg-divider button { white-space: nowrap; padding: 3px 9px; }
  #debugsplit .dbg-divider button:hover:not(:disabled) { border-color: #e7c873; }
  #debugsplit .dbg-divider button:disabled { opacity: .4; cursor: default; }
  #debugsplit .dbg-divider button.on { color: #10140f; background: #e7c873; border-color: transparent; font-weight: 700; }
  #debugsplit .dbg-pane-name { text-align: center; font-weight: 700; color: #e7c873; font-variant-numeric: tabular-nums; }
  #debugsplit .dbg-gap { flex: 1; }
  #debugsplit [data-dbg-split="flip"] { font-size: 15px; }
  /* Head to head: the strip widens to hold each half's Civ AI and numbers.
     It is as wide as a picker's longest listing asks, Elo and whole name. */
  #debugsplit.dbg-versus-on:not(.dbg-col) .dbg-divider { width: min-content; min-width: 138px; }
  #debugsplit .dbg-versus { display: flex; flex-direction: column; gap: 5px; cursor: default; min-width: 0; }
  /* Head to head is open as a split opens, so the strip holds everything it
     has in a window of any height: the two copies stand side by side, and so
     do View and the switch of layout, where the strip is wide enough for
     two, and in a window too short for the rest the numbers scroll inside
     the strip. What drops a branch is never off the screen. */
  #debugsplit .dbg-pair { display: contents; }
  #debugsplit.dbg-versus-on:not(.dbg-col) .dbg-pair { display: flex; gap: 6px; }
  #debugsplit.dbg-versus-on:not(.dbg-col) .dbg-pair > button { flex: 1 1 0; min-width: 0; white-space: nowrap; }
  #debugsplit:not(.dbg-col) .dbg-versus { flex: 0 1 auto; min-height: 0; overflow: hidden auto; scrollbar-width: thin; }
  /* A window too short for the chart as well goes without it first, so the
     turn to play both to stays in view. */
  @media (max-height: 800px) { #debugsplit:not(.dbg-col) .dbg-vs-chart { display: none; } }
  #debugsplit .dbg-versus[hidden] { display: none; }
  #debugsplit.dbg-col .dbg-versus { flex-direction: row; align-items: center; gap: 10px; overflow: hidden; }
  #debugsplit .dbg-versus select, #debugsplit .dbg-versus input {
    min-width: 0; height: 24px; padding: 0 4px; border-radius: 7px; font: inherit; line-height: normal;
    color: inherit; background: #060d0b; border: 1px solid #35504a;
  }
  #debugsplit .dbg-versus select { flex: none; width: auto; max-width: none; }
  #debugsplit .dbg-versus input { width: 44px; text-align: center; }
  #debugsplit .dbg-vs-line { display: flex; align-items: center; gap: 4px; min-width: 0; }
  #debugsplit .dbg-vs-line > span { font-weight: 700; }
  #debugsplit .dbg-vs-a { color: #ff8f6b; }
  #debugsplit .dbg-vs-b { color: #6fd3e0; }
  #debugsplit .dbg-vs-stat { cursor: pointer; }
  #debugsplit .dbg-vs-stat.charted i { color: inherit; text-decoration: underline; }
  #debugsplit .dbg-vs-chart { width: 100%; height: 48px; border-radius: 6px; background: #060d0b; border: 1px solid #22332e; }
  #debugsplit.dbg-col .dbg-vs-chart { display: none; }
  #debugsplit .dbg-vs-table { display: grid; gap: 3px; font-variant-numeric: tabular-nums; }
  #debugsplit.dbg-col .dbg-vs-table { display: flex; gap: 12px; white-space: nowrap; }
  #debugsplit .dbg-vs-stat { display: grid; grid-template-columns: 1fr 1fr; column-gap: 6px; }
  #debugsplit.dbg-col .dbg-vs-stat { display: inline-flex; gap: 5px; }
  #debugsplit .dbg-vs-stat i { grid-column: 1 / -1; font-style: normal; color: #9fb3ab; }
  #debugsplit .dbg-vs-stat b { font-weight: 400; text-align: center; }
  #debugsplit .dbg-vs-stat b.lead { font-weight: 700; color: #e7c873; }
`;
// The host's record of the split, or null when the screen is whole.
let debugSplit = null;

// ---- one move
//
// One screen and two change places in one move (operator 2026-11-22: "i do
// not like the whole transition between ai debugger 1 screen and 2 screens").
// A half is a whole page and takes about a second to be there, and the page
// under the halves takes a moment to become the half that was kept. None of
// that is shown. The window is held as it stands while the other picture is
// made ready out of sight (`debugHold`): the box is not drawn again and takes
// no press. Then the one picture takes the other's place, once
// (`debugSwap`): the box goes from one shape to the other, the map goes to
// the half that carries it on (or the kept half to the map), and the rest
// comes and goes around them.
let debugHeld = false;
// Whether one picture is taking the other's place right now.
let debugMoving = false;
// A browser that is not painting (a window nobody can see) may leave a move
// waiting to begin, or to end. The change is not left waiting with it, and
// neither is the word that the move is over.
const DEBUG_SWAP_PATIENCE_MS = 500, DEBUG_SWAP_OVER_MS = 2000;
function debugHold(on) {
  debugHeld = on;
  document.documentElement.classList.toggle("dbg-held", on);
}
// Where the middle of what shows of this page's map stands, in the page's
// own window. A half is asked by the host, which reads it off the half's
// frame.
function debugFocusNow() {
  const canvas = document.getElementById("map");
  if (!canvas || typeof mapFocusPoint !== "function") return null;
  const box = canvas.getBoundingClientRect(), focus = mapFocusPoint();
  return {x: box.left + focus.x, y: box.top + focus.y};
}
// What the map is shown in, on one screen or in one half, and where in it
// the middle of the map stands: the map area of this page, or a half's frame.
function debugStage(frame = null) {
  try {
    if (frame) return {element: frame, focus: frame.contentWindow.debugFocusNow()};
    const element = document.getElementById("maparea"), focus = debugFocusNow();
    const box = element.getBoundingClientRect();
    return {element, focus: focus ? {x: focus.x - box.left, y: focus.y - box.top} : null};
  } catch (_) {
    return {element: frame || document.getElementById("maparea"), focus: null};
  }
}
// `change` makes the window the other picture. Where the browser can show one
// picture becoming another (a view transition) it does: the box goes from
// its one shape to its other, and the stage the map was shown in (`from`)
// goes to the one it is shown in once the change is made (what `to` gives),
// each carried so that the middle of its map is where the other's is. With
// no `from` the whole window gives way at once: a veil lifting has no map
// under it to carry anywhere. Where the browser cannot, or the person has
// asked for less motion, the change is simply made. Either way it is made
// once, and what comes back settles when the move is over.
function debugSwap(change, from = null, to = null) {
  let made = false;
  const make = () => { if (!made) { made = true; change(); } };
  const still = typeof document.startViewTransition !== "function" || document.hidden ||
    (typeof matchMedia === "function" && matchMedia("(prefers-reduced-motion: reduce)").matches);
  if (still) { make(); return Promise.resolve(); }
  const root = document.documentElement, named = new Set();
  const name = (element, as) => { if (element) { element.style.viewTransitionName = as; named.add(element); } };
  let waiting = 0, ending = 0, ended = false;
  const over = () => {
    if (ended) return;
    ended = true;
    clearTimeout(waiting);
    clearTimeout(ending);
    for (const element of named) element.style.viewTransitionName = "";
    root.classList.remove("dbg-swapping");
    root.style.removeProperty("--dbg-carry-x");
    root.style.removeProperty("--dbg-carry-y");
    debugMoving = false;
  };
  debugMoving = true;
  root.classList.add("dbg-swapping");
  if (from) { name(debugBar, "dbg-bar"); name(from.element, "dbg-stage"); }
  try {
    const move = document.startViewTransition(() => {
      if (made) return;
      // One thing at a time carries a name: the old picture has been taken.
      if (from) from.element.style.viewTransitionName = "";
      make();
      const next = from && to ? to() : null;
      if (!next) return;
      name(next.element, "dbg-stage");
      if (from.focus && next.focus) {
        root.style.setProperty("--dbg-carry-x", `${Math.round(next.focus.x - from.focus.x)}px`);
        root.style.setProperty("--dbg-carry-y", `${Math.round(next.focus.y - from.focus.y)}px`);
      }
    });
    waiting = setTimeout(() => {
      if (made) return;
      make();
      try { move.skipTransition(); } catch (_) {}
    }, DEBUG_SWAP_PATIENCE_MS);
    ending = setTimeout(() => {
      make();
      try { move.skipTransition(); } catch (_) {}
      over();
    }, DEBUG_SWAP_OVER_MS);
    return move.finished.then(over, () => { make(); over(); });
  } catch (_) {
    make(); over();
    return Promise.resolve();
  }
}
// The veil over a game that opens on its two tracks, until the two are there.
function debugVeil(on) {
  let veil = document.getElementById("dbg-veil");
  if (!on) { if (veil) veil.remove(); return; }
  if (veil) return;
  veil = document.createElement("div");
  veil.id = "dbg-veil";
  veil.setAttribute("role", "status");
  veil.innerHTML = "<b>CIVVIS</b><span>Opening both timelines</span>";
  document.body.appendChild(veil);
}
// Head to head: what each half says about its seat, set side by side. The
// yields are what the seat makes a turn.
const DEBUG_VERSUS_ROWS = [
  ["score", "Score", "Score"], ["cities", "Cities", "Cities"], ["population", "Population", "Population"],
  ["science", "Science", "Science per turn"], ["culture", "Culture", "Culture per turn"],
  ["gold", "Gold", "Gold per turn"], ["faith", "Faith", "Faith per turn"],
  ["military", "Military", "Military strength"],
];
// The two halves' lines in the chart, and the letters that name them.
const DEBUG_VERSUS_COLOURS = {a: "#ff8f6b", b: "#6fd3e0"};
// One of the numbers over the turns since the split, a line for each half.
// A turn a half has not reported is passed over, so a half that is behind
// simply has a shorter line.
function debugVersusChart(canvas) {
  const key = debugSplit.chart, cx = canvas.getContext("2d");
  cx.clearRect(0, 0, canvas.width, canvas.height);
  const turns = [...new Set([...debugSplit.history.a.keys(), ...debugSplit.history.b.keys()])].sort((x, y) => x - y);
  const values = [];
  for (const letter of ["a", "b"])
    for (const numbers of debugSplit.history[letter].values())
      if (typeof numbers[key] === "number") values.push(numbers[key]);
  if (turns.length < 2 || !values.length) return;
  const low = Math.min(...values), span = Math.max(...values) - low || 1;
  const first = turns[0], last = turns[turns.length - 1];
  const x = turn => 5 + (turn - first) / (last - first) * (canvas.width - 10);
  const y = value => canvas.height - 6 - (value - low) / span * (canvas.height - 12);
  cx.lineWidth = 3; cx.lineJoin = "round"; cx.lineCap = "round";
  for (const letter of ["a", "b"]) {
    cx.beginPath();
    let drawing = false;
    for (const turn of turns) {
      const numbers = debugSplit.history[letter].get(turn);
      if (!numbers || typeof numbers[key] !== "number") continue;
      if (drawing) cx.lineTo(x(turn), y(numbers[key])); else cx.moveTo(x(turn), y(numbers[key]));
      drawing = true;
    }
    cx.strokeStyle = DEBUG_VERSUS_COLOURS[letter];
    cx.stroke();
    // Where the half stands now, when it has walked back along its turns.
    const now = debugSplit.panes[letter].turn;
    if (now !== null && now > first && now < last) {
      cx.globalAlpha = .55;
      cx.fillStyle = DEBUG_VERSUS_COLOURS[letter];
      cx.fillRect(Math.round(x(now)) - 1, 0, 2, canvas.height);
      cx.globalAlpha = 1;
    }
  }
  const charted = DEBUG_VERSUS_ROWS.find(row => row[0] === key);
  canvas.title = `${charted ? charted[2] : key}, turn ${first} to ${last}. Click a number's name to chart it`;
}
function debugVersusNumber(value) {
  return typeof value === "number" && Number.isFinite(value) ? String(Math.round(value * 10) / 10) : "–";
}

function debugSplitTell(letter, message) {
  const pane = debugSplit ? debugSplit.panes[letter] : null;
  if (pane && pane.frame.contentWindow) pane.frame.contentWindow.postMessage(message, location.origin);
}
// What the host's deck is drawn from: half A's account of its own, while the
// halves are locked and A has given one. Otherwise the host shows no deck.
function debugSplitDeck() {
  return debugSplit && debugSplit.locked ? debugSplit.panes.a.deck || null : null;
}
// Where the timeline of a split branches. It branches on the turn Branch was
// pressed on, from the moment it is pressed: one timeline up to there, two
// from there, as Doctor Strange's Civ 6 draws its worlds parting (operator
// 2026-11-26: "please take more of a doctor strange's civ 6 approach.
// branches should split from 1 to 2... not 2 to 1"). Two halves that go on
// playing the same moves are still two branches, each with its own Civ AI.
// Until then the timeline stayed one rail while the two worlds were the same
// (operator 2026-11-21: "branching the timeline should keep 1 timeline where
// they share the timeline and then branch to 2 when they actually diverge in
// simulated worlds"), and a branch whose Civ AI played as A's did looked
// like two pickers joined into one timeline.
//
// It branches earlier only where the halves' worlds part earlier: when one
// has been walked back behind that turn and played on from there. Which
// worlds are the same is the engine's to say. Each half tells the host the
// turns it holds and the engine's print of each one's world (`pane-marks`,
// from `debug.turns` and `debug.worlds`), and two halves hold the same world
// on a turn they both give the same print for. A print carries everything
// played before its turn, so the last turn they agree on is the last one
// they share. A branch made behind the last turn played branches on the turn
// it was made on: A keeps the turns past it, and B, the branch, begins there
// (`op: "branch"`).
//
// Also given: the first and the last turn each half holds.
function debugSplitFork() {
  const from = debugSplit.from ?? state.turn;
  const lines = {}, first = {}, last = {};
  for (const letter of ["a", "b"]) {
    const pane = debugSplit.panes[letter], marks = pane.marks || {}, turn = pane.turn ?? from;
    let line = Array.isArray(marks.turns) && marks.turns.length > 0 && Array.isArray(marks.worlds) &&
      marks.worlds.length === marks.turns.length ? marks : null;
    // A branch that has not yet said it begins on the turn it was made on
    // (`debugSplit.branches`) still names the turns ahead of it. They are
    // not its own, and are left out as they will be.
    const cut = letter === "b" && debugSplit.branches ? from : Infinity;
    if (line && cut !== Infinity) {
      const held = line.turns.filter(kept => kept <= cut).length;
      line = held > 0 ? {turns: line.turns.slice(0, held), worlds: line.worlds.slice(0, held)} : null;
    }
    lines[letter] = line;
    first[letter] = Math.min(line ? line.turns[0] : pane.first ?? turn, turn);
    last[letter] = Math.min(cut, Math.max(line ? line.turns[line.turns.length - 1] : pane.last ?? from, turn));
  }
  // Until both halves have said what they hold, and with an engine that
  // gives no prints, they are taken to be one world up to the turn the
  // screen was split on.
  let shared = Math.min(from, last.a, last.b);
  if (lines.a && lines.b) {
    const theirs = new Map(lines.b.turns.map((turn, at) => [turn, lines.b.worlds[at]]));
    // Two halves with no world in common have nothing to share.
    shared = Math.min(first.a, first.b);
    for (let at = lines.a.turns.length - 1; at >= 0; at--) {
      if (theirs.get(lines.a.turns[at]) !== lines.a.worlds[at]) continue;
      shared = lines.a.turns[at];
      break;
    }
  }
  // A split is always a tree that has forked; `forked` is what the lanes'
  // drawing reads (`tree.forked`).
  return {turn: Math.min(shared, from), forked: true, first, last};
}
// Open or close the lock. It closes only on two halves that stand on the
// same turn, which is where it then keeps them.
function debugSplitLock(on) {
  if (!debugSplit || debugSplit.busy || debugSplit.running > 0) return;
  const {a, b} = debugSplit.panes;
  if (on && (a.turn === null || a.turn !== b.turn)) return;
  debugSplit.locked = on;
  debugPopOpen = null;
  // The decks as they stand under the new lock are still to come.
  a.deck = b.deck = null;
  debugSplit.settling = on;
  drawDebugSplit();
}
// Each half is told how the split stands whenever that changes: whether the
// two are locked, and whether the other is its twin. Two locked halves that
// seat the same Civ AI would be asked the same things; if they are also the
// same world, which only the engine knows, they are played once.
function debugSplitSync(again = null) {
  const {a, b} = debugSplit.panes;
  const twins = debugSplit.locked && a.strategy !== undefined && a.strategy === b.strategy;
  const stands = `${debugSplit.locked}:${twins}`;
  // A world past the two is told the same, and has no twin.
  for (const letter of Object.keys(debugSplit.panes)) {
    const pane = debugSplit.panes[letter], other = letter === "a" ? b : letter === "b" ? a : null;
    if (pane.told === stands && again !== letter) continue;
    pane.told = stands;
    debugSplitTell(letter, {civvis: "pane-lock", on: debugSplit.locked, twin: twins && other ? other.id : null});
  }
}
// Two halves locked after playing apart are made the same in what the one
// panel shows of them: B takes A's layers, breakpoints and view of the map.
// Halves that already agree are asked nothing, and stay the same world.
function debugSplitSettle() {
  const {a, b} = debugSplit.panes;
  if (!debugSplit.settling || !a.deck || !b.deck) return;
  debugSplit.settling = false;
  const mine = a.deck.timeline, theirs = b.deck.timeline;
  const differ = (one, other) => JSON.stringify(one) !== JSON.stringify(other);
  const ask = body => debugSplitTell("b", {civvis: "pane-op", body});
  if (differ(mine.layers, theirs.layers))
    ask({op: "layers", layers: mine.layers.filter(layer => layer.on).map(layer => layer.id)});
  if (mine.reveal !== theirs.reveal) ask({op: "reveal", on: mine.reveal === true});
  if (differ(mine.breakpoints, theirs.breakpoints)) {
    const {turns, categories, phrases, important, every_seat, eras, nuclear, conditions} = mine.breakpoints;
    ask({op: "breakpoints", turns, categories, phrases, important, every_seat, eras, nuclear: nuclear === true, conditions});
  }
}
// A control on the deck, named so that the same control can be found on
// another page's deck.
function debugRemoteKey(element) {
  if (element.id) return `#${element.id}`;
  const named = [...element.attributes].find(attribute => attribute.name.startsWith("data-dbg"));
  return named ? `[${named.name}="${named.value.replace(/["\\]/g, "\\$&")}"]` : null;
}
// A press or a change on the host's deck while the halves are locked. It is
// passed to half A's own deck with the host's fields as they stand, and that
// deck does what it would have done had it been pressed there. B follows.
// A press is a button's, and a change is a field's: a box that is ticked is
// heard once, as the change it makes.
const DEBUG_REMOTE_FIELDS = "#dbg-turns, [data-dbg-field], [data-dbg-layer], [data-dbg-category]";
function debugRemote(kind, target) {
  const held = target instanceof Element ? target.closest(kind === "pane-press"
    ? "[data-dbg], [data-dbg-goto], [data-dbg-drop], [data-dbg-cond-remove]" : `#dbg-turn, ${DEBUG_REMOTE_FIELDS}`) : null;
  if (!held || held.disabled || !debugSplitDeck() || debugSplit.busy) return;
  const key = debugRemoteKey(held);
  // Which view a page shows is the page's own: each half is told.
  if (key === '[data-dbg="solo-ui"]') {
    for (const letter of ["a", "b"]) debugSplitTell(letter, {civvis: "pane-solo", on: !debugSplitDeck().solo});
    return;
  }
  const fields = [...new Set([held, ...debugBar.querySelectorAll(DEBUG_REMOTE_FIELDS)])]
    .filter(field => field.matches("input, select"))
    .map(field => field.type === "checkbox" ? {key: debugRemoteKey(field), checked: field.checked}
      : {key: debugRemoteKey(field), value: field.value});
  debugSplitTell("a", {civvis: kind, key, fields});
  // A save point's name has been sent with the press that used it.
  if (key === '[data-dbg="point"]') debugBar.querySelector("[data-dbg-field=pointname]").value = "";
}
function drawDebugSplit() {
  // Two screens that are being put away are held as they stand.
  if (!debugSplit || debugSplit.busy) return;
  const host = debugSplit.host;
  host.classList.toggle("dbg-col", debugSplit.dir === "col");
  host.style.setProperty("--dbg-a", `${debugSplit.share}fr`);
  host.style.setProperty("--dbg-b", `${1 - debugSplit.share}fr`);
  for (const letter of ["a", "b"]) {
    const turn = debugSplit.panes[letter].turn;
    host.querySelector(`[data-dbg-name="${letter}"]`).textContent =
      `${letter.toUpperCase()}${turn === null ? "" : ` · ${turn}`}`;
  }
  debugSplitSync();
  const view = host.querySelector('[data-dbg-split="view"]');
  view.classList.toggle("on", debugSplit.view);
  view.title = debugSplit.view
    ? "The two maps move together. Click to let each look where it likes."
    : "Each map looks where it likes. Click to move the two together.";
  host.querySelector('[data-dbg-split="flip"]').textContent = debugSplit.dir === "col" ? "◧" : "⬒";
  for (const button of host.querySelectorAll("[data-dbg-split]")) button.disabled = debugSplit.busy;
  // The two lanes in the bar, and the lock that joins them, are drawn from
  // the same record.
  drawDebugBar();
  host.classList.toggle("dbg-versus-on", debugSplit.versus);
  host.querySelector('[data-dbg-split="versus"]').classList.toggle("on", debugSplit.versus);
  const versus = host.querySelector("[data-dbg-versus]");
  versus.hidden = !debugSplit.versus;
  if (!debugSplit.versus) return;
  for (const letter of ["a", "b"])
    debugOfferAgents(host.querySelector(`[data-dbg-strategy="${letter}"]`), debugSplit.panes[letter].strategy);
  const a = debugSplit.panes.a.numbers || {}, b = debugSplit.panes.b.numbers || {};
  const table = DEBUG_VERSUS_ROWS.map(([key, label, title]) => {
    const lead = side => typeof a[key] === "number" && typeof b[key] === "number" &&
      (side === "a" ? a[key] > b[key] : b[key] > a[key]) ? ' class="lead"' : "";
    return `<span class="dbg-vs-stat${key === debugSplit.chart ? " charted" : ""}" data-dbg-stat="${key}" title="${title}: A, then B. Click to chart it"><i>${label}</i>` +
      `<b${lead("a")}>${debugVersusNumber(a[key])}</b><b${lead("b")}>${debugVersusNumber(b[key])}</b></span>`;
  }).join("");
  const numbers = host.querySelector("[data-dbg-numbers]");
  if (numbers.dataset.drawn !== table) { numbers.dataset.drawn = table; numbers.innerHTML = table; }
  debugVersusChart(host.querySelector("[data-dbg-chart]"));
  const reached = Math.max(debugSplit.panes.a.turn ?? 0, debugSplit.panes.b.turn ?? 0);
  host.querySelector("[data-dbg-until]").placeholder = String(reached + 25);
  const run = host.querySelector('[data-dbg-split="versus-run"]');
  run.textContent = debugSplit.running > 0 ? "⏹" : "▶▶";
  run.classList.toggle("on", debugSplit.running > 0);
  // The same start again, for another pair of Civ AIs.
  const back = host.querySelector('[data-dbg-split="versus-back"]');
  back.title = `Put both halves back on turn ${debugSplit.from}, where they split, to play it again with other Civ AIs`;
  back.disabled = debugSplit.busy || debugSplit.running > 0 || debugSplit.from === null ||
    ["a", "b"].every(letter => debugSplit.panes[letter].turn === debugSplit.from);
}
// How long a split waits for its halves before it is shown as it is.
const DEBUG_SPLIT_WAIT_MS = 6000;
// The two screens are there: both halves have said where they stand and what
// is on their timelines, the one panel has A's deck to draw, each seats the
// Civ AI it was given, the branch has let go of the turns ahead of it, and
// each has painted itself looking where it was told to.
function debugSplitReady() {
  const split = debugSplit;
  if (!split || split.shown) return;
  const there = pane => pane.turn !== null && !!pane.marks && !!pane.numbers && pane.agreed && pane.painted;
  if (there(split.panes.a) && there(split.panes.b) && split.panes.a.deck && !split.branches) debugSplitReveal();
}
// One screen becomes two, in one move: the box takes the shape of a split,
// the map goes to A, which carries it on, and B and the strip come in
// beside it.
function debugSplitReveal() {
  const split = debugSplit;
  if (!split || split.shown) return;
  split.shown = true;
  clearTimeout(split.waiting);
  return debugSwap(() => {
    // Whatever became of the split, the window is let go of.
    debugHold(false);
    debugVeil(false);
    debugBar.classList.remove("dbg-branching");
    if (debugSplit !== split) { drawDebugBar(); return; }
    split.host.classList.remove("dbg-unseen");
    drawDebugSplit();
    // The box has taken its other shape and the halves the room that leaves
    // them. Each fits itself to it now, not a frame from now, so what is
    // shown first is each half as it will stand.
    for (const letter of ["a", "b"]) {
      const frame = split.panes[letter].frame;
      try { void frame.getBoundingClientRect(); frame.contentWindow.dispatchEvent(new Event("resize")); } catch (_) {}
    }
  }, split.look === undefined ? null : debugStage(),
    () => debugSplit === split ? debugStage(split.panes.a.frame) : null).then(() => {
    // Seen: from here each half's map is its own to move.
    if (debugSplit === split) for (const letter of ["a", "b"]) debugSplitTell(letter, {civvis: "pane-seen"});
  });
}
// `seat` is the Civ AI the branch is made for: the one picked when Branch
// asked, or when another was picked on the one timeline. `fresh` is a game
// that opens on its two tracks: it has not been shown on one, and is not, so
// the window waits under a veil where a branch made by hand waits showing the
// one screen it was made from. Nothing was pressed for it, so nothing was
// asked (operator 2026-11-22: "branch b should only ask when branch is
// selected. not just auto ask from the start"): B begins on A's Civ AI, as
// any branch made for none does, and B's picker seats another.
function debugSplitOpen(dir, versus = null, seat = null, fresh = false) {
  if (debugSplit || DEBUG_PANE !== null || !debugTimeline() || debugBusy || autoplaying) return;
  if (!document.getElementById("debugsplit-style")) {
    const style = document.createElement("style");
    style.id = "debugsplit-style";
    style.textContent = DEBUG_SPLIT_STYLE;
    document.head.appendChild(style);
  }
  // The window is held as it stands, with nothing open over it, until both
  // halves are there (`debugSplitReady`). Where the one screen is looking is
  // where the halves will open looking.
  debugAsk = null; debugPopOpen = null;
  drawDebugBar();
  const look = fresh ? undefined : debugLookNow();
  debugHold(true);
  if (fresh) debugVeil(true); else debugBar.classList.add("dbg-branching");
  const nonce = Date.now().toString(36);
  const host = document.createElement("div");
  host.id = "debugsplit";
  host.className = "dbg-unseen";
  host.innerHTML = `
    <iframe data-dbg-frame="a" title="Simulator A"></iframe>
    <div class="dbg-divider" role="toolbar" aria-label="Split simulators" title="Drag to resize; double-click to make the halves equal">
      <div class="dbg-pane-name" data-dbg-name="a"></div>
      <button data-dbg-split="drop-a" title="Drop A: close branch A and keep playing B, with every turn B can walk back to">Drop A</button>
      <span class="dbg-gap"></span>
      <span class="dbg-pair">
        <button data-dbg-split="copy-a" title="A ⇒ B: make B a copy of A as it stands now, turns behind it included">A ⇒ B</button>
        <button data-dbg-split="copy-b" title="A ⇐ B: make A a copy of B as it stands now, turns behind it included">A ⇐ B</button>
      </span>
      <span class="dbg-pair">
        <button data-dbg-split="view">View</button>
        <button data-dbg-split="flip" title="Switch between side by side and top and bottom"></button>
      </span>
      <button data-dbg-split="versus" title="Head to head: seat a Civ AI of its own in each half, play both and set their numbers side by side">VS</button>
      <div class="dbg-versus" data-dbg-versus hidden>
        <div class="dbg-vs-line"><span class="dbg-vs-a">A</span><select data-dbg-strategy="a" title="The Civ AI that plays A's seat when A is stepped or run" aria-label="Civ AI for A"></select></div>
        <div class="dbg-vs-line"><span class="dbg-vs-b">B</span><select data-dbg-strategy="b" title="The Civ AI that plays B's seat when B is stepped or run" aria-label="Civ AI for B"></select></div>
        <div class="dbg-vs-table" data-dbg-numbers></div>
        <canvas class="dbg-vs-chart" data-dbg-chart width="252" height="96"></canvas>
        <div class="dbg-vs-line" title="Play both halves, each with its own Civ AI, until this turn opens or a breakpoint stops one">
          <button data-dbg-split="versus-back">↺</button>
          <input type="text" data-dbg-until inputmode="numeric" maxlength="4" aria-label="Play both to turn">
          <button data-dbg-split="versus-run"></button>
        </div>
      </div>
      <span class="dbg-gap"></span>
      <button data-dbg-split="drop-b" title="Drop B: close branch B and keep playing A, with every turn A can walk back to">Drop B</button>
      <div class="dbg-pane-name" data-dbg-name="b"></div>
    </div>
    <iframe data-dbg-frame="b" title="Simulator B"></iframe>`;
  // Head to head, the numbers of the two side by side, is open unless it was
  // put away (operator 2026-11-22: "Default the stats window open").
  let remembered = true;
  try { remembered = localStorage.getItem("civvis-debug-versus") !== "0"; } catch (_) {}
  // A is the timeline as it stands, and B the branch made on the open turn.
  // When turns are kept ahead of that turn they stay A's, and B is told to
  // begin where it stands (`branches`, until B says it holds none of them).
  const timeline = debugTimeline();
  // `shown` is whether the two screens have taken the one's place, and `look`
  // where each half opens looking (undefined for a game that opens this way,
  // which was looking nowhere).
  debugSplit = {dir, locked: true, settling: true, view: true, share: .5, busy: false, panes: {}, host,
    versus: versus ?? remembered, running: 0, until: null, chart: "score", history: {a: new Map(), b: new Map()},
    from: state ? state.turn : null,
    branches: Math.max(timeline.last ?? state.turn, (timeline.beyond || {}).to ?? 0) > state.turn,
    shown: false, look,
    waiting: setTimeout(debugSplitReveal, DEBUG_SPLIT_WAIT_MS)};
  for (const letter of ["a", "b"]) {
    const frame = host.querySelector(`[data-dbg-frame="${letter}"]`);
    // A half starts on the Civ AI picked here, and is told if it loads on another.
    debugSplit.panes[letter] = {frame, id: `${letter}-${nonce}`, turn: null, first: null, last: null,
      deck: null, told: null, strategy: letter === "b" && seat ? seat : debugStrategy(),
      agreed: false, painted: false};
  }
  debugRememberSeat();
  host.addEventListener("click", event => {
    const stat = event.target.closest("[data-dbg-stat]");
    if (stat && debugSplit) { debugSplit.chart = stat.dataset.dbgStat; drawDebugSplit(); return; }
    const button = event.target.closest("[data-dbg-split]");
    if (!button || button.disabled || !debugSplit) return;
    const action = button.dataset.dbgSplit;
    if (action === "view") debugSplit.view = !debugSplit.view;
    else if (action === "flip") debugSplit.dir = debugSplit.dir === "col" ? "row" : "col";
    else if (action === "copy-a" || action === "copy-b") {
      // A copy is the other half's whole history, and so is its line in the chart.
      const [from, to] = action === "copy-a" ? ["a", "b"] : ["b", "a"];
      debugSplit.history[to] = new Map(debugSplit.history[from]);
      debugSplitTell(to, {civvis: "pane-copy", from: debugSplit.panes[from].id});
    }
    // A branch is dropped by its own name, and the other is kept.
    else if (action === "drop-a") debugSplitClose("b");
    else if (action === "drop-b") debugSplitClose("a");
    else if (action === "versus") {
      debugSplit.versus = !debugSplit.versus;
      try { localStorage.setItem("civvis-debug-versus", debugSplit.versus ? "1" : "0"); } catch (_) {}
    } else if (action === "versus-run") debugVersusRun();
    else if (action === "versus-back") {
      // Each half is told by name, so it makes no difference whether they are locked.
      for (const letter of ["a", "b"])
        if (debugSplit.panes[letter].turn !== debugSplit.from)
          debugSplitTell(letter, {civvis: "pane-follow", turn: debugSplit.from, how: "travel"});
    }
    drawDebugSplit();
  });
  host.addEventListener("change", event => {
    const letter = event.target.dataset.dbgStrategy;
    if (letter) debugSplitSeat(letter, event.target.value);
  });
  // The page's keys must not fire while a turn is being typed in the strip.
  host.addEventListener("keydown", event => {
    if (!event.target.matches("input, select")) return;
    event.stopPropagation();
    if (event.key === "Enter" && event.target.matches("[data-dbg-until]")) { debugVersusRun(); drawDebugSplit(); }
  });
  // The strip between the halves is also the handle that sizes them.
  const divider = host.querySelector(".dbg-divider");
  divider.addEventListener("pointerdown", event => {
    if (event.target.closest("button, select, input, [data-dbg-stat]") || !debugSplit) return;
    // Capture keeps the drag when the pointer runs over a frame; the frames
    // are also made deaf to the pointer for as long as it lasts.
    try { divider.setPointerCapture(event.pointerId); } catch (_) {}
    host.classList.add("dbg-dragging");
  });
  divider.addEventListener("pointermove", event => {
    if (!debugSplit || !host.classList.contains("dbg-dragging")) return;
    const box = host.getBoundingClientRect();
    const along = debugSplit.dir === "col"
      ? (event.clientY - box.top) / box.height : (event.clientX - box.left) / box.width;
    debugSplit.share = Math.max(.2, Math.min(.8, along));
    drawDebugSplit();
  });
  for (const kind of ["pointerup", "pointercancel"])
    divider.addEventListener(kind, () => host.classList.remove("dbg-dragging"));
  divider.addEventListener("dblclick", event => {
    if (event.target.closest("button, select, input, [data-dbg-stat]") || !debugSplit) return;
    debugSplit.share = .5;
    drawDebugSplit();
  });
  document.body.appendChild(host);
  drawDebugSplit();
  for (const letter of ["a", "b"]) {
    const pane = debugSplit.panes[letter];
    pane.frame.src = debugPaneAddress(pane.id);
  }
}
// A half's page: the viewer's own address with the half's name. The viewer is
// the folder this page is in, whichever of the simulator's pages holds the
// split (`/test/doctor-strange` and `/test/debugger` are both the viewer at
// `/test/`), so a half asks nothing of the page that holds it by its address.
// On the published site every half is at the one address `?pane`, its name
// after `#`, which a browser neither sends nor files a cached page under:
// each half loads the copy already held, revalidated with a 304, where
// `?pane=<name>` (a name with the time in it) was a new address each visit,
// downloaded whole for each half (operator 2026-11-26: "civivs seems slow
// switching between civvis and dr strange"). The shim there puts the half's
// name on each request itself. The query stays so the address is never the
// holding page's own, which a browser will not load in a frame of itself.
// A socket server serves the page from this machine and tells the halves
// apart by the name in each request's `Referer` (`src/server/panes.rs`),
// which never carries a fragment, so there the name stays `?pane=<name>`.
function debugPaneAddress(id) {
  const root = new URL("./", location.href).pathname;
  return window.CIVVIS_PUBLISHED_BUILD ? `${root}?pane#${id}` : `${root}?pane=${id}`;
}
// Which Civ AI plays a half's seat is the host's to say: it keeps the choice
// and tells the half, from the strip or from the head of the half's lane.
function debugSplitSeat(letter, name) {
  if (!debugSplit) return;
  debugSplit.panes[letter].strategy = name;
  debugSplitTell(letter, {civvis: "pane-strategy", name});
  drawDebugSplit();
}
// Play halves to a turn, each with its own Civ AI. The turn is where the
// lanes show the run is headed until every half that was run says it is done.
function debugSplitRun(until, runners) {
  debugSplit.until = until;
  debugSplit.running = runners.length;
  for (const letter of runners) debugSplitTell(letter, {civvis: "pane-run", until});
}
// Play both halves to a turn. Locked, A is run and B follows it turn by
// turn; unlocked, each is run by itself. The same button stops them.
function debugVersusRun() {
  if (!debugSplit || debugSplit.busy) return;
  if (debugSplit.running > 0) {
    for (const letter of ["a", "b"]) debugSplitTell(letter, {civvis: "pane-stop"});
    debugSplit.running = 0;
    debugSplit.until = null;
    return;
  }
  const box = debugSplit.host.querySelector("[data-dbg-until]");
  const until = parseInt(box.value || box.placeholder, 10);
  const reached = Math.max(debugSplit.panes.a.turn ?? 0, debugSplit.panes.b.turn ?? 0);
  if (!Number.isFinite(until) || until <= reached) { box.value = ""; box.focus(); return; }
  debugSplitRun(until, debugSplit.locked ? ["a"] : ["a", "b"]);
}
// A lane of the split's timeline, let go of on a turn. Locked, both halves
// go; unlocked, only the half whose lane it is. A turn every one of them has
// played is walked to as it stood, and a turn ahead of any is played to.
// Locked, the half whose lane it is goes of its own accord and the other
// follows it, so two halves that are one world are walked as one.
function debugSplitGoTo(letter, text) {
  const turn = parseInt(text, 10);
  if (!debugSplit || debugSplit.busy || debugSplit.running > 0 || !Number.isFinite(turn)) { drawDebugSplit(); return; }
  const letters = debugSplit.locked ? ["a", "b"] : [letter];
  const played = name => Math.max(debugSplit.panes[name].last ?? 0, debugSplit.panes[name].turn ?? 0);
  if (letters.some(name => turn > played(name))) debugSplitRun(turn, debugSplit.locked ? ["a"] : [letter]);
  else if (debugSplit.panes[letter].turn !== turn)
    debugSplitTell(letter, debugSplit.locked ? {civvis: "pane-go", turn} : {civvis: "pane-follow", turn, how: "travel"});
  drawDebugSplit();
}
// What a half did that left its turn where it was, as the other half is
// asked to do it. A walk that ended on the same turn went to the top of it.
// A save point is one half's own, so the half whose lane it stands on is
// asked to go to it. It goes of its own accord, as from its own panel, so a
// locked other half is brought to the same turn.
function debugSplitGoToSave(letter, id) {
  if (!debugSplit || debugSplit.busy || debugSplit.running > 0 || !Number.isFinite(id)) return;
  debugSplitTell(letter, {civvis: "pane-op", body: {op: "goto_save", id}, lead: true});
}
function debugMirrored(body) {
  return ["back", "forward", "jump", "restart"].includes(body.op) ? {op: "restart"} : body;
}
// The screen is whole again, playing the half that was kept: its session
// takes the host's place in the engine, so nothing it could walk back to is lost.
//
// Two screens become one in one move, as one became two. The two are held as
// they stand, with nothing greyed, while the engine makes the kept half the
// game and this page draws it under them. Then the kept half goes to the map,
// which is looking where that half was, the box takes the shape of the one
// timeline, and the other half and the strip go.
async function debugSplitClose(keep) {
  if (!debugSplit || debugSplit.busy || !debugSplit.shown) return;
  const split = debugSplit, kept = split.panes[keep];
  split.busy = true;
  debugHold(true);
  split.host.classList.add("dbg-closing");
  // Where the kept half is looking, and where in it the middle of its map
  // stands, read before anything is asked of it.
  let look = null;
  try { look = kept.frame.contentWindow.debugLookNow(); } catch (_) {}
  const stage = debugStage(kept.frame);
  let note = "";
  try {
    const next = await fetchJSON("/debug", {method: "POST",
      headers: {"Content-Type": "application/json"},
      body: JSON.stringify({op: "adopt", pane: kept.id})}, 60000);
    if (next.error) throw new Error(next.error);
    debugForgetStandingOrders(next.turn);
    sel = null; selCity = null;
    // The half that is kept keeps its Civ AI: the seat is still that agent's.
    const page = document.getElementById("autoplaystrategy");
    if (page && kept.strategy && [...page.options].some(option => option.value === kept.strategy)) page.value = kept.strategy;
    debugRememberSeat();
    render(next);
    note = `Branch ${keep === "a" ? "B" : "A"} closed · ${keep.toUpperCase()} is the timeline`;
  } catch (error) {
    note = `Branch ${keep.toUpperCase()} could not be kept: ${error.message || error}`;
  }
  await debugSwap(() => {
    split.host.remove();
    debugSplit = null;
    debugHold(false);
    // The box is drawn as the one timeline's, which gives the map its room,
    // and then the map looks where the kept half did.
    debugSay(note);
    debugCameraLook(look);
  }, stage, () => debugStage());
}
window.addEventListener("message", event => {
  if (!debugSplit || event.origin !== location.origin) return;
  const data = event.data;
  if (!data || typeof data.civvis !== "string") return;
  // A page that sets more worlds than two side by side keeps the rest here
  // as well, under the next letters (`strangeWorldOpen`).
  const letter = Object.keys(debugSplit.panes).find(name => debugSplit.panes[name].frame.contentWindow === event.source);
  if (!letter) return;
  const other = letter === "a" ? "b" : "a";
  if (data.civvis === "pane-turn") {
    debugSplit.panes[letter].turn = data.turn;
    // A half that has just loaded is told how the split stands, and the
    // branch that it holds nothing past the turn it was made on.
    if (data.how === "boot") {
      debugSplitSync(letter);
      if (letter === "b" && debugSplit.branches) debugSplitTell("b", {civvis: "pane-op", body: {op: "branch"}});
      // And, while it is unseen, where to look: it says when it has painted that.
      if (!debugSplit.shown) debugSplitTell(letter, {civvis: "pane-look", view: debugSplit.look || null, say: true});
    }
    drawDebugSplit();
    debugSplitReady();
    // A pane that moved because it was following is not followed back. A
    // twin that was played with it has only its state to read.
    if (debugSplit.locked && data.how !== "follow" && data.how !== "boot")
      debugSplitTell(other, {civvis: "pane-follow", turn: data.turn, how: data.twinned ? "twin" : data.how});
  } else if (data.civvis === "pane-did") {
    // What one locked half did without its turn moving, the other does too.
    if (!debugSplit.locked || !data.body) return;
    if (data.twinned) debugSplitTell(other, {civvis: "pane-follow", turn: debugSplit.panes[letter].turn, how: "twin"});
    else debugSplitTell(other, {civvis: "pane-op", body: debugMirrored(data.body)});
  } else if (data.civvis === "pane-marks") {
    debugSplit.panes[letter].marks = {flags: Array.isArray(data.flags) ? data.flags : [],
      stops: Array.isArray(data.stops) ? data.stops : [], beyond: data.beyond || null,
      eras: Array.isArray(data.eras) ? data.eras : [], by: Array.isArray(data.by) ? data.by : [],
      swans: Array.isArray(data.swans) ? data.swans : [], seat: data.seat ?? null,
      turns: Array.isArray(data.turns) ? data.turns : null, worlds: Array.isArray(data.worlds) ? data.worlds : null};
    // The branch has let go of the turns that were ahead of it.
    if (letter === "b" && debugSplit.branches && Array.isArray(data.turns) && !data.beyond &&
        data.turns.every(turn => turn <= debugSplit.from)) debugSplit.branches = false;
    drawDebugBar();
    debugSplitReady();
  } else if (data.civvis === "pane-deck") {
    debugSplit.panes[letter].deck = data.deck;
    debugSplitSettle();
    drawDebugBar();
    debugSplitReady();
  } else if (data.civvis === "pane-shown") {
    debugSplit.panes[letter].painted = true;
    debugSplitReady();
  } else if (data.civvis === "pane-camera") {
    // In Doctor Strange each world frames its own seat's ground as it plays,
    // and only a look the person gave one of them is shared: each world's
    // own framing, every turn, had every other world draw its map again.
    const shared = !STRANGE || (data.view && data.view.own === true);
    if (debugSplit.view && shared)
      for (const name of Object.keys(debugSplit.panes))
        if (name !== letter) debugSplitTell(name, {civvis: "pane-look", view: data.view});
  } else if (data.civvis === "pane-numbers") {
    const pane = debugSplit.panes[letter];
    pane.numbers = data.numbers;
    pane.first = data.first;
    pane.last = data.last;
    // Turns past the last one the half still keeps belong to a history it
    // has since played over.
    const history = debugSplit.history[letter];
    for (const turn of [...history.keys()]) if (turn > data.last) history.delete(turn);
    history.set(data.turn, data.numbers);
    // A half that was told which Civ AI to seat is told again if it lost it:
    // a copy or a reload starts it on the page's own choice.
    if (pane.strategy === undefined) pane.strategy = data.strategy;
    else if (pane.strategy !== data.strategy) debugSplitTell(letter, {civvis: "pane-strategy", name: pane.strategy});
    pane.agreed = pane.strategy === data.strategy;
    drawDebugSplit();
    debugSplitReady();
  } else if (data.civvis === "pane-ready") {
    // A half holding a turn it was given: the host shows it with the other's.
    if (STRANGE && strange.run && data.turn === strange.run.asked) debugSplit.panes[letter].ready = data.turn;
    else debugSplitTell(letter, {civvis: "pane-show"});
    drawDebugSplit();
  } else if (data.civvis === "pane-followed") {
    debugSplit.panes[letter].settled = typeof data.turn === "number" && typeof data.wanted === "number"
      ? {turn: data.turn, wanted: data.wanted} : null;
    drawDebugSplit();
  } else if (data.civvis === "pane-ran") {
    debugSplit.running = Math.max(0, debugSplit.running - 1);
    if (debugSplit.running === 0) debugSplit.until = null;
    drawDebugSplit();
  } else if (data.civvis === "pane-search") {
    // Autosearch pressed on a half's own panel: the search is this page's,
    // which holds the engine, and begins from that half.
    debugSearchShow(true, letter);
  } else if (data.civvis === "pane-seated") {
    // A half playing a line that was found seats each Civ AI of it in turn.
    if (typeof data.name === "string") debugSplit.panes[letter].strategy = data.name;
    drawDebugSplit();
  } else if (data.civvis === "pane-ahead-done") {
    // A world played ahead of its open turn, for the page that asked it to.
    if (STRANGE) strangeAheadDone(letter, data);
  } else if (data.civvis === "pane-over") {
    // A world whose game is over, or is again not (`strangePaneResult`).
    if (STRANGE) strangeOver(letter, data.over);
  } else if (data.civvis === "pane-close") {
    // Close, pressed on the card of a world whose game is over.
    if (STRANGE) strangeClose(letter);
  } else if (data.civvis === "pane-swan-done") {
    // A world a black swan was struck on, for the page that asked for it.
    if (STRANGE) strangeSwanDone(letter, data);
  } else if (data.civvis === "pane-planned") {
    if (STRANGE) strangeSearchPlanned(letter, data.done);
    else if (debugSearch && debugSearch.playing === letter) debugSearchPlayed(debugSearch, data.done);
  }
});

// The left panel's picker is the same choice, made in the single player view.
// (The header's picker writes it and says so itself.)
{
  const picker = document.getElementById("autoplaystrategy");
  if (picker) picker.addEventListener("change", event => { if (event.isTrusted) debugSeat(); });
}

// ---- the pane's side
let debugPaneTurn = null;
let debugFollowWanted = null;
function debugPaneReport() {
  if (DEBUG_PANE === null || !state || SPEC || state.turn === debugPaneTurn) return;
  const first = debugPaneTurn === null;
  debugPaneTurn = state.turn;
  // The command deck is the host's to show; half a window has no room for it.
  if (first) togglePanel(true, false);
  const how = first ? "boot" : debugFollowing || debugMirroring ? "follow" : debugTravelling ? "travel"
    : autoplaying ? "step" : "end";
  window.parent.postMessage({civvis: "pane-turn", pane: DEBUG_PANE, turn: state.turn, how, twinned: debugTwinned},
    location.origin);
}
// What is on this half's deck, for the host to draw the one panel of a
// locked split from. Said when it changes, and only while this half's own
// deck is put away.
let debugDeckSaid = "";
function debugPaneDeck(deck) {
  if (!debugPaneLocked) return;
  const said = JSON.stringify(deck);
  if (said === debugDeckSaid) return;
  debugDeckSaid = said;
  window.parent.postMessage({civvis: "pane-deck", pane: DEBUG_PANE, deck}, location.origin);
}
// What stands on this half's timeline, for the host to draw on the half's
// lane: its save points, its breakpoint turns and the eras it has been
// through. Said when it changes, locked or not, because the host's is the
// only timeline a half has.
let debugMarksSaid = "";
function debugPaneMarks(timeline) {
  const flags = [...(timeline.saves || [])].sort((a, b) => a.turn - b.turn || a.id - b.id);
  const stops = timeline.breakpoints.turns;
  // And the kept turns ahead that another Civ AI played, when there are any.
  const beyond = timeline.beyond || null;
  const eras = timeline.eras || [];
  // The turns this half holds and the engine's print of each one's world:
  // what the host reads where the two halves' timeline branches from
  // (`debugSplitFork`).
  const turns = timeline.turns || [], worlds = timeline.worlds || null;
  // And whose play each stretch of them was, for the host to name there.
  const by = timeline.played_by || [];
  // And the black swans that have struck the line, for the host to mark on it.
  const swans = timeline.swans || [];
  // And the Civ AI whose line those turns are, as the engine names it: the
  // host holds a future played out for that line against them
  // (`strangeSightStale`).
  const seat = timeline.seat ?? null;
  const said = JSON.stringify([flags, stops, beyond, eras, turns, worlds, by, swans, seat]);
  if (said === debugMarksSaid) return;
  debugMarksSaid = said;
  window.parent.postMessage({civvis: "pane-marks", pane: DEBUG_PANE, flags, stops, beyond, eras, turns, worlds, by, swans, seat},
    location.origin);
}
// What the host tells a half to do after the other is done one thing at a
// time, in the order it was told, and not while the half is in the middle of
// something else.
let debugTold = Promise.resolve();
function debugInTurn(work) {
  debugTold = debugTold.then(async () => {
    for (let waited = 0; waited < 1200 && (debugBusy || turnAdvancePending); waited++)
      await new Promise(next => setTimeout(next, 25));
    await work();
  }).catch(error => console.error("debugger split", error));
}
// What this half says about its seat, for the host to set beside the other's.
let debugNumbersSaid = "";
function debugPaneNumbers() {
  if (DEBUG_PANE === null || !state || SPEC) return;
  const seat = state.players.find(player => player.id === 0) || {}, yields = seat.yields || {};
  // And what it has made over the game so far, which Doctor Strange's Civ 6
  // sets beside each yield (`STRANGE_TOTALS`).
  const totals = seat.totals || {};
  const numbers = {score: seat.score, cities: seat.cities, population: seat.population,
    production: yields.production,
    science: yields.science, culture: yields.culture, gold: yields.gold, faith: yields.faith,
    military: seat.military,
    production_total: totals.production, science_total: totals.science, culture_total: totals.culture,
    gold_total: totals.gold, faith_total: totals.faith};
  const last = Math.max((state.debug || {}).last ?? state.turn, state.turn);
  const first = Math.min((state.debug || {}).first ?? state.turn, state.turn);
  const said = JSON.stringify([state.turn, first, last, numbers, debugStrategy() || ""]);
  if (said === debugNumbersSaid) return;
  debugNumbersSaid = said;
  window.parent.postMessage({civvis: "pane-numbers", pane: DEBUG_PANE, numbers, turn: state.turn, first, last,
    strategy: debugStrategy() || ""}, location.origin);
}
// A world of Doctor Strange's whose game is over says so in its own box, in a
// short card, and time runs on for the other worlds (operator 2026-11-26:
// "for doctor strange, when a screen finishes it should not halt the game
// (unless the final screen). we should make a quikc summary screen to
// describe the outcome, with maybe a "view map" and a "close" option"). The
// host is told the turn the game ended on (`pane-over`) and leaves the world
// out of the turns past it (`strangeEnded`). View map puts the card away and
// leaves a chip in the corner to bring it back; Close deletes the world
// (`pane-close`, `strangeClose`), as its ✕ does. A seat that has fallen gets
// the card too, though its world plays on. The viewer's own result card,
// and the countdown to a new game it carries, are not this page's: a new
// game would end the world (`STRANGE_PANE_STYLE`, `finaleHoldsForTheReader`).
let strangeResultSaid = "null", strangeResultViewed = "";
function strangeWorldFate() {
  const me = (state.players || [])[0];
  if (!me) return null;
  const civ = id => ((state.players || [])[id] || {}).civ || "A rival";
  const verdict = victoryVerdict(state.victory_type, state.victory_label);
  if (!gameFinished(state))
    return me.alive === false ? {how: "fell", turn: state.turn, kicker: "Defeat", title: `${me.civ} has been defeated`,
      verdict: "Its world plays on without it"} : null;
  const turn = state.victory_turn ?? state.turn;
  const winners = Array.isArray(state.winners) && state.winners.length ? state.winners
    : state.winner !== null && state.winner !== undefined ? [state.winner] : [];
  if (winners.includes(0)) return {how: "won", turn, kicker: "Victory", title: `${me.civ} won`, verdict};
  if (!winners.length) return {how: "over", turn, kicker: "Game over", title: "No one won",
    verdict: state.draw === true ? "Turn limit reached" : ""};
  return {how: "lost", turn, kicker: "Defeat", title: `${winners.map(civ).join(" + ")} won`, verdict};
}
function strangePaneResult() {
  if (!STRANGE_PANE || !state) return;
  const fate = strangeWorldFate();
  const over = fate && fate.how !== "fell" ? {turn: fate.turn, how: fate.how} : null;
  const said = JSON.stringify(over);
  if (said !== strangeResultSaid) {
    strangeResultSaid = said;
    window.parent.postMessage({civvis: "pane-over", pane: DEBUG_PANE, over}, location.origin);
  }
  strangeResultDraw(fate);
}
function strangeResultDraw(fate) {
  let card = document.getElementById("strange-result"), chip = document.getElementById("strange-result-chip");
  if (!fate) {
    if (card) card.hidden = true;
    if (chip) chip.hidden = true;
    strangeResultViewed = "";
    return;
  }
  if (!card) {
    card = Object.assign(document.createElement("div"), {id: "strange-result", hidden: true});
    card.setAttribute("role", "dialog");
    chip = Object.assign(document.createElement("button"), {id: "strange-result-chip", type: "button", hidden: true});
    document.body.append(card, chip);
    card.addEventListener("click", event => {
      const button = event.target.closest("[data-sr]");
      if (!button || button.disabled) return;
      if (button.dataset.sr === "map") { strangeResultViewed = card.dataset.key; strangeResultDraw(strangeWorldFate()); }
      else window.parent.postMessage({civvis: "pane-close", pane: DEBUG_PANE}, location.origin);
    });
    chip.addEventListener("click", () => { strangeResultViewed = ""; strangeResultDraw(strangeWorldFate()); });
  }
  const key = `${state.seed}|${fate.how}|${fate.turn}`, me = (state.players || [])[0] || {};
  if (card.dataset.key !== key) {
    const count = value => Math.round(+value || 0).toLocaleString();
    card.className = chip.className = `strange-result ${fate.how}`;
    card.setAttribute("aria-label", `${fate.kicker}: ${fate.title}`);
    card.innerHTML = `<span class="sr-kicker">${fate.kicker}</span>` +
      `<strong class="sr-title">${escapeAttr(fate.title)}</strong>` +
      (fate.verdict ? `<span class="sr-verdict">${escapeAttr(fate.verdict)}</span>` : "") +
      `<span class="sr-meta">Turn ${fate.turn} · Score ${count(me.score)} · ${count(me.cities)} cities</span>` +
      `<div class="sr-actions"><button type="button" data-sr="map">View map</button>` +
      `<button type="button" data-sr="close">Close</button></div>`;
    chip.textContent = `${fate.how === "won" ? "★" : "✕"} ${fate.kicker} · turn ${fate.turn}`;
    chip.title = "Show this world's result again";
    card.dataset.key = key;
  }
  // The last world open stays, as it does for its ✕.
  let many = false;
  try { many = window.parent.strangeOpen().length > 1; } catch (_) {}
  const close = card.querySelector('[data-sr="close"]');
  close.disabled = !many;
  close.title = many ? "Delete this world: its line goes from the tree and its box from the window" : "The last world stays open";
  card.hidden = strangeResultViewed === key;
  chip.hidden = !card.hidden;
}
// Reach the turn the other pane is on, the way it got there: walked along
// the timeline, stepped by the Civ AI, or by ending the turn as it stands.
// A host taking two halves through the turns together (`strangeAsk`) has
// each hold the turn it is given, unshown, until the other has its own: then
// both are shown at once. A half is never held for ever.
let debugHolding = false, debugShow = null;
function debugHoldTurn(turn) {
  return new Promise(go => {
    const shown = ahead => { if (debugShow === shown) debugShow = null; go(+ahead || 0); };
    debugShow = shown;
    window.parent.postMessage({civvis: "pane-ready", pane: DEBUG_PANE, turn}, location.origin);
    setTimeout(shown, 120000);
  });
}
// This half's world is played on from the last turn it holds and comes back
// to the open one, in one request (the engine's `ahead`). The page goes on
// showing the turn it showed and now holds more turns ahead of it, so only
// what the debugger holds is new: the answer is asked for lean, and the host
// is told what was seen on the way. `held` is the world the page stands on:
// the one it is about to draw, where this is asked as a held turn is shown.
async function debugPlayAhead(turns, held = state) {
  const said = {civvis: "pane-ahead-done", pane: DEBUG_PANE, readings: [], alerts: [], error: null};
  try {
    if (!held || !held.debug || debugRunning || autoplaying || gameFinished(held)) said.error = "this world cannot be played ahead now";
    else {
      said.from = Math.max(held.debug.last ?? held.turn, held.turn);
      const next = await fetchJSON("/debug", {method: "POST", headers: {"Content-Type": "application/json"},
        body: JSON.stringify({op: "ahead", turns: +turns || 1, strategy: debugStrategy(), lean: true})},
        AUTOPLAY_BATCH_TIMEOUT_MS);
      said.error = next.error || null;
      if (next.debug) {
        // What was played is the world's line wherever the page stands on
        // it. What the debugger holds is this page's only while the page is
        // on the turn the answer came back to.
        said.readings = next.readings || [];
        said.alerts = next.alerts || [];
        said.last = Math.max(next.debug.last ?? next.turn, next.turn);
        if (next.turn === state.turn) {
          state.debug = next.debug;
          drawDebugBar();
          debugPaneNumbers();
        }
      }
    }
  } catch (error) { said.error = String(error.message || error); }
  window.parent.postMessage(said, location.origin);
}
async function debugFollow(turn, how, hold = false) {
  debugFollowWanted = {turn, how};
  if (debugFollowing) return;
  debugFollowing = true;
  debugHolding = hold === true && DEBUG_PANE !== null;
  let aimed = turn;
  try {
    for (let guard = 0; guard < 1000 && debugFollowWanted; guard++) {
      const wanted = debugFollowWanted;
      aimed = wanted.turn;
      if (wanted.how === "twin") {
        // The engine made this half a copy of the other as it answered it.
        await debugRefresh();
        if (debugFollowWanted === wanted) debugFollowWanted = null;
        continue;
      }
      if (!debugTimeline() || state.turn === wanted.turn || gameFinished(state)) {
        if (debugFollowWanted === wanted) debugFollowWanted = null;
        continue;
      }
      const before = state.turn;
      if (state.turn > wanted.turn || (wanted.how === "travel" && state.debug.last >= wanted.turn)) {
        // A walk lands on the nearest marked turn, and that is where it stays.
        await debugOp({op: "jump", turn: wanted.turn});
        if (debugFollowWanted === wanted) debugFollowWanted = null;
      // A turn the other half walked to and this one has no turn of its own
      // for is played to, by this half's Civ AI.
      } else if (wanted.how === "step" || wanted.how === "travel") await debugStep();
      else await send({type: "end_turn"});
      if (state.turn === before && debugFollowWanted === wanted) debugFollowWanted = null;
    }
  } finally {
    debugFollowing = false;
    debugFollowWanted = null;
    debugHolding = false;
    // Where it came to, for a host taking two halves through the turns
    // together to tell a half that is still playing from one that stopped
    // short (`strangeStep`).
    if (DEBUG_PANE !== null)
      window.parent.postMessage({civvis: "pane-followed", pane: DEBUG_PANE, turn: state.turn, wanted: aimed}, location.origin);
  }
}
// Where this half's map is looking, as the few numbers the camera is made of.
// A half says so when it changes; a half told to look somewhere does not say
// it back.
let debugCameraSaid = "";
function debugCameraNow() {
  return {x: cam.x, y: cam.y, scale: cam.scale, rot: cam.rot, tilt: cam.tilt,
    pan: {x: SKY_PAN.x, y: SKY_PAN.y}};
}
// Where this page's map is looking, for a page of another size to look at
// the same ground: the camera, and the place on the map that stands at the
// middle of what shows of it (`focus`). Null while the camera is on its way
// somewhere, which is nowhere to send another page.
function debugLookNow() {
  if (!state || SPEC || cameraFlight || cameraZoom) return null;
  const view = debugCameraNow();
  if (typeof currentMapFocusWorld === "function") view.focus = currentMapFocusWorld();
  // Whether the person has moved the map themselves: a page left to frame
  // the seat's own ground does so again at the next turn, and one that was
  // put somewhere by hand stays there.
  view.own = typeof watchedEmpireAutoFrame !== "undefined" && !watchedEmpireAutoFrame;
  return view;
}
// Two halves are the same size, and one looks where the other does by taking
// its camera whole. A page of another size takes the zoom and the bearing,
// and puts the same place at the middle of its own map (`view.focus`).
function debugCameraLook(view) {
  if (!state || !view || !view.pan) return;
  if (view.own === true && typeof takeCameraControl === "function") takeCameraControl();
  cameraFlight = null; cameraZoom = null;
  clearCameraInertia();
  cam.x = +view.x; cam.y = +view.y; cam.scale = +view.scale; cam.rot = +view.rot; cam.tilt = +view.tilt;
  SKY_PAN = {x: +view.pan.x, y: +view.pan.y};
  if (Array.isArray(view.focus) && typeof reframeCurrentMapFocus === "function") reframeCurrentMapFocus(view.focus);
  else { draw(); drawMini(); }
  debugCameraSaid = JSON.stringify(debugCameraNow());
}
// Where an unseen half was told to look, kept until it is seen.
let debugLookKept = null;
if (DEBUG_PANE !== null) {
  // After the page's own listener, which has fitted the map to the new room.
  window.addEventListener("resize", () => { if (debugLookKept) debugCameraLook(debugLookKept); });
  window.addEventListener("message", async event => {
    if (event.source !== window.parent || event.origin !== location.origin) return;
    const data = event.data;
    if (!data || typeof data.civvis !== "string") return;
    if (data.civvis === "pane-follow") debugInTurn(() => debugFollow(data.turn, data.how, data.hold));
    else if (data.civvis === "pane-show") { if (debugShow) debugShow(data.ahead); }
    else if (data.civvis === "pane-lock") {
      debugPaneLocked = data.on === true;
      debugTwin = typeof data.twin === "string" ? data.twin : null;
      debugDeckSaid = "";
      drawDebugBar();
    } else if (data.civvis === "pane-press" || data.civvis === "pane-change") {
      // The host's deck, pressed or changed while the halves are locked.
      // This half's own deck takes it, with the host's fields as they stood.
      if (!debugBar || typeof data.key !== "string") return;
      for (const field of data.fields || []) {
        const held = field.key ? debugBar.querySelector(field.key) : null;
        if (!held) continue;
        if (field.checked === undefined) held.value = field.value; else held.checked = field.checked;
      }
      const held = debugBar.querySelector(data.key);
      if (!held || held.disabled) return;
      if (data.civvis === "pane-press") held.click();
      else held.dispatchEvent(new Event("change", {bubbles: true}));
    } else if (data.civvis === "pane-op" && data.body) {
      // Asked from the host's own deck, this is this half's to do and to
      // say. Otherwise it is what the other half did, done here too with
      // this half's own Civ AI, and not a move of its own to be followed.
      const body = "strategy" in data.body ? {...data.body, strategy: debugStrategy()} : data.body;
      if (data.lead === true) { debugSay(""); debugOp(body, AUTOPLAY_BATCH_TIMEOUT_MS); }
      else debugInTurn(async () => {
        debugMirroring = true;
        try { await debugOp(body, AUTOPLAY_BATCH_TIMEOUT_MS); } finally { debugMirroring = false; }
      });
    } else if (data.civvis === "pane-solo") setSoloUi(data.on === true);
    else if (data.civvis === "pane-go") { debugSay(""); debugGoToTurn(String(data.turn)); }
    else if (data.civvis === "pane-look") {
      debugCameraLook(data.view);
      // A half that is still unseen says when it has painted what it was
      // told to look at: the host shows the two when both have. Being shown
      // changes the room the half has, so it keeps what it was told and
      // looks there again in its new room, until the host says it is seen.
      if (data.say === true) {
        debugLookKept = data.view || null;
        // Two frames on, it has been painted. A window nobody is looking
        // at paints nothing, and has nothing to wait for.
        const painted = document.hidden ? drawn => setTimeout(drawn, 0) : drawn => requestAnimationFrame(drawn);
        painted(() => painted(() =>
          window.parent.postMessage({civvis: "pane-shown", pane: DEBUG_PANE}, location.origin)));
      }
    } else if (data.civvis === "pane-seen") debugLookKept = null;
    else if (data.civvis === "pane-strategy") {
      const select = document.getElementById("autoplaystrategy");
      if (select && [...select.options].some(option => option.value === data.name)) select.value = data.name;
      debugPaneNumbers();
      // It is this half's own seat, and nothing for the other half to do too.
      debugInTurn(async () => {
        debugMirroring = true;
        try { await debugSeat(); } finally { debugMirroring = false; }
      });
    } else if (data.civvis === "pane-run") {
      // A half that cannot run says so, or the host would wait on it.
      if (debugRunning || autoplaying || !debugTimeline())
        window.parent.postMessage({civvis: "pane-ran", pane: DEBUG_PANE}, location.origin);
      else debugRun(+data.until || null);
    } else if (data.civvis === "pane-stop") {
      if (debugRunning) { debugStopAsked = true; drawDebugBar(); }
    } else if (data.civvis === "pane-plan" && data.plan) {
      // A line Autosearch found, for this half to play (`debugPlayPlan`).
      debugInTurn(async () => {
        const done = await debugPlayPlan(data.plan, typeof data.copy === "string" ? data.copy : null);
        window.parent.postMessage({civvis: "pane-planned", pane: DEBUG_PANE, done}, location.origin);
      });
    }
    else if (data.civvis === "pane-ahead") {
      // Asked while the worlds stand still, it takes its turn among what
      // this half was told to do.
      debugInTurn(() => debugPlayAhead(data.turns, debugTimeline() ? state : null));
    }
    else if (data.civvis === "pane-swan") {
      // A black swan strikes this half's world as it stands (the engine's
      // `black_swan`): the world that comes back is drawn, and the host is
      // told what struck. It is not a move of this half's own for a locked
      // other half to do too: the host strikes each world it means to.
      debugInTurn(async () => {
        const said = {civvis: "pane-swan-done", pane: DEBUG_PANE, swan: null, error: null};
        debugMirroring = true;
        try {
          if (!debugTimeline() || debugRunning || autoplaying || gameFinished(state)) said.error = "this world cannot be struck now";
          else {
            const body = {op: "black_swan", seed: Number.isFinite(+data.seed) ? Math.max(0, Math.floor(+data.seed)) : 0};
            if (typeof data.kind === "string" && data.kind) body.kind = data.kind;
            const next = await debugOp(body, 60000);
            if (!next) said.error = "the engine did not answer";
            else if (next.error) said.error = next.error;
            else said.swan = next.black_swan || null;
            if (!said.swan && !said.error) said.error = "the engine struck nothing";
          }
        } catch (error) { said.error = String(error.message || error); }
        finally { debugMirroring = false; }
        window.parent.postMessage(said, location.origin);
      });
    }
    else if (data.civvis === "pane-copy") {
      // A copy is not a move of this half's own, so the other is not told to follow it.
      debugInTurn(async () => {
        debugMirroring = true;
        try {
          const next = await debugOp({op: "copy", from: data.from}, 60000);
          if (next && !next.error) debugSay("Copied from the other simulator");
        } finally {
          debugMirroring = false;
        }
      });
    }
  });
  setInterval(() => {
    if (!state || SPEC) return;
    const said = JSON.stringify(debugCameraNow());
    if (said === debugCameraSaid) return;
    debugCameraSaid = said;
    window.parent.postMessage({civvis: "pane-camera", pane: DEBUG_PANE, view: JSON.parse(said)}, location.origin);
  }, 120);
}

// -------------------------------------------------------------- autosearch
//
// Operator 2026-11-22: "build out autosearch button in the ai debugger. make
// it a bit bigger than the others. autosearch should allow use to use some
// fraction of the cores of this machine to each run a search up and down the
// timeline. allow a reasonable objective function to be chosen. score or prod
// or city or fpt or gpt".
//
// Operator 2026-11-23, pasting a review of it: "Generalize its objective into
// your four lists, move the search core from JS into Rust, and use the model
// outside the simulation loop, never inside it."
//
// The question a search answers: from the open turn, which Civ AIs should
// hold the seat, and when, to meet four lists so many turns on. What the
// seat Needs and Must Avoid are conditions held to a line, on the last turn,
// always, ever or by a turn; what it Wants and Doesn't Want are numbers with
// a weight. A line is a plan of who holds the seat from which turn.
//
// The climb is the engine's (`src/server/autosearch.rs`, `POST /autosearch`):
// it begins with every Civ AI holding the seat alone, ranks the lines it has
// (every Need met and no Must Avoid broken first, then the fewest broken,
// then the most wanted, as percent over the Civ AI seated alone, then the
// fewest handovers), keeps every line nothing beats on every term (the
// front), and hands out lines around the best and the front, at wide strides
// first and finer ones after, until nothing is left to try.
//
// This page plays them. Every core that is given to it plays on a copy of
// the engine (a fork, `beta/shim.js`), so the page's own game is never
// touched and goes on being played while the search runs. A copy has the
// whole timeline behind it. To play a line it walks back down the timeline
// to the turn where that line parts from the one it played last (`jump`),
// and plays up it from there (`play`), and the engine reads every term of the
// four lists on every turn it plays (`terms`). That walk back is exact: the
// agents kept with a turn sit down again, so a line played from the middle
// of another is the line played straight through
// (`a_schedule_walked_back_to_is_the_schedule_played_straight`). A Must Avoid
// broken for good ends a line on the turn it is broken; nothing is cut on a
// soft number.
//
// A found line is then played on the page, by the page's own controls, from
// the turn the search began on: one timeline plays it itself, and a split
// plays it in the half that is picked. What the seat has when it is done is
// said beside what the search read, so a line that did not play out as found
// is seen at once.

// The numbers the four lists may read: the seat's own, by the names its row
// of the standings gives them (`obs::standing`), and whether it has won the
// game or lost it. A key reaches into the row with a dot. The engine reads
// any key of the omniscient observation too: what a term reads is the
// spectator's view of the whole table, not what the seat can see.
// The operator's five come first, in the operator's order.
const DEBUG_SEARCH_KEYS = [
  ["score", "Score", "SCORE"], ["yields.production", "Production a turn", "PROD"], ["cities", "Cities", "CITY"],
  ["yields.faith", "Faith a turn", "FPT"], ["gold_per_turn", "Gold a turn, net", "GPT"],
  ["yields.science", "Science a turn", "SCI"], ["yields.culture", "Culture a turn", "CUL"],
  ["population", "Population", "POP"], ["yields.food", "Food a turn", "FOOD"],
  ["won", "Won the game", "WON"], ["lost", "Lost: fallen, or beaten", "LOST"],
];
const DEBUG_SEARCH_LISTS = [
  ["need", "Need", "What a line must meet. A line that meets every Need comes before any that does not"],
  ["avoid", "Must avoid", "What a line must keep clear of. One held always, ever or by a turn ends a line on the turn it is broken"],
  ["want", "Want", "More is better: counted as percent over the Civ AI seated, playing alone, times its weight"],
  ["dont_want", "Don't want", "Less is better: counted the same way, the other way round"],
];
const DEBUG_SEARCH_WHENS = [["end", "at the end"], ["always", "always"], ["ever", "ever"], ["by", "by turn"]];
const DEBUG_SEARCH_TESTS = ["<", "<=", ">", ">=", "==", "!="];
const DEBUG_SEARCH_TERMS_MOST = 8;
// The victories a Civ AI can be aimed at (`VictoryTarget` in the engine).
const DEBUG_SEARCH_TARGETS = ["science", "culture", "religious", "diplomatic", "domination", "score"];
// What the panel kept before the lists, by the name of the one number.
const DEBUG_SEARCH_OLD_GOALS = {score: "score", prod: "yields.production", city: "cities", fpt: "yields.faith",
  gpt: "gold_per_turn", sci: "yields.science", cul: "yields.culture"};
// How far ahead a search may be asked to play, and how far it is set to.
const DEBUG_SEARCH_TURNS = [10, 20, 30, 50, 100], DEBUG_SEARCH_TURNS_PICK = 30;
// The shares of this computer's cores on offer, after a single core.
const DEBUG_SEARCH_SHARES = [[.25, "a quarter"], [.5, "half"], [.75, "three quarters"], [1, "all"]];
// The most copies of the engine one page holds (`FORKS_MOST` in beta/shim.js).
const DEBUG_SEARCH_FORKS_MOST = 64;
// A copy plays a few turns a request, so the panel sees it move and a search
// that is stopped is not waited on for long.
const DEBUG_SEARCH_CHUNK = 5;
// The two lines of the chart: the Civ AI seated, alone, and the best line
// found. Slots 1 and 2 of the charts' palette as stepped for a dark surface;
// they were checked against this one for contrast and for colour blindness.
const DEBUG_SEARCH_COLOURS = {alone: "#3987e5", best: "#d95926"};
const DEBUG_SEARCH_KEY = "civvis-debug-search-v2";
// The lists a panel opens with: win, and do not lose, which is what the
// engine searches for when nothing is named (`Objective::win_and_do_not_lose`),
// and the most of one number.
function debugSearchDefaultLists(want = "score") {
  return {need: [{key: "won", test: "==", value: 1, when: "end"}],
    avoid: [{key: "lost", test: "==", value: 1, when: "always"}],
    want: [{key: want, weight: 1}], dont_want: []};
}
// The lists as kept, with whatever does not read as a term left out.
function debugSearchTidyLists(lists) {
  const tidy = {need: [], avoid: [], want: [], dont_want: []};
  const known = key => typeof key === "string" && /^[a-z_][a-z0-9_.]*$/i.test(key);
  for (const [list] of DEBUG_SEARCH_LISTS) {
    for (const term of Array.isArray(lists && lists[list]) ? lists[list].slice(0, DEBUG_SEARCH_TERMS_MOST) : []) {
      if (!term || !known(term.key)) continue;
      if (list === "want" || list === "dont_want") {
        const weight = Number(term.weight);
        tidy[list].push({key: term.key, weight: Number.isFinite(weight) && weight !== 0 ? weight : 1});
      } else {
        const value = Number(term.value), turn = Math.round(Number(term.turn));
        const when = DEBUG_SEARCH_WHENS.some(([held]) => held === term.when) ? term.when : "end";
        tidy[list].push({key: term.key, test: DEBUG_SEARCH_TESTS.includes(term.test) ? term.test : ">=",
          value: Number.isFinite(value) ? value : 0, when, ...(when === "by" ? {turn: turn > 0 ? turn : 1} : {})});
      }
    }
  }
  return tidy;
}
// What the panel is set to, kept for the next visit. A choice kept from
// before the lists is its one number, wanted.
const debugSearchSet = (() => {
  const set = {lists: debugSearchDefaultLists(), turns: DEBUG_SEARCH_TURNS_PICK, share: .5, half: "a", targets: false, genes: ""};
  try {
    const kept = JSON.parse(localStorage.getItem(DEBUG_SEARCH_KEY) || "null")
      || JSON.parse(localStorage.getItem("civvis-debug-search-v1") || "{}") || {};
    if (kept.lists && typeof kept.lists === "object") set.lists = debugSearchTidyLists(kept.lists);
    else if (DEBUG_SEARCH_OLD_GOALS[kept.goal]) set.lists = debugSearchDefaultLists(DEBUG_SEARCH_OLD_GOALS[kept.goal]);
    if (DEBUG_SEARCH_TURNS.includes(kept.turns)) set.turns = kept.turns;
    if (typeof kept.share === "number" && kept.share >= 0 && kept.share <= 1) set.share = kept.share;
    set.targets = kept.targets === true;
    if (typeof kept.genes === "string") set.genes = kept.genes.slice(0, 400);
  } catch (_) {}
  return set;
})();
// Whether the panel is up, and the search that is running or last ran.
let debugSearchShown = false;
let debugSearch = null;

// A plan says which Civ AI holds the seat when: `[[turn, name], ...]`, each
// from its turn until the next one's, the first from the turn the search
// began on.
function debugPlanWho(plan, turn) {
  let who = plan[0][1];
  for (const [from, name] of plan) {
    if (from > turn) break;
    who = name;
  }
  return who;
}
// The stretches a plan is made of, up to the turn it was played to.
function debugPlanStretches(plan, end) {
  return plan.map(([from, name], at) => ({from, name, to: Math.min(end, at + 1 < plan.length ? plan[at + 1][0] : end)}))
    .filter(stretch => stretch.to > stretch.from);
}
// One of the numbers, by its key: its name and its short sign.
function debugSearchKeyed(key) {
  const held = DEBUG_SEARCH_KEYS.find(([known]) => known === key);
  return held ? {key, name: held[1], sign: held[2]} : {key, name: key, sign: key.toUpperCase()};
}
// A number read off a row of the standings, or the state's row for the seat,
// by a key that reaches into it with a dot.
function debugSearchRead(row, key) {
  const value = String(key).split(".").reduce((held, part) => held && typeof held === "object" ? held[part] : undefined, row);
  return typeof value === "number" && Number.isFinite(value) ? value : typeof value === "boolean" ? +value : null;
}
// The four lists as the engine reads them.
function debugSearchObjective(lists = debugSearchSet.lists) {
  const tidy = debugSearchTidyLists(lists);
  return {need: tidy.need.map(term => ({seat: 0, ...term})), avoid: tidy.avoid.map(term => ({seat: 0, ...term})),
    want: tidy.want.map(term => ({seat: 0, ...term})), dont_want: tidy.dont_want.map(term => ({seat: 0, ...term}))};
}
// The Civ AIs a search may hand the seat to: every one the page offers, and,
// when asked for, the one seated aimed at each victory and with each gene
// named switched on (`strategies::build_named`).
function debugSearchLetters(agents, seated) {
  const letters = [...agents];
  if (debugSearchSet.targets) for (const target of DEBUG_SEARCH_TARGETS) letters.push(`${seated}@${target}`);
  for (const tag of debugSearchSet.genes.split(/[\s,]+/).map(held => held.trim()).filter(Boolean)) letters.push(`${seated}+${tag}`);
  return [...new Set(letters)];
}
// A plan's turn count after the turn the search began on.
function debugSearchSpan(search) {
  return Math.max(1, search.end - search.from);
}
// The cores on offer: one, and each share of what this computer has. A small
// device is offered two at most, because each is the engine's memory again.
function debugSearchCores() {
  const all = Math.max(1, Math.min(DEBUG_SEARCH_FORKS_MOST, navigator.hardwareConcurrency || 4));
  const most = DEBUG_SMALL_DEVICE ? Math.min(all, 2) : all;
  const offered = [];
  for (const [share, word] of [[0, ""], ...DEBUG_SEARCH_SHARES]) {
    const count = share === 0 ? 1 : Math.max(1, Math.min(most, Math.round(all * share)));
    if (!offered.some(held => held.count === count)) offered.push({share, count, word});
  }
  const picked = offered.reduce((near, held) =>
    Math.abs(held.share - debugSearchSet.share) < Math.abs(near.share - debugSearchSet.share) ? held : near);
  return {all, offered, picked};
}
// Where a search begins: the world of the one timeline on its open turn, or
// of the half of a split that is picked. Only the page that holds the engine
// searches; a half asks its host.
function debugSearchRoot() {
  const timeline = debugTimeline();
  if (!timeline || DEBUG_PANE !== null) return null;
  // Doctor Strange's Civ 6 searches from the base timeline, the world that
  // is followed, which is the one Branch branches from.
  if (STRANGE && debugSplit) {
    const letter = strange.follow, pane = debugSplit.panes[letter];
    if (!pane || !strangeOpen().includes(letter) || pane.turn === null || pane.turn === undefined) return null;
    return {letter, pane: pane.id, turn: pane.turn, seated: pane.strategy || "", idle: strangeIdle()};
  }
  if (debugSplit) {
    const letter = debugSearchSet.half === "b" ? "b" : "a", pane = debugSplit.panes[letter];
    if (pane.turn === null) return null;
    return {letter, pane: pane.id, turn: pane.turn, seated: pane.strategy || debugStrategy() || "",
      idle: !debugSplit.busy && debugSplit.running === 0 && (!pane.deck || pane.deck.idle !== false)};
  }
  return {letter: null, pane: null, turn: state.turn, seated: debugStrategy() || "",
    idle: !debugBusy && !autoplaying && !turnAdvancePending && !timeline.playing};
}
function debugSearchPost(path, body) {
  return fetchJSON(path, {method: "POST", headers: {"Content-Type": "application/json"}, body: JSON.stringify(body)}, 600000);
}
// One request to one copy of the engine, answered lean. A search that has
// been stopped hears nothing more from its copies.
const DEBUG_SEARCH_OVER = "the search was stopped";
async function debugSearchAsk(search, fork, body) {
  const answer = await debugSearchPost(fork.path, {...body, lean: true});
  if (search !== debugSearch || !search.live) throw new Error(DEBUG_SEARCH_OVER);
  if (!answer) throw new Error("the engine gave no answer");
  return answer;
}
// One request to the climb, which is the page's own engine's.
async function debugSearchClimb(search, body) {
  const answer = await debugSearchPost("/autosearch", body);
  if (search !== debugSearch || !search.live) throw new Error(DEBUG_SEARCH_OVER);
  if (!answer || answer.error) throw new Error(answer && answer.error ? answer.error : "the engine gave no answer");
  return answer;
}
// A number as the panel writes it: whole, or to one place.
function debugSearchNumber(value) {
  return typeof value === "number" && Number.isFinite(value) ? String(Math.round(value * 10) / 10) : "–";
}
// A share over the baseline as the panel writes it.
function debugSearchPercent(value) {
  return typeof value === "number" && Number.isFinite(value) ? `${value >= 0 ? "+" : ""}${debugSearchNumber(value)}%` : "–";
}

async function debugSearchStart() {
  if (debugSearch && (debugSearch.live || debugSearch.playing)) return;
  const root = debugSearchRoot();
  if (!root || !root.idle) return;
  const agents = [...((document.getElementById("autoplaystrategy") || {}).options || [])]
    .map(option => option.value).filter(Boolean).filter(name => !/[@+]/.test(name));
  if (!agents.length) return;
  const seated = agents.includes(root.seated) ? root.seated : agents[0];
  const turns = debugSearchSet.turns, objective = debugSearchObjective();
  const search = debugSearch = {
    live: true, state: "starting", note: "", game: String(state.seed ?? ""), objective, turns, from: root.turn, end: root.turn + turns,
    letter: root.letter, pane: root.pane, seated, letters: debugSearchLetters(agents, seated), cores: debugSearchCores().picked.count,
    forks: [], memory: 0, print: null, report: null, start: null,
    began: performance.now(), ended: null, played: 0, futures: 0, playing: null, said: "", hover: null, pumping: false, again: false,
  };
  drawDebugBar(false);
  try {
    // The climb first: the engine says now whether it can search these lists
    // with these Civ AIs, before any copy of it is made.
    let begun;
    try {
      begun = await debugSearchPost("/autosearch", {op: "begin", objective, from: search.from, end: search.end,
        letters: search.letters, seated, forks: search.cores});
    } catch (error) {
      begun = {error: String(error.message || error)};
    }
    if (search !== debugSearch || !search.live) return;
    if (!begun || begun.error) throw new Error(`The engine could not begin this search: ${(begun && begun.error) || "it gave no answer"}`);
    search.report = begun;
    let made;
    try {
      // In Doctor Strange's Civ 6 each world is played on an engine of its
      // own (`beta/shim.js`), so the copies are made of the base timeline's.
      made = STRANGE && search.pane ? await debugSearchPost("/forks", {count: search.cores, pane: search.pane})
        : await debugSearchPost("/forks", {count: search.cores});
    } catch (error) {
      made = {error: String(error.message || error)};
    }
    if (search !== debugSearch || !search.live) return;
    if (!made || made.error || !(made.forks > 0)) {
      throw new Error("Autosearch plays on copies of the engine, and this build could not make them" +
        ` (${(made && made.error) || "it has no forks"}). It runs in the browser build`);
    }
    search.memory = made.memory_bytes || 0;
    // As many cores as copies were made: the tab may have had room for
    // fewer than were asked for (`tabEnginesMB`).
    search.cores = made.forks;
    const half = root.pane ? `&pane=${encodeURIComponent(root.pane)}` : "";
    search.forks = Array.from({length: made.forks}, (_, at) => ({n: at + 1, path: `/debug?fork=${at + 1}${half}`,
      at: search.from, from: search.from, plan: null, series: [], busy: false, dead: false}));
    drawDebugBar(false);
    const prints = await Promise.all(search.forks.map(fork => debugSearchRootOn(search, fork)));
    if (prints.some(print => print !== prints[0])) throw new Error("the copies of the engine do not hold the same world");
    search.print = prints[0];
    search.state = "running";
    debugSearchPump(search);
  } catch (error) {
    if (search.live) debugSearchEnd(search, "failed", String(error.message || error));
  }
}
// A copy is made ready: nothing stops a future on it but the turn it is
// played to (the page's breakpoints are the page's) and a Must Avoid broken
// for good, a half keeps its agents with the turns it marks from here on
// (`remember` in the engine), it reads the four lists on every turn it plays
// (`objective`, which the engine checks against the world first), and it
// stands on the top of the turn the search begins on. The answer is the
// engine's print of that world.
async function debugSearchRootOn(search, fork) {
  const refused = answer => { if (answer.error) throw new Error(answer.error); return answer; };
  refused(await debugSearchAsk(search, fork, {op: "breakpoints"}));
  if (search.pane) refused(await debugSearchAsk(search, fork, {op: "remember"}));
  const set = await debugSearchAsk(search, fork, {op: "objective", objective: search.objective});
  if (set.error) throw new Error(`The engine will not read these lists: ${set.error}`);
  const there = refused(await debugSearchAsk(search, fork, {op: "jump", turn: search.from}));
  if (there.turn !== search.from) throw new Error(`turn ${search.from} is no longer kept, so nothing can be played from it`);
  if (!there.seat) throw new Error("this engine is older than this page, and does not say how the seat stands");
  fork.at = fork.from = search.from;
  fork.plan = null;
  fork.series = [];
  // Where the first Want stands on the turn the search begins on: the start
  // of every line in the chart.
  const first = search.objective.want[0];
  if (first && search.start === null) search.start = debugSearchRead(there.seat, first.key);
  const marks = there.debug || {};
  return (marks.worlds || [])[(marks.turns || []).indexOf(search.from)] ?? null;
}
// Every copy that is free asks the climb for a line. A copy the climb has
// nothing for waits until another brings a line back; when none is playing
// and the climb has nothing left to try, the search is done.
async function debugSearchPump(search) {
  if (search !== debugSearch || !search.live) return;
  if (search.pumping) { search.again = true; return; }
  search.pumping = true;
  try {
    do {
      search.again = false;
      for (const fork of search.forks) {
        if (fork.busy || fork.dead) continue;
        fork.busy = true;
        let next;
        try {
          next = await debugSearchClimb(search, {op: "next", holding: fork.plan, at: fork.at});
        } catch (error) {
          fork.busy = false;
          throw error;
        }
        if (!next.plan) {
          fork.busy = false;
          if (next.done && !search.forks.some(held => held.busy)) {
            const best = search.report && search.report.best;
            debugSearchEnd(search, best ? "done" : "failed", best ? "" : search.note || "no line was played out");
            return;
          }
          continue;
        }
        debugSearchRun(search, fork, next.plan, typeof next.from === "number" ? next.from : search.from);
      }
    } while (search.again && search.live);
    if (search.live && !search.forks.some(fork => !fork.dead))
      debugSearchEnd(search, "failed", search.note || "every copy of the engine failed");
  } catch (error) {
    if (search.live && error.message !== DEBUG_SEARCH_OVER) debugSearchEnd(search, "failed", String(error.message || error));
  } finally {
    search.pumping = false;
  }
  debugSearchDrawSoon();
}
// One line played on one copy. The copy walks back to the turn the line
// parts from the line it holds, with the line's Civ AI for that turn named,
// so the agent kept with the turn sits down again if it is the same one and
// a new one if not, and plays up the timeline from there a stretch at a time,
// keeping what each turn read of the four lists. A line ends early where the
// game does, or where a Must Avoid is broken for good. What it read goes
// back to the climb.
async function debugSearchRun(search, fork, plan, from) {
  fork.busy = true;
  let refused = null;
  try {
    let turn = Math.max(search.from, from);
    let there = await debugSearchAsk(search, fork, {op: "jump", turn, strategy: debugPlanWho(plan, turn)});
    if (!there.error && there.turn !== turn) {
      // A turn that is no longer kept answers with the nearest before it,
      // which is this line's as well: its own Civ AI is named for it.
      turn = there.turn;
      if (turn < search.from) throw new Error(`turn ${search.from} is no longer kept`);
      there = await debugSearchAsk(search, fork, {op: "jump", turn, strategy: debugPlanWho(plan, turn)});
      if (!there.error && there.turn !== turn) there = {error: `turn ${turn} is no longer kept`};
    }
    if (there.error) refused = there.error;
    else {
      fork.plan = plan;
      fork.at = fork.from = turn;
      fork.series.length = turn - search.from;
      while (fork.at < search.end) {
        let stop = search.end;
        for (const [start] of plan) if (start > fork.at) { stop = Math.min(stop, start); break; }
        const asked = Math.min(DEBUG_SEARCH_CHUNK, stop - fork.at);
        const answer = await debugSearchAsk(search, fork, {op: "play", turns: asked, strategy: debugPlanWho(plan, fork.at)});
        const terms = Array.isArray(answer.terms) ? answer.terms : [];
        if (!terms.length && answer.error && !/no turn left/.test(answer.error)) { refused = answer.error; break; }
        fork.series.push(...terms);
        fork.at += terms.length;
        search.played += terms.length;
        // The game ended on the way, or a Must Avoid ended the line: it is read there.
        if (answer.finished || answer.killed || terms.length < asked) break;
        debugSearchDrawSoon();
      }
    }
    if (refused === null) {
      search.report = await debugSearchClimb(search, {op: "took", plan, series: fork.series, last: fork.at});
      search.futures += 1;
    } else {
      // A Civ AI the engine would not seat, or a turn it does not hold: the
      // line goes back once, and the copy plays others.
      search.note = refused;
      fork.plan = null;
      search.report = await debugSearchClimb(search, {op: "failed", plan});
    }
  } catch (error) {
    if (search !== debugSearch || !search.live) return;
    // A copy that stopped answering is left out, and its line goes back to
    // the climb, which tries it once more on another.
    fork.dead = true;
    fork.plan = null;
    search.note = String(error.message || error);
    try { search.report = await debugSearchClimb(search, {op: "failed", plan}); } catch (_) {}
  } finally {
    fork.busy = false;
  }
  debugSearchPump(search);
}
// The search is over, however it ended. Its copies are let go at once: each
// is the engine's memory over again, and so is the climb. What it found
// stays to be played.
function debugSearchEnd(search, how, note = "") {
  if (!search.live) return;
  search.live = false;
  search.state = how;
  search.note = note;
  search.ended = performance.now();
  debugSearchPost("/forks", {count: 0}).catch(() => {});
  debugSearchPost("/autosearch", {op: "end"}).catch(() => {});
  drawDebugBar(false);
}
let debugSearchDrawing = false;
function debugSearchDrawSoon() {
  if (debugSearchDrawing) return;
  debugSearchDrawing = true;
  setTimeout(() => {
    debugSearchDrawing = false;
    // A window held while one screen and two change places is left as it stands.
    if (debugBar && !debugBar.hidden && !debugHeld) debugDrawSearch(debugBar);
  }, 120);
}
function debugSearchShow(on, letter = null) {
  debugSearchShown = on;
  if (on) debugPopOpen = null;
  if (letter && !(debugSearch && debugSearch.live)) debugSearchSet.half = letter;
  drawDebugBar();
}
function debugSearchKeep() {
  try {
    localStorage.setItem(DEBUG_SEARCH_KEY, JSON.stringify({lists: debugSearchSet.lists, turns: debugSearchSet.turns,
      share: debugSearchSet.share, targets: debugSearchSet.targets, genes: debugSearchSet.genes}));
  } catch (_) {}
}
// A change to one term of the four lists, read off the row it was made in.
function debugSearchTermChanged(field) {
  const row = field.closest("[data-term-at]"), list = row && row.closest("[data-list]");
  if (!row || !list) return false;
  const lists = debugSearchTidyLists(debugSearchSet.lists), terms = lists[list.dataset.list];
  const term = terms && terms[+row.dataset.termAt];
  if (!term) return false;
  const part = field.dataset.term, number = Number(field.value);
  if (part === "key") term.key = field.value;
  else if (part === "test") term.test = field.value;
  else if (part === "when") { term.when = field.value; if (term.when === "by" && !term.turn) term.turn = (debugSearchRoot() || {turn: 1}).turn + debugSearchSet.turns; }
  else if ((part === "value" || part === "weight" || part === "turn") && field.value.trim() !== "" && Number.isFinite(number)) term[part] = number;
  else return false;
  debugSearchSet.lists = debugSearchTidyLists(lists);
  return true;
}
// The button and the panel are the box's own, and hear for themselves: a
// press on them is not the deck's, which in a locked split belongs to the
// halves.
function debugSearchWire(bar) {
  const panel = bar.querySelector("#dbg-search"), open = bar.querySelector('[data-dbg-search="open"]');
  open.addEventListener("click", event => {
    event.stopPropagation();
    // A half has no engine of its own to copy: its host searches.
    if (DEBUG_PANE !== null) window.parent.postMessage({civvis: "pane-search", pane: DEBUG_PANE}, location.origin);
    else debugSearchShow(!debugSearchShown);
  });
  panel.addEventListener("click", event => {
    event.stopPropagation();
    const button = event.target.closest("[data-dbg-search]");
    if (!button || button.disabled) return;
    const action = button.dataset.dbgSearch;
    if (action === "close") debugSearchShow(false);
    else if (action === "go") {
      if (debugSearch && debugSearch.live) debugSearchEnd(debugSearch, "stopped");
      else debugSearchStart();
    } else if (action === "play") debugSearchPlay(button.value);
    else if (action === "branch" && STRANGE) strangeSearchBranch();
    else if (action === "add" || action === "remove") {
      const lists = debugSearchTidyLists(debugSearchSet.lists), list = button.dataset.list || button.value;
      if (!lists[list]) return;
      if (action === "remove") lists[list].splice(+button.dataset.at, 1);
      else if (lists[list].length < DEBUG_SEARCH_TERMS_MOST)
        lists[list].push(list === "want" || list === "dont_want" ? {key: "score", weight: 1} : {key: "cities", test: ">=", value: 1, when: "end"});
      debugSearchSet.lists = debugSearchTidyLists(lists);
      debugSearchKeep();
      drawDebugBar(false);
    }
  });
  panel.addEventListener("change", event => {
    event.stopPropagation();
    const target = event.target;
    if (target.dataset.term) { if (!debugSearchTermChanged(target)) return; }
    else if (target.id === "dbg-search-turns") debugSearchSet.turns = +target.value || DEBUG_SEARCH_TURNS_PICK;
    else if (target.id === "dbg-search-cores") debugSearchSet.share = +target.value || 0;
    else if (target.id === "dbg-search-half") debugSearchSet.half = target.value === "b" ? "b" : "a";
    else if (target.id === "dbg-search-targets") debugSearchSet.targets = target.checked;
    else if (target.id === "dbg-search-genes") debugSearchSet.genes = target.value.slice(0, 400);
    else return;
    if (target.tagName === "SELECT") target.blur();
    debugSearchKeep();
    drawDebugBar(false);
  });
  panel.addEventListener("keydown", event => {
    event.stopPropagation();
    if (event.key === "Escape") { debugSearchShow(false); return; }
    // The arrows walk the chart's line under the keys, as the pointer does.
    const chart = event.target instanceof Element ? event.target.closest("#dbg-search-chart svg") : null;
    if (!chart || !debugSearch || !(event.key === "ArrowLeft" || event.key === "ArrowRight")) return;
    event.preventDefault();
    const plot = debugSearchPlot(debugSearch);
    const at = debugSearch.hover ?? plot.last;
    debugSearch.hover = Math.max(plot.first, Math.min(plot.last, at + (event.key === "ArrowRight" ? 1 : -1)));
    debugSearchTip();
  });
  // The chart says the numbers of the turn under the pointer.
  panel.addEventListener("pointermove", event => {
    const chart = event.target instanceof Element ? event.target.closest("#dbg-search-chart svg") : null;
    if (!debugSearch) return;
    const before = debugSearch.hover;
    if (chart) {
      const box = chart.getBoundingClientRect(), plot = debugSearchPlot(debugSearch);
      const along = ((event.clientX - box.left) / box.width * plot.width - plot.left) / (plot.width - plot.left - plot.right);
      debugSearch.hover = Math.round(plot.first + Math.max(0, Math.min(1, along)) * (plot.last - plot.first));
    } else debugSearch.hover = null;
    if (debugSearch.hover !== before) debugSearchTip();
  });
  panel.addEventListener("pointerleave", () => {
    if (debugSearch && debugSearch.hover !== null) { debugSearch.hover = null; debugSearchTip(); }
  });
  panel.addEventListener("focusin", event => {
    if (debugSearch && event.target instanceof Element && event.target.closest("#dbg-search-chart svg") && debugSearch.hover === null) {
      debugSearch.hover = debugSearchPlot(debugSearch).last;
      debugSearchTip();
    }
  });
}
// A Civ AI by a name short enough for a row of the panel: the roster's
// name, and a victory it is aimed at or a gene switched on after it.
function debugSearchAgent(name) {
  const [head, ...genes] = String(name).split("+");
  const [base, target] = head.split("@");
  return [debugAgentName(base), target ? `@${titleCase(target)}` : "", ...genes.map(gene => `+${gene}`)].filter(Boolean).join(" ");
}
// The first Want over the turns, for a line the report carries: from the
// turn the search began on, where every line stands alike.
function debugSearchSeries(search, line) {
  const points = Array.isArray(line && line.chart) ? line.chart.filter(point => typeof point[1] === "number") : [];
  return search.start === null ? points : [[search.from, search.start], ...points];
}
// The chart's measures: the turns it spans and the room around its plot.
function debugSearchPlot(search) {
  const report = search.report || {}, lasts = [report.best, report.baseline].filter(Boolean).map(line => line.last);
  return {width: 372, height: 118, left: 10, right: 46, top: 12, bottom: 20, first: search.from,
    last: Math.max(search.from + 1, ...lasts)};
}
// The first Want over the turns: the Civ AI seated, alone, and the best line
// found, a line each, with the value each ends on written at its end.
function debugSearchChart(search) {
  const report = search.report || {}, want = search.objective.want[0];
  if (!want) return "";
  const plot = debugSearchPlot(search);
  const lines = [];
  if (report.baseline) lines.push(["alone", report.baseline]);
  if (report.best && (!report.baseline || report.best.key !== report.baseline.key)) lines.push(["best", report.best]);
  const points = lines.map(([name, line]) => [name, debugSearchSeries(search, line)]).filter(([, held]) => held.length > 0);
  const values = points.flatMap(([, held]) => held.map(point => point[1]));
  if (!values.length) return "";
  const low = Math.min(...values), high = Math.max(...values), span = high - low || 1;
  const x = turn => plot.left + (turn - plot.first) / (plot.last - plot.first) * (plot.width - plot.left - plot.right);
  const y = value => plot.height - plot.bottom - (value - low) / span * (plot.height - plot.top - plot.bottom);
  const fixed = number => number.toFixed(1);
  let svg = `<svg viewBox="0 0 ${plot.width} ${plot.height}" role="img" tabindex="0" aria-label="${escapeAttr(debugSearchKeyed(want.key).name)} from turn ${plot.first} to turn ${plot.last}">`;
  // Recessive: a hairline at the foot and one at the top of the range.
  for (const value of [low, high])
    svg += `<line x1="${plot.left}" x2="${plot.width - plot.right}" y1="${fixed(y(value))}" y2="${fixed(y(value))}" stroke="#22332e" stroke-width="1"/>`;
  svg += `<text x="${plot.left}" y="${plot.height - 6}">turn ${plot.first}</text>` +
    `<text x="${plot.width - plot.right}" y="${plot.height - 6}" text-anchor="end">turn ${plot.last}</text>`;
  // Where each line ends, its value: two that would overlap are set apart.
  const ends = points.map(([name, held]) => ({name, turn: held[held.length - 1][0], value: held[held.length - 1][1]}));
  const labels = ends.map(end => ({...end, at: y(end.value) + 3.5}));
  if (labels.length === 2 && Math.abs(labels[0].at - labels[1].at) < 12) {
    const [upper, lower] = labels[0].at <= labels[1].at ? labels : [labels[1], labels[0]];
    const middle = (upper.at + lower.at) / 2;
    upper.at = middle - 6;
    lower.at = middle + 6;
  }
  for (const [name, held] of points) {
    const colour = DEBUG_SEARCH_COLOURS[name];
    svg += `<path d="${held.map(([turn, value], at) => `${at ? "L" : "M"}${fixed(x(turn))} ${fixed(y(value))}`).join("")}" fill="none" stroke="${colour}" stroke-width="2" stroke-linejoin="round" stroke-linecap="round"/>`;
  }
  for (const end of ends)
    svg += `<circle cx="${fixed(x(end.turn))}" cy="${fixed(y(end.value))}" r="4" fill="${DEBUG_SEARCH_COLOURS[end.name]}" stroke="#060d0b" stroke-width="2"/>`;
  for (const label of labels)
    svg += `<text class="dbg-value" x="${fixed(x(label.turn) + 8)}" y="${fixed(label.at)}">${debugSearchNumber(label.value)}</text>`;
  svg += `<line class="dbg-cross" x1="0" x2="0" y1="${plot.top - 4}" y2="${plot.height - plot.bottom}" stroke="#9fb3ab" stroke-width="1" visibility="hidden"/></svg>`;
  return svg;
}
// The turn under the pointer on the chart: a hairline on it, and beside it
// what each line reads there. A name is written as text, never as markup.
function debugSearchTip() {
  const search = debugSearch, box = document.getElementById("dbg-search-chart");
  if (!search || !box) return;
  const cross = box.querySelector(".dbg-cross");
  let tip = box.querySelector(".dbg-search-tip");
  if (search.hover === null || !cross) {
    if (cross) cross.setAttribute("visibility", "hidden");
    if (tip) tip.hidden = true;
    return;
  }
  const plot = debugSearchPlot(search), turn = search.hover, report = search.report || {};
  const along = plot.left + (turn - plot.first) / (plot.last - plot.first) * (plot.width - plot.left - plot.right);
  cross.setAttribute("x1", along.toFixed(1));
  cross.setAttribute("x2", along.toFixed(1));
  cross.setAttribute("visibility", "visible");
  if (!tip) {
    tip = document.createElement("div");
    tip.className = "dbg-search-tip";
    box.appendChild(tip);
  }
  tip.hidden = false;
  tip.textContent = "";
  const head = document.createElement("div");
  head.textContent = `Turn ${turn}`;
  tip.appendChild(head);
  const rows = [];
  if (report.best && (!report.baseline || report.best.key !== report.baseline.key)) rows.push(["best", "Best line", report.best]);
  if (report.baseline) rows.push(["alone", `${debugSearchAgent(search.seated)} alone`, report.baseline]);
  for (const [name, label, line] of rows) {
    const point = debugSearchSeries(search, line).find(held => held[0] === turn);
    if (!point) continue;
    const row = document.createElement("div"), key = document.createElement("i"), number = document.createElement("b");
    key.style.borderColor = DEBUG_SEARCH_COLOURS[name];
    number.textContent = debugSearchNumber(point[1]);
    row.append(key, number, document.createTextNode(label));
    tip.appendChild(row);
  }
  // Beside the hairline, on whichever side has the room.
  const share = along / plot.width;
  tip.style.left = share < .55 ? `calc(${(share * 100).toFixed(1)}% + 8px)` : "auto";
  tip.style.right = share < .55 ? "auto" : `calc(${((1 - share) * 100).toFixed(1)}% + 8px)`;
}
// One term of the four lists as a row of the panel: what it reads, and how
// it is held, or how much it counts.
function debugSearchTermRow(list, term, at, locked) {
  const off = locked ? " disabled" : "";
  const keys = [...DEBUG_SEARCH_KEYS.map(([key, name]) => [key, name]),
    ...(DEBUG_SEARCH_KEYS.some(([key]) => key === term.key) ? [] : [[term.key, term.key]])];
  const pick = (part, options, picked, label) => `<select data-term="${part}" aria-label="${escapeAttr(label)}"${off}>` +
    options.map(([value, name]) => `<option value="${escapeAttr(String(value))}"${String(value) === String(picked) ? " selected" : ""}>${escapeAttr(name)}</option>`).join("") +
    "</select>";
  const number = (part, value, label, width) => `<input type="text" inputmode="decimal" data-term="${part}" value="${escapeAttr(String(value))}" aria-label="${escapeAttr(label)}" style="width:${width}px"${off}>`;
  let row = pick("key", keys, term.key, "What it reads");
  if (list === "want" || list === "dont_want") row += `<span class="dbg-search-times">×</span>${number("weight", term.weight, "Weight", 40)}`;
  else {
    row += pick("test", DEBUG_SEARCH_TESTS.map(test => [test, test]), term.test, "How it is held") +
      number("value", term.value, "Value", 44) +
      pick("when", DEBUG_SEARCH_WHENS, term.when, "When");
    if (term.when === "by") row += number("turn", term.turn, "Turn", 44);
  }
  row += `<button data-dbg-search="remove" data-list="${list}" data-at="${at}" title="Take this out of the list" aria-label="Remove"${off}>✕</button>`;
  return `<div class="dbg-search-term" data-term-at="${at}">${row}</div>`;
}
// The four lists, as the panel is set to them, or as a running search was.
function debugSearchListsHtml(lists, locked) {
  return DEBUG_SEARCH_LISTS.map(([list, name, about]) => {
    const terms = lists[list] || [];
    const add = locked || terms.length >= DEBUG_SEARCH_TERMS_MOST ? ""
      : `<button class="dbg-search-add" data-dbg-search="add" value="${list}" title="Add to ${escapeAttr(name)}">+ ${escapeAttr(name)}</button>`;
    return `<div class="dbg-search-list" data-list="${list}"><span title="${escapeAttr(about)}">${escapeAttr(name)}</span>` +
      `<div>${terms.map((term, at) => debugSearchTermRow(list, term, at, locked)).join("")}${terms.length ? "" : `<i>none</i>`}${add}</div></div>`;
  }).join("");
}
// What a line came to, said in a sentence: the lists it met and broke, and
// the Wants against the Civ AI seated alone.
function debugSearchVerdict(search, line) {
  const objective = search.objective, verdict = line.verdict || {};
  const words = [];
  if (objective.need.length) words.push(`meets ${(verdict.need || []).filter(Boolean).length} of ${objective.need.length} Need${objective.need.length === 1 ? "" : "s"}`);
  if (objective.avoid.length) {
    const broken = (verdict.avoid || []).filter(kept => !kept).length;
    words.push(broken ? `breaks ${broken} of ${objective.avoid.length} Must Avoid${objective.avoid.length === 1 ? "" : "s"}` : "breaks no Must Avoid");
  }
  if (typeof verdict.killed === "number") words.push(`ended on turn ${verdict.killed}`);
  return words.join(", ");
}
// The Autosearch button, and its panel when it is up: what it is set to,
// what the search is doing, and what it has found.
function debugDrawSearch(bar) {
  const panel = document.getElementById("dbg-search"), open = bar.querySelector('[data-dbg-search="open"]');
  if (!panel || !open) return;
  // A search is one game's: a new game lets go of it and of what it found.
  if (debugSearch && state && debugSearch.game !== String(state.seed ?? "")) {
    const old = debugSearch;
    debugSearch = null;
    if (old.live) {
      old.live = false;
      debugSearchPost("/forks", {count: 0}).catch(() => {});
      debugSearchPost("/autosearch", {op: "end"}).catch(() => {});
    }
  }
  const search = debugSearch, live = !!(search && search.live);
  // A line being played in a half that is gone will not be answered for.
  if (search && search.playing && search.playing !== "here" && !debugSplit) search.playing = null;
  // One panel under the deck at a time.
  if (debugPopOpen) debugSearchShown = false;
  const shown = debugSearchShown && DEBUG_PANE === null;
  open.classList.toggle("on", shown || live);
  open.classList.toggle("dbg-live", live);
  open.setAttribute("aria-expanded", shown ? "true" : "false");
  const count = live ? `${search.futures}` : "";
  const counted = document.getElementById("dbg-search-count");
  if (counted.textContent !== count) counted.textContent = count;
  // Doctor Strange's Civ 6 has its own button, on its deck, and the bar's is unseen.
  const mine = STRANGE && strange.hud ? strange.hud.querySelector('[data-strange="search"]') : null;
  if (mine) {
    mine.classList.toggle("on", shown || live);
    mine.setAttribute("aria-expanded", shown ? "true" : "false");
    const lines = mine.querySelector("[data-strange-search-count]"), words = live ? `· ${search.futures}` : "";
    if (lines.textContent !== words) lines.textContent = words;
  }
  panel.hidden = !shown;
  if (!shown) return;
  // A window like the box's other panels: where its title was dragged to,
  // or under its own button. With the deck put away (a split whose halves
  // are unlocked) it has no button to stand under, and opens under the box.
  // Doctor Strange's opens beside the card its button is on, over the tree.
  if (mine) strangeSearchPlace(panel, bar);
  else if (debugWindowPlace[debugWindowKey(panel)] || open.getBoundingClientRect().width > 0) debugPlaceWindow(panel, bar, open);
  else { panel.style.left = "8px"; panel.style.top = ""; }
  // As tall as the window has room for under it, and what does not fit scrolls.
  panel.style.maxHeight = `${Math.max(160, window.innerHeight - panel.getBoundingClientRect().top - 8)}px`;
  const root = debugSearchRoot();
  const text = (id, words) => {
    const part = document.getElementById(id);
    if (part.textContent !== words) part.textContent = words;
  };
  const locked = live || !!(search && search.playing);
  const offer = (id, options, picked) => {
    const select = document.getElementById(id), drawn = JSON.stringify(options);
    if (select.dataset.drawn !== drawn) {
      select.dataset.drawn = drawn;
      select.innerHTML = options.map(([value, label]) =>
        `<option value="${escapeAttr(String(value))}">${escapeAttr(label)}</option>`).join("");
    }
    if (document.activeElement !== select) select.value = String(picked);
    select.disabled = locked;
  };
  // What the panel is set to. While a search runs it shows that search's.
  const from = live ? search.from : root ? root.turn : null;
  const turns = live ? search.turns : debugSearchSet.turns;
  const cores = debugSearchCores();
  // The four lists: drawn again only when they change, so a field being
  // typed in is not taken away from under the keys.
  const listed = document.getElementById("dbg-search-lists");
  const lists = live ? debugSearchTidyLists(search.objective) : debugSearchSet.lists;
  const drawnLists = JSON.stringify([lists, locked]);
  if (listed.dataset.drawn !== drawnLists && !listed.contains(document.activeElement)) {
    listed.dataset.drawn = drawnLists;
    listed.innerHTML = debugSearchListsHtml(lists, locked);
  }
  offer("dbg-search-turns", DEBUG_SEARCH_TURNS.map(ahead =>
    [ahead, from === null ? `${ahead} turns on` : `turn ${from + ahead} · ${ahead} turns on`]), turns);
  offer("dbg-search-cores", cores.offered.map(held =>
    [held.share, held.count === 1 ? `1 of ${cores.all} cores` : `${held.count} of ${cores.all} cores · ${held.word}`]),
    live ? (cores.offered.find(held => held.count === search.cores) || cores.picked).share : cores.picked.share);
  const halves = debugSplit ? ["a", "b"].map(letter => {
    const pane = debugSplit.panes[letter];
    return [letter, `${letter.toUpperCase()}${pane.turn === null ? "" : ` · turn ${pane.turn}`}${pane.strategy ? ` · ${debugAgentName(pane.strategy)}` : ""}`];
  }) : [];
  offer("dbg-search-half", halves, live && search.letter ? search.letter : debugSearchSet.half);
  // Doctor Strange's searches from the base timeline, which is picked on its deck.
  document.getElementById("dbg-search-half").hidden = document.getElementById("dbg-search-half-word").hidden = !debugSplit || STRANGE;
  const targets = document.getElementById("dbg-search-targets"), genes = document.getElementById("dbg-search-genes");
  targets.checked = debugSearchSet.targets;
  targets.disabled = genes.disabled = locked;
  if (document.activeElement !== genes && genes.value !== debugSearchSet.genes) genes.value = debugSearchSet.genes;
  const letter = live ? search.letter : root ? root.letter : null;
  // What the seat did by hand in the open turn is not part of a line: every
  // line is played from the top of the turn.
  const acted = !debugSplit && (debugTimeline().acted || 0) > 0;
  text("dbg-search-about", from === null ? "The branch has no turn open yet."
    : STRANGE ? `From the timeline followed, ${letter.toUpperCase()}, on turn ${from}: which player strategies should hold the seat, and when, ` +
      `to meet these lists by turn ${from + turns}. Each core plays paths on a copy of ${letter.toUpperCase()}, so the worlds are left as they are, ` +
      "and the best path found can be made a branch."
    : `From ${acted ? "the top of " : ""}turn ${from}${letter ? ` of ${letter.toUpperCase()}` : ""}: which Civ AIs should hold the seat, and when, ` +
      `to meet these lists by turn ${from + turns}. This game is left as it is while the cores search.`);
  const go = panel.querySelector('[data-dbg-search="go"]');
  const goWords = live ? "⏹ Stop" : search && search.report && search.report.best ? "Search again" : "Search";
  if (go.textContent !== goWords) go.textContent = goWords;
  go.classList.toggle("on", live);
  go.disabled = !live && (!root || !root.idle || !!(search && search.playing));
  go.title = live ? "Stop the search. The best line found so far stays to be played"
    : !root || !root.idle ? "The game is being played: a search begins from a turn that stands still"
    : `Search from turn ${root.turn}`;
  // What the search is doing.
  let status = "";
  if (search) {
    const seconds = Math.max(.001, ((search.ended ?? performance.now()) - search.began) / 1000);
    const work = `${search.futures} line${search.futures === 1 ? "" : "s"} · ${search.played} turns`;
    if (search.state === "starting") status = `Copying the engine to ${search.cores} core${search.cores === 1 ? "" : "s"}…`;
    else if (search.state === "running") status = `${work} · ${Math.round(search.played / seconds)} turns a second`;
    else if (search.state === "done") status = `Done in ${debugSearchNumber(seconds)} s: ${work}. Nothing else it tried was better`;
    else if (search.state === "stopped") status = `Stopped after ${debugSearchNumber(seconds)} s: ${work}`;
    else status = `The search could not go on: ${search.note}`;
  }
  text("dbg-search-status", status);
  const run = document.getElementById("dbg-search-run");
  run.hidden = !(live && search.forks.length > 0);
  if (!run.hidden) {
    const span = debugSearchSpan(search);
    const heads = search.forks.map(fork => {
      const left = (fork.from - search.from) / span * 100, width = Math.max(0, fork.at - fork.from) / span * 100;
      return `<i class="${fork.busy ? "busy" : ""}"><b style="left:${left.toFixed(1)}%;width:${Math.max(1.5, width).toFixed(1)}%"></b></i>`;
    }).join("");
    debugDrawn(document.getElementById("dbg-search-heads"), heads);
    text("dbg-search-from", `turn ${search.from}`);
    text("dbg-search-to", `turn ${search.end}`);
    const each = search.memory ? ` · ${Math.round(search.memory / 1048576)} MB each` : "";
    text("dbg-search-size", `${search.forks.length} cop${search.forks.length === 1 ? "y" : "ies"} of the engine${each}`);
  }
  // What it has found.
  const report = search ? search.report || {} : {};
  const found = document.getElementById("dbg-search-found"), best = report.best || null;
  found.hidden = !best;
  if (!best) return;
  const alone = report.baseline || null, seated = escapeAttr(debugSearchAgent(search.seated));
  const first = search.objective.want[0], keyed = first ? debugSearchKeyed(first.key) : null;
  const value = line => line && line.verdict && line.verdict.want ? line.verdict.want[0] : null;
  let said = `${live ? "Best so far" : "Best found"}`;
  const verdict = debugSearchVerdict(search, best);
  if (verdict) said += `: it ${escapeAttr(verdict)}`;
  if (keyed) said += `${verdict ? ";" : ":"} <b>${escapeAttr(keyed.sign)} ${debugSearchNumber(value(best))}</b> on turn ${best.last}`;
  if (alone && best.key === alone.key) said += `. That is ${seated} alone: no other line has beaten it`;
  else if (alone && (search.objective.want.length || search.objective.dont_want.length))
    said += `, <b>${debugSearchPercent(best.gain)}</b> on what is wanted against ${seated} alone` +
      (keyed ? ` (${escapeAttr(keyed.sign)} ${debugSearchNumber(value(alone))})` : "");
  debugDrawn(document.getElementById("dbg-search-best"), `${said}.`);
  // When no line meets every Need, that names a behaviour none of these Civ
  // AIs has: what a gene would be written for.
  const needs = search.objective.need.length || search.objective.avoid.length;
  text("dbg-search-front", !needs ? `${report.front_size || 0} line${report.front_size === 1 ? "" : "s"} on the front: none beats another on every term.`
    : report.feasible ? `${report.feasible} of ${report.lines} lines meet every Need and break no Must Avoid; ${report.front_size} on the front.`
    : `No line of ${report.lines} meets every Need and breaks no Must Avoid: none of these Civ AIs does what is needed, which names a behaviour to write a gene for.`);
  debugDrawn(document.getElementById("dbg-search-plan"), debugPlanStretches(best.plan, best.last).map(stretch =>
    `<span title="${escapeAttr(debugSearchAgent(stretch.name))} holds the seat from turn ${stretch.from} to turn ${stretch.to}"><b>${stretch.from}</b> ${escapeAttr(debugSearchAgent(stretch.name))}</span>`).join(""));
  const chart = document.getElementById("dbg-search-chart"), drawn = debugSearchChart(search);
  if (chart.dataset.drawn !== drawn) {
    chart.dataset.drawn = drawn;
    chart.innerHTML = drawn;
    debugSearchTip();
  }
  const key = (name, label) => `<span><i style="border-color:${DEBUG_SEARCH_COLOURS[name]}"></i>${label}</span>`;
  debugDrawn(document.getElementById("dbg-search-key"), !drawn ? "" :
    (alone && best.key !== alone.key ? key("best", "Best line") : "") + (alone ? key("alone", `${seated} alone`) : ""));
  // Every Civ AI holding the seat alone, in the order the engine ranks them:
  // what the line that was found is set against, and what each of them
  // comes to by itself.
  const lone = (report.alone || []).slice().sort((one, other) =>
    (other.feasible - one.feasible) || (one.verdict.broken - other.verdict.broken) || (other.gain - one.gain));
  debugDrawn(document.getElementById("dbg-search-table"), lone.map(line => {
    const name = escapeAttr(debugSearchAgent(line.plan[0][1])), held = alone && line.key === alone.key;
    const reads = keyed ? debugSearchNumber(value(line)) : debugSearchPercent(line.gain);
    return `<span${held ? ' class="dbg-top"' : ""} title="${name} alone from turn ${search.from}${held ? ": the Civ AI that is seated" : ""}${line.feasible ? "" : ". It does not meet every Need"}">${name}</span>` +
      `<span class="dbg-v">${reads}</span>`;
  }).join(""));
  // Doctor Strange's makes it a branch instead (`strangeSearchBranch`).
  if (STRANGE) { strangeSearchOffer(search, best, live); return; }
  // Playing it: on the one timeline, or in either half of a split. A line is
  // played on the world it was found in, so a timeline branched or closed
  // since the search has to be searched again.
  const moved = (search.pane === null) !== (debugSplit === null) ||
    (debugSplit !== null && debugSplit.panes[search.letter].id !== search.pane);
  const busy = live || !!search.playing || moved || !root || !root.idle;
  const button = (value, label, title) =>
    `<button data-dbg-search="play" value="${value}" title="${escapeAttr(title)}"${busy ? " disabled" : ""}>${label}</button>`;
  const walked = `walks back to turn ${search.from} and plays the line to turn ${best.last}`;
  let play;
  if (!debugSplit) play = button("here", "▶ Play it", `Play it: this timeline ${walked}, over the turns it holds past there`);
  else play = ["a", "b"].map(letter => {
    const name = letter.toUpperCase(), other = search.letter === "a" ? "A" : "B";
    return button(letter, `▶ Play it in ${name}`, letter === search.letter
      ? `Play it in ${name}: ${name} ${walked}, over the turns it holds past there`
      : `Play it in ${name}: ${name} is made a copy of ${other}, ${walked}. ${other} stays as it is, to set beside it`);
  }).join("");
  const note = moved ? "The timeline was branched or a branch closed since this search: search again to play a line."
    : live ? "Stop the search to play the best line so far." : search.said;
  debugDrawn(document.getElementById("dbg-search-play"), `${play}<span class="dbg-search-status">${escapeAttr(note)}</span>`);
}
// The best line is played: on the one timeline by this page, and in a split
// by the half that was picked, which is first made a copy of the half the
// search began from when it is the other one.
async function debugSearchPlay(target) {
  const search = debugSearch, best = search && search.report ? search.report.best : null;
  if (!search || search.live || !best || search.playing) return;
  const first = search.objective.want[0];
  const plan = {from: search.from, print: search.print, want: first ? first.key : null,
    value: best.verdict && best.verdict.want ? best.verdict.want[0] : null,
    stretches: debugPlanStretches(best.plan, best.last)};
  if (!plan.stretches.length) return;
  if ((search.pane === null) !== (debugSplit === null)) return;
  if (!debugSplit) {
    search.playing = "here";
    search.said = `Playing it from turn ${search.from}…`;
    drawDebugBar(false);
    debugSearchPlayed(search, await debugPlayPlan(plan));
    return;
  }
  if (debugSplit.panes[search.letter].id !== search.pane || debugSplit.busy || debugSplit.running > 0) return;
  const letter = target === "b" ? "b" : "a";
  const copy = letter === search.letter ? null : search.pane;
  // A copy is the other half's whole history, and so is its line in the chart.
  if (copy) debugSplit.history[letter] = new Map(debugSplit.history[search.letter]);
  search.playing = letter;
  search.said = `Playing it in ${letter.toUpperCase()} from turn ${search.from}…`;
  debugSplitTell(letter, {civvis: "pane-plan", plan, copy});
  drawDebugBar(false);
}
function debugSearchPlayed(search, done) {
  search.playing = null;
  search.said = done && typeof done.note === "string" ? done.note : "";
  drawDebugBar(false);
}
// Play a line that was found, on this page's own world: the one timeline, or
// one half of a split (told by its host, `pane-plan`).
//
// The world is walked back to the turn the search began on, with the line's
// first Civ AI named, which is what each copy the search played on did: the
// agents kept with that turn sit down again (a half keeps none, and seats
// new ones), and the line is played from there. Each stretch of it seats its
// Civ AI and runs to the stretch's last turn, as Auto-play would, so a
// breakpoint stops it and so does Stop. A Civ AI aimed at a victory or with
// a gene switched on is not on the page's picker: it is put there for the
// line. The answer says how far it got and, when it was played out, what the
// seat has of the first Want beside what the search read.
async function debugPlayPlan(plan, copy = null) {
  const fail = note => { debugSay(note); return {ok: false, turn: state ? state.turn : null, note}; };
  if (debugRunning || autoplaying || debugBusy || turnAdvancePending || !debugTimeline())
    return fail("The game is being played: play the line when it stands still");
  const page = document.getElementById("autoplaystrategy");
  const stretches = Array.isArray(plan.stretches) ? plan.stretches : [];
  const offered = name => [...page.options].some(option => option.value === name);
  if (!page || !stretches.length || stretches.some(stretch => !offered(stretch.name) && !/[@+]/.test(stretch.name)))
    return fail("That line names a Civ AI this page does not offer");
  // The Civ AI of a stretch is this page's pick from then on. A half says so
  // to its host, which keeps each half's, and plays what follows by itself:
  // the other half is not its twin while their Civ AIs differ.
  const seat = name => {
    if (!offered(name)) page.add(new Option(debugSearchAgent(name), name));
    page.value = name;
    debugRememberSeat();
    if (DEBUG_PANE === null) return;
    debugTwin = null;
    window.parent.postMessage({civvis: "pane-seated", pane: DEBUG_PANE, name}, location.origin);
  };
  if (copy) {
    // A copy is not a move of this half's own, so the other is not told to follow it.
    debugMirroring = true;
    try {
      const copied = await debugOp({op: "copy", from: copy}, 60000);
      if (!copied || copied.error) return fail("The other branch could not be copied to play the line in");
    } finally {
      debugMirroring = false;
    }
  }
  debugSay("");
  for (const [at, stretch] of stretches.entries()) {
    seat(stretch.name);
    if (at === 0) {
      const there = await debugOp({op: "jump", turn: plan.from, strategy: stretch.name});
      if (!there || there.error) return fail(`Turn ${plan.from} could not be walked back to${there && there.error ? `: ${there.error}` : ""}`);
      const marks = there.debug || {};
      const print = (marks.worlds || [])[(marks.turns || []).indexOf(plan.from)] ?? null;
      if (there.turn !== plan.from || (plan.print !== null && plan.print !== undefined && print !== plan.print))
        return fail(`Turn ${plan.from} is no longer the world that was searched: search again from here`);
    } else {
      const seated = await debugSeat();
      if (!seated || seated.error) return fail(`${debugSearchAgent(stretch.name)} could not be seated on turn ${state.turn}`);
    }
    await debugRun(stretch.to);
    // Stopped short: by a breakpoint, by hand, or by the end of the game.
    // The bar has said which.
    if (state.turn < stretch.to)
      return {ok: false, turn: state.turn, note: `Stopped on turn ${state.turn}, before the line was played out: ${debugNoteText || "it was not finished"}`};
  }
  const keyed = plan.want ? debugSearchKeyed(plan.want) : null;
  const seated = (state.players || []).find(player => player.id === 0) || {};
  const got = keyed ? debugSearchRead(seated, keyed.key) : null, read = typeof got === "number";
  let note = `Played to turn ${state.turn}`;
  if (keyed && read && typeof plan.value === "number") {
    note += Math.abs(got - plan.value) < 1e-6
      ? `: ${keyed.sign} ${debugSearchNumber(got)}, as found`
      : `: ${keyed.sign} ${debugSearchNumber(got)}, not the ${debugSearchNumber(plan.value)} found`;
  }
  debugSay(note);
  return {ok: true, turn: state.turn, value: read ? got : null, note};
}

// ------------------------------------------------- Doctor Strange's Civ 6
//
// Operator 2026-11-23: "it will have a scrolling guitar hero setup. where the
// timeline we are on and following is center screen. forward in time is up.
// forward will branch out more in the tree. bottom left quad and bottom right
// quad, roughly are our world to compare. 2 simulatoins. this is limited at 2
// for this demo. the ai debugger we will make to scale to any number of
// screens."
//
// `/doctor-strange` is a played game under the debugger, shown another way. It
// began as the same split: one engine, two worlds A and B, locked to one
// turn, each with a Civ AI of its own in the seat. What changes is the
// picture. The worlds are boxes along the bottom of the window, map and
// little else. The
// timeline is not a bar across the top but a highway up the middle: the turn
// that is open is a line across the centre of the window, the turns played
// run down from it between the worlds, and the turns to come run up from it
// and away. Playing scrolls it, as the notes come down a rhythm game's board.
//
// The highway is a tree. The turns the two worlds share are its trunk; from
// the turn they part on, each has a branch of its own. The timeline being
// followed stands in the middle, straight up the window, and the other peels
// off towards its own world's side. A branch is made in one step, by picking
// the player strategy that plays it (`strangeBranch`): the other world
// becomes a copy of the followed one on the open turn with that strategy in
// the seat, and the branch it was on stays on the tree, struck off where it
// ended, so the tree grows a fork for every branch made.
//
// Operator 2026-11-24: "branches should be labeled with their series of
// player strategies. like MoonshotMaven-150-JackOfAllTrades-160-MoonshotMaven
// would mean moonshot maven played the first 1-149 turns, then jackof
// alltrades from 150-159, then moonshotmaven 160 onwards", "we should
// highlight a box in the lower part of the screen with the color of the
// branch. each box should have an exit to close the box. the boxes should
// somewhat automatically resize and the divide between the boxes on the
// bottom and the lines on the top", "The cursor over a world on the bottom
// should highlight that world's line. we should also not spew random lines
// on the timeline. Move turn count up to the center line, as that is the
// current turn."
//
// So a branch is named by who played it (`strangeSeries`, from the engine's
// own record of each stretch), and has a colour of its own, which is also
// the colour of the box its world is shown in. The line of the open turn is
// the divide between the tree above and the boxes below; the page places it
// by the shape of the boxes, and it can be dragged. Only what was played is
// drawn as a line.
//
// Operator 2026-11-24, the same day: "instead of random lines, we can
// compute (a degree in advance) the futures of the paths we are on. if there
// are 2 screens below, or 1, or 3. we should be able delete branches and
// that removes the line and the window. we should be able to handle scaling
// windows to a high number. we should also monitor resource usage - maybe
// track memory used and cpu % actively on the screen, in the upper right.
// the upper right chart and drop to the lower right of the upper half of the
// screen", and "an experience i want for example, is maybe to be simulating
// along 3 paths. 2 different player-strategies at the start of the game and
// 1 branched during the game. (also, we should default open doctor strange
// civ 6 with our top strategy on the right and the challenger on the left
// ...) ... we should have for example nuclear war visual forecasts out in
// the future (we will compute the future of each path). these should alert
// on the top section out in the distance."
//
// So the worlds are as many as are branched, up to `STRANGE_LETTERS`: the
// split's own two and, past them, pages this section makes and the split's
// listener keeps beside them (`strangeWorldOpen`). A branch is a new world
// in a new box and deleting one takes its line and its box away
// (`strangeClose`). The tree is worked out from what every world says it
// holds (`strangeTree`). The split's own lock joins two halves, so it is
// left open and the worlds are taken through the turns together by this
// section, by name (`strangeAsk`). Each world's future is played ahead of
// the open turn by the engine (`ahead`, `strangeAheadNext`), is drawn as the
// line it is, and carries what was seen coming on it. What all of it costs
// is read and shown in the top right corner (`strangeMeasure`).
//
// Operator 2026-11-24, later: "add a 'Generate Black Swan' - this will be a
// random, unpredicted event that spawns and changes the game. Could be
// instant nuclear war. Bio risk could collapse the population of all cities
// by 50%. Radiation event that starts leaking radiation around the world.
// Leader collapse. Civ blossoms out into powerful hostile nuclear armed
// microstates. stuff like that. black swans on a civilizational timeline."
//
// So there is Generate Black Swan, at the top of the sky (`strangeSwan`).
// The engine strikes one on a world as it stands (`black_swan`, `src/game/black_swan.rs`): from
// that turn it is another world, the future that was played ahead of it is
// let go of and played again, and the swan is a mark on its line where it
// struck and a word in the line's name. Locked, the same swan strikes every
// world, as everything else happens to them together; unlocked, the one
// that is followed.
//
// Everything here is the host's. A half only puts its panels away
// (`strange-pane`), and all that plays, walks and copies is the split's own
// (`debugSplitRun`, `debugSplitGoTo`, `pane-copy`, `debugSplitSeat`). The
// bar is still there, unseen, because half A's deck is drawn from it.

// Whether this page is Doctor Strange's Civ 6, and whether it is a half of one.
// Which of the simulator's pages a page is, its address says
// (`CIVVIS_PAGE_MODE`, set by the head script of web/index.html from
// `/doctor-strange`, `/debugger` or `/edge-of-tomorrow`, or from an old
// link's `?mode=`); a half asks the page that holds it.
const STRANGE = (() => {
  try { return DEBUG_PANE === null && window.CIVVIS_PAGE_MODE === "strange"; }
  catch (_) { return false; }
})();
const STRANGE_PANE = (() => {
  try { return DEBUG_PANE !== null && window.parent.CIVVIS_PAGE_MODE === "strange"; }
  catch (_) { return false; }
})();
// Whether this page is Edge of Tomorrow (`/edge-of-tomorrow`, the last
// section of this file). It is said here, before anything is drawn: the page
// is the Civilization VI screen from its first frame, whatever view the AI
// Debugger was last left in, and that choice is not kept for the AI Debugger
// to find.
const TOMORROW = (() => {
  try { return DEBUG_PANE === null && window.CIVVIS_PAGE_MODE === "tomorrow"; }
  catch (_) { return false; }
})();
if (TOMORROW) {
  document.documentElement.classList.add("tomorrow");
  SOLO_UI = true;
}
// Whether this page is CIVVIS: Moneyball (`/moneyball`, the last section of
// this file): the seat is played by the player strategy the person picks, a
// stretch of turns at a time, by its numbers alone.
const MONEYBALL = (() => {
  try { return DEBUG_PANE === null && window.CIVVIS_PAGE_MODE === "moneyball"; }
  catch (_) { return false; }
})();
if (MONEYBALL) document.documentElement.classList.add("moneyball");
// In Edge of Tomorrow the seat's only decisions are Auto-play and the jump
// (operator, 2026-11-26: "a player's only controls are the auto play stuff
// and the jump back / wake here"): the page sends no action, citizen or
// focus of its own for the seat. Auto-play, Jump back and Checkpoint here go to the
// engine by their own routes.
window.civvisSeatHandsOff = TOMORROW || MONEYBALL;
// And it shows the seat's research box (governors, research, civic,
// government and policies; `liveTrackerShown` in app.js), as the Debugger
// view does, and opens on the standings and the victory tracker.
window.civvisSeatTracker = TOMORROW;
// The player strategies the second world is given when the picker shows no
// rating to rank them by, in the order they are tried.
const STRANGE_RIVALS = ["siege", "moonshot", "holy", "tourist", "forward", "swing", "jack", "stock"];
// The highway's own colour: the board, and the line of the open turn.
const STRANGE_GOLD = "#f3d27a";
// The colours branches are given, in the order they are handed out. The
// first two are the split's own for A and B.
const STRANGE_INKS = [DEBUG_VERSUS_COLOURS.a, DEBUG_VERSUS_COLOURS.b, "#b79cff", "#8fe07a", "#ff7fc1", "#7aa8ff",
  "#ffb347", "#5fe3b5", "#e0d16f", "#d58bff", "#9ad0ff", "#ff9f9f"];
// The worlds that may be open at once, by name. Each is a whole page and a
// world of its own in the engine (`panes.rs` names a pane by a letter).
const STRANGE_LETTERS = "abcdefghijkl";
// How long the worlds stand still before a page for the next branch is made
// (`strangeSpare`), and how often that is looked at.
const STRANGE_SPARE_STILL_MS = 1500, STRANGE_SPARE_EVERY_MS = 1000;
// How tall a world's name plate is, the least room a box is left, and the
// strip left clear under the line of the open turn where no column between
// the boxes shows where the worlds stand on it.
const STRANGE_HEAD = 34, STRANGE_BOX_LEAST = 120, STRANGE_SILL = 30;
// The longest a branch's name is written on the tree, where room is short.
const STRANGE_NAME_MOST = 36;
// Send it (`strangeSendIt`): how many worlds it fills the page up to
// (operator 2026-11-27: "lets have the button spawn up to a total of 12
// worlds. so we can have a nice grid down below"), and how long it waits for
// their worlds to be there before it lets time run on without the rest.
const STRANGE_SEND_WORLDS = 12, STRANGE_SEND_WAIT_MS = 90000;
// How many worlds Branch 5 Next Best makes (`strangeNext`).
const STRANGE_NEXT_MANY = 5;
// How far ahead of the open turn each world's future is played, and the most
// turns of it a world is asked for at once (`strangeAheadNext`): while the
// worlds stand still, and between the turns of time running, where the next
// turn is shown only once what was asked for has been played.
const STRANGE_AHEAD = 12, STRANGE_AHEAD_CHUNK = 4, STRANGE_AHEAD_RUNNING = 2;
// How long a world asked for its future is waited on to answer.
const STRANGE_AHEAD_WAIT_MS = 90000;
// A world's whole future is played to the end of its game a stretch at a
// time (`strangeForesee`): how long a stretch is meant to take, the turns the
// first is asked for and the most any is, how long one is waited on, how
// long a world that could not be asked is left before it is asked again, and
// the least time between two futures begun for one world.
const STRANGE_SEE_MS = 400, STRANGE_SEE_FIRST = 4, STRANGE_SEE_MOST = 40;
const STRANGE_SEE_WAIT_MS = 120000, STRANGE_SEE_AGAIN_MS = 3000, STRANGE_SEE_ANEW_MS = 1000;
// The sign over the far end of each branch (`strangeFate`): how wide and how
// tall it is where there is room for all of it ("On current path," over
// what is coming, and the wars under it), for its first line, and for a word
// of it; the room left between two, and where the first row stands.
const STRANGE_FATES = {full: [292, 56], line: [178, 24], word: [88, 24]};
const STRANGE_FATE_GAP = 6, STRANGE_FATE_TOP = 50;
// The right-hand column at the top: what the page costs, the black swan, the
// victory tracker of the world followed and the defeat tracker.
const STRANGE_RIGHT_WIDE = 300;
// How often what the page costs is read, and how many readings are drawn.
const STRANGE_METER_MS = 2000, STRANGE_METER_KEPT = 60;
// What is seen coming on a world's future, by kind: its sign and its colour.
const STRANGE_SIGNS = {nuclear: ["☢", "#ff5a4d"], war: ["⚔", "#ffb347"], won: ["★", "#8fe07a"], over: ["✕", "#c9d2cd"]};
// A black swan: its sign, the colour it is drawn in on black, how long the
// page goes on saying that one struck, and how long a world is waited on to
// say that it was.
const STRANGE_SWAN = "🦢", STRANGE_SWAN_INK = "#f1ecff", STRANGE_SWAN_MS = 14000, STRANGE_SWAN_WAIT_MS = 90000;
// The worlds' card. Operator 2026-11-23: "lets track cumulative prod,
// science, culture, gold, faith next to the current turn yields". A yield
// has two columns a world: what it makes a turn, and what it has made over
// the game so far (the engine's `totals`, Gold net of upkeep). The rest are
// one figure each, in the first. Past three worlds there is room for the
// first column only.
const STRANGE_VERSUS_ROWS = [
  ["score", "Score", "Score"], ["cities", "Cities", "Cities"], ["population", "Population", "Population"],
  ["production", "Production", "Production"], ...DEBUG_VERSUS_ROWS.filter(([key]) => !["score", "cities", "population"].includes(key)),
];
const STRANGE_TOTALS = ["production", "science", "culture", "gold", "faith"];
// The band's heads where it is narrow.
const STRANGE_VERSUS_SHORT = {population: "Pop", production: "Prod", science: "Sci", culture: "Cul", military: "Mil"};

const STRANGE_STYLE = `
  /* The bar is the split's one panel, so it stays where it is, unseen and deaf. */
  html.strange.dbg-split-open #debugbar { visibility: hidden !important; pointer-events: none !important; }
  html.strange #debugsplit {
    inset: 0 !important; display: block !important; overflow: hidden; background: #03070a;
    --strange-head: ${STRANGE_HEAD}px;
    /* Where the line of the open turn is and how wide the board is at it:
       the page's to say (\`strangeLayout\`). */
    --strange-line: 50%; --strange-board: 160px; --strange-turn-left: calc(50% + 110px);
  }
  html.strange #debugsplit .dbg-divider { display: none !important; }
  /* The worlds: a box each under the line of the open turn, in the colour of
     its branch. Where each stands is the page's to say. */
  html.strange #debugsplit iframe {
    position: absolute; z-index: 1; box-sizing: border-box;
    border: 2px solid var(--strange-tint, #35504a); border-top: 0;
  }
  /* A world of the split's own two that is closed is still there, unseen. */
  html.strange #debugsplit iframe.strange-shut { visibility: hidden; pointer-events: none; }
  html.strange #debugsplit.strange-dragging iframe { pointer-events: none; }
  html.strange #debugsplit iframe.strange-over { box-shadow: 0 0 26px -2px var(--strange-tint); z-index: 2; }
  #strange-sky { position: absolute; inset: 0; z-index: 0; width: 100%; height: 100%; display: block; cursor: pointer; }
  #strange-hud {
    position: absolute; inset: 0; z-index: 2; pointer-events: none; user-select: none;
    color: #e8e3d2; font: 12px/1.25 system-ui, sans-serif;
    /* The title's card, which the cards beside it stand clear of. */
    --strange-card: min(360px, 36vw);
  }
  #strange-hud > * { pointer-events: auto; }
  /* The highway's own controls. The simulator's standings and victory
     tracker, which stand on it (\`strangeStandingsMount\`), keep theirs as
     the simulator draws them, and the simulator's type. */
  #strange-hud button:where(:not(:is(.strange-standings, .strange-victory) *)), #strange-hud select:where(:not(:is(.strange-standings, .strange-victory) *)) {
    height: 26px; padding: 0 8px; border-radius: 8px; font: inherit; line-height: 1; cursor: pointer;
    color: inherit; background: #14231f; border: 1px solid #35504a;
  }
  #strange-hud select:where(:not(:is(.strange-standings, .strange-victory) *)) { min-width: 0; max-width: 100%; background: #060d0b; }
  #strange-hud button:where(:not(:is(.strange-standings, .strange-victory) *)):hover:not(:disabled) { border-color: ${STRANGE_GOLD}; }
  #strange-hud button:where(:not(:is(.strange-standings, .strange-victory) *)):disabled { opacity: .38; cursor: default; }
  #strange-hud :is(.strange-standings, .strange-victory) {
    font: var(--type-body)/1.5 var(--font-ui); color: var(--text); letter-spacing: .005em; user-select: text;
  }
  #strange-hud .strange-card { border: 1px solid #35504a; border-radius: 12px; background: #07110fd9; }
  #strange-hud .strange-title {
    position: absolute; left: 14px; top: 12px; box-sizing: border-box; width: var(--strange-card); padding: 10px 12px 10px;
  }
  /* The title a size up again and plainer to read (operator 2026-11-27:
     "Make the title line a bit bigger. and more readable"; 2026-11-26: "title
     can be a bit bigger"): 25px, a lighter gold with a shadow under it, in a
     card 30px wider than it was so it stays one line; a narrower window
     narrows the card (36vw) and the title with it. */
  #strange-hud .strange-title b {
    display: block; color: #ffe2a0; font: 700 clamp(17px, 2.5vw, 25px)/1.2 Georgia, serif; letter-spacing: .01em;
    text-shadow: 0 1px 2px #000c; white-space: nowrap;
  }
  /* Home, Charts/ and CIVVIS over the title (operator 2026-11-27: "make the
     top buttons Home, Charts/, and CIVVIS and color code them appropriately
     ... maybe make the buttons above it a bit different section"): the
     site's own colours for them, as every page's list has them (Home and
     Charts/ blue, CIVVIS gold), in a strip of their own over a rule. They
     were Home, CIVVIS and a gold New game (2026-11-26); the new games are
     under the title now. */
  #strange-hud .strange-title .strange-top {
    display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 5px;
    margin: -2px -4px 10px; padding: 0 4px 9px; border-bottom: 1px solid #35504a;
  }
  #strange-hud .strange-title .strange-top > a {
    box-sizing: border-box; height: 26px; padding: 0 8px; display: flex; align-items: center; justify-content: center;
    border: 1px solid #3b6788; border-radius: 6px; background: #274b66; color: #a8d8f5;
    font-size: 12px; font-weight: 700; letter-spacing: normal; text-transform: none; text-decoration: none; white-space: nowrap;
  }
  #strange-hud .strange-title .strange-top > a:hover { color: #fff; border-color: #66c0f4; background: #315d7e; }
  #strange-hud .strange-title .strange-top > a.strange-civvis { color: #17130c; border-color: #efd184; background: linear-gradient(145deg, #dec176, #aa7d30); }
  #strange-hud .strange-title .strange-top > a.strange-civvis:hover { color: #000; border-color: #fff3c9; background: linear-gradient(145deg, #f0d68e, #c0913c); }
  /* 2P Pangaea New Game and 6P Continents New Game under the title
     (operator 2026-11-27: "under the title, lets have a 2P Pangaea New Game
     option and 6P Continents New Game option"), in the gold New game had. */
  #strange-hud .strange-title .strange-news { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 5px; margin-top: 8px; }
  /* Each on two lines, the world over "New Game", as the AI Debugger's
     pair are: on one line "6P Continents New Game" was cut short where the
     fonts run wider (Linux's), in half of a 360px card. */
  #strange-hud .strange-title .strange-news button {
    width: 100%; height: auto; min-height: 36px; padding: 3px 4px; overflow: hidden;
    color: ${STRANGE_GOLD}; border-color: #8a7440; line-height: 1.15; letter-spacing: 0;
  }
  #strange-hud .strange-title .strange-news button > span {
    display: block; margin: 0; color: inherit; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  #strange-hud .strange-title .strange-news .strange-news-map { font-size: 12.5px; font-weight: 700; }
  #strange-hud .strange-title .strange-news .strange-news-new { margin-top: 1px; font-size: 10px; font-weight: 700; letter-spacing: .08em; text-transform: uppercase; opacity: .78; }
  #strange-hud .strange-title .strange-news button:hover:not(:disabled) { color: #10140f; background: ${STRANGE_GOLD}; border-color: transparent; }
  #strange-hud .strange-title span { display: block; margin-top: 4px; color: #bccac4; }
  /* What the page costs, in the top right corner (operator 2026-11-24: "we
     should also monitor resource usage - maybe track memory used and cpu %
     actively on the screen, in the upper right"). */
  #strange-hud .strange-meter {
    flex: none; padding: 8px 12px 8px; display: grid; gap: 4px 10px;
    grid-template-columns: auto auto 132px; align-items: center; font-variant-numeric: tabular-nums;
  }
  #strange-hud .strange-meter i { font-style: normal; color: #9fb3ab; }
  #strange-hud .strange-meter b { text-align: right; min-width: 4.4em; color: ${STRANGE_GOLD}; }
  #strange-hud .strange-meter canvas { width: 132px; height: 22px; display: block; }
  #strange-hud .strange-meter em { grid-column: 1 / -1; font-style: normal; font-size: 11px; color: #7f958d; }
  /* The top of the page (operator 2026-11-26: "for doctor strange's civvis,
     lets list the computed outcomes on the right side of the top scetion of
     the screen, ranked from best to worst. cpu and memory can go at the top.
     then: "Predicted Paths". The numbers table can go across the top, from
     the right side of the upper left box to the left side of the
     resources/predicted paths box i described. lets truncate the timelines
     map below this"): a column on the right, from the top down to the line
     (what the page costs, the black swan, the predicted paths, the defeat
     tracker; it scrolls when it is taller than the room), and the worlds'
     numbers across the top between it and the title. The tree is drawn
     under the numbers. */
  #strange-hud .strange-right {
    position: absolute; right: 14px; top: 12px; width: ${STRANGE_RIGHT_WIDE}px; box-sizing: border-box;
    max-height: calc(var(--strange-line) - 24px); display: flex; flex-direction: column; gap: 8px;
    overflow-y: auto; overscroll-behavior: contain; scrollbar-width: thin;
  }
  #strange-hud .strange-versus {
    position: absolute; top: var(--strange-versus-top, 12px); left: calc(14px + var(--strange-card) + 12px);
    right: calc(14px + ${STRANGE_RIGHT_WIDE}px + 12px); box-sizing: border-box; max-height: 34vh; overflow: auto;
    padding: 8px 12px 9px; display: grid; gap: 3px 0; align-items: baseline; font-variant-numeric: tabular-nums;
  }
  #strange-hud .strange-versus { container-type: inline-size; }
  #strange-hud .strange-versus em { font-style: normal; font-size: 11px; color: #9fb3ab; text-align: right; padding-left: 10px; white-space: nowrap; }
  #strange-hud .strange-versus em .strange-short { display: none; }
  @container (max-width: 760px) {
    #strange-hud .strange-versus em .strange-long { display: none; }
    #strange-hud .strange-versus em .strange-short { display: inline; }
  }
  /* A yield's total beside what it makes a turn, not under it: a row a
     number is one line high, and the band leaves the tree its room. */
  #strange-hud .strange-versus b small { display: inline; margin-left: .7em; font-size: 10.5px; font-weight: 400; color: #8fa39b; }
  #strange-hud .strange-versus b small.lead { color: ${STRANGE_GOLD}; font-weight: 700; }
  /* The world followed, as the simulator shows a world (operator 2026-11-27:
     "in doc stranges civvis, i can't actually tell what civ we are. please
     add the player hud in for the "Following" game. we can have this player
     hud above the path hud that has all the paths listed. lets add the
     victory track to the right as well"): the page's own standings over the
     worlds' numbers, and its own victory tracker in the column on the right,
     both drawn from the world followed (\`strangeStandingsDraw\`). The seat
     the worlds play is the standings' gold row. The turn is the line's, so
     the standings leave out their turn box; the tracker leaves out the
     record of games; and neither keeps what moves, sizes or hides a panel,
     because the highway says where these two stand. */
  #strange-hud .strange-standings {
    position: absolute; top: 12px; left: calc(14px + var(--strange-card) + 12px);
    right: calc(14px + ${STRANGE_RIGHT_WIDE}px + 12px);
  }
  #strange-hud .strange-standings:empty, #strange-hud .strange-victory:empty { display: none; }
  html.strange #strange-hud #playerhud {
    position: relative !important; inset: auto !important; width: auto !important; height: auto !important;
    max-height: 30vh; display: grid !important; grid-template-columns: minmax(0, 1fr) !important;
  }
  html.strange #strange-hud #playerhud > :is(.victory-turn, .turn-plate-seam, .turn-plate-restore, .standings-restore, .overlay-close, .widget-resize-handle) { display: none !important; }
  html.strange #strange-hud #playerhud > .player-standings { grid-column: 1 !important; display: flex !important; }
  #strange-hud .strange-victory { flex: 1 1 auto; min-height: 250px; display: flex; }
  html.strange #strange-hud #victoryhud {
    position: relative !important; inset: auto !important; width: auto !important; height: auto !important;
    flex: 1 1 auto; min-height: 0; display: grid !important; grid-template-rows: minmax(0, 1fr) !important;
  }
  html.strange #strange-hud #victoryhud > :is(.sim-stats-region, .sim-stats-seam, .overlay-close, .widget-resize-handle) { display: none !important; }
  html.strange #strange-hud #victoryhud > .victory-region { grid-row: 1 !important; }
  /* A civilization's name flies the simulator's own map to it, and that map
     is not shown here; a panel's heading drags the panel. */
  html.strange #strange-hud :is(#playerhud, #victoryhud) :is([data-hud-action="capital"], [data-widget-drag]) { pointer-events: none; cursor: default; }
  /* Under each column's letters, the player strategy that holds the seat
     there now (operator 2026-11-27: "add the names of the active strategy to
     the header of the paths hud"). */
  #strange-hud .strange-versus .strange-who {
    padding: 0 6px 3px; text-align: center; font-size: 11px; font-weight: 700;
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
  }
  /* End outcome, the last row (operator 2026-11-27: "lets merge the
     predicted paths into the path hud. make another row that is end outcome:
     and list the XXX wins, turnXXX, no wars, no nuclear wars"): how each
     world's future ends, as it has been played out to the end of its game
     (\`strangeEndOutcome\`). Worlds that share a column each have a line,
     led by the letter. The best is lit as the most of a row is. */
  /* The row is two lines high whatever it says, so the band, and the line
     of the open turn under it, stay where they are while the words change
     and while worlds share a column and part: a world alone in its column
     has both lines, two worlds sharing one have a line each, and past two
     the first world's line is followed by how many more there are. What is
     cut short is said whole on the pointer. */
  #strange-hud .strange-versus .strange-end {
    align-self: start; box-sizing: border-box; min-width: 0; height: 30px; padding: 2px 6px 0; overflow: hidden;
    text-align: center; font-size: 11px; line-height: 14px; color: #dfe8e3;
  }
  #strange-hud .strange-versus .strange-end > span { display: block; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  #strange-hud .strange-versus .strange-end > span:only-child {
    display: -webkit-box; -webkit-box-orient: vertical; -webkit-line-clamp: 2; white-space: normal;
  }
  #strange-hud .strange-versus .strange-end [data-outcome="won"] { color: ${STRANGE_GOLD}; }
  #strange-hud .strange-versus .strange-end [data-outcome="lost"] { color: #ff8a80; }
  #strange-hud .strange-versus .strange-end b { font-weight: 700; }
  #strange-hud .strange-versus em.strange-end-head { align-self: start; padding-top: 3px; }
  /* The outcomes are listed there now, not signed over each branch. */
  #strange-hud .strange-fate { display: none !important; }
  /* The defeat tracker (operator 2026-11-26: "add an inverse victory tracker
     - track progress to a nuclear war and losing. Same format as the victory
     track. we can call this defeat tracker. track turns away from a nuclear
     war"): the victory tracker's own paths, rows and meters, one row a world,
     beside the worlds' numbers, on the line, so the line stands no lower for
     it. A meter fills as the world nears it. */
  #strange-hud .strange-defeat { flex: none; box-sizing: border-box; padding: 6px 9px 7px; }
  #strange-hud .strange-defeat:empty { display: none; }
  #strange-hud .strange-defeat .hud-section-heading { margin: 0 0 3px; color: #e8e3d2; font-weight: 800; font-size: 11px;
    letter-spacing: .12em; text-transform: uppercase; }
  #strange-hud .strange-defeat .victory-track { padding: 4px 2px 5px; }
  #strange-hud .strange-defeat .victory-civ { pointer-events: none; }
  #strange-hud .strange-defeat .victory-meter { height: 4.5px; margin: 1px 0 0 10px; border-radius: 3px; }
  #strange-hud .strange-versus i { font-style: normal; color: #9fb3ab; }
  #strange-hud .strange-versus b { font-weight: 400; text-align: center; min-width: 3.2em; padding: 0 6px; }
  #strange-hud .strange-versus b.strange-total { min-width: 3.6em; padding-left: 8px; color: #bccac4; }
  #strange-hud .strange-versus .strange-pair { padding-left: 20px; }
  #strange-hud .strange-versus b.lead { font-weight: 700; color: ${STRANGE_GOLD}; }
  #strange-hud .strange-versus .strange-lit {
    padding: 1px 5px; margin-right: -5px; border-radius: 5px;
    background: color-mix(in srgb, ${STRANGE_GOLD} 22%, transparent);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, ${STRANGE_GOLD} 45%, transparent);
  }
  #strange-hud .strange-versus .strange-row-exit {
    width: 15px; height: 15px; margin-right: 3px; padding: 0; border-radius: 4px; vertical-align: 1px;
    font-size: 9px; color: #9fb3ab; background: transparent; border-color: #35504a;
  }
  #strange-hud .strange-versus .strange-row-exit::before { content: "✕"; }
  #strange-hud .strange-versus .strange-row-exit:hover:not(:disabled) { color: #10140f; background: ${STRANGE_GOLD}; }
  #strange-hud .strange-versus .strange-row-exit:disabled { opacity: .35; cursor: default; }
  #strange-hud .strange-versus .strange-letter { font-weight: 700; text-align: center; padding: 0 6px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  #strange-hud .strange-versus .strange-letter.strange-pair { padding-left: 20px; }
  #strange-hud .strange-versus em {
    font-style: normal; font-size: 10px; letter-spacing: .06em; text-transform: uppercase; text-align: left;
    color: #7f958d; padding: 0 10px 0 0;
  }
  #strange-hud .strange-versus em.strange-total { padding-left: 8px; }
  #strange-hud .strange-versus em.strange-pair { padding-left: 20px; }
  /* All 14,000,605 (operator 2026-11-26: "add a button to the left side top
     half but the bottom of it"): at the foot of the tree on the left, where
     the worlds' numbers stand on the right, and over the turn written on the
     line when that would be under it (\`--strange-send-lift\`). In Doctor
     Strange's colours, as his button in the site list is. */
  #strange-hud .strange-send {
    position: absolute; left: 14px; bottom: calc(100% - var(--strange-line) + 12px + var(--strange-send-lift, 0px));
    box-sizing: border-box; width: min(250px, 30vw); height: 36px; padding: 0 14px;
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
    font: 700 15px/1 Georgia, serif; letter-spacing: .03em; font-variant-numeric: tabular-nums;
    color: #ffe3a3; background: linear-gradient(135deg, #1c2a7a 30%, #a3122f); border: 1px solid #ff9a2e;
    box-shadow: 0 0 6px rgba(255, 138, 30, .6);
  }
  #strange-hud .strange-send:hover:not(:disabled) {
    color: #fff; background: linear-gradient(135deg, #2638a0 30%, #c8193d); border-color: #ffb85c;
    box-shadow: 0 0 10px rgba(255, 138, 30, .6);
  }
  #strange-hud .strange-send.on, #strange-hud .strange-send.on:disabled { opacity: 1; cursor: progress; box-shadow: 0 0 18px rgba(255, 138, 30, .9); }
  /* The divide between the tree and the boxes is the line of the open turn,
     and is taken hold of there. */
  #strange-hud .strange-divide {
    position: absolute; z-index: 5; left: 0; right: 0; top: calc(var(--strange-line) - 5px); height: 10px; cursor: row-resize;
  }
  /* A grip in the middle says it can be moved (operator 2026-11-26: "lets
     also make sure the dividing line beteen the bottom worlds and the top
     sectoin is moveable"). The whole width takes hold of it. */
  #strange-hud .strange-divide::after {
    content: ""; position: absolute; left: 50%; top: 2px; width: 64px; height: 6px; margin-left: -32px; box-sizing: border-box;
    border-radius: 3px; background: #9fb3ab55; border: 1px solid #9fb3ab88;
  }
  #strange-hud .strange-divide:hover::after, html.strange #debugsplit.strange-dragging .strange-divide::after { background: ${STRANGE_GOLD}; border-color: ${STRANGE_GOLD}; }
  /* A world's name plate, across the top of its box: the box's upper edge,
     in the colour of its branch. */
  #strange-hud .strange-world {
    position: absolute; height: var(--strange-head); box-sizing: border-box; padding: 0 5px 0 9px;
    display: flex; align-items: center; gap: 8px; white-space: nowrap; overflow: hidden;
    border: 2px solid var(--strange-tint); border-bottom: 1px solid #22332e;
    background: color-mix(in srgb, var(--strange-tint) 13%, #0a1512);
  }
  #strange-hud .strange-world.strange-over { background: color-mix(in srgb, var(--strange-tint) 30%, #0a1512); }
  /* One box has no column beside it for the trunk to come down, so its name
     plate is cut away around where the world stands on the line. */
  #strange-hud .strange-world.strange-notch {
    -webkit-mask: radial-gradient(circle 32px at 50% 0, transparent 30px, #000 31px);
    mask: radial-gradient(circle 32px at 50% 0, transparent 30px, #000 31px);
  }
  #strange-hud .strange-world > b { font: 700 17px Georgia, serif; color: var(--strange-tint); }
  /* The branch's name: the player strategies that played it, in order. */
  #strange-hud .strange-world .strange-name {
    flex: 0 1 auto; min-width: 5em; overflow: hidden; text-overflow: ellipsis;
    color: var(--strange-tint); font-weight: 700; font-variant-numeric: tabular-nums;
  }
  #strange-hud .strange-world select { flex: none; width: auto; max-width: 30%; }
  #strange-hud .strange-world [data-strange-line] { flex: 1 100 0; min-width: 0; overflow: hidden; text-overflow: ellipsis; color: #bccac4; font-variant-numeric: tabular-nums; }
  #strange-hud .strange-world button { flex: none; }
  #strange-hud .strange-world button.on { color: #10140f; background: ${STRANGE_GOLD}; border-color: transparent; font-weight: 700; cursor: default; opacity: 1; }
  #strange-hud .strange-world .strange-exit { width: 26px; padding: 0; font-weight: 700; }
  #strange-hud .strange-world .strange-exit:hover:not(:disabled) { color: #10140f; background: var(--strange-tint); border-color: transparent; }
  /* The hand on the highway, on the title's card. */
  #strange-hud .strange-deck { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 5px; margin-top: 9px; }
  #strange-hud .strange-deck button { width: 100%; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  #strange-hud .strange-deck .strange-play {
    grid-column: 1 / -1; height: 32px; font-size: 14px; font-weight: 700;
    color: #10140f; background: ${STRANGE_GOLD}; border-color: transparent;
  }
  /* Following: the timeline a branch is made from, picked before Create
     Branch (operator 2026-11-26: "change base timeline to "following:"",
     and "change "branch: ..." to "Create Branch: then the button. don't need
     "branch" on the button""), in its own colour, by its letter and its
     name. Each of the two says what it is on its left, and the two pickers
     start at one edge whatever the words measure in the font at hand: the
     labels are the deck's first column, the pickers its second. */
  #strange-hud .strange-deck.strange-branching { grid-template-columns: max-content minmax(0, 1fr); align-items: center; column-gap: 7px; }
  #strange-hud .strange-deck .strange-row { display: contents; }
  #strange-hud .strange-deck .strange-row > span { margin: 0; color: #9fb3ab; font-size: 11px; white-space: nowrap; }
  #strange-hud .strange-deck .strange-row > select { width: 100%; min-width: 0; }
  #strange-hud .strange-deck .strange-base select {
    height: 28px; font-weight: 700; color: var(--strange-base, #e8e3d2);
    border-color: var(--strange-base, #35504a); border-left-width: 6px; text-overflow: ellipsis;
  }
  /* A branch is made by picking who plays it: the picker is the button. */
  #strange-hud .strange-deck .strange-branch {
    height: 30px; font-weight: 700; color: ${STRANGE_GOLD};
    background: #14231f; border-color: ${STRANGE_GOLD}99; text-overflow: ellipsis;
  }
  #strange-hud .strange-deck .strange-branch:disabled { opacity: .38; cursor: default; }
  /* Over Branch, the next best (operator 2026-11-26: "in doctor strange's
     civvis, above the branch button, Branch Next Best and "Branch 5 Next
     Best" on the right"): a row across the deck between Following and Create
     Branch, its two halves the two buttons, in Create Branch's gold on its
     dark, a line thinner. */
  #strange-hud .strange-deck .strange-nexts { grid-column: 1 / -1; display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 5px; }
  #strange-hud .strange-deck .strange-next {
    height: 28px; font-weight: 700; color: ${STRANGE_GOLD}; background: #14231f; border-color: ${STRANGE_GOLD}66;
  }
  #strange-hud .strange-deck .strange-next:hover:not(:disabled) { border-color: ${STRANGE_GOLD}; }
  #strange-hud .strange-deck .strange-next:disabled { opacity: .38; cursor: default; }
  /* Autosearch, from the base timeline, under Branch (operator 2026-11-25:
     "add the autosearch stuff to doctor strange's civ 6. we should be able
     to efficiently search paths to try to find a desirable path"). A size
     larger than the buttons beside it and in gold, as in the AI Debugger;
     lit while a search runs, with the lines it has played counted. */
  #strange-hud .strange-deck .strange-search {
    grid-column: 1 / -1; height: 32px; display: flex; align-items: center; justify-content: center; gap: 6px;
    font-size: 13.5px; font-weight: 700; color: ${STRANGE_GOLD}; border-color: ${STRANGE_GOLD}; font-variant-numeric: tabular-nums;
  }
  #strange-hud .strange-deck .strange-search svg { display: block; flex: none; }
  /* Its words are the button's colour, not the card's for what it says. */
  #strange-hud .strange-deck .strange-search span { display: inline; margin: 0; color: inherit; }
  #strange-hud .strange-deck .strange-search.on { color: #10140f; background: ${STRANGE_GOLD}; border-color: transparent; }
  /* Its panel is the AI Debugger's, which is the bar's own, and the bar is
     unseen here: the panel alone is shown, a window over the tree. */
  html.strange.dbg-split-open #debugbar #dbg-search { visibility: visible !important; pointer-events: auto !important; }
  /* A black swan is not one of the hand's moves on the highway: it comes
     out of the blue. So it is struck from over the far end of the tree, at
     the top of the sky, between the cards: the button, and beside it which.
     Nothing is drawn up there but the far tips of the lines. */
  #strange-hud .strange-swans { flex: none; display: flex; gap: 5px; min-width: 0; }
  #strange-hud .strange-swans .strange-swan { flex: 1 1 auto; }
  #strange-hud .strange-swans .strange-swan {
    flex: 0 1 auto; min-width: 0; height: 32px; padding: 0 14px; font-size: 13px; font-weight: 700;
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
    color: ${STRANGE_SWAN_INK}; background: #000; border-color: ${STRANGE_SWAN_INK}b3;
  }
  #strange-hud .strange-swans .strange-swan:hover:not(:disabled) { border-color: #fff; box-shadow: 0 0 16px -2px ${STRANGE_SWAN_INK}; }
  #strange-hud .strange-swans select {
    flex: 0 0 92px; width: 92px; height: 32px; color: ${STRANGE_SWAN_INK}; background: #000;
    border-color: ${STRANGE_SWAN_INK}66;
  }
  /* What struck, under the button for as long as it is news. */
  #strange-hud .strange-omen {
    position: absolute; left: 50%; top: calc(var(--strange-band, 40px) + 10px); transform: translateX(-50%); box-sizing: border-box;
    width: max-content; max-width: min(470px, calc(100% - 2 * var(--strange-card) - 64px)); padding: 9px 14px 10px;
    text-align: center; cursor: pointer; color: ${STRANGE_SWAN_INK}; background: #000000eb;
    border: 1px solid ${STRANGE_SWAN_INK}; border-radius: 12px; box-shadow: 0 0 34px -4px ${STRANGE_SWAN_INK}99;
  }
  #strange-hud .strange-omen[hidden] { display: none; }
  #strange-hud .strange-omen b { display: block; font-size: 10.5px; font-weight: 700; letter-spacing: .16em; text-transform: uppercase; color: #bdb6d6; }
  #strange-hud .strange-omen strong { display: block; margin-top: 3px; font: 700 21px/1.15 Georgia, serif; color: #fff; }
  #strange-hud .strange-omen span { display: block; margin-top: 5px; }
  #strange-hud .strange-omen em { display: block; margin-top: 5px; font-style: normal; font-size: 11px; color: #bdb6d6; }
  /* The sign over the far end of each branch: what is coming in its world
     (operator 2026-11-24: "the top of each branch should have either: no
     nuclear war in the next N turns, no nuclear war, or nuclear war in N
     turns (Era). we should note wars as well"). In the branch's colour, and
     red where a nuclear war is coming. Where each stands is the page's to
     say (\`strangeLayout\`). */
  #strange-hud .strange-fate {
    position: absolute; box-sizing: border-box; padding: 3px 9px 4px 29px; overflow: hidden; white-space: nowrap;
    border: 1px solid #35504a; border-top: 3px solid var(--strange-tint); border-radius: 0 0 10px 10px;
    background: #07110fe6; font-variant-numeric: tabular-nums; cursor: default;
  }
  #strange-hud .strange-fate > b { position: absolute; left: 9px; top: 2px; font: 700 15px/18px Georgia, serif; color: var(--strange-tint); }
  /* "On current path," (operator 2026-11-25: "add to message: on current
     path, no nuclear war..."): what is coming is what comes if the world
     keeps to its line. Over what it says, where there is room for it. */
  #strange-hud .strange-fate small { display: block; font-size: 11px; line-height: 13px; color: #9fb3ab; overflow: hidden; text-overflow: ellipsis; }
  #strange-hud .strange-fate small:empty, #strange-hud .strange-fate.strange-fate-slim small { display: none; }
  #strange-hud .strange-fate strong { display: block; font-size: 12px; line-height: 17px; overflow: hidden; text-overflow: ellipsis; }
  #strange-hud .strange-fate span { display: block; font-size: 11px; line-height: 15px; color: #bccac4; overflow: hidden; text-overflow: ellipsis; }
  #strange-hud .strange-fate span:empty, #strange-hud .strange-fate.strange-fate-slim span { display: none; }
  /* How much of the game its future has been played through. */
  #strange-hud .strange-fate i { position: absolute; left: 0; bottom: 0; height: 2px; width: var(--strange-seen, 0%); background: var(--strange-tint); opacity: .85; }
  #strange-hud .strange-fate[data-fate="coming"], #strange-hud .strange-fate[data-fate="begun"] {
    border-color: #ff5a4d; border-top-color: var(--strange-tint); box-shadow: 0 0 18px -5px #ff5a4d;
  }
  #strange-hud .strange-fate[data-fate="coming"] strong, #strange-hud .strange-fate[data-fate="begun"] strong { color: #ff8f85; }
  #strange-hud .strange-fate[data-fate="clear"] strong { color: #8fe07a; }
  #strange-hud .strange-fate[data-fate="none"] strong { color: #9fb3ab; font-weight: 400; }
  #strange-hud .strange-fate.strange-over { background: color-mix(in srgb, var(--strange-tint) 22%, #07110f); }
  #strange-hud .strange-swans, #strange-hud .strange-omen { z-index: 1; }
  /* Turn 1, Era back, −10, −1, +1, +10, Era forward, in one row (operator
     2026-11-27: "this should be a clean set of controls. lets try to put in
     order: turn 1, era back ..."): the four steps share what the three
     named ends leave. */
  #strange-hud .strange-steps { grid-column: 1 / -1; display: grid; grid-template-columns: max-content max-content repeat(4, minmax(0, 1fr)) max-content; gap: 5px; }
  #strange-hud .strange-steps button { padding: 0; font-variant-numeric: tabular-nums; }
  #strange-hud .strange-steps [data-strange="first"], #strange-hud .strange-steps [data-strange^="era-"] { padding: 0 7px; }
  #strange-hud .strange-steps button i { font-style: normal; font-size: 9px; letter-spacing: -.15em; margin: 0 4px 0 -1px; }
  #strange-hud .strange-steps [data-strange="era-on"] i { margin: 0 0 0 4px; }
  #strange-hud .strange-title .strange-say { margin-top: 7px; color: #9fb3ab; font-size: 11px; min-height: 2.5em; }
  /* Operator 2026-11-26: "move "to the fork" up to be below the title on the
     left. move futures up to be right of this. then the -10, -1, 1, 10 line.
     then play time.then "B stopped on turn 185..." drop locked. should always
     be locked. drop the civvis and civvis ai debugger things". What makes a
     branch comes after what is said: the timeline followed, Create Branch
     and Autosearch. Later the same day: "the turn box doesnt appear right
     initially odten. please drop to the fork and future to the end": To the
     fork and Futures are gone, so the steps come first under the title. The
     futures are always played out (strange.futures), and a press on the
     highway, or the steps, go back. */
  #strange-hud .strange-title .strange-say + .strange-deck { margin-top: 4px; }
  /* The open turn is the line across the window, and is written on it, with
     its era, on the right of the tree, its left edge on the edge of the
     green path the worlds' lines run down between the boxes, or past the
     lanes on that side when they reach further (operator 2026-11-27: "please
     move turn count to the right side of the box. and move in a bit (if
     there aren't too many lines) to line up with the edge of the center
     green path with all the historical lines"; it stood left of the board
     before, off the window once a dozen worlds widened it). With so many
     lanes right of the followed world that it would stand over them, it is
     written left of the tree instead, the same way, where that is clear.
     Where is the page's to say (\`--strange-turn-left\`, \`strangeTurnLeft\`). */
  #strange-hud .strange-now {
    position: absolute; bottom: calc(100% - var(--strange-line)); box-sizing: border-box; height: 38px; padding: 0 11px;
    display: flex; align-items: center; gap: 6px; white-space: nowrap; pointer-events: none;
    border: 1px solid ${STRANGE_GOLD}88; border-bottom: 0; border-radius: 11px 11px 0 0; background: #07110fe6;
  }
  #strange-hud .strange-turn { left: var(--strange-turn-left); align-items: baseline; padding-top: 5px; color: #9fb3ab; font-weight: 700; letter-spacing: .12em; }
  #strange-hud .strange-turn b { color: ${STRANGE_GOLD}; font: 700 28px/1 Georgia, serif; letter-spacing: 0; font-variant-numeric: tabular-nums; }
  #strange-hud .strange-era { margin-left: 4px; color: #bccac4; letter-spacing: .08em; text-transform: uppercase; }
  #strange-hud .strange-era::before { content: "· "; }
  #strange-hud .strange-era:empty { display: none; }
  @media (max-height: 640px) { #strange-hud .strange-title > span, #strange-hud .strange-title .strange-say { display: none; } }
`;
// A half of it is a map: the standings, the tracker and the seat's own boxes
// are put away, and so is its own panel, because every world is moved by the
// page's one hand. The numbers of the worlds are set side by side by the host.
// The world minimap and the map's controls (deck, compass, lenses, zoom) are
// put away too: a world's box is its map alone, still moved by drag and wheel.
const STRANGE_PANE_STYLE = `
  html.strange-pane #playerhud, html.strange-pane #victoryhud, html.strange-pane #livepause,
  html.strange-pane #moments, html.strange-pane #site-pick, html.strange-pane #buildmark,
  html.strange-pane #debugbar, html.strange-pane #humanfooter,
  html.strange-pane .minimap-frame, html.strange-pane #map-controls-dock,
  html.strange-pane #lens-picker, html.strange-pane #winner { display: none !important; }
  #strange-result {
    position: fixed; left: 50%; top: 50%; z-index: 40; transform: translate(-50%, -50%);
    width: min(290px, calc(100vw - 24px)); display: grid; gap: 4px; justify-items: center;
    padding: 14px 16px 12px; text-align: center; color: #eef2ee;
    background: #08100eeb; border: 1px solid var(--sr-ink); border-radius: 12px; box-shadow: 0 18px 50px #000b;
    font: 500 12px/1.4 var(--font-ui);
  }
  #strange-result[hidden], #strange-result-chip[hidden] { display: none; }
  .strange-result.won { --sr-ink: #8fe07a; }
  .strange-result.lost, .strange-result.fell { --sr-ink: #ff8f7e; }
  .strange-result.over { --sr-ink: #c9d2cd; }
  #strange-result .sr-kicker { color: var(--sr-ink); font-size: 11px; font-weight: 800; letter-spacing: .14em; text-transform: uppercase; }
  #strange-result .sr-title { font-size: 17px; font-weight: 750; }
  #strange-result .sr-verdict { color: #d3dcd7; }
  #strange-result .sr-meta { color: #9fb0a8; font-size: 11px; }
  #strange-result .sr-actions { display: flex; gap: 8px; margin-top: 8px; }
  #strange-result button, #strange-result-chip {
    padding: 7px 12px; color: #eef2ee; background: #16211e; border: 1px solid #4a5a54; border-radius: 7px;
    font: 700 12px/1 var(--font-ui); cursor: pointer;
  }
  #strange-result button[data-sr="map"] { color: #0b120f; background: var(--sr-ink); border-color: transparent; }
  #strange-result button:disabled { opacity: .45; cursor: default; }
  #strange-result-chip { position: fixed; left: 10px; top: 10px; z-index: 40; color: var(--sr-ink); background: #08100ee6; border-color: var(--sr-ink); }
`;

// The host's record of the highway, beside the split's own.
const strange = {
  paintFrame: 0,
  // The timeline being followed: the one in the middle.
  follow: "a",
  // The turn on the line across the window, which follows the followed
  // world's turn, and the lane each world's branch stands in (0 is the
  // middle). Both move by easing.
  cam: null, lanes: {},
  // The worlds that are open, as their boxes stand from left to right, and
  // the order they were made in: an older world is what a newer one is a
  // branch of (`strangeTree`).
  order: ["a", "b"], born: {a: 0, b: 1}, births: 2,
  // Where each branch was made: the world it is a branch of and that
  // world's turn then ({from, turn}). Until it stands on that turn its copy
  // is on its way, and it is the same world as the one it came from.
  made: {},
  // The colour of each world's branch, and how many colours have been
  // handed out.
  tint: {a: STRANGE_INKS[0], b: STRANGE_INKS[1]}, inks: 2,
  // The tree as it was last worked out, and which worlds had parted from
  // the one they branch from when it was.
  tree: null, parted: {},
  // Whether the worlds go through the turns together, and whether they were
  // asked to while they stand on different turns: they are locked once the
  // rest have come to the followed one's (`strangeStep`).
  locked: true, lockWanted: false,
  // Where the line of the open turn was dragged to, as a share of the
  // window's height; null while the page places it. `floor` is the least it
  // may be, under the cards at the top, and `cards` where the cards are:
  // measured again once they have changed (`stale`, `watch`, `strangeCards`).
  divide: null, floor: 0, cards: [], stale: true, shape: "", watch: null, resized: false,
  // The world whose box the pointer is over.
  over: null,
  // Whether each world's future is played ahead of the open turn, and the
  // worlds being asked for more of their own now, by name: {turns, since}.
  // Every world is asked at once, and none for more while it has yet to
  // answer.
  futures: true, peeks: {},
  // The worlds asked for more of their whole future now, by name: {token,
  // since} (`strangeSee`), the tokens handed out, and where the signs over
  // the branches stand and how much each says (`strangeFates`).
  seeing: {}, sights: 0, fates: [], fateMode: "full",
  // The black swan being struck: {kind, seed, first, waiting, rest, struck,
  // failed, since}, null when none is (`strangeSwan`). And the one that last
  // struck, for as long as the page says so: {swan, letters, since}.
  swan: null, omen: null,
  // What the page costs, as it was read each time, and the last reading's
  // own figures to take the next from.
  meter: [], metered: null, measuring: false, meterTimer: null,
  canvas: null, hud: null, then: 0, hover: null, said: "", drawn: "",
  // Whether the bar has said there is no room for the worlds.
  sorry: false,
  // The worlds being taken through the turns together (`strangeAsk`):
  // {end, at, asked, stop, play, letters}. Null when they are not.
  run: null,
  // A branch playing a path Autosearch found (`strangeSearchBranch`):
  // {letter, from, search}. Null when none is.
  planning: null,
  // All 14,000,605 under way (`strangeSendIt`): {base, turn, letters, short,
  // since}, with `turn` null while time is still stopping for it. Null when
  // it is not.
  send: null,
  // Whether the controls were last drawn as free to press.
  idleWas: null,
};

function strangeAgents() {
  return [...((document.getElementById("autoplaystrategy") || {}).options || [])].map(option => option.value);
}
// The player strategies on offer, the best first, and whether that is known.
// The page's picker lists them by rating, best first, each with its rating
// before its name once something has measured it ("1512 - JackOfAllTrades",
// `strategiesByElo` and `strategyListing`); where the first has none shown
// they are in the order of `STRANGE_RIVALS`.
function strangeRanked() {
  const options = [...((document.getElementById("autoplaystrategy") || {}).options || [])];
  const offered = options.map(option => option.value);
  const rated = options.length > 1 && options[0].textContent.includes(" - ") && /\d/.test(options[0].textContent.split(" - ")[0]);
  if (rated) return {rated, names: offered};
  return {rated, names: [...STRANGE_RIVALS.filter(name => offered.includes(name)), ...offered.filter(name => !STRANGE_RIVALS.includes(name))]};
}
// The worlds whose boxes are open, from left to right.
function strangeOpen() { return debugSplit ? strange.order.filter(letter => debugSplit.panes[letter]) : []; }
// A player strategy as a branch's name carries it: its name as one word.
function strangeHandle(id) { return id ? debugAgentName(id).replace(/[^A-Za-z0-9]+/g, "") : "ByHand"; }
// A branch's name: the player strategies that played it, in order, with the
// turn each took the seat on between them. MoonshotMaven-150-JackOfAllTrades
// is MoonshotMaven up to turn 149 and JackOfAllTrades from turn 150.
//
// Who played which turns is the engine's record (`played_by`, a half's
// `marks.by`), read as far as the last turn of the world's own line. The
// strategy seated now is named last when it is not the one that played the
// line's last stretch: it plays from there.
function strangeSeries(letter, tree = strangeTree()) {
  const pane = debugSplit.panes[letter], last = tree.worlds[letter].last, told = (pane.marks || {}).by;
  const parts = [];
  for (const stretch of Array.isArray(told) ? told : []) {
    if (stretch.from >= last) break;
    if (parts.length === 0 || parts[parts.length - 1].by !== (stretch.by || null)) parts.push({from: stretch.from, by: stretch.by || null});
  }
  const seated = pane.strategy || null;
  if (parts.length === 0 || (seated && parts[parts.length - 1].by !== seated)) parts.push({from: last, by: seated});
  // A black swan that struck the line is in its name on the turn it struck,
  // among the strategies that played either side of it.
  const [head, ...rest] = parts;
  const tail = [...rest.map(part => ({turn: part.from, word: `${part.from}-${strangeHandle(part.by)}`, after: 0})),
    ...strangeSwans(letter).filter(swan => swan.turn <= last).map(swan => ({turn: swan.turn, word: `${STRANGE_SWAN}${swan.turn}`, after: 1}))]
    .sort((one, other) => one.turn - other.turn || one.after - other.after);
  return [strangeHandle(head.by), ...tail.map(part => part.word)].join("-");
}
// The black swans that have struck a world's line, as far as it is kept: the
// engine's account, which the world says with its marks (`debug.swans`).
function strangeSwans(letter) {
  const pane = debugSplit ? debugSplit.panes[letter] : null, told = pane && pane.marks ? pane.marks.swans : null;
  return Array.isArray(told) ? told.filter(swan => swan && typeof swan.turn === "number") : [];
}
// The next colour for a branch: not one an open world is drawn in.
function strangeInk() {
  const taken = strangeOpen().map(letter => strange.tint[letter]);
  for (let tried = 0; tried < STRANGE_INKS.length; tried++) {
    const ink = STRANGE_INKS[strange.inks++ % STRANGE_INKS.length];
    if (!taken.includes(ink)) return ink;
  }
  return STRANGE_INKS[0];
}
function strangeSay(text) { strange.said = text; strangeDraw(); }
// The game's last turn, when it has one.
function strangeLimit() {
  return [state.turn_limit, state.max_turns].find(turns => Number.isFinite(turns) && turns > 0) || null;
}
// Whether the worlds can be asked for something now.
function strangeIdle() {
  const split = debugSplit;
  if (!split || !split.shown || split.busy || split.running > 0 || strange.run || strange.lockWanted || strange.swan ||
    strange.planning) return false;
  const open = strangeOpen();
  return open.length > 0 && open.every(letter => split.panes[letter].turn !== null && !split.panes[letter].born);
}
// The window, as the highway is laid out in it. The open turn is a line
// across it. `up` and `down` are how far the highway runs from it, and each
// `tau` how many turns it takes to cover most of that: a turn near the line
// is tall and one far from it thin, so the whole game is on the board and
// the turns about to be played are the ones with room.
//
// The line is also the divide between the tree and the boxes. The boxes
// stand in rows under it, however many there are (operator 2026-11-24: "if
// there are 2 screens below, or 1, or 3", "we should be able to handle
// scaling windows to a high number"): up to three in a row, then two rows,
// then three. Left to the page the line gives a box the shape of a map,
// three high to five wide, within a share of the window, so one box is
// taller than two and takes the line up with it, and many boxes take no
// more than two thirds of the window between them. The line stays under the
// cards at the top, and leaves a box some room. Two boxes have the trunk's
// column between them; otherwise a strip is left clear under the line where
// the worlds stand on it, or with one box its plate is cut away there.
function strangeGeometry() {
  const W = window.innerWidth, H = window.innerHeight;
  const lane = Math.max(170, Math.min(240, W * .13));
  const open = strangeOpen(), count = Math.max(1, open.length);
  const rows = count <= 3 ? 1 : count <= 8 ? 2 : 3, cols = Math.ceil(count / rows);
  const gutter = count === 2 ? lane : 0, sill = count >= 3 ? STRANGE_SILL : 0;
  const wide = (W - gutter) / cols;
  const tall = rows === 1 ? STRANGE_HEAD + Math.max(H * .36, Math.min(H * .56, wide * .6))
    : Math.max(STRANGE_BOX_LEAST, Math.min((H * .66 - sill) / rows, STRANGE_HEAD + wide * .6));
  const wanted = strange.divide === null ? H - sill - rows * tall : strange.divide * H;
  const least = Math.min(strange.floor, H * .6), most = Math.max(least, H - sill - rows * STRANGE_BOX_LEAST);
  const cy = Math.round(Math.max(least, Math.min(most, wanted)));
  // Where each box stands: a row that is not full shares the width out.
  const boxes = {}, high = (H - cy - sill) / rows;
  open.forEach((letter, at) => {
    const row = Math.floor(at / cols), inRow = row === rows - 1 ? count - cols * (rows - 1) : cols, across = (W - gutter) / inRow;
    const col = at - row * cols;
    boxes[letter] = {x: col * across + (gutter && col === 1 ? gutter : 0), y: cy + sill + row * high, w: across, h: high};
  });
  // How far to either side of the followed world the others stand, in lanes.
  const mine = Math.max(0, open.indexOf(strange.follow));
  const reach = Math.max(1, mine, open.length - 1 - mine);
  return {W, H, gutter, lane, sill, boxes, reach, cx: W / 2, cy, up: Math.max(60, cy - (strange.treeTop ?? 16)), down: H - cy - 12, tauUp: 26, tauDown: 20,
    // How far apart two lanes stand: on the line, and at the far end.
    near: Math.min(42, lane / 2 - 34, (W * .44) / reach), far: Math.min(300, W * .15, (W / 2 - 90) / reach)};
}
// How wide the board is on the line of the open turn, to either side.
function strangeBoard(g) { return g.near * Math.max(2.7, g.reach * 2 + .7) + 40; }
// Where the turn on the line stands, by its left edge: right of the middle,
// on the edge of the green path the worlds' lines run down between the
// boxes, half a lane out, unless the lanes on that side reach further, then
// just past the outermost of them; never so far that it runs under the
// column on the right, which comes down near the line, or off the window.
// Where the lanes on the right leave it no room, it stands left of the tree
// the same way, if the lanes on that side do; with neither side clear, on
// the right as far out as there is room. `wide` is how wide the turn's chip
// is, and `column` where that column stands, as last measured
// (`strangeCards`).
function strangeTurnLeft(g, wide = strange.turnWide || 0, column = strange.turnColumn || null) {
  const open = strangeOpen(), mine = Math.max(0, open.indexOf(strange.follow));
  const clear = lanes => Math.max(g.lane / 2, g.near * lanes + 16);
  const right = g.cx + clear(open.length - 1 - mine), left = g.cx - clear(mine) - wide;
  const most = (column && column.bottom > g.cy - 50 ? column.left - 10 : g.W - 12) - Math.ceil(wide);
  if (right <= most) return Math.round(right);
  // Left of the tree it stands by its right edge, so it is placed to the
  // fraction of a pixel its width is: rounded, and its width rounded up, its
  // edge fell a pixel or more short of the green path.
  if (left >= 12) return +left.toFixed(2);
  return Math.round(Math.max(12, most));
}
function strangeY(g, turn, cam) {
  const ahead = turn - cam;
  return ahead >= 0 ? g.cy - g.up * (1 - Math.exp(-ahead / g.tauUp)) : g.cy + g.down * (1 - Math.exp(ahead / g.tauDown));
}
// The turn a height of the window stands for.
function strangeTurnAt(g, y, cam) {
  if (y <= g.cy) return cam - g.tauUp * Math.log(Math.max(1e-4, 1 - (g.cy - y) / g.up));
  return cam + g.tauDown * Math.log(Math.max(1e-4, 1 - (y - g.cy) / g.down));
}
function strangeSpread(g, y) {
  return y >= g.cy ? g.near : g.near + (g.far - g.near) * Math.pow(Math.min(1, (g.cy - y) / g.up), 1.5);
}
function strangeRamp(x) { return x <= 0 ? 0 : x >= 1 ? 1 : x * x * (3 - 2 * x); }

// The tree of the open worlds. Which worlds are the same is the engine's to
// say, as it is for the split's two (`debugSplitFork`): each world tells the
// host the turns it holds and the engine's print of each one's world, and
// two hold the same world on a turn they both give the same print for. A
// world is a branch of the older world it shares the most turns with, from
// the last turn the two share, and the oldest is the trunk. A world that
// holds no turn past that one has not parted yet and stands on the other's
// line.
//
// A world that is a step ahead in the middle of a move has not left the
// other, which is about to play the same turn: while the worlds are moving,
// one that is ahead alone has parted only if it had already (`strange.parted`).
//
// For each world: `parent` and the turn it branches on (`fork`), the first
// and last turn it holds, and whether it has `parted`. Also the first turn
// of all, and the turn the trunk first forks on (`splits`).
function strangeTree() {
  const split = debugSplit, from = split.from ?? state.turn, open = strangeOpen();
  const moving = split.busy || split.running > 0 || strange.run !== null;
  const line = {}, first = {}, last = {};
  for (const letter of open) {
    const pane = split.panes[letter], marks = pane.marks || {}, turn = pane.turn ?? from;
    let held = Array.isArray(marks.turns) && marks.turns.length > 0 && Array.isArray(marks.worlds) &&
      marks.worlds.length === marks.turns.length ? marks : null;
    // The split's branch, until it has said it begins on the turn it was
    // made on, still names turns ahead that are not its own.
    const cut = letter === "b" && split.branches ? from : Infinity;
    if (held && cut !== Infinity) {
      const kept = held.turns.filter(mark => mark <= cut).length;
      held = kept > 0 ? {turns: held.turns.slice(0, kept), worlds: held.worlds.slice(0, kept)} : null;
    }
    line[letter] = held ? new Map(held.turns.map((mark, at) => [mark, held.worlds[at]])) : null;
    first[letter] = Math.min(held ? held.turns[0] : pane.first ?? turn, turn);
    last[letter] = Math.min(cut, Math.max(held ? held.turns[held.turns.length - 1] : pane.last ?? from, turn));
  }
  // The last turn two worlds hold as one.
  const shared = (one, other) => {
    if (!line[one] || !line[other]) return Math.min(from, last[one], last[other]);
    const turns = [...line[one].keys()];
    for (let at = turns.length - 1; at >= 0; at--) if (line[other].get(turns[at]) === line[one].get(turns[at])) return turns[at];
    return Math.min(first[one], first[other]);
  };
  const aged = [...open].sort((one, other) => (strange.born[one] ?? 0) - (strange.born[other] ?? 0));
  const worlds = {}, parted = {};
  aged.forEach((letter, at) => {
    let parent = null, fork = first[letter];
    for (const elder of aged.slice(0, at)) {
      const turn = shared(letter, elder);
      if (parent === null || turn > fork) { parent = elder; fork = turn; }
    }
    const apart = parent !== null && Math.min(last[letter], last[parent]) > fork;
    parted[letter] = parent !== null && last[letter] > fork && (apart || !moving || strange.parted[letter] === true);
    worlds[letter] = {parent, fork: parent === null ? first[letter] : fork, first: first[letter], last: last[letter], parted: parted[letter]};
  });
  strange.parted = parted;
  const root = aged[0] ?? null;
  const splits = Math.min(Infinity, ...aged.filter(letter => worlds[letter].parted && worlds[letter].parent === root).map(letter => worlds[letter].fork));
  return {from, root, splits, worlds, first: Math.min(from, ...open.map(letter => first[letter]))};
}
// The open worlds that are the same world now, together, for the numbers
// band: on the same turn, with the same print of it from the engine, read
// as `strangeTree` reads it. A world with no print for its turn is its own.
// Each group stands where its first world stands, in the order given.
function strangeSame(open = strangeOpen()) {
  const split = debugSplit, from = split.from ?? state.turn, groups = [], prints = [];
  for (const letter of open) {
    const pane = split.panes[letter], marks = pane.marks || {}, turn = pane.turn ?? from;
    // The split's branch names turns ahead that are not its own until it
    // has said where it begins.
    const own = !(letter === "b" && split.branches && turn > from);
    const at = own && Array.isArray(marks.turns) && Array.isArray(marks.worlds) && marks.worlds.length === marks.turns.length
      ? marks.turns.indexOf(turn) : -1;
    const print = at >= 0 && marks.worlds[at] !== null && marks.worlds[at] !== undefined ? `${turn}|${marks.worlds[at]}` : null;
    const same = print === null ? -1 : prints.indexOf(print);
    if (same >= 0) groups[same].push(letter);
    else { groups.push([letter]); prints.push(print); }
  }
  return groups;
}
// New game (operator 2026-11-26: "to the top of dr strange game, above the
// title, lets add home, civvis, new game buttons. new game can be gold"):
// the page again, at its own address, which opens on a world of its own
// with a seed of its own (`OPENING_SEED` in beta/shim.js). An address that
// pins a world with `?game=` is let go of.
// A new game is the page again on a new world (the published page draws a
// new opening seed each visit): one of `players` civilizations on `map` when
// they are named (2P Pangaea New Game, 6P Continents New Game), which the
// page reads from its address as any link's (`beta/shim.js`).
function strangeNewGameAddress(players = null, map = null, from = window.location.href) {
  const next = new URL(from);
  next.searchParams.delete("game");
  if (players) next.searchParams.set("players", String(players));
  if (map) next.searchParams.set("map", map);
  return next.href;
}
function strangeNewGame(players = null, map = null) {
  window.location.assign(strangeNewGameAddress(players, map));
}

// A world's box is made: its name plate, and for a world past the split's
// own two its page and its record, which the split's own listener then
// keeps as it keeps A's and B's. A new page begins as the game the split was
// made from, and is made what it is to be once it is there (`born`).
function strangeWorldOpen(letter, born = null) {
  const split = debugSplit, hud = strange.hud, name = letter.toUpperCase();
  const pane = split.panes[letter] || strangeFrame(letter, born);
  pane.frame.classList.remove("strange-shut");
  pane.alerts = []; pane.ahead = new Map(); pane.spent = null; pane.over = null;
  strangeSightDrop(letter);
  if (hud.querySelector(`[data-strange-world="${letter}"]`)) return;
  // The sign over the far end of its branch (`strangeFate`).
  const fate = document.createElement("div");
  fate.className = "strange-fate";
  fate.dataset.strangeFate = letter;
  fate.setAttribute("role", "status");
  fate.innerHTML = `<b>${name}</b><small></small><strong></strong><span></span><i></i>`;
  hud.insertBefore(fate, hud.querySelector(".strange-now"));
  const plate = document.createElement("div");
  plate.className = "strange-world";
  plate.dataset.strangeWorld = letter;
  plate.innerHTML = `
      <b>${name}</b>
      <span class="strange-name" data-strange-name="${letter}"></span>
      <select data-strange-agent="${letter}" title="The player strategy in ${name}'s seat: it plays ${name} from the open turn, and is named in the branch's name from there" aria-label="Player strategy for ${name}"></select>
      <span data-strange-line="${letter}"></span>
      <button data-strange="follow-${letter}"></button>
      <button class="strange-exit" data-strange="close-${letter}" aria-label="Delete ${name}">✕</button>`;
  hud.insertBefore(plate, hud.querySelector(".strange-now"));
  // The pointer over a world's box, or its name plate, picks out its line.
  for (const held of [plate, fate, ...(pane.heard ? [] : [pane.frame])]) {
    held.addEventListener("mouseenter", () => { strange.over = letter; strangeLayout(); });
    held.addEventListener("mouseleave", () => { if (strange.over === letter) { strange.over = null; strangeLayout(); } });
  }
  pane.heard = true;
}
// A world's page, and its record beside A's and B's.
function strangeFrame(letter, born = null) {
  const split = debugSplit, frame = document.createElement("iframe");
  frame.dataset.dbgFrame = letter;
  frame.title = `Simulator ${letter.toUpperCase()}`;
  split.host.insertBefore(frame, strange.hud);
  const pane = split.panes[letter] = {frame, id: `${letter}-${split.panes.a.id.slice(2)}`, turn: null, first: null, last: null,
    deck: null, told: null, strategy: born ? born.name : undefined, agreed: false, painted: false, born};
  split.history[letter] = new Map();
  frame.src = debugPaneAddress(pane.id);
  return pane;
}
// ---- What a tab may hold
//
// Operator 2026-11-23, a standing rule: "please be careful about a memory
// blowup on civvis. considering the typical amount of memory people
// computres have". A visitor is an 8 GB laptop with other things open, and
// a tab is all it costs that computer: the page's own process, its part of
// the GPU process, and every engine's memory (`beta/worker.js`). An engine's
// memory never shrinks, so what counts is the most it has held.
//
// Doctor Strange's worlds are each a page and an engine of their own
// (`strangeFrame`, `beta/shim.js`), and nothing held them under a ceiling.
// Measured on the published engine (PR #5939: headless Chrome, the page's
// four-seat Pangaea, the page's process and the GPU process added up), two
// worlds were 1.1 GB on turn 1, 1.8 GB by turn 120 and 2.2 GB when their
// games ended (turn 190), four were 2.3-2.7 GB by turn 120, and All
// 14,000,605's ten went past 5 GB by turn 150 and were stopped there. What a
// tab is made of by the end of a game, the most it holds: the page itself,
// with its own engine and its part of the GPU process, about 920 MB
// (`pageMB`); each world's box, the viewer again in a frame, about 200 MB
// more (`boxMB`, 70 MB on turn 1: a box's page grows as its game does); and
// each world's engine, which keeps every turn played until it passes the
// line it is told (`room`, 1 GB unless told; the engine's least is 128 MB),
// with a copy of it for each future played ahead (`strangeForesee`). A
// world's engine was about 55 MB on turn 60, 200 MB on 120 and 330 MB at the
// end.
//
// So a tab is given a ceiling, 3/16 of the memory the browser says the
// computer has (`navigator.deviceMemory`, which Chrome rounds down to a
// power of two: 8 on an 8 GB laptop, 16 on a 16 GB one): 1.5 GB on the 8 GB
// budget, where a browser that does not say (Safari, Firefox) or says less
// is put too, and 3 GB on 16 GB. A link may say how much the computer has
// instead (`?memory=32`, in GB). A phone or a small device opens on the one
// timeline, as before, with its 384 MB line. The worlds live inside the
// ceiling (`strangeBudgetOf`):
//   - as many worlds may be open as fit with each box and each world's
//     engine at its least, and one future played beside them, and never
//     fewer than the two the page opens on; past that a branch is refused
//     and the page says why (`strangeFullWords`), and All 14,000,605 and
//     Branch 5 Next Best make as many as fit;
//   - what is left is the worlds' kept turns, the same line for each world,
//     and as many futures are played at once as fit beside them;
//   - a world deleted, or a page waiting to be one, holds no engine (`shim.js`
//     lets go of the engine of a world out of sight), a world's future played
//     to its end lets go of its copy (`strangeSeen`), and no page is made
//     ahead for a branch that would not fit (`strangeSpare`).
// The AI Debugger and Edge of Tomorrow keep their turns under the same
// ceiling (`debugRoomMB`).
const TAB_MEMORY = {share: 3 / 16, budgetGB: 8, mostGB: 64, pageMB: 920, boxMB: 200, opens: 2, engineLeastMB: 128, engineMostMB: 3072, smallMB: 384};
// The memory this computer is taken to have, in GB: what a link says, or
// else what the browser says, but never less than the 8 GB budget, which is
// what a browser that says nothing is given.
function tabMemoryGB(told = navigator.deviceMemory, asked = new URLSearchParams(location.search).get("memory")) {
  const linked = asked === null || asked === "" ? NaN : Number(asked);
  if (Number.isFinite(linked) && linked > 0) return Math.min(TAB_MEMORY.mostGB, Math.max(1, linked));
  const gb = Number(told);
  return Math.min(TAB_MEMORY.mostGB, Math.max(TAB_MEMORY.budgetGB, Number.isFinite(gb) ? gb : 0));
}
// The ceiling for the whole tab, in MB.
function tabCeilingMB(gb = tabMemoryGB()) {
  return Math.round(gb * 1024 * TAB_MEMORY.share);
}
// The line an engine is told, kept where the engine keeps it (`room`).
function tabEngineMB(megabytes) {
  return Math.max(TAB_MEMORY.engineLeastMB, Math.min(TAB_MEMORY.engineMostMB, Math.floor(megabytes)));
}
// Doctor Strange under a ceiling, with `worlds` open in `boxes` boxes (a
// box out of sight is a page still: one closed, or one made ahead):
//   fits     the worlds that fit: each box, each world's engine at its least
//            and one future beside them, by the end of a game;
//   most     the most that may be open: as many as fit, but the two the
//            page opens on however few do (on the 8 GB budget one does);
//   roomMB   the line each open world's engine is told: what the boxes
//            leave, shared by the worlds and the futures played at once;
//   futures  how many futures are played at once: every world's where each
//            world and its copy fit at the least, else one at a time.
function strangeBudgetOf(ceilingMB, worlds, boxes = worlds) {
  const {pageMB, boxMB, engineLeastMB: least, opens} = TAB_MEMORY;
  const engines = count => ceilingMB - pageMB - count * boxMB;
  let fits = 1;
  while (fits < STRANGE_LETTERS.length && engines(fits + 1) >= (fits + 2) * least) fits++;
  const open = Math.max(1, worlds), held = Math.max(0, engines(Math.max(open, boxes)));
  const futures = Math.max(1, Math.min(open, Math.floor(held / least) - open));
  return {ceilingMB, fits, most: Math.max(opens, fits), futures, roomMB: tabEngineMB(held / (open + futures))};
}
// This page's: the ceiling of this computer, the worlds open and the boxes there are.
function strangeBudget() {
  const split = debugSplit, open = strangeOpen().length;
  return strangeBudgetOf(tabCeilingMB(), open, split ? Math.max(open, Object.keys(split.panes).length) : open);
}
// What the page says when no more worlds fit: the dozen there are names
// for, or what this computer's memory holds. `are` says they are open now.
function strangeFullWords(budget = strangeBudget(), are = true) {
  const most = budget.most, gb = (budget.ceilingMB / 1024).toFixed(1);
  if (most >= STRANGE_LETTERS.length) return are ? `${STRANGE_LETTERS.length} worlds are open, which is the most` : `${STRANGE_LETTERS.length} worlds is the most`;
  return `${most} world${most === 1 ? "" : "s"} ${are ? `${most === 1 ? "is" : "are"} open, which is` : "is"} the most this computer's memory holds` +
    ` (a tab is kept under ${gb} GB)`;
}
// The line the engine of the AI Debugger's page, or of Edge of Tomorrow, is
// told: what the page and the boxes of a split leave under the ceiling. The
// halves of a split are worlds on the page's one engine, so each is told the
// same line; a small device keeps its own.
function debugRoomMB() {
  if (DEBUG_SMALL_DEVICE) return TAB_MEMORY.smallMB;
  const boxes = debugSplit ? Object.keys(debugSplit.panes).length : 0;
  return tabEngineMB(tabCeilingMB() - TAB_MEMORY.pageMB - boxes * TAB_MEMORY.boxMB);
}
// What every engine of this tab may hold together, the page's own, each
// world's and every copy of one: what the page and its boxes leave under the
// ceiling. A fork for Autosearch is made only where it fits in that
// (`beta/shim.js`, `forksThatFit`).
function tabEnginesMB() {
  return tabCeilingMB() - TAB_MEMORY.pageMB - (debugSplit ? Object.keys(debugSplit.panes).length : 0) * TAB_MEMORY.boxMB;
}
// The page's own engine, told its line while the deck is drawn idle, and
// told again if a new game forgot it.
function debugRoomAsk(timeline) {
  const megabytes = debugRoomMB();
  if (timeline.memory_line === megabytes || Date.now() - debugRoomAskedAt < 10000) return;
  debugRoomAskedAt = Date.now();
  // Asked of the engine as it is, not as a step of the bar's (`debugOp`): it
  // changes nothing in the game, so the bar is not made busy by it and
  // nothing pressed meanwhile, nor the game's opening, waits on it.
  fetchJSON("/debug", {method: "POST", headers: {"Content-Type": "application/json"},
    body: JSON.stringify({op: "room", megabytes})}).catch(() => {});
}
// The room a world's turns are kept in. Each world of Doctor Strange plays
// on an engine of its own (`beta/shim.js`), and an engine keeps its turns up
// to its memory line: each is told the line its share of the ceiling allows
// (`strangeBudget`), again as worlds are opened and deleted. Past its line a
// world gives back its oldest turns first. A half of the AI Debugger's split
// is a world on the page's one engine, and is told that engine's line.
let strangeRoomTold = null;
function strangePaneRoom(timeline) {
  let megabytes;
  try { megabytes = STRANGE_PANE ? window.parent.strangeBudget().roomMB : window.parent.debugRoomMB(); } catch (_) { return; }
  if (DEBUG_SMALL_DEVICE) megabytes = TAB_MEMORY.smallMB;
  if (timeline.memory_line === megabytes || strangeRoomTold === megabytes) return;
  strangeRoomTold = megabytes;
  // Told in turn with what the host tells this world (`debugInTurn`), never
  // between two of its steps: an op of the world's own in flight there is
  // what the next step of a follow finds, and that step is dropped, so the
  // world stopped short the moment time ran on after worlds were opened
  // (All 14,000,605 does that at once).
  debugInTurn(async () => { if (strangeRoomTold === megabytes) await debugOp({op: "room", megabytes}); });
}
// A page for the next branch is made ahead of time, out of sight, while the
// worlds stand still. A world's page is the whole viewer again, and making
// one took about half a second of the thread every world draws on, so a
// branch waited for it (0.85 to 1 s from the pick to the world on its turn)
// and, with time running, so did every other world. The branch now opens on
// the page that is waiting, as it does on a box closed before (`there` in
// `strangeBranch`), and the next is made once the worlds stand still again.
// One waits at a time, and none where a closed box is already waiting.
function strangeSpare() {
  const split = debugSplit;
  if (!split || !split.shown || !strange.hud || !strangeIdle()) { strange.stillSince = null; return; }
  const now = performance.now();
  if (strange.stillSince === null || strange.stillSince === undefined) strange.stillSince = now;
  if (now - strange.stillSince < STRANGE_SPARE_STILL_MS) return;
  if (Object.keys(split.panes).some(letter => !strange.order.includes(letter))) return;
  const letter = [...STRANGE_LETTERS].find(free => !strange.order.includes(free));
  // Nor one for a branch there is no room for (`strangeBudget`).
  if (!letter || strangeOpen().length >= strangeBudget().most) return;
  strangeFrame(letter).frame.classList.add("strange-shut");
}
// A branch, made in one step by picking the player strategy that plays it
// (operator 2026-11-24: "when we branch, we should select the player
// strategy and make the branch. in one step. Branch: 1100-MoonshotMaven").
// It is a world of its own, in a box of its own beside the one it was made
// from: the followed world as it stands on the open turn, with that strategy
// in the seat and a colour of its own. The two are one world until one of
// them plays a turn.
//
// A branch may also be made from a path Autosearch found (`plan`, from
// `strangeSearchBranch`): the world is born with the path's first strategy
// in the seat and then plays the path.
function strangeBranch(name, from = strange.follow, plan = null) {
  if (!strangeIdle() || !name) return null;
  const letter = strangeMake(name, from, from, plan);
  if (!letter) { strangeSay(`${strangeFullWords()}. Delete one to branch again`); return null; }
  strangeSay(`Branch ${debugSplit.panes[from].turn}-${strangeHandle(name)} is world ${letter.toUpperCase()}, from ${from.toUpperCase()}`);
  return letter;
}
// A branch's world is made, in a box beside `after`: the next free letter,
// or null when every one is taken.
function strangeMake(name, from, after = from, plan = null) {
  const split = debugSplit;
  const letter = [...STRANGE_LETTERS].find(free => !strange.order.includes(free));
  if (!letter || strangeOpen().length >= strangeBudget().most) return null;
  strange.order.splice(strange.order.indexOf(after) + 1, 0, letter);
  strange.born[letter] = strange.births++;
  strange.made[letter] = {from, turn: split.panes[from] ? split.panes[from].turn : null};
  strange.tint[letter] = strangeInk();
  strange.lanes[letter] = strange.lanes[from] ?? 0;
  const born = {from, name, plan}, there = !!split.panes[letter];
  strangeWorldOpen(letter, born);
  // One of the split's own two is there already, out of sight, and is made
  // the branch at once; a new page is, once it has loaded (`strangeStep`).
  if (there) { split.panes[letter].born = born; if (split.panes[letter].turn !== null) strangeBear(letter); }
  return letter;
}

// ---- Send it
//
// Operator 2026-11-26: "add a button to the left side top half but the
// bottom of it in doctor strange that says "All 14,000,605" and when clicked
// every ai-strategy we have (we can say top 10 for now) spawns from the
// current point and all start simulating towards the end. time carries
// forward for all. spawn a window for each". Then 2026-11-27: "change the
// all 14,000,000... button to "Send it". hover text can say spwan 10
// branches and run all out to the end. also, work to optimize this process
// so we are runing branches as fast as possible. we should be utilizing free
// cores to speed up other world. actually lets have the button spawn up to a
// total of 12 worlds. so we can have a nice grid down below."
//
// So the button stands at the foot of the tree on the left, where the
// worlds' numbers stand on the right. Pressed, the base timeline is branched
// on its open turn for the best player strategies that have no world of
// their own there yet, best first, each a world in a box of its own, until
// `STRANGE_SEND_WORLDS` worlds are open (twelve: three rows of four boxes),
// or as many as this computer's memory holds (`strangeBudget`), or until
// every player strategy has its world. A strategy that already has its
// world there is not given another: the one the base timeline seats, and
// one seated in a world that is still the same world as the base on this
// turn (`strangeSeated`), since that would be the same game played again.
// Once they are all there, time runs on for every open world to the end of
// the game, as fast as the worlds are played (`strangeAsk`, `fast`): the
// page shows the furthest turn every world holds instead of each turn in
// between, so drawing a dozen boxes a turn no longer holds the worlds back,
// and each world's engine plays on ahead the whole time, on a core of its
// own. Time already running is stopped first and the worlds branch from the
// turn it stops on. With nothing left to branch, it runs time on for the
// worlds that are open.

// The player strategies that have a world of their own on the base
// timeline's open turn: the one it seats, and each seated in a world that is
// still the same world as the base on this turn.
function strangeSeated(base) {
  const split = debugSplit, pane = split.panes[base], turn = pane.turn;
  const print = letter => {
    const marks = split.panes[letter].marks || {}, at = Array.isArray(marks.turns) ? marks.turns.indexOf(turn) : -1;
    return at >= 0 && Array.isArray(marks.worlds) ? marks.worlds[at] : undefined;
  };
  const mine = print(base);
  // A branch made from the base on this turn whose copy has not landed yet
  // is the base's world too (`strange.made`), or a quick second press would
  // branch its strategy again.
  const coming = letter => { const made = strange.made[letter]; return !!made && made.from === base && made.turn === turn && split.panes[letter].turn !== turn; };
  return new Set([pane.strategy, ...strangeOpen().filter(letter => letter !== base && (coming(letter) || split.panes[letter].turn === turn &&
    mine !== undefined && print(letter) === mine)).map(letter => split.panes[letter].strategy)]);
}
// The player strategies Send it branches the base timeline for, best first:
// as many as there is room for up to `STRANGE_SEND_WORLDS` open worlds.
function strangeSendNames(base = strange.follow) {
  const room = Math.min(STRANGE_SEND_WORLDS, strangeBudget().most) - strangeOpen().length;
  return room > 0 ? strangeNextNames(base).slice(0, room) : [];
}

// ---- Branch Next Best
//
// Operator 2026-11-26: "in doctor strange's civvis, above the branch button,
// Branch Next Best and "Branch 5 Next Best" on the right".
//
// Each branches the base timeline on its open turn for the best-rated player
// strategies that have no world of their own there yet (by All's rule,
// `strangeSeated`), best first: one, or five, each a world in a box of its
// own beside the base. Pressed again, each takes the next ones down the
// ranking, since the worlds just made are the base's own world on this turn.
// Like Branch, they make the worlds and leave time as it is, and past the
// most worlds that can be open the best are made and the page says how many
// were left out.

// Every player strategy without a world of its own on the base's open turn,
// best first.
function strangeNextNames(base = strange.follow) {
  const seated = strangeSeated(base), offered = strangeAgents();
  return strangeRanked().names.filter(name => offered.includes(name) && !seated.has(name));
}
function strangeNext(count) {
  const split = debugSplit;
  if (!split || !strangeIdle() || strange.send) return [];
  const base = strange.follow, turn = split.panes[base].turn, names = strangeNextNames(base).slice(0, count);
  const room = strangeBudget().most - strangeOpen().length;
  if (names.length === 0) { strangeSay(`Every player strategy has a world of its own on turn ${turn} already`); return []; }
  if (room <= 0) { strangeSay(`${strangeFullWords()}. Delete one to branch again`); return []; }
  const letters = [];
  for (const name of names.slice(0, room)) {
    const letter = strangeMake(name, base, letters.length ? letters[letters.length - 1] : base);
    if (!letter) break;
    letters.push(letter);
  }
  const short = names.length - letters.length;
  const made = letters.map((letter, at) => `${letter.toUpperCase()} ${turn}-${strangeHandle(names[at])}`).join(", ");
  strangeSay(count === 1 && letters.length === 1
    ? `Branch ${turn}-${strangeHandle(names[0])} is world ${letters[0].toUpperCase()}, from ${base.toUpperCase()}: the best player strategy without a world there`
    : `Branch ${count} Next Best: ${made}, from ${base.toUpperCase()}${short > 0 ? `. ${short} more did not fit: ${strangeFullWords(undefined, false)}` : ""}`);
  return letters;
}
function strangeSendIt() {
  const split = debugSplit;
  if (!split || strange.send) return;
  const going = split.running > 0 || strange.run !== null;
  if (going) {
    // With no room for another world, the worlds running now are run out to
    // the end as fast as they are played.
    if (strangeSendRoom().room <= 0) {
      if (strange.run && !strange.run.stop && !strange.run.fast) {
        strange.run.fast = true;
        strangeSay(`Send it: time runs on for all ${strangeOpen().length} worlds, to the end.`);
      }
      return;
    }
    // Time stops first, and the worlds branch from the turn it stops on.
    strange.send = {base: strange.follow, turn: null, letters: [], short: 0, since: performance.now()};
    if (!(strange.run && strange.run.stop)) strangePlay();
    strangeDraw();
    return;
  }
  if (!strangeIdle()) return;
  strangeSendSpawn(strange.follow);
}
// The room Send it has: for how many more worlds, up to the twelve, and how
// many of those this computer's memory holds.
function strangeSendRoom(budget = strangeBudget(), open = strangeOpen().length) {
  return {room: Math.max(0, Math.min(STRANGE_SEND_WORLDS, budget.most) - open), wanted: Math.max(0, STRANGE_SEND_WORLDS - open)};
}
function strangeSendSpawn(base) {
  const split = debugSplit, turn = split.panes[base].turn, {room, wanted} = strangeSendRoom();
  const names = strangeNextNames(base).slice(0, wanted);
  // Making a world draws the split, which looks here again (`strangeStep`):
  // until the last is made, there is nothing to do yet.
  const send = strange.send = {base, turn, letters: [], short: 0, none: names.length === 0, since: performance.now(), making: true};
  try {
    for (const name of names.slice(0, room)) {
      const letter = strangeMake(name, base, send.letters.length ? send.letters[send.letters.length - 1] : base);
      if (!letter) break;
      send.letters.push(letter);
    }
  } finally { send.making = false; }
  send.short = names.length - send.letters.length;
  if (send.letters.length) strangeSay(`Send it: ${send.letters.length} world${send.letters.length === 1 ? "" : "s"} branch ${base.toUpperCase()} on turn ${turn}…`);
  strangeSendWatch();
}
// Each time the split is drawn, and now and then besides: once time has
// stopped, the worlds are branched, and once they are all there, time runs,
// as fast as the worlds are played (`fast`).
function strangeSendWatch() {
  const send = strange.send, split = debugSplit;
  if (!send || send.making || !split) return;
  const late = performance.now() - send.since > STRANGE_SEND_WAIT_MS;
  if (send.turn === null) {
    if (split.running > 0 || strange.run !== null || !strangeIdle()) {
      if (late) { strange.send = null; strangeSay("The worlds did not stop, so nothing was branched"); }
      return;
    }
    strangeSendSpawn(strangeOpen().includes(send.base) ? send.base : strange.follow);
    return;
  }
  // Time was set running by hand meanwhile, which is what was to be done.
  if (split.running > 0 || strange.run !== null) { strange.send = null; strangeDraw(); return; }
  // A new world's page opens on the game's first turn and is a copy of the
  // base once it says it stands on the base's turn: time runs from there.
  const copied = send.letters.every(letter => !strange.order.includes(letter) || !split.panes[letter] || split.panes[letter].turn === send.turn);
  if (!strangeIdle() || (!copied && !late)) {
    if (late && !strangeIdle()) { strange.send = null; strangeSay("Not every branch has opened yet: press Send it again once they have"); }
    return;
  }
  strange.send = null;
  const made = send.letters.filter(letter => strangeOpen().includes(letter)).length, open = strangeOpen().length;
  strangePlay(null, true);
  const runs = split.running > 0 || strange.run !== null;
  const left = send.short > 0 ? ` ${send.short} more did not fit: ${strangeFullWords(undefined, false)}.` : "";
  const why = send.none && open < STRANGE_SEND_WORLDS ? `Every player strategy has a world of its own on turn ${send.turn}` : "";
  const what = made > 0 ? `Send it: ${made} branch${made === 1 ? "" : "es"} of ${send.base.toUpperCase()} from turn ${send.turn}` : why || "Send it";
  strangeSay(runs ? `${what}${made > 0 ? ", and" : ":"} time runs on for all ${open} worlds, to the end.${left}`
    : `${what}.${left}`);
}
// A world that is there is made what it was opened to be: the world it
// branches from, as that stands, with its own strategy in the seat.
function strangeBear(letter) {
  const split = debugSplit, pane = split.panes[letter], born = pane.born;
  pane.born = null;
  strangeSightDrop(letter);
  if (split.panes[born.from]) {
    split.history[letter] = new Map(split.history[born.from]);
    pane.settled = null;
    debugSplitTell(letter, {civvis: "pane-copy", from: split.panes[born.from].id});
  }
  debugSplitSeat(letter, born.name);
  // A path Autosearch found is played in it next, from the turn it was
  // found on: the world's page does each in the order it is told them.
  if (born.plan) debugSplitTell(letter, {civvis: "pane-plan", plan: born.plan});
}
// A branch is deleted: its line goes from the tree and its box from the
// window, and the boxes that are left share the room (operator 2026-11-24:
// "we should be able delete branches and that removes the line and the
// window"). The last world stays. A world past the split's own two is let
// go of, page and all; one of the two is put out of sight, because the
// split is made of them, and is the next branch's box.
function strangeClose(letter) {
  const split = debugSplit;
  if (!strangeCanClose(letter)) return;
  const name = strangeSeries(letter), at = strange.order.indexOf(letter);
  strange.order.splice(at, 1);
  if (strange.follow === letter) strange.follow = strange.order[Math.max(0, at - 1)];
  if (strange.over === letter) strange.over = null;
  delete strange.peeks[letter];
  strangeSightDrop(letter, true);
  for (const held of strange.hud.querySelectorAll(`[data-strange-world="${letter}"], [data-strange-fate="${letter}"]`)) held.remove();
  if (letter === "a" || letter === "b") split.panes[letter].frame.classList.add("strange-shut");
  else {
    split.panes[letter].frame.remove();
    delete split.panes[letter];
    delete split.history[letter];
  }
  strange.tree = null;
  strangeSay(`${name} deleted`);
}
// Locked, the worlds go through the turns together (operator 2026-11-23:
// "the doctor strange tool should step through turns in sync when locked
// (which is the default). always on the same turn. difference being only
// the intentional differentces: like say 2 diferent player startegies").
// The page asks each world for the same next turn at once and asks for
// another only when all stand on it: none is ever ahead of another by more
// than the turn the engine is playing for it. Each world plays with its own
// player strategy, and a turn a world has already played along its own
// timeline is walked to as it stood (`debugFollow`, "travel"). Unlocked,
// each world plays on by itself, as in the AI Debugger.
//
// `play` is time running, which shows every turn; a step to a turn goes as
// far at once as every world has already played.
function strangeTravel(end, play = false, fast = false) {
  const split = debugSplit;
  const at = strangeNow();
  if (!strange.locked) {
    if (end > Math.max(split.panes[strange.follow].last ?? 0, at)) debugSplitRun(end, [strange.follow]);
    else debugSplitGoTo(strange.follow, String(end));
    return;
  }
  // A world left standing short of a turn (`strangeStep`) is asked again.
  for (const letter of strangeOpen()) delete split.panes[letter].stood;
  strange.run = {end, at, asked: null, stop: false, play, fast: play && fast, letters: strangeOpen()};
  strangeAsk(strange.run);
}
// The turn the worlds stand on: the followed world's, unless the worlds are
// locked and its game ended before the others' did, or it was left standing
// short of a turn, and time went on for them (`strangeEnded`): then theirs,
// the furthest any world stands on.
function strangeNow() {
  const split = debugSplit, follow = split.panes[strange.follow];
  const end = follow.over ? follow.over.turn : follow.stood;
  if (!strange.locked || typeof end !== "number" || follow.turn !== end) return follow.turn;
  return Math.max(follow.turn ?? 0, ...strangeOpen().map(letter => split.panes[letter].turn ?? 0));
}
// The last turn a world holds on its own line.
function strangeKept(letter) {
  const pane = debugSplit.panes[letter];
  return Math.max(pane.last ?? 0, pane.turn ?? 0);
}
// The next turn the worlds are asked for: one turn on, or as far on as all
// have already played, or the turn gone back to.
function strangeAsk(run) {
  const split = debugSplit;
  run.letters = run.letters.filter(letter => strange.order.includes(letter) && split.panes[letter]);
  // Era forward plays on to the first turn of the followed world's next era,
  // which the world holds before time comes to it, its turns played ahead
  // being kept: time stops there. The followed world's game over first, it
  // stops where that is.
  if (run.era && !run.stop) {
    const dawn = split.panes[run.era.letter] ? (split.panes[run.era.letter].marks || {}).eras || [] : [];
    const began = dawn.find(era => era.turn > run.era.from);
    if (began && began.turn < run.end) {
      run.end = Math.max(run.at, began.turn);
      run.said = `The ${debugEraName(began)} era began on turn ${began.turn}`;
    } else if (!split.panes[run.era.letter] || strangeEnded(run.era.letter, run.at + 1)) {
      run.end = run.at;
      run.said = `${run.era.letter.toUpperCase()}'s game is over`;
    }
  }
  if (run.stop || run.at === run.end || run.letters.length === 0) { strangeRunOver(run); return; }
  // Time running shows every turn on the way, one after another (operator
  // 2026-11-27: "turn count appears to be skipping turns ... please fix such
  // that the turn counts iterates more smoothly and the mapstate match the
  // turn count"). Send it (`fast`) is no exception: its worlds' engines play
  // on ahead of the turn shown the whole time, so each turn it shows is one
  // its worlds already hold, and showing it is a walk along their lines.
  const next = run.end < run.at ? run.end
    : run.play ? run.at + 1 : Math.min(run.end, Math.max(run.at + 1, Math.min(...run.letters.map(strangeKept))));
  // A world whose game is over by then stays on the turn it ended on, and
  // time goes on for the rest; it stops only when every world's game is over
  // (`strangeEnded`). Gone back to before its end, a world is in again.
  const live = run.letters.filter(letter => !strangeEnded(letter, next));
  if (live.length === 0) { strangeRunOver(run, strangeOverNote()); return; }
  // While time runs, each world's future is played on ahead of it as it goes
  // (`strangeAheadNext`), and a world is shown the turns it holds as it
  // comes to them. Time waits on that only for a world that does not hold
  // the next turn yet: one that does is asked for more and shown the turn it
  // holds meanwhile, so time does not stop to look ahead.
  if (strange.futures && run.play && next > run.at) {
    strangeAheadNext(true);
    if (live.some(letter => strange.peeks[letter] && strangeKept(letter) < next)) {
      run.asked = null;
      strangeDraw();
      return;
    }
  }
  run.asked = next;
  run.shown = false;
  for (const letter of live) {
    split.panes[letter].settled = null;
    split.panes[letter].ready = null;
    debugSplitTell(letter, {civvis: "pane-follow", turn: run.asked, how: "travel", hold: true});
  }
  strangeDraw();
}
// The worlds holding a turn are shown it, at once. While time runs each is
// asked in the same breath for the turn of its future that showing this one
// takes from what it holds (`strangeAheadOwed`), so its engine plays that
// while the page draws the turn, and is never left with nothing to do.
function strangeShow() {
  const run = strange.run, running = !!run && run.play && !run.stop && strange.futures;
  for (const letter of strangeOpen()) {
    const pane = debugSplit.panes[letter];
    if (pane.ready === null || pane.ready === undefined) continue;
    const ahead = running ? strangeAheadOwed(letter, pane.ready, STRANGE_AHEAD_RUNNING) : 0;
    pane.ready = null;
    if (ahead > 0) strange.peeks[letter] = {turns: ahead, since: performance.now()};
    debugSplitTell(letter, ahead > 0 ? {civvis: "pane-show", ahead} : {civvis: "pane-show"});
  }
}
// Every turn is on the screen for at least one frame (operator 2026-11-27:
// "doc strange should show 1 frame from every turn at least"). A world draws
// the turn it is shown at once (`render`), but a world whose next turn it
// already holds can be shown that in the same frame, and a turn drawn and
// drawn over before the browser paints is never seen. So once every world
// has drawn a turn, the next is asked for at once but shown only after a
// frame has been painted: the frame asked for here is painted with every
// world's turn on it, and the next turn is shown in a task after it, so not
// even its count is painted early. A window nobody sees paints nothing and
// waits for nothing.
function strangeAwaitPaint(run) {
  if (document.hidden) return;
  run.painted = false;
  const frame = ++strange.paintFrame;
  const painted = () => {
    if (strange.paintFrame !== frame || run.painted) return;
    run.painted = true;
    if (strange.run === run) strangeStep();
  };
  requestAnimationFrame(() => setTimeout(painted, 0));
  // rAF stops for a window hidden meanwhile.
  setTimeout(painted, 250);
}
function strangePainted(run) {
  return run.painted !== false || document.hidden;
}
// The turn on the screen. While time runs, it is the one the worlds were
// last told to show (`strangeStep`), and the label says it in the same
// breath, so it changes with the maps: read off the followed world, it
// changed only once that world had said it was there and the page was next
// drawn, a turn behind the maps one look in seven, and a turn the page was
// not drawn on was never said at all.
function strangeShowing() {
  const run = strange.run;
  return run && typeof run.showing === "number" ? run.showing : strangeNow();
}
function strangeTurnSay(turn) {
  const held = strange.hud && strange.hud.querySelector('[data-strange="turn"]');
  if (held && typeof turn === "number" && held.textContent !== String(turn)) held.textContent = String(turn);
}
// Whether a world's game is over before `turn`: it ended on an earlier one
// (`pane-over`), or the world stands short of a turn it could not play while
// time ran on (`stood`, `strangeStep`). Its world is not taken to `turn`,
// and is not waited on.
function strangeEnded(letter, turn) {
  const pane = debugSplit && debugSplit.panes[letter];
  if (!pane || typeof turn !== "number") return false;
  return (!!pane.over && turn > pane.over.turn) || (typeof pane.stood === "number" && turn > pane.stood);
}
// A world has said its game is over, or is again not. Time that was waiting
// on its turn goes on without it (`strangeStep`, from the split's drawing).
function strangeOver(letter, over) {
  const pane = debugSplit && debugSplit.panes[letter];
  if (!pane) return;
  pane.over = over && typeof over.turn === "number" ? {turn: over.turn, how: over.how} : null;
  // A world left standing short of a turn because its game was over, which
  // it says only now (`strangeStep`): it is over, and if time stopped on it,
  // what is said is that.
  if (pane.over && typeof pane.stood === "number") {
    delete pane.stood;
    if (!strange.run && strangeOpen().every(open => debugSplit.panes[open].over)) strangeSay(strangeOverNote());
  }
  drawDebugSplit();
}
// What is said when time stops because no world has a game left to play.
function strangeOverNote() {
  const open = strangeOpen();
  if (open.length === 1) {
    const over = debugSplit.panes[open[0]].over;
    return `${open[0].toUpperCase()}'s game is over${over ? ` on turn ${over.turn}` : ""}`;
  }
  return "Every world's game is over";
}
// Whether a world whose game is over may be deleted now, while time runs on
// for the others: it stands on the turn its game ended on, is not part of
// what is moving, and nothing else is being done to the worlds.
function strangeEndedHere(letter) {
  const split = debugSplit, pane = split && split.panes[letter], run = strange.run;
  if (!pane || !pane.over || pane.turn !== pane.over.turn || !strangeQuiet()) return false;
  return !run || strangeEnded(letter, run.asked ?? run.at + 1);
}
// Nothing is being done to the worlds but time running on, if it is: no
// branch being made, no world brought to another's turn, no black swan, no
// path being played, no strategies being branched at once.
function strangeQuiet() {
  const split = debugSplit;
  return !!split && split.shown && !split.busy && split.running === 0 && !strange.lockWanted && !strange.swan &&
    !strange.planning && !strange.send;
}
// Whether a world may be deleted now: the last one open stays, and a world
// goes while the worlds stand still, or while time runs on locked, the rest
// going on without it (`strangeStep` waits on open worlds only).
function strangeCanClose(letter) {
  if (!strange.order.includes(letter) || strange.order.length < 2) return false;
  return strangeIdle() || strangeEndedHere(letter) || (!!strange.run && strangeQuiet());
}
function strangeRunOver(run, note = "") {
  if (strange.run !== run) return;
  strange.run = null;
  strangeShow();
  strangeSay(note || (run.stop ? `Paused on turn ${run.at}` : run.said || ""));
  // The worlds stand still again: their futures are played out from here.
  strangeAheadNext();
}
// Each time the split is drawn: whether the worlds stand on the turn they
// were asked for, and the next can be asked for.
function strangeStep() {
  const split = debugSplit, run = strange.run;
  if (!split) { strange.run = null; return; }
  strangeForesee();
  // The split's own lock joins its two halves and no others. Here every
  // world is moved by this page, by name, so the split is left unlocked.
  if (split.shown && split.locked && !split.busy && split.running === 0) { debugSplitLock(false); return; }
  for (const letter of strangeOpen())
    if (split.panes[letter].born && split.panes[letter].turn !== null) strangeBear(letter);
  if (strange.lockWanted && !run && split.running === 0) {
    const turn = split.panes[strange.follow].turn, rest = strangeOpen().filter(letter => letter !== strange.follow);
    const short = rest.find(letter => split.panes[letter].turn !== turn && split.panes[letter].settled &&
      split.panes[letter].settled.wanted === turn);
    if (rest.every(letter => split.panes[letter].turn === turn)) { strange.lockWanted = false; strange.locked = true; }
    else if (short) {
      strange.lockWanted = false;
      strangeSay(`${short.toUpperCase()} could not come to turn ${turn}, so the worlds stay unlocked`);
    }
  }
  if (strange.send) strangeSendWatch();
  if (!run || run.asked === null) { strangeAheadNext(); return; }
  // Each world holds the turn it has played until the rest have played
  // their own; one that was there already holds nothing.
  const ready = letter => {
    const pane = split.panes[letter];
    return pane.ready === run.asked || (pane.turn === run.asked && !!pane.settled && pane.settled.wanted === run.asked);
  };
  const letters = run.letters.filter(letter => strange.order.includes(letter) && split.panes[letter] && !strangeEnded(letter, run.asked));
  if (letters.length === 0) { strangeRunOver(run, strangeOverNote()); return; }
  if (!run.shown && letters.every(ready) && strangePainted(run)) {
    run.shown = true;
    run.showing = run.asked;
    strangeShow();
    strangeTurnSay(run.showing);
  }
  const there = letter => split.panes[letter].turn === run.asked;
  if (letters.every(there)) {
    // Time runs as fast as the worlds are played and drawn: a turn is on
    // the screen for as long as the next takes, and no longer, but for at
    // least one frame (`strangePainted`). The next is asked for at once.
    run.at = run.asked;
    run.asked = null;
    strangeAwaitPaint(run);
    strangeAsk(run);
    return;
  }
  // A world that is done following and stands short of the turn could not
  // play it: its game is over (a world that ends its game on a turn it is
  // asked for may say so only later, `pane-over`), or its engine failed the
  // turn. It does not stop time for the rest (operator 2026-11-26: "play time
  // should not stop. when clicked, it should compute all the way out to the
  // end or until stopped"): it is shown the turn it stands on and stays
  // there, as a world whose game is over does (`strangeEnded`), and time runs
  // on for the others. The next move asks it for its turns again
  // (`strangeTravel`). Time stops only when no world is left to play.
  const shorts = letters.filter(letter => !there(letter) && split.panes[letter].settled &&
    split.panes[letter].settled.wanted === run.asked);
  if (shorts.length === 0) return;
  for (const letter of shorts) {
    const pane = split.panes[letter];
    pane.stood = pane.turn;
    pane.ready = null;
    debugSplitTell(letter, {civvis: "pane-show"});
  }
  const stopped = letters => letters.map(letter => `${letter.toUpperCase()} stopped on turn ${split.panes[letter].stood}`).join(", ");
  if (shorts.length === letters.length) {
    // Every world left in the run stands short: they are named, with any
    // that stood earlier in it, as their answers came one at a time.
    strangeRunOver(run, `${stopped(strangeOpen().filter(letter => typeof split.panes[letter].stood === "number"))}: no world could play on`);
    return;
  }
  strangeSay(`${stopped(shorts)}, short of turn ${run.asked}. Time runs on for the rest`);
  strangeStep();
}

// ---- the futures
//
// Operator 2026-11-24: "instead of random lines, we can compute (a degree in
// advance) the futures of the paths we are on", "simulating 3 paths along
// their current strategy and we should have for example nuclear war visual
// forecasts out in the future (we will compute the future of each path).
// these should alert on the top section out in the distance."
//
// Each world is played ahead of the open turn along its own player strategy,
// a few turns at a time, by the engine's `ahead`: the world plays on from
// the last turn it holds and comes back to the open one, in one request, so
// nothing else ever finds it away. The turns are kept ahead of the open one
// like any turn played and walked back from, so they are drawn as the line
// they are, and time running on is shown them instead of playing them
// again. What the engine saw coming on the way (a nuclear detonation, a
// war, the end of the game) is kept with the world and put up on its line
// at the turn it happens on.
//
// Operator 2026-11-24, later: "Right now Doctor Strange's Civ 6 goes 4 turns
// and pauses. This should not happen. It should be a continuous effort. and
// blazingly fast." Time used to stop every fourth turn while each world in
// turn was played four turns on (0.4 s at turn 10 and 1.2 s by turn 90, two
// worlds, measured on the published page), and every turn between was held
// on the screen for a sixth of a second. Now every world is asked at once
// and time does not wait for the answer: a world holds the next turns
// already, so it is shown them while its future is played on behind them.
// Each turn shown, a world is asked for the turn that showing it took from
// what it holds, so what it holds stays `STRANGE_AHEAD` turns deep and time
// runs at the pace the worlds are played at. The engine keeps the agents
// that played a world's last turn ahead to play its next (`debug_ahead`).

// Whether a world has no more turns to be played ahead: it was asked for
// some and played fewer, and holds the same line under the same strategy.
function strangeSpent(letter) {
  const pane = debugSplit.panes[letter];
  return !!pane.spent && pane.spent.seat === pane.strategy && pane.spent.last === strangeKept(letter);
}
// How many turns of its future a world standing on `turn` is asked for now,
// `most` at most: none while it has yet to answer the last time it was
// asked, has none left to play, or holds `STRANGE_AHEAD` of them already.
function strangeAheadOwed(letter, turn, most) {
  const pane = debugSplit.panes[letter];
  if (strange.peeks[letter] || turn === null || turn === undefined || pane.born || strangeSpent(letter)) return 0;
  const kept = Math.max(strangeKept(letter), turn), limit = strangeLimit() ?? Infinity;
  return Math.max(0, Math.min(most, STRANGE_AHEAD - (kept - turn), limit - kept));
}
// Every world that holds fewer than `STRANGE_AHEAD` turns ahead of where it
// stands is asked for more, all of them at once and each for its own: a
// world answers when it has played them and is then asked again, until it
// holds them all. Nothing is asked while the worlds are being moved, except
// between the turns of time running (`between`), where a world is asked for
// a turn or two at a time so that the next turn is not long in coming.
function strangeAheadNext(between = false) {
  const split = debugSplit;
  if (!split || !split.shown || split.busy || split.running > 0 || strange.lockWanted || !strange.futures) return;
  if (strange.run && !between) return;
  for (const letter of strangeOpen()) {
    // A world playing a path Autosearch found is asked for nothing else.
    if (strange.planning && strange.planning.letter === letter) continue;
    const turns = strangeAheadOwed(letter, split.panes[letter].turn, between ? STRANGE_AHEAD_RUNNING : STRANGE_AHEAD_CHUNK);
    if (turns === 0) continue;
    strange.peeks[letter] = {turns, since: performance.now()};
    debugSplitTell(letter, {civvis: "pane-ahead", turns});
  }
}
// A world that was asked for its future and never answered is not waited on
// for ever: time that was waiting on it runs on, and it is asked again.
function strangeAheadWatch() {
  const now = performance.now();
  const lost = Object.keys(strange.peeks).filter(letter =>
    !strange.order.includes(letter) || now - strange.peeks[letter].since >= STRANGE_AHEAD_WAIT_MS);
  if (lost.length === 0) return;
  for (const letter of lost) delete strange.peeks[letter];
  if (strange.run && strange.run.asked === null) strangeAsk(strange.run);
  else strangeAheadNext();
}
// A world has played ahead (`pane-ahead-done`, from the split's listener):
// what it saw coming is kept from the turn it played on from, and how it
// stood on each turn. Time that was waiting on it runs on. An answer nobody
// is waiting for is from a world that has since been deleted, or one that
// took too long, and says nothing of the world that is there now.
function strangeAheadDone(letter, data) {
  const split = debugSplit, pane = split ? split.panes[letter] : null, peek = strange.peeks[letter];
  if (!peek) return;
  delete strange.peeks[letter];
  if (pane) {
    const readings = Array.isArray(data.readings) ? data.readings : [];
    const from = typeof data.from === "number" ? data.from : pane.turn ?? 0;
    pane.alerts = [...(pane.alerts || []).filter(alert => alert.turn < from),
      ...(Array.isArray(data.alerts) ? data.alerts : []).filter(alert => alert && typeof alert.turn === "number")];
    pane.ahead = pane.ahead || new Map();
    for (const turn of [...pane.ahead.keys()]) if (turn > from) pane.ahead.delete(turn);
    for (const reading of readings) if (reading && typeof reading.turn === "number") pane.ahead.set(reading.turn, reading);
    if (typeof data.last === "number") pane.last = Math.max(pane.last ?? 0, data.last);
    pane.spent = data.error || readings.length < peek.turns ? {seat: pane.strategy, last: strangeKept(letter)} : null;
  }
  if (strange.run && strange.run.asked === null) strangeAsk(strange.run);
  // While Send it runs time on, a world that has answered is asked for the
  // next of its turns at once: its engine plays on the whole time, and the
  // turns shown catch up with it as often as the page can draw them.
  else if (strange.run && strange.run.fast && !strange.run.stop) strangeAheadNext(true);
  else strangeAheadNext();
  strangeDraw();
}

// ---- the whole of a world's future
//
// Operator 2026-11-24: "each branch in doctor strange's civ 6 should simulate
// out to the end of the branch. we can multithread this across the processor
// to help. The top of each branch should have either: no nuclear war in the
// next N turns, no nuclear war, or nuclear war in N turns (Era). we should
// note wars as well. we should optimize the simulation speed so we simulate
// out pretty quickly. Can build out the most likely paths at all time."
//
// The turns a world holds ahead (`strangeAheadNext`) are a dozen, each kept
// whole to be shown the moment time comes to it. Past them the world's
// future is played to the end of its game by the engine's `foresee`: a copy
// of the world at the end of its line, with the agents that play on from
// there, played on a stretch a request and keeping nothing but what it saw
// coming. That is the future this world goes on to play: the engine is
// deterministic, and each turn foreseen carries the print the world's own
// turn will carry (`strangeSightStale`).
//
// Where the engine is the browser's, each world's future is played on an
// engine of its own, forked from the world's (`beta/shim.js`, "a world's
// scout"): the worlds and their futures are all played at once on as many
// cores, and a world's turn never waits behind its future. A game served by
// this computer plays a world's future on the world itself, each world
// behind a lock of its own, so there the futures of all the worlds are
// played at once while the worlds stand still.
//
// It is always being built: a world is asked the moment it is open, asked
// again each time it answers until its game has ended, and asked anew when
// its line has been played another way (another player strategy, a black
// swan, a branch). What was seen is put up on the world's line at the turn
// it happens on, and summed up on the sign over the far end of its branch
// (`strangeFate`).

// Whether a world's future has an engine of its own to be played on.
function strangeScouts() {
  return !!window.__civvisBeta && typeof window.__civvisBeta.scouts === "number";
}
// What is seen of a world's future is let go of: it is another world now.
// `gone` is a world that is closed, whose engine is let go of with it.
function strangeSightDrop(letter, gone = false) {
  const pane = debugSplit ? debugSplit.panes[letter] : null;
  delete strange.seeing[letter];
  if (!pane) return;
  pane.sight = null;
  pane.sightAfter = 0; pane.sightTries = 0; pane.sightBegan = 0;
  if (gone && strangeScouts())
    fetchJSON(`/debug?pane=${encodeURIComponent(pane.id)}&scout=done`, {method: "POST", headers: {"Content-Type": "application/json"},
      body: JSON.stringify({op: "status", lean: true})}).catch(() => {});
}
// Whether what was seen is no longer this world's future: another player
// strategy holds the seat, or a turn the world holds on its line is another
// world than the one foreseen on that turn (the engine's print of each,
// `debug.worlds`). The world's page says what it holds a moment after the
// world is given another strategy, so the turns are held against what was
// seen only when they are the line of the Civ AI it was seen for, as the
// engine names both. Worked out again only when the world has said anew
// what it holds.
function strangeSightStale(letter) {
  const pane = debugSplit.panes[letter], sight = pane.sight;
  if (!sight) return false;
  if (sight.asked !== (pane.strategy || null)) return true;
  const marks = pane.marks || null;
  if (sight.read === marks) return sight.stale;
  sight.read = marks;
  sight.stale = false;
  if (marks && (marks.seat ?? null) === sight.seat && Array.isArray(marks.turns) && Array.isArray(marks.worlds) &&
      marks.turns.length === marks.worlds.length)
    sight.stale = marks.turns.some((turn, at) => sight.prints.has(turn) && sight.prints.get(turn) !== marks.worlds[at]);
  return sight.stale;
}
// Every world whose future has not been played to its end is asked for more
// of it, all of them at once and as many as there are cores to spare, the
// followed world first. Where each has an engine for it those are the cores
// the worlds and the page leave. Without one a world plays its own future:
// a game served by this computer holds each world behind a lock of its own
// (`panes.rs`), so the worlds' futures are still played side by side, but a
// world asked for its future is not free to play a turn, and so they are
// asked only while the worlds stand still with nothing else asked of them.
function strangeForesee() {
  const split = debugSplit;
  if (!split || !split.shown || !strange.futures || !strange.hud) return;
  // While Autosearch runs it has the cores it was given, and no future is
  // begun beside it: one on its way plays its stretch out, and the rest
  // carry on when the search is over.
  if (debugSearch && debugSearch.live) return;
  // While Send it runs time on, no future is begun either: the worlds are
  // playing their futures themselves, each on its engine, and are only as
  // fast as the slowest of them, which a scout on a core beside it slows.
  // The futures carry on once time stops, from wherever the worlds are.
  if (strange.run && strange.run.fast) return;
  const open = strangeOpen(), own = strangeScouts(), now = performance.now();
  if (!own && (!strangeIdle() || Object.keys(strange.peeks).length > 0)) return;
  const most = Math.max(1, (navigator.hardwareConcurrency || 4) - (own ? open.length : 0) - 1);
  // A future on an engine of its own is played on a copy of its world's
  // engine, its scout, from the time it is begun until it is played to its
  // end (`strangeSeen` lets it go then): no more are begun than the tab has
  // room for (`strangeBudget`), and the rest wait for one to end.
  const copies = own ? strangeBudget().futures : Infinity;
  let copied = open.filter(name => !!strange.seeing[name] || (!!split.panes[name].sight && !split.panes[name].sight.done)).length;
  let out = Object.keys(strange.seeing).length;
  for (const letter of [strange.follow, ...open.filter(name => name !== strange.follow)]) {
    if (out >= most) break;
    const pane = split.panes[letter];
    if (!pane || !open.includes(letter) || pane.born || pane.turn === null || pane.turn === undefined || strange.seeing[letter]) continue;
    if (strange.planning && strange.planning.letter === letter) continue;
    // A world that has not said who holds its seat is not asked yet: its
    // future is that player strategy's.
    if (pane.strategy === undefined) continue;
    // A future that is no longer the world's is begun again, and not more
    // than once in a while: a world that is still changing is left to.
    if (pane.sight && strangeSightStale(letter)) {
      // Its scout is a copy of the world as it was, and is made anew.
      if (!pane.sight.done) copied--;
      pane.sight = null; pane.sightAfter = Math.max(pane.sightAfter || 0, (pane.sightBegan || 0) + STRANGE_SEE_ANEW_MS);
    }
    if ((pane.sight && pane.sight.done) || now < (pane.sightAfter || 0)) continue;
    if (!pane.sight) {
      if (copied >= copies) continue;
      copied++;
      pane.sightBegan = now;
    }
    strangeSee(letter);
    out++;
  }
}
// One stretch of a world's future: as many turns as the last took the time
// of `STRANGE_SEE_MS` to play, so that the page hears how it is going a few
// times a second however long a turn has become.
async function strangeSee(letter) {
  const split = debugSplit, pane = split.panes[letter], sight = pane.sight || null;
  const token = ++strange.sights, asked = pane.strategy || null, since = performance.now();
  const turns = !sight ? STRANGE_SEE_FIRST
    : Math.max(1, Math.min(STRANGE_SEE_MOST, Math.round(STRANGE_SEE_MS / Math.max(1, sight.pace))));
  strange.seeing[letter] = {token, since};
  let told;
  try {
    told = await fetchJSON(`/debug?pane=${encodeURIComponent(pane.id)}&scout=${sight ? "1" : "new"}`,
      {method: "POST", headers: {"Content-Type": "application/json"},
        body: JSON.stringify({op: "foresee", turns, lean: true, ...(asked ? {strategy: asked} : {}),
          ...(sight ? {known: sight.turn} : {anew: true})})}, STRANGE_SEE_WAIT_MS);
  } catch (error) { told = {error: String(error.message || error)}; }
  // An answer nobody is waiting for is of a world that has since been let
  // go of, or played another way.
  const seeing = strange.seeing[letter];
  if (!seeing || seeing.token !== token) return;
  delete strange.seeing[letter];
  if (debugSplit !== split || split.panes[letter] !== pane) return;
  strangeSeen(letter, told, {asked, took: performance.now() - since});
  strangeForesee();
}
// What a world's future has shown so far is kept with the world: the print
// of each turn foreseen, what is coming and when, how far it has been played
// and whether that is the end of the game. An answer says only what is new
// since the turn the page said it knew of, unless it is of a future begun
// again (`anew`), which is told whole.
function strangeSeen(letter, told, how) {
  const pane = debugSplit.panes[letter], seen = told && !told.error ? told.foresight : null;
  if (!seen || typeof seen.turn !== "number" || typeof seen.from !== "number") {
    // Asked again in a while, and each time it fails in a longer one. What
    // was seen so far is kept: the engine goes on from where it was, or
    // says that it has begun again.
    pane.sightTries = (pane.sightTries || 0) + 1;
    pane.sightAfter = performance.now() + Math.min(60000, STRANGE_SEE_AGAIN_MS * 2 ** Math.min(5, pane.sightTries - 1));
    return;
  }
  pane.sightTries = 0; pane.sightAfter = 0;
  let sight = pane.sight;
  const whole = typeof seen.since !== "number";
  if (!sight || whole || seen.anew === true || sight.from !== seen.from || sight.base !== seen.base)
    sight = pane.sight = {from: seen.from, base: seen.base, asked: how.asked, seat: seen.seat ?? null, prints: new Map(), seen: [],
      began: performance.now(), pace: 20, read: undefined, stale: false};
  else sight.seen = sight.seen.filter(event => event.turn < seen.since);
  for (const pair of Array.isArray(seen.worlds) ? seen.worlds : [])
    if (Array.isArray(pair) && typeof pair[0] === "number") sight.prints.set(pair[0], pair[1]);
  sight.seen.push(...(Array.isArray(seen.seen) ? seen.seen : []).filter(event => event && typeof event.turn === "number"));
  Object.assign(sight, {turn: seen.turn, done: seen.done === true, limit: typeof seen.limit === "number" ? seen.limit : null,
    eras: Array.isArray(seen.eras) ? seen.eras : [], detonations: seen.detonations || 0, victory: seen.victory || null,
    // The world may have said what it holds while this was out.
    read: undefined});
  if (seen.played > 0) sight.pace = how.took / seen.played;
  if (sight.done) {
    sight.took = performance.now() - sight.began;
    // The game has ended in it: its engine has nothing more to play.
    if (strangeScouts())
      fetchJSON(`/debug?pane=${encodeURIComponent(pane.id)}&scout=done`, {method: "POST", headers: {"Content-Type": "application/json"},
        body: JSON.stringify({op: "status", lean: true})}).catch(() => {});
  }
  strangeDraw();
}
// What the sign over a world's branch says (operator 2026-11-24: "the top of
// each branch should have either: no nuclear war in the next N turns, no
// nuclear war, or nuclear war in N turns (Era). we should note wars as
// well"). Turns are counted from the world's open turn, so the sign counts
// down as time runs:
//   - a nuclear war is coming: in how many turns, and the world era it
//     comes in; one that has begun says since when;
//   - none is, in a future played to the end of the game: no nuclear war;
//   - none is as far as the future has been played yet: none in the next N
//     turns, and N grows as it is played on.
// Under it the next war to be declared, and how many more there are after
// it. Over it, "On current path," (operator 2026-11-25: "add to message: on
// current path, no nuclear war..."): it is the future of the world's line as
// it stands, under the player strategies it has now. `state` is which of
// those it is (`coming`, `begun`, `clear`, `so-far`, `none`), `path`, `head`
// and `wars` the three lines (`path` empty while there is nothing seen yet),
// `line` and `word` the first said in less room, `seen` how much of the game
// the future has been played through, and `title` all of it.
function strangeFate(letter) {
  const pane = debugSplit.panes[letter], sight = pane.sight, turn = pane.turn;
  const turns = count => `${count} turn${count === 1 ? "" : "s"}`;
  const none = (head, word, title) => ({state: "none", path: "", head, line: head, word, wars: "", seen: 0, title});
  if (!strange.futures && !sight) return none("Futures off", "off", "Futures are off: this world's future is not being played out");
  if (!sight || typeof turn !== "number" || typeof sight.turn !== "number")
    return none("Looking ahead…", "…", "This world's future is being played out to the end of its game");
  const era = seen => typeof seen.era === "number" || seen.era_name ? debugEraName({era: seen.era, name: seen.era_name}) : "";
  const reach = sight.turn - turn, limit = sight.limit ?? strangeLimit();
  const first = sight.seen.find(seen => seen.kind === "nuclear") || null;
  const end = sight.done ? [...sight.seen].reverse().find(seen => seen.kind === "won" || seen.kind === "over") || null : null;
  let state, head, line, word;
  if (first && first.turn < turn) {
    state = "begun";
    head = `☢ Nuclear war since turn ${first.turn}${era(first) ? ` (${era(first)} Era)` : ""}`;
    line = `☢ Since turn ${first.turn}`; word = `☢ t${first.turn}`;
  } else if (first) {
    const away = first.turn - turn;
    state = "coming";
    head = `☢ Nuclear war ${away === 0 ? "this turn" : `in ${turns(away)}`}${era(first) ? ` (${era(first)} Era)` : ""}`;
    line = away === 0 ? "☢ Nuclear war this turn" : `☢ In ${turns(away)}${era(first) ? ` · ${era(first)}` : ""}`;
    word = `☢ ${away}t`;
  } else if (sight.done) {
    state = "clear"; head = line = "☮ No nuclear war"; word = "☮ None";
  } else if (reach > 0) {
    state = "so-far"; head = `No nuclear war in the next ${turns(reach)}`; line = `None in next ${turns(reach)}`; word = `✓ ${reach}t`;
  } else return none("Looking ahead…", "…", "This world's future is being played out to the end of its game");
  // The wars still to be declared, the next first.
  const coming = sight.seen.filter(seen => seen.kind === "war" && seen.turn >= turn);
  const short = text => String(text || "").replace(" declares war on ", " on ");
  const over = !end ? "" : `${end.kind === "won" ? "★" : "🏁"} ${end.text}, turn ${end.turn}`;
  // Said in fewer words on the sign, where the line is short.
  const brief = over.replace(" wins the game,", " wins,").replace(" is out of the game,", " is out,");
  const wars = coming.length > 0
    ? `⚔ War ${coming[0].turn === turn ? "this turn" : `in ${turns(coming[0].turn - turn)}`}: ${short(coming[0].text)}${coming.length > 1 ? ` · ${coming.length - 1} more` : ""}`
    : sight.done ? `No wars ahead${brief ? ` · ${brief}` : ""}` : reach > 0 ? `No war in the next ${turns(reach)}` : "";
  const last = sight.done ? sight.turn : limit;
  const share = sight.done ? 1 : last && last > sight.from ? Math.max(0, Math.min(1, (sight.turn - sight.from) / (last - sight.from))) : 0;
  const pace = sight.pace > 0 ? ` at ${(1000 / sight.pace).toFixed(sight.pace > 100 ? 1 : 0)} turns a second` : "";
  const path = "On current path,";
  const said = head.replace(/^[☢☮] /, "");
  const title = [`World ${letter.toUpperCase()}: ${path.toLowerCase()} ${said.charAt(0).toLowerCase()}${said.slice(1)}`,
    ...sight.seen.filter(seen => seen.kind === "nuclear" && seen.turn >= turn).slice(0, 6).map(seen => `☢ Turn ${seen.turn}: ${seen.text}`),
    ...(coming.length > 0 ? coming.slice(0, 8).map(seen => `⚔ Turn ${seen.turn}: ${seen.text}`)
      : [sight.done ? "No wars ahead" : wars]),
    ...(coming.length > 8 ? [`⚔ and ${coming.length - 8} more wars`] : []),
    ...(over ? [`${over}${sight.victory ? ` (${titleCase(String(sight.victory))})` : ""}`] : []),
    sight.done ? `Its future is played to the end of the game, turn ${sight.turn}${typeof sight.took === "number" ? `, in ${(sight.took / 1000).toFixed(1)} s` : ""}`
      : `Its future is played to turn ${sight.turn}${limit ? ` of ${limit}` : ""}${pace}, and is being played on`]
    .filter(Boolean).join("\n");
  return {state, path, head, line, word, wars, seen: share, title};
}
// How near a world is to defeat, read from the same future as its sign: the
// turns until a nuclear war and until the game is lost (another civilization
// wins it or the seat is out of it). `fill` is how near, against the turns
// the game has left: full when it is here, empty when none is coming.
function strangeDefeat(letter) {
  const pane = debugSplit.panes[letter], sight = pane.sight, turn = pane.turn;
  const turns = count => `${count} turn${count === 1 ? "" : "s"}`;
  if (!strange.futures && !sight) {
    const off = {value: "Off", fill: 0, title: "Futures are off: this world's future is not being played out"};
    return {nuclear: off, losing: off};
  }
  if (!sight || typeof turn !== "number" || typeof sight.turn !== "number") {
    const looking = {value: "…", fill: 0, title: "This world's future is being played out to the end of its game"};
    return {nuclear: looking, losing: looking};
  }
  const limit = sight.limit ?? strangeLimit(), left = limit ? Math.max(1, limit - turn) : null;
  const near = away => left ? Math.max(0, Math.min(1, 1 - away / left)) : 0;
  const reach = Math.max(0, sight.turn - turn);
  const first = sight.seen.find(seen => seen.kind === "nuclear") || null;
  const end = sight.done ? [...sight.seen].reverse().find(seen => seen.kind === "won" || seen.kind === "over") || null : null;
  const nuclear = first && first.turn < turn
    ? {value: "Now", fill: 1, title: `A nuclear war since turn ${first.turn}`}
    : first ? {value: `${first.turn - turn}t`, fill: near(first.turn - turn),
      title: `A nuclear war ${first.turn === turn ? "this turn" : `in ${turns(first.turn - turn)}`}, turn ${first.turn}: ${first.text}`}
    : sight.done ? {value: "None", fill: 0, title: "No nuclear war before the game ends"}
    : {value: `>${reach}t`, fill: 0, title: `No nuclear war in the next ${turns(reach)}, as far as its future is played`};
  const lost = end && end.kind === "over" && end.text !== "The game ends";
  const losing = lost ? {value: `${end.turn - turn}t`, fill: near(end.turn - turn),
      title: `Lost ${end.turn === turn ? "this turn" : `in ${turns(end.turn - turn)}`}: ${end.text}, turn ${end.turn}`}
    : end && end.kind === "won" ? {value: "Wins", fill: 0, title: `Not lost: ${end.text}, turn ${end.turn}`}
    : end ? {value: "None", fill: 0, title: `Not lost: ${end.text}, turn ${end.turn}`}
    : {value: `>${reach}t`, fill: 0, title: `Not lost in the next ${turns(reach)}, as far as its future is played`};
  return {nuclear, losing};
}
// A world's predicted path, from the same future as its sign: what it
// comes to, and where it stands among the others. Best first: the seat
// wins (sooner is better); no nuclear war and nothing lost (the higher
// score is better); a nuclear war coming but the game not lost (later is
// better); the game lost (later is better); a future not yet seen.
function strangeOutcome(letter) {
  const pane = debugSplit.panes[letter], sight = pane.sight, turn = pane.turn;
  const score = Number((pane.numbers || {}).score) || 0;
  const fate = strangeFate(letter);
  if (!sight || typeof turn !== "number" || typeof sight.turn !== "number")
    return {rank: [4, 0], outcome: "unknown", head: fate.head, wars: "", title: fate.title};
  const turns = count => `${count} turn${count === 1 ? "" : "s"}`;
  const first = sight.seen.find(seen => seen.kind === "nuclear") || null;
  const end = sight.done ? [...sight.seen].reverse().find(seen => seen.kind === "won" || seen.kind === "over") || null : null;
  const lost = !!end && end.kind === "over" && end.text !== "The game ends";
  const won = !!end && end.kind === "won";
  const reach = Math.max(0, sight.turn - turn);
  const rank = won ? [0, end.turn] : lost ? [3, -end.turn] : first ? [2, -first.turn] : [1, -score];
  const said = end ? String(end.text).replace(" wins the game", " wins").replace(" is out of the game", " is out") : "";
  const head = won ? `★ ${said}, turn ${end.turn}` : lost ? `✗ ${said}, turn ${end.turn}`
    : end ? `🏁 ${said}, turn ${end.turn}` : `Not decided in the next ${turns(reach)}`;
  const coming = sight.seen.filter(seen => seen.kind === "war" && seen.turn >= turn);
  const wars = coming.length > 0
    ? `⚔ War ${coming[0].turn === turn ? "this turn" : `in ${turns(coming[0].turn - turn)}`}: ${String(coming[0].text || "").replace(" declares war on ", " on ")}${coming.length > 1 ? ` · ${coming.length - 1} more` : ""}`
    : sight.done ? "No wars ahead" : `No war in the next ${turns(reach)}`;
  return {rank, outcome: won ? "won" : lost ? "lost" : first ? "nuclear" : "clear", head, nuclear: fate.head, wars, title: fate.title};
}
// A world's end outcome, as the paths table's last row says it (operator
// 2026-11-27: "make another row that is end outcome: and list the XXX wins,
// turnXXX, no wars, no nuclear wars"): who wins and on what turn, then the
// wars and the nuclear wars still to come on the way there, from the future
// played out to the end of its game. A future still being played says how
// far it has got, and what it has seen so far. It ranks as `strangeOutcome`
// does, which was the order of the predicted paths it replaces.
function strangeEndOutcome(letter) {
  const pane = debugSplit.panes[letter], sight = pane.sight, turn = pane.turn;
  const told = strangeOutcome(letter);
  if (!sight || typeof turn !== "number" || typeof sight.turn !== "number") return {...told, text: "Looking ahead…"};
  const end = sight.done ? [...sight.seen].reverse().find(seen => seen.kind === "won" || seen.kind === "over") || null : null;
  const first = sight.seen.find(seen => seen.kind === "nuclear") || null;
  const wars = sight.seen.filter(seen => seen.kind === "war" && seen.turn >= turn).length;
  const yet = sight.done ? "" : " so far";
  const who = !end ? `Not decided by turn ${sight.turn}`
    : end.text === "The game ends" ? "No winner"
    : String(end.text).replace(" wins the game", " wins").replace(" is out of the game", " is out");
  const text = [who, ...(end ? [`turn ${end.turn}`] : []),
    wars ? `${wars} war${wars === 1 ? "" : "s"}${yet}` : `no wars${yet}`,
    !first ? `no nuclear wars${yet}` : first.turn < turn ? `nuclear war since turn ${first.turn}` : `nuclear war turn ${first.turn}`].join(", ");
  return {...told, text};
}
// The standings and the victory tracker of the world followed (operator
// 2026-11-27: "i can't actually tell what civ we are. please add the player
// hud in for the "Following" game"). They are the simulator's own two
// panels, which the highway covers where they stand: they are moved onto it
// (`strangeStandingsMount`), and the simulator draws them as it always
// does, reading the world followed in place of its own (`drawPlayerHud`,
// wrapped where this page is made). Each world hands over the world it holds
// (`strangeWorldNow`, said in each world's page), and they are drawn again
// when that world has another turn to show or another world is followed.
// The panels are kept here, so that a highway made again takes them up, and
// a highway gone hands them back to the map they came from.
let strangePanels = null;
function strangeStandingsMount(hud) {
  const players = strangePanels ? strangePanels.players : document.getElementById("playerhud");
  const victory = strangePanels ? strangePanels.victory : document.getElementById("victoryhud");
  strange.standings = typeof drawPlayerHud === "function" && !!players && !!victory;
  strange.standingsWorld = null;
  if (!strange.standings) return;
  strangePanels = {players, victory};
  hud.querySelector(".strange-standings").append(players);
  hud.querySelector(".strange-victory").append(victory);
}
function strangeStandingsHome() {
  const area = document.getElementById("maparea");
  if (!strangePanels || !area) return;
  for (const panel of [strangePanels.players, strangePanels.victory]) if (!panel.isConnected) area.prepend(panel);
}
function strangeFollowedWorld() {
  try {
    const pane = debugSplit && debugSplit.panes[strange.follow], frame = pane && pane.frame;
    const world = frame && frame.contentWindow && typeof frame.contentWindow.strangeWorldNow === "function"
      ? frame.contentWindow.strangeWorldNow() : null;
    return world && Array.isArray(world.players) ? world : null;
  } catch (_) { return null; }
}
function strangeStandingsDraw() {
  if (!strange.standings) return;
  const world = strangeFollowedWorld();
  if (!world || world === strange.standingsWorld) return;
  strange.standingsWorld = world;
  try { drawPlayerHud(); } catch (error) { console.error("doctor strange standings", error); }
}
// The defeat tracker: the victory tracker's format, with a path for a
// nuclear war and one for losing, each counting the turns away.
const STRANGE_DEFEAT_PATHS = [["nuclear", "Nuclear war", "#ff8a3d"], ["losing", "Losing", "#e2574c"]];
function strangeDefeatDraw(hud, open, tree) {
  const card = hud.querySelector("[data-strange-defeat]");
  if (!card) return;
  const read = Object.fromEntries(open.map(letter => [letter, strangeDefeat(letter)]));
  const html = `<div class="hud-section-heading"><span>Defeat tracker</span></div>` +
    STRANGE_DEFEAT_PATHS.map(([key, label, colour]) =>
      `<section class="victory-track" data-defeat="${key}" style="--track:${colour}" aria-label="${label}: turns away, each world">` +
      `<div class="victory-heading"><b>${label}</b><div class="victory-subheading"><span>Turns away</span></div></div>` +
      `<div class="victory-body"><div class="victory-list">` + open.map(letter => {
        const at = read[letter][key], name = `${letter.toUpperCase()} · ${strangeSeries(letter, tree)}`;
        return `<div class="victory-entry" data-defeat-world="${letter}" style="--civ:${strange.tint[letter]}" title="${escapeAttr(`${name}: ${at.title}`)}">` +
          `<div class="victory-line"><span class="victory-civ-dot"></span><span class="victory-civ">${escapeAttr(name)}</span>` +
          `<span class="victory-value">${escapeAttr(at.value)}</span></div>` +
          `<div class="victory-meter"><b style="width:${Math.round(at.fill * 100)}%"></b></div></div>`;
      }).join("") + `</div></div></section>`).join("");
  if (card.dataset.drawn !== html) {
    const grew = card.dataset.rows !== String(open.length);
    card.dataset.drawn = html;
    card.dataset.rows = String(open.length);
    card.innerHTML = html;
    if (grew) strange.stale = true;
  }
}
// Where the signs stand: each over the far end of its own branch, in rows
// between the cards at the top, as much of each as there is room for. Signs
// that would stand on one another are moved apart along their row, and
// where a row has not the room for all of them they take turns at two.
function strangeFates(g, open) {
  const follow = open.includes(strange.follow) ? strange.follow : open[0];
  const [title, meter] = strange.cards;
  const least = title ? title.right + 8 : 8, most = Math.max(least + 120, meter ? meter.left - 8 : g.W - 8);
  const room = most - least, gap = STRANGE_FATE_GAP;
  const fits = name => Math.max(1, Math.floor((room + gap) / (Math.min(STRANGE_FATES[name][0], room) + gap)));
  const mode = ["full", "line"].find(name => Math.ceil(open.length / fits(name)) <= 2) || "word";
  const wide = Math.min(STRANGE_FATES[mode][0], room), tall = STRANGE_FATES[mode][1];
  const rows = Math.ceil(open.length / fits(mode));
  const wanted = open.map(letter => ({letter, at: g.cx + (open.indexOf(letter) - open.indexOf(follow)) * g.far}))
    .sort((one, other) => one.at - other.at);
  const put = [];
  for (let row = 0; row < rows; row++) {
    const held = wanted.filter((_, at) => at % rows === row);
    const lefts = held.map(sign => Math.max(least, Math.min(most - wide, sign.at - wide / 2)));
    for (let at = 1; at < lefts.length; at++) lefts[at] = Math.max(lefts[at], lefts[at - 1] + wide + gap);
    for (let at = lefts.length - 1; at >= 0; at--)
      lefts[at] = Math.min(lefts[at], at === lefts.length - 1 ? most - wide : lefts[at + 1] - wide - gap);
    held.forEach((sign, at) => put.push({letter: sign.letter, left: lefts[at], top: STRANGE_FATE_TOP + row * (tall + gap), wide, tall}));
  }
  return {mode, put};
}

// Time runs on from the open turn, each world with its own player strategy,
// or stops.
// `fast` is Send it's: time shows the furthest turn every world holds
// (`strangeAsk`), not each turn on the way.
function strangePlay(until = null, fast = false) {
  const split = debugSplit;
  if (!split || split.busy || !split.shown) return;
  // Paused, the worlds stop on the same turn: the one they were asked for,
  // or the one they stand on while time waits on a world's future.
  if (strange.run) {
    strange.run.stop = true;
    if (strange.run.asked === null) strangeRunOver(strange.run); else strangeStep();
    strangeDraw();
    return;
  }
  if (split.running > 0) {
    for (const letter of strangeOpen()) debugSplitTell(letter, {civvis: "pane-stop"});
    split.running = 0;
    split.until = null;
    strangeDraw();
    return;
  }
  if (!strangeIdle()) return;
  const reached = strangeNow() ?? 0;
  const end = until ?? strangeLimit() ?? reached + 250;
  if (end <= reached) return;
  strangeSay("");
  if (strange.locked) strangeTravel(end, true, fast);
  else debugSplitRun(end, strangeOpen());
  strangeDraw();
}
// Go to a turn: one that was played is gone back to as it stood, and one
// still ahead is played to. Locked, every world goes, together; unlocked,
// the one that is followed.
function strangeGoTo(turn) {
  if (!strangeIdle() || !Number.isFinite(turn)) return;
  const split = debugSplit, limit = strangeLimit();
  const first = Math.max(...strangeOpen().map(letter => split.panes[letter].first ?? 1));
  const to = Math.max(first, Math.min(limit ?? Infinity, Math.round(turn)));
  if (to === strangeNow()) return;
  strangeSay("");
  strangeTravel(to);
}
// ---- Turn 1, Era back and Era forward
//
// Operator 2026-11-27: "lets add era back and era forward to the controls.
// lets add a turn 1 option too. this should be a clean set of controls. lets
// try to put in order: turn 1, era back (which goes to the start of the era,
// then the last era start). we should make sure to save these turns for fast
// jumping. we have another thread working to cut out half the turns".
//
// The steps read, left to right: Turn 1, Era back, −10, −1, +1, +10, Era
// forward, and each takes every world, together, as the steps do. An era is
// the followed world's: its engine says the first turn of each world era it
// has been through (`marks.eras`, the turns played ahead included), and
// keeps that turn whatever else it lets go of to save room
// (`Timeline::thin`), as it keeps the first. Going back to one is a jump to a
// turn held, not turns played again.

// Era back: the first turn of the era the followed world stands in, or,
// standing on that, of the era before.
function strangeEraBack(turn = strangeNow()) {
  const eras = ((debugSplit.panes[strange.follow].marks || {}).eras || []).filter(era => era.turn < turn);
  return eras.length ? eras[eras.length - 1] : null;
}
// The first turn of the next era along the followed world's line, once it
// has played that far.
function strangeEraNext(turn = strangeNow()) {
  return ((debugSplit.panes[strange.follow].marks || {}).eras || []).find(era => era.turn > turn) || null;
}
// The era the followed world stands in.
function strangeEraNow(turn = strangeNow()) {
  const came = ((debugSplit.panes[strange.follow].marks || {}).eras || []).filter(era => era.turn <= turn).pop();
  return came ? came.era : typeof state.world_era === "number" ? state.world_era : 0;
}
// Era forward: on to the first turn of the next era, when the followed world
// has played it; otherwise time runs on, every world with its own player
// strategy, until the followed world comes into a new era (`strangeAsk`).
function strangeEraOn() {
  if (!strangeIdle()) return;
  const next = strangeEraNext();
  if (next) { strangeGoTo(next.turn); return; }
  const now = strangeNow() ?? 0, limit = strangeLimit();
  if (limit !== null && now >= limit) return;
  strangeSay("");
  strangeTravel(limit ?? now + 250, true);
  if (strange.run) strange.run.era = {letter: strange.follow, from: now};
  strangeDraw();
}
// Open or close the lock. Worlds on different turns are locked on the
// followed one's: the rest are brought to it first.
// ---- autosearch
//
// Operator 2026-11-25: "add the autosearch stuff to doctor strange's civ 6.
// we should be able to efficiently search paths to try to find a desirable
// path".
//
// A path is what a branch's name says: which player strategy held the seat
// from which turn. That is what the AI Debugger's Autosearch searches for
// (`debugSearch*`, and the engine's climb, `src/server/autosearch.rs`), so it
// is the same search here: its button is on the deck under Branch, its panel
// is the AI Debugger's, and it searches from the base timeline on its open
// turn (`debugSearchRoot`). Each core given to it plays paths on a copy of
// that world's own engine (`POST /forks` naming the world, `beta/shim.js`),
// so the worlds stand as they were while it runs, and no world's future is
// begun beside it (`strangeForesee`): the cores it was given are its own.
//
// The path it found is made a branch (`strangeSearchBranch`): a new world, a
// copy of the base timeline, plays the path from the turn the search began
// on to its last turn, as the copies played it, and says what it came to
// beside what the search read. It is named for the strategies that played
// it, and the sign over its far end says how the game goes on from there.
// Locked, the other worlds then play on to that turn too, each with its own
// player strategy, so all of them stand on one turn again with the path
// beside them, as a half locked to one that plays a found line does in the
// AI Debugger. It is not walked back to the turn it was branched on: a world
// walks forward only along turns its seated strategy played, so a path of
// several would be played again from its first handover.

// The panel opens beside the card its button is on, over the tree, unless
// it was dragged somewhere else.
function strangeSearchPlace(panel, bar) {
  if (debugWindowPlace[debugWindowKey(panel)]) { debugPlaceWindow(panel, bar, null); return; }
  const frame = bar.getBoundingClientRect(), card = strange.hud.querySelector(".strange-title").getBoundingClientRect();
  const left = Math.max(8, Math.min(card.right + 10, window.innerWidth - panel.offsetWidth - 8));
  panel.style.left = `${Math.round(left - frame.left)}px`;
  panel.style.top = `${Math.round(card.top - frame.top)}px`;
}
// A path as a branch's name says it (`strangeSeries`): MoonshotMaven-150-
// JackOfAllTrades is MoonshotMaven up to turn 149 and JackOfAllTrades from 150.
function strangeSearchPath(plan, last) {
  const stretches = debugPlanStretches(plan, last);
  return stretches.map((stretch, at) => at === 0 ? strangeHandle(stretch.name) : `${stretch.from}-${strangeHandle(stretch.name)}`).join("-");
}
// What the search found, made a branch: the button, and why it cannot be
// pressed when it cannot. The search was of one world on one turn, so that
// world has to be open and on that turn still.
function strangeSearchOffer(search, best, live) {
  const split = debugSplit, base = split ? split.panes[search.letter] : null, name = String(search.letter || "").toUpperCase();
  const gone = !base || base.id !== search.pane || !strangeOpen().includes(search.letter);
  const moved = !gone && base.turn !== search.from;
  const same = !gone && best.plan.length === 1 && best.plan[0][1] === base.strategy;
  const full = strangeOpen().length >= strangeBudget().most;
  const path = strangeSearchPath(best.plan, best.last);
  const off = live || !!search.playing || gone || moved || same || full || !strangeIdle();
  const note = gone ? `${name}, the world searched, is closed: search again to branch a path.`
    : moved ? `${name} is no longer on turn ${search.from}, where the search began: take the worlds back to turn ${search.from}, or search again.`
    : live ? "Stop the search to branch the best path so far."
    : search.said ? search.said
    : same ? `That is ${name}'s own path: nothing beat the strategy it seats.`
    : full ? `${strangeFullWords()}. Delete one to branch this path.`
    : `${path}, from turn ${search.from}.`;
  const title = `Branch it: a new world, a copy of ${name} on turn ${search.from}, plays ${path} to turn ${best.last}` +
    (strange.locked ? `, and the other worlds play on to turn ${best.last} beside it` : "");
  debugDrawn(document.getElementById("dbg-search-play"),
    `<button data-dbg-search="branch" title="${escapeAttr(title)}"${off ? " disabled" : ""}>⑂ Branch it</button>` +
    `<span class="dbg-search-status">${escapeAttr(note)}</span>`);
}
function strangeSearchBranch() {
  const search = debugSearch, best = search && search.report ? search.report.best : null;
  if (!search || search.live || search.playing || !best || !strangeIdle() || !debugSplit) return;
  const base = debugSplit.panes[search.letter];
  if (!base || base.id !== search.pane || !strangeOpen().includes(search.letter) || base.turn !== search.from) return;
  const first = search.objective.want[0];
  const plan = {from: search.from, print: search.print, want: first ? first.key : null,
    value: best.verdict && best.verdict.want ? best.verdict.want[0] : null,
    stretches: debugPlanStretches(best.plan, best.last)};
  if (!plan.stretches.length) return;
  const path = strangeSearchPath(best.plan, best.last);
  const letter = strangeBranch(plan.stretches[0].name, search.letter, plan);
  if (!letter) return;
  strange.planning = {letter, from: search.from, path, search};
  search.playing = letter;
  search.said = `World ${letter.toUpperCase()} is playing ${path} from turn ${search.from}…`;
  strangeSay(`World ${letter.toUpperCase()} is the path found from ${search.letter.toUpperCase()}: ${path}. Playing it from turn ${search.from}…`);
  drawDebugBar(false);
}
// The branch has played the path (`pane-planned`, from the split's
// listener), or stopped short and says why. Played, and locked, the other
// worlds are taken to the turn it ended on, as time is (`strangeAsk`), each
// playing on with its own player strategy; the branch is not asked to move.
function strangeSearchPlanned(letter, done) {
  const planning = strange.planning;
  if (!planning || planning.letter !== letter) return;
  strange.planning = null;
  const search = planning.search, split = debugSplit, pane = split ? split.panes[letter] : null;
  if (search === debugSearch) debugSearchPlayed(search, done);
  if (!pane) return;
  const name = letter.toUpperCase(), played = !!(done && done.ok), end = played && typeof done.turn === "number" ? done.turn : null;
  const behind = end === null ? [] : strangeOpen().filter(other => other !== letter && (split.panes[other].turn ?? end) < end);
  const said = played ? `World ${name} is the path found, ${planning.path}. ${done.note}`
    : `World ${name} did not play the path out: ${(done && done.note) || "its page did not say why"}`;
  if (!strange.locked || behind.length === 0) { strangeSay(said); return; }
  strangeSay(`${said}. The other worlds play on to turn ${end} beside it…`);
  // What is said when they are there is what the branch came to.
  strange.run = {end, at: planning.from, asked: null, stop: false, play: false, letters: behind, said};
  strangeAsk(strange.run);
}
// A branch whose page has gone is not waited on.
function strangeSearchWatch() {
  const planning = strange.planning;
  if (!planning || (debugSplit && debugSplit.panes[planning.letter] && strange.order.includes(planning.letter))) return;
  strangeSearchPlanned(planning.letter, {ok: false, note: "the world was closed"});
}

// ---- black swans
//
// Operator 2026-11-24: "add a 'Generate Black Swan' - this will be a random,
// unpredicted event that spawns and changes the game", "black swans on a
// civilizational timeline."
//
// The engine strikes one on a world as it stands and says what happened
// (`pane-swan`, the engine's `black_swan`). Which one it is and where it
// lands are drawn from a seed the page makes, unless one was picked. The
// followed world is struck first and says which it was; locked, the rest
// are then struck by that same one with the same seed, so worlds that are
// still one world stay one, and worlds that have parted meet the same event.
// Unlocked, only the followed world is struck, and parts from the rest
// there. A world that was struck holds nothing ahead of the open turn any
// more: its future is played again from the world it now is
// (`strangeAheadNext`), which is how a swan nothing foresaw changes what is
// seen coming.
function strangeSwan(kind = "", seed = Math.floor(Math.random() * 4294967296)) {
  if (!strangeIdle()) return;
  const first = strange.follow, rest = strange.locked ? strangeOpen().filter(letter => letter !== first) : [];
  strange.swan = {kind: kind || null, seed, first, waiting: [first], rest, struck: [], failed: [], since: performance.now()};
  strange.omen = null;
  debugSplitTell(first, {civvis: "pane-swan", kind: kind || null, seed});
  strangeSay("");
}
// A world says it was struck, or why it was not (`pane-swan-done`, from the
// split's listener).
function strangeSwanDone(letter, data) {
  const split = debugSplit, swanning = strange.swan, pane = split ? split.panes[letter] : null;
  if (!swanning || !swanning.waiting.includes(letter)) return;
  swanning.waiting = swanning.waiting.filter(name => name !== letter);
  const swan = pane && !data.error && data.swan && typeof data.swan.turn === "number" ? data.swan : null;
  if (swan) {
    swanning.struck.push({letter, swan});
    // What it held ahead was played in a world this did not happen in.
    pane.alerts = (pane.alerts || []).filter(alert => alert.turn < swan.turn);
    for (const turn of [...(pane.ahead || new Map()).keys()]) if (turn > swan.turn) pane.ahead.delete(turn);
    for (const turn of [...split.history[letter].keys()]) if (turn > swan.turn) split.history[letter].delete(turn);
    pane.last = swan.turn;
    pane.spent = null;
    strangeSightDrop(letter);
  } else swanning.failed.push({letter, error: data.error || "it could not be struck"});
  if (letter === swanning.first) {
    // The rest are struck by the one the first drew. If it drew none, none is.
    const rest = swan ? swanning.rest.filter(name => strange.order.includes(name) && split.panes[name]) : [];
    swanning.rest = [];
    if (rest.length > 0) {
      swanning.kind = swan.kind;
      swanning.waiting = rest;
      swanning.since = performance.now();
      for (const name of rest) debugSplitTell(name, {civvis: "pane-swan", kind: swan.kind, seed: swanning.seed});
    }
  }
  if (swanning.waiting.length === 0) strangeSwanOver(swanning);
  else strangeDraw();
}
function strangeSwanOver(swanning) {
  if (strange.swan !== swanning) return;
  strange.swan = null;
  strange.tree = null;
  const struck = swanning.struck, failed = swanning.failed;
  if (struck.length > 0) {
    strange.omen = {swan: struck[0].swan, letters: struck.map(held => held.letter), since: performance.now()};
    const spared = failed.length === 0 ? ""
      : `. ${failed.map(held => held.letter.toUpperCase()).join(", ")} ${failed.length === 1 ? "was" : "were"} spared: ${failed[0].error}`;
    strangeSay(`${STRANGE_SWAN} Black swan on turn ${struck[0].swan.turn}: ${struck[0].swan.name}${spared}`);
  } else strangeSay(`No black swan struck: ${failed.length > 0 ? failed[0].error : "no world answered"}`);
  // Each world's future is played again, from the world it now is.
  strangeAheadNext();
}
// A world that was asked and never answered is not waited on for ever, and
// what struck is news for only so long.
function strangeSwanWatch() {
  const swanning = strange.swan, now = performance.now();
  if (swanning && now - swanning.since >= STRANGE_SWAN_WAIT_MS) {
    for (const letter of swanning.waiting) swanning.failed.push({letter, error: "it did not answer"});
    swanning.waiting = [];
    strangeSwanOver(swanning);
  }
  if (strange.omen && now - strange.omen.since >= STRANGE_SWAN_MS) { strange.omen = null; strangeDraw(); }
}

// ---- what the page costs
//
// Operator 2026-11-24: "we should also monitor resource usage - maybe track
// memory used and cpu % actively on the screen, in the upper right". Every
// world is a page and a game of its own, so this is what scaling the
// windows up costs. The engine's host says what it can (`/machine-metrics`):
// a game served by this computer gives the computer's own load and memory
// in use, and one played by the browser gives how much of its time the
// engine has been at work and the memory it holds, to which the page's own
// is added where the browser tells it.
async function strangeMeasure() {
  if (!debugSplit || !strange.hud || !strange.hud.isConnected || strange.measuring || document.hidden) return;
  strange.measuring = true;
  let told = null;
  try { told = await fetchJSON("/machine-metrics", {}, 4000); } catch (_) {}
  strange.measuring = false;
  told = told && typeof told === "object" ? told : {};
  const now = performance.now(), was = strange.metered;
  const number = value => typeof value === "number" && Number.isFinite(value);
  const sample = {cpu: null, cpuOf: "", memory: null, memoryOf: "", bytes: false};
  if (number(told.cpu_percent)) { sample.cpu = told.cpu_percent; sample.cpuOf = "this computer, all of its cores"; }
  else if (number(told.engine_busy_ms)) {
    // Each world may play on an engine of its own (`beta/shim.js`), and the
    // time they have all spent is added up: a share of the computer's cores.
    const engines = number(told.engine_threads) && told.engine_threads > 1 ? told.engine_threads : 1;
    const cores = engines > 1 ? Math.max(engines, navigator.hardwareConcurrency || engines) : 1;
    if (was && was.busy !== null && now > was.at)
      sample.cpu = Math.max(0, Math.min(100, (told.engine_busy_ms - was.busy) / (now - was.at) * 100 / cores));
    sample.cpuOf = engines > 1 ? `the ${engines} engines, of this computer's ${cores} cores` : "the engine, of the one core it plays on";
  }
  const heap = performance.memory && number(performance.memory.usedJSHeapSize) ? performance.memory.usedJSHeapSize : null;
  if (number(told.memory_percent)) { sample.memory = told.memory_percent; sample.memoryOf = "this computer, of all it has"; }
  else if (number(told.engine_bytes)) {
    sample.memory = told.engine_bytes + (heap ?? 0); sample.bytes = true;
    const engines = number(told.engine_threads) && told.engine_threads > 1 ? `the ${told.engine_threads} engines` : "the engine";
    sample.memoryOf = heap === null ? engines : `${engines} and this page`;
  } else if (heap !== null) { sample.memory = heap; sample.bytes = true; sample.memoryOf = "this page"; }
  strange.metered = {at: now, busy: number(told.engine_busy_ms) ? told.engine_busy_ms : null};
  strange.meter.push(sample);
  if (strange.meter.length > STRANGE_METER_KEPT) strange.meter.shift();
  strangeMeterDraw();
}
function strangeBytes(bytes) {
  return bytes >= 1073741824 ? `${(bytes / 1073741824).toFixed(2)} GB` : `${Math.round(bytes / 1048576)} MB`;
}
function strangeMeterDraw() {
  const hud = strange.hud, now = strange.meter[strange.meter.length - 1];
  if (!hud || !hud.isConnected || !now) return;
  for (const [key, of, tint] of [["cpu", "cpuOf", "#9ad0ff"], ["memory", "memoryOf", "#8fe07a"]]) {
    const gauge = hud.querySelector(`[data-strange-gauge="${key}"]`), value = now[key];
    const text = value === null ? "–" : key === "memory" && now.bytes ? strangeBytes(value) : `${Math.round(value)}%`;
    if (gauge.textContent !== text) gauge.textContent = text;
    gauge.title = now[of] ? `${key === "cpu" ? "Processor" : "Memory"} in use: ${now[of]}` : "Not told";
    // The readings kept, oldest first: a share of a hundred, or of the most seen.
    const canvas = hud.querySelector(`[data-strange-trace="${key}"]`), pen = canvas.getContext("2d");
    const values = strange.meter.map(sample => sample[key]);
    const top = key === "memory" && now.bytes ? Math.max(1, ...values.filter(seen => seen !== null)) * 1.15 : 100;
    pen.clearRect(0, 0, canvas.width, canvas.height);
    pen.strokeStyle = tint; pen.lineWidth = 2; pen.lineJoin = "round";
    pen.beginPath();
    let drawn = 0;
    values.forEach((seen, at) => {
      if (seen === null) return;
      const px = canvas.width - 2 - (values.length - 1 - at) * (canvas.width - 4) / (STRANGE_METER_KEPT - 1);
      const py = canvas.height - 2 - Math.min(1, seen / top) * (canvas.height - 4);
      if (drawn++ === 0) pen.moveTo(px, py); else pen.lineTo(px, py);
    });
    pen.stroke();
  }
}

// The highway is made when the split is: the sky behind the worlds, and
// over them what is written on it and the hand that moves it.
function strangeMount() {
  const split = debugSplit;
  if (!split || split.host.querySelector("#strange-sky")) return;
  if (!document.getElementById("strange-style")) {
    const style = document.createElement("style");
    style.id = "strange-style";
    style.textContent = STRANGE_STYLE;
    document.head.appendChild(style);
  }
  // The page opens on the best player strategy and the one to beat it with
  // (operator 2026-11-24: "we should default open doctor strange civ 6 with
  // our top strategy on the right and the challenger on the left. you can
  // try to piece together a strategy on the left to beat the number 1 on the
  // right"): the top rated is B, on the right, the next is A, on the left,
  // and A is the one followed, so a branch is made from the challenger.
  // Where the picker shows no ratings A keeps the page's own choice and B is
  // given another, or the two would be one world.
  const ranked = strangeRanked();
  if (ranked.rated && ranked.names.length > 1) { split.panes.b.strategy = ranked.names[0]; split.panes.a.strategy = ranked.names[1]; }
  else split.panes.b.strategy = ranked.names.find(name => name !== split.panes.a.strategy) || split.panes.a.strategy;
  Object.assign(strange, {follow: "a", cam: null, lanes: {a: 0, b: 1}, order: ["a", "b"], born: {a: 0, b: 1}, births: 2, made: {},
    tint: {a: STRANGE_INKS[0], b: STRANGE_INKS[1]}, inks: 2, tree: null, parted: {}, locked: true, lockWanted: false,
    divide: null, over: null, peek: null, run: null, swan: null, omen: null, said: "", drawn: "", meter: [], metered: null,
    seeing: {}, fates: [], fateMode: "full", planning: null, send: null});
  const canvas = document.createElement("canvas");
  canvas.id = "strange-sky";
  canvas.setAttribute("role", "img");
  canvas.setAttribute("aria-label", "The timelines of the worlds, as a tree: forward in time is up");
  const hud = document.createElement("div");
  hud.id = "strange-hud";
  hud.innerHTML = `
    <div class="strange-title strange-card">
      <nav class="strange-top" aria-label="Pages">
        <a href="home" target="_top">Home</a><a href="charts/ai-insights/" target="_top">Charts/</a><a class="strange-civvis" href="./" target="_top">CIVVIS</a>
      </nav>
      <b title="One world, many timelines. Forward in time is up: the line across the window is the open turn, what was played runs down from it and each world's future is played out ahead">Doctor Strange's CIVVIS</b>
      <div class="strange-news" role="group" aria-label="New game">
        <button data-strange="new2" data-players="2" data-map="pangaea" title="A new game: a new world of 2 civilizations on a Pangaea, one timeline again. The worlds open now are let go of"><span class="strange-news-map">2P Pangaea</span> <span class="strange-news-new">New Game</span></button>
        <button data-strange="new6" data-players="6" data-map="continents" title="A new game: a new world of 6 civilizations on Continents, one timeline again. The worlds open now are let go of"><span class="strange-news-map">6P Continents</span> <span class="strange-news-new">New Game</span></button>
      </div>
      <div class="strange-deck strange-time" role="toolbar" aria-label="Timeline">
        <div class="strange-steps">
          <button data-strange="first" aria-label="Turn 1">Turn 1</button>
          <button data-strange="era-back" aria-label="Era back"><i>◀◀</i>Era</button>
          <button data-strange="back10" title="The worlds go back ten turns, as they stood">−10</button>
          <button data-strange="back1" title="The worlds go back a turn, as they stood">−1</button>
          <button data-strange="on1" title="The worlds play a turn, each with its own player strategy">+1</button>
          <button data-strange="on10" title="The worlds play ten turns, each with its own player strategy">+10</button>
          <button data-strange="era-on" aria-label="Era forward">Era<i>▶▶</i></button>
        </div>
        <button class="strange-play" data-strange="play"></button>
      </div>
      <div class="strange-say" data-strange="say" role="status"></div>
      <div class="strange-deck strange-branching" role="group" aria-label="Branch">
        <label class="strange-row strange-base"><span>Following:</span><select data-strange="base" aria-label="Following: the world a branch is made from"></select></label>
        <div class="strange-nexts" role="group" aria-label="Branch the next best">
          <button class="strange-next" data-strange="next1">Branch Next Best</button>
          <button class="strange-next" data-strange="next5">Branch 5 Next Best</button>
        </div>
        <label class="strange-row strange-create"><span>Create Branch:</span><select class="strange-branch" data-strange="branch" aria-label="Create Branch: from the timeline followed, here, played by"></select></label>
        <button class="strange-search" data-strange="search" aria-expanded="false" aria-label="Autosearch" title="Autosearch: from the timeline followed, on its open turn, find which player strategies should hold the seat, and when, to meet what you ask for. A share of this computer's cores each plays paths up and down the timeline on a copy of that world, and the best path found can be made a branch"><svg viewBox="0 0 16 16" width="14" height="14" aria-hidden="true"><circle cx="6.5" cy="6.5" r="4.5" fill="none" stroke="currentColor" stroke-width="2"/><path d="M10 10l4.5 4.5" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"/></svg><span>Autosearch</span><span data-strange-search-count></span></button>
      </div>
    </div>
    <div class="strange-right">
      <div class="strange-meter strange-card" aria-label="What this page costs">
        <i>CPU</i><b data-strange-gauge="cpu">–</b><canvas data-strange-trace="cpu" width="264" height="44"></canvas>
        <i>Memory</i><b data-strange-gauge="memory">–</b><canvas data-strange-trace="memory" width="264" height="44"></canvas>
        <em data-strange-worlds></em>
      </div>
      <div class="strange-swans" role="group" aria-label="Black swan">
        <button class="strange-swan" data-strange="swan"></button>
        <select data-strange="swan-kind" aria-label="Which black swan"></select>
      </div>
      <div class="strange-victory" data-strange-victory></div>
      <section class="strange-defeat strange-card" data-strange-defeat aria-label="Defeat tracker: how near each world is to a nuclear war and to losing the game"></section>
    </div>
    <div class="strange-standings" data-strange-standings></div>
    <div class="strange-versus strange-card" data-strange="numbers"></div>
    <button class="strange-send" data-strange="send">Send it</button>
    <div class="strange-omen" data-strange="omen" role="alert" title="Click to put away" hidden></div>
    <div class="strange-divide" title="Drag to give the tree or the worlds more room. Double-click to let the page place it"></div>
    <div class="strange-now strange-turn"><span>TURN</span><b data-strange="turn"></b><span data-strange="limit"></span><span class="strange-era" data-strange="era"></span></div>`;
  hud.addEventListener("click", event => {
    const button = event.target.closest("[data-strange]");
    if (button && button.dataset.players) { strangeNewGame(Number(button.dataset.players), button.dataset.map); return; }
    if (!button || button.disabled || !debugSplit) return;
    const action = button.dataset.strange, now = strangeNow() ?? 1;
    if (action === "play") strangePlay();
    else if (action === "first") strangeGoTo(1);
    else if (action === "era-back") { const began = strangeEraBack(); if (began) strangeGoTo(began.turn); }
    else if (action === "era-on") strangeEraOn();
    else if (action === "back10") strangeGoTo(now - 10);
    else if (action === "back1") strangeGoTo(now - 1);
    else if (action === "on1") strangeGoTo(now + 1);
    else if (action === "on10") strangeGoTo(now + 10);
    else if (action === "search") debugSearchShow(!debugSearchShown);
    else if (action === "send") strangeSendIt();
    else if (action === "next1") strangeNext(1);
    else if (action === "next5") strangeNext(STRANGE_NEXT_MANY);
    else if (action === "swan") strangeSwan(hud.querySelector('[data-strange="swan-kind"]').value);
    else if (action === "omen") { strange.omen = null; strangeDraw(); }
    else if (action.startsWith("follow-")) { strange.follow = action.slice(7); strangeDraw(); }
    else if (action.startsWith("close-")) strangeClose(action.slice(6));
  });
  hud.addEventListener("change", event => {
    if (!debugSplit) return;
    // The base timeline picked is the world followed: Branch branches from it.
    if (event.target.dataset.strange === "base") {
      const letter = event.target.value;
      event.target.blur();
      if (strangeOpen().includes(letter)) { strange.follow = letter; strangeDraw(); }
      return;
    }
    // Picking who plays a branch makes it. The picker is a button again.
    if (event.target.dataset.strange === "branch") {
      const name = event.target.value;
      event.target.value = "";
      event.target.blur();
      strangeBranch(name);
      return;
    }
    const letter = event.target.dataset.strangeAgent;
    if (letter) debugSplitSeat(letter, event.target.value);
  });
  // The page's keys are not a picker's.
  hud.addEventListener("keydown", event => { if (event.target.matches("select")) event.stopPropagation(); });
  // The divide is dragged by the line of the open turn. The boxes are deaf
  // to the pointer for as long as that lasts, or they would take it.
  const divide = hud.querySelector(".strange-divide");
  divide.addEventListener("pointerdown", event => {
    try { divide.setPointerCapture(event.pointerId); } catch (_) {}
    split.host.classList.add("strange-dragging");
  });
  divide.addEventListener("pointermove", event => {
    if (!split.host.classList.contains("strange-dragging")) return;
    strange.divide = event.clientY / window.innerHeight;
    strangeLayout();
  });
  for (const kind of ["pointerup", "pointercancel"])
    divide.addEventListener(kind, () => split.host.classList.remove("strange-dragging"));
  divide.addEventListener("dblclick", () => { strange.divide = null; strangeLayout(); });
  canvas.addEventListener("pointermove", event => { strange.hover = {x: event.clientX, y: event.clientY}; });
  canvas.addEventListener("pointerleave", () => { strange.hover = null; });
  canvas.addEventListener("click", event => {
    if (strange.cam === null) return;
    strangeGoTo(strangeTurnAt(strangeGeometry(), event.clientY, strange.cam));
  });
  split.host.prepend(canvas);
  split.host.appendChild(hud);
  strange.canvas = canvas; strange.hud = hud;
  strange.then = 0;
  strangeStandingsMount(hud);
  for (const letter of strange.order) strangeWorldOpen(letter);
  // A card that has changed its size is measured once the browser has laid
  // the page out, which is when it says so.
  if (strange.watch) strange.watch.disconnect();
  strange.watch = typeof ResizeObserver === "function" ? new ResizeObserver(() => { strange.stale = true; strangeLayout(); }) : null;
  if (strange.watch) for (const name of STRANGE_CARDS) strange.watch.observe(hud.querySelector(name));
  if (!strange.resized) {
    strange.resized = true;
    window.addEventListener("resize", () => { strange.stale = true; strangeLayout(); });
  }
  strange.stale = true;
  strangeLayout();
  clearInterval(strange.meterTimer);
  strange.meterTimer = setInterval(() => { strangeAheadWatch(); strangeSwanWatch(); strangeSearchWatch(); strangeSendWatch(); strangeForesee(); strangeMeasure(); }, STRANGE_METER_MS);
  strangeMeasure();
  requestAnimationFrame(strangePaint);
}
// Where things stand in the window: the line of the open turn, which is the
// divide between the tree and the boxes, each open world's box, and the
// colour of each.
// Where the cards stand. The cards at the top keep their room: the line
// stays under them, with the worlds' own card standing on it, and a name on
// the tree keeps off them.
//
// Measuring makes the browser lay the page out, so the cards are measured
// when one has changed its shape, and not each time a world speaks, which is
// a dozen times a turn and was an eighth of all the page did while time ran.
// What is drawn says when it has put something else on a card (`strangeDraw`,
// `strange.shape`), the line moving moves the worlds' card with it, and a
// card that has grown by itself, a number gaining a figure, is seen by the
// browser once it has laid the page out anyway (`strangeMount`).
// The turn on the line is one of them: when its number grows a digit (9 to
// 10, 99 to 100) it is wider, and standing left of the tree it is placed by
// its right edge (`strangeTurnLeft`), so it is measured again. Measured only
// when the other cards changed, it stood a digit off the green path from
// turn 10 on (published-build red on `check_doctor_strange_send_it` since
// #5982).
const STRANGE_CARDS = [".strange-title", ".strange-right", ".strange-standings", ".strange-versus", ".strange-omen", ".strange-send",
  ".strange-turn"];
function strangeCards() {
  const hud = strange.hud;
  const box = name => hud.querySelector(name).getBoundingClientRect();
  // The worlds' numbers stand under the standings of the world followed,
  // where those are shown.
  const standings = hud.querySelector(".strange-standings"), shown = standings.childElementCount > 0;
  const under = shown ? Math.ceil(standings.getBoundingClientRect().bottom + 8) : 12;
  if (hud.style.getPropertyValue("--strange-versus-top") !== `${under}px`) hud.style.setProperty("--strange-versus-top", `${under}px`);
  const title = box(".strange-title"), versus = box(".strange-versus"), column = box(".strange-right");
  // The tree is drawn under the worlds' numbers, and the line stands low
  // enough to leave it room.
  strange.treeTop = Math.ceil(versus.bottom + 14);
  if (hud.style.getPropertyValue("--strange-band") !== `${Math.ceil(versus.bottom)}px`)
    hud.style.setProperty("--strange-band", `${Math.ceil(versus.bottom)}px`);
  // All 14,000,605 stands at the foot of the tree on the left, on the line
  // as the worlds' numbers do on the right, unless the turn written on the
  // line would be under it there: then it stands over the turn.
  const turn = box(".strange-turn"), foot = box(".strange-send");
  // What the turn on the line needs to stand clear of the column on the right.
  strange.turnWide = turn.width;
  strange.turnColumn = {left: column.left, bottom: column.bottom};
  const lift = turn.right > foot.left - 10 && turn.left < foot.right + 10 ? Math.ceil(turn.height) + 8 : 0;
  if (hud.style.getPropertyValue("--strange-send-lift") !== `${lift}px`) hud.style.setProperty("--strange-send-lift", `${lift}px`);
  strange.floor = Math.ceil(Math.max(title.bottom + 46, title.bottom + 10 + foot.height + 12 + lift, versus.bottom + 170));
  strange.cards = [{left: title.left, right: title.right, top: title.top, bottom: title.bottom, side: -1},
    {left: column.left, right: column.right, top: column.top, bottom: column.bottom, side: 1},
    {left: versus.left, right: versus.right, top: versus.top, bottom: versus.bottom, side: 1}];
  if (shown) {
    const held = standings.getBoundingClientRect();
    strange.cards.push({left: held.left, right: held.right, top: held.top, bottom: held.bottom, side: 1});
  }
  // And off what the black swan says while it says it.
  for (const held of [hud.querySelector(".strange-omen")]) {
    if (held.hidden) continue;
    const said = held.getBoundingClientRect();
    strange.cards.push({left: said.left, right: said.right, top: said.top, bottom: said.bottom, side: 1});
  }
  const all = box(".strange-send");
  strange.cards.push({left: all.left, right: all.right, top: all.top, bottom: all.bottom, side: -1});
  strange.stale = false;
}
function strangeLayout() {
  const split = debugSplit, hud = strange.hud;
  if (!split || !hud || !hud.isConnected) return;
  if (strange.stale) strangeCards();
  const g = strangeGeometry(), host = split.host, open = strangeOpen();
  const put = (held, name, value) => {
    if (held.style.getPropertyValue(name) === value) return false;
    held.style.setProperty(name, value);
    return true;
  };
  // The worlds' card stands on the line, and has moved if the line has.
  if (put(host, "--strange-line", `${g.cy}px`)) strangeCards();
  // The turn on the line moves with the board and the lanes, and Send it
  // with it (`strangeCards`).
  if (put(host, "--strange-board", `${Math.round(strangeBoard(g) + 6)}px`)) strangeCards();
  if (put(host, "--strange-turn-left", `${strangeTurnLeft(g)}px`)) strangeCards();
  for (const letter of open) {
    const at = g.boxes[letter], pane = split.panes[letter], plate = hud.querySelector(`[data-strange-world="${letter}"]`);
    if (!at || !plate) continue;
    for (const held of [plate, pane.frame]) {
      put(held, "--strange-tint", strange.tint[letter]);
      put(held, "left", `${Math.round(at.x)}px`);
      put(held, "width", `${Math.round(at.x + at.w) - Math.round(at.x)}px`);
      held.classList.toggle("strange-over", strange.over === letter);
    }
    put(plate, "top", `${Math.round(at.y)}px`);
    put(pane.frame, "top", `${Math.round(at.y) + STRANGE_HEAD}px`);
    put(pane.frame, "height", `${Math.max(0, Math.round(at.y + at.h) - Math.round(at.y) - STRANGE_HEAD)}px`);
    plate.classList.toggle("strange-notch", open.length === 1);
  }
  // The sign over the far end of each branch.
  const fates = strangeFates(g, open);
  strange.fateMode = fates.mode;
  strange.fates = fates.put;
  for (const at of fates.put) {
    const sign = hud.querySelector(`[data-strange-fate="${at.letter}"]`);
    if (!sign) continue;
    put(sign, "--strange-tint", strange.tint[at.letter]);
    put(sign, "left", `${Math.round(at.left)}px`);
    put(sign, "top", `${Math.round(at.top)}px`);
    put(sign, "width", `${Math.round(at.wide)}px`);
    put(sign, "height", `${at.tall}px`);
    sign.classList.toggle("strange-over", strange.over === at.letter);
  }
}

// What is written on the highway, drawn from the split's record whenever the
// split is drawn.
function strangeDraw() {
  const split = debugSplit, hud = strange.hud;
  if (!split || !hud || !hud.isConnected) return;
  const open = strangeOpen();
  if (open.length === 0) return;
  if (!open.includes(strange.follow)) strange.follow = open[0];
  const tree = strange.tree = strangeTree(), panes = split.panes, follow = strange.follow;
  const idle = strangeIdle(), many = open.length > 1, one = !Number.isFinite(tree.splits);
  const turn = strangeNow() ?? tree.from, limit = strangeLimit(), showing = strangeShowing() ?? turn;
  const set = (name, text) => { const held = hud.querySelector(`[data-strange="${name}"]`); if (held.textContent !== text) held.textContent = text; return held; };
  set("turn", String(showing));
  set("limit", limit ? `/ ${limit}` : "");
  // The era is the last the followed world says it came into by the open turn.
  const came = ((panes[follow].marks || {}).eras || []).filter(era => era.turn <= showing).pop();
  set("era", came ? `${debugEraName(came)} era` : typeof state.world_era === "number" && showing === state.turn ? `${debugEraName({era: state.world_era})} era` : "");
  const going = split.running > 0 || strange.run !== null;
  const play = set("play", going ? (strange.run && strange.run.stop ? "⏸ Pausing…" : "⏸ Pause Time") : "▶ Play Time");
  play.disabled = !split.shown || split.busy || (!going && (!idle || (limit !== null && turn >= limit)));
  play.title = going ? (!many ? "Stop time on the turn being played"
    : strange.locked ? "Stop the worlds together, on the turn they are all playing" : "Stop the worlds on the turns they are playing")
    : !many ? "Time runs on from here: the world plays on, a turn at a time"
    : strange.locked ? "Time runs on from here: the worlds play on, a turn at a time and always on the same turn, each with its own player strategy"
    : "Time runs on from here: the worlds play on, each by itself with its own player strategy";
  // Generate Black Swan, and which: any, drawn at random, or one by name.
  // The engine says which there are (`debug.swan_kinds`).
  const struck = strange.locked && many ? open : [follow];
  const swan = set("swan", strange.swan ? `${STRANGE_SWAN} Striking…` : `${STRANGE_SWAN} Generate Black Swan`);
  swan.disabled = !idle;
  swan.title = struck.length > 1
    ? `An event nothing saw coming strikes every world on turn ${turn}, the same one on each, and changes the game from there. Unlock to strike ${follow.toUpperCase()} alone`
    : `An event nothing saw coming strikes world ${follow.toUpperCase()} on turn ${turn} and changes the game from there`;
  const kinds = hud.querySelector('[data-strange="swan-kind"]');
  const sorts = `<option value="" title="One drawn at random, among those the world has something for">Any</option>` +
    (((state || {}).debug || {}).swan_kinds || []).map(kind =>
      `<option value="${escapeAttr(kind.id)}" title="${escapeAttr(kind.about)}">${escapeAttr(kind.name)}</option>`).join("");
  if (kinds.dataset.drawn !== sorts && document.activeElement !== kinds) {
    const picked = kinds.value;
    kinds.dataset.drawn = sorts;
    kinds.innerHTML = sorts;
    kinds.value = [...kinds.options].some(option => option.value === picked) ? picked : "";
  }
  kinds.disabled = !idle;
  kinds.title = "Which black swan Generate strikes: any, drawn at random, or this one";
  // What struck, for as long as it is news: the followed world's account.
  const omen = hud.querySelector('[data-strange="omen"]'), news = strange.omen;
  const told = !news ? "" : (() => {
    const text = String(news.swan.text || ""), name = String(news.swan.name || "Black swan");
    const letters = news.letters.map(letter => letter.toUpperCase());
    const where = letters.length === 1 ? `World ${letters[0]}`
      : `Worlds ${letters.slice(0, -1).join(", ")} and ${letters[letters.length - 1]}, the same on each`;
    return `<b>${STRANGE_SWAN} Black swan · turn ${news.swan.turn}</b><strong>${escapeAttr(name)}</strong>` +
      `<span>${escapeAttr(text.startsWith(`${name}. `) ? text.slice(name.length + 2) : text)}</span><em>${where}</em>`;
  })();
  if (omen.dataset.drawn !== told) { omen.dataset.drawn = told; omen.innerHTML = told; }
  omen.hidden = !news;
  const first = Math.max(...open.map(letter => panes[letter].first ?? 1));
  // Turn 1, Era back and Era forward (`strangeEraBack`, `strangeEraOn`).
  const eraBack = strangeEraBack(turn), eraNext = strangeEraNext(turn), lastEra = strangeEraNow(turn) >= ERA_NAMES.length - 1;
  for (const [name, off] of [["first", turn <= first], ["era-back", !eraBack || turn <= first], ["back10", turn <= first], ["back1", turn <= first],
    ["on1", limit !== null && turn >= limit], ["on10", limit !== null && turn >= limit],
    ["era-on", (limit !== null && turn >= limit) || (!eraNext && lastEra)]])
    hud.querySelector(`[data-strange="${name}"]`).disabled = !idle || off;
  hud.querySelector('[data-strange="first"]').title = first > 1 ? `The worlds go back to turn ${first}, the first every world holds, as they stood`
    : "The worlds go back to turn 1, as they stood";
  hud.querySelector('[data-strange="era-back"]').title = eraBack
    ? `Era back: the worlds go back to turn ${eraBack.turn}, where the ${debugEraName(eraBack)} era began for ${follow.toUpperCase()}. Pressed there, to the start of the era before`
    : "Era back: to the start of this era, then the era before. This is the first turn of the first era";
  hud.querySelector('[data-strange="era-on"]').title = eraNext
    ? `Era forward: the worlds go on to turn ${eraNext.turn}, where the ${debugEraName(eraNext)} era begins for ${follow.toUpperCase()}`
    : lastEra ? `Era forward: ${follow.toUpperCase()} is in the last era`
    : `Era forward: the worlds play on, each with its own player strategy, until ${follow.toUpperCase()} comes into a new era`;
  // Before Branch, the base timeline: the world the branch is made from
  // (operator 2026-11-25: "the option to branch in the simulator should be
  // preceded by a selection of the base timeline"). Every open world is on
  // offer, left to right, by its letter and its name, and the picker wears
  // the colour of the one picked. The base is the followed world: picking
  // one here is Follow, and Follow on a box picks it here.
  const base = hud.querySelector('[data-strange="base"]');
  const bases = open.map(letter => `<option value="${letter}">${letter.toUpperCase()} · ${escapeAttr(strangeSeries(letter, tree))}</option>`).join("");
  if (base.dataset.drawn !== bases && document.activeElement !== base) { base.dataset.drawn = bases; base.innerHTML = bases; }
  if (base.value !== follow && [...base.options].some(option => option.value === follow)) base.value = follow;
  if (base.style.getPropertyValue("--strange-base") !== strange.tint[follow]) base.style.setProperty("--strange-base", strange.tint[follow]);
  base.title = many ? `Following ${follow.toUpperCase()}: Create Branch makes its branch from it, as it stands on turn ${turn}. Pick another world to follow and branch from it`
    : `Following ${follow.toUpperCase()}, the one world open: Create Branch makes its branch from it`;
  // Branch is a picker of who plays the branch, and picking makes it. Each
  // choice is what the branch's name gains: the open turn and the strategy.
  // The one the base timeline seats is not offered: that is the same world.
  const budget = strangeBudget(), full = open.length >= budget.most;
  const branch = hud.querySelector('[data-strange="branch"]');
  const offers = `<option value="">⑂ ${turn}-(player strategy)</option>` + strangeAgents().filter(name => name !== panes[follow].strategy)
    .map(name => `<option value="${escapeAttr(name)}">${turn}-${escapeAttr(strangeHandle(name))}</option>`).join("");
  if (branch.dataset.drawn !== offers && (document.activeElement !== branch || !idle)) {
    branch.dataset.drawn = offers;
    branch.innerHTML = offers;
    branch.value = "";
  }
  branch.disabled = !idle || full;
  branch.title = full ? `${strangeFullWords(budget)}. Delete one to branch again`
    : `Create a branch of the timeline followed, ${follow.toUpperCase()}, on turn ${turn}: pick the player strategy that plays the branch from here, and it is made, a world of its own in a box of its own`;
  // Send it: the best player strategies without a world there, each
  // branched from the base timeline on this turn, up to twelve worlds, and
  // time run on for every world to the end (`strangeSendIt`). Time running
  // is stopped for it first, so it can be pressed then too.
  const send = hud.querySelector('[data-strange="send"]'), {room, wanted} = strangeSendRoom(budget, open.length);
  const fresh = strangeNextNames(follow).slice(0, wanted), take = fresh.slice(0, room), waiting = !!strange.send;
  const ended = limit !== null && turn >= limit, fast = !!(strange.run && strange.run.fast);
  send.classList.toggle("on", waiting || fast);
  send.setAttribute("aria-busy", waiting ? "true" : "false");
  send.disabled = waiting || split.busy || !!(strange.run && strange.run.stop) || (going ? room <= 0 && (fast || !strange.run) : !idle || ended);
  const all = open.length + take.length, short = fresh.length - take.length;
  send.title = waiting ? "Spawning the branches, then every world runs out to the end"
    : going && room <= 0 ? (fast ? `Running all ${open.length} worlds out to the end` : `Run all ${open.length} worlds out to the end, as fast as they are played`)
    : going ? `Stop time, spawn branches of ${follow.toUpperCase()} on the turn it stops on, up to ${STRANGE_SEND_WORLDS} worlds, and run all out to the end`
    : ended ? "The game is over in every world"
    : take.length === 0 ? `${room <= 0 ? strangeFullWords(budget) : `Every player strategy has a world of its own on turn ${turn}`}: run all ${open.length} worlds out to the end`
    : `Spawn ${take.length} branch${take.length === 1 ? "" : "es"} of ${follow.toUpperCase()} and run all ${all} worlds out to the end` +
      (short > 0 ? ` (${short} more do not fit: ${strangeFullWords(budget, false)})`
        : all < STRANGE_SEND_WORLDS ? `: every player strategy has a world then` : "");
  // Branch Next Best and Branch 5 Next Best, over Branch (`strangeNext`): the
  // best player strategies without a world of their own on this turn.
  const nextBest = strangeNextNames(follow);
  for (const [action, count] of [["next1", 1], ["next5", STRANGE_NEXT_MANY]]) {
    const button = hud.querySelector(`[data-strange="${action}"]`), take = nextBest.slice(0, Math.min(count, room));
    button.disabled = !idle || full || waiting || nextBest.length === 0;
    button.title = full ? `${strangeFullWords(budget)}. Delete one to branch again`
      : nextBest.length === 0 ? `Every player strategy has a world of its own on turn ${turn} already`
      : `Branch the timeline followed, ${follow.toUpperCase()}, on turn ${turn} for the ` +
        (count === 1 ? "best player strategy without a world of its own there, a world" : `${count} best player strategies without a world of their own there, each a world`) +
        ` in a box of its own: ${take.map(name => `${turn}-${strangeHandle(name)}`).join(", ")}` +
        (take.length < Math.min(count, nextBest.length) ? ` (room for ${take.length})` : "");
  }
  const seats = new Set(open.map(letter => panes[letter].strategy));
  set("say", strange.said || (!many ? `World ${open[0].toUpperCase()} is the one open. Branch opens another beside it`
    : one ? (seats.size === 1 ? "The worlds seat the same player strategy: they are one world until one is given another"
      : "One world so far. Play, and each player strategy takes it its own way")
    : open.length === 2 ? `${open[0].toUpperCase()} and ${open[1].toUpperCase()} parted on turn ${tree.splits}`
    : `The worlds first parted on turn ${tree.splits}`));
  for (const letter of open) {
    const pane = panes[letter], numbers = pane.numbers || {}, name = letter.toUpperCase();
    const picker = hud.querySelector(`[data-strange-agent="${letter}"]`);
    if (!picker) continue;
    debugOfferAgents(picker, pane.strategy);
    picker.disabled = !idle;
    const series = strangeSeries(letter, tree), named = hud.querySelector(`[data-strange-name="${letter}"]`);
    if (named.textContent !== series) { named.textContent = series; named.title = `Branch ${series}: the player strategies that played it, and the turn each took the seat on. ${STRANGE_SWAN} is a black swan, on the turn it struck`; }
    const exit = hud.querySelector(`[data-strange-world="${letter}"] [data-strange="close-${letter}"]`);
    exit.disabled = !strangeCanClose(letter);
    exit.title = many ? `Delete ${name}: its line goes from the tree and its box from the window` : "The last world stays open";
    // A black swan that is still going on in this world is said first: the
    // latest by name, and how many more there are, because the plate is short.
    const lasting = strangeSwans(letter).filter(swan => swan.ongoing && swan.turn <= (pane.turn ?? 0));
    const still = lasting.length === 0 ? []
      : [`${STRANGE_SWAN} ${lasting[lasting.length - 1].name}${lasting.length > 1 ? ` +${lasting.length - 1}` : ""}`];
    const line = [...still, ...(strange.locked || pane.turn === null ? [] : [`Turn ${pane.turn}`]),
      ...[["score", "Score"], ["cities", "Cities"], ["science", "Science"]]
        .filter(([key]) => typeof numbers[key] === "number")
        .map(([key, label]) => `${label} ${debugVersusNumber(numbers[key])}`)].join(" · ");
    const held = hud.querySelector(`[data-strange-line="${letter}"]`);
    if (held.textContent !== line) held.textContent = line;
    const button = hud.querySelector(`[data-strange="follow-${letter}"]`);
    const on = follow === letter;
    button.hidden = !many;
    const words = on ? "Following" : "Follow";
    if (button.textContent !== words) button.textContent = words;
    button.classList.toggle("on", on);
    button.disabled = on;
    button.title = on ? `${name} is the timeline in the middle`
      : `Put ${name}'s timeline in the middle: it is then followed, and Create Branch branches from ${name}`;
  }
  // The sign over each branch: as much of what is coming as there is room
  // for (`strangeFates`), and all of it when the pointer is on it.
  const mode = strangeFates(strangeGeometry(), open).mode;
  for (const letter of open) {
    const sign = hud.querySelector(`[data-strange-fate="${letter}"]`);
    if (!sign) continue;
    const fate = strangeFate(letter), [path, head, wars] = [sign.querySelector("small"), sign.querySelector("strong"), sign.querySelector("span")];
    const words = mode === "full" ? fate.head : mode === "line" ? fate.line : fate.word;
    if (path.textContent !== fate.path) path.textContent = fate.path;
    if (head.textContent !== words) head.textContent = words;
    if (wars.textContent !== fate.wars) wars.textContent = fate.wars;
    if (sign.dataset.fate !== fate.state) sign.dataset.fate = fate.state;
    if (sign.title !== fate.title) sign.title = fate.title;
    sign.classList.toggle("strange-fate-slim", mode !== "full");
    const share = `${Math.round(fate.seen * 100)}%`;
    if (sign.style.getPropertyValue("--strange-seen") !== share) sign.style.setProperty("--strange-seen", share);
  }
  // The worlds' numbers across the top (operator 2026-11-26: "swap the x and
  // y for the top table. cols should roughly align in order with the below
  // lines. can merge equivalent states. mabye notation A,B"): a column a
  // world, left to right as the worlds' lines stand under it, and a row a
  // number. Each says where the world stands now (for a yield, what it
  // makes a turn) and, beside a yield, what it has made so far, in whole
  // numbers, while there are three columns or fewer. Worlds that are the
  // same world, on the same turn (`strangeSame`), are one column headed
  // "A,B". The most of a row is lit, in gold on a chip of gold (operator
  // 2026-11-26: "in doctor strange hud, we should highlight the top value
  // in each row. so the branch with the top score, most cities, etc."):
  // every column that has it, unless every column has the same. Each world
  // in a column's head has an ✕ that deletes it, as the ✕ on its box does
  // ("we should also be able to exit branches. maybe from the top, have an
  // option to close branches").
  const columns = strangeSame(open);
  const wide = columns.length <= 3;
  const numbers = group => group.map(letter => panes[letter].numbers).find(Boolean) || {};
  const lead = (key, group) => {
    const values = columns.map(other => numbers(other)[key]);
    if (values.length < 2 || !values.every(other => typeof other === "number")) return false;
    const top = Math.max(...values);
    return numbers(group)[key] === top && values.some(other => other < top);
  };
  const lit = (on, text) => on ? `<span class="strange-lit">${text}</span>` : text;
  const cell = (key, group) => {
    const value = numbers(group)[key], made = numbers(group)[`${key}_total`], most = lead(`${key}_total`, group);
    const total = !wide || !STRANGE_TOTALS.includes(key) ? ""
      : `<small${most ? ' class="lead"' : ""}>${lit(most, typeof made === "number" && Number.isFinite(made) ? (Math.round(made) || 0).toLocaleString("en-US") : "–")}</small>`;
    return `<b${lead(key, group) ? ' class="lead"' : ""}>${lit(lead(key, group), debugVersusNumber(value))}${total}</b>`;
  };
  const exit = letter => {
    const can = strangeCanClose(letter), name = strangeSeries(letter, tree);
    return `<button class="strange-row-exit" data-strange="close-${letter}"${can ? "" : " disabled"} ` +
      `title="${escapeAttr(many ? `Delete ${name}: its line goes from the tree and its box from the window` : "The last world stays open")}" ` +
      `aria-label="Delete ${escapeAttr(name)}"></button>`;
  };
  const head = group => {
    const names = group.map(letter => letter.toUpperCase());
    const said = group.length > 1 ? ` title="${names.slice(0, -1).join(", ")} and ${names[names.length - 1]} are the same world on turn ${panes[group[0]].turn ?? turn}"` : "";
    return `<span class="strange-letter"${said}>` +
      group.map(letter => `${exit(letter)}<span style="color:${strange.tint[letter]}">${letter.toUpperCase()}</span>`).join(",") + "</span>";
  };
  const who = group => `<span class="strange-who">` + group.map(letter => {
    const held = strangeHandle(panes[letter].strategy);
    return `<span style="color:${strange.tint[letter]}" title="${escapeAttr(`${letter.toUpperCase()} · ${strangeSeries(letter, tree)}: ${held} holds the seat now`)}">${escapeAttr(held)}</span>`;
  }).join(", ") + `</span>`;
  // End outcome, each world's own (`strangeEndOutcome`): the best is lit,
  // unless every world comes to the same.
  const ends = Object.fromEntries(open.map(letter => [letter, strangeEndOutcome(letter)]));
  const before = (one, other) => one.rank[0] - other.rank[0] || one.rank[1] - other.rank[1];
  const best = open.map(letter => ends[letter]).reduce((one, other) => before(other, one) < 0 ? other : one, ends[open[0]]);
  const even = open.every(letter => before(ends[letter], best) === 0);
  const end = group => {
    const shown = group.length > 2 ? group.slice(0, 1) : group, more = group.slice(shown.length);
    return `<span class="strange-end">` + shown.map(letter => {
      const at = ends[letter], line = escapeAttr(at.text);
      const lead = group.length > 1 ? `<b style="color:${strange.tint[letter]}">${letter.toUpperCase()}:</b> ` : "";
      return `<span data-end-world="${letter}" data-outcome="${at.outcome}" title="${escapeAttr(`${at.text}\n${at.title}`)}">${lead}` +
        `${lit(!even && before(at, best) === 0, line)}</span>`;
    }).join("") + (more.length === 0 ? "" : `<span class="strange-end-more" title="${escapeAttr(more.map(letter =>
      `${letter.toUpperCase()}: ${ends[letter].text}`).join("\n"))}">+${more.length} more: ${more.map(letter => letter.toUpperCase()).join(", ")}</span>`) +
      `</span>`;
  };
  const table = `<i></i>` + columns.map(head).join("") + `<i></i>` + columns.map(who).join("") + STRANGE_VERSUS_ROWS.map(([key, label, title]) => {
    const summed = STRANGE_TOTALS.includes(key);
    const short = STRANGE_VERSUS_SHORT[key] || label;
    return `<em title="${summed && wide ? `${title}: a turn, and beside it made over the game so far${key === "gold" ? " (net of upkeep)" : ""}` : title}">` +
      `<span class="strange-long">${label}</span><span class="strange-short">${short}</span></em>` +
      columns.map(group => cell(key, group)).join("");
  }).join("") + `<em class="strange-end-head" title="How each world's game ends, as its future has been played out to the end: who wins and on what turn, then the wars and nuclear wars still to come"><span class="strange-long">End outcome</span><span class="strange-short">Outcome</span></em>` +
    columns.map(end).join("");
  if (strange.drawn !== table) {
    strange.drawn = table;
    const card = hud.querySelector('[data-strange="numbers"]');
    card.style.gridTemplateColumns = `auto repeat(${columns.length}, minmax(0, 1fr))`;
    card.innerHTML = table;
  }
  strangeDefeatDraw(hud, open, tree);
  strangeStandingsDraw();
  const count = `${open.length} world${open.length === 1 ? "" : "s"} open`;
  const said = hud.querySelector("[data-strange-worlds]");
  if (said.textContent !== count) said.textContent = count;
  // What gives a card another shape: the worlds on the worlds' card, what
  // the title's card says, and a black swan's news.
  const shape = `${columns.map(group => group.join("")).join(",")}|${wide}|${hud.querySelector('[data-strange="say"]').textContent}|${omen.dataset.drawn}|` +
    `${String(turn).length}|${hud.querySelector('[data-strange="era"]').textContent}`;
  if (shape !== strange.shape) { strange.shape = shape; strange.stale = true; }
  strangeLayout();
  // Autosearch's panel says what can be done with the worlds as they stand.
  if (debugSearchShown || (debugSearch && debugSearch.live)) debugSearchDrawSoon();
}

// One frame of the highway.
function strangePaint(time) {
  const split = debugSplit, canvas = strange.canvas;
  if (!split || !canvas || !canvas.isConnected) { strange.canvas = null; clearInterval(strange.meterTimer); return; }
  requestAnimationFrame(strangePaint);
  if (document.hidden) return;
  const open = strangeOpen();
  if (open.length === 0) return;
  // What may be pressed turns on what a half says of itself, which is not
  // something the split is drawn again for.
  const idle = strangeIdle();
  if (idle !== strange.idleWas) { strange.idleWas = idle; strangeDraw(); if (idle) strangeAheadNext(); }
  let g = strangeGeometry();
  const ratio = Math.min(2, window.devicePixelRatio || 1);
  if (canvas.width !== Math.round(g.W * ratio) || canvas.height !== Math.round(g.H * ratio)) {
    canvas.width = Math.round(g.W * ratio);
    canvas.height = Math.round(g.H * ratio);
    // A window of another size has its line elsewhere.
    strange.stale = true;
    strangeLayout();
    g = strangeGeometry();
  }
  const pen = canvas.getContext("2d");
  pen.setTransform(ratio, 0, 0, ratio, 0, 0);
  pen.clearRect(0, 0, g.W, g.H);
  const tree = strange.tree || (strange.tree = strangeTree()), worlds = tree.worlds;
  const follow = open.includes(strange.follow) ? strange.follow : open[0];
  if (open.some(letter => !worlds[letter])) { strange.tree = null; return; }
  const turnOf = letter => split.panes[letter].turn ?? tree.from;
  const since = Math.min(.1, strange.then ? (time - strange.then) / 1000 : 0);
  strange.then = time;
  // The line follows the followed world's turn, and each branch its lane:
  // as many lanes to the side of the followed one as its box is boxes away.
  const ease = 1 - Math.exp(-since * 7), want = turnOf(follow);
  if (strange.cam === null || Math.abs(want - strange.cam) > 60) strange.cam = want;
  strange.cam += (want - strange.cam) * ease;
  if (Math.abs(want - strange.cam) < .003) strange.cam = want;
  for (const letter of open) {
    const lane = open.indexOf(letter) - open.indexOf(follow);
    const now = strange.lanes[letter] ?? lane;
    strange.lanes[letter] = now + (lane - now) * ease;
  }
  const cam = strange.cam, limit = strangeLimit();
  const first = Math.max(1, tree.first);
  const y = turn => strangeY(g, turn, cam);
  // The lane a world's line is in on a turn. The trunk stands in the middle
  // until it first forks; a branch leaves the line it branches from on the
  // turn it does, from where that line stood then, for its own lane.
  const laneAt = (letter, turn) => {
    const world = worlds[letter];
    if (world.parent === null) return strange.lanes[letter] * strangeRamp((turn - tree.splits) / 4);
    if (!world.parted || turn <= world.fork) return laneAt(world.parent, turn);
    const left = laneAt(world.parent, world.fork);
    return left + (strange.lanes[letter] - left) * strangeRamp((turn - world.fork) / 4);
  };
  const x = (letter, turn) => g.cx + laneAt(letter, turn) * strangeSpread(g, y(turn));
  const colour = strange.tint;
  // The pointer over a world's box picks out its line, and the rest stand back.
  const over = strange.over !== null && open.includes(strange.over) ? strange.over : null;
  const quiet = letter => over === null || over === letter ? .95 : .4;
  const low = Math.max(first, Math.floor(cam - 5 * g.tauDown)), high = Math.ceil(cam + 5 * g.tauUp);
  const end = limit ?? Infinity;

  // The sky: darker away from the line, with a glow where the turn is open.
  const glow = pen.createRadialGradient(g.cx, g.cy, 10, g.cx, g.cy, Math.max(g.W, g.H) * .62);
  glow.addColorStop(0, "#12312b"); glow.addColorStop(.35, "#081712"); glow.addColorStop(1, "#03070a");
  pen.fillStyle = glow;
  pen.fillRect(0, 0, g.W, g.H);
  // A black swan that has just struck lights the whole sky for a moment.
  const news = strange.omen, struckFor = news ? performance.now() - news.since : Infinity;
  if (struckFor < 900) {
    pen.globalAlpha = .34 * (1 - struckFor / 900);
    pen.fillStyle = STRANGE_SWAN_INK;
    pen.fillRect(0, 0, g.W, g.H);
    pen.globalAlpha = 1;
  }

  // The board: a fret for every turn, heavier on every fifth and tenth, as
  // wide as the tree is at that height.
  const fan = Math.max(2.7, g.reach * 2 + .7);
  const half = at => at >= g.cy ? g.lane / 2 - 10 : Math.min(g.W / 2 - 12, strangeSpread(g, at) * fan + 40);
  pen.lineCap = "round"; pen.lineJoin = "round";
  pen.textBaseline = "middle";
  pen.font = "600 11px system-ui, sans-serif";
  for (let turn = low; turn <= Math.min(high, end); turn++) {
    const at = y(turn), room = Math.abs(y(turn + 1) - at);
    const tenth = turn % 10 === 0, fifth = turn % 5 === 0;
    if (room < (tenth ? .6 : fifth ? 2 : 5)) continue;
    const wide = half(at);
    pen.strokeStyle = STRANGE_GOLD;
    pen.globalAlpha = Math.min(1, room / 9) * (tenth ? .34 : fifth ? .17 : .08);
    pen.lineWidth = tenth ? 1.5 : 1;
    pen.beginPath(); pen.moveTo(g.cx - wide, at); pen.lineTo(g.cx + wide, at); pen.stroke();
    if (tenth && room > 1.6) {
      pen.globalAlpha = Math.min(.8, room / 5);
      pen.fillStyle = "#bccac4";
      pen.textAlign = at >= g.cy ? "left" : "right";
      pen.fillText(String(turn), at >= g.cy ? g.cx - wide + 2 : g.cx - wide - 8, at >= g.cy ? at - 8 : at);
    }
  }
  // Its two edges, from the far end to the line.
  pen.globalAlpha = .22; pen.strokeStyle = STRANGE_GOLD; pen.lineWidth = 1.5;
  for (const side of [-1, 1]) {
    pen.beginPath();
    for (let at = g.cy - g.up; at <= g.cy; at += 6) {
      const px = g.cx + side * half(Math.min(at, g.cy - .01));
      if (at === g.cy - g.up) pen.moveTo(px, at); else pen.lineTo(px, at);
    }
    pen.stroke();
  }
  // The game's last turn, under its flag.
  if (limit !== null && limit <= high) {
    const at = y(limit), wide = half(at);
    pen.globalAlpha = .9; pen.strokeStyle = "#e8e3d2"; pen.lineWidth = 2; pen.setLineDash([6, 6]);
    pen.beginPath(); pen.moveTo(g.cx - wide, at); pen.lineTo(g.cx + wide, at); pen.stroke();
    pen.setLineDash([]);
    pen.fillStyle = "#e8e3d2"; pen.textAlign = "left";
    pen.fillText(`🏁 ${limit}`, g.cx + wide + 8, at);
  }
  // The eras the followed world has been through, each named on its first turn.
  for (const era of (split.panes[follow].marks || {}).eras || []) {
    if (!Number.isFinite(era.turn) || era.turn < low || era.turn > high || era.turn <= first) continue;
    const at = y(era.turn), wide = half(at);
    pen.globalAlpha = .75; pen.strokeStyle = STRANGE_GOLD; pen.lineWidth = 1.5;
    pen.beginPath(); pen.moveTo(g.cx - wide, at); pen.lineTo(g.cx + wide, at); pen.stroke();
    if (at < g.cy - 14) {
      pen.fillStyle = STRANGE_GOLD; pen.textAlign = "left";
      pen.fillText(debugEraName(era).toUpperCase(), g.cx + wide + 8, at);
    }
  }
  pen.globalAlpha = 1;

  // A stretch of one line of the tree, from one turn to another.
  const path = (from, to, where) => {
    const begin = Math.max(from, low), stop = Math.min(to, high);
    if (stop <= begin) return false;
    pen.beginPath();
    for (let turn = begin, at = 0; ; turn = Math.min(stop, turn + .25), at++) {
      const [px, py] = where(turn);
      if (at === 0) pen.moveTo(px, py); else pen.lineTo(px, py);
      if (turn >= stop) break;
    }
    return true;
  };
  const stroke = (tint, width, alpha) => {
    pen.strokeStyle = tint;
    pen.globalAlpha = alpha * .3; pen.lineWidth = width + 8; pen.stroke();
    pen.globalAlpha = alpha; pen.lineWidth = width; pen.stroke();
    pen.globalAlpha = 1;
  };
  // Words on the tree, beside a point of it: to the left of the point or to
  // the right, off the cards and off what is written already, and cut short
  // when long: a name keeps its end, which is who plays now, and anything
  // else its beginning.
  const written = [];
  const write = (words, px, at, side, tint, alpha, name = true) => {
    const text = words.length <= STRANGE_NAME_MOST ? words
      : name ? `…${words.slice(1 - STRANGE_NAME_MOST)}` : `${words.slice(0, STRANGE_NAME_MOST - 1)}…`;
    const wide = pen.measureText(text).width;
    let left = side > 0 ? px + 10 : px - 10 - wide, py = at;
    for (const card of strange.cards)
      if (py > card.top - 8 && py < card.bottom + 8 && left < card.right + 8 && left + wide > card.left - 8)
        left = card.side < 0 ? card.right + 8 : card.left - 8 - wide;
    left = Math.max(6, Math.min(g.W - 6 - wide, left));
    for (let moved = 0; moved < 8 && written.some(([from, to, level]) => left < to + 6 && left + wide > from - 6 && Math.abs(level - py) < 13); moved++)
      py -= 13;
    written.push([left, left + wide, py]);
    pen.globalAlpha = alpha; pen.textAlign = "left";
    pen.lineWidth = 3; pen.strokeStyle = "#03070a"; pen.strokeText(text, left, py);
    pen.fillStyle = tint; pen.fillText(text, left, py);
    pen.globalAlpha = 1;
  };
  // The worlds that have a line of their own: the trunk, and each that has
  // parted from the line it branches from. The followed one is drawn last.
  const lined = open.filter(letter => worlds[letter].parent === null || worlds[letter].parted)
    .sort((one, other) => (one === follow) - (other === follow));
  const where = letter => turn => [x(letter, turn), y(turn)];
  const begins = letter => worlds[letter].parent === null ? first : worlds[letter].fork;

  // What has not been played is not drawn as if it had been (operator
  // 2026-11-24: "we should also not spew random lines on the timeline").
  // From each world's last turn there is only the way straight on, dashed.
  // Between the two is what was foreseen: the world's future as it has been
  // played out so far on a copy (`strangeForesee`), a thin line as far as
  // that has come, which stops where the game ends in it.
  for (const letter of lined) {
    const tip = worlds[letter].last, sight = split.panes[letter].sight;
    const seen = sight && typeof sight.turn === "number" ? Math.max(tip, Math.min(sight.turn, end)) : tip;
    if (seen > tip && path(tip, Math.min(seen, high), where(letter))) {
      pen.strokeStyle = colour[letter]; pen.lineWidth = 1.75; pen.globalAlpha = over === null || over === letter ? .7 : .3;
      pen.stroke();
      pen.globalAlpha = 1;
    }
    if ((sight && sight.done) || seen >= end || seen > high) continue;
    pen.setLineDash([3, 7]);
    pen.lineDashOffset = -(time / 60) % 10;
    if (path(seen, Math.min(end, high), where(letter))) stroke(colour[letter], 1.5, over === null || over === letter ? .55 : .25);
    pen.setLineDash([]);
  }

  // The tree as played: the trunk, and a branch for each world from the
  // turn it parts on. A turn ahead of the open one that a world holds is
  // played, its future as its player strategy makes it, and is drawn so.
  for (const letter of lined)
    if (path(begins(letter), worlds[letter].last, where(letter))) stroke(colour[letter], letter === follow ? 4.5 : 3.5, quiet(letter));
  // The line of the world under the pointer, from the first turn to its
  // last: up the trunk and out along its branch, lit.
  if (over !== null && path(first, worlds[over].last, where(over))) {
    pen.strokeStyle = colour[over];
    pen.globalAlpha = .28; pen.lineWidth = 20; pen.stroke();
    pen.globalAlpha = 1; pen.lineWidth = 6.5; pen.stroke();
    pen.strokeStyle = "#ffffff"; pen.globalAlpha = .85; pen.lineWidth = 1.5; pen.stroke();
    pen.globalAlpha = 1;
  }
  // Each line is named where it ends, when that is ahead of the open turn,
  // with the score its world has there. On the open turn its name is on its
  // box, under the line.
  pen.font = "700 11px system-ui, sans-serif";
  for (const letter of lined) {
    const tip = Math.min(worlds[letter].last, high), py = y(tip);
    if (py > g.cy - 30) continue;
    const there = (split.panes[letter].ahead || new Map()).get(worlds[letter].last) || split.history[letter].get(worlds[letter].last);
    const score = there && typeof there.score === "number" ? ` · ${Math.round(there.score)}` : "";
    const side = strange.lanes[letter] > 0 ? 1 : -1;
    write(`${strangeSeries(letter, tree)}${score}`, x(letter, tip), py - 12, side, colour[letter], quiet(letter));
    // Over the name, how many turns of this line are played ahead of its
    // world's open turn (operator 2026-11-26: "add precomputed notes to the
    // map above holy roller (17 precomputed)").
    const ahead = worlds[letter].last - (split.panes[letter].turn ?? tree.from);
    if (ahead > 0) {
      pen.font = "600 10px system-ui, sans-serif";
      write(`${ahead} precomputed`, x(letter, tip), py - 25, side, colour[letter], quiet(letter) * .8, false);
      pen.font = "700 11px system-ui, sans-serif";
    }
  }
  // A city founded is a gem on the line that founded it.
  for (const letter of lined) {
    let before = null;
    for (const turn of [...split.history[letter].keys()].sort((one, other) => one - other)) {
      const cities = (split.history[letter].get(turn) || {}).cities;
      if (typeof cities !== "number") continue;
      if (before !== null && cities > before && turn >= low && turn <= high && turn > begins(letter) && turn <= worlds[letter].last) {
        pen.save();
        pen.translate(x(letter, turn), y(turn)); pen.rotate(Math.PI / 4);
        pen.fillStyle = "#fff6d8"; pen.strokeStyle = "#7a5a18"; pen.lineWidth = 1;
        pen.fillRect(-4, -4, 8, 8); pen.strokeRect(-4, -4, 8, 8);
        pen.restore();
      }
      before = cities;
    }
  }
  // What is coming, on the line it is coming on, at the turn it happens: a
  // sign that comes down the board as the turns are played. A nuclear
  // detonation is always named; the rest are named where there is room.
  pen.font = "700 11px system-ui, sans-serif";
  const named = [];
  for (const letter of open) {
    const tip = worlds[letter].last, sight = split.panes[letter].sight;
    // What the turns it holds saw, and past them what its future played out
    // to the end has seen (`strangeForesee`). Far off, where the turns are
    // close together, a sign that would stand on the last of its kind is
    // left out: a nuclear detonation never is.
    const coming = [...(split.panes[letter].alerts || []).filter(alert => alert.turn < tip),
      ...(sight ? sight.seen.filter(seen => seen.turn >= tip) : [])];
    // Up at the top of the tree, under the worlds' numbers, nothing is
    // named: the predicted paths say what is to come, and the rest is named
    // as it comes down the board.
    const band = (strange.treeTop ?? STRANGE_FATE_TOP) + 10;
    const stood = {};
    for (const alert of coming) {
      if (alert.turn < cam - .5 || alert.turn > high) continue;
      const at = alert.turn + .5, px = x(letter, at), py = y(at);
      if (py > g.cy - 6 || py < (strange.treeTop ?? 14)) continue;
      const [sign, tint] = STRANGE_SIGNS[alert.kind] || ["!", STRANGE_GOLD], grave = alert.kind === "nuclear";
      if (stood[alert.kind] !== undefined && Math.abs(stood[alert.kind] - py) < (grave ? 3 : 16)) continue;
      stood[alert.kind] = py;
      if (grave) {
        pen.strokeStyle = tint; pen.lineWidth = 2; pen.globalAlpha = .25 + .35 * (.5 + .5 * Math.sin(time / 220));
        pen.beginPath(); pen.arc(px, py, 15 + 3 * Math.sin(time / 220), 0, Math.PI * 2); pen.stroke();
      }
      pen.globalAlpha = quiet(letter);
      pen.fillStyle = "#07110f"; pen.strokeStyle = tint; pen.lineWidth = 2;
      pen.beginPath(); pen.arc(px, py, 10, 0, Math.PI * 2); pen.fill(); pen.stroke();
      pen.fillStyle = tint; pen.textAlign = "center";
      pen.fillText(sign, px, py + .5);
      const side = strange.lanes[letter] > 0 || (strange.lanes[letter] === 0 && named.length % 2 === 1) ? 1 : -1;
      if (py > band && (grave || !named.some(([taken, sided]) => sided === side && Math.abs(taken - py) < 14))) {
        named.push([py, side]);
        const said = alert.kind === "war" ? `War: ${String(alert.text || "").replace(" declares war on ", " on ")}` : alert.text || alert.kind;
        write(`Turn ${alert.turn} · ${said}`, px + side * 4, py, side, tint, quiet(letter), false);
      }
      pen.globalAlpha = 1;
    }
  }

  // A black swan is a mark on the line it struck, on the turn it struck: a
  // black disc flagged out to the side of the line, clear of the world
  // standing there. Worlds that were one world when it struck share the
  // mark. One that is still going on beats, and one that has just struck
  // rings out. Ahead of the open turn, where there is room, it is named.
  pen.font = "700 13px system-ui, sans-serif";
  const flagged = [];
  for (const letter of [...open].sort((one, other) => (other === follow) - (one === follow))) {
    for (const swan of strangeSwans(letter)) {
      if (swan.turn < low || swan.turn > high || swan.turn > worlds[letter].last || swan.turn < worlds[letter].first) continue;
      const px = x(letter, swan.turn), py = y(swan.turn);
      if (py < 14 || py > g.H - 14 || flagged.some(([fx, fy]) => Math.abs(fx - px) < 3 && Math.abs(fy - py) < 3)) continue;
      flagged.push([px, py]);
      const side = strange.lanes[letter] > 0 ? 1 : -1, fx = px + side * (Math.abs(py - g.cy) < 34 ? 46 : 24);
      const fresh = !!news && struckFor < 2600 && news.swan.turn === swan.turn && news.letters.includes(letter);
      pen.globalAlpha = quiet(letter);
      pen.strokeStyle = STRANGE_SWAN_INK; pen.lineWidth = 1.5;
      pen.beginPath(); pen.moveTo(px, py); pen.lineTo(fx, py); pen.stroke();
      if (fresh) {
        const out = struckFor / 2600;
        pen.globalAlpha = (1 - out) * .8; pen.lineWidth = 2.5;
        pen.beginPath(); pen.arc(fx, py, 12 + out * 70, 0, Math.PI * 2); pen.stroke();
      } else if (swan.ongoing) {
        pen.globalAlpha = .25 + .3 * (.5 + .5 * Math.sin(time / 320)); pen.lineWidth = 2;
        pen.beginPath(); pen.arc(fx, py, 17 + 2 * Math.sin(time / 320), 0, Math.PI * 2); pen.stroke();
      }
      pen.globalAlpha = quiet(letter);
      pen.fillStyle = "#000"; pen.lineWidth = 2;
      pen.beginPath(); pen.arc(fx, py, 12, 0, Math.PI * 2); pen.fill(); pen.stroke();
      pen.fillStyle = STRANGE_SWAN_INK; pen.textAlign = "center";
      pen.fillText(STRANGE_SWAN, fx, py + 1);
      pen.font = "700 11px system-ui, sans-serif";
      if (py < g.cy - 20) write(`Turn ${swan.turn} · ${swan.name}`, fx + side * 8, py, side, STRANGE_SWAN_INK, quiet(letter), false);
      pen.font = "700 13px system-ui, sans-serif";
      pen.globalAlpha = 1;
    }
  }

  // The line of the open turn, across the window.
  const line = pen.createLinearGradient(0, 0, g.W, 0);
  line.addColorStop(0, colour[open[0]]); line.addColorStop(.5, STRANGE_GOLD); line.addColorStop(1, colour[open[open.length - 1]]);
  pen.strokeStyle = line; pen.globalAlpha = .28; pen.lineWidth = 7;
  pen.beginPath(); pen.moveTo(0, g.cy); pen.lineTo(g.W, g.cy); pen.stroke();
  pen.globalAlpha = .95; pen.lineWidth = 1.5;
  pen.beginPath(); pen.moveTo(0, g.cy); pen.lineTo(g.W, g.cy); pen.stroke();
  pen.globalAlpha = 1;

  // Where each world stands. Worlds that are one world so far stand as one.
  // The followed one has the circle of the spell about it, turning.
  pen.font = "700 12px Georgia, serif"; pen.textAlign = "center";
  const stands = [];
  for (const letter of [...open].sort((one, other) => (one === follow) - (other === follow))) {
    const turn = turnOf(letter), px = x(letter, turn), py = y(turn);
    const held = stands.find(stand => Math.abs(stand.px - px) < 3 && Math.abs(stand.py - py) < 3);
    if (held) held.letters.push(letter); else stands.push({px, py, letters: [letter]});
  }
  for (const {px, py, letters} of stands) {
    const mine = letters.includes(follow), tint = colour[mine ? follow : letters[0]];
    if (over !== null && letters.includes(over)) {
      pen.strokeStyle = colour[over]; pen.globalAlpha = .5 + .3 * Math.sin(time / 180); pen.lineWidth = 3;
      pen.beginPath(); pen.arc(px, py, 15, 0, Math.PI * 2); pen.stroke();
      pen.globalAlpha = 1;
    }
    if (mine) {
      pen.strokeStyle = STRANGE_GOLD;
      for (const [radius, dash, speed] of [[19, [2, 5], 1], [26, [9, 4, 2, 4], -.6]]) {
        pen.globalAlpha = .75; pen.lineWidth = 1.5; pen.setLineDash(dash); pen.lineDashOffset = time / 1000 * 18 * speed;
        pen.beginPath(); pen.arc(px, py, radius, 0, Math.PI * 2); pen.stroke();
      }
      pen.setLineDash([]); pen.globalAlpha = 1;
    }
    const text = letters.slice().sort().join("").toUpperCase();
    pen.fillStyle = "#07110f"; pen.strokeStyle = tint; pen.lineWidth = 2.5;
    pen.beginPath(); pen.arc(px, py, text.length > 2 ? 14 : 11, 0, Math.PI * 2); pen.fill(); pen.stroke();
    pen.fillStyle = tint;
    pen.fillText(text.length > 3 ? `${text.slice(0, 2)}+` : text, px, py + .5);
  }

  // Under the pointer: the turn that height stands for, and what a press does.
  const hover = strange.hover;
  if (hover && strangeIdle()) {
    const turn = Math.max(first, Math.min(end, Math.round(strangeTurnAt(g, hover.y, cam)))), at = y(turn);
    const played = Math.max(...open.map(letter => worlds[letter].last));
    if (turn !== turnOf(follow) && at > 8 && at < g.H - 8) {
      const wide = half(at);
      pen.globalAlpha = .9; pen.strokeStyle = "#ffffff"; pen.lineWidth = 1;
      pen.beginPath(); pen.moveTo(g.cx - wide, at); pen.lineTo(g.cx + wide, at); pen.stroke();
      pen.font = "600 11px system-ui, sans-serif"; pen.textAlign = "center"; pen.fillStyle = "#ffffff";
      pen.fillText(turn > played ? `Play to turn ${turn}` : `Go to turn ${turn}`, g.cx, at - 10);
      pen.globalAlpha = 1;
    }
  }
}

if (STRANGE_PANE) {
  document.documentElement.classList.add("strange-pane");
  const style = document.createElement("style");
  style.id = "strange-pane-style";
  style.textContent = STRANGE_PANE_STYLE;
  document.head.appendChild(style);
  // A world's box shows its map and its chart and nothing else of the page
  // (`STRANGE_PANE_STYLE` puts the rest away), and every world draws on the
  // one thread of the page that holds them. What is out of sight is not
  // drawn or measured: the debugger's bar (its deck and marks are still
  // worked out and told to the host, `drawDebugBar`), the civilizations'
  // panel, the heights the corner boxes publish, and the fitting of type in
  // panels nobody sees. Three worlds spent about a fifth of the thread on
  // these, every turn.
  // (The viewer's own, where the page has them.)
  debugDock = () => {};
  if (typeof drawPlayerHud === "function") drawPlayerHud = () => {};
  // The page that holds the worlds draws the standings and the victory
  // tracker of the one it follows, from the world as this page holds it
  // (`strangeStandingsDraw`).
  window.strangeWorldNow = () => state;
  // A world whose game is over is not started again on a countdown: that
  // would end the world (`strangeWorldFate`).
  if (typeof finaleHoldsForTheReader === "function") finaleHoldsForTheReader = () => true;
  if (typeof publishSoloHeight === "function") publishSoloHeight = () => {};
  // Nor are a played seat's chores at the top of a turn done for a box
  // nobody plays by hand: picking the unit that wants orders and flying the
  // map to it (`advanceToNextActionUnit`, which drew the map a second time
  // every turn), and the leader ribbon and the pause control a box never
  // shows (`drawSoloHud`, `paintLivePause`, `placeLivePause`). With six
  // worlds these were about a third of what the worlds' pages spent on a
  // turn; four worlds played 11 to 22% more turns a second without them.
  // Nor is the minimap a box no longer shows drawn (`drawMini`).
  // Nor is the build's label placed beside the minimap, both of which a box
  // hides (`updateBuildMarker`, `syncBuildMarkerPosition`): placing it
  // measured the page every turn, the browser laying it out to say where the
  // minimap stood. With ten worlds that was 3% of the thread they share.
  for (const name of ["advanceToNextActionUnit", "drawSoloHud", "paintLivePause", "placeLivePause", "drawMini",
    "updateBuildMarker", "syncBuildMarkerPosition"])
    if (typeof window[name] === "function") window[name] = () => {};
  if (typeof responsiveTypeMutationObserver !== "undefined") responsiveTypeMutationObserver.disconnect();
  // While the worlds move, a world draws a turn once, when it is handed it
  // (`render`), and plays none of the motion in between. At four or five
  // turns a second a turn's units, floating numbers, labels and camera
  // flight never finish before the next turn begins, so every world kept
  // its animation running all the time and drew its map seven or eight
  // times a turn, as if the thread were its own: with three worlds a
  // quarter of the thread they all share. Its camera is meanwhile where it
  // is going at once, as for a page that asked for reduced motion. Standing
  // still, a world moves as any page does, and a person moving a world's
  // map keeps its frames.
  const moving = () => { try { return !window.parent.strangeIdle(); } catch (_) { return false; } };
  if (typeof REDUCED_MOTION_QUERY !== "undefined") {
    const asked = Object.getOwnPropertyDescriptor(MediaQueryList.prototype, "matches").get;
    Object.defineProperty(REDUCED_MOTION_QUERY, "matches", {get: () => moving() || asked.call(REDUCED_MOTION_QUERY)});
  }
  if (typeof animTick === "function") {
    let touched = -Infinity;
    const touch = event => { if (event.type !== "pointermove" || event.buttons) touched = performance.now(); };
    for (const kind of ["pointerdown", "pointermove", "wheel", "keydown"]) addEventListener(kind, touch, {capture: true, passive: true});
    const tick = animTick;
    animTick = function (now) {
      if (now - touched > 1500 && moving()) { requestAnimationFrame(animTick); return; }
      return tick(now);
    };
  }
  // Nor, while the worlds move, is the hidden minimap's frame fitted to the
  // box at each turn drawn (`syncDefaultMinimapGeometry`, which measured the
  // page to place it: 4% of the thread with ten worlds). A box fits it again
  // at the first turn drawn once the worlds stand still.
  if (typeof syncDefaultMinimapGeometry === "function") {
    const fit = syncDefaultMinimapGeometry;
    syncDefaultMinimapGeometry = function (...given) { if (!moving()) return fit.apply(this, given); };
  }
}
if (STRANGE) {
  document.documentElement.classList.add("strange");
  // The split is made by the debugger and drawn by it. The highway is made
  // with it, and is drawn again whenever it is.
  const open = debugSplitOpen;
  debugSplitOpen = function (...given) {
    const before = debugSplit;
    const opened = open(...given);
    if (debugSplit && debugSplit !== before) strangeMount();
    return opened;
  };
  setInterval(() => { try { strangeSpare(); } catch (error) { console.error("doctor strange", error); } }, STRANGE_SPARE_EVERY_MS);
  // The standings and the victory tracker are the world followed
  // (`strangeStandingsDraw`): the simulator's own drawing, given that world
  // to read for as long as it draws. Until a world can be read it draws the
  // page's own, which is the same game.
  // Watch as is not a column of them here: it turns the simulator's own map
  // to a civilization's eyes, and that map is not shown.
  if (typeof playerHudColumnExists === "function") {
    const exists = playerHudColumnExists;
    playerHudColumnExists = column => column.key !== "watch" && exists(column);
  }
  if (typeof drawPlayerHud === "function") {
    const drawStandings = drawPlayerHud;
    drawPlayerHud = function () {
      if (strangePanels && !strangePanels.players.isConnected) strangeStandingsHome();
      const world = strange.standings && strangePanels && strangePanels.players.isConnected ? strangeFollowedWorld() : null;
      if (!world) return drawStandings();
      const own = state;
      state = world;
      try { return drawStandings(); } finally { state = own; }
    };
  }
  // What the worlds do next is worked out at once, every time; the split
  // and the highway are drawn once a frame. Each world says several things
  // a turn (its turn, its marks, its numbers, that it followed, that it is
  // ready), the host was drawn again for every one of them, and every
  // drawing measures the page: with three worlds that was a score of
  // layouts a turn on the one thread all of them draw on. A tab out of
  // sight paints no frames, so a short timer draws it as well.
  const draw = drawDebugSplit;
  let drawing = null;
  drawDebugSplit = function () {
    try { strangeStep(); } catch (error) { console.error("doctor strange", error); }
    if (drawing) return;
    const now = () => {
      if (!drawing) return;
      cancelAnimationFrame(drawing.frame);
      clearTimeout(drawing.timer);
      drawing = null;
      draw();
      try { strangeDraw(); } catch (error) { console.error("doctor strange", error); }
    };
    drawing = {frame: requestAnimationFrame(now), timer: setTimeout(now, 50)};
  };
  // With no room for two worlds (a small device, a narrow window) the game
  // stays on its one timeline, which is the AI Debugger, and the bar says so.
  const tracks = debugOpenTheGame;
  debugOpenTheGame = function (...given) {
    const opened = tracks(...given);
    if (!debugSplit && !strange.sorry && debugOpenedGame !== null && debugTimeline()) {
      strange.sorry = true;
      debugSay("Doctor Strange's CIVVIS sets two worlds side by side and needs a wider window. This is the CIVVIS AI Debugger, on one timeline");
    }
    return opened;
  };
}

// `render` is the one door a state comes through, whoever asked for it.
// `drawTurnLoop` is the page's own word that the turn's standing changed —
// an end of turn in flight, a request finished — which is exactly when the
// bar's buttons change what they may do, with no new state to say so.
{
  const drawState = render;
  render = function (st, ...rest) {
    const drawn = drawState(st, ...rest);
    try { drawDebugBar(); debugPaneReport(); debugPaneNumbers(); strangePaneResult(); debugOpenTheGame(); } catch (error) { console.error("debugger bar", error); }
    return drawn;
  };
  // In the middle of a round stepped a seat at a time the turn is not the
  // person's. Ending it means letting the rest of the round play, and
  // nothing else is theirs to do until it comes back.
  const sendAction = send;
  send = function (action, ...rest) {
    const timeline = debugTimeline();
    // The world a split is being made from stays as it is until it is made.
    if (timeline && debugHeld) return Promise.resolve();
    if (!timeline || !timeline.playing) return sendAction(action, ...rest);
    if (action && action.type === "end_turn")
      return debugOp({op: "finish_round"}, AUTOPLAY_BATCH_TIMEOUT_MS);
    debugSay(`${debugPlaysNext(timeline.playing)}; your turn comes back when the round finishes`);
    return Promise.resolve();
  };
  const drawLoop = drawTurnLoop;
  drawTurnLoop = function (...rest) {
    const drawn = drawLoop(...rest);
    try { drawDebugBar(false); } catch (error) { console.error("debugger bar", error); }
    return drawn;
  };
}
// Ctrl/Cmd+Z takes back the last action, where the hand already expects it.
window.addEventListener("keydown", event => {
  if (!(event.ctrlKey || event.metaKey) || event.altKey || event.shiftKey || event.key.toLowerCase() !== "z") return;
  if (event.target instanceof HTMLElement && event.target.matches("input, textarea, select")) return;
  // Under a split this page's own world waits: the key is the halves', through the one panel.
  if (debugSplit) {
    const deck = debugSplitDeck();
    if (deck && deck.timeline.can_undo) { event.preventDefault(); debugRemote("pane-press", debugBar.querySelector('[data-dbg="undo"]')); }
    return;
  }
  const timeline = debugTimeline();
  if (!timeline || !timeline.can_undo || autoplaying) return;
  event.preventDefault();
  debugUndo();
});
window.addEventListener("resize", () => { if (debugTimeline()) drawDebugBar(); });
// A page whose first state was drawn before this script arrived has had no
// `render` of ours yet: draw the bar now, and a half of a split says where it
// stands, or its host would wait for a report until something moved.
if (state) { drawDebugBar(); debugPaneReport(); debugPaneNumbers(); strangePaneResult(); debugOpenTheGame(); }

// ═══════════════════════════════════════════════════════════════════════════
// Edge of Tomorrow
// ═══════════════════════════════════════════════════════════════════════════
//
// Operator 2026-11-25: "above civ 6 and below ai debugger add: Edge of
// Tomorrow. This should be civ 6 with a jump back (like the concept from the
// movie)".
//
// `/edge-of-tomorrow` is a played game with the seat in the person's own hands,
// on the Civilization VI screen, and one thing that game does not have: a
// jump back. The world goes back to the turn the person woke on, exactly as
// it stood, and they play it again knowing what they saw. Each time round is
// a loop, and the loops are counted.
//
// A jump back comes two ways. The person asks for one (Jump back, in the bar
// across the bottom), or the game is lost: the seat's civilization falls, or
// somebody else wins. A lost game does not end here. Its card says how the
// loop ended and counts down to the jump.
//
// The world a jump goes back to is the engine's own mark of that turn
// (`server/debugger.rs`, `jump`), so nothing is played again to get there.
// The engine is deterministic and keeps the agents beside each mark, so the
// rivals play the same game again until the person plays a different one,
// which is what makes what was seen worth keeping. The bar keeps it: its
// line is the whole game, with where each earlier loop ended and, ahead of
// the open turn, what earlier loops saw done to the seat (a war declared on
// it, a city of its taken), read from the game's own record of its wars
// (`state.wars`).
//
// The turn woken on is the first until the person moves it (Checkpoint here). The
// world as it stands is then kept as a save point of the engine's
// (`save_point`), which is never thinned away as an old turn's mark may be,
// and a jump back goes to that (`goto_save`).
//
// Nothing here is the engine's. It is this page counting, over the timeline
// every played game keeps, and it is gone with the page, as save points are.
// The debugger's own bar is out of sight (`html.tomorrow #debugbar`), and
// the seat's Auto-play, which that bar takes over in the AI Debugger, stays
// where a played game has it, in the left panel.

// The name the wake point is kept under among the engine's save points. It
// is one name: a world kept under it again replaces the one before.
const TOMORROW_SAVE = "Edge of Tomorrow";
// How long a lost loop's card stands before the jump back, in seconds.
const TOMORROW_COUNT = 8;
// What is done to the seat in a war, as the game records it
// (`WarHighlight::kind`): the sign and the colour of its mark on the line,
// and the words for it.
const TOMORROW_SEEN = {
  declared: ["⚔", "#ffb347", (actor, seat) => `${actor} declared war on ${seat}`],
  city_captured: ["⌂", "#ff8f6b", (actor, seat, city) => `${actor} captured ${city || `a city of ${seat}`}`],
  capital_captured: ["♛", "#ff5a4d", (actor, seat, city) => `${actor} captured ${city ? `${city}, ` : ""}the capital of ${seat}`],
  city_razed: ["✹", "#ff5a4d", (actor, seat, city) => `${actor} razed ${city || `a city of ${seat}`}`],
  nuclear_strike: ["☢", "#ff5a4d", (actor, seat, city) => `${actor} struck ${city || seat} with a nuclear weapon`],
  conquest: ["✕", "#ff5a4d", (actor, seat) => `${actor} conquered ${seat}`],
};

const TOMORROW_STYLE = `
  /* Operator 2026-11-26: "lets move the civvis: edge of tomorrow top bar to
     the bottom of the screen. the yield bar and other stuff can stay on
     top". The bar stands across the foot of the window from where the map
     area begins (measured, --tmr-left) to the right edge, and the map area
     ends above it by the height it takes (--tmr-bottom): the body is a row
     of flex items, so a margin at the foot of the map area takes that much
     off its height, and whatever stands at the map area's foot stands on
     the bar. On a narrow window the map area is a block a whole window
     tall, and the floating deck stops above the bar. */
  html.tmr-docked #maparea { margin-bottom: var(--tmr-bottom, 0px); }
  @media (max-width: 720px) {
    html.tmr-docked #maparea { height: calc(100% - var(--tmr-bottom, 0px)); margin-bottom: 0; }
    html.tmr-docked #side { bottom: calc(8px + var(--tmr-bottom, 0px)); max-height: calc(100dvh - 140px - var(--tmr-bottom, 0px)); }
  }
  #tomorrowbar {
    position: fixed; z-index: 8000; inset: auto 0 0 var(--tmr-left, 0px);
    display: grid; grid-template-columns: auto minmax(0, 1fr) auto; align-items: center; gap: 6px 16px;
    padding: 8px 12px; color: #dfe6ec; font-size: 12px; line-height: 1.2; user-select: none;
    background: linear-gradient(180deg, #131b23, #0a0f14); border-top: 1px solid #44525f; border-left: 1px solid #2b3742;
    box-shadow: 0 -6px 18px #0008;
  }
  #tomorrowbar[hidden] { display: none !important; }
  /* The name, and under it the loop this is. */
  #tomorrowbar .tmr-name { display: grid; gap: 3px; white-space: nowrap; }
  #tomorrowbar .tmr-name b { color: #9fb0bf; font-size: 10px; font-weight: 800; letter-spacing: .16em; text-transform: uppercase; }
  #tomorrowbar .tmr-loop { display: flex; align-items: baseline; gap: 6px; color: #9fb0bf; font-size: 10px; font-weight: 800; letter-spacing: .16em; text-transform: uppercase; }
  #tomorrowbar .tmr-loop i { color: #f5a742; font: 700 24px/1 Georgia, serif; font-style: normal; letter-spacing: 0; font-variant-numeric: tabular-nums; }
  /* The line is the whole game, its first turn at the left end. */
  #tomorrowbar .tmr-track { display: grid; gap: 4px; min-width: 0; }
  #tomorrowbar .tmr-rail { position: relative; height: 38px; border-radius: 7px; background: #0a1118; border: 1px solid #2f3d4a; }
  #tomorrowbar .tmr-rail > * { position: absolute; }
  /* What earlier loops lived through, from the turn woken on to the furthest
     any of them reached, and along its foot what this loop has played. */
  #tomorrowbar .tmr-lived { top: 0; bottom: 0; border-radius: 6px; background: repeating-linear-gradient(135deg, #1b3144 0 6px, #16283a 6px 12px); }
  #tomorrowbar .tmr-done { bottom: 0; height: 4px; border-radius: 0 2px 2px 6px; background: linear-gradient(90deg, #8a5a18, #f5a742); }
  #tomorrowbar .tmr-tick { top: 0; bottom: 0; padding: 1px 0 0 3px; border-left: 1px solid #ffffff1a; color: #8493a0; font-size: 9px; font-variant-numeric: tabular-nums; pointer-events: none; }
  /* What earlier loops saw coming stands in the upper half of the line, and
     where an earlier loop ended in the lower, so that a loop that ended in
     the war it saw shows both, one over the other. */
  #tomorrowbar .tmr-end, #tomorrowbar .tmr-mem {
    width: 16px; height: 16px; margin-left: -8px; display: grid; place-items: center;
    font-style: normal; font-size: 15px; line-height: 1; text-shadow: 0 0 2px #000, 0 0 4px #000; cursor: default;
  }
  #tomorrowbar .tmr-mem { top: 1px; }
  #tomorrowbar .tmr-end { bottom: 4px; color: #ff6b5e; font-weight: 800; }
  #tomorrowbar .tmr-end.tmr-by-hand { color: #c3ced7; }
  /* Where a jump back wakes, and the turn that is open. */
  #tomorrowbar .tmr-wake { top: -4px; bottom: -4px; width: 4px; margin-left: -2px; border-radius: 2px; background: #eef3f7; box-shadow: 0 0 6px #eef3f7aa; }
  #tomorrowbar .tmr-now {
    top: 50%; min-width: 30px; height: 22px; padding: 0 6px; transform: translate(-50%, -50%); display: grid; place-items: center;
    color: #1a1206; background: #f5a742; border: 1px solid #ffd9a0; border-radius: 11px;
    font: 800 12px/1 system-ui, sans-serif; font-variant-numeric: tabular-nums; box-shadow: 0 2px 8px #000a;
  }
  #tomorrowbar .tmr-said { min-height: 14px; color: #9fb0bf; font-size: 11px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  #tomorrowbar .tmr-said b { color: #dfe6ec; font-weight: 700; }
  /* Operator 2026-11-26: "on the right, the biuttons shold be: (top row):
     turns, civ ai (bottom row): jump back, auto-play N turns, wake here. new
     game remains in the left panel." The left panel's Auto-play (its two
     pickers, its button and its note, \`#autoplaybar\`) is moved in here
     whole by \`tomorrowMount\`, so the page drives it as it always has; its
     boxes give way (\`display: contents\`) and each piece takes its place in
     the grid: Turns over Jump back, CIVVIS AI over Auto-play and Checkpoint
     here.
     Operator 2026-11-26: "lets make sure the buttons in the lower right
     aren't moving around when simming". The columns are fixed: Auto-play's
     words change as it plays ("Stop · 27 turns left", then "1 turn left"),
     and a column as wide as its words moved every button and picker beside
     it a few pixels each turn. Each button now fills its column, its words
     centred in it, so nothing moves. And "make the turn selection and
     (rename to CIVVIS AI:) things a bit large/taller": the two pickers are
     36px tall in 14px type, their names 11px. */
  #tomorrowbar .tmr-acts {
    display: grid; grid-template-columns: 150px 204px 150px; align-items: center; gap: 6px 8px;
    grid-template-areas: "turns ai ai" "jump auto wake";
  }
  #tomorrowbar #autoplaybar, #tomorrowbar .autoplay-fields { display: contents; }
  #tomorrowbar .autoplay-field { display: flex; align-items: center; gap: 7px; min-width: 0; }
  #tomorrowbar .autoplay-field > span { flex: none; color: #9fb0bf; font-size: 11px; font-weight: 800; letter-spacing: .14em; text-transform: uppercase; }
  #tomorrowbar .autoplay-field select { flex: 1; min-width: 0; height: 36px; padding: 0 8px; font-size: 14px; }
  #tomorrowbar .tmr-turns { grid-area: turns; }
  #tomorrowbar .tmr-ai { grid-area: ai; }
  #tomorrowbar #autoplaybtn {
    grid-area: auto; width: 100%; min-width: 0; height: 40px; padding: 0 10px; border-radius: 10px; font-size: 13px;
    overflow: hidden; text-overflow: ellipsis; font-variant-numeric: tabular-nums;
  }
  #tomorrowbar #autoplaynote { display: none; }
  #tmr-jump { grid-area: jump; }
  #tmr-wake { grid-area: wake; }
  #tomorrowbar button { font: inherit; cursor: pointer; white-space: nowrap; }
  #tomorrowbar button:disabled { cursor: default; opacity: .38; }
  #tmr-jump {
    display: grid; justify-items: center; gap: 1px; width: 100%; min-width: 0; height: 40px; padding: 0 12px; border-radius: 10px;
    color: #1a1206; background: linear-gradient(180deg, #f9b858, #d98a1e); border: 1px solid #ffd9a0;
    box-shadow: 0 2px 10px #f5a74233;
  }
  #tmr-jump span { align-self: end; font-size: 14px; font-weight: 800; letter-spacing: .06em; text-transform: uppercase; }
  #tmr-jump small { align-self: start; font-size: 10px; font-weight: 700; letter-spacing: .04em; opacity: .8; font-variant-numeric: tabular-nums; }
  #tmr-jump:hover:not(:disabled) { background: linear-gradient(180deg, #ffc978, #e89a2c); }
  #tmr-jump:disabled { filter: grayscale(.7); box-shadow: none; }
  /* A lost loop that is being looked around waits on this button. */
  #tmr-jump.tmr-due { animation: tmr-due 1.4s ease-in-out infinite; }
  @keyframes tmr-due { 50% { box-shadow: 0 0 0 4px #f5a74255, 0 2px 14px #f5a74288; } }
  @media (prefers-reduced-motion: reduce) { #tmr-jump.tmr-due { animation: none; box-shadow: 0 0 0 3px #f5a74266; } }
  #tmr-wake {
    width: 100%; min-width: 0; height: 40px; padding: 0 10px; border-radius: 10px; color: #c9d2da; background: #141c24; border: 1px solid #44525f;
    font-size: 11px; font-weight: 700; letter-spacing: .04em; overflow: hidden; text-overflow: ellipsis;
  }
  #tmr-wake:hover:not(:disabled) { color: #fff; border-color: #eef3f7; }
  /* With little room the name goes over the line, beside the buttons. */
  @media (max-width: 900px) {
    #tomorrowbar { grid-template-columns: minmax(0, 1fr) auto; }
    #tomorrowbar .tmr-name { grid-auto-flow: column; justify-content: start; align-items: baseline; gap: 12px; }
    #tomorrowbar .tmr-track { grid-column: 1 / -1; grid-row: 2; }
    #tomorrowbar .tmr-acts { grid-template-columns: 124px 172px 124px; }
    #tmr-jump { height: 36px; padding: 0 8px; }
    #tmr-jump span { font-size: 12px; }
    #tmr-wake { height: 36px; padding: 0 6px; }
    #tomorrowbar #autoplaybtn { height: 36px; padding: 0 6px; font-size: 12px; }
  }
  /* The card of a lost loop: the result card's own shape, with the jump
     where a new game was offered. */
  #winner .tmr-card .winner-actions { grid-template-columns: minmax(0, 1fr); justify-items: center; }
  #winner .tmr-card .winner-actions .winner-again { width: auto; }
  html.tmr-looking #winner { display: none !important; }
  /* Operator 2026-11-26: "civ autoplay should be gold and new game grey. in
     civ edge of tomorrow". The left panel's two buttons trade places: the
     seat's Auto-play takes the page's gold (its \`button.primary\`), and
     Start new game, here and at the foot of Game Setup, the page's plain
     grey (its \`button\`). Auto-play stays gold while it runs, as Stop. */
  html.tomorrow .spec-controls #restart-sim.primary, html.tomorrow #setup-start.primary {
    color: #c7ccca; background: linear-gradient(180deg, #23272a, #171a1d); border-color: #42484c; box-shadow: none;
  }
  html.tomorrow .spec-controls #restart-sim.primary:hover, html.tomorrow #setup-start.primary:hover {
    color: var(--text); background: linear-gradient(180deg, #2d3236, #202428); border-color: #5b6368;
  }
  html.tomorrow .spec-controls #restart-sim.primary .sub { color: var(--muted); }
  html.tomorrow .autoplay-bar #autoplaybtn {
    color: #17130c; background: linear-gradient(135deg, #d8b663, #b58435); border-color: #d8b763;
    box-shadow: 0 7px 22px #0006, inset 0 1px #fff4; font-weight: 850;
  }
  html.tomorrow .autoplay-bar #autoplaybtn:hover {
    color: #120f09; background: linear-gradient(135deg, #e2c26f, #c18f3e); border-color: #e2c370;
  }
  /* Operator 2026-11-26: "for edge of tomorrow, remove all the on screen
     controls. a player's only controls are the auto play stuff and the jump
     back / wake here. i guess we should leave the top stuff that is
     providing info. the bottom stuff that player make decisions we gotta
     remove." The unit's commands, the turn's action panel (its choices and
     End turn), the research and civic prompts under the launch bar, and the
     notices that ask for a decision are gone. The top panel, the launch and
     report buttons, the map and the left panel stay, and the seat is moved
     only by Auto-play (\`civvisSeatHandsOff\`). */
  html.tomorrow #ubar, html.tomorrow #actionpanel, html.tomorrow #worldtracker,
  html.tomorrow #notify .notice.tone-action, html.tomorrow #civtop { display: none !important; }
  /* Operator 2026-11-27: "from edge of tomorrow we can remove the old yield
     hud with science, culture, etc." Civ 6's yield strip (\`#civtop\`, in
     the rule above) is gone, and so is the room the screen keeps for it: the
     standings take the top of the window, and the launch bar, the hooks and
     the tracker stand under them, by the screen's own rules, which all count
     the strip's height. */
  html.tomorrow body.civ6-frame #maparea { --civtop-height: 0px; }
  /* Operator 2026-11-26: "civvis: edge of tomortrow should not have the
     world minimap or <...(spin)x thing showing". The minimap and the map's
     controls (deck, compass, zoom, spin, close) are put away, and the map is
     still moved by drag and wheel. Auto-play has left the left panel for the
     bar, so its empty place there goes too. */
  html.tomorrow .minimap-frame, html.tomorrow #map-controls-dock,
  html.tomorrow #humanfooter { display: none !important; }
  /* Operator 2026-11-26: "we should also have the victory tracker and player
     hud showing for this. and the governors / tech / policy screen". The
     standings and the victory tracker open with the game (the Civilization
     VI screen's Rankings, which still folds them), and the seat's research
     box stands as it does in the Debugger view (\`civvisSeatTracker\`),
     without the live mirror's Pause and New game. */
  html.tomorrow .livepause-buttons, html.tomorrow #livepause-note { display: none !important; }
  /* The hooks (Rankings, Diplomacy, Reports, Era, Trade routes, City-States,
     Espionage) stand as on every Civilization VI screen: a row in the upper
     right, in their own order, under the standings (operator 2026-11-27:
     "the bar with espioage should be in the normal order and horizontal").
     They stood in a column beside the tracker from 2026-11-26 ("move the
     menu with quick deals and other stuff inside, to the left of the victory
     tracker"). */
  /* The standings open short here (\`measureTurnPlateNeed\`, below), so the
     turn box is shorter than what it lists. Its rows down to the turn count
     keep their own height rather than giving way (the age, which hides its
     overflow, would fold to nothing and the turn's numeral ride up over it),
     and the box's own edge covers the rest. */
  html.tomorrow #playerhud .victory-turn > :not(.turn-settings) { min-height: min-content; }
`;

const tomorrow = {
  // The game the loops are of, by its server and its seed, and the turn it
  // was last seen on. A turn that goes back unasked is another game.
  game: null, turn: 0,
  loop: 1,
  // Where a jump back wakes: the turn, the engine's save point for it (none
  // while it is the first turn kept), and how much of that turn was played.
  wake: {turn: 1, save: null, acted: 0},
  // The furthest turn any loop has reached.
  far: 1,
  // How each earlier loop ended: {loop, turn, how, words}.
  ends: [],
  // What earlier loops saw done to the seat, and what this one has so far:
  // {turn, kind, text, loop}, by turn, kind, who did it and to which city.
  seen: new Map(), now: new Map(),
  // A jump is on its way to the engine.
  jumping: false,
  // The end this loop has met ({how, words}), the count to the jump back,
  // and whether the person asked to look around first.
  fate: null, count: null, looking: false,
  // What the bar was last told to say, and the turn it was said on.
  said: null,
};
let tomorrowBar = null;

function tomorrowIdle() {
  return !debugBusy && !autoplaying && !turnAdvancePending && !tomorrow.jumping;
}
function tomorrowCiv(id) {
  return ((state.players || [])[id] || {}).civ || "A rival";
}
function tomorrowSay(text) {
  tomorrow.said = text ? {text, turn: state ? state.turn : 0} : null;
}
// How the game has gone for the seat on the world shown: it fell, it lost,
// it won, or (null) the game is still its to play.
function tomorrowFate() {
  const me = (state.players || [])[0];
  if (!me) return null;
  if (!gameFinished(state)) return me.alive === false ? {how: "fell", words: `${me.civ} has been defeated`} : null;
  const winners = Array.isArray(state.winners) && state.winners.length ? state.winners
    : state.winner !== null && state.winner !== undefined ? [state.winner] : [];
  if (winners.includes(0)) return {how: "won", words: `${me.civ} won`};
  if (!winners.length) return {how: "lost", words: "The game ended with no winner"};
  return {how: "lost",
    words: `${winners.map(tomorrowCiv).join(" + ")} won: ${victoryVerdict(state.victory_type, state.victory_label)}`};
}
// Whether there is anything to jump back from: a turn past the one woken on,
// something done in that turn, or a game that is over.
function tomorrowCanJump() {
  const t = tomorrow, timeline = debugTimeline();
  return !!timeline && (state.turn > t.wake.turn || (timeline.acted || 0) !== t.wake.acted || !!t.fate);
}
// The state that was just drawn, read for what the loops are made of.
function tomorrowWatch() {
  const t = tomorrow, timeline = debugTimeline();
  if (!timeline) return;
  const game = `${state.server_instance ?? ""}|${state.seed ?? ""}`;
  if (t.game !== game || (state.turn < t.turn && !t.jumping)) {
    tomorrowCountOff();
    Object.assign(t, {game, loop: 1, wake: {turn: timeline.first ?? state.turn, save: null, acted: 0}, far: state.turn,
      ends: [], seen: new Map(), now: new Map(), fate: null, looking: false, said: null});
  }
  // A wake point whose save point the engine no longer holds (a saved game
  // was loaded over this one) is the first turn again.
  if (t.wake.save !== null && !(timeline.saves || []).some(save => save.id === t.wake.save))
    t.wake = {turn: timeline.first ?? state.turn, save: null, acted: 0};
  t.turn = state.turn;
  t.far = Math.max(t.far, state.turn);
  for (const war of state.wars || []) {
    for (const seen of war.highlights || []) {
      const how = TOMORROW_SEEN[seen.kind];
      if (!how || seen.subject !== 0 || seen.turn < t.wake.turn) continue;
      const key = `${seen.turn}|${seen.kind}|${seen.actor}|${seen.city || ""}`;
      if (!t.now.has(key)) t.now.set(key, {turn: seen.turn, kind: seen.kind, loop: t.loop,
        text: how[2](tomorrowCiv(seen.actor), tomorrowCiv(0), seen.city)});
    }
  }
  const fate = tomorrowFate(), card = document.getElementById("winner");
  if (!fate) {
    tomorrowCountOff();
    t.fate = null; t.looking = false;
    document.documentElement.classList.remove("tmr-looking");
    if (card) delete card.dataset.tomorrow;
    return;
  }
  if (!t.fate) {
    t.fate = fate;
    if (fate.how !== "won" && !t.jumping) tomorrowCountOn();
  }
  tomorrowCard();
}
// A lost loop counts down to its jump back. The jump waits for whatever the
// page is in the middle of.
function tomorrowCountOn() {
  tomorrowCountOff();
  tomorrow.count = {left: TOMORROW_COUNT, timer: setInterval(() => {
    const count = tomorrow.count;
    if (!count) return;
    if (count.left > 0) count.left -= 1;
    tomorrowCardCount();
    if (count.left <= 0 && tomorrowIdle()) tomorrowJump();
  }, 1000)};
}
function tomorrowCountOff() {
  if (tomorrow.count) clearInterval(tomorrow.count.timer);
  tomorrow.count = null;
}
function tomorrowCardCount() {
  const button = document.getElementById("tmr-card-jump");
  if (!button) return;
  const count = tomorrow.count;
  button.textContent = `Jump back to turn ${tomorrow.wake.turn}${count ? ` (${count.left})` : ""}`;
}
// The result card, when the game is over. A game the seat won keeps the
// card it has, which then says which loop won it. A game it lost is not the
// end of this one: the page's count to a new game is stopped, and the card
// is the loop's, with the jump back where a new game was offered. The page
// draws its own card again whenever the result's signature changes, so this
// is asked after every state.
function tomorrowCard() {
  const t = tomorrow, card = document.getElementById("winner");
  if (!card || !t.fate) return;
  const drawn = card.dataset.signature || "";
  if (card.dataset.tomorrow === drawn && card.querySelector(".tmr-card, .tmr-won")) return;
  card.dataset.tomorrow = drawn;
  if (t.fate.how === "won") {
    const turn = card.querySelector(".winner-content > .winner-meta:not(.winner-player)");
    if (turn) turn.insertAdjacentHTML("afterend", `<span class="winner-meta tmr-won">Won on loop ${t.loop}</span>`);
    return;
  }
  clearFinaleCountdown();
  card.innerHTML = `<div class="winner-content tmr-card">
    <span class="winner-kicker">Loop ${t.loop} ends · Turn ${escapeAttr(String(reportedTurn(state)))}</span>
    <span class="winner-mark" aria-hidden="true">↺</span>
    <strong class="winner-title">${escapeAttr(t.fate.words)}</strong>
    <span class="winner-verdict">The world goes back to turn ${t.wake.turn}, as it stood</span>
    ${reignSummary(state)}
    <span class="winner-actions">
      <button class="primary winner-again" id="tmr-card-jump" type="button"
        title="Go back now. The count does it when it reaches zero."></button>
      <button class="winner-again winner-playon" id="tmr-card-stay" type="button"
        title="Stop the count and put this card away, to look at the world as it ended. Jump back is in the bar across the bottom.">Look around first</button>
    </span>
  </div>`;
  tomorrowCardCount();
}
// Where the page says a game that is over is followed by another (the End
// Turn button and the Next action panel, `drawTurnLoop` and
// `paintActionCorner` in app.js), a lost loop is followed by the jump back.
function tomorrowWords() {
  const t = tomorrow;
  if (!t.fate || t.fate.how === "won") return;
  const hint = document.querySelector("#endturn .endturn-hint");
  if (hint && hint.textContent === "Start another below") hint.textContent = "Jump back, in the bar above";
  const sub = document.querySelector("#actionhead .action-sub");
  if (sub && sub.textContent === "Start another from the deck.") sub.textContent = "Jump back, in the bar above.";
}
// The jump back: the world as it stood on the turn woken on, and the next
// loop. What this loop saw done to the seat goes with the person.
async function tomorrowJump() {
  const t = tomorrow;
  if (!debugTimeline() || !tomorrowIdle() || !tomorrowCanJump()) return null;
  tomorrowCountOff();
  const ended = {loop: t.loop, turn: reportedTurn(state) ?? state.turn, how: t.fate ? t.fate.how : "jumped",
    words: t.fate ? t.fate.words : "jumped back by hand"};
  t.jumping = true;
  tomorrowDraw(false);
  let next = null;
  try {
    next = await debugOp(t.wake.save !== null ? {op: "goto_save", id: t.wake.save} : {op: "jump", turn: t.wake.turn});
  } finally {
    t.jumping = false;
  }
  if (!next || next.error) {
    tomorrowSay(next && next.error ? `The jump back did not happen: ${next.error}` : "The jump back did not happen. Try again");
    tomorrowDraw();
    return null;
  }
  t.ends.push(ended);
  for (const [key, seen] of t.now) t.seen.set(key, seen);
  t.now = new Map();
  t.loop += 1;
  t.fate = null; t.looking = false;
  document.documentElement.classList.remove("tmr-looking");
  tomorrowSay(`Loop ${ended.loop} ended on turn ${ended.turn}: ${ended.words}`);
  tomorrowWatch();
  // As the top of a turn does, the next unit waiting for orders is taken up.
  if (typeof advanceToNextActionUnit === "function" && !sel && !selCity) advanceToNextActionUnit(true);
  tomorrowDraw();
  return next;
}
// Checkpoint here (operator 2026-11-26: "Change "wake here" to "Checkpoint
// here""; it was Wake here): from now on a jump back wakes on the world as
// it stands. Asked while standing on a checkpoint that was set, it is
// cleared, and a jump back wakes on the first turn again, so a checkpoint
// set by mistake is not for good.
async function tomorrowWakeHere() {
  const t = tomorrow, timeline = debugTimeline();
  if (!timeline || !tomorrowIdle() || t.fate) return;
  const first = timeline.first ?? state.turn;
  const standing = state.turn === t.wake.turn && (timeline.acted || 0) === t.wake.acted;
  if (standing && t.wake.save === null) return;
  if (standing) {
    const next = await debugOp({op: "drop_save", id: t.wake.save});
    if (!next || next.error) { tomorrowSay("The checkpoint could not be cleared. Try again"); tomorrowDraw(); return; }
    t.wake = {turn: first, save: null, acted: 0};
    tomorrowSay(`Checkpoint cleared: a jump back returns to turn ${first} again`);
  } else {
    const next = await debugOp({op: "save_point", name: TOMORROW_SAVE});
    const kept = next && !next.error ? (((next.debug || {}).saves) || []).find(save => save.name === TOMORROW_SAVE) : null;
    if (!kept) {
      tomorrowSay(next && next.error ? `This turn was not kept: ${next.error}` : "This turn was not kept. Try again");
      tomorrowDraw();
      return;
    }
    t.wake = {turn: kept.turn, save: kept.id, acted: kept.acted || 0};
    for (const [key, seen] of t.seen) if (seen.turn < t.wake.turn) t.seen.delete(key);
    tomorrowSay(`Checkpoint on turn ${kept.turn}: a jump back now returns here`);
  }
  tomorrowDraw();
}
function tomorrowMount() {
  const style = document.createElement("style");
  style.id = "tomorrow-style";
  style.textContent = TOMORROW_STYLE;
  document.head.appendChild(style);
  const bar = document.createElement("div");
  bar.id = "tomorrowbar";
  bar.hidden = true;
  bar.setAttribute("role", "toolbar");
  bar.setAttribute("aria-label", "CIVVIS: Edge of Tomorrow");
  bar.innerHTML = `
    <div class="tmr-name"><b>CIVVIS: Edge of Tomorrow</b><span class="tmr-loop">Loop <i id="tmr-loop">1</i></span></div>
    <div class="tmr-track">
      <div class="tmr-rail" id="tmr-rail" role="img"></div>
      <div class="tmr-said" id="tmr-said" role="status"></div>
    </div>
    <div class="tmr-acts">
      <button type="button" id="tmr-jump"></button>
      <button type="button" id="tmr-wake"></button>
    </div>`;
  document.body.appendChild(bar);
  bar.querySelector("#tmr-jump").addEventListener("click", () => { tomorrowJump(); });
  bar.querySelector("#tmr-wake").addEventListener("click", () => { tomorrowWakeHere(); });
  // The left panel's Auto-play comes into the bar whole: the same pickers
  // and button, so the page's own Auto-play reads and drives them where they
  // stand (`autoplay`, `syncAutoplayLabel`). Each picker's field is named
  // for its place on the bar, and what its note says is said on the bar.
  const autoplay = document.getElementById("autoplaybar");
  if (autoplay) {
    bar.querySelector(".tmr-acts").prepend(autoplay);
    for (const [picker, place] of [["autoplayturns", "tmr-turns"], ["autoplaystrategy", "tmr-ai"]]) {
      const field = document.getElementById(picker)?.closest(".autoplay-field");
      if (field) field.classList.add(place);
    }
    // The agent picker's name is CIVVIS AI here (operator 2026-11-26:
    // "rename to CIVVIS AI:"); the left panel of the other pages says Civ AI.
    const named = document.querySelector("#autoplaybar .tmr-ai > span");
    if (named) named.textContent = "CIVVIS AI";
    const note = document.getElementById("autoplaynote");
    if (note && typeof MutationObserver === "function") new MutationObserver(() => {
      if (!note.textContent) return;
      tomorrowSay(note.textContent);
      tomorrowDraw(false);
    }).observe(note, {childList: true, characterData: true, subtree: true});
  }
  // The lost loop's card is drawn into the page's own result box.
  const card = document.getElementById("winner");
  if (card) card.addEventListener("click", event => {
    const pressed = event.target instanceof Element ? event.target : null;
    if (!pressed) return;
    if (pressed.closest("#tmr-card-jump")) tomorrowJump();
    else if (pressed.closest("#tmr-card-stay")) {
      tomorrowCountOff();
      tomorrow.looking = true;
      document.documentElement.classList.add("tmr-looking");
      tomorrowDraw();
    }
  });
  tomorrowBar = bar;
  return bar;
}
// The bar stands under the map area, from where it begins to the right edge.
let tomorrowDocked = -1, tomorrowDockedLeft = -1;
function tomorrowDock(bar) {
  const root = document.documentElement;
  const left = bar.hidden ? 0 : debugMapLeft();
  if (left !== tomorrowDockedLeft) {
    tomorrowDockedLeft = left;
    root.style.setProperty("--tmr-left", `${left}px`);
  }
  const height = bar.hidden ? 0 : Math.ceil(bar.getBoundingClientRect().height);
  if (height === tomorrowDocked) return;
  tomorrowDocked = height;
  root.classList.toggle("tmr-docked", height > 0);
  root.style.setProperty("--tmr-bottom", `${height}px`);
  window.dispatchEvent(new Event("resize"));
}
// `measure` is whether the bar may have changed size: a drawing that only
// follows the turn's standing (`drawTurnLoop`) does not measure the page.
function tomorrowDraw(measure = true) {
  const bar = tomorrowBar || tomorrowMount();
  const t = tomorrow, timeline = debugTimeline();
  bar.hidden = !timeline;
  if (!timeline) { tomorrowDock(bar); return; }
  const idle = tomorrowIdle(), turn = state.turn, wake = t.wake.turn;
  const first = timeline.first ?? turn;
  // The line runs from the first turn to the last the game may have. A game
  // with no last turn has room ahead of the furthest turn reached.
  const limit = Number.isFinite(state.turn_limit) && state.turn_limit > 0 ? state.turn_limit : null;
  const lo = Math.min(first, wake, turn);
  const hi = Math.max(limit ?? Math.ceil((Math.max(t.far, turn) + 25) / 50) * 50, t.far, turn, lo + 1);
  const along = at => 100 * (Math.max(lo, Math.min(hi, at)) - lo) / (hi - lo);
  const place = at => `left:${along(at).toFixed(2)}%`;
  const span = (from, to) => `${place(from)};width:${Math.max(0, along(to) - along(from)).toFixed(2)}%`;
  const step = hi - lo > 300 ? 100 : hi - lo > 120 ? 50 : hi - lo > 40 ? 25 : 10;
  const ahead = [...t.seen.values()].filter(seen => seen.turn > turn).sort((one, other) => one.turn - other.turn);
  // Drawn in this order, each over the one before: what was lived, the
  // turns counted along it, the marks, and the open turn over them all.
  let rail = `<span class="tmr-lived" style="${span(wake, t.far)}"></span><span class="tmr-done" style="${span(wake, turn)}"></span>`;
  for (let at = Math.ceil((lo + 1) / step) * step; at < hi; at += step)
    rail += `<span class="tmr-tick" style="${place(at)}">${at}</span>`;
  for (const end of t.ends) {
    if (end.turn < wake) continue;
    rail += `<i class="tmr-end${end.how === "jumped" ? " tmr-by-hand" : ""}" style="${place(end.turn)}" ` +
      `title="${escapeAttr(`Loop ${end.loop} ended on turn ${end.turn}: ${end.words}`)}">${end.how === "jumped" ? "↺" : "✕"}</i>`;
  }
  for (const seen of ahead) {
    const [sign, colour] = TOMORROW_SEEN[seen.kind];
    rail += `<i class="tmr-mem" style="${place(seen.turn)};color:${colour}" ` +
      `title="${escapeAttr(`Loop ${seen.loop}, turn ${seen.turn}: ${seen.text}`)}">${sign}</i>`;
  }
  rail += `<b class="tmr-wake" style="${place(wake)}" title="Checkpoint: a jump back returns to turn ${wake}"></b>` +
    `<b class="tmr-now" style="${place(turn)}" title="Turn ${turn} of loop ${t.loop}">${turn}</b>`;
  const railBox = bar.querySelector("#tmr-rail");
  debugDrawn(railBox, rail);
  railBox.setAttribute("aria-label", `Loop ${t.loop}, turn ${turn}. A jump back returns to turn ${wake}`);
  debugDrawn(bar.querySelector("#tmr-loop"), String(t.loop));
  // What the bar says: what it was just told, then what is remembered of the
  // turns ahead, then how the last loop ended.
  const last = t.ends[t.ends.length - 1];
  let said = "";
  if (t.said && t.said.turn === turn) said = escapeAttr(t.said.text);
  else if (ahead.length) {
    said = `Seen in loop ${ahead[0].loop}: <b>turn ${ahead[0].turn}</b>, ${escapeAttr(ahead[0].text)}` +
      (ahead.length > 1 ? ` · and ${ahead.length - 1} more ahead` : "");
  } else if (last) said = escapeAttr(`Loop ${last.loop} ended on turn ${last.turn}: ${last.words}`);
  else said = `Jump back returns the world to turn ${wake}, as it stood. What you saw of it stays on this line`;
  debugDrawn(bar.querySelector("#tmr-said"), said);
  const jump = bar.querySelector("#tmr-jump");
  debugDrawn(jump, `<span>↺ Jump back</span><small>to turn ${wake}</small>`);
  jump.disabled = !idle || !tomorrowCanJump();
  jump.classList.toggle("tmr-due", !!t.fate && t.fate.how !== "won" && t.looking);
  jump.title = `Go back to turn ${wake} as the world stood then, and begin loop ${t.loop + 1}. ` +
    "The rivals play the same game again until you play a different one";
  const here = bar.querySelector("#tmr-wake");
  const standing = turn === wake && (timeline.acted || 0) === t.wake.acted;
  const back = standing && t.wake.save !== null;
  debugDrawn(here, back ? "Clear checkpoint" : "Checkpoint here");
  here.disabled = !idle || !!t.fate || (standing && !back);
  here.title = back ? `Clear the checkpoint: a jump back returns to turn ${first} again`
    : `Checkpoint here: from now on a jump back returns to turn ${turn}, as it stands`;
  if (measure) tomorrowDock(bar);
}

// Where the map is open on Edge of Tomorrow's screen, for the camera to
// centre on and fit into (operator 2026-11-26: "please try to center edge of
// tomorrow map a bit bettter , consider what is on the screen"). The
// Civilization VI screen lays its top strip (`#civtop`), its launch buttons
// (`#launchbar`) and its tools (`#hooksbar`) over the map, and the live
// tracker (`#livepause`) stands in the lower right. The viewer framed on the
// whole canvas, but for that tracker, which it cleared by giving up the whole
// lower strip, so the seat sat high, under the launch buttons. Here an
// overlay along a good part of an edge (a third of it or more) moves that
// edge in, and one in a corner, smaller than that both ways, stands on the
// ground as the map runs under it. In place of the viewer's `openBesideBox`,
// which both the camera's focus (`mapFocusBounds`) and an automatic frame's
// stage (`observedFrameRect`) go through, so the two agree. Canvas-local.
const TOMORROW_OVER_MAP = ["civtop", "launchbar", "hooksbar"];
function tomorrowOverMap(element, origin) {
  if (!element || element.hidden) return null;
  const seen = getComputedStyle(element);
  if (seen.display === "none" || seen.visibility === "hidden") return null;
  const rect = element.getBoundingClientRect();
  if (!rect.width || !rect.height) return null;
  return {left:rect.left - origin.left, right:rect.right - origin.left, top:rect.top - origin.top, bottom:rect.bottom - origin.top};
}
function tomorrowOpenGround(bounds, box) {
  const origin = cv.getBoundingClientRect();
  const over = TOMORROW_OVER_MAP.map(id => tomorrowOverMap(document.getElementById(id), origin)).concat([box]).filter(Boolean);
  const open = {...bounds};
  for (const part of over) {
    const across = (Math.min(part.right, open.right) - Math.max(part.left, open.left)) / Math.max(1, open.right - open.left);
    const down = (Math.min(part.bottom, open.bottom) - Math.max(part.top, open.top)) / Math.max(1, open.bottom - open.top);
    if (across <= 0 || down <= 0 || (across < 1 / 3 && down < 1 / 3)) continue;
    if (across >= down) {
      if ((part.top + part.bottom) / 2 < (open.top + open.bottom) / 2) open.top = Math.max(open.top, part.bottom);
      else open.bottom = Math.min(open.bottom, part.top);
    } else if ((part.left + part.right) / 2 < (open.left + open.right) / 2) open.left = Math.max(open.left, part.right);
    else open.right = Math.min(open.right, part.left);
  }
  // A screen too crowded to leave a stage keeps the one it was given.
  return open.right - open.left >= 160 && open.bottom - open.top >= 140 ? open : bounds;
}

if (TOMORROW) {
  if (typeof openBesideBox === "function") openBesideBox = tomorrowOpenGround;
  // And an automatic frame fits into that same open ground. The viewer's
  // frame keeps fixed room for a world minimap in the lower left and map
  // controls under it, which this page puts away (`TOMORROW_STYLE`): kept,
  // they narrowed the stage a fitted empire is zoomed to by a quarter.
  if (typeof observedFrameRect === "function" && typeof mapFocusBounds === "function") observedFrameRect = () => mapFocusBounds();
  // The standings and the victory tracker open with the game: the Civilization
  // VI screen's Rankings are opened before its first state is drawn, and the
  // choice a played game keeps between visits (`settleSoloRankings`) is not
  // read. Rankings still folds them for a look at the map.
  soloRankingsSettled = true;
  let ranked = false;
  // Behind `render` and `drawTurnLoop`, as the debugger's bar is: every
  // state is read for the loops, and the buttons follow the turn's standing.
  const drawState = render;
  render = function (st, ...rest) {
    if (!ranked) { ranked = true; toggleRankingsReport(true, false); }
    const drawn = drawState(st, ...rest);
    try { tomorrowWatch(); tomorrowDraw(); tomorrowWords(); } catch (error) { console.error("edge of tomorrow", error); }
    return drawn;
  };
  const drawLoop = drawTurnLoop;
  drawTurnLoop = function (...rest) {
    const drawn = drawLoop(...rest);
    try { tomorrowDraw(false); tomorrowWords(); } catch (error) { console.error("edge of tomorrow", error); }
    return drawn;
  };
  window.addEventListener("resize", () => { if (debugTimeline()) tomorrowDraw(); });
  // Operator 2026-11-26: "lets start with the player hud a more minimized
  // size. turn count should completely show. the stuff below it can be
  // covered by the minimiziation". The standings open as tall as the turn
  // box needs for its turn count, and no taller: what the box lists under it
  // (Mode, POV, the civilizations and city-states left) is covered until the
  // box is pulled taller. The roster's own floor still holds every seat.
  if (typeof measureTurnPlateNeed === "function") measureTurnPlateNeed = function (hud) {
    const plate = hud.querySelector(".victory-turn"), count = plate?.querySelector(".turn-count");
    const box = plate?.getBoundingClientRect();
    if (!count || !box.height) return 0;
    // 8px: the plate's own bottom padding and border, under its turn count.
    return Math.ceil(count.getBoundingClientRect().bottom - box.top + 8 + hud.getBoundingClientRect().height - box.height);
  };
  if (state) { tomorrowWatch(); tomorrowDraw(); tomorrowWords(); }
}

// ═══════════════════════════════════════════════════════════════════════════
// CIVVIS: Moneyball
// ═══════════════════════════════════════════════════════════════════════════
//
// Operator 2026-11-27: "add CIVVIS: Moneyball. This is a minigame where a
// user only looks at the stats and tries to play the best game. WE can have a
// default set of stats but users should get to see there own. User
// essentially has N columns, each with a different player strategy simulated
// out N turns. rows are different stats. cities, mil strength, food,
// happiness, hous, prod, science yield, culture yield, etc. User selects how
// many turns out they want to skip at a time (default can be 10). We can also
// estimate how long the game will go (given the strategies, how long do we
// think the game will go)? How many guesses we expect a person to make. if
// game 200 turns and increment 10, then 20 guesses. we can highlight top in
// each row. player strategy should be the top strategies but randomized on
// screen. user can chose a row to sort by though if they want."
//
// `/moneyball` is a played game whose seat is played, a stretch of turns at a
// time, by the player strategy the person picks, and they pick it by its
// numbers alone. Over the map stands a board: a column for each of the top
// rated player strategies, in an order drawn afresh each time and named only
// by a letter, each played out that stretch of turns on a copy of the world
// (the copies Autosearch plays on, `POST /forks`), and a row for each figure
// of how the seat stands at the end of it (`obs::standing`). The best of each
// row is lit, and a row's name sorts the columns by it. Picking a column
// plays that strategy those turns on the world itself (`debugPlayPlan`), which
// comes out as its column said, the engine being deterministic, and the board
// is dealt again from there.
//
// Each copy then plays its strategy on to the end of the game, without
// keeping its turns (`foresee`), and where they end says how long the game
// will likely go, and so how many picks are left.
//
// Nothing here is the engine's but the copies and their answers, and what
// the board holds is gone with the page.

// The figures a row can show: the key the person's choice is kept under, its
// name, how it reads off a seat's standing, whether it is shown before the
// person chooses, and what it is.
const MONEYBALL_ROWS = [
  ["score", "Score", seat => seat.score, true, "The game's score"],
  ["cities", "Cities", seat => seat.cities, true, "Cities held"],
  ["population", "Population", seat => seat.population, true, "Citizens in every city together"],
  ["military", "Military strength", seat => seat.military, true, "The army's strength, as the standings show it"],
  ["food", "Food", seat => seat.yields && seat.yields.food, true, "Food a turn"],
  ["amenities", "Happiness", seat => seat.amenities, true, "Amenities over what a city needs, the average of the cities"],
  ["housing", "Housing", seat => seat.housing, true, "Housing to spare, the average of the cities"],
  ["production", "Production", seat => seat.yields && seat.yields.production, true, "Production a turn"],
  ["science", "Science", seat => seat.yields && seat.yields.science, true, "Science a turn"],
  ["culture", "Culture", seat => seat.yields && seat.yields.culture, true, "Culture a turn"],
  ["gold", "Gold", seat => seat.gold_per_turn, true, "Gold a turn, after upkeep"],
  ["faith", "Faith", seat => seat.yields && seat.yields.faith, false, "Faith a turn"],
  ["food_surplus", "Food surplus", seat => seat.food_surplus, false, "Food a city grows on, after its citizens eat, the average of the cities"],
  ["loyalty", "Loyalty", seat => seat.loyalty, false, "Loyalty a turn, the average of the cities"],
  ["techs", "Techs", seat => seat.techs, false, "Technologies researched"],
  ["civics", "Civics", seat => seat.civics, false, "Civics adopted"],
  ["tourism", "Tourism", seat => seat.tourism, false, "Tourism earned over the game"],
];
// How many turns a pick plays, and how many columns there are to pick from.
const MONEYBALL_TURNS = [5, 10, 20, 25, 50];
const MONEYBALL_COLUMNS = [3, 4, 6, 8];
const MONEYBALL_LETTERS = "ABCDEFGH";
// A copy plays on to the end of the game this many turns at a time, so a
// pick made meanwhile waits no longer than one stretch of it.
const MONEYBALL_SIGHT_TURNS = 25;
const MONEYBALL_SAVE = "civvis-moneyball-v1";

const moneyball = {
  // What the person chose: turns a pick, columns, the rows shown, and
  // whether the strategies' names are shown.
  set: (() => {
    const kept = {turns: 10, columns: 6, rows: MONEYBALL_ROWS.filter(row => row[3]).map(row => row[0]), names: false};
    try {
      const saved = JSON.parse(localStorage.getItem(MONEYBALL_SAVE) || "null");
      if (saved && MONEYBALL_TURNS.includes(saved.turns)) kept.turns = saved.turns;
      if (saved && MONEYBALL_COLUMNS.includes(saved.columns)) kept.columns = saved.columns;
      if (saved && Array.isArray(saved.rows)) kept.rows = saved.rows.filter(key => MONEYBALL_ROWS.some(row => row[0] === key));
      if (saved && typeof saved.names === "boolean") kept.names = saved.names;
    } catch (_) { /* the stock choice */ }
    return kept;
  })(),
  // The deal on the board: the world it was dealt from (its seed and turn),
  // the seat as it stands there, and a column for each strategy.
  dealt: null,
  // A count that grows with every deal: an answer to an older one is let go.
  deal: 0,
  // The row the columns are sorted by, or null for the order they were drawn.
  sort: null,
  // The picks made, oldest first: {from, to, letter, name}.
  picks: [],
  // A pick being played on the world.
  playing: false,
  // The board shows the map instead.
  away: false,
  said: "",
};
let moneyballBoard = null;

const MONEYBALL_STYLE = `
  /* The board stands over the map (operator 2026-11-27: "a minigame where a
     user only looks at the stats"), from the map's left edge to the
     window's right. */
  #moneyball {
    position: fixed; z-index: 9000; top: 0; right: 0; bottom: 0; left: var(--mb-left, 0px);
    display: flex; flex-direction: column; gap: 10px; padding: 14px 18px 16px; overflow: auto;
    background: radial-gradient(120% 90% at 50% 0%, #143525 0%, #0b1a13 55%, #08110d 100%);
    color: #e9f1e6; font: 13px/1.35 -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Arial, sans-serif;
  }
  #moneyball[hidden] { display: none; }
  /* The seat is played by the picks alone: the left panel's Auto-play, which
     would hand it to a Civ AI for a run of turns past the board, is put
     away (its picker stays in the page, which seats each pick by it). */
  html.moneyball #humanfooter { display: none !important; }
  #moneyball.is-away { bottom: auto; overflow: visible; background: #0b1a13ee; border-bottom: 1px solid #2f8a4a; }
  #moneyball.is-away .mb-body { display: none; }
  .mb-head { display: flex; flex-wrap: wrap; align-items: baseline; gap: 6px 16px; }
  .mb-title { margin: 0; font-size: 18px; font-weight: 800; letter-spacing: 0.04em; color: #f3f6ee; }
  .mb-title i { font-style: normal; color: #8fe07a; }
  .mb-turn { color: #cfe6c4; font-size: 14px; font-variant-numeric: tabular-nums; }
  .mb-turn b { color: #fff; }
  .mb-tools { display: flex; flex-wrap: wrap; align-items: center; gap: 6px 12px; margin-left: auto; }
  .mb-tools label { display: inline-flex; align-items: center; gap: 5px; color: #b7cdb0; font-size: 12px; }
  .mb-tools select, .mb-tools button, .mb-pick, .mb-rows summary {
    font: inherit; font-size: 12px; color: #f3f6ee; background: #16301f; border: 1px solid #2f6a43; border-radius: 4px; padding: 3px 8px; cursor: pointer;
  }
  .mb-tools button[aria-pressed="true"] { background: #2f8a4a; border-color: #8fe07a; }
  .mb-rows { position: relative; }
  .mb-rows summary { list-style: none; }
  .mb-rows summary::-webkit-details-marker { display: none; }
  .mb-rows[open] .mb-row-list { display: grid; }
  .mb-row-list {
    display: none; position: absolute; z-index: 2; right: 0; top: calc(100% + 4px); grid-template-columns: repeat(2, max-content); gap: 4px 14px;
    padding: 8px 10px; background: #0d2017; border: 1px solid #2f6a43; border-radius: 6px; box-shadow: 0 8px 24px #000a;
  }
  .mb-row-list label { color: #e9f1e6; }
  .mb-estimate { margin: 0; color: #cfe6c4; font-size: 13px; }
  .mb-estimate b { color: #fff; font-variant-numeric: tabular-nums; }
  .mb-said { margin: 0; min-height: 1.3em; color: #ffd27a; font-size: 12.5px; }
  .mb-table { border-collapse: separate; border-spacing: 0; width: 100%; font-variant-numeric: tabular-nums; }
  .mb-table th, .mb-table td { padding: 6px 10px; border-bottom: 1px solid #1f3d2b; text-align: right; white-space: nowrap; }
  .mb-table thead th { position: sticky; top: 0; z-index: 1; background: #0e2218; color: #cfe6c4; font-weight: 700; vertical-align: bottom; }
  .mb-table thead th small { display: block; color: #8fb38a; font-size: 11px; font-weight: 500; }
  .mb-table tbody th { text-align: left; font-weight: 600; }
  .mb-table .mb-sort { font: inherit; color: #e9f1e6; background: none; border: 0; padding: 0; cursor: pointer; text-align: left; }
  .mb-table .mb-sort:hover, .mb-table .mb-sort:focus-visible { color: #8fe07a; outline: none; text-decoration: underline; }
  .mb-table .mb-sort[aria-sort="descending"] { color: #8fe07a; }
  .mb-table .mb-sort[aria-sort="descending"]::after { content: " \\25BE"; }
  .mb-table .mb-now { color: #9fb99a; background: #0c1c14; }
  .mb-table td.is-top { color: #10200f; background: linear-gradient(180deg, #a8f08f, #6fcf62); font-weight: 800; border-radius: 3px; }
  .mb-table td small { margin-left: 5px; font-size: 11px; color: #8fb38a; }
  .mb-table td.is-top small { color: #1f4a1c; }
  .mb-table td.is-waiting { color: #5e7a59; }
  .mb-table tr.mb-ends td, .mb-table tr.mb-ends th { color: #b7cdb0; font-size: 12px; }
  .mb-table thead th.mb-same { color: #e9f1e6; letter-spacing: 0.04em; }
  .mb-table .mb-end-each { display: block; }
  .mb-table tfoot td { border-bottom: 0; padding-top: 10px; }
  .mb-pick { width: 100%; padding: 6px 8px; font-size: 13px; font-weight: 700; background: #2f8a4a; border-color: #8fe07a; }
  .mb-pick:hover:not(:disabled) { background: #38a057; }
  .mb-pick:disabled { cursor: default; opacity: 0.45; }
  .mb-picks { margin: 0; padding: 0 0 0 18px; color: #9fb99a; font-size: 12px; columns: 2 280px; }
  .mb-over { margin: 0; padding: 10px 12px; border: 1px solid #8fe07a; border-radius: 6px; background: #12301f; color: #f3f6ee; font-size: 14px; }
  .mb-over button { margin-left: 10px; }
  @media (max-width: 760px) {
    #moneyball { padding: 10px; }
    .mb-tools { margin-left: 0; }
    .mb-table th, .mb-table td { padding: 5px 6px; }
  }
`;

function moneyballSave() {
  try { localStorage.setItem(MONEYBALL_SAVE, JSON.stringify(moneyball.set)); } catch (_) { /* not kept */ }
}
function moneyballIdle() {
  return !!debugTimeline() && !debugBusy && !debugRunning && !autoplaying && !turnAdvancePending && !moneyball.playing;
}
// The top rated player strategies the page offers, as many as there are
// columns, best first (`RULES.strategies`, which the picker lists by Elo).
// With nothing rated, those the page offers in Doctor Strange's order.
function moneyballStrategies(count) {
  const offered = strangeAgents();
  const roster = typeof RULES !== "undefined" && RULES && Array.isArray(RULES.strategies) ? RULES.strategies : [];
  const rated = roster.filter(entry => entry && offered.includes(entry.name) && Number.isFinite(Number(entry.elo)))
    .sort((one, other) => Number(other.elo) - Number(one.elo)).map(entry => entry.name);
  const rest = [...STRANGE_RIVALS, ...offered].filter(name => offered.includes(name) && !rated.includes(name));
  return [...new Set([...rated, ...rest])].slice(0, count);
}
// The order the columns are drawn in: shuffled afresh each deal, so a column
// is not known by where it stands.
function moneyballShuffled(count) {
  const order = Array.from({length: count}, (_, at) => at);
  for (let at = order.length - 1; at > 0; at -= 1) {
    const other = Math.floor(Math.random() * (at + 1));
    [order[at], order[other]] = [order[other], order[at]];
  }
  return order;
}
function moneyballSay(text) {
  moneyball.said = text || "";
  moneyballDraw();
}
// One request to one copy, answered lean. An answer to a deal that has been
// replaced is not wanted.
async function moneyballAsk(deal, path, body) {
  const answer = await debugSearchPost(path, {...body, lean: true});
  if (deal !== moneyball.deal) throw new Error(MONEYBALL_GONE);
  if (!answer) throw new Error("the engine gave no answer");
  return answer;
}
const MONEYBALL_GONE = "the board was dealt again";

// The board is dealt from the world as it stands: each column's strategy
// plays the turns of a pick on a copy of the world, and then plays on to the
// end of the game. Copies are made for as many columns as the tab has room
// for (`tabEnginesMB`); where there are fewer, columns wait their turn.
async function moneyballDealBoard() {
  if (!moneyballIdle() || !state) return;
  const deal = ++moneyball.deal;
  const turn = state.turn, turns = moneyball.set.turns;
  const names = moneyballStrategies(moneyball.set.columns);
  const shuffled = moneyballShuffled(names.length);
  moneyball.sort = null;
  moneyball.dealt = {
    seed: String(state.seed ?? ""), turn, turns, now: null,
    columns: shuffled.map((which, at) => ({letter: MONEYBALL_LETTERS[at], name: names[which], seat: null, readings: [],
      world: null, finished: false, winner: null, end: null, error: null})),
  };
  moneyballSay("");
  if (!names.length) { moneyballSay("This page offers no player strategy to pick from"); return; }
  let made;
  try {
    made = await debugSearchPost("/forks", {count: names.length});
  } catch (error) {
    made = {error: String(error.message || error)};
  }
  if (deal !== moneyball.deal) return;
  if (!made || made.error || !(made.forks > 0)) {
    moneyballSay("Moneyball plays its columns on copies of the engine, and this build could not make them" +
      ` (${(made && made.error) || "it has no forks"}). It runs on civvis.ai, in the browser`);
    return;
  }
  const queue = [...moneyball.dealt.columns];
  const copy = async at => {
    const path = `/debug?fork=${at + 1}`;
    for (let column = queue.shift(); column; column = queue.shift()) await moneyballPlay(deal, path, column, turn, turns);
  };
  try {
    await Promise.all(Array.from({length: Math.min(made.forks, queue.length)}, (_, at) => copy(at)));
  } catch (error) {
    if (error.message !== MONEYBALL_GONE) moneyballSay(`A column could not be played: ${error.message || error}`);
  }
}
// One column: its strategy seated on the turn the board was dealt on, the
// turns of a pick played, and then the rest of the game seen.
async function moneyballPlay(deal, path, column, turn, turns) {
  try {
    await moneyballAsk(deal, path, {op: "breakpoints"});
    const there = await moneyballAsk(deal, path, {op: "jump", turn, strategy: column.name});
    if (there.error || there.turn !== turn) throw new Error(there.error || `turn ${turn} is not kept`);
    if (!moneyball.dealt.now && there.seat) moneyball.dealt.now = there.seat;
    const played = await moneyballAsk(deal, path, {op: "play", turns, strategy: column.name});
    if (played.error && !/no turn left/.test(played.error)) throw new Error(played.error);
    column.readings = Array.isArray(played.readings) ? played.readings : [];
    column.seat = played.seat || column.readings[column.readings.length - 1] || there.seat || null;
    column.world = moneyballPrint(played);
    column.finished = played.finished === true;
    column.winner = played.winner ?? null;
    moneyballDraw();
    if (column.finished) {
      column.end = {turn: played.turn, done: true, victory: null};
      moneyballDraw();
      return;
    }
    // On to the end of the game, a stretch at a time.
    let seen = null;
    for (let stretch = 0; stretch < 40; stretch += 1) {
      const asked = await moneyballAsk(deal, path, {op: "foresee", turns: MONEYBALL_SIGHT_TURNS, strategy: column.name,
        ...(seen ? {known: seen.turn} : {anew: true})});
      const sight = asked && asked.foresight;
      if (!sight || typeof sight.turn !== "number") break;
      seen = sight;
      column.end = {turn: sight.turn, done: sight.done === true, victory: sight.victory || null, limit: sight.limit ?? null};
      moneyballDraw();
      if (sight.done || !(sight.played > 0)) break;
    }
  } catch (error) {
    if (error.message === MONEYBALL_GONE) throw error;
    column.error = String(error.message || error);
    moneyballDraw();
  }
}
// A column picked: its strategy plays the turns of a pick on the world itself.
async function moneyballPick(letter) {
  const dealt = moneyball.dealt;
  const column = dealt && dealt.columns.find(held => held.letter === letter);
  if (!column || !column.seat || !moneyballIdle() || !state || state.turn !== dealt.turn) return;
  moneyball.playing = true;
  moneyball.deal += 1;
  const to = dealt.turn + dealt.turns;
  const group = moneyballGroups(dealt.columns).find(held => held.columns.includes(column));
  moneyball.picks.push({from: dealt.turn, to, letter: group ? group.label : letter, name: column.name});
  moneyballSay(`Playing column ${letter} to turn ${to}…`);
  let played = null;
  try {
    played = await debugPlayPlan({from: dealt.turn, print: null, stretches: [{name: column.name, to}]});
  } catch (error) {
    played = {ok: false, note: String(error.message || error)};
  } finally {
    moneyball.playing = false;
  }
  moneyball.said = played && !played.ok ? played.note || "The pick could not be played" : "";
  moneyball.dealt = null;
  moneyballWatch();
}
// The print of the world an answer stands on (`world_print`), with its
// turn: two copies on the same turn with the same print hold the same world,
// as Doctor Strange reads two of its panes (`strangeSame`). `null` when the
// answer does not say.
function moneyballPrint(answer) {
  const marks = answer && answer.debug, turn = answer && answer.turn;
  if (!marks || typeof turn !== "number" || !Array.isArray(marks.turns) || !Array.isArray(marks.worlds)
    || marks.worlds.length !== marks.turns.length) return null;
  const at = marks.turns.indexOf(turn);
  return at >= 0 && marks.worlds[at] !== null && marks.worlds[at] !== undefined ? `${turn}|${marks.worlds[at]}` : null;
}
// The columns as the board draws them (operator 2026-11-28: "civvis
// moneyball we can merge identical states"): columns whose strategies played
// the turns of a pick to the same world are one column, named by all their
// letters, in the place of the first of them. A column not yet played, or
// whose world was not said, stands alone.
function moneyballGroups(columns) {
  const groups = [], held = new Map();
  for (const column of columns) {
    const same = column.world && !column.error ? held.get(column.world) : undefined;
    if (same) { same.columns.push(column); continue; }
    const group = {columns: [column]};
    groups.push(group);
    if (column.world && !column.error) held.set(column.world, group);
  }
  for (const group of groups) {
    group.first = group.columns[0];
    group.letters = group.columns.map(column => column.letter);
    group.label = group.letters.join("=");
  }
  return groups;
}
function moneyballRead(row, seat) {
  if (!seat) return null;
  const value = row[2](seat);
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}
function moneyballNumber(value) {
  if (value === null) return "–";
  const whole = Math.abs(value) >= 100 || Number.isInteger(value);
  return whole ? Math.round(value).toLocaleString("en-US") : (Math.round(value * 10) / 10).toFixed(1);
}
// How long the game will likely go: where the columns' futures end, the
// middle of them, and how many picks that leaves.
function moneyballEstimate(dealt) {
  const ends = dealt.columns.filter(column => column.end && column.end.done).map(column => column.end.turn).sort((one, other) => one - other);
  const waiting = dealt.columns.some(column => !column.error && !(column.end && column.end.done));
  if (!ends.length) return {words: waiting ? "Working out how long the game will go…" : "", ends};
  const middle = ends[Math.floor((ends.length - 1) / 2)];
  const left = Math.max(0, Math.ceil((middle - dealt.turn) / dealt.turns));
  const spread = ends[0] === ends[ends.length - 1] ? `turn ${ends[0]}` : `turns ${ends[0]}–${ends[ends.length - 1]}`;
  return {middle, left, ends,
    words: `The game looks set to end around turn ${middle} (${spread} if a column played on${waiting ? ", still working" : ""}): ` +
      `about ${left} more ${left === 1 ? "pick" : "picks"} at ${dealt.turns} turns a pick, ${moneyball.picks.length} made so far.`};
}

function moneyballMount() {
  const style = document.createElement("style");
  style.id = "moneyball-style";
  style.textContent = MONEYBALL_STYLE;
  document.head.appendChild(style);
  const board = document.createElement("section");
  board.id = "moneyball";
  board.hidden = true;
  board.setAttribute("aria-label", "CIVVIS: Moneyball");
  board.innerHTML = `
    <div class="mb-head">
      <h2 class="mb-title">CIVVIS: <i>Moneyball</i></h2>
      <span class="mb-turn" id="mb-turn"></span>
      <div class="mb-tools">
        <label>Turns a pick <select id="mb-turns"></select></label>
        <label>Columns <select id="mb-columns"></select></label>
        <details class="mb-rows"><summary>Stats</summary><div class="mb-row-list" id="mb-row-list"></div></details>
        <button type="button" id="mb-names" aria-pressed="false" title="Show which player strategy each column is">Names</button>
        <button type="button" id="mb-away" aria-pressed="false" title="Fold the board up to look at the map">Map</button>
      </div>
    </div>
    <div class="mb-body">
      <p class="mb-estimate" id="mb-estimate"></p>
      <p class="mb-said" id="mb-said" role="status"></p>
      <div id="mb-over"></div>
      <table class="mb-table" id="mb-table"></table>
      <ol class="mb-picks" id="mb-picks"></ol>
    </div>`;
  document.body.appendChild(board);
  const turns = board.querySelector("#mb-turns"), columns = board.querySelector("#mb-columns");
  for (const value of MONEYBALL_TURNS) turns.add(new Option(String(value), String(value)));
  for (const value of MONEYBALL_COLUMNS) columns.add(new Option(String(value), String(value)));
  turns.value = String(moneyball.set.turns);
  columns.value = String(moneyball.set.columns);
  const again = () => { moneyballSave(); moneyball.dealt = null; moneyball.deal += 1; moneyballWatch(); };
  turns.addEventListener("change", () => { moneyball.set.turns = Number(turns.value); again(); });
  columns.addEventListener("change", () => { moneyball.set.columns = Number(columns.value); again(); });
  const list = board.querySelector("#mb-row-list");
  for (const [key, name, , , what] of MONEYBALL_ROWS) {
    const label = document.createElement("label");
    label.title = what;
    const box = document.createElement("input");
    box.type = "checkbox";
    box.value = key;
    box.checked = moneyball.set.rows.includes(key);
    box.addEventListener("change", () => {
      moneyball.set.rows = MONEYBALL_ROWS.map(row => row[0]).filter(row => row === key ? box.checked : moneyball.set.rows.includes(row));
      moneyballSave();
      moneyballDraw();
    });
    label.append(box, name);
    list.appendChild(label);
  }
  const names = board.querySelector("#mb-names");
  names.addEventListener("click", () => { moneyball.set.names = !moneyball.set.names; moneyballSave(); moneyballDraw(); });
  board.querySelector("#mb-away").addEventListener("click", () => { moneyball.away = !moneyball.away; moneyballDraw(); });
  // A row's name sorts the columns by it, the most first; again, back to
  // the order they were drawn in.
  board.querySelector("#mb-table").addEventListener("click", event => {
    const sort = event.target.closest("[data-mb-sort]");
    if (sort) { moneyball.sort = moneyball.sort === sort.dataset.mbSort ? null : sort.dataset.mbSort; moneyballDraw(); return; }
    const pick = event.target.closest("[data-mb-pick]");
    if (pick) moneyballPick(pick.dataset.mbPick);
  });
  moneyballBoard = board;
}
// The board as it stands: the deal's columns in the order drawn or sorted,
// a row for each figure chosen with the best lit, where the game will likely
// end, and the picks made.
function moneyballDraw() {
  const board = moneyballBoard;
  if (!board) return;
  board.hidden = !state;
  board.classList.toggle("is-away", moneyball.away);
  board.querySelector("#mb-away").setAttribute("aria-pressed", String(moneyball.away));
  board.querySelector("#mb-names").setAttribute("aria-pressed", String(moneyball.set.names));
  document.documentElement.style.setProperty("--mb-left", `${debugMapLeft()}px`);
  if (!state) return;
  const dealt = moneyball.dealt;
  const turnLine = board.querySelector("#mb-turn");
  turnLine.textContent = "";
  const shown = document.createElement("b");
  shown.textContent = `Turn ${state.turn}`;
  turnLine.append(shown, ` · pick ${moneyball.picks.length + 1}`);
  board.querySelector("#mb-said").textContent = moneyball.said;
  const over = board.querySelector("#mb-over");
  over.textContent = "";
  if (gameFinished(state)) {
    const me = (state.players || [])[0] || {};
    const winners = Array.isArray(state.winners) && state.winners.length ? state.winners
      : state.winner !== null && state.winner !== undefined ? [state.winner] : [];
    const card = document.createElement("p");
    card.className = "mb-over";
    card.textContent = `${winners.includes(0) ? `${me.civ || "You"} won` : "Game over"} on turn ${state.turn}: ` +
      `${victoryVerdict(state.victory_type, state.victory_label)}. ${moneyball.picks.length} ${moneyball.picks.length === 1 ? "pick" : "picks"}, score ${moneyballNumber(typeof me.score === "number" ? me.score : null)}.`;
    const fresh = document.createElement("button");
    fresh.type = "button";
    fresh.className = "mb-pick";
    fresh.style.width = "auto";
    fresh.textContent = "Play again";
    fresh.addEventListener("click", () => location.reload());
    card.appendChild(fresh);
    over.appendChild(card);
  }
  const estimate = dealt ? moneyballEstimate(dealt) : {words: ""};
  board.querySelector("#mb-estimate").textContent = estimate.words;
  const table = board.querySelector("#mb-table");
  table.textContent = "";
  if (dealt) {
    const rows = MONEYBALL_ROWS.filter(row => moneyball.set.rows.includes(row[0]));
    let order = [...dealt.columns];
    const sortRow = rows.find(row => row[0] === moneyball.sort);
    if (sortRow) order.sort((one, other) => (moneyballRead(sortRow, other.seat) ?? -Infinity) - (moneyballRead(sortRow, one.seat) ?? -Infinity));
    const groups = moneyballGroups(order);
    const cell = (tag, text, cls) => {
      const node = document.createElement(tag);
      if (cls) node.className = cls;
      if (text !== undefined && text !== null) node.textContent = text;
      return node;
    };
    const head = document.createElement("thead"), headRow = document.createElement("tr");
    headRow.appendChild(cell("th", "Stat"));
    const nowHead = cell("th", "Now", "mb-now");
    nowHead.appendChild(cell("small", `turn ${dealt.turn}`));
    headRow.appendChild(nowHead);
    for (const group of groups) {
      const th = cell("th", group.label);
      th.dataset.mbLetters = group.letters.join("");
      if (group.columns.length > 1) {
        th.classList.add("mb-same");
        th.title = `${group.letters.slice(0, -1).join(", ")} and ${group.letters[group.letters.length - 1]} are the same world on turn ` +
          `${dealt.turn + dealt.turns}: their strategies played these turns alike`;
      }
      th.appendChild(cell("small", moneyball.set.names ? group.columns.map(column => debugSearchAgent(column.name)).join(" / ")
        : `turn ${dealt.turn + dealt.turns}`));
      headRow.appendChild(th);
    }
    head.appendChild(headRow);
    table.appendChild(head);
    const body = document.createElement("tbody");
    for (const row of rows) {
      const tr = document.createElement("tr");
      tr.dataset.row = row[0];
      const th = document.createElement("th");
      const sort = cell("button", row[1], "mb-sort");
      sort.type = "button";
      sort.dataset.mbSort = row[0];
      sort.title = `${row[4]}. Sort the columns by it`;
      if (moneyball.sort === row[0]) sort.setAttribute("aria-sort", "descending");
      th.appendChild(sort);
      tr.appendChild(th);
      const now = moneyballRead(row, dealt.now);
      tr.appendChild(cell("td", moneyballNumber(now), "mb-now"));
      const values = groups.map(group => moneyballRead(row, group.first.seat));
      const known = values.filter(value => value !== null);
      const top = known.length > 1 ? Math.max(...known) : null;
      groups.forEach(({first: column}, at) => {
        const value = values[at];
        const td = cell("td", column.error ? "×" : value === null ? (column.seat ? "–" : "…") : moneyballNumber(value),
          value !== null && top !== null && value === top && known.some(other => other !== top) ? "is-top" : !column.seat && !column.error ? "is-waiting" : "");
        if (value !== null && now !== null && value !== now) td.appendChild(cell("small", `${value > now ? "+" : "−"}${moneyballNumber(Math.abs(value - now))}`));
        if (column.error) td.title = column.error;
        tr.appendChild(td);
      });
      body.appendChild(tr);
    }
    // Where each column's game would end if it played on.
    const ends = document.createElement("tr");
    ends.className = "mb-ends";
    ends.appendChild(cell("th", "Game ends"));
    ends.appendChild(cell("td", "", "mb-now"));
    // A merged column's strategies part after the pick, so each says where
    // its own game ends, unless they all end alike.
    const endWords = column => {
      const end = column.end;
      return column.error ? "×" : !end ? "…" : end.done ? `turn ${end.turn}${end.victory ? ` · ${titleCase(String(end.victory))}` : ""}` : `past turn ${end.turn}…`;
    };
    for (const group of groups) {
      const said = group.columns.map(endWords);
      const td = cell("td", new Set(said).size === 1 ? said[0] : null);
      if (new Set(said).size > 1) group.columns.forEach((column, at) => td.appendChild(cell("span", `${column.letter}: ${said[at]}`, "mb-end-each")));
      ends.appendChild(td);
    }
    body.appendChild(ends);
    table.appendChild(body);
    const foot = document.createElement("tfoot"), footRow = document.createElement("tr");
    footRow.append(cell("td"), cell("td"));
    const free = moneyballIdle() && !gameFinished(state) && state.turn === dealt.turn;
    for (const {first: column, label} of groups) {
      const td = document.createElement("td");
      const pick = cell("button", `Pick ${label}`, "mb-pick");
      pick.type = "button";
      pick.dataset.mbPick = column.letter;
      pick.disabled = !free || !column.seat;
      td.appendChild(pick);
      footRow.appendChild(td);
    }
    foot.appendChild(footRow);
    table.appendChild(foot);
  }
  const picks = board.querySelector("#mb-picks");
  picks.textContent = "";
  for (const pick of moneyball.picks) {
    const item = document.createElement("li");
    // The turns it played: from the turn picked on to the one before the
    // turn the world then stands on.
    item.textContent = `Turns ${pick.from}–${pick.to - 1}: ${pick.letter}, ${debugSearchAgent(pick.name)}`;
    picks.appendChild(item);
  }
}
// The state that was just drawn: a world that stands still on a turn the
// board was not dealt from is dealt again.
function moneyballWatch() {
  if (!moneyballBoard || !state) return;
  const dealt = moneyball.dealt;
  const fresh = !dealt || dealt.turn !== state.turn || dealt.seed !== String(state.seed ?? "") || dealt.turns !== moneyball.set.turns
    || dealt.columns.length !== Math.min(moneyball.set.columns, Math.max(1, strangeAgents().length));
  if (fresh && moneyballIdle() && !gameFinished(state)) moneyballDealBoard();
  moneyballDraw();
}

if (MONEYBALL) {
  moneyballMount();
  const drawState = render;
  render = function (st, ...rest) {
    const drawn = drawState(st, ...rest);
    try { moneyballWatch(); } catch (error) { console.error("moneyball", error); }
    return drawn;
  };
  window.addEventListener("resize", () => moneyballDraw());
  // A world that stands still is dealt from once it is idle, whatever drew it last.
  setInterval(() => { try { if (!moneyball.dealt && state) moneyballWatch(); } catch (error) { console.error("moneyball", error); } }, 1000);
  if (state) moneyballWatch();
}
