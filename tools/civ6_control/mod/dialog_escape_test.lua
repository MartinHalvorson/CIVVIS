-- Offline test for which generic in-game dialogs the autoclose shim may end.
--
-- ⚠ THE OLD RULE WAS A BUTTON COUNT AND ITS JUSTIFICATION NAMED THE WRONG
-- SCREEN. `InGamePopup` was armed only when a dialog had exactly one button,
-- because "raze or keep this city ... has two and asks everything". Raze/keep
-- is `RazeCity.lua` with its own `RazeCity.xml`; it is queued through
-- `UIManager:QueuePopup`, holds its own input handler, and never reaches
-- `PopupDialogInGame`. So the count was refusing a class it never protected,
-- and every shipped Ok/Cancel and Yes/No dialog — the ones written to be
-- declined — sat over the map until the desktop backstop or the watchdog.
--
-- `CivvisDialogDismissable` reads the data the dialog carries instead: the
-- `CommandString` that `PopupDialogInGame:AddCancelButton` writes and that the
-- shipped `InGamePopup.InputHandler` activates on Escape. This pins that rule
-- against option lists taken from the shipped callers, so a future edit cannot
-- quietly widen it to a forced choice.
--
-- Run: lua5.1 tools/civ6_control/mod/dialog_escape_test.lua

local here = arg[0]:match("(.*)/[^/]*$") or "."

local function stub()
	return setmetatable({}, {
		__index = function() return stub() end,
		__call = function() return stub() end,
		__newindex = function() end,
	})
end

local EXPORTS = { CivvisDialogDismissable = true }

setmetatable(_G, { __index = function(_, k)
	if EXPORTS[k] then return rawget(_G, k); end
	return stub();
end })

-- ⚠ `ContextPtr:GetID()` has to answer with a STRING. The shim formats the
-- screen's name into every event it writes, so a stub table there kills the
-- chunk at load for a reason that has nothing to do with the rule under test.
-- Naming the real context also exercises the arming branch this change edits.
rawset(_G, "ContextPtr", setmetatable({
	GetID = function() return "InGamePopup"; end,
}, { __index = function() return stub(); end }))

local chunk, err = loadfile(here .. "/CivvisControlAutoClose.lua")
assert(chunk, "could not load the autoclose shim: " .. tostring(err))

-- ⚠⚠ REPORT THE PCALL RESULT. A chunk that dies at load must fail this test
-- rather than pass it because the export happened first.
local ran, runtime_err = pcall(chunk)
assert(ran, "CivvisControlAutoClose.lua raised at chunk load: " .. tostring(runtime_err))

local dismissable = rawget(_G, "CivvisDialogDismissable")
assert(type(dismissable) == "function", "CivvisDialogDismissable is not exported")

local CANCEL = "_CMD_CANCEL";
local CONFIRM = "_CMD_CONFIRM";
local DEFAULT = "_CMD_DEFAULT";

local function button(command)
	return { Type = "Button", Content = "x", CommandString = command };
end

-- Every case is an option list of the shape a shipped caller builds. The
-- comment on each names the caller, so a disagreement is checkable against the
-- installed game rather than against this file's opinion.
local CASES = {
	{
		name = "UnitCaptured: AddDefaultButton only",
		options = { { Type = "Text", Content = "captured" }, button(DEFAULT) },
		dismissable = true,
		buttons = DEFAULT,
	},
	{
		name = "UnitPanel: ShowYesNoDialog on deleting a unit",
		options = { { Type = "Text", Content = "delete?" }, button(CONFIRM), button(CANCEL) },
		dismissable = true,
		buttons = CONFIRM .. "+" .. CANCEL,
	},
	{
		name = "WorldInput: ICBM launch confirmation",
		options = { { Type = "Title", Content = "launch" }, button(CONFIRM), button(CANCEL) },
		dismissable = true,
		buttons = CONFIRM .. "+" .. CANCEL,
	},
	{
		name = "GovernmentScreen: anarchy switch confirmation",
		options = { button(CONFIRM), button(CANCEL) },
		dismissable = true,
		buttons = CONFIRM .. "+" .. CANCEL,
	},
	-- ★ THE LINE. Two buttons and no cancel is a forced choice: whichever way
	-- Escape resolves it, it has chosen. Left exactly as it ships.
	{
		name = "a two-way choice with no decline path",
		options = { { Type = "Text", Content = "pick one" }, button(CONFIRM), button(CONFIRM) },
		dismissable = false,
		buttons = CONFIRM .. "+" .. CONFIRM,
	},
	{
		name = "three buttons, none of them a cancel",
		options = { button(CONFIRM), button(CONFIRM), button(CONFIRM) },
		dismissable = false,
		buttons = CONFIRM .. "+" .. CONFIRM .. "+" .. CONFIRM,
	},
	-- A dialog whose buttons carry no command string at all: one is still an
	-- acknowledgement, two are still a choice, and the census records neither
	-- name because there was none to record.
	{
		name = "one unnamed button",
		options = { { Type = "Button", Content = "ok" } },
		dismissable = true,
		buttons = "",
	},
	{
		name = "two unnamed buttons",
		options = { { Type = "Button", Content = "a" }, { Type = "Button", Content = "b" } },
		dismissable = false,
		buttons = "",
	},
	-- Non-button rows never count. A dialog of pure text with one button is an
	-- acknowledgement however many lines it carries.
	{
		name = "text rows do not count as buttons",
		options = {
			{ Type = "Text", Content = "one" },
			{ Type = "Text", Content = "two" },
			{ Type = "Count", Content = 5 },
			button(DEFAULT),
		},
		dismissable = true,
		buttons = DEFAULT,
	},
	-- The shipped generic fallback `InGamePopup.OnPopupOpen` builds when it is
	-- handed nothing: Accept plus Cancel. It is declinable, and it is the one
	-- dialog the game raises to say a caller passed no options at all.
	{
		name = "InGamePopup's own generic Are-You-Sure fallback",
		options = { { Type = "Text", Content = "are you sure" }, button(CONFIRM), button(CANCEL) },
		dismissable = true,
		buttons = CONFIRM .. "+" .. CANCEL,
	},
	{
		name = "no options at all",
		options = nil,
		dismissable = false,
		buttons = "",
	},
}

for _, case in ipairs(CASES) do
	local ok, buttons = dismissable(case.options);
	assert(ok == case.dismissable,
		case.name .. ": expected dismissable=" .. tostring(case.dismissable) ..
		", got " .. tostring(ok));
	assert(buttons == case.buttons,
		case.name .. ": expected buttons=\"" .. case.buttons ..
		"\", got \"" .. tostring(buttons) .. "\"");
end

-- The rule must never be satisfied by a cancel that is not a button. A row of
-- another type carrying the command string is not a control a person or an
-- Escape can activate.
local ok = dismissable({ { Type = "Text", CommandString = CANCEL }, button(CONFIRM), button(CONFIRM) });
assert(ok == false, "a CANCEL command on a non-Button row must not arm the dialog");

print("dialog escape: " .. tostring(#CASES + 1) .. " option lists check out");
