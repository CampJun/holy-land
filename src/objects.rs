//! Interior objects — furnishings placed inside buildings that carry a
//! gameplay *usage*. An object is a sprite (from the sprite-tag catalog) plus
//! an `ObjectKind` derived from the tag's role. Stored per-cell on
//! `CellState.object`, rendered as an overlay below the roof (so they stay
//! hidden until the player steps inside), and — for `Light` — fed into the
//! existing fire-light path so a brazier illuminates the room.
//!
//! Phase 2 wires `Decoration` + `Light`; `Station` / `Container` carry their
//! parameter now but gain behaviour in phases 3 and 4.

use crate::sprite_tags::{Role, Tag};
use crate::sprites::Sprite;

#[derive(Clone, Debug, PartialEq)]
pub enum ObjectKind {
    /// Purely visual dressing.
    Decoration,
    /// Emits light (hearth / brazier / candle).
    Light,
    /// Crafting station tied to a skill (phase 3).
    Station(String),
    /// Loot container keyed to a loot table (phase 4).
    Container(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Object {
    pub sprite: Sprite,
    pub kind: ObjectKind,
}

impl Object {
    /// Build an object from a catalog tag. Returns `None` for a `Terrain` tag
    /// (those paint the base layer, not the object layer) or an unresolvable
    /// sheet.
    pub fn from_tag(t: &Tag) -> Option<Object> {
        let kind = match &t.role {
            Role::Decoration => ObjectKind::Decoration,
            Role::Light => ObjectKind::Light,
            Role::Station { skill } => ObjectKind::Station(skill.clone()),
            Role::Container { loot } => ObjectKind::Container(loot.clone()),
            Role::Terrain { .. } => return None,
        };
        Some(Object { sprite: t.sprite()?, kind })
    }

    /// True for objects that should act as a light source.
    pub fn emits_light(&self) -> bool {
        matches!(self.kind, ObjectKind::Light)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sprite_tags::Tag;

    fn tag(role: Role) -> Tag {
        Tag { name: "t".into(), sheet: "misc".into(), col: 0, row: 0, role }
    }

    #[test]
    fn from_tag_maps_roles_and_terrain_is_not_an_object() {
        assert_eq!(Object::from_tag(&tag(Role::Light)).unwrap().kind, ObjectKind::Light);
        assert!(Object::from_tag(&tag(Role::Light)).unwrap().emits_light());
        assert_eq!(Object::from_tag(&tag(Role::Decoration)).unwrap().kind, ObjectKind::Decoration);
        assert_eq!(
            Object::from_tag(&tag(Role::Station { skill: "metallurgy".into() })).unwrap().kind,
            ObjectKind::Station("metallurgy".into())
        );
        // A terrain tag is a base-layer cell, never an object.
        assert!(Object::from_tag(&tag(Role::Terrain { passable: true, blocks_sight: false })).is_none());
    }
}
