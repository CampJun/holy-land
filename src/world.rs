use hecs::{Entity, World as Ecs};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tile {
    Floor,
    Wall,
    Portal,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Region {
    Oasis,
    Wilderness,
}

impl Region {
    pub fn save_key(self) -> &'static str {
        match self {
            Region::Oasis => "oasis",
            Region::Wilderness => "wilderness",
        }
    }

    pub fn from_save_key(k: &str) -> Option<Self> {
        match k {
            "oasis" => Some(Region::Oasis),
            "wilderness" => Some(Region::Wilderness),
            _ => None,
        }
    }
}

pub struct RegionMap {
    pub width: u32,
    pub height: u32,
    pub tiles: Vec<Tile>,
}

impl RegionMap {
    pub fn tile_at(&self, wx: i64, wy: i64) -> Tile {
        if wx < 0 || wy < 0 || wx >= self.width as i64 || wy >= self.height as i64 {
            return Tile::Wall;
        }
        self.tiles[(wy as u32 * self.width + wx as u32) as usize]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Item {
    Reed,
}

impl Item {
    pub fn name(self) -> &'static str {
        match self {
            Item::Reed => "Reed",
        }
    }

    pub fn glyph(self) -> u8 {
        match self {
            Item::Reed => b'"',
        }
    }

    fn save_key(self) -> &'static str {
        match self {
            Item::Reed => "reed",
        }
    }

    fn from_save_key(k: &str) -> Option<Self> {
        match k {
            "reed" => Some(Item::Reed),
            _ => None,
        }
    }
}

#[derive(Clone, Default, Debug)]
pub struct Inventory {
    stacks: Vec<(Item, u32)>,
}

impl Inventory {
    pub fn add(&mut self, item: Item, count: u32) {
        if count == 0 {
            return;
        }
        if let Some(stack) = self.stacks.iter_mut().find(|(i, _)| *i == item) {
            stack.1 += count;
        } else {
            self.stacks.push((item, count));
        }
    }

    pub fn remove(&mut self, item: Item, count: u32) -> bool {
        let Some(idx) = self.stacks.iter().position(|(i, _)| *i == item) else {
            return false;
        };
        if self.stacks[idx].1 < count {
            return false;
        }
        self.stacks[idx].1 -= count;
        if self.stacks[idx].1 == 0 {
            self.stacks.remove(idx);
        }
        true
    }

    pub fn count(&self, item: Item) -> u32 {
        self.stacks
            .iter()
            .find(|(i, _)| *i == item)
            .map(|(_, c)| *c)
            .unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.stacks.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (Item, u32)> + '_ {
        self.stacks.iter().copied()
    }

    pub fn to_save(&self) -> BTreeMap<String, u32> {
        self.stacks
            .iter()
            .map(|(i, c)| (i.save_key().to_string(), *c))
            .collect()
    }

    pub fn from_save(map: &BTreeMap<String, u32>) -> Self {
        let mut stacks = Vec::new();
        for (k, c) in map {
            if *c == 0 {
                continue;
            }
            if let Some(item) = Item::from_save_key(k) {
                stacks.push((item, *c));
            }
        }
        Self { stacks }
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

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct Keeper;

pub struct World {
    pub region: Region,
    pub oasis: RegionMap,
    pub wilderness: RegionMap,
    pub ecs: Ecs,
    pub player: Entity,
    pub keeper: Entity,
    reed_positions: Vec<Position>,
    harvested_reeds: Vec<Position>,
    pub inventory: Inventory,
    pub oasis_intro_complete: bool,
}

// Where the player lands when stepping through a portal in either direction.
// Portals themselves sit at the shared `PORTAL_Y` row; the player is placed one
// tile inside the destination region so they don't immediately bounce back.
const PORTAL_Y: i32 = 15;
const OASIS_PORTAL_X: i32 = 39;
const WILDERNESS_PORTAL_X: i32 = 0;

impl World {
    pub fn new(width: u32, height: u32) -> Self {
        let oasis = build_oasis_map(width, height);
        let wilderness = build_wilderness_map(width, height);

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
        let keeper = ecs.spawn((
            Keeper,
            Position { x: 5, y: 3 },
            Renderable {
                glyph: b'&',
                fg: [210, 170, 95, 255],
                bg: [20, 17, 13, 255],
            },
        ));
        let reed_positions = vec![
            Position { x: 8, y: 5 },
            Position { x: 9, y: 5 },
            Position { x: 10, y: 5 },
            Position { x: 8, y: 6 },
            Position { x: 10, y: 6 },
        ];

        Self {
            region: Region::Oasis,
            oasis,
            wilderness,
            ecs,
            player,
            keeper,
            reed_positions,
            harvested_reeds: Vec::new(),
            inventory: Inventory::default(),
            oasis_intro_complete: false,
        }
    }

    pub fn current_map(&self) -> &RegionMap {
        match self.region {
            Region::Oasis => &self.oasis,
            Region::Wilderness => &self.wilderness,
        }
    }

    pub fn tile_at(&self, wx: i64, wy: i64) -> Tile {
        self.current_map().tile_at(wx, wy)
    }

    pub fn set_region(&mut self, region: Region, player_pos: Position) {
        self.region = region;
        self.set_player_pos(player_pos);
    }

    pub fn tick_oasis(&mut self, _dt: std::time::Duration) {
        // Wall-clock NPC behavior lives here once schedules / reed-regrow land.
        // Currently nothing in the oasis advances on its own.
    }

    pub fn tick_wilderness(&mut self) {
        // Discrete world step after each player action. Demon AI / blessing
        // tick / run-scoped timers land here in later steps.
    }

    pub fn try_move_player(&mut self, dx: i32, dy: i32) {
        let pos = *self
            .ecs
            .get::<&Position>(self.player)
            .expect("player has Position");
        let nx = pos.x + dx;
        let ny = pos.y + dy;
        let target = self.tile_at(nx as i64, ny as i64);
        let passable = matches!(target, Tile::Floor | Tile::Portal);
        if !passable {
            return;
        }
        let blocked_by_keeper =
            self.region == Region::Oasis && self.is_keeper_at(nx, ny);
        if blocked_by_keeper {
            return;
        }
        if target == Tile::Portal {
            let (dest_region, dest_pos) = match self.region {
                Region::Oasis => (
                    Region::Wilderness,
                    Position {
                        x: WILDERNESS_PORTAL_X + 1,
                        y: PORTAL_Y,
                    },
                ),
                Region::Wilderness => (
                    Region::Oasis,
                    Position {
                        x: OASIS_PORTAL_X - 1,
                        y: PORTAL_Y,
                    },
                ),
            };
            self.set_region(dest_region, dest_pos);
            return;
        }
        *self
            .ecs
            .get::<&mut Position>(self.player)
            .expect("player has Position") = Position { x: nx, y: ny };
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

    pub fn keeper_pos(&self) -> Position {
        *self
            .ecs
            .get::<&Position>(self.keeper)
            .expect("keeper has Position")
    }

    pub fn reed_count(&self) -> u32 {
        self.inventory.count(Item::Reed)
    }

    pub fn consume_reeds(&mut self, n: u32) -> bool {
        self.inventory.remove(Item::Reed, n)
    }

    pub fn harvested_reeds(&self) -> Vec<[i32; 2]> {
        self.harvested_reeds.iter().map(|p| [p.x, p.y]).collect()
    }

    pub fn restore_oasis_state(
        &mut self,
        harvested_reeds: &[[i32; 2]],
        oasis_intro_complete: bool,
        inventory: Inventory,
    ) {
        self.harvested_reeds.clear();
        for [x, y] in harvested_reeds.iter().copied() {
            let p = Position { x, y };
            if self.reed_positions.contains(&p) && !self.harvested_reeds.contains(&p) {
                self.harvested_reeds.push(p);
            }
        }
        self.inventory = inventory;
        self.oasis_intro_complete = oasis_intro_complete;
    }

    pub fn is_unharvested_reed_at(&self, x: i32, y: i32) -> bool {
        let p = Position { x, y };
        self.reed_positions.contains(&p) && !self.harvested_reeds.contains(&p)
    }

    pub fn try_harvest_reed_near_player(&mut self) -> bool {
        if self.region != Region::Oasis {
            return false;
        }
        let player = self.player_pos();
        let Some(reed) = self
            .reed_positions
            .iter()
            .copied()
            .find(|p| !self.harvested_reeds.contains(p) && is_near(player, *p))
        else {
            return false;
        };
        self.harvested_reeds.push(reed);
        self.inventory.add(Item::Reed, 1);
        true
    }

    pub fn player_is_adjacent_to_keeper(&self) -> bool {
        self.region == Region::Oasis && is_adjacent(self.player_pos(), self.keeper_pos())
    }

    fn is_keeper_at(&self, x: i32, y: i32) -> bool {
        self.keeper_pos() == Position { x, y }
    }
}

fn idx(w: u32, x: i32, y: i32) -> usize {
    (y as u32 * w + x as u32) as usize
}

fn build_oasis_map(width: u32, height: u32) -> RegionMap {
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

    // Punch the east-edge portal to the wilderness.
    tiles[idx(width, OASIS_PORTAL_X, PORTAL_Y)] = Tile::Portal;

    RegionMap {
        width,
        height,
        tiles,
    }
}

fn build_wilderness_map(width: u32, height: u32) -> RegionMap {
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

    // West-edge portal back to the oasis.
    tiles[idx(width, WILDERNESS_PORTAL_X, PORTAL_Y)] = Tile::Portal;

    RegionMap {
        width,
        height,
        tiles,
    }
}

fn is_near(a: Position, b: Position) -> bool {
    (a.x - b.x).abs() <= 1 && (a.y - b.y).abs() <= 1
}

fn is_adjacent(a: Position, b: Position) -> bool {
    (a.x - b.x).abs() + (a.y - b.y).abs() == 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn harvests_nearby_reeds_once() {
        let mut world = World::new(40, 30);
        world.set_player_pos(Position { x: 8, y: 4 });

        assert!(world.try_harvest_reed_near_player());
        assert_eq!(world.reed_count(), 1);
        assert!(!world.is_unharvested_reed_at(8, 5));

        assert!(world.try_harvest_reed_near_player());
        assert_eq!(world.reed_count(), 2);
    }

    #[test]
    fn consume_reeds_drains_inventory() {
        let mut world = World::new(40, 30);
        world.set_player_pos(Position { x: 8, y: 4 });
        world.try_harvest_reed_near_player();
        world.try_harvest_reed_near_player();

        assert!(!world.consume_reeds(3));
        assert_eq!(world.reed_count(), 2);
        assert!(world.consume_reeds(2));
        assert_eq!(world.reed_count(), 0);
        assert!(world.inventory.is_empty());
    }

    #[test]
    fn restores_only_known_harvested_reeds() {
        let mut world = World::new(40, 30);
        let mut inv = Inventory::default();
        inv.add(Item::Reed, 2);
        world.restore_oasis_state(&[[8, 5], [99, 99], [8, 5]], true, inv);

        assert_eq!(world.reed_count(), 2);
        assert!(world.oasis_intro_complete);
        assert_eq!(world.harvested_reeds(), vec![[8, 5]]);
    }

    #[test]
    fn keeper_blocks_movement() {
        let mut world = World::new(40, 30);
        world.set_player_pos(Position { x: 4, y: 3 });
        world.try_move_player(1, 0);

        assert_eq!(world.player_pos(), Position { x: 4, y: 3 });
        assert!(world.player_is_adjacent_to_keeper());
    }

    #[test]
    fn portal_crosses_regions() {
        let mut world = World::new(40, 30);
        world.set_player_pos(Position {
            x: OASIS_PORTAL_X - 1,
            y: PORTAL_Y,
        });
        assert_eq!(world.region, Region::Oasis);

        // Step east onto the oasis portal -> wilderness, just inside.
        world.try_move_player(1, 0);
        assert_eq!(world.region, Region::Wilderness);
        assert_eq!(
            world.player_pos(),
            Position {
                x: WILDERNESS_PORTAL_X + 1,
                y: PORTAL_Y
            }
        );
        // Wilderness keeper interaction is gated by region.
        assert!(!world.player_is_adjacent_to_keeper());
        assert!(!world.try_harvest_reed_near_player());

        // Step west onto the wilderness portal -> oasis.
        world.try_move_player(-1, 0);
        assert_eq!(world.region, Region::Oasis);
        assert_eq!(
            world.player_pos(),
            Position {
                x: OASIS_PORTAL_X - 1,
                y: PORTAL_Y
            }
        );
    }
}
