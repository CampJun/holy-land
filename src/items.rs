// Item identity, weights, metadata, and player pack. Phase 3 introduces this
// module from scratch; the previous Holy Land Inventory/Item enum lived in
// world.rs and was gutted in phase 2.
//
// Design notes (see Survival - ItemInstance and weights.md, Survival - Chunk
// and per-cell items.md):
// - `ItemKind` partitions into fungible (twig, stick, ...) and unique (axe,
//   waterskin, ...). Fungibles stack into a single `ItemInstance` with
//   `count >= 1`; uniques always have `count == 1` and carry per-instance
//   metadata.
// - Weight is per-unit (`weight_g_each`); total stack weight is `count *
//   weight_g_each`. Source of truth: `ItemInstance::total_weight_g`.
// - Save format uses stable string `save_key()`s for forward-compat. Unknown
//   keys on load are dropped silently (see `Pack::from_save`).

use crate::save::{ItemInstanceSave, ItemMetadataSave, PackSave};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ItemKind {
    Axe,
    Knife,
    Pack,
    Tent,
    Bedroll,
    CookingPan,
    Waterskin,
    FlintAndSteel,
    Herb,
    Twig,
    Stick,
    Firewood,
    GrassBlade,
    Stone,
    MossPatch,
    Mud,
    Ration,
}

impl ItemKind {
    pub fn save_key(self) -> &'static str {
        match self {
            ItemKind::Axe => "axe",
            ItemKind::Knife => "knife",
            ItemKind::Pack => "pack",
            ItemKind::Tent => "tent",
            ItemKind::Bedroll => "bedroll",
            ItemKind::CookingPan => "cooking_pan",
            ItemKind::Waterskin => "waterskin",
            ItemKind::FlintAndSteel => "flint_and_steel",
            ItemKind::Herb => "herb",
            ItemKind::Twig => "twig",
            ItemKind::Stick => "stick",
            ItemKind::Firewood => "firewood",
            ItemKind::GrassBlade => "grass_blade",
            ItemKind::Stone => "stone",
            ItemKind::MossPatch => "moss_patch",
            ItemKind::Mud => "mud",
            ItemKind::Ration => "ration",
        }
    }

    pub fn from_save_key(s: &str) -> Option<Self> {
        Some(match s {
            "axe" => ItemKind::Axe,
            "knife" => ItemKind::Knife,
            "pack" => ItemKind::Pack,
            "tent" => ItemKind::Tent,
            "bedroll" => ItemKind::Bedroll,
            "cooking_pan" => ItemKind::CookingPan,
            "waterskin" => ItemKind::Waterskin,
            "flint_and_steel" => ItemKind::FlintAndSteel,
            "herb" => ItemKind::Herb,
            "twig" => ItemKind::Twig,
            "stick" => ItemKind::Stick,
            "firewood" => ItemKind::Firewood,
            "grass_blade" => ItemKind::GrassBlade,
            "stone" => ItemKind::Stone,
            "moss_patch" => ItemKind::MossPatch,
            "mud" => ItemKind::Mud,
            "ration" => ItemKind::Ration,
            _ => return None,
        })
    }

    pub fn is_fungible(self) -> bool {
        matches!(
            self,
            ItemKind::Twig
                | ItemKind::Stick
                | ItemKind::Firewood
                | ItemKind::GrassBlade
                | ItemKind::Stone
                | ItemKind::MossPatch
                | ItemKind::Mud
                | ItemKind::Ration
        )
    }

    #[allow(dead_code)] // used by the command-menu (phase 7) and HUD inventory panel
    pub fn name(self) -> &'static str {
        match self {
            ItemKind::Axe => "axe",
            ItemKind::Knife => "knife",
            ItemKind::Pack => "pack",
            ItemKind::Tent => "tent",
            ItemKind::Bedroll => "bedroll",
            ItemKind::CookingPan => "cooking pan",
            ItemKind::Waterskin => "waterskin",
            ItemKind::FlintAndSteel => "flint and steel",
            ItemKind::Herb => "herb",
            ItemKind::Twig => "twig",
            ItemKind::Stick => "stick",
            ItemKind::Firewood => "firewood",
            ItemKind::GrassBlade => "grass blade",
            ItemKind::Stone => "stone",
            ItemKind::MossPatch => "moss patch",
            ItemKind::Mud => "mud",
            ItemKind::Ration => "ration",
        }
    }

    /// Glyph + RGB foreground color for ground rendering. Background uses the
    /// cell's terrain background so items sit "on" the floor visually.
    pub fn glyph_color(self) -> (u8, [u8; 3]) {
        match self {
            ItemKind::Twig => (b',', [200, 170, 110]),
            ItemKind::Stick => (b'/', [200, 170, 110]),
            ItemKind::Firewood => (b'=', [110, 80, 50]),
            ItemKind::GrassBlade => (b'"', [80, 160, 70]),
            ItemKind::Stone => (b'*', [150, 150, 150]),
            ItemKind::MossPatch => (b'%', [50, 100, 50]),
            ItemKind::Mud => (b'%', [110, 80, 50]),
            ItemKind::Ration => (b'%', [220, 200, 160]),
            ItemKind::Axe => (b'P', [180, 180, 200]),
            ItemKind::Knife => (b'-', [180, 180, 200]),
            ItemKind::Pack => (b'[', [130, 90, 50]),
            ItemKind::Waterskin => (b'u', [100, 140, 200]),
            ItemKind::FlintAndSteel => (b'!', [230, 140, 60]),
            ItemKind::Tent => (b'A', [200, 180, 140]),
            ItemKind::Bedroll => (b'=', [220, 200, 160]),
            ItemKind::CookingPan => (b'O', [80, 80, 90]),
            ItemKind::Herb => (b'*', [80, 160, 70]),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ItemMetadata {
    None,
    Waterskin { water_uses: u8 },
}

impl Default for ItemMetadata {
    fn default() -> Self {
        ItemMetadata::None
    }
}

impl ItemMetadata {
    pub fn to_save(&self) -> ItemMetadataSave {
        match *self {
            ItemMetadata::None => ItemMetadataSave::None,
            ItemMetadata::Waterskin { water_uses } => ItemMetadataSave::Waterskin { water_uses },
        }
    }

    pub fn from_save(s: &ItemMetadataSave) -> Self {
        match *s {
            ItemMetadataSave::None => ItemMetadata::None,
            ItemMetadataSave::Waterskin { water_uses } => ItemMetadata::Waterskin { water_uses },
        }
    }
}

#[derive(Clone, Debug)]
pub struct ItemInstance {
    pub kind: ItemKind,
    pub count: u16,
    pub weight_g_each: u32,
    pub charges: Option<u16>,
    pub metadata: ItemMetadata,
}

impl ItemInstance {
    pub fn unique(
        kind: ItemKind,
        weight_g: u32,
        charges: Option<u16>,
        metadata: ItemMetadata,
    ) -> Self {
        Self {
            kind,
            count: 1,
            weight_g_each: weight_g,
            charges,
            metadata,
        }
    }

    pub fn stack(
        kind: ItemKind,
        count: u16,
        weight_g_each: u32,
        charges: Option<u16>,
        metadata: ItemMetadata,
    ) -> Self {
        Self {
            kind,
            count,
            weight_g_each,
            charges,
            metadata,
        }
    }

    pub fn total_weight_g(&self) -> u32 {
        (self.weight_g_each as u64)
            .saturating_mul(self.count as u64)
            .min(u32::MAX as u64) as u32
    }

    pub fn to_save(&self) -> ItemInstanceSave {
        ItemInstanceSave {
            kind: self.kind.save_key().to_string(),
            count: self.count,
            weight_g_each: self.weight_g_each,
            charges: self.charges,
            metadata: self.metadata.to_save(),
        }
    }

    pub fn from_save(s: &ItemInstanceSave) -> Option<Self> {
        let kind = ItemKind::from_save_key(&s.kind)?;
        Some(Self {
            kind,
            count: s.count.max(1),
            weight_g_each: s.weight_g_each,
            charges: s.charges,
            metadata: ItemMetadata::from_save(&s.metadata),
        })
    }
}

#[derive(Clone, Debug)]
pub struct Pack {
    pub capacity_g: u32,
    pub contents: Vec<ItemInstance>,
}

impl Pack {
    pub fn empty(capacity_g: u32) -> Self {
        Self {
            capacity_g,
            contents: Vec::new(),
        }
    }

    pub fn total_weight_g(&self) -> u32 {
        self.contents
            .iter()
            .map(|i| i.total_weight_g() as u64)
            .sum::<u64>()
            .min(u32::MAX as u64) as u32
    }

    /// Try to add an item. If it doesn't fit (over capacity), return the item
    /// back to the caller untouched. Fungible items merge into an existing
    /// matching stack when possible.
    pub fn try_add(&mut self, item: ItemInstance) -> Result<(), ItemInstance> {
        let new_weight =
            (self.total_weight_g() as u64).saturating_add(item.total_weight_g() as u64);
        if new_weight > self.capacity_g as u64 {
            return Err(item);
        }
        if item.kind.is_fungible()
            && matches!(item.metadata, ItemMetadata::None)
            && item.charges.is_none()
        {
            if let Some(existing) = self.contents.iter_mut().find(|i| {
                i.kind == item.kind
                    && matches!(i.metadata, ItemMetadata::None)
                    && i.charges.is_none()
                    && i.weight_g_each == item.weight_g_each
            }) {
                existing.count = existing.count.saturating_add(item.count);
                return Ok(());
            }
        }
        self.contents.push(item);
        Ok(())
    }

    pub fn to_save(&self) -> PackSave {
        PackSave {
            capacity_g: self.capacity_g,
            contents: self.contents.iter().map(|i| i.to_save()).collect(),
        }
    }

    /// Rebuild a pack from a save. Unknown item kinds are dropped silently
    /// (forward-compat across schema versions).
    pub fn from_save(s: &PackSave) -> Self {
        Self {
            capacity_g: s.capacity_g,
            contents: s
                .contents
                .iter()
                .filter_map(ItemInstance::from_save)
                .collect(),
        }
    }
}

/// Slice-1 starting inventory. Total 13.2 kg in a 15 kg pack.
pub fn starting_pack() -> Pack {
    let mut p = Pack::empty(15_000);
    p.contents
        .push(ItemInstance::unique(ItemKind::Axe, 1_000, None, ItemMetadata::None));
    p.contents
        .push(ItemInstance::unique(ItemKind::Knife, 200, None, ItemMetadata::None));
    p.contents.push(ItemInstance::stack(
        ItemKind::Ration,
        3,
        500,
        None,
        ItemMetadata::None,
    ));
    p.contents.push(ItemInstance::unique(
        ItemKind::Waterskin,
        1_200,
        Some(4),
        ItemMetadata::Waterskin { water_uses: 4 },
    ));
    p.contents.push(ItemInstance::unique(
        ItemKind::Waterskin,
        1_200,
        Some(4),
        ItemMetadata::Waterskin { water_uses: 4 },
    ));
    p.contents.push(ItemInstance::unique(
        ItemKind::FlintAndSteel,
        100,
        None,
        ItemMetadata::None,
    ));
    p.contents
        .push(ItemInstance::unique(ItemKind::Tent, 5_000, None, ItemMetadata::None));
    p.contents.push(ItemInstance::unique(
        ItemKind::Bedroll,
        2_000,
        None,
        ItemMetadata::None,
    ));
    p.contents.push(ItemInstance::unique(
        ItemKind::CookingPan,
        1_000,
        None,
        ItemMetadata::None,
    ));
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_KINDS: &[ItemKind] = &[
        ItemKind::Axe,
        ItemKind::Knife,
        ItemKind::Pack,
        ItemKind::Tent,
        ItemKind::Bedroll,
        ItemKind::CookingPan,
        ItemKind::Waterskin,
        ItemKind::FlintAndSteel,
        ItemKind::Herb,
        ItemKind::Twig,
        ItemKind::Stick,
        ItemKind::Firewood,
        ItemKind::GrassBlade,
        ItemKind::Stone,
        ItemKind::MossPatch,
        ItemKind::Mud,
        ItemKind::Ration,
    ];

    #[test]
    fn save_key_round_trip_for_every_kind() {
        for &k in ALL_KINDS {
            let key = k.save_key();
            assert_eq!(
                ItemKind::from_save_key(key),
                Some(k),
                "round-trip failed for {:?} via key {}",
                k,
                key
            );
        }
    }

    #[test]
    fn unknown_save_key_returns_none() {
        assert_eq!(ItemKind::from_save_key("definitely_not_a_thing"), None);
    }

    #[test]
    fn starting_pack_weighs_13200g() {
        assert_eq!(starting_pack().total_weight_g(), 13_200);
    }

    #[test]
    fn starting_pack_has_two_waterskins_as_distinct_uniques() {
        let p = starting_pack();
        let ws_count = p
            .contents
            .iter()
            .filter(|i| i.kind == ItemKind::Waterskin)
            .count();
        assert_eq!(ws_count, 2);
    }

    #[test]
    fn fungible_items_stack_in_pack() {
        let mut pack = Pack::empty(1_000);
        let twigs1 = ItemInstance::stack(ItemKind::Twig, 3, 5, None, ItemMetadata::None);
        let twigs2 = ItemInstance::stack(ItemKind::Twig, 2, 5, None, ItemMetadata::None);
        assert!(pack.try_add(twigs1).is_ok());
        assert!(pack.try_add(twigs2).is_ok());
        assert_eq!(pack.contents.len(), 1);
        assert_eq!(pack.contents[0].count, 5);
        assert_eq!(pack.total_weight_g(), 25);
    }

    #[test]
    fn unique_items_dont_stack() {
        let mut pack = Pack::empty(10_000);
        let ws1 = ItemInstance::unique(
            ItemKind::Waterskin,
            1_200,
            Some(4),
            ItemMetadata::Waterskin { water_uses: 4 },
        );
        let ws2 = ItemInstance::unique(
            ItemKind::Waterskin,
            1_200,
            Some(4),
            ItemMetadata::Waterskin { water_uses: 4 },
        );
        assert!(pack.try_add(ws1).is_ok());
        assert!(pack.try_add(ws2).is_ok());
        assert_eq!(pack.contents.len(), 2);
    }

    #[test]
    fn over_capacity_bounces_entire_item() {
        let mut pack = Pack::empty(1_000);
        let heavy = ItemInstance::unique(ItemKind::Tent, 5_000, None, ItemMetadata::None);
        let result = pack.try_add(heavy);
        assert!(result.is_err());
        assert_eq!(pack.contents.len(), 0);
        assert_eq!(pack.total_weight_g(), 0);
    }
}
