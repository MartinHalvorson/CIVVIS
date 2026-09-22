-- WorldInput.lua:1202-1206: larger GetMapZoom values zoom OUT.
-- CameraManager.lua:37-49: combat style 1 pans; style 2 also zooms.
-- Keep the native camera, but bound its distance during automated play.
CivvisMapView = {};
local cfg = CivvisControlConfig or {};
local MIN_ZOOM = 0.80;
-- Native float readback can land just below the requested decimal value.
local MIN_ACCEPTED_ZOOM = MIN_ZOOM - 0.001;
local retryBlocked = false;
local adjusting = false;
local lastVerified = nil;

function CivvisMapView.Enforce()
    if cfg.Play == false or not cfg.CivvisDecides or adjusting then return; end
    adjusting = true;
    local ok, before, after = pcall(function()
        -- Preserve disabled combat following; convert only the close-up mode.
        for _, key in ipairs({"LookAtPlayerTurnCombat", "LookAtPlayerOffTurnCombat"}) do
            pcall(function()
                if Options.GetUserOption("Gameplay", key) == 2 then
                    Options.SetUserOption("Gameplay", key, 1);
                end
            end);
        end
        local zoom = UI.GetMapZoom();
        if type(zoom) ~= "number" or zoom ~= zoom then return; end
        if zoom >= MIN_ACCEPTED_ZOOM then
            retryBlocked = false;
            return zoom, zoom;
        end
        -- A refused or asynchronous correction must not retry on every
        -- Camera_Updated event. The one-second HUD pulse re-arms it.
        if retryBlocked then return; end
        retryBlocked = true;
        UI.SetMapZoom(MIN_ZOOM, 0.0, 0.0);
        UI.SetRestoreMapZoom(MIN_ZOOM);
        local observed = UI.GetMapZoom();
        if type(observed) == "number" and observed >= MIN_ACCEPTED_ZOOM then
            retryBlocked = false;
        end
        return zoom, observed;
    end);
    adjusting = false;
    if ok and type(before) == "number" and type(after) == "number"
            and (lastVerified ~= (after >= MIN_ACCEPTED_ZOOM)
                or before < MIN_ACCEPTED_ZOOM) then
        lastVerified = after >= MIN_ACCEPTED_ZOOM;
        pcall(function()
            Automation.Log("CIVVISJSON " .. string.format(
                '{"kind":"map_view","run":"%s","before":%.6f,"zoom":%.6f,"minimum":%.2f,"verified":%s}',
                tostring(cfg.RunTag or "unset"), before, after, MIN_ZOOM,
                tostring(after >= MIN_ACCEPTED_ZOOM)));
        end);
    end
end

function CivvisMapView.Pulse()
    retryBlocked = false;
    CivvisMapView.Enforce();
end

if cfg.Play ~= false and cfg.CivvisDecides then
    for _, name in ipairs({"Camera_Updated", "LoadGameViewStateDone",
            "LocalPlayerTurnBegin", "CombatVisEnd"}) do
        pcall(function() Events[name].Add(CivvisMapView.Enforce); end);
    end
    CivvisMapView.Enforce();
end
