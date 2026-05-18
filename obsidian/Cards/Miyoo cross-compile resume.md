Finish Half 2 of Session 1: get the binary running on real Miyoo Mini Plus hardware.

Resume checklist (from `holyland-ROADMAP.md` § Session 1 — H2):
- `rustup target add armv7-unknown-linux-musleabihf` (already done in tarball builds).
- Fetch the musl ARM cross-toolchain from `musl.cc`: `armv7l-linux-musleabihf-cross.tgz`.
- Update `.cargo/config.toml` linker path or use `cross` (Docker-based).
- SDL2 `bundled+static-link` features should compile SDL2 from source against musl. Watch for `pkg-config` gotchas around `libudev-sys` (transitive via gilrs); may need to disable gilrs hot-plug or stub udev.
- Package: binary + `assets/` + launch shell script in a folder dropped on the Miyoo SD card under `App/HolyLand/`.
- Test: launch from device menu, confirm dpad walks the `@`, Start quits.

The current cross build (`./cross/build-onion.sh`) already runs inside `miyoomini-rust` and links against Onion's prebuilt SDL2 under `cross/onion-libs/` — a musl static-link path would be an alternative to the Onion-libs path, not a replacement. Decide which target lineage is canonical before resuming.

Source: `holyland-ROADMAP.md` § Session 1 (Resume H2 checklist); `AGENTS.md` § Deploy / iterate workflow.
