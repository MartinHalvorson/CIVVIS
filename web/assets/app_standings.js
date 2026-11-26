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
// surplus"). `src/obs.rs` sends them on each player: `avg_loyalty_per_turn`,
// `avg_housing_surplus`, `avg_food_surplus` and `avg_amenity_surplus`
// (happiness is amenities over need, the number Civ VI's moods are cut from).
// LOY is the change in loyalty a turn, signed, not the loyalty itself
// (operator 2026-11-26: "loy should be the avg change in loyalty (often like
// +20 or +5 ot -7). avg of those").
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
  const text = Math.abs(value) < 10 ? value.toFixed(1).replace(/\.0$/, "") : String(Math.round(value));
  return value > 0 ? `+${text}` : text;
}
function averageTitle(name, value, signed = true) {
  const n = averageStat(value);
  return n === null ? `${name}: not shown to this view`
    : `${name}: ${signed && n > 0 ? "+" : ""}${n} per city`;
}
// A civilization by three letters, where a name is a label rather than part
// of a sentence: the standings' CIV column, the victory tracker, the war
// log's names and the game's result graphs (operator 2026-11-26: "maybe use 3
// letter abbreviations for the civs. in player hud and in other places, like
// victory tracker. lets use the 3 letter abbreviation most places"). The full
// name stays in every tooltip and every sentence. Most are their first three
// letters; where two of the 105 civilizations would share them, and where
// the first three read as nothing, the country's own code is used instead.
const CIV_SHORT = {
  America: "USA", Australia: "AUS", Austria: "AUT", Bengal: "BGL", Benin: "BEN", "Gran Colombia": "COL",
  "Great Zimbabwe": "ZIM", Greece: "GRE", India: "IND", Indonesia: "IDN", Netherlands: "NED", Romania: "ROU", Rome: "ROM",
};
function civShort(civ) {
  const name = String(civ ?? ""), titled = name.charAt(0).toUpperCase() + name.slice(1);
  if (CIV_SHORT[titled]) return CIV_SHORT[titled];
  const letters = name.replace(/[^A-Za-z]/g, "");
  return letters.length >= 3 ? letters.slice(0, 3).toUpperCase() : name;
}

// P10 HAP: the tenth percentile of the civ's city happiness (amenities over
// need), and its cell points at the city standing there (operator 2026-11-26:
// "change min hap to p10 hap", after "to the left of hap, add min hap which is
// the least happy city. clicking on a civ's min hap should point at that's civs
// least happy city"). The engine reads the percentile and names the city
// (`tenth_percentile` in src/obs.rs). The cell answers with the `city` action
// the city list's names use (`focusHudCity`): the map goes to the city and the
// spectator's side panel opens on it. A view that is not shown the city, or a
// page that has not got it, keeps the watch action every other figure has.
function p10HappinessCity(player) {
  const id = player.p10_amenity_city;
  return Number.isInteger(id) ? (state?.cities || []).find(city => city.id === id) || null : null;
}
function p10HappinessRow(key, player) {
  const value = averageStat(player.p10_amenity_surplus), city = p10HappinessCity(player);
  return [key, "P10 HAP", value, value === null ? "10th percentile happiness: not shown to this view"
    : `10th percentile happiness: ${value > 0 ? "+" : ""}${value} amenities over need · ${city?.name || "a city"} stands there`];
}
function p10HappinessCell(player, value, label, isLeader) {
  const city = p10HappinessCity(player), civ = escapeAttr(player.civ);
  const [action, verb] = city
    ? [`data-hud-action="city" data-hud-city="${city.id}"`, `Go to ${escapeAttr(city.name || "the city")}`]
    : [`data-hud-action="watch"`, `Watch ${civ}`];
  return `<button class="ribbon-stat p10_happiness${isLeader ? " category-leader" : ""}" data-hud-col="p10_happiness" ` +
    `${action} data-hud-civ="${player.id}" title="${escapeAttr(label)} · ${verb}" ` +
    `aria-label="${escapeAttr(label)}; ${verb.toLowerCase()}"><b>${ribbonAverage("p10_happiness", value)}</b></button>`;
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
  loyalty:"Average change in its cities' loyalty per turn. Below 0, cities slide toward breaking away.",
  population:"Citizens across all its cities.",
  housing:"Average spare housing per city. Near 0 or below, cities grow slowly.",
  food:"Average food left per city after feeding its citizens. More means faster growth.",
  p10_happiness:"Happiness near the bottom of its cities (10th percentile), so one unhappy city does not set it. Click a cell to go to the city there.",
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
