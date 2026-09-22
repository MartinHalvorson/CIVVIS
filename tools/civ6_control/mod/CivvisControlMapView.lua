-- WorldInput.lua:1202-1206: larger GetMapZoom values zoom OUT.
-- CameraManager.lua:37-49: combat style 1 pans; style 2 also zooms.
-- Keep the native camera, but bound its distance during automated play.
CivvisMapView = {};
local cfg = CivvisControlConfig or {};
local MIN_ZOOM = 0.80;
local adjusting = false;
local reported = false;

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
        if zoom < MIN_ZOOM then
            UI.SetMapZoom(MIN_ZOOM, 0.0, 0.0);
            UI.SetRestoreMapZoom(MIN_ZOOM);
        end
        return zoom, UI.GetMapZoom();
    end);
    adjusting = false;
    if ok and type(before) == "number" and type(after) == "number"
            and (not reported or before < MIN_ZOOM) then
        reported = true;
        pcall(function()
            Automation.Log("CIVVISJSON " .. string.format(
                '{"kind":"map_view","run":"%s","before":%.3f,"zoom":%.3f,"minimum":%.2f,"verified":%s}',
                tostring(cfg.RunTag or "unset"), before, after, MIN_ZOOM,
                tostring(after >= MIN_ZOOM)));
        end);
    end
end

if cfg.Play ~= false and cfg.CivvisDecides then
    for _, name in ipairs({"Camera_Updated", "LoadGameViewStateDone",
            "LocalPlayerTurnBegin", "CombatVisEnd"}) do
        pcall(function() Events[name].Add(CivvisMapView.Enforce); end);
    end
    CivvisMapView.Enforce();
end
