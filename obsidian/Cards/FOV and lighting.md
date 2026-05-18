Line-of-sight culling and demon perception.

`AGENTS.md` § "What is NOT done yet" lists `FOV / lighting` as an outstanding item — no implementation exists today. Two related but separable jobs:

- **Field of view** — compute the set of cells visible from the player's position each turn (recursive shadowcasting is the standard pick for grid roguelikes). Renderer dims or hides cells outside FOV. Memory of explored-but-not-currently-visible cells displayed differently.
- **Demon perception** — hostile entities only chase when the player is in their FOV (or when the player attacks from outside FOV — sound-bubble cheat). Hooks into the existing chase AI added in slice step 2.

Render pipeline already does per-cell diff, so dimming/hiding cells is a `fg`/`bg` change on the affected cells — no new draw path. Keep the FOV cache invalidated only when the player moves or a tile blocking light is added/removed.

Source: `AGENTS.md` § What is NOT done yet.
