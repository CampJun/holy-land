Polearm reach mechanic + bow/crossbow ranged with ammo, line-of-sight, and a targeting cursor. Ranged ships in v1 alongside melee.

## Polearm reach (melee)

Spear, lance, and gisarme/bill are reach-2 weapons: they can attack a target up to 2 tiles away (cardinal or diagonal, subject to LoS).

**No-reach penalty:** if you swing a reach-2 weapon at an adjacent (1-tile) target, you suffer a ~-30% damage modifier (matches CDDA's `mult_damage(0.7)` for polearm-not-reaching). You can still hit, but the weapon is awkward at point-blank.

**Reach-2 targeting:** the player initiates `Attack` and selects from any valid target in reach. AI does the same when wielding a polearm.

## Ranged weapons (Bow, Crossbow)

Both ship in v1.

### Bow
- Slow draw (medium move-cost per shot).
- Pulls one arrow from pack ammo per shot.
- Damage type: stab.
- Trains Bow proficiency under Ranged skill.

### Crossbow
- Very slow reload (heavy move-cost; reload is a separate verb).
- Holds one bolt loaded at a time. Player presses `Shoot` to fire (fast), then `Reload` to chamber another bolt (slow).
- Damage type: stab + bash (the bolt's impact has heft).
- Higher damage per shot than bow.
- Trains Crossbow proficiency under Ranged skill.

### Line of sight

Re-uses the existing FOV shadowcasting code (`recompute_fov` in main.rs). A shot requires the target to be in the attacker's current FOV. Cover (trees, walls, smoke from [[Survival - Ambient effects - Smoke particles and FOV]]) blocks shots.

Projectile travel is instant in v1 (no in-flight projectile entity), but the path is shown as a brief glyph trail for one frame for visual feedback.

### Ammo and quivers

- **Arrows** and **bolts** are stackable item kinds in the pack (e.g. `Arrow ×24`).
- **Quiver** (worn): increases ammo carry capacity / speeds reload. Optional gear; not required to shoot.
- **Wielded ranged weapon auto-pulls** from any compatible ammo in pack OR quiver. No need to "load" each shot manually (except the crossbow's explicit `Reload`).
- **Recovery:** arrows fired into the ground or stuck in corpses can be picked up. Each pickup has a break chance (placeholder 30%) — the arrow shaft snaps on impact sometimes.

### Targeting cursor (UI)

When the player initiates `Shoot`, a targeting cursor opens on a visible enemy. The HUD shows:

- **Visible to-hit %** computed from the contested-roll math (this is the only place hit% is shown to the player; melee remains hidden).
- **Range to target** in tiles.
- **Body-part submenu** for aimed shots (declare a target part with extra penalty).

Player commits with the action button; cancels with the back button (no move-cost spent on cancel).

## Stamina cost (light-touch)

Drawing a bow and reloading a crossbow consume modest stamina (`heavy action`); see [[Survival - Combat - Stamina]]. Normal sustained shots are not stamina-prohibitive.

## Visible to-hit % only for ranged

Per the brainstorm decision: melee is hidden-roll with descriptive log. Ranged shows hit%. Rationale: melee is reflexive; ranged is deliberate aiming.

## Open

- Whether throwing daggers / hand axes / stones ship as a v1 Ranged verb (currently deferred to a follow-up — but throwing is period-plausible and might be worth a small v1 stub).
- Exact reach-2 LoS rules (does a wall between the attacker and a reach-2 target block? — probably yes for now).
- Whether crit on a ranged shot can pierce cover (probably no in v1).
- Whether the targeting cursor allows shooting at empty tiles (e.g. for area effects later). v1: target must be an entity.

Source: combat refactor brainstorm 2026-05-21.
