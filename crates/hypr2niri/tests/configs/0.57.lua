-- Hyprland 0.55+ (and only this since 0.57): hyprland.lua.
local mod = "SUPER"
hl.monitor({ output = "DP-1", mode = "2560x1440@165", position = "0x0", scale = 1 })
hl.config({
    general = { gaps_in = 4, gaps_out = 8, border_size = 1 },
    decoration = { rounding = 6, shadow = { enabled = true, color = 0xee1a1a1a } },
    input = { kb_layout = "us", repeat_rate = 40, touchpad = { natural_scroll = true } },
})
hl.env("XCURSOR_THEME", "Bibata-Modern-Ice")
hl.window_rule({ name = "pip", match = { title = "^(Picture-in-Picture)$" }, float = true })
hl.layer_rule({ match = { namespace = "waybar" }, blur = true })
hl.gesture({ fingers = 3, direction = "horizontal", action = "workspace" })
hl.on("hyprland.start", function () hl.exec_cmd("waybar") end)
hl.bind(mod .. " + Return", hl.dsp.exec_cmd("kitty"))
hl.bind(mod .. " + C", hl.dsp.window.close())
hl.bind(mod .. " + SHIFT + 3", hl.dsp.window.move({ workspace = 3, silent = true }))
hl.bind(mod .. " + V", hl.dsp.window.float({ action = "toggle" }))
