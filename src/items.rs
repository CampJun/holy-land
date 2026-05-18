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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

    /// True if at least one `ItemInstance` of `kind` is present with
    /// `count > 0`. Works for both fungible stacks and unique entries.
    pub fn has_stack(&self, kind: ItemKind) -> bool {
        self.contents
            .iter()
            .any(|i| i.kind == kind && i.count > 0)
    }

    /// Consume one unit of a stack of `kind`: decrement count by 1; if the
    /// count hits 0, remove the entry. Returns true if a unit was taken.
    /// For unique items (count always 1), this removes the entry entirely.
    pub fn take_one_from_stack(&mut self, kind: ItemKind) -> bool {
        let Some(idx) = self
            .contents
            .iter()
            .position(|i| i.kind == kind && i.count > 0)
        else {
            return false;
        };
        self.contents[idx].count -= 1;
        if self.contents[idx].count == 0 {
            self.contents.remove(idx);
        }
        true
    }

    /// True if any waterskin in the pack still has at least one water use.
    pub fn has_waterskin_with_water(&self) -> bool {
        self.contents.iter().any(|i| {
            i.kind == ItemKind::Waterskin
                && matches!(i.metadata, ItemMetadata::Waterskin { water_uses } if water_uses > 0)
        })
    }

    /// Consume one charge of water from the first waterskin that has any.
    /// Decrements `water_uses` and reduces the waterskin's weight by the
    /// per-use water mass (250 g). Returns true if a charge was consumed.
    pub fn drink_one_water_use(&mut self) -> bool {
        for item in self.contents.iter_mut() {
            if item.kind != ItemKind::Waterskin {
                continue;
            }
            if let ItemMetadata::Waterskin {
                ref mut water_uses,
            } = item.metadata
            {
                if *water_uses > 0 {
                    *water_uses -= 1;
                    item.weight_g_each = item.weight_g_each.saturating_sub(250);
                    return true;
                }
            }
        }
        false
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
    fn has_stack_finds_rations_in_starting_pack() {
        let p = starting_pack();
        assert!(p.has_stack(ItemKind::Ration));
        assert!(p.has_stack(ItemKind::Axe));
        assert!(!p.has_stack(ItemKind::Twig));
    }

    #[test]
    fn take_one_from_stack_decrements_and_removes_on_zero() {
        let mut p = Pack::empty(10_000);
        p.contents
            .push(ItemInstance::stack(ItemKind::Ration, 2, 500, None, ItemMetadata::None));
        assert!(p.take_one_from_stack(ItemKind::Ration));
        assert_eq!(p.contents[0].count, 1);
        assert!(p.take_one_from_stack(ItemKind::Ration));
        assert!(p.contents.is_empty());
        assert!(!p.take_one_from_stack(ItemKind::Ration));
    }

    #[test]
    fn take_one_from_stack_works_for_unique_items() {
        // Unique kinds always have count 1; taking one removes the entry.
        let mut p = Pack::empty(10_000);
        p.contents
            .push(ItemInstance::unique(ItemKind::Knife, 200, None, ItemMetadata::None));
        assert!(p.take_one_from_stack(ItemKind::Knife));
        assert!(p.contents.is_empty());
    }

    #[test]
    fn has_waterskin_with_water_reflects_metadata() {
        let mut p = Pack::empty(10_000);
        p.contents.push(ItemInstance::unique(
            ItemKind::Waterskin,
            1_200,
            Some(4),
            ItemMetadata::Waterskin { water_uses: 4 },
        ));
        assert!(p.has_waterskin_with_water());

        // Drained waterskin: still in pack but has no water.
        p.contents[0].metadata = ItemMetadata::Waterskin { water_uses: 0 };
        p.contents[0].weight_g_each = 200; // empty weight
        assert!(!p.has_waterskin_with_water());
    }

    #[test]
    fn drink_one_water_use_decrements_uses_and_weight() {
        let mut p = Pack::empty(10_000);
        p.contents.push(ItemInstance::unique(
            ItemKind::Waterskin,
            1_200,
            Some(4),
            ItemMetadata::Waterskin { water_uses: 4 },
        ));
        assert!(p.drink_one_water_use());
        match p.contents[0].metadata {
            ItemMetadata::Waterskin { water_uses } => assert_eq!(water_uses, 3),
            other => panic!("expected Waterskin metadata, got {:?}", other),
        }
        assert_eq!(p.contents[0].weight_g_each, 950); // 1200 - 250
    }

    #[test]
    fn drink_one_water_use_returns_false_when_no_water() {
        let mut p = Pack::empty(10_000);
        p.contents.push(ItemInstance::unique(
            ItemKind::Waterskin,
            200,
            Some(0),
            ItemMetadata::Waterskin { water_uses: 0 },
        ));
        assert!(!p.drink_one_water_use());
    }

    #[test]
    fn drink_one_water_use_picks_first_with_water_then_second() {
        let mut p = Pack::empty(10_000);
        // First skin empty, second full. The second one should drain.
        p.contents.push(ItemInstance::unique(
            ItemKind::Waterskin,
            200,
            Some(0),
            ItemMetadata::Waterskin { water_uses: 0 },
        ));
        p.contents.push(ItemInstance::unique(
            ItemKind::Waterskin,
            1_200,
            Some(4),
            ItemMetadata::Waterskin { water_uses: 4 },
        ));
        assert!(p.drink_one_water_use());
        match p.contents[1].metadata {
            ItemMetadata::Waterskin { water_uses } => assert_eq!(water_uses, 3),
            other => panic!("got {:?}", other),
        }
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
