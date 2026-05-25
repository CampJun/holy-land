// Cornwall + Devon authored overmap data. This module is the *structure*
// of the game world — coastline, biomes, rivers, roads, named sites.
// It is identical in every save and every seed. The world_seed only
// varies per-chunk wilderness contents (trees, debris, decorations);
// the polygons + polylines + site anchors live here as `pub const`
// tables and never change between runs.
//
// Why this lives as Rust consts (not assets/cornwall.cbor):
//   - No asset-loading code, no Miyoo packaging step, no runtime parse.
//   - Total payload ~5 KB binary; edits require recompile, which is
//     fine for a small fixed dataset.
//   - Deterministic by construction: the linker is the canonical source.
//
// Why NOT a procgen pipeline (the original Survival card sketch):
//   The card's noise → sink-fill → A* rivers → MST roads scales to a
//   ~64×64 region. The real Cornwall+Devon peninsula is ~3,275×1,750
//   chunks. MST road A* over 5.7 M cells would take ~30–90 minutes on
//   Miyoo per world-init. Authored polygons + per-chunk point-in-polygon
//   lookup is microseconds.
//
// Coordinate system: cell (0, 0) is Exeter. Positive x = east, positive
// y = south. WGS84 transform documented in `obsidian/Cornwall-World.md`
// §2.2 (1 cell = 5 ft = 1.524 m). Site coordinates below are copied
// from §2.3; polygon vertices are sketched from the centroid + bounding
// hints in §3.2.

use std::collections::HashSet;
use std::sync::OnceLock;

use crate::world::{ChunkCoord, CHUNK_H, CHUNK_W};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Biome {
    LowlandFarm,
    RiverValley,
    EstuaryMarsh,
    CoastCliff,
    CoastBeach,
    OakWoodland,
    BeechCombe,
    DartmoorGranite,
    BodminMoorGranite,
    ExmoorHeath,
    Sea,
    TownEdge,
    RuinHinterland,
}

/// Iteration order for tests + overmap-UI scan loops. Keep in sync with
/// the `Biome` enum variants.
pub const ALL_BIOMES: &[Biome] = &[
    Biome::LowlandFarm,
    Biome::RiverValley,
    Biome::EstuaryMarsh,
    Biome::CoastCliff,
    Biome::CoastBeach,
    Biome::OakWoodland,
    Biome::BeechCombe,
    Biome::DartmoorGranite,
    Biome::BodminMoorGranite,
    Biome::ExmoorHeath,
    Biome::Sea,
    Biome::TownEdge,
    Biome::RuinHinterland,
];

impl Biome {
    /// CP437 glyph for this biome on the overmap. Per Cornwall-World §5.3.
    pub fn overmap_glyph(self) -> u8 {
        match self {
            Biome::LowlandFarm => b'.',
            Biome::RiverValley => b'~',
            Biome::EstuaryMarsh => b':',
            Biome::CoastCliff => 0xB2,       // ▓
            Biome::CoastBeach => b',',
            Biome::OakWoodland => 0x05,      // ♣
            Biome::BeechCombe => 0x05,       // ♣ (distinguished by tint)
            Biome::DartmoorGranite => 0x1E,  // ▲
            Biome::BodminMoorGranite => 0x1E,
            Biome::ExmoorHeath => 0xF0,      // ≡
            Biome::Sea => 0xF7,              // ≈
            Biome::TownEdge => b'.',         // background only — site overlay wins
            Biome::RuinHinterland => b'.',   // background only — site overlay wins
        }
    }

    /// Foreground RGB color for the biome glyph on the overmap.
    /// Per Cornwall-World §5.3.
    pub fn overmap_fg(self) -> [u8; 3] {
        match self {
            Biome::LowlandFarm => [90, 130, 70],         // dim green
            Biome::RiverValley => [120, 170, 220],       // bright blue
            Biome::EstuaryMarsh => [70, 100, 130],       // dim blue
            Biome::CoastCliff => [110, 110, 110],        // dark gray
            Biome::CoastBeach => [210, 200, 120],        // yellow
            Biome::OakWoodland => [50, 110, 40],         // dark green
            Biome::BeechCombe => [110, 170, 80],         // medium green
            Biome::DartmoorGranite => [130, 125, 125],   // dim gray
            Biome::BodminMoorGranite => [130, 125, 125],
            Biome::ExmoorHeath => [140, 90, 150],        // dim purple
            Biome::Sea => [40, 80, 140],                 // deep blue
            Biome::TownEdge => [110, 100, 80],           // dim — overlay shines
            Biome::RuinHinterland => [110, 100, 80],
        }
    }

    /// Human-readable label shown on the overmap's bottom info line.
    pub fn display_name(self) -> &'static str {
        match self {
            Biome::LowlandFarm => "Lowland farm",
            Biome::RiverValley => "River valley",
            Biome::EstuaryMarsh => "Estuary marsh",
            Biome::CoastCliff => "Coast cliff",
            Biome::CoastBeach => "Coast beach",
            Biome::OakWoodland => "Oak woodland",
            Biome::BeechCombe => "Beech combe",
            Biome::DartmoorGranite => "Dartmoor moor",
            Biome::BodminMoorGranite => "Bodmin moor",
            Biome::ExmoorHeath => "Exmoor heath",
            Biome::Sea => "Sea",
            Biome::TownEdge => "Town edge",
            Biome::RuinHinterland => "Ritual hinterland",
        }
    }

    /// Game-seconds it takes a fast-travelling pilgrim to cross one cell
    /// of this biome. Per Cornwall-World §1.2: ~2 s open ground, ~4 s
    /// moor/cliff/marsh. The `has_road` chunk flag overrides this to
    /// ~1.2 s in the fast-travel cost formula.
    pub fn cell_travel_cost_secs(self) -> u32 {
        match self {
            // Open lowland & woodland
            Biome::LowlandFarm => 2,
            Biome::BeechCombe => 3,
            Biome::OakWoodland => 3,
            Biome::TownEdge => 2,
            // River corridors — riparian, fords slow you
            Biome::RiverValley => 3,
            // Coast band
            Biome::CoastBeach => 3,
            // Moor / cliff / marsh — broken country
            Biome::DartmoorGranite => 4,
            Biome::BodminMoorGranite => 4,
            Biome::ExmoorHeath => 4,
            Biome::CoastCliff => 4,
            Biome::EstuaryMarsh => 4,
            // Ritual hinterlands — cropped turf, easy walking
            Biome::RuinHinterland => 2,
            // Sea is impassable on foot; cost never read but must be finite.
            Biome::Sea => 9_999,
        }
    }
}

/// Per-site type tag used to pick the overmap overlay glyph. Driven from
/// `Cornwall-World.md` §5.3.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SiteKind {
    Cathedral,
    Castle,
    Town,
    HolyWell,
    StoneCircle,
    SacredPool,
}

impl SiteKind {
    pub fn overmap_glyph(self) -> u8 {
        match self {
            SiteKind::Cathedral => 0xF0,    // ‡-ish (CP437 has no double-dagger; ≡ stands in)
            SiteKind::Castle => 0xD7,       // ╫
            SiteKind::Town => 0xFE,         // ■ (square / borough)
            SiteKind::HolyWell => b'o',     // ○
            SiteKind::StoneCircle => 0xEA,  // Ω
            SiteKind::SacredPool => b'O',   // ◯
        }
    }

    pub fn overmap_fg(self) -> [u8; 3] {
        match self {
            SiteKind::Cathedral => [240, 240, 240],   // bright white
            SiteKind::Castle => [240, 220, 110],      // bright yellow
            SiteKind::Town => [220, 200, 100],
            SiteKind::HolyWell => [120, 220, 220],    // bright cyan
            SiteKind::StoneCircle => [220, 130, 220], // bright magenta
            SiteKind::SacredPool => [140, 220, 220],
        }
    }
}

pub struct BiomePolygon {
    pub vertices: &'static [(i64, i64)],
    pub biome: Biome,
}

pub struct Polyline {
    pub points: &'static [(i64, i64)],
    /// Cells within this radius of the polyline are considered "on" it.
    /// Rivers are wide (~120 m); minor roads are narrow (~40 m).
    pub width_cells: u16,
}

pub struct NamedSite {
    pub name: &'static str,
    pub anchor_cell: (i64, i64),
    /// Chebyshev radius (in chunks) around the anchor chunk inside which
    /// `biome_override` wins. Exeter = 4 (9×9 cathedral city); towns = 2
    /// (5×5); ritual sites = 1 (3×3).
    pub anchor_radius_chunks: i32,
    pub biome_override: Biome,
    /// Drives the overmap overlay glyph at the site's anchor chunk.
    pub kind: SiteKind,
}

pub struct OvermapInfo {
    pub biome: Biome,
    pub has_river: bool,
    pub has_road: bool,
    /// Site this chunk anchors, if any. Surfaced for the overmap-UI
    /// follow-up card; chunkgen only reads `biome` today.
    #[allow(dead_code)]
    pub named_site: Option<&'static NamedSite>,
}

/// Player spawn cell inside the starting chunk (0, 0). Cell (20, 15) is
/// the chunk center; world coord (20, 15) lies ~30 m south-east of
/// Exeter's WGS84-anchored origin.
pub const EXETER_SPAWN_CELL: (i32, i32) = (CHUNK_W as i32 / 2, CHUNK_H as i32 / 2);

/// Cells from the peninsula outline beyond which a non-polygon land
/// chunk gets categorized as coast (cliff in the north, beach in the
/// south). ~530 m — about three chunks deep.
const COAST_BAND_CELLS: i64 = 350;

/// Peninsula outline (closed, clockwise). All cells outside this polygon
/// are `Sea`; cells inside fall through to the polygon / coast-band /
/// LowlandFarm cascade. Sketched to encompass every named site in
/// `Cornwall-World.md` §2.3 plus the inland biome polygon footprints.
pub const PENINSULA_OUTLINE: &[(i64, i64)] = &[
    // East / north edge — runs from inland Devon up to the Bristol Channel.
    (15_000, 2_000),
    (15_000, -33_000),
    (5_000, -33_500),
    (-5_000, -33_500),
    (-15_000, -33_000),
    (-25_000, -30_000),
    // North Cornwall coast — Hartland, Bude, Tintagel.
    (-40_000, -25_000),
    (-50_000, -18_000),
    (-55_000, -10_000),
    (-58_000, -2_000),
    (-65_000, 8_000),
    (-75_000, 15_000),
    (-90_000, 30_000),
    (-100_000, 38_000),
    // Penwith / Land's End.
    (-103_000, 42_500),
    (-100_000, 47_000),
    (-90_000, 48_500),
    // Lizard.
    (-80_000, 50_500),
    (-72_000, 47_000),
    // South coast — Falmouth, Fowey, Plymouth Sound, Dart.
    (-60_000, 38_000),
    (-50_000, 28_000),
    (-30_000, 27_500),
    (-15_000, 25_500),
    (0, 23_000),
    (8_000, 17_000),
    (12_000, 10_000),
];

pub const BIOME_POLYGONS: &[BiomePolygon] = &[
    BiomePolygon {
        // Dartmoor — granitic upland, ~30 km × 30 km centered (-20955, 10589).
        biome: Biome::DartmoorGranite,
        vertices: &[
            (-30_800, 10_589),
            (-25_000, 2_000),
            (-13_000, 2_000),
            (-8_000, 10_589),
            (-13_000, 19_000),
            (-25_000, 19_000),
        ],
    },
    BiomePolygon {
        // Bodmin Moor — granitic upland, ~20 km × 15 km centered (-51527, 9206).
        biome: Biome::BodminMoorGranite,
        vertices: &[
            (-58_088, 9_206),
            (-55_000, 4_285),
            (-48_000, 4_285),
            (-44_966, 9_206),
            (-48_000, 14_128),
            (-55_000, 14_128),
        ],
    },
    BiomePolygon {
        // Exmoor — royal forest, trimmed to fit inside the peninsula
        // outline (real Exmoor's coastal half lies outside our outline).
        biome: Biome::ExmoorHeath,
        vertices: &[
            (-20_300, -29_000),
            (-17_000, -32_500),
            (-3_908, -32_500),
            (-606, -29_000),
            (-3_908, -26_439),
            (-17_000, -26_439),
        ],
    },
    BiomePolygon {
        // Penwith ritual zone — Mên-an-Tol, Madron, Merry Maidens cluster.
        biome: Biome::RuinHinterland,
        vertices: &[
            (-101_500, 44_000),
            (-100_000, 40_000),
            (-96_500, 40_000),
            (-95_000, 44_000),
            (-96_500, 48_000),
            (-100_000, 48_000),
        ],
    },
    BiomePolygon {
        // Tamar river valley corridor — Bristol Channel down to Plymouth Sound.
        biome: Biome::RiverValley,
        vertices: &[
            (-23_500, -25_000),
            (-21_000, 0),
            (-25_000, 15_000),
            (-29_000, 27_000),
            (-32_000, 27_000),
            (-30_000, 15_000),
            (-27_000, 0),
            (-26_500, -25_000),
        ],
    },
    BiomePolygon {
        // Wistman's Wood / east Dartmoor fringe — OakWoodland anchor.
        biome: Biome::OakWoodland,
        vertices: &[
            (-18_000, 12_000),
            (-14_000, 9_000),
            (-10_000, 12_000),
            (-12_000, 16_000),
            (-16_000, 16_000),
        ],
    },
    BiomePolygon {
        // East Devon hedgerow vales — BeechCombe anchor (east of Exeter).
        biome: Biome::BeechCombe,
        vertices: &[
            (-2_000, -5_000),
            (8_000, -5_000),
            (12_000, 5_000),
            (8_000, 12_000),
            (-2_000, 12_000),
        ],
    },
    BiomePolygon {
        // Tamar estuary / Plymouth Sound — EstuaryMarsh ria belt.
        biome: Biome::EstuaryMarsh,
        vertices: &[
            (-31_000, 23_000),
            (-25_000, 23_000),
            (-25_000, 27_500),
            (-31_000, 27_500),
        ],
    },
];

pub const RIVERS: &[Polyline] = &[
    Polyline {
        // Tamar — Bristol Channel headwaters to Plymouth Sound.
        points: &[
            (-25_000, -25_000),
            (-24_500, -15_000),
            (-25_500, -5_000),
            (-26_500, 5_000),
            (-27_500, 15_000),
            (-28_000, 22_000),
            (-28_063, 26_029),
        ],
        width_cells: 30,
    },
    Polyline {
        // Fowey — Dozmary Pool source down to Fowey/Lostwithiel.
        points: &[
            (-50_130, 11_463),
            (-50_000, 17_000),
            (-52_000, 22_000),
            (-52_844, 22_912),
        ],
        width_cells: 28,
    },
    Polyline {
        // Camel — Bodmin Moor headwaters to Padstow.
        points: &[
            (-50_000, 9_206),
            (-55_000, 6_000),
            (-62_000, 6_500),
            (-65_000, 7_000),
        ],
        width_cells: 28,
    },
    Polyline {
        // Fal — central Cornwall down to Truro / Carrick Roads.
        points: &[
            (-65_000, 20_000),
            (-67_000, 25_000),
            (-70_000, 30_000),
            (-70_713, 33_442),
        ],
        width_cells: 28,
    },
    Polyline {
        // Lynher — Bodmin Moor across to the Tamar.
        points: &[
            (-49_000, 11_000),
            (-40_000, 17_000),
            (-32_000, 22_000),
            (-29_000, 26_000),
        ],
        width_cells: 25,
    },
    Polyline {
        // Helford — south Cornwall ria.
        points: &[(-78_000, 44_000), (-81_264, 45_183)],
        width_cells: 25,
    },
    Polyline {
        // River Exe — Exmoor source through Tiverton + Exeter to
        // Exmouth. The defining river of east Devon and the player's
        // immediate water source near the (0, 0) spawn.
        points: &[
            (-7_000, -32_000),
            (-3_000, -20_000),
            (5_000, -8_000),
            (0, 0),
            (4_000, 5_000),
            (8_000, 8_000),
        ],
        width_cells: 28,
    },
    Polyline {
        // River Dart — Dartmoor headwaters down through Totnes to
        // Dartmouth on the south Devon coast.
        points: &[
            (-20_000, 11_000),
            (-15_000, 16_000),
            (-7_272, 21_185),
            (-2_370, 27_122),
        ],
        width_cells: 28,
    },
    Polyline {
        // River Teign — east Dartmoor to Teignmouth.
        points: &[
            (-10_000, 11_000),
            (-5_000, 14_000),
            (2_000, 18_000),
        ],
        width_cells: 25,
    },
];

pub const ROADS: &[Polyline] = &[
    Polyline {
        // King's Highway: Exeter → Tavistock → Launceston → Bodmin → Truro → Penzance.
        points: &[
            (0, 0),
            (-10_000, 5_000),
            (-20_000, 8_000),
            (-28_610, 12_600),
            (-35_000, 10_000),
            (-38_653, 6_241),
            (-45_000, 10_000),
            (-50_000, 14_000),
            (-55_546, 18_266),
            (-62_000, 25_000),
            (-70_713, 33_442),
            (-80_000, 40_000),
            (-90_000, 42_000),
            (-95_129, 42_440),
        ],
        width_cells: 30,
    },
    Polyline {
        // South Devon road: Exeter → Totnes → Plymouth.
        points: &[
            (0, 0),
            (-3_000, 10_000),
            (-7_272, 21_185),
            (-15_000, 25_000),
            (-22_000, 26_000),
            (-28_063, 26_029),
        ],
        width_cells: 25,
    },
    Polyline {
        // Bodmin → Tintagel borough spur.
        points: &[
            (-55_546, 18_266),
            (-56_000, 12_000),
            (-57_190, 3_962),
        ],
        width_cells: 22,
    },
    Polyline {
        // Launceston → Lydford spur.
        points: &[
            (-38_653, 6_241),
            (-32_000, 6_000),
            (-26_754, 5_739),
        ],
        width_cells: 22,
    },
    Polyline {
        // Truro → Helston south road.
        points: &[
            (-70_713, 33_442),
            (-76_000, 38_000),
            (-81_264, 45_183),
        ],
        width_cells: 22,
    },
    Polyline {
        // Bodmin → Lostwithiel → Restormel south spur.
        points: &[
            (-55_546, 18_266),
            (-53_500, 19_500),
            (-52_844, 22_912),
            (-52_940, 20_089),
        ],
        width_cells: 22,
    },
];

/// Visible stream cells: a cell whose center lies within this many
/// cells (squared) of any river polyline gets `StreamWater`.
const STREAM_RADIUS_SQ: i64 = 4; // ~2 cell radius, 4–5 cell visible width

/// Visible road cells: a cell within this squared distance of any road
/// polyline gets `BareDirt`. Roads are narrower than streams.
const ROAD_RADIUS_SQ: i64 = 2; // ~1.4 cell radius, 2–3 cell visible width

/// True if the cell at world coords `(wx, wy)` overlaps a visible
/// river. Used by `chunkgen` to stamp `StreamWater` over the local
/// terrain on river-crossing chunks.
pub fn cell_on_river(wx: i64, wy: i64) -> bool {
    RIVERS
        .iter()
        .any(|pl| min_polyline_dist_sq((wx, wy), pl.points) <= STREAM_RADIUS_SQ)
}

/// True if the cell at world coords `(wx, wy)` overlaps a visible
/// beaten-earth road.
pub fn cell_on_road(wx: i64, wy: i64) -> bool {
    ROADS
        .iter()
        .any(|pl| min_polyline_dist_sq((wx, wy), pl.points) <= ROAD_RADIUS_SQ)
}

/// Direction vector `(dx, dy)` of the nearest road polyline segment to
/// the chunk at `cc`. Used by the overmap UI to draw road glyphs by
/// the segment's actual orientation instead of guessing from the
/// 4-neighbor pattern (which produces staircase loops on diagonal
/// roads). Returns None if no road polylines exist.
pub fn nearest_road_direction(cc: ChunkCoord) -> Option<(i64, i64)> {
    let center = chunk_center(cc);
    let mut best_dist_sq = i64::MAX;
    let mut best_dir: Option<(i64, i64)> = None;
    for road in ROADS {
        for win in road.points.windows(2) {
            let d = point_to_segment_dist_sq(center, win[0], win[1]);
            if d < best_dist_sq {
                best_dist_sq = d;
                best_dir = Some((win[1].0 - win[0].0, win[1].1 - win[0].1));
            }
        }
    }
    best_dir
}

pub const NAMED_SITES: &[NamedSite] = &[
    NamedSite { name: "Exeter",             anchor_cell: (0, 0),              anchor_radius_chunks: 4, biome_override: Biome::TownEdge,       kind: SiteKind::Cathedral },
    NamedSite { name: "Dartmouth",          anchor_cell: (-2_370, 27_122),    anchor_radius_chunks: 2, biome_override: Biome::TownEdge,       kind: SiteKind::Town },
    NamedSite { name: "Totnes",             anchor_cell: (-7_272, 21_185),    anchor_radius_chunks: 2, biome_override: Biome::TownEdge,       kind: SiteKind::Town },
    NamedSite { name: "Tavistock Abbey",    anchor_cell: (-28_610, 12_600),   anchor_radius_chunks: 2, biome_override: Biome::TownEdge,       kind: SiteKind::Cathedral },
    NamedSite { name: "Lydford Castle",     anchor_cell: (-26_754, 5_739),    anchor_radius_chunks: 2, biome_override: Biome::TownEdge,       kind: SiteKind::Castle },
    NamedSite { name: "Plymouth",           anchor_cell: (-28_063, 26_029),   anchor_radius_chunks: 2, biome_override: Biome::TownEdge,       kind: SiteKind::Town },
    NamedSite { name: "St Germans Priory",  anchor_cell: (-36_264, 23_698),   anchor_radius_chunks: 2, biome_override: Biome::TownEdge,       kind: SiteKind::Cathedral },
    NamedSite { name: "Launceston Castle",  anchor_cell: (-38_653, 6_241),    anchor_radius_chunks: 2, biome_override: Biome::TownEdge,       kind: SiteKind::Castle },
    NamedSite { name: "Liskeard",           anchor_cell: (-43_443, 19_694),   anchor_radius_chunks: 2, biome_override: Biome::TownEdge,       kind: SiteKind::Town },
    NamedSite { name: "The Hurlers",        anchor_cell: (-43_196, 15_016),   anchor_radius_chunks: 1, biome_override: Biome::RuinHinterland, kind: SiteKind::StoneCircle },
    NamedSite { name: "Rillaton Barrow",    anchor_cell: (-43_057, 14_660),   anchor_radius_chunks: 1, biome_override: Biome::RuinHinterland, kind: SiteKind::StoneCircle },
    NamedSite { name: "St Clether Well",    anchor_cell: (-49_437, 5_928),    anchor_radius_chunks: 1, biome_override: Biome::RuinHinterland, kind: SiteKind::HolyWell },
    NamedSite { name: "Dozmary Pool",       anchor_cell: (-50_130, 11_463),   anchor_radius_chunks: 1, biome_override: Biome::RuinHinterland, kind: SiteKind::SacredPool },
    NamedSite { name: "King Arthur's Hall", anchor_cell: (-53_851, 9_643),    anchor_radius_chunks: 1, biome_override: Biome::RuinHinterland, kind: SiteKind::StoneCircle },
    NamedSite { name: "St Nectan's Kieve",  anchor_cell: (-53_943, 3_088),    anchor_radius_chunks: 1, biome_override: Biome::RuinHinterland, kind: SiteKind::HolyWell },
    NamedSite { name: "Lostwithiel",        anchor_cell: (-52_844, 22_912),   anchor_radius_chunks: 2, biome_override: Biome::TownEdge,       kind: SiteKind::Town },
    NamedSite { name: "Restormel Castle",   anchor_cell: (-52_940, 20_089),   anchor_radius_chunks: 2, biome_override: Biome::TownEdge,       kind: SiteKind::Castle },
    NamedSite { name: "Bodmin",             anchor_cell: (-55_546, 18_266),   anchor_radius_chunks: 2, biome_override: Biome::TownEdge,       kind: SiteKind::Cathedral },
    NamedSite { name: "Tintagel Castle",    anchor_cell: (-57_190, 3_962),    anchor_radius_chunks: 2, biome_override: Biome::TownEdge,       kind: SiteKind::Castle },
    NamedSite { name: "Merlin's Cave",      anchor_cell: (-57_104, 4_020),    anchor_radius_chunks: 1, biome_override: Biome::RuinHinterland, kind: SiteKind::SacredPool },
    NamedSite { name: "Truro",              anchor_cell: (-70_713, 33_442),   anchor_radius_chunks: 2, biome_override: Biome::TownEdge,       kind: SiteKind::Town },
    NamedSite { name: "Helston",            anchor_cell: (-81_264, 45_183),   anchor_radius_chunks: 2, biome_override: Biome::TownEdge,       kind: SiteKind::Town },
    NamedSite { name: "Loe Pool",           anchor_cell: (-82_379, 45_838),   anchor_radius_chunks: 1, biome_override: Biome::RuinHinterland, kind: SiteKind::SacredPool },
    NamedSite { name: "Madron Well",        anchor_cell: (-95_129, 42_440),   anchor_radius_chunks: 1, biome_override: Biome::RuinHinterland, kind: SiteKind::HolyWell },
    NamedSite { name: "Men-an-Tol",         anchor_cell: (-97_340, 42_126),   anchor_radius_chunks: 1, biome_override: Biome::RuinHinterland, kind: SiteKind::StoneCircle },
    NamedSite { name: "Merry Maidens",      anchor_cell: (-97_851, 46_567),   anchor_radius_chunks: 1, biome_override: Biome::RuinHinterland, kind: SiteKind::StoneCircle },
];

/// World-coord center of `cc`. Used for point-in-polygon and distance-
/// to-polyline tests when classifying a chunk's biome.
pub fn chunk_center(cc: ChunkCoord) -> (i64, i64) {
    let x = cc.cx as i64 * CHUNK_W as i64 + CHUNK_W as i64 / 2;
    let y = cc.cy as i64 * CHUNK_H as i64 + CHUNK_H as i64 / 2;
    (x, y)
}

/// Cascade:
///   1. Named-site anchor radius → biome_override + named_site = Some.
///   2. Outside peninsula outline → Sea.
///   3. Inside a biome polygon → that biome (first match wins; polygons
///      shouldn't overlap meaningfully — Dartmoor vs Bodmin etc.).
///   4. Within COAST_BAND of the outline → CoastCliff (north) or
///      CoastBeach (south).
///   5. Otherwise → LowlandFarm.
///
/// `has_river` / `has_road` are independent — a road can cross any
/// biome, a river can run through Lowland. Named-site chunks still get
/// the road/river flags so the road into Bodmin is drawn through the
/// town edge.
pub fn overmap_info_at(cc: ChunkCoord) -> OvermapInfo {
    let center = chunk_center(cc);

    let named_site = NAMED_SITES.iter().find(|site| {
        let site_cc = chunk_for_cell(site.anchor_cell);
        let dx = (cc.cx - site_cc.cx).abs();
        let dy = (cc.cy - site_cc.cy).abs();
        dx <= site.anchor_radius_chunks && dy <= site.anchor_radius_chunks
    });

    let biome = if let Some(site) = named_site {
        site.biome_override
    } else if !point_in_polygon(center, PENINSULA_OUTLINE) {
        Biome::Sea
    } else {
        let mut b: Option<Biome> = None;
        for poly in BIOME_POLYGONS {
            if point_in_polygon(center, poly.vertices) {
                b = Some(poly.biome);
                break;
            }
        }
        b.unwrap_or_else(|| {
            let d_sq = min_polyline_dist_sq(center, PENINSULA_OUTLINE);
            if d_sq < COAST_BAND_CELLS * COAST_BAND_CELLS {
                // North/south split on y ~ Plymouth Sound latitude.
                // Above (y < 18000) = north Cornish cliff coast; below =
                // south estuary/beach coast.
                if center.1 < 18_000 {
                    Biome::CoastCliff
                } else {
                    Biome::CoastBeach
                }
            } else {
                Biome::LowlandFarm
            }
        })
    };

    // has_river / has_road look up a chunk in a precomputed
    // `HashSet<ChunkCoord>` — see `road_chunks()` / `river_chunks()`.
    // The set is the 4-connected rasterization of every polyline, with
    // L-shaped bridge chunks inserted at diagonal steps so the overmap
    // connector glyphs always have an orthogonal neighbor to attach to
    // (no broken staircase, no closed-loop corner cluster).
    let has_river = biome != Biome::Sea && river_chunks().contains(&cc);
    let has_road = biome != Biome::Sea && road_chunks().contains(&cc);

    OvermapInfo {
        biome,
        has_river,
        has_road,
        named_site,
    }
}

fn chunk_for_cell(cell: (i64, i64)) -> ChunkCoord {
    ChunkCoord {
        cx: cell.0.div_euclid(CHUNK_W as i64) as i32,
        cy: cell.1.div_euclid(CHUNK_H as i64) as i32,
    }
}

/// Chunk that a named site's anchor cell falls in. Exposed so the
/// overmap UI can render the site overlay glyph at exactly the anchor
/// chunk (rather than every chunk within the site's radius).
pub fn chunk_for_anchor(site: &NamedSite) -> ChunkCoord {
    chunk_for_cell(site.anchor_cell)
}

/// Precomputed 4-connected chunk trail for every road polyline. Lazily
/// computed on first overmap render (negligible startup cost — walking
/// every polyline 1 cell at a time across all of Cornwall is a few ms).
fn road_chunks() -> &'static HashSet<ChunkCoord> {
    static SET: OnceLock<HashSet<ChunkCoord>> = OnceLock::new();
    SET.get_or_init(|| rasterize_polylines(ROADS))
}

fn river_chunks() -> &'static HashSet<ChunkCoord> {
    static SET: OnceLock<HashSet<ChunkCoord>> = OnceLock::new();
    SET.get_or_init(|| rasterize_polylines(RIVERS))
}

fn rasterize_polylines(polylines: &[Polyline]) -> HashSet<ChunkCoord> {
    let mut out = HashSet::new();
    for pl in polylines {
        rasterize_polyline_into(&mut out, pl.points);
    }
    out
}

/// Walk each segment of `pts` in 1-cell steps, recording the chunk at
/// every position. At each diagonal chunk-to-chunk transition, insert
/// a horizontal-first L-bridge chunk so the resulting trail is
/// 4-connected — every road chunk has an orthogonal road neighbor on
/// each side of the trail, which lets the cardinal-neighbor connector
/// glyph picker draw a continuous staircase instead of disconnected
/// `─` / `│` stubs at diagonal steps.
fn rasterize_polyline_into(out: &mut HashSet<ChunkCoord>, pts: &[(i64, i64)]) {
    let mut last: Option<ChunkCoord> = None;
    for win in pts.windows(2) {
        let a = win[0];
        let b = win[1];
        let dx = (b.0 - a.0) as f64;
        let dy = (b.1 - a.1) as f64;
        let length = (dx * dx + dy * dy).sqrt();
        let steps = (length.ceil() as i64).max(1);
        for i in 0..=steps {
            let t = i as f64 / steps as f64;
            let x = (a.0 as f64 + t * dx).floor() as i64;
            let y = (a.1 as f64 + t * dy).floor() as i64;
            let cc = ChunkCoord {
                cx: x.div_euclid(CHUNK_W as i64) as i32,
                cy: y.div_euclid(CHUNK_H as i64) as i32,
            };
            if last == Some(cc) {
                continue;
            }
            if let Some(prev) = last {
                let dcx = cc.cx - prev.cx;
                let dcy = cc.cy - prev.cy;
                if dcx.abs() == 1 && dcy.abs() == 1 {
                    // Diagonal step — bridge with a horizontal-first
                    // L (i.e., the chunk at the new x and the old y).
                    // Pick a consistent direction so the trail stays
                    // a clean staircase, not a zigzag.
                    out.insert(ChunkCoord {
                        cx: cc.cx,
                        cy: prev.cy,
                    });
                }
            }
            out.insert(cc);
            last = Some(cc);
        }
    }
}

/// True if any segment of the polyline passes through (or starts/ends
/// inside) chunk `cc`'s bounding box. Kept for tests and potential
/// future use; the live has_road/has_river path now uses
/// `road_chunks()` / `river_chunks()` for O(1) lookups + bridged
/// diagonals.
#[allow(dead_code)]
fn polyline_crosses_chunk(cc: ChunkCoord, pts: &[(i64, i64)]) -> bool {
    let x_min = cc.cx as i64 * CHUNK_W as i64;
    let y_min = cc.cy as i64 * CHUNK_H as i64;
    let x_max = x_min + CHUNK_W as i64;
    let y_max = y_min + CHUNK_H as i64;
    for win in pts.windows(2) {
        if segment_intersects_rect(win[0], win[1], x_min, y_min, x_max, y_max) {
            return true;
        }
    }
    false
}

/// Liang-Barsky segment-rectangle clipping. Returns true if any part
/// of the segment AB lies inside the closed rectangle
/// `[xmin..=xmax] × [ymin..=ymax]`.
fn segment_intersects_rect(
    a: (i64, i64),
    b: (i64, i64),
    xmin: i64,
    ymin: i64,
    xmax: i64,
    ymax: i64,
) -> bool {
    // Half-open ownership: each chunk owns its lower-left corner but
    // not the upper-right. Without this, a polyline endpoint that
    // lands exactly on a grid corner (e.g., every road that begins at
    // Exeter's (0, 0)) would flag all 4 chunks sharing that corner —
    // their connector glyphs then close into a `┌┐┴┘` loop.
    let in_rect = |p: (i64, i64)| p.0 >= xmin && p.0 < xmax && p.1 >= ymin && p.1 < ymax;
    if in_rect(a) || in_rect(b) {
        return true;
    }
    let dx = (b.0 - a.0) as f64;
    let dy = (b.1 - a.1) as f64;
    let mut t_enter = 0.0_f64;
    let mut t_exit = 1.0_f64;
    let clip = |p_num: f64, p_den: f64, t_enter: &mut f64, t_exit: &mut f64| -> bool {
        if p_den == 0.0 {
            // Segment parallel to this edge. The constraint reduces to
            // `0 ≤ q`; reject only when `q < 0` (line outside this
            // half-plane), else continue checking other edges.
            return p_num >= 0.0;
        }
        let t = p_num / p_den;
        if p_den < 0.0 {
            if t > *t_exit {
                return false;
            }
            if t > *t_enter {
                *t_enter = t;
            }
        } else {
            if t < *t_enter {
                return false;
            }
            if t < *t_exit {
                *t_exit = t;
            }
        }
        true
    };
    // Standard Liang-Barsky form: for each clip edge express the
    // half-plane constraint as `p * t ≤ q`, then call `clip(q, p, …)`.
    //   left:   x ≥ xmin  →  p = -dx,  q = a.0 - xmin
    //   right:  x ≤ xmax  →  p =  dx,  q = xmax - a.0
    //   bottom: y ≥ ymin  →  p = -dy,  q = a.1 - ymin
    //   top:    y ≤ ymax  →  p =  dy,  q = ymax - a.1
    if !clip(a.0 as f64 - xmin as f64, -dx, &mut t_enter, &mut t_exit) {
        return false;
    }
    if !clip(xmax as f64 - a.0 as f64, dx, &mut t_enter, &mut t_exit) {
        return false;
    }
    if !clip(a.1 as f64 - ymin as f64, -dy, &mut t_enter, &mut t_exit) {
        return false;
    }
    if !clip(ymax as f64 - a.1 as f64, dy, &mut t_enter, &mut t_exit) {
        return false;
    }
    // Strict inequality: a segment that only kisses a corner of the
    // rect (t_enter == t_exit) doesn't actually traverse the chunk's
    // interior. Without this the corner-tangent flags neighbors that
    // the polyline never really enters.
    t_enter < t_exit
}

/// Ray-casting point-in-polygon. The polygon is implied closed (the
/// last vertex connects back to the first). f64 internally to avoid
/// i64 multiplication overflow on peninsula-scale coords.
fn point_in_polygon(p: (i64, i64), poly: &[(i64, i64)]) -> bool {
    if poly.len() < 3 {
        return false;
    }
    let px = p.0 as f64;
    let py = p.1 as f64;
    let mut inside = false;
    let n = poly.len();
    let mut j = n - 1;
    for i in 0..n {
        let xi = poly[i].0 as f64;
        let yi = poly[i].1 as f64;
        let xj = poly[j].0 as f64;
        let yj = poly[j].1 as f64;
        let crosses = (yi > py) != (yj > py);
        if crosses {
            let x_intersect = xi + (py - yi) * (xj - xi) / (yj - yi);
            if px < x_intersect {
                inside = !inside;
            }
        }
        j = i;
    }
    inside
}

/// Minimum squared distance from p to any segment of the polyline.
fn min_polyline_dist_sq(p: (i64, i64), pts: &[(i64, i64)]) -> i64 {
    if pts.len() < 2 {
        if let Some(&q) = pts.first() {
            let dx = p.0 - q.0;
            let dy = p.1 - q.1;
            return dx * dx + dy * dy;
        }
        return i64::MAX;
    }
    let mut best = i64::MAX;
    for w in pts.windows(2) {
        let d = point_to_segment_dist_sq(p, w[0], w[1]);
        if d < best {
            best = d;
        }
    }
    best
}

/// Squared distance from p to the segment ab. f64 internally; result
/// cast back to i64 (segment-distance values stay well within i64).
fn point_to_segment_dist_sq(p: (i64, i64), a: (i64, i64), b: (i64, i64)) -> i64 {
    let ax = a.0 as f64;
    let ay = a.1 as f64;
    let bx = b.0 as f64;
    let by = b.1 as f64;
    let px = p.0 as f64;
    let py = p.1 as f64;
    let abx = bx - ax;
    let aby = by - ay;
    let ab_len_sq = abx * abx + aby * aby;
    let t = if ab_len_sq > 0.0 {
        ((px - ax) * abx + (py - ay) * aby) / ab_len_sq
    } else {
        0.0
    };
    let t = t.clamp(0.0, 1.0);
    let cx = ax + abx * t;
    let cy = ay + aby * t;
    let dx = px - cx;
    let dy = py - cy;
    (dx * dx + dy * dy) as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cc(cx: i32, cy: i32) -> ChunkCoord {
        ChunkCoord { cx, cy }
    }

    #[test]
    fn exeter_chunk_is_town_edge() {
        let info = overmap_info_at(cc(0, 0));
        assert_eq!(info.biome, Biome::TownEdge);
        assert!(info.named_site.is_some());
        assert_eq!(info.named_site.unwrap().name, "Exeter");
    }

    #[test]
    fn far_offshore_chunk_is_sea() {
        // South of Lizard by hundreds of km — definitely open ocean.
        let info = overmap_info_at(cc(-2_000, 4_000));
        assert_eq!(info.biome, Biome::Sea);
        assert!(!info.has_river);
        assert!(!info.has_road);
    }

    #[test]
    fn dartmoor_centroid_is_dartmoor_granite() {
        // Centroid (-20955, 10589) → chunk (-524, 353). Inside the
        // Dartmoor polygon, outside any named-site radius.
        let info = overmap_info_at(cc(-524, 353));
        assert_eq!(info.biome, Biome::DartmoorGranite);
    }

    #[test]
    fn bodmin_moor_centroid_is_bodmin_moor_granite() {
        // (-51527, 9206) is inside the Bodmin Moor polygon AND inside
        // King Arthur's Hall's 3×3 radius around (-53851, 9643).
        // Use a centroid cell *outside* the named-site radius to test
        // the polygon path: (-52000, 9206) → chunk (-1300, 307).
        // King Arthur's Hall site chunk is (-53851/40, 9643/30) =
        // (-1347, 321); Dozmary site chunk is (-50130/40, 11463/30) =
        // (-1254, 382). Chunk (-1300, 307) is 47 / 14 away from
        // Arthur's Hall (radius 1) and 46 / 75 away from Dozmary —
        // outside both, so the Bodmin polygon should win.
        let info = overmap_info_at(cc(-1_300, 307));
        assert_eq!(info.biome, Biome::BodminMoorGranite);
    }

    #[test]
    fn tintagel_chunk_is_town_edge() {
        // Tintagel anchor cell (-57190, 3962). div_euclid by (40, 30) →
        // chunk (-1430, 132).
        let info = overmap_info_at(cc(-1_430, 132));
        assert_eq!(info.biome, Biome::TownEdge);
        assert_eq!(info.named_site.unwrap().name, "Tintagel Castle");
    }

    #[test]
    fn segment_intersects_rect_handles_corners_and_diagonals() {
        // Endpoint inside the rect: accept.
        assert!(segment_intersects_rect((5, 5), (100, 100), 0, 0, 10, 10));
        // Both endpoints outside but segment crosses: accept.
        assert!(segment_intersects_rect((-5, 5), (15, 5), 0, 0, 10, 10));
        // Diagonal segment clipping a corner.
        assert!(segment_intersects_rect((-5, -5), (20, 20), 0, 0, 10, 10));
        // Both endpoints outside on the same side: reject.
        assert!(!segment_intersects_rect((20, 0), (30, 30), 0, 0, 10, 10));
        // Parallel to edge, outside.
        assert!(!segment_intersects_rect((0, 20), (10, 20), 0, 0, 10, 10));
        // Parallel to edge, inside.
        assert!(segment_intersects_rect((0, 5), (10, 5), 0, 0, 10, 10));
    }

    #[test]
    fn road_chunk_trail_is_4_connected() {
        // Walk the road chunk set in a Chebyshev-1 ball around chunk
        // (0, 0) and assert that every road chunk has at least one
        // road neighbor among its 4 cardinal directions — i.e., no
        // diagonally-isolated chunks. (Without bridging, slope-1:2
        // polylines like King's Highway leave chunks whose only road
        // neighbor is a diagonal, breaking the staircase.)
        let mut visited = 0;
        for dy in -10..=10 {
            for dx in -10..=10 {
                let here = cc(dx, dy);
                if !overmap_info_at(here).has_road {
                    continue;
                }
                visited += 1;
                let has_orth_neighbor = [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)]
                    .iter()
                    .any(|(ndx, ndy)| {
                        overmap_info_at(cc(here.cx + ndx, here.cy + ndy)).has_road
                    });
                assert!(
                    has_orth_neighbor,
                    "road chunk {:?} has no orthogonal road neighbor — trail breaks here",
                    here
                );
            }
        }
        assert!(visited > 5, "expected several road chunks in the sampled region");
    }

    #[test]
    fn polyline_endpoint_on_grid_corner_does_not_cluster_four_chunks() {
        // Regression for the "loops at Exeter" bug. Several road
        // polylines start exactly at world cell (0, 0), which is the
        // corner of chunks (0, 0), (-1, 0), (0, -1), (-1, -1). The
        // closed-interval bbox test flagged all four → connector
        // glyphs closed into a ┌┐┴┘ loop. Half-open in_rect + strict
        // Liang-Barsky makes only chunk (0, 0) own that corner.
        assert!(overmap_info_at(cc(0, 0)).has_road);
        assert!(
            !overmap_info_at(cc(-1, -1)).has_road,
            "chunk (-1, -1) only touches polyline at its NE corner — must NOT flag"
        );
        assert!(
            !overmap_info_at(cc(0, -1)).has_road,
            "chunk (0, -1) only touches polyline at its SE corner — must NOT flag"
        );
        // The chunk directly along the polyline's initial direction
        // (south-westward toward Tavistock) still flags.
        assert!(
            overmap_info_at(cc(-1, 0)).has_road,
            "chunk (-1, 0) is on the polyline's first segment — must flag"
        );
    }

    #[test]
    fn road_chunks_flag_along_kings_highway() {
        // First segment of the King's Highway goes from (0, 0) (Exeter)
        // toward (-10000, 5000). Chunks the segment actually crosses
        // should have has_road = true; nearby off-axis chunks should
        // NOT (the prior 30-cell band gave a 2-wide trail and looped
        // glyphs).
        let on_path = overmap_info_at(cc(0, 0));
        assert!(on_path.has_road, "Exeter origin chunk must be on a road");
        // Chunk a few rows north of the polyline at the same x — not
        // on the segment, must NOT flag has_road.
        let off_path = overmap_info_at(cc(-2, -5));
        assert!(
            !off_path.has_road,
            "off-polyline chunk at (-2, -5) should not flag has_road"
        );
    }

    #[test]
    fn road_corridor_flag_at_exeter() {
        // Exeter chunk (0, 0) sits on the King's Highway start. Sites
        // override biome but should still pick up has_road = true.
        let info = overmap_info_at(cc(0, 0));
        assert!(info.has_road, "Exeter must be on a road");
    }

    #[test]
    fn tamar_corridor_chunk_has_river() {
        // Tamar polyline passes near (-27500, 15000) → chunk (-688, 500).
        let info = overmap_info_at(cc(-688, 500));
        assert!(info.has_river, "Tamar corridor chunk must flag has_river");
    }

    #[test]
    fn point_in_polygon_basic() {
        let square: &[(i64, i64)] = &[(0, 0), (10, 0), (10, 10), (0, 10)];
        assert!(point_in_polygon((5, 5), square));
        assert!(!point_in_polygon((15, 5), square));
        assert!(!point_in_polygon((-1, 5), square));
    }

    #[test]
    fn every_biome_has_glyph_color_name_cost() {
        for b in ALL_BIOMES {
            assert!(b.overmap_glyph() != 0, "biome {:?} missing glyph", b);
            let fg = b.overmap_fg();
            assert!(
                fg != [0, 0, 0],
                "biome {:?} has black overmap_fg — unreadable on bg",
                b
            );
            assert!(
                !b.display_name().is_empty(),
                "biome {:?} missing display_name",
                b
            );
            // Sea cost is sentinel-large but finite; the rest are bounded.
            let cost = b.cell_travel_cost_secs();
            assert!(cost > 0, "biome {:?} has zero travel cost", b);
        }
    }

    #[test]
    fn every_named_site_has_kind_and_glyph() {
        for site in NAMED_SITES {
            assert!(site.kind.overmap_glyph() != 0, "{} missing glyph", site.name);
            assert!(!site.name.is_empty());
        }
    }

    #[test]
    fn site_kinds_have_distinct_glyphs() {
        // Each SiteKind variant should map to a unique CP437 byte so the
        // overmap actually visually distinguishes cathedral / castle / etc.
        let kinds = [
            SiteKind::Cathedral,
            SiteKind::Castle,
            SiteKind::Town,
            SiteKind::HolyWell,
            SiteKind::StoneCircle,
            SiteKind::SacredPool,
        ];
        let mut seen = std::collections::HashSet::new();
        for k in kinds {
            assert!(
                seen.insert(k.overmap_glyph()),
                "duplicate SiteKind glyph for {:?}",
                k
            );
        }
    }

    #[test]
    fn lookup_perf_smoke() {
        // 10_000 chunks should classify in well under 50 ms on desktop.
        let start = std::time::Instant::now();
        let mut acc: u64 = 0;
        for cx in -100..100 {
            for cy in -50..50 {
                let info = overmap_info_at(cc(cx, cy));
                acc = acc.wrapping_add(info.biome as u64);
            }
        }
        let elapsed = start.elapsed();
        assert!(acc < u64::MAX, "use acc so the call isn't optimized out");
        assert!(
            elapsed < std::time::Duration::from_millis(200),
            "overmap_info_at × 20k took {:?}",
            elapsed
        );
    }
}
