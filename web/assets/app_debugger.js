"use strict";
// The simulator debugger's bar.
//
// A played game keeps a mark of every turn its seat has opened (the engine's
// `server/debugger.rs`), and every state it sends says where the world sits
// on that timeline (`state.debug`). This script is the hand on it: a header
// box at the top of the window, beside the left panel. The whole game is one
// long timeline across the top of the box: turn 1 at its left end, the last
// turn under a chequered flag at its right, the first turn of each era named
// on it, and a marker that moves right as turns open. A turn that was played
// is gone back to as it stood, and a turn still ahead is played to by the
// seat's Civ AI. Under the timeline the open turn stands in the middle with
// the turn buttons either side of it (Turn 1, Era Back, ten back, one back;
// one on, ten on, Era Next, Play to End). Auto-play, layers, breakpoints and
// Branch Timeline are at
// the left, and stepping in, undo, save points, reveal and the Single player
// switch at the right.
//
// Branch Timeline splits the screen in two, side by side, at once. The two
// halves begin locked: they stay on the same turn, and this box is the one
// control panel for both. Clicking the lock opens it, and then each half has
// a panel of its own. The timeline stays one for as long as the two halves
// hold the same world, and branches in two at the turn their worlds differ.
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
  /* While the screen is split the box stays up, over the two halves, and has
     the window's whole width: the halves cover the left panel. */
  html.dbg-split-open #debugbar { z-index: 100001; left: 0; }

  /* Under the timeline, one row of three parts. The turn that is open stands
     in the middle, under the middle of the timeline, with the turn buttons on
     either side of it. Auto-play, layers, breakpoints and Branch Timeline are
     at the left end, and the rest of the tools at the right end. */
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
  /* The picker's own label is for a box with room to spare. */
  #debugbar:not(.dbg-roomy) .dbg-roomy-word { display: none; }
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
  /* Wide enough for a count, so the row does not move as it plays. */
  #debugbar [data-dbg="autoplay"] { min-width: 112px; font-weight: 700; font-variant-numeric: tabular-nums; }
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
     keeps the one rail for as long as its two halves are the same world, and
     from the turn they differ each half has a branch of its own. It is bright
     on purpose, sky behind and the turns played in sun across it. */
  #debugbar .dbg-time { display: grid; gap: 4px; min-width: 0; }
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
  /* A split has three lanes in one grid. The world's lane is the trunk: the
     one timeline of two halves that are the same world, as tall as both
     halves' rows. Each half's lane is a row of its own, the old height, and
     its slider runs the whole length of it: the top of the timeline is the
     hand on A and the bottom the hand on B, on the trunk as on the branches.
     The lock stands across both rows, between the first turn and the rail. */
  #debugbar .dbg-half .dbg-rail { height: 40px; }
  #debugbar.dbg-split-on .dbg-time {
    grid-template-columns: auto auto auto auto minmax(0, 1fr) auto; grid-template-rows: 40px 40px;
    column-gap: 8px; align-items: center;
  }
  #debugbar.dbg-split-on .dbg-lane { display: contents; }
  #debugbar.dbg-split-on .dbg-lane > * { grid-row: 1 / span 2; }
  #debugbar.dbg-split-on .dbg-lane[data-dbg-lane="a"] > * { grid-row: 1; }
  #debugbar.dbg-split-on .dbg-lane[data-dbg-lane="b"] > * { grid-row: 2; }
  #debugbar.dbg-split-on .dbg-lane-name { grid-column: 1; }
  #debugbar.dbg-split-on .dbg-lane select.dbg-agent { grid-column: 2; }
  #debugbar.dbg-split-on .dbg-start { grid-column: 3; }
  #debugbar.dbg-split-on .dbg-rail { grid-column: 5; }
  #debugbar.dbg-split-on .dbg-finish { grid-column: 6; }
  /* The two ends of the game are named once, by the trunk. */
  #debugbar .dbg-half .dbg-start, #debugbar .dbg-half .dbg-finish { display: none; }
  #debugbar.dbg-split-on .dbg-lane[data-dbg-lane=""] .dbg-start,
  #debugbar.dbg-split-on .dbg-lane[data-dbg-lane=""] .dbg-finish { width: auto; }
  /* The trunk is drawn and not pressed: what is pressed is a half's slider. */
  #debugbar.dbg-split-on .dbg-lane[data-dbg-lane=""] .dbg-rail { height: auto; align-self: stretch; pointer-events: none; }
  /* --dbg-fork is how far along the rail the last turn the halves hold as
     one world stands. What a half holds past it is the half's own, and is
     drawn in the half's row from there: the turns it has played, in its
     colour. Where the two worlds differ (dbg-forked) the rail itself
     branches: the trunk is shown up to that turn, and each half's own rail
     from it, with a ruler of its own. Until then the trunk is the whole
     timeline, and a half's rail shows only when its slider has the keys. */
  #debugbar.dbg-split-on .dbg-half .dbg-deco { clip-path: inset(0 0 0 calc(100% * var(--dbg-fork, 1))); }
  #debugbar.dbg-forked .dbg-lane[data-dbg-lane=""] .dbg-sky {
    right: calc((100% - 36px) * (1 - var(--dbg-fork, 1)) + 18px); border-right: 0; border-radius: 12px 0 0 12px;
  }
  #debugbar.dbg-forked .dbg-lane[data-dbg-lane=""] .dbg-deco { clip-path: inset(0 calc(100% * (1 - var(--dbg-fork, 1))) 0 0); }
  #debugbar.dbg-forked .dbg-half .dbg-sky {
    left: calc((100% - 36px) * var(--dbg-fork, 1) + 18px); border-left: 0; border-radius: 0 12px 12px 0;
  }
  #debugbar:not(.dbg-forked) .dbg-half .dbg-sky { background: none; border-color: transparent; box-shadow: none; }
  #debugbar:not(.dbg-forked) .dbg-half .dbg-ruler { visibility: hidden; }
  /* The turn the timeline branches on, said where it does: between the two
     branches, at their start. */
  #debugbar .dbg-fork {
    position: absolute; z-index: 2; top: 50%; transform: translate(6px, -50%); padding: 2px 7px; border-radius: 999px;
    white-space: nowrap; font-size: 11px; font-weight: 700; font-variant-numeric: tabular-nums;
    color: #ffffff; background: #0b3554; box-shadow: 0 0 0 1px #d3f2ff;
  }
  /* The lock that joins the two halves of a split. A bracket runs from the
     middle of one half's row to the middle of the other's with the lock on
     it: whole and bright while the halves are locked on the same turn,
     broken and dim while each keeps its own. */
  #debugbar .dbg-tie { display: none; }
  #debugbar.dbg-split-on .dbg-tie {
    display: block; grid-column: 4; grid-row: 1 / span 2; align-self: stretch; position: relative;
    width: 32px; margin-right: -8px;
  }
  #debugbar .dbg-tie::before {
    content: ""; position: absolute; left: 14px; right: 0; top: 20px; bottom: 20px;
    border: 2px dashed #6f8a80; border-right: 0; border-radius: 9px 0 0 9px;
  }
  #debugbar.dbg-locked .dbg-tie::before { border-style: solid; border-color: #ffd23f; }
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
    content: ""; position: absolute; z-index: -1; left: -2px; top: 50%; width: 4px; height: 44px;
    border-radius: 2px; background: #ffd23f; box-shadow: 0 0 0 1px #3a1d0060;
  }
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
  /* A half's band stands under its ruler. The trunk's runs from the top of
     A's to the bottom of B's, so the one band becomes the two. */
  #debugbar.dbg-split-on .dbg-band { top: 16px; bottom: 5px; }
  #debugbar .dbg-band.dbg-own { background: var(--dbg-own); }
  /* When turns were thinned to make room, the band pales and the turns that
     can still be gone back to stand out on it. */
  #debugbar .dbg-thinned .dbg-band { background: #fff4b8; box-shadow: 0 0 0 1px #b8770a40; }
  #debugbar .dbg-kept-turns i { position: absolute; top: 20px; bottom: 20px; width: 2px; margin-left: -1px; background: #e08a00; }
  /* Where a run is headed. */
  #debugbar .dbg-goal {
    position: absolute; top: 20px; bottom: 20px; border-right: 2px solid var(--dbg-own);
    background: repeating-linear-gradient(-45deg, #ffffffc0 0 5px, transparent 5px 10px);
  }
  #debugbar.dbg-split-on .dbg-goal { top: 16px; bottom: 5px; }
  /* Kept turns ahead that another Civ AI played: not this seat's line, and
     played over by the one picked now. */
  #debugbar .dbg-beyond {
    position: absolute; top: 25px; bottom: 25px; min-width: 4px; border-radius: 0 5px 5px 0;
    background: repeating-linear-gradient(90deg, #3f5f7ad0 0 4px, transparent 4px 8px); box-shadow: 0 0 0 1px #3f5f7a90;
  }
  #debugbar.dbg-split-on .dbg-beyond { top: 20px; bottom: 9px; }
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
  /* Two halves on one turn of the trunk have one marker, ringed in both
     their colours. */
  #debugbar.dbg-split-on .dbg-lane[data-dbg-lane=""] .dbg-cursor b {
    min-width: 48px; border: 3px solid transparent; box-shadow: 0 3px 9px #0b3554a0;
    background: linear-gradient(#ffffff, #ffffff) padding-box, linear-gradient(180deg, #ff6b4a 50%, #b06cff 50%) border-box;
  }
  /* Locked, a place on the timeline is both halves': the line under the
     pointer runs the height of both rows. */
  #debugbar.dbg-locked .dbg-lane[data-dbg-lane="a"] .dbg-ghost { bottom: -44px; }
  #debugbar.dbg-locked .dbg-lane[data-dbg-lane="b"] .dbg-ghost { top: -44px; }
  /* Save points stand on a rail as flags and turn breakpoints as red dots,
     each at its own turn. */
  #debugbar .dbg-marks i {
    position: absolute; top: -5px; width: 11px; height: 11px; margin-left: -6px; cursor: pointer; pointer-events: auto;
    border-radius: 2px 2px 50% 50%; border: 1px solid #10140f; background: var(--gold-bright, #e7c873);
  }
  #debugbar .dbg-marks i.auto { background: #8fb4a8; }
  #debugbar .dbg-marks i.stop { top: auto; bottom: -5px; border-radius: 50%; background: #e2574c; }
  /* Two lanes stand four pixels apart, so there a mark keeps to its own rail:
     A's breakpoint would otherwise sit under B's save point on the same turn. */
  #debugbar.dbg-split-on .dbg-marks i { top: -2px; }
  #debugbar.dbg-split-on .dbg-marks i.stop { top: auto; bottom: -2px; }
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
     the timeline, and its line runs the height of both rows. */
  #debugbar .dbg-marks u.dbg-both { top: -44px; }
  /* Unlocked, it is neither half's to go to: each has its own slider. */
  #debugbar .dbg-marks u.dbg-still b { pointer-events: none; cursor: default; }
  /* What letting go of the timeline here would do, said under the pointer. */
  #debugbar .dbg-hover {
    position: absolute; z-index: 4; top: 82px; transform: translateX(-50%); padding: 4px 10px;
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
  #debugbar.dbg-merged .dbg-hover { top: 98px; }
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
// The tools at each end may wrap to two rows of 26px buttons; taller than
// this, they have wrapped to three, and the words on them go. A box at least
// `DEBUG_ROOMY` wide has room for the Civ AI picker's label as well.
const DEBUG_DECK_ROWS = 70, DEBUG_ROOMY = 1480;
// The turns Auto-play is set to when the box is built.
const DEBUG_AUTOPLAY_TURNS = 10;
// The turn counts Auto-play offers, with Era among them: played until a new
// era begins. An era is held open 40 to 60 Standard turns (`ERA_MINIMUM_TURNS`
// and `ERA_MAXIMUM_TURNS` in src/game.rs), scaled by the game's speed, so Era
// stands where the middle of that span falls: between 30 and 40 on Online
// (27 to 40 turns, the stock speed), and later on slower speeds.
const DEBUG_AUTOPLAY_CHOICES = [1, 2, 3, 4, 5, 10, 20, 30, 40, 50, 100, 150, 200, 250];
const DEBUG_ERA_TURNS = {online: 33, quick: 42, standard: 50, epic: 65, marathon: 100};
function debugEraAfter() {
  const turns = DEBUG_ERA_TURNS[(state || {}).game_speed] || DEBUG_ERA_TURNS.online;
  return DEBUG_AUTOPLAY_CHOICES.filter(choice => choice <= turns).pop();
}
// A game of another speed moves Era to its place, and what is picked stays.
function debugPlaceEra(select) {
  const era = select.querySelector('option[value="era"]');
  const after = select.querySelector(`option[value="${debugEraAfter()}"]`);
  if (!era || !after || after.nextElementSibling === era) return;
  const pick = select.value;
  after.after(era);
  select.value = pick;
}
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
// and null for an ordinary page.
const DEBUG_PANE = (() => {
  try {
    const pane = new URLSearchParams(location.search).get("pane");
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
}
const DEBUG_TRAVEL_OPS = new Set(["back", "forward", "jump", "restart", "undo", "copy", "goto_save"]);
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
// Why the engine stopped a run, in a line, with somewhere to look if it has one.
function debugSayStop(stopped) {
  if (!stopped) return;
  if (stopped.kind === "turn") debugSay(`Stopped at turn ${stopped.turn}`);
  else if (stopped.kind === "era") {
    const era = ((debugTimeline() || {}).eras || []).find(began => began.era === stopped.era) || {era: stopped.era};
    debugSay(`Stopped on turn ${stopped.turn}: the ${debugEraName(era)} era began`);
  }
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
  try {
    do {
      const next = await debugOp({op: "step", strategy: debugStrategy()}, AUTOPLAY_BATCH_TIMEOUT_MS, run);
      if (!next || next.error) break;
      played += 1;
      debugPlayed = played;
      const stopped = (next.debug || {}).stopped;
      if (stopped) { debugSayStop(stopped); break; }
      if (until !== null && next.turn >= until) { debugSay(`Played to turn ${next.turn}`); break; }
      if (dawn(next)) { debugSayStop({kind: "era", turn: next.turn, era: next.world_era}); break; }
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
// out), or every turn the game has left, and a breakpoint stops it early.
// Pressed while anything is being played, it stops that.
function debugAutoPlay() {
  if (debugRunning) { debugStopAsked = true; drawDebugBar(); return; }
  if (!debugTimeline()) return;
  const pick = document.getElementById("dbg-turns").value || String(DEBUG_AUTOPLAY_TURNS);
  if (pick === "era") { debugRun(null, state.world_era ?? 0); return; }
  debugRun(pick === "all" ? null : state.turn + Math.max(1, parseInt(pick, 10) || 1));
}
// The turn buttons walk the turns already played, and past the last of them
// the Civ AI plays on. Going on does not play over what was played: it walks
// to the last turn first. (Dragging the timeline ahead from an earlier turn
// is the way to play those turns again; it says so under the pointer.)
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
        <span class="dbg-lane-name">${key.toUpperCase()}</span>${key ? `
        <select class="dbg-agent" data-dbg-agent="${key}" title="The Civ AI that plays ${key.toUpperCase()}'s seat when it is stepped, run or played to" aria-label="Civ AI for ${key.toUpperCase()}"></select>` : ""}
        <b class="dbg-start"></b>
        <div class="dbg-rail">
          <div class="dbg-sky"></div>
          <div class="dbg-deco"><div class="dbg-ruler"></div><div class="dbg-band"></div><div class="dbg-band dbg-own"></div><div class="dbg-beyond" hidden></div><div class="dbg-kept-turns"></div><div class="dbg-goal"></div></div>
          <div class="dbg-hands"><div class="dbg-ghost" hidden></div>${key ? "" : `<span class="dbg-fork" hidden></span>`}<div class="dbg-cursor"><b></b></div></div>
          <input type="range" id="${id}" step="1"${key ? ` data-dbg-half="${key}"` : ""} aria-label="${label}">
          <div class="dbg-marks"></div>
        </div>
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
    <div class="dbg-time">${debugLaneMarkup("", "dbg-slider", "Turn, on the whole game")}${
      debugLaneMarkup("a", "dbg-slider-a", "Turn of simulator A")}${
      debugLaneMarkup("b", "dbg-slider-b", "Turn of simulator B")}
      <div class="dbg-tie"><button class="dbg-lock" data-dbg-lock aria-pressed="true"><svg viewBox="0 0 16 18" width="15" height="17" aria-hidden="true">
        <path class="dbg-shackle dbg-shackle-shut" d="M4.5 8.5V5.5a3.5 3.5 0 0 1 7 0v3"/>
        <path class="dbg-shackle dbg-shackle-open" d="M4.5 8.5V4.5a3.5 3.5 0 0 1 7 0v.5"/>
        <rect x="2" y="8" width="12" height="9" rx="2" fill="currentColor"/></svg></button></div>
    </div>
    <div class="dbg-deck">
      <div class="dbg-side dbg-left">
        <div class="dbg-cluster">
          <label class="dbg-field dbg-agent-field" title="The Civ AI that plays your seat when it is auto-played, stepped or played to"><span class="dbg-roomy-word">Civ AI</span>
            <select class="dbg-agent" id="dbg-agent" aria-label="Civ AI for your seat"></select></label>
          <select id="dbg-turns" title="How many turns Auto-play plays before it hands the seat back. Era is until a new era begins (in the last era, to the end of the game), and All is every turn the game has left" aria-label="Turns to auto-play">${
            DEBUG_AUTOPLAY_CHOICES.map(turns =>
              `<option value="${turns}"${turns === DEBUG_AUTOPLAY_TURNS ? " selected" : ""}>${turns} turn${turns === 1 ? "" : "s"}</option>${
                turns === debugEraAfter() ? '<option value="era">Era</option>' : ""}`).join("")
          }<option value="all">All turns</option></select>
          <button data-dbg="autoplay" title="Auto-play: the Civ AI plays the turns picked, and stops early when a breakpoint does. While it plays this counts the turns, and pressing it stops" aria-label="Auto-play"><i id="dbg-run-sign">▶▶</i><span class="dbg-word" id="dbg-run-word">Auto-play</span><span id="dbg-run-count"></span></button>
          <button class="dbg-apart" data-dbg-pop="layers" title="Layers: which decisions the Civ AI takes for you each turn">Layers<span class="dbg-count" id="dbg-layer-count"></span></button>
          <button data-dbg-pop="breaks" title="Breakpoints: what stops Auto-play and a turn played to" aria-label="Breakpoints"><span class="dbg-dot">●</span><span class="dbg-word">Breakpoints</span><span class="dbg-count" id="dbg-break-count"></span></button>
          <button data-dbg="split" title="Branch Timeline: two simulators from this turn, side by side. They begin locked on the same turn, with this one panel for both; click the lock between their timelines and each plays a history of its own" aria-label="Branch Timeline"><i>⑂</i><span class="dbg-word">Branch Timeline</span></button>
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
  for (const box of bar.querySelectorAll(".dbg-lane")) {
    const part = name => box.querySelector(`.dbg-${name}`);
    debugLanes[box.dataset.dbgLane] = {box, input: box.querySelector("input"), ruler: part("ruler"),
      band: part("band"), own: part("own"), beyond: part("beyond"), kept: part("kept-turns"), goal: part("goal"), ghost: part("ghost"),
      cursor: part("cursor"), pill: box.querySelector(".dbg-cursor b"), marks: part("marks"), agent: part("agent"),
      start: part("start"), finish: part("finish"), fork: part("fork")};
  }

  bar.addEventListener("click", event => {
    if (event.target.closest("[data-dbg-lock]")) {
      if (debugSplit) debugSplitLock(!debugSplit.locked);
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
    if (named && debugTimeline()) { debugSay(""); debugGoToTurn(named.dataset.dbgTurn); return; }
    const point = event.target.closest("[data-dbg-goto]");
    if (point && debugTimeline()) { debugSay(""); debugOp({op: "goto_save", id: +point.dataset.dbgGoto}); return; }
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
    // Side by side, at once: the strip between the halves stacks them
    // instead, and brings up head to head.
    else if (action === "split") { debugPopOpen = null; debugSplitOpen("row"); }
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
      // The page's own picker is the one every step reads.
      const page = document.getElementById("autoplaystrategy");
      if (page) { page.value = target.value; page.dispatchEvent(new Event("change", {bubbles: true})); }
      target.blur();
      drawDebugBar(false);
      debugSeat();
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
    debugHover(on && !on.closest(".dbg-marks i, .dbg-marks u b") ? on.closest(".dbg-rail") : null, event.clientX);
  });
  bar.addEventListener("pointerleave", () => debugHover(null));
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
// The ruler on a lane: a tick every few turns and a number on the round ones
// between the two ends, which are named beside the rail. `quiet` is a stretch
// of turns whose numbers are left out, because something else is said there.
function debugRulerMarkup(lo, hi, finish = null, quiet = null) {
  const length = hi - lo;
  const major = length <= 60 ? 10 : length <= 150 ? 25 : length <= 300 ? 50 : length <= 600 ? 100 : 250;
  const minor = major / 5;
  let ruler = "";
  for (let turn = Math.ceil((lo + 1) / minor) * minor; turn < hi; turn += minor) {
    const at = debugAlong(turn, lo, hi), round = turn % major === 0;
    ruler += `<i${round ? ' class="major"' : ""} style="left:${at}%"></i>`;
    if (round && !(quiet && turn >= quiet[0] && turn <= quiet[1])) ruler += `<b style="left:${at}%">${turn}</b>`;
  }
  if (finish !== null && finish > lo && finish < hi)
    ruler += `<i class="dbg-finish-line" style="left:${debugAlong(finish, lo, hi)}%" title="Turn ${finish}: the last turn of the game"></i>`;
  return ruler;
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
    const beyond = model.beyond && turn >= model.beyond.from && turn <= model.beyond.to
      ? `, over the turns ${debugAgentName(model.beyond.by)} played` : "";
    return (model.turn < model.last ? `${name} · play on from turn ${model.turn} to here` : `${name} · play to here`) + beyond;
  }
  if (turn < model.first) return `${name} · before the timeline begins, goes to turn ${model.first}`;
  if (model.kept && !model.kept.has(turn)) {
    const near = model.turns.filter(kept => kept <= turn).pop() ?? model.first;
    return `${name} · no longer kept, goes to turn ${near}`;
  }
  const flags = (model.named || model.flags || []).filter(point => point.turn === turn).map(point => ` · ⚑ ${point.name}`).join("");
  // The turn a split branches on is the last one its halves hold as one world.
  const branches = model.branches === turn ? " · the last turn A and B are the same world" : "";
  return `${name}${turn === model.turn ? " · open now" : ""}${flags}${branches}`;
}
// One lane, drawn from what it stands for: the turns the game may run to
// (`lo` to `hi`), the turns played (`first` to `last`), the turn that is open,
// and, past `fork`, the part of its history that is one half's own.
//
// A split draws three: the trunk, and a lane for each half. It says itself
// where each marker stands (`marker`, null for none), because two halves on
// one turn of the trunk have one marker between them, and on the trunk where
// the timeline branches (`branches`).
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
  const finish = model.finish ?? null, quiet = model.quiet || null;
  const axis = `${lo}:${hi}:${finish}:${quiet || ""}`;
  if (lane.ruler.dataset.axis !== axis) {
    lane.ruler.dataset.axis = axis;
    lane.ruler.innerHTML = debugRulerMarkup(lo, hi, finish, quiet);
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
  lane.box.classList.toggle("dbg-thinned", !!model.kept);
  const kept = model.kept ? `${lo}:${hi}:${model.turns.join()}` : "";
  if (lane.kept.dataset.turns !== kept) {
    lane.kept.dataset.turns = kept;
    lane.kept.innerHTML = model.kept ? model.turns.map(turn => `<i style="left:${at(turn)}%"></i>`).join("") : "";
  }
  const goal = model.goal !== null && model.goal > model.turn ? model.goal : null;
  lane.goal.hidden = goal === null;
  if (goal !== null) debugPlace(lane.goal, span(model.turn, goal));
  lane.cursor.hidden = shown === null;
  if (shown !== null) {
    debugPlace(lane.cursor, `left:${at(shown)}%`);
    if (lane.pill.textContent !== String(shown)) lane.pill.textContent = shown;
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
      `<u${era.both ? ` class="dbg-both${era.still ? " dbg-still" : ""}"` : ""} style="left:${at(era.turn)}%"><b data-dbg-turn="${era.turn}" title="The ${escapeAttr(titleCase(String(era.name || `era ${era.era}`)))} era began on turn ${era.turn}. Click to go there">${
        escapeAttr(titleCase(String(era.name || `era ${era.era}`)))}</b></u>`).join("") +
    (model.stops || []).filter(turn => turn >= lo && turn <= hi).map(turn =>
      `<i class="stop" data-dbg-turn="${turn}" style="left:${at(turn)}%" title="Breakpoint on turn ${turn}. Click to ${
        turn > last ? "play to it" : "go there"}"></i>`).join("") +
    (model.flags || []).filter(point => point.turn >= lo && point.turn <= hi).map(point =>
      `<i${point.auto ? ' class="auto"' : ""} data-dbg-goto="${point.id}" style="left:${at(point.turn)}%"` +
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
// The words on the tools are for a box with room for them: when the tools at
// either end wrap past two rows, the words go and the signs stay, first the
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
  if (fresh) { debugFitted = width; bar.classList.remove("dbg-roomy", "dbg-snug", "dbg-tight"); }
  const deck = bar.querySelector(".dbg-deck"), has = name => bar.classList.contains(name);
  const tall = () => deck.offsetHeight > DEBUG_DECK_ROWS;
  if (stacked || DEBUG_PANE !== null) bar.classList.add("dbg-tight");
  // The picker's label is tried first and is the first thing to go.
  if (fresh && width >= DEBUG_ROOMY && !has("dbg-tight")) bar.classList.add("dbg-roomy");
  if (has("dbg-roomy") && tall()) bar.classList.remove("dbg-roomy");
  if (!has("dbg-tight") && !has("dbg-snug") && tall()) bar.classList.add("dbg-snug");
  if (!has("dbg-tight") && tall()) bar.classList.add("dbg-tight");
}
// The box begins where the map area does, at the right side of the left
// panel, and the map area stands under it. Where that is and how tall the box
// is are measured, and the page is told when either changes: the map is
// fitted to the room it has. A split covers the left panel, and its box has
// the whole width.
let debugDocked = 0, debugDockedLeft = -1;
function debugMapLeft() {
  const area = document.getElementById("maparea");
  return area && !debugSplit ? Math.max(0, Math.round(area.getBoundingClientRect().left)) : 0;
}
function debugDock(bar) {
  const left = bar.hidden ? 0 : debugMapLeft();
  if (left !== debugDockedLeft) {
    debugDockedLeft = left;
    document.documentElement.style.setProperty("--dbg-left", `${left}px`);
  }
  if (!bar.hidden && (!debugSplit || bar.classList.contains("dbg-merged"))) debugFit(bar);
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
    // bar shows the two halves' history. That is one timeline for as long as
    // they hold the same world, and from the last turn they do
    // (`debugSplitFork`) a branch for each, in the half's own colour.
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
    debugDrawLane("", {name: "", ...axis, first: Math.min(fork.first.a, fork.first.b), last: fork.turn,
      turn: Math.min(panes.a.turn ?? from, panes.b.turn ?? from), fork: null, kept: null, turns: null, beyond: null,
      goal, idle: false, marker: one ? stands.a : null, branches: fork.forked ? fork.turn : null});
    // A half has no timeline of its own to show them on, so its save points,
    // its breakpoint turns and the eras it has been through stand on its
    // lane (`pane-marks`): A's in the top row of the timeline and B's in the
    // bottom row. What both halves hold on the trunk is said once there, as
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
    // The turn the timeline branches on is said between the two branches,
    // where B's ruler would have its numbers.
    const quiet = fork.forked ? [fork.turn, fork.turn + Math.ceil((axis.hi - axis.lo) * .045)] : null;
    for (const letter of ["a", "b"]) {
      const pane = panes[letter];
      debugDrawLane(letter, {name: letter.toUpperCase(), ...axis, first: fork.first[letter], last: fork.last[letter],
        turn: pane.turn ?? from, fork: fork.turn, kept: null, turns: null, ...marks[letter], named: told(letter, "flags"),
        beyond: beyond(letter), goal: fork.forked ? goal : null, idle: pane.turn !== null && !debugSplit.busy && !running,
        marker: one ? null : stands[letter], branches: fork.forked ? fork.turn : null, quiet: letter === "b" ? quiet : null});
      const agent = debugLanes[letter].agent;
      debugOfferAgents(agent, pane.strategy);
      agent.disabled = debugSplit.busy || running;
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
    if (deck === null) { if (measure) debugDock(bar); return; }
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
      eras: timeline.eras || [], goal: debugRunning ? debugGoal : null, idle: deck.idle});
  }
  debugDrawDeck(bar, deck);
  if (DEBUG_PANE !== null) { debugPaneDeck(deck); debugPaneMarks(timeline); }
  if (measure) debugDock(bar);
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
  debugOfferAgents(agent, deck.strategy);
  agent.disabled = !idle;
  agent.closest(".dbg-field").hidden = agent.options.length === 0;
  const turnsPick = document.getElementById("dbg-turns");
  turnsPick.disabled = !idle;
  debugPlaceEra(turnsPick);

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
  // The host of a split asks nothing of its own world, which waits.
  if (DEBUG_SMALL_DEVICE && !debugSplit && idle && timeline.memory_line == null && Date.now() - debugRoomAskedAt > 10000) {
    debugRoomAskedAt = Date.now();
    debugOp({op: "room", megabytes: 384});
  }

  const layersOn = timeline.layers.filter(layer => layer.on).length;
  document.getElementById("dbg-layer-count").textContent = layersOn ? `${layersOn}` : "";
  const conditions = breaks.conditions || [];
  const breakCount = breaks.turns.length + breaks.categories.length + breaks.phrases.length +
    conditions.length + (breaks.important ? 1 : 0) + (breaks.eras ? 1 : 0);
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
    const from = button.getBoundingClientRect(), box = bar.getBoundingClientRect();
    pop.style.left = `${Math.max(8, Math.min(from.left - box.left, box.width - pop.offsetWidth - 8))}px`;
    pop.style.top = `${from.bottom - box.top + 6}px`;
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
//   host → pane  pane-look    look there too
// Closing the split is the host's own request to the engine: the half that
// is kept becomes the host's session (`op: "adopt"`), timeline and all.
//
// The host's bar stays up across the top of the window while the screen is
// split, and its timeline is the two halves': one timeline for as long as
// they hold the same world, and a branch for each from the turn their worlds
// differ (`debugSplitFork`). Each half has a slider the length of the game,
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
// Where the timeline of a split branches: the last turn its two halves hold
// as the same world, and whether each has since played a turn of its own
// (operator 2026-11-21: "branching the timeline should keep 1 timeline where
// they share the timeline and then branch to 2 when they actually diverge in
// simulated worlds").
//
// Which worlds are the same is the engine's to say. Each half tells the host
// the turns it holds and the engine's print of each one's world
// (`pane-marks`, from `debug.turns` and `debug.worlds`), and two halves hold
// the same world on a turn they both give the same print for. A print carries
// everything played before its turn, so the last turn they agree on is the
// last one they share.
//
// A half that has only played further than the other has not left it: the
// other may yet play the same turns. The timeline branches once both hold a
// turn past the last one they share, which is a turn that is two worlds.
//
// Also given: the first and the last turn each half holds.
function debugSplitFork() {
  const from = debugSplit.from ?? state.turn;
  const lines = {}, first = {}, last = {};
  for (const letter of ["a", "b"]) {
    const pane = debugSplit.panes[letter], marks = pane.marks || {}, turn = pane.turn ?? from;
    const line = Array.isArray(marks.turns) && marks.turns.length > 0 && Array.isArray(marks.worlds) &&
      marks.worlds.length === marks.turns.length ? marks : null;
    lines[letter] = line;
    first[letter] = Math.min(line ? line.turns[0] : pane.first ?? turn, turn);
    last[letter] = Math.max(line ? line.turns[line.turns.length - 1] : pane.last ?? from, turn);
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
  return {turn: shared, forked: Math.min(last.a, last.b) > shared, first, last};
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
  for (const [letter, other] of [["a", b], ["b", a]]) {
    const pane = debugSplit.panes[letter];
    if (pane.told === stands && again !== letter) continue;
    pane.told = stands;
    debugSplitTell(letter, {civvis: "pane-lock", on: debugSplit.locked, twin: twins ? other.id : null});
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
    const {turns, categories, phrases, important, every_seat, eras, conditions} = mine.breakpoints;
    ask({op: "breakpoints", turns, categories, phrases, important, every_seat, eras, conditions});
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
  if (!debugSplit) return;
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
function debugSplitOpen(dir, versus = null) {
  if (debugSplit || DEBUG_PANE !== null || !debugTimeline() || debugBusy || autoplaying) return;
  if (!document.getElementById("debugsplit-style")) {
    const style = document.createElement("style");
    style.id = "debugsplit-style";
    style.textContent = DEBUG_SPLIT_STYLE;
    document.head.appendChild(style);
  }
  const nonce = Date.now().toString(36);
  const host = document.createElement("div");
  host.id = "debugsplit";
  host.innerHTML = `
    <iframe data-dbg-frame="a" title="Simulator A"></iframe>
    <div class="dbg-divider" role="toolbar" aria-label="Split simulators" title="Drag to resize; double-click to make the halves equal">
      <div class="dbg-pane-name" data-dbg-name="a"></div>
      <button data-dbg-split="copy-a" title="Make B a copy of A as it stands now, turns behind it included">A → B</button>
      <button data-dbg-split="keep-a" title="Close the split and keep playing A, with every turn it can walk back to">Keep A</button>
      <span class="dbg-gap"></span>
      <button data-dbg-split="view">View</button>
      <button data-dbg-split="flip" title="Switch between side by side and top and bottom"></button>
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
      <button data-dbg-split="keep-b" title="Close the split and keep playing B, with every turn it can walk back to">Keep B</button>
      <button data-dbg-split="copy-b" title="Make A a copy of B as it stands now, turns behind it included">B → A</button>
      <div class="dbg-pane-name" data-dbg-name="b"></div>
    </div>
    <iframe data-dbg-frame="b" title="Simulator B"></iframe>`;
  let remembered = false;
  try { remembered = localStorage.getItem("civvis-debug-versus") === "1"; } catch (_) {}
  debugSplit = {dir, locked: true, settling: true, view: true, share: .5, busy: false, panes: {}, host,
    versus: versus ?? remembered, running: 0, until: null, chart: "score", history: {a: new Map(), b: new Map()},
    from: state ? state.turn : null};
  for (const letter of ["a", "b"]) {
    const frame = host.querySelector(`[data-dbg-frame="${letter}"]`);
    // A half starts on the Civ AI picked here, and is told if it loads on another.
    debugSplit.panes[letter] = {frame, id: `${letter}-${nonce}`, turn: null, first: null, last: null,
      deck: null, told: null, strategy: debugStrategy()};
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
    else if (action === "keep-a") debugSplitClose("a");
    else if (action === "keep-b") debugSplitClose("b");
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
    pane.frame.src = `${location.pathname}?pane=${pane.id}`;
  }
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
async function debugSplitClose(keep) {
  if (!debugSplit || debugSplit.busy) return;
  debugSplit.busy = true; drawDebugSplit();
  let note = "";
  try {
    const next = await fetchJSON("/debug", {method: "POST",
      headers: {"Content-Type": "application/json"},
      body: JSON.stringify({op: "adopt", pane: debugSplit.panes[keep].id})}, 60000);
    if (next.error) throw new Error(next.error);
    debugForgetStandingOrders(next.turn);
    sel = null; selCity = null;
    // The half that is kept keeps its Civ AI: the seat is still that agent's.
    const page = document.getElementById("autoplaystrategy"), kept = debugSplit.panes[keep].strategy;
    if (page && kept && [...page.options].some(option => option.value === kept)) page.value = kept;
    debugRememberSeat();
    render(next);
    note = `Playing on from simulator ${keep.toUpperCase()}`;
  } catch (error) {
    note = `Simulator ${keep.toUpperCase()} could not be kept: ${error.message || error}`;
  }
  debugSplit.host.remove();
  debugSplit = null;
  debugSay(note);
}
window.addEventListener("message", event => {
  if (!debugSplit || event.origin !== location.origin) return;
  const data = event.data;
  if (!data || typeof data.civvis !== "string") return;
  const letter = ["a", "b"].find(name => debugSplit.panes[name].frame.contentWindow === event.source);
  if (!letter) return;
  const other = letter === "a" ? "b" : "a";
  if (data.civvis === "pane-turn") {
    debugSplit.panes[letter].turn = data.turn;
    // A half that has just loaded is told how the split stands.
    if (data.how === "boot") debugSplitSync(letter);
    drawDebugSplit();
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
      eras: Array.isArray(data.eras) ? data.eras : [],
      turns: Array.isArray(data.turns) ? data.turns : null, worlds: Array.isArray(data.worlds) ? data.worlds : null};
    drawDebugBar();
  } else if (data.civvis === "pane-deck") {
    debugSplit.panes[letter].deck = data.deck;
    debugSplitSettle();
    drawDebugBar();
  } else if (data.civvis === "pane-camera") {
    if (debugSplit.view) debugSplitTell(other, {civvis: "pane-look", view: data.view});
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
    drawDebugSplit();
  } else if (data.civvis === "pane-ran") {
    debugSplit.running = Math.max(0, debugSplit.running - 1);
    if (debugSplit.running === 0) debugSplit.until = null;
    drawDebugSplit();
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
  const said = JSON.stringify([flags, stops, beyond, eras, turns, worlds]);
  if (said === debugMarksSaid) return;
  debugMarksSaid = said;
  window.parent.postMessage({civvis: "pane-marks", pane: DEBUG_PANE, flags, stops, beyond, eras, turns, worlds},
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
  const numbers = {score: seat.score, cities: seat.cities, population: seat.population,
    science: yields.science, culture: yields.culture, gold: yields.gold, faith: yields.faith,
    military: seat.military};
  const last = Math.max((state.debug || {}).last ?? state.turn, state.turn);
  const first = Math.min((state.debug || {}).first ?? state.turn, state.turn);
  const said = JSON.stringify([state.turn, first, last, numbers, debugStrategy() || ""]);
  if (said === debugNumbersSaid) return;
  debugNumbersSaid = said;
  window.parent.postMessage({civvis: "pane-numbers", pane: DEBUG_PANE, numbers, turn: state.turn, first, last,
    strategy: debugStrategy() || ""}, location.origin);
}
// Reach the turn the other pane is on, the way it got there: walked along
// the timeline, stepped by the Civ AI, or by ending the turn as it stands.
async function debugFollow(turn, how) {
  debugFollowWanted = {turn, how};
  if (debugFollowing) return;
  debugFollowing = true;
  try {
    for (let guard = 0; guard < 1000 && debugFollowWanted; guard++) {
      const wanted = debugFollowWanted;
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
function debugCameraLook(view) {
  if (!state || !view || !view.pan) return;
  cameraFlight = null; cameraZoom = null;
  clearCameraInertia();
  cam.x = +view.x; cam.y = +view.y; cam.scale = +view.scale; cam.rot = +view.rot; cam.tilt = +view.tilt;
  SKY_PAN = {x: +view.pan.x, y: +view.pan.y};
  draw(); drawMini();
  debugCameraSaid = JSON.stringify(debugCameraNow());
}
if (DEBUG_PANE !== null) {
  window.addEventListener("message", async event => {
    if (event.source !== window.parent || event.origin !== location.origin) return;
    const data = event.data;
    if (!data || typeof data.civvis !== "string") return;
    if (data.civvis === "pane-follow") debugInTurn(() => debugFollow(data.turn, data.how));
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
    else if (data.civvis === "pane-look") debugCameraLook(data.view);
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

// `render` is the one door a state comes through, whoever asked for it.
// `drawTurnLoop` is the page's own word that the turn's standing changed —
// an end of turn in flight, a request finished — which is exactly when the
// bar's buttons change what they may do, with no new state to say so.
{
  const drawState = render;
  render = function (st, ...rest) {
    const drawn = drawState(st, ...rest);
    try { drawDebugBar(); debugPaneReport(); debugPaneNumbers(); } catch (error) { console.error("debugger bar", error); }
    return drawn;
  };
  // In the middle of a round stepped a seat at a time the turn is not the
  // person's. Ending it means letting the rest of the round play, and
  // nothing else is theirs to do until it comes back.
  const sendAction = send;
  send = function (action, ...rest) {
    const timeline = debugTimeline();
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
if (state) { drawDebugBar(); debugPaneReport(); debugPaneNumbers(); }
