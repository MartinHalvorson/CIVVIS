-- ★ DIAGNOSTIC (`tools/civ6_hotseat.py --probe`): a see-through popup where the
-- hotseat cover sits, to measure what a see-through wait would stand on.
--
-- It is queued exactly as the cover is (CivvisHotseatCover.lua: the PausePanel
-- slot, `AlwaysVisibleInQueue`), draws nothing, and consumes every message.
-- `LuaEvents.CivvisProbeGlass(up, own)` from CivvisHotseatProbe.lua raises and
-- lowers it. With `own` it moves the map itself the way WorldInput.lua does:
--   wheel   WorldInput.lua:1455-1465  GetWheel() * -(1/12000 * 6), UI.SetMapZoom(z, 0, 0)
--   drag    WorldInput.lua:520-526    UIManager:GetNormalizedMousePos(), UI.GetMapLookAtWorldTarget(),
--                                     UI.GetWorldFromNormalizedScreenPos_NoWrap(x, y)
--           WorldInput.lua:578        UI.DragMap(x, y, startWorldX, startWorldY, startFocusX, startFocusY)
--   keys    WorldInput.lua:309        UI.PanMap(panX, panY)
-- Without `own` it only consumes, which shows what gets past a consuming popup
-- (the engine's own camera actions, EdgeScroll.lua's strips).
-- Every message is counted by name and reported when the glass changes.
include("PopupPriorityLoader_", true);

local cfg = CivvisControlConfig or {};
local PREFIX = "CIVVISJSON ";
local RUN = tostring(cfg.RunTag or "unset");

CivvisGlass = { up = false, own = false, counts = {}, names = {}, drag = nil,
	held = {}, minX = nil, maxX = nil, minY = nil, maxY = nil, wheel = 0 };

function CivvisGlass.report(kind, text)
	pcall(function()
		Automation.Log(PREFIX .. string.format('{"kind":"%s","ctx":"glass","run":"%s",%s}', kind, RUN, text));
	end);
	pcall(function()
		Automation.Log(PREFIX .. string.format('{"kind":"probe_flush","ctx":"glass","run":"%s"}', RUN));
	end);
end
function CivvisGlass.tally()
	local c = CivvisGlass;
	local parts = {};
	for name, count in pairs(c.counts) do parts[#parts + 1] = name .. "=" .. count; end
	table.sort(parts);
	local function num(v) return type(v) == "number" and string.format("%.4f", v) or "null"; end
	c.report("probe_glass", string.format(
		'"up":%s,"own":%s,"messages":"%s","wheel":%s,"min_x":%s,"max_x":%s,"min_y":%s,"max_y":%s',
		tostring(c.up), tostring(c.own), table.concat(parts, " "), num(c.wheel),
		num(c.minX), num(c.maxX), num(c.minY), num(c.maxY)));
	c.counts, c.wheel, c.minX, c.maxX, c.minY, c.maxY = {}, 0, nil, nil, nil, nil;
end

-- Keys that pan (the shipped CameraPan actions' defaults), as { dx, dy }.
function CivvisGlass.panKeys()
	local keys = {};
	pcall(function()
		keys[Keys.VK_LEFT], keys[Keys.A] = { -1, 0 }, { -1, 0 };
		keys[Keys.VK_RIGHT], keys[Keys.D] = { 1, 0 }, { 1, 0 };
		keys[Keys.VK_UP], keys[Keys.W] = { 0, 1 }, { 0, 1 };
		keys[Keys.VK_DOWN], keys[Keys.S] = { 0, -1 }, { 0, -1 };
	end);
	return keys;
end
function CivvisGlass.pan()
	local x, y = 0, 0;
	for _, step in pairs(CivvisGlass.held) do x, y = x + step[1], y + step[2]; end
	pcall(function() UI.PanMap(math.max(-1, math.min(1, x)), math.max(-1, math.min(1, y))); end);
end

function CivvisGlass.onInput(input)
	local c = CivvisGlass;
	local msg = nil;
	pcall(function() msg = input:GetMessageType(); end);
	local name = c.names[msg] or ("msg" .. tostring(msg));
	c.counts[name] = (c.counts[name] or 0) + 1;
	local x, y = nil, nil;
	pcall(function() x, y = UIManager:GetNormalizedMousePos(); end);
	if type(x) == "number" and type(y) == "number" then
		c.minX, c.maxX = math.min(c.minX or x, x), math.max(c.maxX or x, x);
		c.minY, c.maxY = math.min(c.minY or y, y), math.max(c.maxY or y, y);
	end
	if name == "MouseWheel" then
		pcall(function() c.wheel = c.wheel + input:GetWheel(); end);
	end
	if not c.own then return true; end
	if name == "MouseWheel" then
		pcall(function()
			UI.SetMapZoom(UI.GetMapZoom() + input:GetWheel() * (-((1.0 / 12000.0) * 6.0)), 0.0, 0.0);
		end);
	elseif name == "LButtonDown" then
		pcall(function()
			local fx, fy = UI.GetMapLookAtWorldTarget();
			local wx, wy = UI.GetWorldFromNormalizedScreenPos_NoWrap(x, y);
			c.drag = { fx = fx, fy = fy, wx = wx, wy = wy };
		end);
	elseif name == "MouseMove" then
		local d = c.drag;
		if d ~= nil then
			pcall(function() UI.DragMap(x, y, d.wx, d.wy, d.fx, d.fy); end);
		end
	elseif name == "LButtonUp" then
		c.drag = nil;
	elseif name == "KeyDown" or name == "KeyUp" then
		local key = nil;
		pcall(function() key = input:GetKey(); end);
		local step = c.panKeys()[key];
		if step ~= nil then
			c.held[key] = name == "KeyDown" and step or nil;
			c.pan();
		end
	end
	return true;
end

function CivvisGlass.set(up, own)
	local c = CivvisGlass;
	if c.up then c.tally(); end
	c.own = own == true;
	c.drag, c.held = nil, {};
	if up and not c.up then
		local priority = nil;
		pcall(function() priority = PopupPriority.PausePanel or PopupPriority.Utmost; end);
		c.up = pcall(function()
			UIManager:QueuePopup(ContextPtr, priority, { AlwaysVisibleInQueue = true });
		end);
	elseif not up and c.up then
		pcall(function() UI.PanMap(0, 0); end);
		pcall(function() UIManager:DequeuePopup(ContextPtr); end);
		c.up = false;
	end
	c.report("probe_glass_set", string.format('"up":%s,"own":%s', tostring(c.up), tostring(c.own)));
end

function CivvisGlass.arm()
	-- A context added by a mod is shown unless it hides itself; this one is a
	-- popup, shown only while it is queued.
	pcall(function() ContextPtr:SetHide(true); end);
	if cfg.Hotseat ~= true or cfg.HotseatProbe == nil or cfg.HotseatProbe == false then return false; end
	pcall(function()
		for name, value in pairs(MouseEvents) do CivvisGlass.names[value] = name; end
		for name, value in pairs(KeyEvents) do CivvisGlass.names[value] = name; end
	end);
	pcall(function() LuaEvents.CivvisProbeGlass.Add(function(up, own) pcall(CivvisGlass.set, up, own); end); end);
	pcall(function()
		-- The struct form, as WorldInput.lua:4245: the wheel is only there.
		ContextPtr:SetInputHandler(function(input)
			if not CivvisGlass.up then return false; end
			local ok, consumed = pcall(CivvisGlass.onInput, input);
			return (not ok) or consumed;
		end, true);
	end);
	return true;
end

CivvisGlass.arm();
