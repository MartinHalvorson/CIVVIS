-- Execute the shipped victory callback with a mocked host, including an export failure.
local file = assert(io.open("tools/civ6_control/mod/CivvisControlAgent.lua"));
local source = file:read("*a"); file:close();
local callback = assert(source:match("(local function onTeamVictory.-)\nlocal function onPlayerDefeat"));
for _, fail in ipairs({false, true}) do
    local rows, exported = {}, false;
    local env = setmetatable({
        pcall = pcall, tostring = tostring,
        try = function(fn, fallback) local ok, v = pcall(fn); if ok then return v; end return fallback; end,
        Game = {GetLocalPlayer = function() return 0; end, GetCurrentGameTurn = function() return 173; end},
        Players = {[0] = {GetTeam = function() return 0; end, GetScore = function() return 672; end}},
        GameInfo = {Victories = {[6] = {VictoryType = "VICTORY_DIPLOMATIC"}}},
        CivvisQueue = {resetTimescale = function() end},
        exportState = function(_, pid, turn, frame)
            assert(pid == 0 and turn == 173 and frame == 0);
            if fail then error("export unavailable"); end
            exported = true;
        end,
        emit = function(kind, row) rows[#rows + 1] = {kind, row}; end,
    }, {__index = _G});
    local chunk = assert(loadstring(callback .. "\nonTeamVictory(0, 6, 1);", "victory"));
    setfenv(chunk, env); chunk();
    assert(rows[#rows][1] == "victory" and rows[#rows][2].won == true);
    assert(exported == not fail);
    if fail then assert(rows[1][1] == "final_state_error"); end
end
print("victory state callback passed");
