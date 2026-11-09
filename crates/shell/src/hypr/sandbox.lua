-- Runs before a hyprland.lua so that `hl` only takes notes. What it notes are the lines a
-- hyprland.conf would have had, in __apolline_notes, for Apolline to read back. Nothing here
-- starts a program or writes a file.

local notes = {}
__apolline_notes = notes

-- The line of the config that called, past this file's own frames.
local function where()
  for level = 2, 40 do
    local info = debug.getinfo(level, "Sl")
    if not info then break end
    if info.source ~= "=apolline-sandbox" and info.what ~= "C" then
      local file = (info.short_src or "?"):match("([^/]+)$") or "?"
      return file .. ":" .. tostring(info.currentline or 0)
    end
  end
  return "hyprland.lua"
end

local function note(key, value)
  local entry = { key = key, value = value, at = where() }
  notes[#notes + 1] = entry
  return entry
end

-- Stands in for whatever a config asks of a library that isn't here: calling, indexing,
-- joining or comparing it all work and give nothing.
local anything = setmetatable({}, {})
local mt = getmetatable(anything)
mt.__index = function() return anything end
mt.__call = function() return anything end
mt.__concat = function() return "" end
mt.__tostring = function() return "" end
mt.__len = function() return 0 end
mt.__unm = function() return 0 end
for _, op in ipairs({ "add", "sub", "mul", "div", "mod", "pow", "idiv" }) do
  mt["__" .. op] = function() return 0 end
end

-- What a program could do outside the config: nothing.
os.execute = function() return nil, "not while importing", 0 end
os.exit = function() error("the config asked to exit", 0) end
os.remove = function() return nil, "not while importing" end
os.rename = function() return nil, "not while importing" end
os.tmpname = function() return "/dev/null" end
io.popen = function() return nil, "not while importing" end
io.write = function() return io end
io.output = function() return io end
local open = io.open
io.open = function(path, mode)
  if mode and mode:find("[wa+]") then
    return nil, "not while importing"
  end
  return open(path, mode)
end
package.loadlib = nil
package.cpath = ""
package.searchers = { package.searchers[1], package.searchers[2] }
package.path = __apolline_dir .. "/?.lua;" .. __apolline_dir .. "/?/init.lua;" .. package.path

-- A module that breaks here, one asking hyprctl say, gives way to `anything` and the rest goes on.
local real_require, real_dofile = require, dofile
require = function(name)
  local ok, result = pcall(real_require, name)
  if ok then return result end
  note("lua:skipped", "require " .. tostring(name) .. ": " .. tostring(result):match("[^\n]*"))
  package.loaded[name] = anything
  return anything
end
dofile = function(path)
  local ok, result = pcall(real_dofile, path)
  if ok then return result end
  note("lua:skipped", "dofile " .. tostring(path) .. ": " .. tostring(result):match("[^\n]*"))
  return anything
end

-- hl.dsp.window.close() is { dsp = "window.close", args = {} }.
local function dispatcher(path)
  return setmetatable({}, {
    __index = function(_, name)
      return dispatcher(path == "" and name or path .. "." .. name)
    end,
    __call = function(_, ...)
      return { dsp = path, args = { ... } }
    end,
  })
end

local ways = { left = "l", right = "r", up = "u", down = "d", l = "l", r = "r", u = "u", d = "d" }

-- What the same bind says in hyprland.conf: its dispatcher and parameters.
local function conf(action)
  if type(action) == "function" then
    return "lua:function", ""
  end
  if type(action) ~= "table" or not action.dsp then
    return "lua:unknown", ""
  end
  local a, o = action.args[1], action.args[1]
  if type(o) ~= "table" then o = {} end
  local d = action.dsp
  if d == "exec_cmd" or d == "exec" then return "exec", tostring(a or "") end
  if d == "exit" then return "exit", "" end
  if d == "window.close" then return "killactive", "" end
  if d == "window.kill" then return "forcekillactive", "" end
  if d == "window.pseudo" then return "pseudo", "" end
  if d == "window.pin" then return "pin", "" end
  if d == "window.center" then return "centerwindow", "" end
  if d == "window.float" then
    return (o.action == nil or o.action == "toggle") and "togglefloating" or "setfloating", ""
  end
  if d == "window.fullscreen" then
    return "fullscreen", (o.mode == "maximized" or o.mode == 1) and "1" or ""
  end
  if d == "window.move" then
    if o.workspace ~= nil then
      return o.silent and "movetoworkspacesilent" or "movetoworkspace", tostring(o.workspace)
    end
    if o.monitor ~= nil then return "movewindow", "mon:" .. tostring(o.monitor) end
    return "movewindow", ways[o.direction] or tostring(o.direction)
  end
  if d == "window.swap" then return "swapwindow", ways[o.direction] or tostring(o.direction) end
  if d == "window.resize" then
    if o.x or o.y then return "resizeactive", tostring(o.x or 0) .. " " .. tostring(o.y or 0) end
    return "resizewindow", ""
  end
  if d == "window.drag" then return "movewindow", "" end
  if d == "focus" then
    if o.workspace ~= nil then return "workspace", tostring(o.workspace) end
    if o.monitor ~= nil then return "focusmonitor", ways[o.monitor] or tostring(o.monitor) end
    if o.direction ~= nil then return "movefocus", ways[o.direction] or tostring(o.direction) end
    return "focuscurrentorlast", ""
  end
  if d == "workspace.toggle_special" then return "togglespecialworkspace", tostring(a or "") end
  if d == "layout" then
    return tostring(a) == "togglesplit" and "togglesplit" or "layoutmsg", tostring(a or "")
  end
  if d == "group.toggle" then return "togglegroup", "" end
  if d == "group.next" then return "changegroupactive", "f" end
  if d == "group.prev" then return "changegroupactive", "b" end
  if d == "dpms" then return "dpms", (o.action or tostring(a or "")) end
  return "lua:" .. d, ""
end

local function plain(value)
  if type(value) == "number" and math.type(value) == "float" then
    return string.format("%g", value)
  end
  return tostring(value)
end

-- { colors = { "rgba(…)", "rgba(…)" }, angle = 45 } reads as hyprland.conf writes it.
local function gradient(value)
  if type(value) == "table" and value.colors then
    local text = table.concat(value.colors, " ")
    return value.angle and (text .. " " .. value.angle .. "deg") or text
  end
  if type(value) == "number" then return string.format("0x%08x", value) end
  return plain(value)
end

local function flatten(prefix, table_, parent)
  for key, value in pairs(table_) do
    local name = prefix == "" and key or (parent == "col" and prefix .. "." .. key or prefix .. ":" .. key)
    if type(value) == "table" and not value.colors and #value == 0 then
      flatten(name, value, key)
    elseif parent == "col" then
      note(name, gradient(value))
    elseif type(value) == "table" then
      local items = {}
      for i, item in ipairs(value) do items[i] = plain(item) end
      note(name, table.concat(items, ","))
    else
      note(name, plain(value))
    end
  end
end

local function handle(entry)
  return setmetatable({}, {
    __index = function(_, name)
      if name == "set_enabled" then
        return function(_, on) entry.off = not on end
      end
      return function() return anything end
    end,
  })
end

hl = setmetatable({
  dsp = dispatcher(""),
  bind = function(keys, action, options)
    options = type(options) == "table" and options or {}
    local parts = {}
    for part in tostring(keys):gmatch("[^+]+") do
      parts[#parts + 1] = part:match("^%s*(.-)%s*$")
    end
    local key = table.remove(parts) or ""
    local flags = (options.mouse and "m" or "") .. (options.repeating and "e" or "")
      .. (options.locked and "l" or "") .. (options.release and "r" or "")
    local name, params = conf(action)
    return handle(note("bind" .. flags, table.concat(parts, " ") .. ", " .. key .. ", " .. name .. ", " .. params))
  end,
  monitor = function(t)
    t = type(t) == "table" and t or {}
    local fields = {
      t.output or "", t.mode or "preferred", t.position or "auto", plain(t.scale or "auto"),
    }
    if t.transform ~= nil then
      fields[#fields + 1] = "transform"
      fields[#fields + 1] = plain(t.transform)
    end
    if t.vrr ~= nil then
      fields[#fields + 1] = "vrr"
      fields[#fields + 1] = plain(t.vrr == true and 1 or t.vrr == false and 0 or t.vrr)
    end
    return handle(note("monitor", table.concat(fields, ", ")))
  end,
  config = function(t)
    if type(t) == "table" then flatten("", t, nil) end
  end,
  window_rule = function(t)
    t = type(t) == "table" and t or {}
    local match = type(t.match) == "table" and t.match or {}
    local target = match.class and ("class:" .. tostring(match.class))
      or match.title and ("title:" .. tostring(match.title)) or "class:.*"
    local entries = {}
    for effect, value in pairs(t) do
      if effect ~= "name" and effect ~= "match" then
        local said = value == true and effect or (effect .. " " .. plain(value))
        entries[#entries + 1] = note("windowrulev2", said .. ", " .. target)
      end
    end
    return handle(entries[1] or {})
  end,
  exec_cmd = function(command)
    note("exec-once", tostring(command))
  end,
  on = function(event, callback)
    if event == "hyprland.start" and type(callback) == "function" then
      local ok, err = pcall(callback)
      if not ok then note("lua:skipped", "hyprland.start: " .. tostring(err)) end
    end
    return anything
  end,
}, {
  __index = function() return function() return anything end end,
})
