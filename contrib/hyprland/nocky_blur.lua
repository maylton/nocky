-- Nocky glass rule for Hyprland 0.55+ (Lua configuration).
-- Require this after broader opaque/no_blur rules so the Nocky exception wins.
hl.window_rule({
  name = "nocky-glass",
  match = { class = "io.github.maylton.Nocky" },
  no_blur = false,
  opaque = false,
  force_rgbx = false,
  xray = false,
})
