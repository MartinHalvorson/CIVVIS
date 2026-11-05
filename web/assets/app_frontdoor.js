"use strict";
// The front door of the published site.
//
// civvis.ai opens on a live game, and for a long time that was all it did: a
// visitor who came to play landed on four AIs playing each other, and the way
// in was two levels down a sidebar — open Game setup, change "Human players",
// press a button whose label had just changed. This screen is what stands in
// front of that. The AI game keeps running behind it as the attract loop, and
// the choice of what to do is the first thing on the page.
//
// Nothing here builds a game. Every control writes the same setup selects the
// sidebar uses and "Begin game" calls the same `startNewSimulation()`, so the
// two ways in cannot disagree about what a game is.
//
// It is a third classic script for the same reason `app_setup.js` is a second:
// it reads shared top-level bindings (`state`, `RULES`) and helpers from the
// scripts before it, and keeping it apart keeps this work out of the renderer.
const FRONT_DOOR_SEEN_KEY = "civvis-front-door-dismissed-v1";
// The published build is the only page a stranger arrives on. The desktop app
// and the operator's exhibition tabs are started by someone who already chose
// what they are doing, so the shim's presence is the whole condition.
const frontDoorHost = () => !!window.CIVVIS_PUBLISHED_BUILD;
// A deep link already said what it wants, a framed page is someone else's
// layout, and a successor page is mid-handoff. `?menu` asks for it regardless.
function frontDoorWanted() {
  if (!frontDoorHost()) return false;
  const query = new URLSearchParams(location.search);
  if (query.has("menu")) return true;
  if ([...query.keys()].length) return false;
  if (window.top !== window) return false;
  if (window.CIVVIS_RESTART_HANDOFF || window.CIVVIS_ADOPTING_LIVE_WORLD) return false;
  try { return sessionStorage.getItem(FRONT_DOOR_SEEN_KEY) !== "1"; } catch (_) { return true; }
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
  const note = fdCivRules(civ)?.note || "";
  const colon = note.indexOf(":");
  return colon < 0 ? {name: "", text: note}
    : {name: note.slice(0, colon), text: note.slice(colon + 1).trim()};
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
  const focus = panel === "create" ? fdElement("fd-begin") : fdElement("fd-play");
  focus?.focus({preventScroll: true});
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
  let saves = [];
  try { saves = (await fetchJSON("/saves", {}, 4000)).saves || []; } catch (_) { saves = []; }
  // Prefer the autosave: it is the seat they last left. The list itself is
  // ordered by turn, which says nothing about which game was played last.
  const latest = saves.find(save => save.name === "autosave") || saves[0];
  button.hidden = !latest;
  if (!latest) return;
  button.dataset.save = latest.name;
  fdElement("fd-continue-sub").textContent =
    `${latest.civ ? `${latest.civ} · ` : ""}Turn ${latest.turn}` +
    (latest.difficulty ? ` · ${titleCase(latest.difficulty)}` : "");
}
function closeFrontDoor(remember = true) {
  const door = fdElement("frontdoor");
  if (!door) return;
  door.hidden = true;
  document.body.classList.remove("frontdoor-open");
  if (remember) try { sessionStorage.setItem(FRONT_DOOR_SEEN_KEY, "1"); } catch (_) {}
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
function fdBegin() {
  fdSet("humanplayers", "single");
  fdSet("leader", fdChoice);
  closeFrontDoor();
  startNewSimulation();
}
function fdWire() {
  const door = fdElement("frontdoor");
  fdElement("fd-play").onclick = () => { fdRenderCreate(); fdShow("create"); };
  fdElement("fd-watch").onclick = () => closeFrontDoor();
  fdElement("fd-back").onclick = () => fdShow("menu");
  fdElement("fd-begin").onclick = fdBegin;
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
