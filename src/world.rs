use hecs::{Entity, World as Ecs};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tile {
    Floor,
    Wall,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
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
    pub width: u32,
    pub height: u32,
    pub tiles: Vec<Tile>,
    pub ecs: Ecs,
    pub player: Entity,
}

impl World {
    pub fn new(width: u32, height: u32) -> Self {
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
        let mid_y = h / 2;
        for x in (w / 4)..(3 * w / 4) {
            if x % 4 != 0 {
                tiles[idx(width, x, mid_y)] = Tile::Wall;
            }
        }

        let mut ecs = Ecs::new();
        let player = ecs.spawn((
            Player,
            Position { x: 2, y: 2 },
            Renderable {
                glyph: b'@',
                fg: [240, 232, 200, 255],
                bg: [20, 17, 13, 255],
            },
        ));

        Self {
            width,
            height,
            tiles,
            ecs,
            player,
        }
    }

    pub fn tile(&self, x: i32, y: i32) -> Tile {
        if x < 0 || y < 0 || x >= self.width as i32 || y >= self.height as i32 {
            return Tile::Wall;
        }
        self.tiles[idx(self.width, x, y)]
    }

    pub fn try_move_player(&mut self, dx: i32, dy: i32) {
        let mut pos = *self
            .ecs
            .get::<&Position>(self.player)
            .expect("player has Position");
        let nx = pos.x + dx;
        let ny = pos.y + dy;
        if self.tile(nx, ny) == Tile::Floor {
            pos = Position { x: nx, y: ny };
            *self
                .ecs
                .get::<&mut Position>(self.player)
                .expect("player has Position") = pos;
        }
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
}

fn idx(w: u32, x: i32, y: i32) -> usize {
    (y as u32 * w + x as u32) as usize
}
