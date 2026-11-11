"use strict";
// The main menu of the published site.
//
// civvis.ai lands on a live game of AIs, and this screen does not stand in
// front of it. It opens only when someone asks: `?menu` in the address, or
// from inside a game. Game setup in the sidebar stays the way in from the
// landing page.
//
// Nothing here builds a game. Every control writes the same setup selects the
// sidebar uses and "Begin game" calls the same `startNewSimulation()`, so the
// two ways in cannot disagree about what a game is.
//
// It is a third classic script for the same reason `app_setup.js` is a second:
// it reads shared top-level bindings (`state`, `RULES`) and helpers from the
// scripts before it, and keeping it apart keeps this work out of the renderer.

// The desktop app and the operator's exhibition tabs never show the menu:
// the shim's presence is the whole condition.
const frontDoorHost = () => !!window.CIVVIS_PUBLISHED_BUILD;
// A visitor lands on the AI game, so only the address can ask for this at load.
function frontDoorWanted() {
  return frontDoorHost() && new URLSearchParams(location.search).has("menu");
}
const fdElement = id => document.getElementById(id);
const FD_ESCAPE = text => String(text ?? "").replace(/[&<>"']/g,
  c => ({"&": "&amp;", "<": "&lt;", ">": "&gt;", "\"": "&quot;", "'": "&#39;"}[c]));
let fdChoice = "";       // the civ picked on the Create Game screen ("" = random)
let fdLeaderFilter = "";
// A stable hue from a civ's name: the tiles need to be told apart at a glance
// and must not change between visits. Art for fifty leaders would be better;
// this is what can be honest about being a crest and not a portrait.
function fdHue(name) {
  let hash = 0;
  for (const char of String(name)) hash = (hash * 31 + char.charCodeAt(0)) >>> 0;
  return hash % 360;
}
function fdMonogram(name) {
  const words = String(name).split(/\s+/).filter(Boolean);
  return (words.length > 1 ? words[0][0] + words[1][0] : words[0].slice(0, 2)).toUpperCase();
}
function fdCrest(name, size = "") {
  return `<span class="fd-crest ${size}" style="--crest:${fdHue(name)}" aria-hidden="true">` +
    `<span>${FD_ESCAPE(fdMonogram(name))}</span></span>`;
}
function fdLeaders() {
  const pools = Array.isArray(RULES?.leader_pools) ? RULES.leader_pools : [];
  const pool = pools.find(entry => entry.id === "civ6") || pools[0];
  return (pool?.leaders || []).filter(entry => entry.available !== false)
    .slice().sort((a, b) => a.civ.localeCompare(b.civ));
}
function fdCivRules(civ) {
  const civs = RULES?.civs;
  if (!civs) return null;
  return civs[civ] || civs[String(civ).toLowerCase()] ||
    civs[String(civ).toLowerCase().replaceAll(" ", "_")] || null;
}
// "Royal Road: +1 gold and +2 combat strength" reads better split.
function fdAbility(civ) {
  // A note can end by citing the engine's trait ids, which are for the audit
  // and not for a player. They stay in the data: its fingerprint is audited.
  const note = (fdCivRules(civ)?.note || "").replace(/\s*\((?=[^)]*_)[A-Z0-9_ +]+\)/g, "");
  const colon = note.indexOf(":");
  const sentence = text => text.charAt(0).toUpperCase() + text.slice(1);
  return colon < 0 ? {name: "", text: sentence(note)}
    : {name: note.slice(0, colon), text: sentence(note.slice(colon + 1).trim())};
}
function fdDifficultyNote(id) {
  const rules = RULES?.difficulties?.[id];
  if (!rules) return "";
  const edge = rules.human_combat_strength || 0;
  const push = rules.ai_yield_pct?.production || 0;
  const head = Object.values(rules.ai_bonus_units || {}).reduce((sum, n) => sum + n, 0);
  if (edge > 0) return `Forgiving. Your units fight at +${edge} strength.`;
  if (!push) return "An even match: nobody gets a head start.";
  return `Rivals build ${push}% faster` +
    (head ? ` and start with ${head} extra units.` : ".");
}
function fdSelectOptions(id) {
  return [...(fdElement(id)?.options || [])]
    .filter(option => !option.disabled && !option.hidden)
    .map(option => ({value: option.value, label: option.textContent, selected: option.selected}));
}
function fdSet(id, value) {
  const select = fdElement(id);
  if (!select || select.value === String(value)) return;
  select.value = String(value);
  select.dispatchEvent(new Event("change", {bubbles: true}));
}
function fdShow(panel) {
  fdElement("fd-menu").hidden = panel !== "menu";
  fdElement("fd-create").hidden = panel !== "create";
  fdElement("fd-pause").hidden = panel !== "pause";
  fdElement("fd-intro").hidden = panel !== "intro";
  fdElement("frontdoor").classList.toggle("fd-centered", panel === "pause" || panel === "intro");
  const focus = {create: "fd-begin", pause: "fd-resume", intro: "fd-intro-go"}[panel] || "fd-play";
  fdElement(focus)?.focus({preventScroll: true});
}
function fdRenderLeaders() {
  const grid = fdElement("fd-leaders");
  if (!grid) return;
  const needle = fdLeaderFilter.trim().toLowerCase();
  const leaders = fdLeaders().filter(entry => !needle ||
    `${entry.civ} ${entry.leader}`.toLowerCase().includes(needle));
  const tile = (civ, title, sub, crest) =>
    `<button type="button" class="fd-leader${fdChoice === civ ? " on" : ""}" role="radio" ` +
    `aria-checked="${fdChoice === civ}" data-civ="${FD_ESCAPE(civ)}">${crest}` +
    `<span class="fd-leader-text"><b>${FD_ESCAPE(title)}</b><small>${FD_ESCAPE(sub)}</small></span></button>`;
  grid.innerHTML =
    (needle ? "" : tile("", "Random", "Surprise me",
      `<span class="fd-crest fd-crest-random" aria-hidden="true"><span>?</span></span>`)) +
    leaders.map(entry => tile(entry.civ, entry.civ, entry.leader, fdCrest(entry.civ))).join("") +
    (leaders.length ? "" : `<p class="fd-empty">No civilization matches that.</p>`);
}
function fdRenderDossier() {
  const box = fdElement("fd-dossier");
  if (!box) return;
  if (!fdChoice) {
    box.innerHTML = `<span class="fd-crest fd-crest-random fd-crest-large" aria-hidden="true"><span>?</span></span>` +
      `<div class="fd-dossier-text"><h3>Any civilization</h3>` +
      `<p class="fd-dossier-sub">History decides who you are.</p>` +
      `<p class="fd-ability">Every civilization has its own strengths. Play the hand you are dealt, or pick one from the list.</p></div>`;
    return;
  }
  const leader = fdLeaders().find(entry => entry.civ === fdChoice);
  const ability = fdAbility(fdChoice);
  box.innerHTML = fdCrest(fdChoice, "fd-crest-large") +
    `<div class="fd-dossier-text"><h3>${FD_ESCAPE(leader?.leader || fdChoice)}</h3>` +
    `<p class="fd-dossier-sub">leads ${FD_ESCAPE(fdChoice)}</p>` +
    (ability.name ? `<p class="fd-ability"><b>${FD_ESCAPE(ability.name)}</b> ${FD_ESCAPE(ability.text)}</p>`
      : (ability.text ? `<p class="fd-ability">${FD_ESCAPE(ability.text)}</p>` : "")) + `</div>`;
}
function fdRenderOptions() {
  const difficulty = fdElement("fd-difficulty");
  if (difficulty) {
    const options = fdSelectOptions("difficulty");
    difficulty.innerHTML = options.map(option =>
      `<button type="button" role="radio" aria-checked="${option.selected}" ` +
      `class="fd-step${option.selected ? " on" : ""}" data-value="${FD_ESCAPE(option.value)}">` +
      `${FD_ESCAPE(option.label)}</button>`).join("");
    const chosen = options.find(option => option.selected);
    fdElement("fd-difficulty-note").textContent = chosen ? fdDifficultyNote(chosen.value) : "";
  }
  for (const [mirror, source] of [["fd-map", "maptype"], ["fd-size", "np"], ["fd-speed", "gamespeed"]]) {
    const select = fdElement(mirror);
    if (!select) continue;
    select.innerHTML = fdSelectOptions(source).map(option =>
      `<option value="${FD_ESCAPE(option.value)}"${option.selected ? " selected" : ""}>${FD_ESCAPE(option.label)}</option>`).join("");
  }
}
function fdRenderCreate() {
  // The engine can still be arriving: the page paints before the module loads,
  // and everything this screen offers is read from `/rules`. Hold the button
  // and keep looking rather than offering a game with no civilizations in it.
  const begin = fdElement("fd-begin");
  begin.disabled = !RULES;
  if (!RULES) {
    begin.textContent = "Loading…";
    setTimeout(() => { if (!fdElement("fd-create").hidden) fdRenderCreate(); }, 250);
  } else begin.textContent = "Begin game";
  // The sidebar's own form holds the choices. Single player is chosen first
  // because that is what makes the leader and difficulty rows mean something.
  fdSet("humanplayers", "single");
  fdRenderLeaders();
  fdRenderDossier();
  fdRenderOptions();
}
async function fdRefreshContinue() {
  const button = fdElement("fd-continue");
  if (!button) return;
  // A game on the table is what Continue means, and it goes straight back to
  // it. Only an AI game in the background sends a player to their saves.
  if (state && !SPEC) {
    const me = state.players?.[0];
    delete button.dataset.save;
    button.hidden = false;
    fdElement("fd-continue-sub").textContent = `${me?.civ || "Your game"} · Turn ${state.turn}` +
      (state.difficulty ? ` · ${titleCase(state.difficulty)}` : "");
    return;
  }
  let saves = [];
  try { saves = (await fetchJSON("/saves", {}, 4000)).saves || []; } catch (_) { saves = []; }
  // The list comes newest first, so this is the seat they last left, whether
  // the game saved itself or they did.
  const latest = saves[0];
  button.hidden = !latest;
  if (!latest) return;
  button.dataset.save = latest.name;
  fdElement("fd-continue-sub").textContent =
    `${latest.civ ? `${latest.civ} · ` : ""}Turn ${latest.turn}` +
    (latest.difficulty ? ` · ${titleCase(latest.difficulty)}` : "");
}
function closeFrontDoor() {
  const door = fdElement("frontdoor");
  if (!door) return;
  door.hidden = true;
  document.body.classList.remove("frontdoor-open", "frontdoor-over-game");
}
function openFrontDoor(panel = "menu") {
  const door = fdElement("frontdoor");
  if (!door) return;
  door.hidden = false;
  document.body.classList.add("frontdoor-open");
  fdRefreshContinue();
  if (panel === "create") fdRenderCreate();
  fdShow(panel);
}
// The menu a game keeps: Escape, or the top bar's menu button. Over a game the
// chrome stays where it is, because this is a pause and not a change of scene.
function openPauseMenu() {
  if (!frontDoorHost() || !state || SPEC) return false;
  fdElement("frontdoor").hidden = false;
  document.body.classList.add("frontdoor-over-game");
  fdElement("fd-sound-label").textContent = `Sound: ${soundEnabled ? "on" : "off"}`;
  fdElement("fd-save-note").textContent = "Your game also saves itself every turn";
  fdShow("pause");
  return true;
}
async function fdSaveGame() {
  const note = fdElement("fd-save-note");
  const name = `${(state.players?.[0]?.civ || "game").replace(/[^A-Za-z0-9]+/g, "-")}-turn-${state.turn}`;
  note.textContent = "Saving…";
  try {
    const done = await fetchJSON("/save", {method: "POST",
      headers: {"Content-Type": "application/json"}, body: JSON.stringify({name})});
    note.textContent = done.error ? `Could not save: ${done.error}` : `Saved as ${name}`;
  } catch (error) {
    note.textContent = `Could not save: ${error.message || error}`;
  }
}
// The first screen of a game says who you are and what you are trying to do,
// once, before the map asks for anything.
function fdShowIntro() {
  const me = state?.players?.[0];
  if (!me || SPEC || state.turn !== 1) return;
  const ability = fdAbility(me.civ);
  fdElement("fd-intro-crest").innerHTML = fdCrest(me.civ, "fd-crest-large");
  fdElement("fd-intro-kicker").textContent = `${me.civ} · ${titleCase(state.difficulty || "")}`;
  fdElement("fd-intro-leader").textContent = me.leader || me.civ;
  fdElement("fd-intro-ability").innerHTML = ability.name
    ? `<b>${FD_ESCAPE(ability.name)}</b>${FD_ESCAPE(ability.text)}` : FD_ESCAPE(ability.text);
  fdElement("fd-intro-goal").textContent =
    "Found your capital, then build, research and expand. Right-click a tile to move a unit.";
  fdElement("frontdoor").hidden = false;
  document.body.classList.add("frontdoor-over-game");
  fdShow("intro");
}
async function fdBegin() {
  fdSet("humanplayers", "single");
  fdSet("leader", fdChoice);
  // A game begun here is a fresh map. The seed field is blank unless somebody
  // asked Advanced setup to replay a world, and that request was made of the
  // start control beside it, not of this door.
  fdSet("mapseed", "");
  closeFrontDoor();
  await startNewSimulation();
  fdShowIntro();
}
function fdWire() {
  const door = fdElement("frontdoor");
  fdElement("fd-play").onclick = () => { fdRenderCreate(); fdShow("create"); };
  fdElement("fd-watch").onclick = () => closeFrontDoor();
  fdElement("fd-back").onclick = () => fdShow("menu");
  fdElement("fd-begin").onclick = fdBegin;
  fdElement("fd-resume").onclick = () => closeFrontDoor();
  fdElement("fd-intro-go").onclick = () => closeFrontDoor();
  fdElement("fd-save").onclick = fdSaveGame;
  fdElement("fd-sound").onclick = () => {
    toggleSound();
    fdElement("fd-sound-label").textContent = `Sound: ${soundEnabled ? "on" : "off"}`;
  };
  fdElement("fd-settings").onclick = () => { closeFrontDoor(); togglePanel(false); };
  fdElement("fd-exit").onclick = () => { closeFrontDoor(); openFrontDoor(); };
  fdElement("fd-continue").onclick = async () => {
    const name = fdElement("fd-continue").dataset.save;
    fdSet("humanplayers", "single");
    closeFrontDoor();
    if (name) await loadSave(name);
  };
  fdElement("fd-advanced").onclick = () => {
    fdSet("humanplayers", "single");
    fdSet("leader", fdChoice);
    closeFrontDoor();
    const setup = fdElement("setupsec");
    if (setup) { setup.open = true; setup.scrollIntoView?.({block: "nearest"}); }
  };
  fdElement("fd-leaders").addEventListener("click", event => {
    const tile = event.target.closest(".fd-leader");
    if (!tile) return;
    fdChoice = tile.dataset.civ;
    fdRenderLeaders();
    fdRenderDossier();
  });
  fdElement("fd-difficulty").addEventListener("click", event => {
    const step = event.target.closest(".fd-step");
    if (!step) return;
    fdSet("difficulty", step.dataset.value);
    fdRenderOptions();
  });
  for (const [mirror, source] of [["fd-map", "maptype"], ["fd-size", "np"], ["fd-speed", "gamespeed"]])
    fdElement(mirror).addEventListener("change", event => {
      fdSet(source, event.target.value);
      fdRenderOptions();
    });
  fdElement("fd-search").addEventListener("input", event => {
    fdLeaderFilter = event.target.value;
    fdRenderLeaders();
  });
  // The map's keys (1 ends a turn, arrows pan) must not fire through the menu,
  // and Escape walks back out one screen at a time.
  door.addEventListener("keydown", event => {
    event.stopPropagation();
    if (event.key !== "Escape") return;
    event.preventDefault();
    if (!fdElement("fd-create").hidden) fdShow("menu");
    else closeFrontDoor();
  });
  // A brand link in the sidebar or the game menu can ask for it back later.
  document.addEventListener("click", event => {
    if (!event.target.closest?.("[data-open-front-door]")) return;
    event.preventDefault();
    openFrontDoor();
  });
}
fdWire();
// `/rules` arrives after this script runs. The option lists it fills are what
// the Create Game screen mirrors, so a first paint waits for it.
if (frontDoorWanted()) {
  document.body.classList.add("frontdoor-open");
  fdElement("frontdoor").hidden = false;
  fdShow("menu");
  fdRefreshContinue();
}
