local file = assert(io.open("tools/civ6_control/mod/CivvisControlAutoClose.lua"));
local source = file:read("*a"); file:close();
local block = assert(source:match("(%-%- ReplaceUIScript bypasses the expansion add%-ons.-)\n%-%- First contact"));
local completed, started, stockFinished, stockPlayed = 0, 0, 0, 0;
local callback, replayButton;
local env = setmetatable({
    NAME = "EndGameMenu", END_SCREENS = {EndGameMenu = true}, Styles = {},
    Mouse = {eLClick = 1},
    Controls = {
        Movie = {SetMovieFinishedCallback = function(_, fn) callback = fn; end},
        MovieFill = {IsVisible = function() return true; end},
        ReplayMovieButton = {RegisterCallback = function(_, _, fn) replayButton = fn; end},
    },
    OnReplayMovie = function() stockPlayed = stockPlayed + 1; end,
    OnMovieExitOrFinished = function() stockFinished = stockFinished + 1; end,
    report = function(kind)
        if kind == "victory_movie_finished" then completed = completed + 1; end
        if kind == "victory_movie_started" then started = started + 1; end
    end,
}, {__index = _G});
env.include = function(name)
    if name == "EndGameMenu_Expansion1" then env.OnHistoricMoments = function() end; end
    if name == "EndGameMenu_Expansion2" then
        env.Styles.VICTORY_DIPLOMATIC = {Movie = "XP2Victory_Diplomatic.bk2"};
    end
end;
local chunk = assert(loadstring(block, "native victory hooks")); setfenv(chunk, env); chunk();
assert(env.Styles.VICTORY_DIPLOMATIC.Movie == "XP2Victory_Diplomatic.bk2");
assert(callback and replayButton);
env.OnReplayMovie(); assert(stockPlayed == 1 and started == 1 and completed == 0);
callback(); assert(stockFinished == 1 and completed == 1);
replayButton(); assert(stockPlayed == 2 and started == 2);
assert(source:find('if END_SCREENS[NAME] then return; end', 1, true), "end screens must not reach the ordinary close ladder");
print("native victory movie hooks passed");
