// Survival shell. Phase 2 gut: replaces the Holy Land oasis/wilderness
// regions, reeds, keeper, vendor, demons, shrine, and Holy-Land-specific
// inventory with the bare-minimum tile grid, ECS, and player movement.
// Subsequent phases add: chunked storage, per-cell item lists, needs/clock,
// FOV, command-menu actions, fire-making, etc. (See the Survival - * cards
// in obsidian/Cards/ and the master plan.)

use hecs::{Entity, World as Ecs};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tile {
    Floor,
    Wall,
}

pub struct TileMap {
    pub width: u32,
    pub height: u32,
    pub tiles: Vec<Tile>,
}

impl TileMap {
    pub fn tile_at(&self, wx: i64, wy: i64) -> Tile {
        if wx < 0 || wy < 0 || wx >= self.width as i64 || wy >= self.height as i64 {
            return Tile::Wall;
        }
        self.tiles[(wy as u32 * self.width + wx as u32) as usize]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct Renderable {
    pub glyph: u8,
    pub fg: [u8; 4],
    pub bg: [u8; 4],
}

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct Player;

pub struct World {
    pub map: TileMap,
    pub ecs: Ecs,
    pub player: Entity,
}

impl World {
    pub fn new(width: u32, height: u32) -> Self {
        let map = build_map(width, height);
        let mut ecs = Ecs::new();
        let spawn = Position {
            x: width as i32 / 2,
            y: height as i32 / 2,
        };
        let player = ecs.spawn((
            Player,
            spawn,
            Renderable {
                glyph: b'@',
                fg: [240, 232, 200, 255],
                bg: [20, 17, 13, 255],
            },
        ));
        Self { map, ecs, player }
    }

    pub fn tile_at(&self, wx: i64, wy: i64) -> Tile {
        self.map.tile_at(wx, wy)
    }

    pub fn player_pos(&self) -> Position {
        *self
            .ecs
            .get::<&Position>(self.player)
            .expect("player has Position")
    }

    pub fn set_player_pos(&mut self, p: Position) {
        *self
            .ecs
            .get::<&mut Position>(self.player)
            .expect("player has Position") = p;
    }

    pub fn try_move_player(&mut self, dx: i32, dy: i32) {
        let pos = self.player_pos();
        let nx = pos.x + dx;
        let ny = pos.y + dy;
        if matches!(self.tile_at(nx as i64, ny as i64), Tile::Floor) {
            self.set_player_pos(Position { x: nx, y: ny });
        }
    }
}

fn idx(w: u32, x: i32, y: i32) -> usize {
    (y as u32 * w + x as u32) as usize
}

fn build_map(width: u32, height: u32) -> TileMap {
    // Phase-2 placeholder world: open floor inside a wall perimeter. Phase
    // 3 swaps this for chunked storage; phase 11 replaces the contents with
    // the authored stream/pond skeleton + seeded forest population.
    let mut tiles = vec![Tile::Floor; (width * height) as usize];
    let w = width as i32;
    let h = height as i32;
    for x in 0..w {
        tiles[idx(width, x, 0)] = Tile::Wall;
        tiles[idx(width, x, h - 1)] = Tile::Wall;
    }
    for y in 0..h {
        tiles[idx(width, 0, y)] = Tile::Wall;
        tiles[idx(width, w - 1, y)] = Tile::Wall;
    }
    TileMap {
        width,
        height,
        tiles,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn player_spawns_at_center() {
        let world = World::new(40, 30);
        assert_eq!(world.player_pos(), Position { x: 20, y: 15 });
    }

    #[test]
    fn walls_block_movement() {
        let mut world = World::new(40, 30);
        world.set_player_pos(Position { x: 1, y: 1 });
        // Step left into the west wall: blocked.
        world.try_move_player(-1, 0);
        assert_eq!(world.player_pos(), Position { x: 1, y: 1 });
        // Step right onto floor: moves.
        world.try_move_player(1, 0);
        assert_eq!(world.player_pos(), Position { x: 2, y: 1 });
    }

    #[test]
    fn out_of_bounds_reads_as_wall() {
        let world = World::new(40, 30);
        assert_eq!(world.tile_at(-1, 5), Tile::Wall);
        assert_eq!(world.tile_at(40, 5), Tile::Wall);
        assert_eq!(world.tile_at(5, -1), Tile::Wall);
        assert_eq!(world.tile_at(5, 30), Tile::Wall);
    }
}
