// Reserved for handheld-specific overrides (Miyoo SD card layout, MinUI/Onion
// app folder conventions, framebuffer-only init quirks).
//
// For now, the linux_handheld build target reuses desktop.rs paths via mod.rs's
// cfg dispatch — handhelds run Linux, so XDG fallbacks usually work. This file
// becomes live in Half 2 of Session 1, when we cross-compile to
// armv7-unknown-linux-musleabihf and discover what the actual on-device paths
// are.
//
// Intentionally empty.
