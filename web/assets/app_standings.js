"use strict";
// The player standings' own figures and headings, carved out of app.js so the
// renderer stays under the size a client script is held to
// (`tools/test_web_assets.py`).
//
// ⚠ THIS SCRIPT IS LOADED BEFORE app.js, for the reason app_palette.js gives
// at length: every script the page loads shares one global scope, and a name
// moved EARLIER can never be read before its declaration has run. Nothing
// here runs app.js code at load: `exactStat` and `PLAYER_HUD_HEAD_LABELS` are
// read only when the standings are drawn or a heading is hovered.
//
// Four of the columns are per-city averages (operator 2026-11-26: "in the
// player hud, to the right of city let's track avg loyalty. to the right of
// pop, lets track avg housing surplus. . to the right of food, lets track avg
// happiness. have all default on", then "lets change food to avg food
// surplus"). `src/obs.rs` sends them on each player: `avg_loyalty`,
// `avg_housing_surplus`, `avg_food_surplus` and `avg_amenity_surplus`
// (happiness is amenities over need, the number Civ VI's moods are cut from).
// FOOD keeps its key, so a saved layout or sort carries over to the new figure.

// Every unit a civilization fields, of any kind. A world that does not say —
// a replay recorded before the count existed, or a mirrored game whose host
// has not sent one — has no figure, which is not the same as having no units.
function playerUnitCount(player) {
  const count = player.unit_count;
  return count === null || count === undefined || !Number.isFinite(+count) ? null : +count;
}

function unitCountTitle(player) {
  const count = playerUnitCount(player);
  if (count === null) return "Units fielded: not reported";
  const military = player.military_unit_count;
  return `Units fielded: ${exactStat(count)}` +
    (military === null || military === undefined ? "" : ` · ${exactStat(military)} military`);
}

// The per-city averages (LOY, HOUS, FOOD, HAP) keep one decimal below ten,
// where a whole number would flatten +0.4 and +1.4 alike, and a surplus says
// its sign. A figure this view is not shown, a rival's loyalty under fog or a
// mirrored game's, is a dash: it is not a zero.
const PLAYER_HUD_AVERAGES = new Set(["loyalty", "housing", "food", "happiness"]);
function averageStat(value) {
  return value === null || value === undefined || !Number.isFinite(+value) ? null : +value;
}
function ribbonAverage(kind, value) {
  if (value === null) return "—";
  if (kind === "loyalty") return String(Math.round(value));
  const text = Math.abs(value) < 10 ? value.toFixed(1).replace(/\.0$/, "") : String(Math.round(value));
  return value > 0 ? `+${text}` : text;
}
function averageTitle(name, value, signed = true) {
  const n = averageStat(value);
  return n === null ? `${name}: not shown to this view`
    : `${name}: ${signed && n > 0 ? "+" : ""}${n} per city`;
}
// MIN HAP: amenities over need in the civ's least happy city, and its cell
// points at that city (operator 2026-11-26: "to the left of hap, add min hap
// which is the least happy city. clicking on a civ's min hap should point at
// that's civs least happy city"). It answers with the `city` action the city
// list's names use (`focusHudCity`): the map goes to the city and the
// spectator's side panel opens on it. A view that is not shown the city, or a
// page that has not got it, keeps the watch action every other figure has.
function leastHappyCity(player) {
  const id = player.min_amenity_city;
  return Number.isInteger(id) ? (state?.cities || []).find(city => city.id === id) || null : null;
}
function leastHappyRow(key, player) {
  const value = averageStat(player.min_amenity_surplus), city = leastHappyCity(player);
  return [key, "MIN HAP", value, value === null ? "Least happy city: not shown to this view"
    : `Least happy city: ${city?.name || "a city"}, ${value > 0 ? "+" : ""}${value} amenities over need`];
}
function leastHappyCell(player, value, label, isLeader) {
  const city = leastHappyCity(player), civ = escapeAttr(player.civ);
  const [action, verb] = city
    ? [`data-hud-action="city" data-hud-city="${city.id}"`, `Go to ${escapeAttr(city.name || "the city")}`]
    : [`data-hud-action="watch"`, `Watch ${civ}`];
  return `<button class="ribbon-stat min_happiness${isLeader ? " category-leader" : ""}" data-hud-col="min_happiness" ` +
    `${action} data-hud-civ="${player.id}" title="${escapeAttr(label)} · ${verb}" ` +
    `aria-label="${escapeAttr(label)}; ${verb.toLowerCase()}"><b>${ribbonAverage("min_happiness", value)}</b></button>`;
}

// One standings row for an average: [key, heading, figure, tooltip].
function averageRow(key, label, value, name, signed = true, extra = "") {
  return [key, label, averageStat(value), averageTitle(name, value, signed) + extra];
}

// What each figure means, in a sentence, shown in a box under its heading
// while the pointer rests there or the heading has focus (operator
// 2026-11-26: "we might need to have expand on hover explanations. keep the
// explanations short and simple though").
const PLAYER_HUD_HEAD_EXPLAIN = {
  cities:"How many cities this civ holds.",
  loyalty:"Average loyalty of its cities, 0 to 100. A city at 0 breaks away.",
  population:"Citizens across all its cities.",
  housing:"Average spare housing per city. Near 0 or below, cities grow slowly.",
  food:"Average food left per city after feeding its citizens. More means faster growth.",
  min_happiness:"Amenities beyond need in its least happy city. Click a cell to go to that city.",
  happiness:"Average amenities beyond what each city needs. 3 or more is happy; below 0 hurts.",
  production:"Production per turn, all cities.", science:"Science per turn.",
  culture:"Culture per turn.", tourism:"Tourism per turn: its culture's pull on other civs.",
  faith:"Faith per turn. * means it founded a religion.", gold:"Gold per turn after costs.",
  units:"Units it fields, of every kind.", military:"Strength of its fighting units.",
  nukes:"Nuclear weapons it holds.", suzerain:"City-states that follow it as suzerain.",
  wonders:"World wonders it holds.", score:"Game score.",
};
let hudExplainTimer = 0;
function hudExplainBox() {
  let box = document.getElementById("hudexplain");
  if (!box) {
    box = Object.assign(document.createElement("div"), {id:"hudexplain", hidden:true});
    box.setAttribute("role", "tooltip");
    document.body.append(box);
  }
  return box;
}
function hideHudExplain() {
  clearTimeout(hudExplainTimer);
  const box = document.getElementById("hudexplain");
  if (box) box.hidden = true;
}
function showHudExplain(head) {
  const key = head.dataset.hudCol, text = PLAYER_HUD_HEAD_EXPLAIN[key];
  if (!text || !head.isConnected) return hideHudExplain();
  const box = hudExplainBox();
  box.innerHTML = `<b>${PLAYER_HUD_HEAD_LABELS[key][1]}</b>${text}<small>Click to sort · drag to move</small>`;
  box.hidden = false;
  const at = head.getBoundingClientRect(), width = box.offsetWidth;
  box.style.left = `${Math.max(6, Math.min(at.left, innerWidth - width - 6))}px`;
  box.style.top = `${at.bottom + 4}px`;
}
const hudExplainHead = event => event.target?.closest?.("#playerhud .hud-head-stat[data-hud-col]");
document.addEventListener("pointerover", event => {
  const head = hudExplainHead(event);
  if (!head || head.contains(event.relatedTarget)) return;
  clearTimeout(hudExplainTimer);
  hudExplainTimer = setTimeout(() => showHudExplain(head), 120);
});
document.addEventListener("pointerout", event => {
  const head = hudExplainHead(event);
  if (head && !head.contains(event.relatedTarget)) hideHudExplain();
});
document.addEventListener("focusin", event => {
  const head = hudExplainHead(event);
  if (head) showHudExplain(head); else hideHudExplain();
});
document.addEventListener("focusout", event => { if (hudExplainHead(event)) hideHudExplain(); });
document.addEventListener("pointerdown", hideHudExplain, true);
addEventListener("scroll", hideHudExplain, true);
