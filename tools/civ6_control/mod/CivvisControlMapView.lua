-- WorldInput.lua:1202-1206: larger GetMapZoom values zoom OUT.
-- CameraManager.lua:32-49: combat style 1 pans to every combat the local
-- seat is in, style 2 also zooms; :59 CombatVisEnd always restores the
-- saved zoom (UI.RestoreMapZoom), so the restore value is ours too.
-- During automated play the camera stays close and on the part of the map
-- where our civilization is acting: the battles we fight, the cities we
-- found or take, the districts and improvements we place, the units we move.
-- That is the view the operator watches and the clip recorder films.
--
-- Activity is remembered per plot and halves in weight every turn. The
-- camera aims at the plot whose surrounding area is hottest, and moves on
-- only when another area is clearly hotter AND far enough away to be a
-- different place, never more often than once per HOLD_PULSES HUD pulses,
-- and never while our own turn is active: one front at a time, not a camera
-- that chases every unit. Native combat panning is off while this runs,
-- because it yanked the view to every skirmish on every seat's turn; the
-- option is read only by CameraManager's own handler (:32, :34), so the
-- CombatVisBegin/End events the agent's ledger reads still fire.
CivvisMapView = {};
local cfg = CivvisControlConfig or {};
local FOCUS_ZOOM = 0.40;
-- Native float readback lands just off the requested value.
local ZOOM_TOLERANCE = 0.05;
local AREA_RADIUS = 3;      -- plots summed into one area's heat
local REAIM_DISTANCE = 4;   -- a hotter area nearer than this is the same place
local REAIM_MARGIN = 1.25;  -- and must beat the watched area by this much
local HOLD_PULSES = 4;      -- one-second HUD pulses between re-aims
local REASSERT_PULSES = 10; -- re-centre on the aim if something else panned
local MEMORY_TURNS = 4;
local MAX_SPOTS = 96;
-- Moves are weighed per plot stepped (UnitMoved fires once per step), so a
-- march counts along its path and most where it stops and fights.
local WEIGHT = { combat = 6, city = 8, district = 2, military = 0.5,
                 civilian = 0.25, improvement = 0.5 };

local spots, spotCount = {}, 0;
local aim = nil;
local pulses, lastAimPulse, lastLookPulse = 0, 0, 0;
local dirty = true;
local retryBlocked = false;
local adjusting = false;
local lastVerified = nil;
local plotDistance = nil;

local function automated() return cfg.Play ~= false and cfg.CivvisDecides; end

local function localPlayer()
    local ok, pid = pcall(function() return Game.GetLocalPlayer(); end);
    return ok and tonumber(pid) or -1;
end

local function currentTurn()
    local ok, turn = pcall(function() return Game.GetCurrentGameTurn(); end);
    return ok and tonumber(turn) or 0;
end

-- Map.GetPlotDistance knows the world's wrap; the fallback only keeps a
-- missing API from blinding the camera.
local function distance(ax, ay, bx, by)
    if plotDistance == nil then
        plotDistance = false;
        pcall(function()
            if type(Map.GetPlotDistance) == "function" then plotDistance = Map.GetPlotDistance; end
        end);
    end
    if plotDistance then
        local ok, d = pcall(plotDistance, ax, ay, bx, by);
        d = ok and tonumber(d) or nil;
        if d ~= nil and d >= 0 then return d; end
    end
    return math.max(math.abs(ax - bx), math.abs(ay - by));
end

local function heatOf(spot, turn)
    return spot.w * 0.5 ^ math.max(0, turn - spot.turn);
end

local function forget(turn)
    for key, spot in pairs(spots) do
        if turn - spot.turn > MEMORY_TURNS or spot.turn > turn then
            spots[key] = nil; spotCount = spotCount - 1;
        end
    end
    while spotCount > MAX_SPOTS do
        local coldKey, coldHeat = nil, nil;
        for key, spot in pairs(spots) do
            local heat = heatOf(spot, turn);
            if coldHeat == nil or heat < coldHeat then coldKey, coldHeat = key, heat; end
        end
        spots[coldKey] = nil; spotCount = spotCount - 1;
    end
end

local function areaHeat(x, y, turn)
    local sum = 0;
    for _, spot in pairs(spots) do
        if distance(x, y, spot.x, spot.y) <= AREA_RADIUS then sum = sum + heatOf(spot, turn); end
    end
    return sum;
end

local function log(fields)
    pcall(function()
        Automation.Log("CIVVISJSON " .. string.format('{"kind":"map_view","run":"%s",%s}',
            tostring(cfg.RunTag or "unset"), fields));
    end);
end

function CivvisMapView.Note(x, y, weight)
    x, y, weight = tonumber(x), tonumber(y), tonumber(weight);
    if not automated() or x == nil or y == nil or weight == nil or x < 0 or y < 0 then return; end
    local turn = currentTurn();
    local key = x .. "," .. y;
    local spot = spots[key];
    if spot == nil then
        spot = { x = x, y = y, w = 0, turn = turn };
        spots[key] = spot; spotCount = spotCount + 1;
    end
    spot.w = heatOf(spot, turn) + weight;
    spot.turn = turn;
    dirty = true;
    if spotCount > MAX_SPOTS then forget(turn); end
end

local function noteMine(playerID, x, y, weight)
    if tonumber(playerID) == localPlayer() then CivvisMapView.Note(x, y, weight); end
end

local function home()
    local pid = localPlayer();
    if pid < 0 then return nil; end
    local ok, x, y = pcall(function()
        local player = Players[pid];
        local capital = player:GetCities():GetCapitalCity();
        if capital ~= nil then return capital:GetX(), capital:GetY(); end
        for _, city in player:GetCities():Members() do return city:GetX(), city:GetY(); end
        for _, unit in player:GetUnits():Members() do
            if unit:GetX() >= 0 then return unit:GetX(), unit:GetY(); end
        end
    end);
    x, y = ok and tonumber(x) or nil, ok and tonumber(y) or nil;
    if x == nil or y == nil or x < 0 or y < 0 then return nil; end
    return { x = x, y = y };
end

-- Combat panning would move the camera off the aim for any skirmish, and
-- unit cycling selects the next ready unit after every move, "Engine
-- automatically moves camera" (NotificationPanel.lua:1265). Nothing in the
-- mod reads the selection; the agent orders units by id. Not on every
-- Camera_Updated: that fires each frame the camera moves.
local function quietNativeCamera()
    for _, key in ipairs({"LookAtPlayerTurnCombat", "LookAtPlayerOffTurnCombat",
                          "AutoUnitCycle"}) do
        pcall(function()
            if Options.GetUserOption("Gameplay", key) ~= 0 then
                Options.SetUserOption("Gameplay", key, 0);
            end
        end);
    end
end

function CivvisMapView.Enforce()
    if not automated() or adjusting then return; end
    adjusting = true;
    local ok, before, after = pcall(function()
        local zoom = UI.GetMapZoom();
        if type(zoom) ~= "number" or zoom ~= zoom then return; end
        if math.abs(zoom - FOCUS_ZOOM) <= ZOOM_TOLERANCE then
            retryBlocked = false;
            return zoom, zoom;
        end
        -- A refused or asynchronous correction must not retry on every
        -- Camera_Updated event. The one-second HUD pulse re-arms it.
        if retryBlocked then return; end
        retryBlocked = true;
        UI.SetMapZoom(FOCUS_ZOOM, 0.0, 0.0);
        UI.SetRestoreMapZoom(FOCUS_ZOOM);
        local observed = UI.GetMapZoom();
        if type(observed) == "number" and math.abs(observed - FOCUS_ZOOM) <= ZOOM_TOLERANCE then
            retryBlocked = false;
        end
        return zoom, observed;
    end);
    adjusting = false;
    if ok and type(before) == "number" and type(after) == "number" then
        local verified = math.abs(after - FOCUS_ZOOM) <= ZOOM_TOLERANCE;
        if lastVerified ~= verified or math.abs(before - FOCUS_ZOOM) > ZOOM_TOLERANCE then
            lastVerified = verified;
            log(string.format('"before":%.6f,"zoom":%.6f,"target":%.2f,"verified":%s',
                before, after, FOCUS_ZOOM, tostring(verified)));
        end
    end
end

local function lookAt(x, y)
    UI.LookAtPlot(x, y);
    lastLookPulse = pulses;
end

local function aimAt(x, y, reason, heat)
    lookAt(x, y);
    aim = { x = x, y = y };
    lastAimPulse = pulses;
    log(string.format('"aim":[%d,%d],"reason":"%s","heat":%.2f,"turn":%d',
        x, y, reason, heat, currentTurn()));
    CivvisMapView.Enforce();
end

-- Our order queue drains while our turn is active; a pan then would compete
-- with the moves for frame time, so the camera moves between our turns.
local function ourTurnActive()
    local ok, active = pcall(function() return Players[localPlayer()]:IsTurnActive(); end);
    return ok and active == true;
end

-- `force` is the turn start: our turn is active but the queue has not begun.
function CivvisMapView.Follow(force)
    if not automated() then return; end
    if aim ~= nil and not force and ourTurnActive() then return; end
    if aim ~= nil and (not dirty or pulses - lastAimPulse < HOLD_PULSES) then
        if pulses - lastLookPulse >= REASSERT_PULSES then lookAt(aim.x, aim.y); end
        return;
    end
    local turn = currentTurn();
    forget(turn);
    -- Within the hottest area, centre on its hottest plot (usually the fight).
    local best, bestHeat, bestOwn = nil, 0, 0;
    for _, spot in pairs(spots) do
        local heat, own = areaHeat(spot.x, spot.y, turn), heatOf(spot, turn);
        if heat > bestHeat or (heat == bestHeat and own > bestOwn) then
            best, bestHeat, bestOwn = spot, heat, own;
        end
    end
    dirty = false;
    if best == nil then
        if aim == nil then
            local start = home();
            if start ~= nil then aimAt(start.x, start.y, "home", 0); end
        end
        return;
    end
    if aim ~= nil and (distance(best.x, best.y, aim.x, aim.y) < REAIM_DISTANCE
            or bestHeat < areaHeat(aim.x, aim.y, turn) * REAIM_MARGIN) then
        if pulses - lastLookPulse >= REASSERT_PULSES then lookAt(aim.x, aim.y); end
        return;
    end
    aimAt(best.x, best.y, "activity", bestHeat);
end

function CivvisMapView.Pulse()
    if not automated() then return; end
    pulses = pulses + 1;
    retryBlocked = false;
    quietNativeCamera();
    pcall(CivvisMapView.Follow);
    CivvisMapView.Enforce();
end

-- A load replays every city, district and improvement onto the map and may
-- land turns before what was remembered, so it starts the memory afresh.
function CivvisMapView.Reset()
    spots, spotCount, aim, dirty = {}, 0, nil, true;
end

local function onCombat(members)
    if type(members) ~= "table" then return; end
    local pid = localPlayer();
    local mine = false;
    for _, slot in ipairs({"ATTACKER", "DEFENDER"}) do
        pcall(function()
            local member = members[CombatVisType[slot]];
            if member ~= nil and tonumber(member.playerID) == pid then mine = true; end
        end);
    end
    for index = 1, 2 do
        local member = members[index];
        if type(member) == "table" and tonumber(member.playerID) == pid then mine = true; end
    end
    if mine then CivvisMapView.Note(members.x, members.y, WEIGHT.combat); end
end

local function onUnitMoved(playerID, unitID, x, y)
    if tonumber(playerID) ~= localPlayer() then return; end
    local weight = WEIGHT.military;
    pcall(function()
        local unit = UnitManager.GetUnit(playerID, unitID);
        local row = GameInfo.Units[unit:GetType()];
        if row.FormationClass == "FORMATION_CLASS_CIVILIAN" then weight = WEIGHT.civilian; end
    end);
    CivvisMapView.Note(x, y, weight);
end

local function onCityOccupationChanged(playerID, cityID)
    pcall(function()
        local city = CityManager.GetCity(playerID, cityID);
        noteMine(playerID, city:GetX(), city:GetY(), WEIGHT.city);
    end);
end

if automated() then
    local handlers = {
        CombatVisBegin = onCombat,
        UnitMoved = onUnitMoved,
        CityAddedToMap = function(playerID, _, x, y) noteMine(playerID, x, y, WEIGHT.city); end,
        CityOccupationChanged = onCityOccupationChanged,
        DistrictAddedToMap = function(playerID, _, _, x, y) noteMine(playerID, x, y, WEIGHT.district); end,
        ImprovementAddedToMap = function(x, y, _, owner) noteMine(owner, x, y, WEIGHT.improvement); end,
        LoadGameViewStateDone = function()
            CivvisMapView.Reset(); quietNativeCamera(); CivvisMapView.Enforce();
        end,
        -- Before our queue drains: take in the rivals' turn, then hold.
        LocalPlayerTurnBegin = function()
            dirty = true; lastLookPulse = pulses - REASSERT_PULSES;
            quietNativeCamera();
            pcall(CivvisMapView.Follow, true);
            CivvisMapView.Enforce();
        end,
        Camera_Updated = CivvisMapView.Enforce,
        CombatVisEnd = CivvisMapView.Enforce,
    };
    for name, handler in pairs(handlers) do
        pcall(function() Events[name].Add(handler); end);
    end
    quietNativeCamera();
    CivvisMapView.Enforce();
end
