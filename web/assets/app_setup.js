"use strict";
// The setup controller is deliberately a second classic script.  It is loaded
// after app.js so its first pass can resolve the renderer's shared helpers, but
// before any user event or /rules response can reach the controls. Keeping this
// boundary lets setup work land without reopening the 32k-line map renderer.
// Mode state, declared here rather than beside the functions that use it: the
// wiring immediately below calls syncSetupMode() during load, and a `let` read
// before its own statement has run is a ReferenceError rather than an
// undefined.
let civ6Status = null;
let civ6StatusTimer = null;
let civ6StatusInFlight = false;
let civvisMapOptions = null;
let civvisSizeOptions = null;
let civVictoryChoices = null;
let victoryRoster = "civvis";
// The Tactics size last chosen for each map, so moving between maps and back
// returns to the size that was picked rather than to the smallest.
const tacticsSizeChoices = {};
document.getElementById("humanplayers").addEventListener("change", syncSetupMode);
document.getElementById("gamemode").addEventListener("change", syncSetupMode);
document.getElementById("leaderpool").addEventListener("change", syncLeaderPool);
document.getElementById("leaderselection").addEventListener("change", syncCustomLeaderSelection);
document.getElementById("teams").addEventListener("change", syncCustomLeaderSelection);
document.getElementById("custom-leader-table").addEventListener("change", event => {
  if (!event.target.matches("[data-custom-civ]")) return;
  const row = event.target.closest("tr");
  const selected = event.target.selectedOptions[0];
  const elo = row?.querySelector("[data-custom-elo]");
  if (elo && selected)
    elo.innerHTML = customEloOptions(event.target.value, selected.dataset.leader || "", "");
});
// Each Tactics world question re-fits the ones below it, on the select
// itself so the work is done before `#newgame-options`'s delegated listener
// stages what is now selected. The battle decides whether the world type and
// map are asked at all, and brings its own era and clock; the world type
// decides which maps are offered; the map decides which sizes are.
document.getElementById("maptype").addEventListener("change", () => {
  syncEarthShape();
  if (tacticsMode()) syncBattlefieldSizes(true);
});
document.getElementById("tacticsworldtype").addEventListener("change", () => {
  syncMapRoster(false, true);
  syncBattlefieldSizes(true);
});
document.getElementById("tactics-scenario").addEventListener("change", () => {
  const scenario = historicalScenario(tacticsScenarioId());
  syncBattlefieldSizes(true);
  if (scenario) scenarioTurnChoice(scenario.turns);
  syncScenarioChoice();
});
// Customize is the one era choice with configuration of its own, so choosing
// it is what unfolds the pool — on the select itself, ahead of the panel's
// delegated staging listener, like every other Tactics control that reshapes
// the form.
document.getElementById("tacticsera").addEventListener("change", () => {
  syncTacticsEraPool();
  const pool = document.getElementById("tactics-era-pool");
  if (pool && !pool.hidden) pool.scrollIntoView({block: "nearest"});
});
// Bound on the select itself, so it has re-fitted the splits to the new size
// before the panel's own delegated listener stages what is now selected.
document.getElementById("np").addEventListener("change", () => {
  // In Tactics the pick is remembered for the map it was made on, so moving
  // to another map and back returns to it.
  if (tacticsMode()) tacticsSizeChoices[tacticsMapScript()] = readSetting("np");
  syncTeams();
  syncCustomLeaderSelection();
});
document.getElementById("savebtn").onclick = () => writeSave();
syncSetupMode();
syncEarthShape();
document.getElementById("diplomacybtn").onclick = () => openDiplomacy();
document.getElementById("paneltoggle").onclick = () => togglePanel();
bindMapAreaEditor();
document.getElementById("collapsepause").onclick = toggleSpecPause;
document.getElementById("specpause").onclick = toggleSpecPause;
function chooseWatchPace(value) {
  const select = document.getElementById("specspeed");
  const ms = Number(value);
  if (!select || !Number.isFinite(ms) || !paceOffered(select, ms)) return;
  select.value = String(ms);
  rememberPace(ms); // the pick outranks any state already in flight
  setPace({ms: paceChoice});
  if (state) drawPlayerHud();
}
// The arena's fog rule, flipped for the battle on screen. The lobby's own
// control follows the flip so a client-issued restart carries the rule the
// battle was just fought under; the server moves its own params in the same
// request, so its automatic successor carries it too.
function chooseTacticsFog(on) {
  const select = document.getElementById("tacticsfog");
  if (select) select.value = on ? "1" : "0";
  setPace({tactics_fog: on});
}
document.getElementById("specspeed").onchange = event => chooseWatchPace(event.currentTarget.value);
restorePace();
document.getElementById("between-game-countdown").onchange = () =>
  chooseBetweenGameCountdown(betweenGameCountdownMs());
// The Tactics card's post-match control is the same knob offered where a
// match is set up, so it goes through the same chooser — which pushes the
// choice to the server and re-syncs every copy, this one included.
document.getElementById("tacticspostmatch").onchange = event =>
  chooseBetweenGameCountdown(Number(event.currentTarget.value));
// The result screen offers the same choice where a viewer actually meets it,
// as a copy of the Display Settings control; the copy is re-rendered with the
// screen, so the listener lives on the permanent overlay.
document.getElementById("winner").addEventListener("change", event => {
  const select = event.target.closest?.("#finale-countdown");
  if (select) chooseBetweenGameCountdown(Number(select.value));
});
restoreBetweenGameCountdown();
document.getElementById("renderresolution").onchange = () => {
  rememberRenderResolution(document.getElementById("renderresolution").value);
  refreshRenderResolution();
};
document.getElementById("performancepreset").onchange = () => {
  const preset = document.getElementById("performancepreset").value;
  rememberRenderResolution(PERFORMANCE_PRESET_RESOLUTION[preset] || "native");
  refreshRenderResolution();
};
restoreRenderResolution();
async function spectatePlayer(player) {
  if (!state || viewChanging) return;
  viewChanging = true;
  syncViewPlayer();
  clearTimeout(specTimer);
  try {
    render(await fetchJSON("/view", { method: "POST",
      headers: {"Content-Type": "application/json"},
      body: JSON.stringify({player}) }), false);
  } catch (e) {
    const errEl = document.getElementById("err");
    errEl.textContent = `Could not change view: ${e.message}`;
    errEl.style.display = "block";
    setTimeout(() => errEl.style.display = "none", 2500);
  } finally {
    viewChanging = false;
    syncViewPlayer();
    scheduleSpec(state);
  }
}
document.getElementById("viewplayer").onchange = ev => {
  const select = ev.currentTarget;
  const player = select.value === "spectator" ? null : +select.value;
  spectatePlayer(player);
};
let newSimulationBusy = false;
let activeSimulationSettingsKey = null;
let settingsStageChain = Promise.resolve();
function readSetting(id) {
  const select = document.getElementById(id);
  return select ? select.value : "";
}
// The Mercy Rule select stores its threshold as the option's text number, so
// a value arriving as 0.9 must still find the "0.90" option: match on the
// number, not the string.
function setMercySelect(select, value) {
  if (!select) return;
  const target = value == null ? null : Number(value);
  const match = [...select.options].find(option =>
    (option.value === "" ? null : Number(option.value)) === target);
  if (match) select.value = match.value;
}
// Require-N can never exceed the victory conditions actually enabled: cap the
// choices live as the checkboxes change, and pull an over-cap selection down.
function syncRequiredVictoriesCap() {
  const select = document.getElementById("requiredvictories");
  if (!select) return;
  const enabled = VICTORY_TRACKS.filter(track =>
    readVictorySetting(`victory-${track.id}`)).length || 1;
  for (const option of select.options)
    option.disabled = Number(option.value) > enabled;
  if (Number(select.value) > enabled) select.value = String(enabled);
}
function readVictorySetting(id) {
  const box = document.getElementById(id);
  return box ? box.checked : true;
}
// A blank numeric field means "use the normal setting for this map or speed".
// Number inputs still accept pasted text, so keep the payload on whole safe
// integers and make a bad value visible on the start control instead of
// silently replacing it with a different world.
function readOptionalWholeSetting(id) {
  const input = document.getElementById(id);
  if (!input || !input.value.trim()) return null;
  const value = input.valueAsNumber;
  return input.validity.valid && Number.isSafeInteger(value) ? value : null;
}
function worldSetupInputError() {
  for (const [id, label] of [
    ["mapseed", "Map seed"],
  ]) {
    const input = document.getElementById(id);
    if (input && input.value.trim() && readOptionalWholeSetting(id) === null)
      return `${label} must be a whole number in the range shown`;
  }
  // A customized era pool with nothing in it names no army at all; the
  // server would refuse it, so the start control says why first.
  if (tacticsMode() && readSetting("tacticsera") === "custom" && !tacticsEraPool().length)
    return "The era pool needs at least one era checked";
  return "";
}
// The eras the Customize pool has checked, earliest first. The ladder is
// written inside the function for the same load-order reason
// `syncScenarioSettings` documents: a top-level const would still be in its
// temporal dead zone when the load path first reads settings.
function tacticsEraPool() {
  const ladder = ["ancient", "classical", "medieval", "renaissance",
                  "industrial", "modern", "atomic", "information"];
  return ladder.filter(era => document.getElementById(`erapool-${era}`)?.checked);
}
// Customize's configuration appears exactly while Customize is the era
// choice — choosing it in the box is what "opens" the pool, and any other
// choice folds it away rather than leaving a dead checklist under a decided
// control.
function syncTacticsEraPool() {
  const pool = document.getElementById("tactics-era-pool");
  if (pool) pool.hidden = readSetting("tacticsera") !== "custom";
}
function setOptionalWorldNumber(id, value, defaultValue = Number.NaN) {
  const input = document.getElementById(id);
  const number = Number(value);
  const min = Number(input?.min || 0);
  const max = Number(input?.max || Number.MAX_SAFE_INTEGER);
  const automatic = Number.isSafeInteger(defaultValue) && number === defaultValue;
  if (input) input.value = Number.isSafeInteger(number) && number >= min && number <= max && !automatic
    ? String(number) : "";
}
// What this computer can do about the other game, and what a run it can see is
// doing. The verification-only mode is retained everywhere and refused where it
// cannot run, so the refusal needs somewhere to be read: a run that silently
// never starts is how a dead Steam client cost eleven ladder attempts, every one
// of them recorded as a loss rather than an attempt that never began.
async function refreshCiv6Status() {
  if (civ6StatusInFlight) return;
  civ6StatusInFlight = true;
  try {
    civ6Status = await fetchJSON("/civ6");
  } catch (error) {
    civ6Status = {ready: false, blocked: `this server did not answer (${error.message || error})`};
  } finally {
    civ6StatusInFlight = false;
  }
  drawCiv6Status();
  updateRestartSimulationButton();
  // A run takes minutes to reach its first turn and hours to finish, so this
  // is a slow poll, and it only runs while somebody is looking at the mode.
  clearTimeout(civ6StatusTimer);
  if (document.body.classList.contains("playing-civ6"))
    civ6StatusTimer = setTimeout(refreshCiv6Status, 10000);
}
function civ6RunLine(run) {
  if (!run) return "";
  const who = [run.leader, run.civ]
    .map(name => (name || "").replace(/^(LEADER|CIVILIZATION)_/, "").toLowerCase())
    .filter(Boolean)
    .map(titleCase);
  // Joined after the names are cased, because titleCase upper-cases every word
  // it is given and "Wu Zetian Of China" is not how anybody writes that.
  const seat = who.length ? who.join(" of ") : "a seat";
  const rung = (run.difficulty || "").replace(/^DIFFICULTY_/, "").toLowerCase();
  const turn = run.max_turns ? `turn ${run.turn} of ${run.max_turns}` : `turn ${run.turn}`;
  const empire = `${run.cities} ${run.cities === 1 ? "city" : "cities"}`;
  const verb = run.live ? "playing" : "played";
  const ended = run.live ? ""
    : run.won === true ? " · won"
    : run.reason ? ` · ${run.reason}` : "";
  return `<b>${escapeAttr(seat)}</b> ${verb} ${rung ? rung + ", " : ""}${turn} · ${empire}${ended}`;
}
function drawCiv6Status() {
  const box = document.getElementById("civ6-status");
  const host = document.getElementById("civ6-host");
  const runLine = document.getElementById("civ6-run");
  if (!box || !host || !runLine) return;
  const status = civ6Status;
  const playing = !!(status && status.run && status.run.live);
  box.classList.toggle("ready", !!(status && status.ready) && !playing);
  box.classList.toggle("blocked", !!(status && status.blocked) && !playing);
  box.classList.toggle("playing", playing);
  host.textContent = !status ? "Checking this computer…"
    : playing ? "A game is being played on this computer"
    : status.blocked ? `Cannot start: ${status.blocked}`
    : "Civilization VI is installed here and free";
  runLine.innerHTML = civ6RunLine(status && status.run);
  runLine.hidden = !runLine.innerHTML;
}
function selectedSimulationSettings() {
  const humanPlayers = readSetting("humanplayers");
  // In Tactics the size control holds a map-size id rather than a seat count:
  // every Tactics map seats two sides, and its dimensions travel
  // explicitly because no seat count implies them. The map is the battle
  // chosen, or the custom map under it; the shape is the size's own.
  const tactics = tacticsMode();
  const battlefield = tactics ? battlefieldSize(readSetting("np")) : null;
  const np = battlefield ? 2 : +readSetting("np");
  const mapScript = tactics ? tacticsMapScript() : readSetting("maptype");
  const mapTopology = battlefield ? (battlefield.topology || "flat") : readSetting("mapshape");
  const mapPoles = readSetting("mappoles");
  const gameSpeed = readSetting("gamespeed");
  const leaderPool = readSetting("leaderpool");
  const baseRuleset = readSetting("baseruleset");
  const startEra = readSetting("startera");
  const futureEra = readSetting("futureera");
  const victoryConditions = Object.fromEntries(VICTORY_TRACKS.map(track =>
    [track.id, readVictorySetting(`victory-${track.id}`)]));
  const mercyRule = readSetting("mercyrule");
  const requiredVictories = Number(readSetting("requiredvictories")) || 1;
  const leader = readSetting("leader");
  const difficulty = readSetting("difficulty");
  const leaderSelection = readSetting("leaderselection") || "automatic";
  const customLeaders = leaderSelection === "custom" ? customLeaderRowsFromDom() : [];
  const customCivs = customLeaders.map(row => row.civ).filter(Boolean);
  const teams = leaderSelection === "custom" && customLeaders.length === np
    ? customLeaders.map(row => row.team === "" ? null : Number(row.team))
    : teamAssignment(np, readSetting("teams"));
  const mapSeed = readOptionalWholeSetting("mapseed");
  // In the Civilization VI mode the map control holds one of that game's
  // scripts, so it travels as `civ6_map` and `map_script` keeps a world this
  // engine could still build — the payload has to stay a valid description of a
  // CIVVIS world for the settings staging and the summary line, which are the
  // same code in every mode.
  const civ6 = humanPlayers === "civ6";
  const civ6Map = civ6 ? mapScript : "";
  const ourMap = civ6
    ? (civ6Maps().find(map => map.id === civ6Map) || {}).civvis || "continents"
    : mapScript;
  return {num_players: np, map_script: ourMap, map_topology: mapTopology,
          map_poles: mapPoles, game_speed: gameSpeed,
          leader_pool: leaderPool,
          // Which of the rated strategies may play the AI civilizations —
          // read in both game modes, because a Tactics arena seats its two
          // sides from the same pool a world does.
          ai_player_pool: readSetting("aiplayerpool") || "best3",
          leader_selection: leaderSelection,
          base_ruleset: baseRuleset, start_era: startEra,
          future_era: futureEra,
          // Automatic mode carries the split rule. Custom mode carries the
          // table's seat-by-seat team assignment and civilization choices.
          teams,
          ...(leaderSelection === "custom" ? {custom_leaders: customLeaders} : {}),
          victory_conditions: victoryConditions,
          mercy_rule: mercyRule === "" ? null : Number(mercyRule),
          required_victory_types: requiredVictories,
          // The arena's settings travel always, not only in Tactics: the
          // server reads them on a battlefield and carries them everywhere
          // else, so a world is unaffected and a mode switch needs no second
          // request.
          tactics_fog: readSetting("tacticsfog") === "1",
          tactics_flag: readSetting("tacticsflag") === "1",
          tactics_turn_limit: Number(readSetting("tacticsturnlimit")) || 250,
          tactics_cities: Number(readSetting("tacticscities")) || 0,
          tactics_production: Number(readSetting("tacticsproduction")) || 0,
          tactics_gold: Number(readSetting("tacticsgold")) || 0,
          tactics_turns_per_tech: Number(readSetting("tacticsturnspertech")) || 0,
          tactics_best_of: Number(readSetting("tacticsbestof")) || 1,
          tactics_unique_units: readSetting("tacticsuniqueunits") === "1",
          tactics_era: readSetting("tacticsera") || "random",
          tactics_eras: tacticsEraPool(),
          ...(mapSeed === null ? {} : {seed: mapSeed}),
          // A battlefield's dimensions are its own setting: no seat count
          // implies an arena size, so the chosen one travels explicitly and
          // the server derives everything else from the battlefield map.
          ...(battlefield ? {width: battlefield.width, height: battlefield.height} : {}),
          spectate: humanPlayers === "ai_sim",
          ...(civ6 ? {mode: "civ6", civ6_map: civ6Map} : {}),
          // A spectated world has nobody to hand a leader or a handicap to.
          // Neither has this one, but its difficulty is the point of it: the
          // other game hands its handicap bonuses to human seats only, which is
          // what makes the ladder climbable from one.
          ...(humanPlayers === "ai_sim"
            ? (leaderSelection === "custom" ? {civs: customCivs} : {})
            : {civs: civ6 || !leader ? [] : [leader], difficulty})};
}
// The handoff screen describes the world being built, so it reports the values
// the payload actually carries rather than re-reading the panel behind it.
function selectedSimulationSummary(settings = null) {
  const chosenText = (id, value) => {
    const select = document.getElementById(id);
    if (!select) return "";
    const option = value === undefined || value === null
      ? select.selectedOptions?.[0]
      : [...select.options].find(entry => entry.value === String(value));
    return (option?.textContent || (value ?? select.value) || "").toString().trim();
  };
  const spectate = settings ? settings.spectate
    : document.getElementById("humanplayers").value === "ai_sim";
  const civ6 = settings ? settings.mode === "civ6"
    : document.getElementById("humanplayers").value === "civ6";
  const tactics = settings ? isBattlefieldMapScript(settings.map_script)
    : readSetting("gamemode") === "tactics";
  const mode = civ6 ? "Firaxis Civ 6"
    : spectate === true ? "AI simulation" : "Single player";
  const arena = !civ6 && tactics ? "Tactics" : "";
  // The world-size option already includes its dimensions and seat count; the
  // compact first clause is enough to identify the choice without turning the
  // handoff screen into a settings table.
  // A named battle is not on the map control — it is chosen one control up
  // — so it is named from the catalog, and its one chart needs no size
  // clause beside it; any other map is named from the control. A Tactics
  // payload carries no seat count that names a size, so its size is found
  // from the dimensions it carries instead.
  const script = settings ? settings.map_script : tactics ? tacticsMapScript() : readSetting("maptype");
  const battle = tactics ? historicalScenario(script) : null;
  const ground = tactics && settings ? battlefieldSizesForScript(script)
    .find(entry => entry.width === settings.width && entry.height === settings.height) : null;
  const size = battle ? "" : ground ? ground.name.split(" · ")[0]
    : chosenText("np", settings?.num_players).split(" · ")[0];
  const map = battle ? battle.name : chosenText("maptype", script).split(" · ")[0];
  const speed = chosenText("gamespeed", settings?.game_speed).split(" · ")[0];
  // The era is named only when it is not the stock start: every other setting
  // on this line is one somebody chose, and "Ancient" is what you get by
  // choosing nothing. It is compared by value rather than by where it sits in
  // the list — the stock start is the first rung that can actually be picked,
  // not the first one listed, because the ladder opens on an age nobody has
  // built.
  const eras = document.getElementById("startera");
  const startEra = settings ? settings.start_era : eras?.value;
  const stockEra = [...(eras?.options || [])]
    .find(option => option.value && !option.disabled)?.value;
  // A Tactics world's era is the arena's own control, whose stock choice is
  // Random; a named battle needs no clause because the battle's name already
  // says more than its era would.
  const tacticsEra = settings ? settings.tactics_era : readSetting("tacticsera");
  const era = tactics
    ? (battle || !tacticsEra || tacticsEra === "random" ? ""
       : tacticsEra === "custom" ? "Cross-era"
       : chosenText("tacticsera", tacticsEra).split(" · ")[0])
    : startEra && startEra !== stockEra ? chosenText("startera", startEra) : "";
  const futures = document.getElementById("futureera");
  const futureEra = settings ? settings.future_era : futures?.value;
  const stockFuture = [...(futures?.options || [])]
    .find(option => option.value && !option.disabled)?.value;
  const future = futureEra && futureEra !== stockFuture
    ? chosenText("futureera", futureEra) : "";
  // Free-for-all is what you get by choosing nothing, so only a real division
  // is worth a clause — and it is named by its split, which is the part that
  // depends on a size this line has already reported.
  const players = settings ? settings.num_players
    : +document.getElementById("np").value || 2;
  const teamRule = settings
    ? teamRuleFromArray(settings.teams)
    : document.getElementById("teams")?.value;
  const teams = teamRule && teamRule !== "ffa" ? teamPhrase(players, teamRule) : "";
  return [mode, arena, size, map, speed, era, future, teams].filter(Boolean).join(" · ");
}
function simulationSettingsKey(settings) {
  return JSON.stringify(settings);
}
function applyQueuedSimulationSettings(settings) {
  if (!settings) return;
  // Staged settings describe a spectated world, so they adopt that mode while
  // one is on screen. They must not overrule the person who just asked for a
  // single-player game and is now looking at it.
  if (SPEC) document.getElementById("humanplayers").value = "ai_sim";
  // The staged map says which game mode the queue describes: a battlefield
  // is the Tactics mode's arena, read back as the battle or the custom world
  // type and map, with the size control holding the Tactics size whose
  // dimensions the queue carries.
  const tactics = isBattlefieldMapScript(settings.map);
  if (tactics) {
    adoptTacticsWorld(settings.map, settings.width, settings.height);
  } else {
    document.getElementById("gamemode").value = "civ";
    syncSetupMode();
    document.getElementById("np").value = String(settings.players);
  }
  // The staged size decides which splits exist, so it is in place first and
  // the queued assignment is read back as the rule that would produce it.
  syncTeams();
  document.getElementById("teams").value = teamRuleFromArray(settings.teams);
  if (!tactics) {
    document.getElementById("maptype").value = settings.map;
    if (settings.shape) document.getElementById("mapshape").value = settings.shape;
  }
  if (settings.poles) document.getElementById("mappoles").value = settings.poles;
  syncEarthShape();
  document.getElementById("gamespeed").value = settings.speed;
  setOptionalWorldNumber("mapseed", settings.seed);
  document.getElementById("leaderpool").value = normalizedLeaderPoolId(settings.leader_pool || "civ6");
  if (settings.ai_pool)
    document.getElementById("aiplayerpool").value = settings.ai_pool;
  if (settings.leader_selection)
    document.getElementById("leaderselection").value = settings.leader_selection;
  syncLeaderPool();
  syncCustomLeaderSelection();
  document.getElementById("baseruleset").value = settings.base_ruleset || "civ6";
  if (settings.start_era) document.getElementById("startera").value = settings.start_era;
  if (settings.future_era) document.getElementById("futureera").value = settings.future_era;
  const victories = new Set(settings.victories || []);
  for (const track of VICTORY_TRACKS)
    document.getElementById(`victory-${track.id}`).checked = victories.has(track.id);
}
function updateRestartSimulationButton() {
  const button = document.getElementById("restart-sim");
  const settings = selectedSimulationSettings();
  const setupError = worldSetupInputError();
  // Picking single player turns the specbar's one start control into the way
  // into that game: it becomes Start new game, and always reads as a change
  // from the world on screen.
  const human = settings.spectate !== true;
  // The control is named after the game it starts, and the other game's name
  // is the whole point of choosing it: nothing else on the screen says that
  // pressing this takes over a real Civilization VI window for hours.
  const civ6 = settings.mode === "civ6";
  const blocked = civ6 && !!(civ6Status && !civ6Status.ready);
  const changed = human || (activeSimulationSettingsKey !== null &&
    simulationSettingsKey(settings) !== activeSimulationSettingsKey);
  button.classList.toggle("primary", changed && !blocked);
  button.classList.toggle("human-start", human);
  // A refusal is shown rather than hidden: the button stays, says so, and the
  // reason sits above it.
  button.disabled = blocked || !!setupError;
  button.querySelector(".lbl").textContent = civ6
    ? "Play Firaxis Civ 6"
    : human
    ? "Start new game"
    : "Restart sim";
  button.title = setupError || (civ6
    ? blocked
      ? `Cannot start: ${civ6Status.blocked}`
      : "Start a real Civilization VI game on this computer and play it"
    : human
    ? "Start a single-player game with the selected settings"
    : changed
    ? "Restart with the selected settings"
    : "Restart with the same settings");
  button.querySelector(".sub").textContent = civ6
    ? "on this computer"
    : human || changed
    ? "with selected settings"
    : "same settings";
}
function stageSelectedSimulationSettings() {
  updateRestartSimulationButton();
  if (worldSetupInputError()) return;
  const payload = selectedSimulationSettings();
  settingsStageChain = settingsStageChain.catch(() => {}).then(async () => {
    const queued = await fetchJSON("/next-game-settings", {method: "POST",
      headers: {"Content-Type": "application/json"},
      body: JSON.stringify(payload)});
    if (queued.error) throw new Error(queued.error);
  });
  settingsStageChain.catch(error => showNewSimulationError(error, "save settings"));
}
function newSimulationPayload() {
  const setupError = worldSetupInputError();
  if (setupError) throw new Error(setupError);
  const settings = selectedSimulationSettings();
  return {...settings, seed: settings.seed ?? Math.floor(Math.random() * 1e9), force: true};
}
const simulationBusyControls = new Map();
function setNewSimulationBusy(busy) {
  newSimulationBusy = busy;
  const controls = [
    ...document.querySelectorAll("#newgame-options select, #newgame-options input, #newgame-options button"),
    document.getElementById("restart-sim"), document.getElementById("specpause"),
    document.getElementById("collapsepause"),
  ].filter(Boolean);
  if (busy) {
    for (const control of controls) {
      if (!simulationBusyControls.has(control))
        simulationBusyControls.set(control, control.disabled);
      control.disabled = true;
    }
  } else {
    for (const [control, disabled] of simulationBusyControls) control.disabled = disabled;
    simulationBusyControls.clear();
    syncEarthShape();
  }
}
function showNewSimulationError(error, action = "start simulation") {
  const errEl = document.getElementById("err");
  errEl.textContent = `Could not ${action}: ${error.message || error}`;
  errEl.style.display = "block";
  setTimeout(() => errEl.style.display = "none", 4000);
}
// The mode select says what the *next* game will be, and it opens on the
// AI-only simulation. Once a world is on screen it opens on that world's mode
// instead, so the persistent restart control describes what it will start.
function adoptRunningMode() {
  const select = document.getElementById("humanplayers");
  if (!select || !state) return;
  select.value = SPEC ? "ai_sim" : "single";
  syncSetupMode();
  updateRestartSimulationButton();
}
// Leader and difficulty are meaningless in a spectated world: nobody is at
// the keyboard. The restart control remains above the settings in either mode
// and transforms to describe the selected next game.
//
// The Civilization VI mode is a third state rather than a variation on either:
// a difficulty is chosen and it is real (that game hands its handicaps to human
// seats, which is the whole reason to occupy one), and nobody is at the
// keyboard once it starts. So the settings follow single player and the
// controls follow the simulation. See docs/CIV6_GAME_MODE.md.
//
// Tactics is a game-mode axis rather than a fourth keyboard state: who plays
// is still the Human players control, and what they play — the full Civ game
// or a battlefield arena — swaps the size and map rosters the way Civ 6
// swaps the map list. Civ 6 outranks it while chosen, because that mode's
// panel configures the other game entirely.
function syncSetupMode() {
  const seats = (document.getElementById("humanplayers") || {}).value || "ai_sim";
  const civ6 = seats === "civ6";
  const tactics = !civ6 && readSetting("gamemode") === "tactics";
  document.body.classList.toggle("spectating", seats === "ai_sim");
  document.body.classList.toggle("playing-civ6", civ6);
  document.body.classList.toggle("playing-tactics", tactics);
  // The pass is asked in the order the mode asks it, and each Tactics
  // question is re-fitted from the one above it: the battle, then the maps
  // its world type offers, then the sizes its map is drawn at.
  placeSetupControls(tactics);
  syncScenarioMenu();
  syncMapRoster(civ6, tactics);
  syncBattlefieldSizes(tactics);
  // Again here rather than only inside the call above, which has several
  // early returns: leaving the arena card greyed out after a switch back to
  // a rolled map would be worse than never having greyed it.
  syncScenarioChoice();
  syncBattlefieldVictories(tactics);
  // The control's label is not relabelled from here. This runs once during
  // load, before the settings state it would read has been initialised, and
  // `#newgame-options`'s delegated change listener already relabels it on
  // every real selection.
  if (civ6) refreshCiv6Status();
}
// In the Civilization VI mode the map control is that game's own list, not
// ours. The two rosters overlap without either containing the other — Grand
// Canals is a CIVVIS world with no counterpart there, Shuffle and Tilted Axis
// are its worlds with no counterpart here — so the list is replaced rather
// than filtered, and a choice that exists in both survives the switch.
function civ6Maps() {
  return (RULES && RULES.civ6 && Array.isArray(RULES.civ6.maps)) ? RULES.civ6.maps : [];
}
function battlefieldScripts() {
  return (RULES && Array.isArray(RULES.battlefield_scripts)) ? RULES.battlefield_scripts : [];
}
function battlefieldSizes() {
  return (RULES && Array.isArray(RULES.battlefield_sizes)) ? RULES.battlefield_sizes : [];
}
// Which Tactics maps are scripted battles. A scenario is fought under the
// economy its battle had rather than the one the arena card offers, so the
// controls the server is about to overrule are shown as fixed instead of
// live. The clock and the series length are still the player's: neither is a
// claim about the battle, only about how long they want to play.
function scenarioScripts() {
  return (RULES && Array.isArray(RULES.scenario_scripts)) ? RULES.scenario_scripts : [];
}
function historicalScenarios() {
  return (RULES && Array.isArray(RULES.historical_scenarios)) ? RULES.historical_scenarios : [];
}
function historicalScenario(id) {
  return historicalScenarios().find(scenario => scenario.id === id) || null;
}
function scenarioTerrainName(terrain) {
  return ({
    land: "Land",
    land_water: "Land & Water",
    water: "Water",
    water_air: "Water & Air",
    land_air: "Land & Air",
    land_water_air: "Land/Water/Air",
  })[terrain] || titleCase(terrain);
}
function scenarioEraNames() {
  return ["Ancient", "Classical", "Medieval", "Renaissance", "Industrial", "Modern", "Atomic", "Information", "Future"];
}
// Whether the setup panel is describing a Tactics game: the mode is chosen,
// and the Civilization VI mode — which configures the other game entirely —
// has not outranked it.
function tacticsMode() {
  return readSetting("humanplayers") !== "civ6" && readSetting("gamemode") === "tactics";
}
// The battle chosen in the Scenario control, or "" for Custom. The control is
// the one source of that answer, so nothing else has to be kept in step with
// it; it is only meaningful while Tactics is the mode.
function tacticsScenarioId() {
  return readSetting("tactics-scenario");
}
// The world type a custom battle is fought on: a flat field or a globe.
function tacticsWorldType() {
  return readSetting("tacticsworldtype") === "planet" ? "planet" : "flat";
}
// The map the Tactics lobby is about to send: a named battle brings its own,
// a custom one is whichever map the world type offers.
function tacticsMapScript() {
  return tacticsScenarioId() || readSetting("maptype");
}
// Which world type a Tactics map belongs to, read from the sizes it is
// offered at: every size of a map carries the topology it is built on, and
// the same /rules answer publishes both lists.
function tacticsScriptWorldType(script) {
  const size = battlefieldSizesForScript(script)[0];
  return size && size.topology === "planet" ? "planet" : "flat";
}
// The custom maps a world type offers: Land on a flat field, Land or Ocean on
// a globe. A named battle is never on it — a battle brings its own map and is
// chosen one control up.
function tacticsMapsForWorldType(worldType) {
  return battlefieldScripts().filter(script =>
    !isScenarioMapScript(script.id) && tacticsScriptWorldType(script.id) === worldType);
}
// Choose an option a select actually offers; a value it does not is left
// alone rather than blanking the control.
function setSelectValue(id, value) {
  const select = document.getElementById(id);
  if (!select) return false;
  if (![...select.options].some(option => option.value === value)) return false;
  select.value = value;
  return true;
}
// The Scenario control: Custom first and by default, then every named battle
// the catalog carries, grouped by era, earliest first. Filled from /rules
// once it has answered, so a battle added to the catalog reaches the menu
// without a markup change; the choice made before a refill survives it.
function syncScenarioMenu() {
  const select = document.getElementById("tactics-scenario");
  const scenarios = historicalScenarios();
  if (!select || !scenarios.length || select.dataset.filled === String(scenarios.length)) return;
  const chosen = select.value;
  select.innerHTML = `<option value="">Custom</option>` + scenarioEraNames().map(era => {
    const battles = scenarios.filter(scenario => scenario.era === era);
    if (!battles.length) return "";
    return `<optgroup label="${escapeAttr(era)}">` + battles.map(scenario =>
      `<option value="${escapeAttr(scenario.id)}" title="${escapeAttr(scenario.summary)}">` +
      `${escapeAttr(scenario.name)} · ${escapeAttr(scenario.date)}</option>`).join("") + `</optgroup>`;
  }).join("");
  select.dataset.filled = String(scenarios.length);
  if (!setSelectValue("tactics-scenario", chosen)) select.value = "";
}
function scenarioBriefMarkup(scenario) {
  const forces = scenario.forces.map((force, index) =>
    `<div class="scenario-force"><strong>${escapeAttr(force.label)}</strong>` +
    `<small>${escapeAttr(scenario.civs[index])} · ${escapeAttr(force.commander)}</small>` +
    `<small>${force.units.map(titleCase).map(escapeAttr).join(" · ")}</small></div>`).join("");
  const weather = (scenario.disasters || [])
    .map(kind => SCENARIO_WEATHER_LABELS[kind] || titleCase(kind).toLowerCase())
    .join(" · ");
  return `<h4>${escapeAttr(scenario.name)} · ${escapeAttr(scenario.date)}</h4>` +
    `<div class="scenario-brief-facts"><span>${escapeAttr(scenario.location)}</span><span>${escapeAttr(scenarioTerrainName(scenario.terrain))}</span>` +
    `<span>${escapeAttr(scenario.turns)} recommended turns</span><span>${escapeAttr(scenario.width)}×${escapeAttr(scenario.height)} chart</span>` +
    (weather ? `<span title="The weather the real battle is remembered for — the one disaster class this arena still runs">Historical weather: ${escapeAttr(weather)}</span>` : "") +
    `</div>` +
    `<p class="scenario-brief-objective"><strong>Objective:</strong> ${escapeAttr(scenario.objective)}</p>` +
    `<p class="scenario-brief-lede">${escapeAttr(scenario.summary)}</p>` +
    `<div class="scenario-brief-forces">${forces}</div>`;
}
// The briefing sits directly under the Scenario control and describes the
// battle chosen there; Custom has no history to brief and shows nothing.
function renderScenarioBrief(scenario) {
  const brief = document.getElementById("tactics-scenario-brief");
  if (!brief) return;
  brief.hidden = !scenario;
  brief.innerHTML = scenario ? scenarioBriefMarkup(scenario) : "";
}
// Everything that follows from which battle is chosen: a named battle hides
// the world-type and map controls it decides itself, fixes the arena economy
// it was fought under, and shows its briefing; Custom hands all of it back.
// Leaving Tactics hands it back too, whatever the control still says.
function syncScenarioChoice() {
  const scenario = tacticsMode() ? historicalScenario(tacticsScenarioId()) : null;
  document.body.classList.toggle("tactics-preset", !!scenario);
  syncScenarioSettings();
  renderScenarioBrief(scenario);
}
// Read a Tactics world back into the panel: the battle if its map is a named
// one, else Custom with the map's world type and the map itself, and then
// the size whose dimensions these are. The mode and the battle are put in
// place first so `syncSetupMode` rebuilds every roster around them; the map
// and the size are chosen after, once their rosters exist.
function adoptTacticsWorld(script, width, height) {
  const scenario = historicalScenario(script);
  document.getElementById("gamemode").value = "tactics";
  if (!setSelectValue("tactics-scenario", scenario ? script : "")) {
    const select = document.getElementById("tactics-scenario");
    if (select) select.value = "";
  }
  if (!scenario) setSelectValue("tacticsworldtype", tacticsScriptWorldType(script));
  syncSetupMode();
  if (!scenario) setSelectValue("maptype", script);
  syncBattlefieldSizes(true);
  const size = battlefieldSizesForScript(script)
    .find(size => size.width === width && size.height === height);
  if (size && setSelectValue("np", size.id)) {
    tacticsSizeChoices[script] = size.id;
    document.getElementById("np").dataset.tacticsChoice = size.id;
  }
  syncScenarioChoice();
}
function scenarioTurnChoice(turns) {
  const select = document.getElementById("tacticsturnlimit");
  if (!select) return;
  const choices = [...select.options].map(option => Number(option.value)).filter(Number.isFinite);
  if (!choices.length) return;
  select.value = String(choices.reduce((best, choice) =>
    Math.abs(choice - turns) < Math.abs(best - turns) ? choice : best, choices[0]));
}
function isScenarioMapScript(id) {
  return scenarioScripts().some(script => script.id === id);
}
function syncScenarioSettings() {
  // The arena settings a scenario decides for itself, by control id. Inside
  // the function deliberately: `syncSetupMode` calls this during load, from a
  // top-level statement that runs *above* where this file's later constants
  // are initialised. A `const` list up there is in its temporal dead zone at
  // that moment, and reading it threw a ReferenceError out of the load path —
  // which took the rest of the module with it and left the whole page blank.
  // A function declaration hoists; the array it closes over must not need to.
  const fixed = ["tacticsfog", "tacticsflag", "tacticscities", "tacticsproduction",
                 "tacticsgold", "tacticsturnspertech", "tacticsuniqueunits"];
  const scenario = tacticsMode() && !!historicalScenario(tacticsScenarioId());
  for (const id of fixed) {
    const select = document.getElementById(id);
    if (!select) continue;
    select.disabled = scenario;
    const label = select.closest("label");
    if (label) label.classList.toggle("setting-fixed", scenario);
    // Show what will actually be played rather than a stale choice sitting
    // under a greyed control: every one of these is off or zero on a
    // scenario, and "0" is the option value both spellings use.
    if (scenario && [...select.options].some(option => option.value === "0")) {
      if (select.dataset.scenarioSaved === undefined) select.dataset.scenarioSaved = select.value;
      select.value = "0";
    } else if (!scenario && select.dataset.scenarioSaved !== undefined) {
      const saved = select.dataset.scenarioSaved;
      if ([...select.options].some(option => option.value === saved)) select.value = saved;
      delete select.dataset.scenarioSaved;
    }
  }
  // The era control is fixed the same way, but to the battle's own era
  // rather than to "0": Gettysburg is Industrial whatever was chosen, and
  // the greyed control should say so. The ladder is written here rather than
  // at top level for the load-order reason above.
  const eras = document.getElementById("tacticsera");
  if (eras) {
    const ladder = ["ancient", "classical", "medieval", "renaissance",
                    "industrial", "modern", "atomic", "information", "future"];
    const battle = tacticsMode() ? historicalScenario(tacticsScenarioId()) : null;
    eras.disabled = !!battle;
    const label = eras.closest("label");
    if (label) label.classList.toggle("setting-fixed", !!battle);
    if (battle && ladder[battle.era_index] &&
        [...eras.options].some(option => option.value === ladder[battle.era_index])) {
      if (eras.dataset.scenarioSaved === undefined) eras.dataset.scenarioSaved = eras.value;
      eras.value = ladder[battle.era_index];
    } else if (!battle && eras.dataset.scenarioSaved !== undefined) {
      const saved = eras.dataset.scenarioSaved;
      if ([...eras.options].some(option => option.value === saved)) eras.value = saved;
      delete eras.dataset.scenarioSaved;
    }
  }
  syncTacticsEraPool();
}
function battlefieldSizesForScript(script) {
  return battlefieldSizes().filter(size => !size.script || size.script === script);
}
function battlefieldSize(id) {
  return battlefieldSizes().find(size => size.id === id) || null;
}
function isBattlefieldMapScript(id) {
  return id === "battlefield" || battlefieldScripts().some(script => script.id === id);
}
// The world on screen right now, in the one term the mode chip cares about.
// Before the first observation the query string is the only evidence there
// is, and a link into a battlefield carries the map in it — so a deep link
// never spends its first seconds offering the mode it just opened.
function watchingBattlefield() {
  if (state && state.map && state.map.script !== undefined)
    return isBattlefieldMapScript(state.map.script);
  const asked = new URL(location.href).searchParams.get("map") || "";
  return isBattlefieldMapScript(asked.trim().toLowerCase());
}
// The world the Tactics side of the chip opens: the same two even armies on
// the same 20x20 field the home page's Tactics card offers, so the site has
// one Tactics world rather than two that differ for no reason. `era=random`
// is the lobby's own Tactics default — a fresh era every battle — said in the
// link, because a link that names nothing leaves the rule on whatever the
// previous world set.
const TACTICS_CHIP_QUERY = "map=battlefield&players=2&era=random&arena=20x20";
// The mode chip beside Home. It always names the mode the deck is NOT
// showing, which makes the whole distance between Civvis and Tactics one click
// in either direction. Going back to Civvis asks for nothing at all, because a
// visit that names no settings is the stock exhibition. Both destinations
// keep the path this document was served from: the front page and /test are
// different builds of the viewer, and choosing a game mode is no reason to
// move a viewer off the lane they came in on.
function syncModeLink(tactics = watchingBattlefield()) {
  const link = document.getElementById("modelink");
  if (!link) return;
  // A circle crossed by its own equator and meridian: the astronomer's Earth,
  // and the brand mark directly above it. Both marks are drawn in outline
  // because the ⌂ they stand beside is — a filled ◉ shouts over it.
  link.textContent = tactics ? "⊕ Civvis" : "⚔ Tactics";
  link.href = tactics ? location.pathname : `${location.pathname}?${TACTICS_CHIP_QUERY}`;
  link.title = tactics
    ? "Leave the arena for the full game: whole civilizations on a fresh world"
    : "Watch the Tactics mode: two even armies on one bounded field";
}
function syncMapRoster(civ6, tactics) {
  const select = document.getElementById("maptype");
  if (!select) return;
  const maps = civ6Maps();
  // Before /rules has answered there is nothing to swap to; syncSetupMode runs
  // again once it has.
  if (civ6 && !maps.length) return;
  if (tactics && !battlefieldScripts().length) return;
  const roster = civ6 ? "civ6" : tactics ? "tactics" : "civvis";
  // In Tactics the roster is the chosen world type's, so a change of world
  // type is a change of roster too.
  const world = tactics ? tacticsWorldType() : "";
  const offered = tactics ? tacticsMapsForWorldType(world) : [];
  if (tactics && !offered.length) return;
  // `/rules` refills this control after load, so what is put back must be
  // whatever it holds now rather than the markup it was born with.
  if (select.dataset.roster !== "civ6" && select.dataset.roster !== "tactics")
    civvisMapOptions = select.innerHTML;
  if ((select.dataset.roster || "civvis") === roster && (select.dataset.tacticsWorld || "") === world)
    return;
  const chosen = select.value;
  const chosenName = select.selectedOptions[0]?.textContent || "";
  // The world chosen for the Civ game survives a round trip through either
  // other roster, so flipping a mode on and off costs nobody their map; the
  // Tactics map likewise survives a trip out to Civ and back.
  if (select.dataset.roster !== "civ6" && select.dataset.roster !== "tactics")
    select.dataset.civvisChoice = chosen;
  if (select.dataset.roster === "tactics") select.dataset.tacticsChoice = chosen;
  if (civ6) {
    select.innerHTML = maps.map(map =>
      `<option value="${escapeAttr(map.id)}">${map.name}</option>`).join("");
    const carried = maps.find(map => map.civvis === chosen);
    select.value = carried ? carried.id : (RULES.civ6.default_map || maps[0].id);
  } else if (tactics) {
    // The Tactics menu is the maps of the chosen world type — Land on a flat
    // field, Land or Ocean on a globe — cut from the battlefield roster in
    // the same /rules answer, so a new arena arrives without a page change.
    // Land stays Land across a change of world type: the same map on the
    // other shape, else the map last chosen here, else the first offered.
    select.innerHTML = offered.map(script =>
      `<option value="${escapeAttr(script.id)}" title="${escapeAttr(script.description)}">${script.name}</option>`).join("");
    const sameName = select.dataset.roster === "tactics" && offered.find(script => script.name === chosenName);
    const carried = sameName || offered.find(script => script.id === select.dataset.tacticsChoice) || offered[0];
    select.value = carried.id;
  } else {
    select.innerHTML = civvisMapOptions;
    const carried = maps.find(map => map.id === chosen);
    const restored = carried && carried.civvis ? carried.civvis : select.dataset.civvisChoice;
    if (restored && [...select.options].some(option => option.value === restored))
      select.value = restored;
  }
  select.dataset.roster = roster;
  select.dataset.tacticsWorld = world;
  syncEarthShape();
}
// An arena is fought over, not converted or researched to death: entering
// Tactics has one victory lane: last army standing through Domination. Its
// clock is a draw deadline, not Score. Leaving it puts back whatever was
// chosen for the Civ game. Its two state variables are declared with the
// other mode state, before the wiring that calls this during load.
function syncBattlefieldVictories(tactics) {
  const roster = tactics ? "tactics" : "civvis";
  if (roster === victoryRoster) return;
  victoryRoster = roster;
  const boxes = VICTORY_TRACKS
    .map(track => [track.id, document.getElementById(`victory-${track.id}`)])
    .filter(([, box]) => box);
  if (tactics) {
    civVictoryChoices = new Set(boxes.filter(([, box]) => box.checked).map(([id]) => id));
    for (const [id, box] of boxes) box.checked = id === "domination";
  } else if (civVictoryChoices) {
    for (const [id, box] of boxes) box.checked = civVictoryChoices.has(id);
  }
  syncRequiredVictoriesCap();
}
// The world-size control is the Tactics-map-size control in Tactics: the same
// select carries a different roster, exactly as the map control does for Civ 6
// above. Which sizes it offers follows from the battle, world type and map
// chosen above it — a named battle lists only the sizes it is charted at, a
// custom map the sizes it is drawn at. Every Tactics map seats exactly two
// sides, so the option says so and the seat-dependent controls re-fit when
// the roster moves either way.
function syncBattlefieldSizes(tactics) {
  const sizes = document.getElementById("np");
  if (!sizes) return;
  if (tactics && !battlefieldSizes().length) return;
  const roster = tactics ? "tactics" : "civvis";
  const sameRoster = (sizes.dataset.roster || "civvis") === roster;
  const label = sizes.closest("label");
  if (sizes.dataset.roster !== "tactics") civvisSizeOptions = sizes.innerHTML;
  if (tactics) {
    if (!sameRoster) sizes.dataset.civvisChoice = sizes.value;
    const script = tacticsMapScript();
    const available = battlefieldSizesForScript(script);
    if (!available.length) return;
    // The size last chosen for this map, if it was this map's roster that
    // was on the control; else the one remembered for the map from an
    // earlier visit; else the smallest.
    const held = sameRoster && sizes.dataset.tacticsScript === script ? sizes.value : "";
    const chosen = held || tacticsSizeChoices[script] || sizes.dataset.tacticsChoice;
    sizes.innerHTML = available.map(size =>
      `<option value="${escapeAttr(size.id)}">${size.name} · 2 civs</option>`).join("");
    const carried = chosen && available.some(size => size.id === chosen)
      ? chosen : available[0].id;
    sizes.value = carried;
    sizes.dataset.tacticsChoice = carried;
    sizes.dataset.tacticsScript = script;
    tacticsSizeChoices[script] = carried;
    // A named battle charted at one size has nothing to choose here: the
    // control stays, saying what will be played, but reads as reported.
    const fixed = !!historicalScenario(script) && available.length === 1;
    sizes.disabled = fixed;
    if (label) label.classList.toggle("setting-fixed", fixed);
  } else {
    if (sameRoster) return;
    const chosen = sizes.value;
    sizes.dataset.tacticsChoice = chosen;
    sizes.innerHTML = civvisSizeOptions;
    const restored = sizes.dataset.civvisChoice;
    if (restored && [...sizes.options].some(option => option.value === restored))
      sizes.value = restored;
    sizes.disabled = false;
    if (label) label.classList.remove("setting-fixed");
  }
  sizes.dataset.roster = roster;
  // Which Tactics map is selected decides how much of the arena card is still
  // the player's, so this follows every change of it.
  syncScenarioSettings();
  syncTeams();
  syncCustomLeaderSelection();
}

function normalizedLeaderPoolId(id) {
  return id === "expanded" ? "historical" : id;
}

// The server owns the ordered roster data.  That makes the picker match the
// identities it can actually seat, including future Today's Leaders records
// that do not have an entry in the Civilization VI rules data.
function syncLeaderPool() {
  const select = document.getElementById("leader");
  const pool = document.getElementById("leaderpool");
  if (!select || !pool || !RULES || !Array.isArray(RULES.leader_pools)) return;
  const selected = select.value;
  const requested = normalizedLeaderPoolId(pool.value);
  const pools = RULES.leader_pools;
  const active = pools.find(entry => entry.id === requested && entry.available)
    || pools.find(entry => entry.available);
  if (!active) return;
  pool.innerHTML = pools.map(entry =>
    `<option value="${escapeAttr(entry.id)}"${entry.available ? "" : " disabled"}` +
    ` title="${escapeAttr(entry.description || "")}">${escapeAttr(entry.name)}</option>`
  ).join("");
  pool.value = active.id;
  const leaders = (active.leaders || [])
    .map(entry => ({
      civ: entry.civ,
      leader: entry.leader || entry.civ,
      latitude: Number(entry.latitude),
      longitude: Number(entry.longitude),
    }))
    .sort((first, second) => first.leader.localeCompare(second.leader));
  select.innerHTML = `<option value="">Random leader</option>` + leaders.map(entry =>
    `<option value="${escapeAttr(entry.civ)}" title="${escapeAttr(
      `True Start: ${entry.latitude.toFixed(2)}°, ${entry.longitude.toFixed(2)}°`
    )}">${escapeAttr(entry.leader)} of ${escapeAttr(entry.civ)}</option>`).join("");
  if (leaders.some(entry => entry.civ === selected)) select.value = selected;
  syncCustomLeaderSelection();
}

function activeLeaderPool() {
  const requested = normalizedLeaderPoolId(readSetting("leaderpool"));
  const pools = Array.isArray(RULES?.leader_pools) ? RULES.leader_pools : [];
  return pools.find(pool => pool.id === requested && pool.available)
    || pools.find(pool => pool.available) || null;
}
function customLeaderEntries() {
  return (activeLeaderPool()?.leaders || [])
    .map(entry => ({
      civ: entry.civ,
      leader: entry.leader || entry.civ,
    }))
    .sort((first, second) => `${first.civ} ${first.leader}`.localeCompare(`${second.civ} ${second.leader}`));
}
function customLeaderRowsFromDom() {
  return [...document.querySelectorAll("#custom-leader-rows tr[data-seat]")].map(row => ({
    team: row.querySelector("[data-custom-team]")?.value ?? "",
    civ: row.querySelector("[data-custom-civ]")?.value ?? "",
    leader: row.querySelector("[data-custom-civ] option:checked")?.dataset.leader || "",
    elo: row.querySelector("[data-custom-elo]")?.value ?? "",
  }));
}
function customLeaderEloOptions(civ, leader) {
  const match = (RULES?.leader_elo_options || []).find(entry =>
    entry.civ === civ && entry.leader === leader);
  return Array.isArray(match?.elos) ? match.elos : [];
}
function customTeamOptions(players, rule, selected) {
  const count = teamCount(players, rule);
  if (!count) return `<option value="" selected>Free-for-all</option>`;
  return Array.from({length: count}, (_, team) =>
    `<option value="${team}"${String(team) === String(selected) ? " selected" : ""}>Team ${team + 1}</option>`
  ).join("");
}
function customEloOptions(civ, leader, selected) {
  const options = customLeaderEloOptions(civ, leader);
  if (!options.length)
    return `<option value="" selected>No recorded ELO</option>`;
  const value = options.some(option => String(option.elo) === String(selected))
    ? String(selected) : String(options[0].elo);
  return options.map(option => {
    const elo = String(option.elo);
    const source = (option.strategies || []).join(", ");
    return `<option value="${escapeAttr(elo)}"${elo === value ? " selected" : ""}` +
      `${source ? ` title="${escapeAttr(source)}"` : ""}>${escapeAttr(elo)}</option>`;
  }).join("");
}
function syncCustomLeaderSelection() {
  const section = document.getElementById("custom-leader-selection");
  const body = document.getElementById("custom-leader-rows");
  if (!section || !body) return;
  const custom = readSetting("leaderselection") === "custom";
  section.hidden = !custom;
  if (!custom) return;
  const prior = customLeaderRowsFromDom();
  const entries = customLeaderEntries();
  // A battlefield id in the size control means two seats, not a seat count.
  const players = battlefieldSize(readSetting("np"))
    ? 2 : Math.max(0, Number(readSetting("np")) || 0);
  const rule = readSetting("teams");
  const automaticTeams = teamAssignment(players, rule);
  if (!entries.length || !players) {
    body.innerHTML = `<tr><td class="custom-leader-empty" colspan="3">No leaders are available in this pool.</td></tr>`;
    return;
  }
  const rows = Array.from({length: players}, (_, seat) => {
    const old = prior[seat] || {};
    const entry = entries.find(candidate => candidate.civ === old.civ) || entries[seat % entries.length];
    const team = teamCount(players, rule)
      ? (Number.isInteger(Number(old.team)) && Number(old.team) < teamCount(players, rule)
        ? String(old.team) : String(automaticTeams[seat] ?? 0))
      : "";
    return {
      team,
      civ: entry.civ,
      leader: entry.leader,
      elo: customLeaderEloOptions(entry.civ, entry.leader).some(option =>
        String(option.elo) === String(old.elo)) ? String(old.elo) : "",
    };
  });
  body.innerHTML = rows.map((row, seat) => `<tr data-seat="${seat}">
    <td><select data-custom-team aria-label="Team for seat ${seat + 1}">${customTeamOptions(players, rule, row.team)}</select></td>
    <td><select data-custom-civ aria-label="Civilization and leader for seat ${seat + 1}">${entries.map(entry =>
      `<option value="${escapeAttr(entry.civ)}" data-leader="${escapeAttr(entry.leader)}"` +
      `${entry.civ === row.civ ? " selected" : ""}>${escapeAttr(entry.civ)} — ${escapeAttr(entry.leader)}</option>`
    ).join("")}</select></td>
    <td><select data-custom-elo aria-label="ELO for seat ${seat + 1}">${customEloOptions(row.civ, row.leader, row.elo)}</select></td>
  </tr>`).join("");
}

// Teams are permanent and decided before the world exists, so the lobby is
// where they are chosen. The control carries the *rule* — two teams, pairs —
// rather than the assignment it makes, because which assignment that is
// depends on a world size chosen two rows further down.
//
// A function rather than a `const`, for the reason above: `syncTeams` is
// reachable from the setup panel's load-time sync, and this is declared far
// below the top-level statement that starts it. Only an early return keeps
// today's load from getting here — the same latent trap that #1447 turned
// into a live one two functions away. A declaration hoists; a `const` does
// not, and reading one early blanks the page.
function teamRules() { return ["2", "3", "4", "pairs"]; }
// A team of one is not a team, so a split is only on offer while every team it
// makes can seat at least two civilizations.
function teamCount(players, rule) {
  if (!Number.isFinite(players) || players < 4) return 0;
  const most = Math.floor(players / 2);
  if (rule === "pairs") return most;
  const teams = Number(rule);
  return Number.isFinite(teams) && teams >= 2 && teams <= most ? teams : 0;
}
// The seats are dealt out in blocks, largest team first, so a division that
// does not come out even puts its spare civ at the top rather than scattering
// odd teams through the middle. Seat 0 is the person at the keyboard, so a
// human always leads the first team.
function teamAssignment(players, rule) {
  const teams = teamCount(players, rule);
  if (!teams) return [];
  const smallest = Math.floor(players / teams);
  const larger = players % teams;
  const assignment = [];
  for (let team = 0; team < teams; team++)
    for (let seat = smallest + (team < larger ? 1 : 0); seat > 0; seat--)
      assignment.push(team);
  return assignment;
}
function teamSizes(players, rule) {
  const sizes = [];
  for (const team of teamAssignment(players, rule)) sizes[team] = (sizes[team] || 0) + 1;
  return sizes;
}
// "4v4", or "4v3v3" where the seats do not divide evenly — until there are so
// many teams that the list stops being readable and the shape is described
// instead: a hundred civilizations in pairs is "50 teams of 2".
function teamSplit(players, rule) {
  const sizes = teamSizes(players, rule);
  if (sizes.length <= 6) return sizes.join("v");
  const smallest = Math.min(...sizes), largest = Math.max(...sizes);
  return `${sizes.length} teams of ` +
    (smallest === largest ? smallest : `${smallest}–${largest}`);
}
// How a division is said inside a sentence rather than inside its own option.
function teamPhrase(players, rule) {
  const split = teamSplit(players, rule);
  return !split || split.includes("teams") ? split : `${split} teams`;
}
// The world on screen and the staged world both arrive as an assignment, which
// is read back here as the rule that would produce it.
function teamRuleFromArray(teams) {
  if (!Array.isArray(teams)) return "ffa";
  const distinct = new Set(teams.filter(team => team !== null && team !== undefined));
  if (distinct.size < 2) return "ffa";
  const count = String(distinct.size);
  return teamRules().includes(count) ? count : "pairs";
}
// Which splits this world size can actually seat, said in the option itself:
// "2 teams · 4v4" is the whole answer, and a split this size cannot seat is
// disabled where it can be seen rather than quietly dropped from the list.
function syncTeams() {
  const select = document.getElementById("teams");
  const size = document.getElementById("np");
  if (!select || !size) return;
  // A battlefield id in the size control means two seats, not a seat count.
  const players = battlefieldSize(size.value) ? 2 : +size.value;
  for (const option of select.options) {
    if (!teamRules().includes(option.value)) continue;
    const name = option.value === "pairs" ? "Pairs" : `${option.value} teams`;
    const split = teamSplit(players, option.value);
    option.disabled = !split;
    option.textContent = split ? `${name} · ${split}` : name;
  }
  // A size that cannot seat the chosen split leaves the lobby free-for-all
  // rather than starting a game with a division nobody asked for.
  if (select.selectedOptions[0]?.disabled) select.value = "ffa";
}

// Fixed geography chooses the coastline, not the projection. Keep the shape
// control available for Earth just as it is for every generated map type.
function syncEarthShape() {
  const type = document.getElementById("maptype");
  const shape = document.getElementById("mapshape");
  if (!type || !shape) return;
  shape.disabled = false;
  shape.title = "";
}

// The saves this server is holding. A build without the save endpoints simply
// does not show the group, rather than showing one that cannot work.
let savesKnown = null;
async function refreshSaves() {
  const group = document.getElementById("saves-group");
  if (!group || SPEC) { if (group) group.style.display = "none"; return; }
  let saves;
  try { saves = (await fetchJSON("/saves", {}, 4000)).saves; }
  catch (error) { group.style.display = "none"; savesKnown = null; return; }
  if (!Array.isArray(saves)) { group.style.display = "none"; return; }
  savesKnown = saves;
  group.style.display = "block";
  document.getElementById("saves-count").textContent = saves.length ? ` ${saves.length}` : "";
  document.getElementById("saves-list").innerHTML = saves.length
    ? saves.slice(0, 8).map(save =>
        `<div class="save-entry"><div><b>${escapeAttr(save.name)}</b><br>` +
        `<span>Turn ${save.turn}${save.civ ? ` · ${escapeAttr(save.civ)}` : ""}` +
        `${save.difficulty ? ` · ${titleCase(save.difficulty)}` : ""}</span></div>` +
        `<button onclick='loadSave(${JSON.stringify(save.name)})'>Load</button></div>`).join("")
    : `<div class="empty-state">No saves yet. One is written at the end of every turn.</div>`;
}
async function loadSave(name) {
  try {
    sel = null; selCity = null;
    const next = await fetchJSON("/load", {method: "POST",
      headers: {"Content-Type": "application/json"},
      body: JSON.stringify({name})});
    if (next.error) throw new Error(next.error);
    render(next);
  } catch (error) {
    showNewSimulationError(error, "load that save");
  }
}
async function writeSave() {
  const box = document.getElementById("savename");
  const name = (box.value || "").trim() || `turn-${state ? state.turn : 0}`;
  try {
    const done = await fetchJSON("/save", {method: "POST",
      headers: {"Content-Type": "application/json"},
      body: JSON.stringify({name})});
    if (done.error) throw new Error(done.error);
    box.value = "";
    await refreshSaves();
  } catch (error) {
    showNewSimulationError(error, "write that save");
  }
}

// Start a real game of Civilization VI on the computer serving this page.
//
// Nothing about the world-handoff path applies: no CIVVIS world is built, this
// server is not replaced, and the page keeps showing whatever it was showing.
// What is started is another application, and it takes about three minutes to
// reach its first turn — so the reply is a receipt and the status line above
// the button is where the run is then followed.
async function startCiv6Game() {
  const button = document.getElementById("restart-sim");
  const host = document.getElementById("civ6-host");
  if (button) button.disabled = true;
  if (host) host.textContent = "Starting Civilization VI…";
  try {
    const reply = await fetchJSON("/civ6/start", {method: "POST",
      headers: {"Content-Type": "application/json"},
      body: JSON.stringify(selectedSimulationSettings())}, 30000);
    if (reply.error) throw new Error(reply.error);
    if (host) host.textContent =
      `Started ${reply.started.tag} · the game takes a few minutes to come up`;
  } catch (error) {
    showNewSimulationError(error, "start Civilization VI");
  } finally {
    if (button) button.disabled = false;
    refreshCiv6Status();
  }
}
async function startNewSimulation(restartSource = "manual") {
  if (newSimulationBusy) return;
  if (readSetting("humanplayers") === "civ6") { startCiv6Game(); return; }
  cancelFinaleCountdown();
  const wasPaused = specPaused;
  const payload = {...newSimulationPayload(), paused: wasPaused};
  const supervised = !!(state && state.supervised) && payload.spectate;
  const handoff = {
    requestedAt: Date.now(), paused: wasPaused, targetSeed: payload.seed,
    supervised, reusePage: supervised,
    finishedInstance: state?.server_instance ?? null,
    finishedSeed: state?.seed ?? null,
    summary: selectedSimulationSummary(payload),
  };
  setNewSimulationBusy(true);
  showWorldTransition(handoff);
  try {
    // A change event stages settings immediately. Let every already-issued
    // settings write finish before the process handoff so none can arrive late
    // and accidentally queue itself on the successor as *its* next game.
    await settingsStageChain.catch(() => {});
    setWorldTransitionStage("Starting the selected world");
    // The supervisor owns the AI exhibition — each simulation is a fresh
    // process on freshly built code. A single-player game is not part of that
    // cycle: it takes over this process in place, so sitting down to play
    // starts immediately instead of waiting out a process handoff.
    sel = null; selCity = null;
    clearTimeout(specTimer);
    if (supervised) {
      // Use the identity captured before the settings write above. The port
      // may belong to a successor by the time this POST runs; that server must
      // reject a request created for the world it replaced.
      const finishedInstance = handoff.finishedInstance;
      const finishedSeed = handoff.finishedSeed;
      const requested = await fetchJSON("/supervisor-new", { method: "POST",
        headers: {"Content-Type": "application/json"},
        body: JSON.stringify({...payload, mode: "restart",
          restart_source: restartSource, replace_world: {
          server_instance: finishedInstance, seed: finishedSeed,
        }}) });
      if (requested.error) throw new Error(requested.error);
      const respawn = document.getElementById("respawn");
      if (respawn) respawn.textContent = "restarting sim on existing code";
      setWorldTransitionStage("Restart accepted · changing worlds");
      waitForSupervisedSuccessor(finishedInstance, finishedSeed);
      return;
    }

    specPaused = wasPaused;
    updateSpecPauseButtons();
    const next = await fetchJSON("/new", { method: "POST",
      headers: {"Content-Type": "application/json"},
      body: JSON.stringify(payload) });
    if (next.error) throw new Error(next.error);
    render(next);
    setPace({paused: wasPaused});
    reportSpecStatus();
    scheduleSpec(state);
    setNewSimulationBusy(false);
  } catch (error) {
    cancelSupervisedSuccessorWatch();
    clearWorldTransition();
    specPaused = wasPaused;
    updateSpecPauseButtons();
    reportSpecStatus();
    scheduleSpec(state);
    setNewSimulationBusy(false);
    showNewSimulationError(error);
  }
}
// Keep the world that just ended instead of taking the next one. Looking
// around uses the next-victory rule but holds the world on its final frame;
// the two play choices resume immediately and either stop for the next
// distinct victory or suppress every later result.
//
// This has to outrun nothing, but it does have to *stop* things: the countdown
// on screen, the successor watcher armed when the result appeared, and — on the
// exhibition — the supervisor's own cooldown, which reads the cleared winner
// out of `/state` and stands down.
let playOnBusy = false;
async function playOnPastVictory(mode, paused) {
  if (playOnBusy) return;
  playOnBusy = true;
  const buttons = [...document.querySelectorAll(".winner-playon")];
  for (const button of buttons) button.disabled = true;
  cancelSupervisedSuccessorWatch();
  cancelFinaleCountdown();
  try {
    const next = await fetchJSON("/play-on", {method: "POST",
      headers: {"Content-Type": "application/json"}, body: JSON.stringify({mode, paused})});
    if (next.error) throw new Error(next.error);
    render(next);
    scheduleSpec(state);
  } catch (error) {
    showNewSimulationError(error, "play on");
    // The world is being retired after all, so put the watcher back or a
    // supervised page would sit on a dead result until somebody reloads it.
    if (state && state.supervised && SPEC)
      waitForSupervisedSuccessor(state.server_instance, state.seed);
  } finally {
    playOnBusy = false;
    for (const button of buttons) button.disabled = false;
  }
}
document.getElementById("restart-sim").onclick = startNewSimulation;
document.getElementById("newgame-options").addEventListener("change", stageSelectedSimulationSettings);
