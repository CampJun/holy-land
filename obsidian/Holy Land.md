---

kanban-plugin: board

---

## Ideas

- [ ] [[Android build]]
	Session 2 — aarch64-linux-android target, cargo-ndk, SDL2 asset manager, lifecycle hooks. Deferred until Miyoo path is real-device-verified.
- [ ] [[Tile-mode sprite override]]
	Optional 16×16 PNG sprites layered on top of CP437. Opt-in per glyph; ASCII remains canonical.
- [ ] [[NPC schedules and relationships]]
	Stardew-shaped: daily routines, gift preferences, friendship levels in the oasis.
- [ ] [[Analog-stick action wheels]]
	Tier 2 input — radial action wheels for stick-equipped devices. Every wheel action also reachable via menu on SNES-class devices.
- [ ] [[Crafting system]]
	Recipes as data; workbench tiles in oasis. Spend salvaged materials to craft items.
- [ ] [[Oasis expansion and building]]
	Spend defeated-demon currency to unlock new buildings and expand the oasis grid.
- [ ] [[Cloud sync integration]]
	Steam Cloud / Google Play Saves / manual export-import. Architecture already supports last-write-wins by `device_id` + `save_counter`.
- [ ] [[Settings and accessibility menu]]
	Gamepad-navigable: audio mixing, colorblind mode, rebinding, difficulty. Ship-blocking eventually but lower priority now.
- [ ] [[Wasteland ruins breadth]]
	3–4 hand-authored or procgen ruin biomes (Egyptian tomb, Babylonian temple, Assyrian palace, Canaanite shrine) tied to meta-progression unlocks.
- [ ] [[Procedural wilderness generation]]
	Seed-based generation beyond hand-authored zones. Integrates with [[Chunk-based world persistence]].
- [ ] [[Item variety and salvage tables]]
	Expand weapons, armor, consumables, demon-specific drops. Each biome gets a distinct item aesthetic.
- [ ] [[Meta-progression scaling]]
	Tune xp curves, demon-currency drops, unlock gating so new runs feel distinct.
- [ ] [[Oasis daily cycle]]
	In-game day/night clock driving NPC schedules, regrow timers, event triggers.
- [ ] [[Late input sampling]]
	Re-poll events just before render to shave a frame of input lag. Polish; not needed until real-time moments arrive.
- [ ] [[16-god roster design]]
	Follow-up plan: draft the 16 gods, their powers (one each, three cost tiers), and their level-1 blessings. Replaces the old Deity roster expansion card.
- [ ] [[Mana pool scaling]]
	Revisit the flat-100 mana pool once caster builds matter. Candidate scaling stats: Spirit, Intellect, both.
- [ ] [[Skill XP sources]]
	Author the per-skill activity-to-XP rules (RS-style: cook food = Cooking XP, swing sword = Attack/Strength XP, etc.).


## Planning

- [ ] [[Miyoo cross-compile resume]]
	Finish Session 1/H2 — musl ARM toolchain, static SDL2, package and test on real hardware.
- [ ] [[FOV and lighting]]
	Line-of-sight culling and demon perception based on player visibility. Listed under AGENTS "NOT done yet".
- [ ] [[Audio system architecture]]
	cpal output + kira mixer + libxmp tracker behind a `MusicEngine` trait. Library choices already locked; implementation slated for Session 5–6.
- [ ] [[Chunk-based world persistence]]
	Widen `Position` to i64, 32×32 chunks, LRU cache, seed-based gen. Save schema bump v1→v2.
- [ ] [[Cultural ruin biomes]]
	Egyptian / Babylonian / Assyrian / Canaanite palette swaps, glyph remixes, distinct enemy + item rosters.
- [ ] [[Save schema migration testing]]
	Round-trip + future-version-rejection tests for the first real schema bump. Validate the chain before it ships.
- [ ] [[Combat math foundation]]
	5 attributes (Stam/Str/Agi/Int/Spi), secondary stats, damage formula `weapon_base + Might × ratio − armor`, hit/dodge/crit/armor math.
- [ ] [[Action point system]]
	100 AP/turn economy, per-weapon swing costs, move cost + Speed scaling, careful-step, adjacency penalty.
- [ ] [[Power system]]
	4 power slots, modifier+dpad activation flow, auto-target → cycle → free-aim. Three cost tiers (light/medium/heavy), mana, cooldowns.
- [ ] [[Blessing system]]
	One power + one multiplicatively-stacking blessing per god. Run-start free blessing roll across all 16 powers; late-game up to 4 starting blessings.
- [ ] [[Blessing combo and stacking]]
	Multiplicative compounding of blessings on a single power. Rewritten to match the new power+blessing model.
- [ ] [[Multi-demon expansion]]
	1–2 new grunt-tier demons with AI variation and biome-flavored loot. References the new tiered enemy schema.
- [ ] [[Skill system]]
	15 Runescape-shaped skills (1 combat + 2 crafting per attribute). Milestone-based skill→attribute conversion. Skills also reduce AP cost of related action.
- [ ] [[Enemy tiered stat blocks]]
	Grunt schema (HP/dmg/AP/move/abilities) vs. boss schema (full attributes + powers). In-striking-range alert UX.
- [ ] [[Combat HUD]]
	AP bar + target info + striking-range alert in default view. Power slots reveal only on modifier hold.
- [ ] [[Action enum overhaul]]
	Extend `src/input.rs::Action` for power selection, target cycle, free-aim, careful-step, brace — without painting out the future analog-stick wheel.


## Implemented



## Playtest

- [ ] [[Blessing balance pass]]
	Playtest blessing power; tune so no blessing is obviously dominant.
- [ ] [[Demon difficulty curve]]
	Playtest the regional ramp; tune HP, damage, AI chase speed.
- [ ] [[Run pacing and feel]]
	Playtest the oasis → blessing → wilderness → death rhythm.


## Completed




%% kanban:settings
```
{"kanban-plugin":"board","list-collapse":[false,false,false,false,false]}
```
%%
