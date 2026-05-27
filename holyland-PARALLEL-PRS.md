# Parallel-PR plan — splitting the Drafts column into two streams

Two agents work simultaneously. File overlap is concentrated on **additive seams** (enum variants, new fields, new match arms), not overlapping edits to the same function bodies.

Source of truth for card scope: `obsidian/Survival.md` Drafts column. This file only describes the *split*.

---

## PR A — "Combat finish-out"

`src/combat.rs` heavy. Closes the open ends of the combat cluster.

### Cards
- **Combat - Stamina** — new pool, regen, heavy-action cost table.
- **Combat - Reach and ranged** — ranged bow/crossbow side. Melee reach already shipped phase 5.
- **Combat - Status armament tiers** — Rabble + Sergeant + Knight loadout tables. Yeoman already shipped phase 3+4.
- **Combat - Weapon skills and proficiencies** — 11 weapon profs + Melee/Ranged/Dodge/Block/3 armor skills.
- **Combat - Skill XP sources** — per-skill XP routing + weapon-prof XP. **Defers the attribute-training half** until PR B's Attributes land.
- **Combat - Mythic and faerie arms** — scaffold only (modifier flags on `ItemMetadata`).
- **Combat - Bestiary slice 1 closeout** — tier variants using new tables.
- **Combat - Damage math**, **Combat - Armor model** — already shipped via phases 1+2+3+5; surface any remaining holes only.

### File ownership
- `src/combat.rs` (rewrite/extend) — exclusive.
- `src/items.rs` — weapons/ammo/armor `ItemDef` rows.
- `src/world.rs` **combat halves only**: `tick_hostiles`, `try_move_player` reach branch, `Wielded`/`OffHand`/`Worn`/`Equipment`, `CombatSkills`, new stamina pool, hostile spawn + loadout-roll.

### Suggested intra-PR order
Stamina pool → Weapon skills + proficiencies (data layer) → Skill XP sources (writes XP) → Reach/ranged → Status tiers → Mythic scaffold → Bestiary tier variants.

---

## PR B — "Player systems + foundations"

`src/main.rs` + `src/world.rs` ergonomics + flora.

### Cards
- **Attributes - STR AGI CON INT SPIRIT** — new ECS component, Attributes info-hub tab, save plumbing.
- **Real world scale carry overload + stride modes** — depends on Attributes; same PR keeps the dep internal.
- **Drop item verb**
- **Wait / pass turn action**
- **Menu wrap around scrolling**
- **Plant lifecycle and seasonal foraging** — FallenLeaves render, mast drops, mushroom spawns, `PlantState` transitions, lazy chunk-load fast-forward.
- **Game time clock** — tent warmth +0.5/min precision (one-liner).
- **Miyoo 30 FPS target**
- **Drag mechanic**

### File ownership
- `src/main.rs` (input handlers, menus, info-hub tab list, HUD) — exclusive.
- `src/input.rs` + `src/action.rs` — new verbs (Drop, Wait, Drag).
- `src/world.rs` **non-combat halves**: `Attributes`, carry cap, `StrideMode`, `MOVE_COST_TILE`, needs/clock tick, chunk dawn-tick for flora.
- `src/flora.rs` — exclusive.
- `src/render.rs` — exclusive (FallenLeaves render-only path).

### Suggested intra-PR order
Attributes → Carry/Stride (uses STR) → Drop/Wait/Menu-wrap (independent UX) → Plant lifecycle → 30 FPS + tent warmth + Drag.

---

## Shared-file rules — the only places conflicts can happen

| Hot zone | Rule |
|---|---|
| `world.rs::spend_moves` | **PR B owns the signature** (adds stride-mode multiplier). PR A pays stamina via a separate `World::spend_stamina(cost)` call beside `spend_moves`, never inside it. |
| `world.rs::CombatSkills.str_bonus` / `agi_mod` | **PR A leaves them in place.** PR B (Attributes card) removes them post-merge and rewires `AttackerStats`/`DefenderStats` to read STR/AGI directly. |
| `world.rs::AttackerStats` / `DefenderStats` builder | PR A may grow it for stamina/weapon-prof reads. PR B's STR/AGI swap is a mechanical merge at the same builder. |
| `main.rs` info-hub tabs | **PR B inserts the Attributes tab** between Skills and the existing tabs. PR A adds prof-XP lines *inside* the existing Skills tab body — no new tab. |
| `main.rs` `ActionId` match arms / input handler | Both append-only. Trivial line-order merge. |
| `save.rs::RunSave` | Both add `Option<…> #[serde(default)]` fields. CBOR is field-named → no on-disk conflict. **No schema bump in either PR.** |
| `world.rs::tick_hostiles`, `try_move_player` combat branch | **PR A owns exclusively.** PR B's stride mode multiplies cost via `spend_moves` only. |
| `world.rs` chunk-dawn tick | **PR B owns** for flora lifecycle. PR A doesn't touch. |

---

## Risk verdict
The only true overlap is `world.rs::CombatSkills` + `AttackerStats`. With the "PR A doesn't touch the str/agi shim fields" rule, post-merge conflict shrinks to a single function body in PR B's Attributes commit. Everything else is enum-variant or struct-field additive.

Save schema: both PRs additive, **no version bump** in either.
