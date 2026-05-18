Recursive shadowcasting for line-of-sight. Trees block; fire is a light source.

## Slice-1 spec
- New module `src/fov.rs`.
- Algorithm: recursive shadowcasting (standard for grid roguelikes; isolated from the renderer).
- Visible cells: full color.
- Explored-but-not-visible cells: dimmed 50%.
- Unexplored cells: black.
- Tree cells (`TerrainKind::TreeTrunk`) block sight (1-cell occlusion).

## Radii
- **Day base**: 20 tiles.
- **Night base**: 3 tiles.
- **Lit fire light source**: extends night vision radius to 8 for cells within 5 of any lit fire entity.

## Caching
- FOV cache invalidated when:
  - Player moves
  - Day→night or night→day transition
  - A fire is lit or extinguished
  - A tree is felled (tile-blocking change)
- Otherwise cached across frames.

## Renderer integration
The per-cell-diff renderer handles FOV cheaply: visibility/dim state is a `fg`/`bg` modifier on each cell. No new draw path; the diff renderer just sees the modified cell colors and pushes them.

## Out of scope (slice 2+)
- Demon/NPC perception cones (no hostiles in slice 1).
- Sound-bubble alert when attacking from outside FOV.
- Per-direction light blockers (windows, half-walls).

Source: existing `[[FOV and lighting]]` card; Holy Land design carried forward.
