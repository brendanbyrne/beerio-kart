use uuid::Uuid;

use crate::domain::DrinkTypeId;

/// Namespace UUID for deterministic drink type IDs.
/// Generated once, never changes. Used in both seed logic and API endpoints.
const DRINK_TYPE_NAMESPACE: Uuid = Uuid::from_bytes([
    0x6b, 0xa7, 0xb8, 0x10, 0x9d, 0xad, 0x11, 0xd1, 0x80, 0xb4, 0x00, 0xc0, 0x4f, 0xd4, 0x30, 0xc8,
]);

/// ASCII Unit Separator. Joins the uppercased name and the alcoholic flag so
/// the two fields can't run together — a control char that can't appear in a
/// drink name, so `("AB", true)` and `("A", "Btrue")`-style collisions are
/// impossible.
const FIELD_SEP: char = '\u{1f}';

/// Compute a deterministic [`DrinkTypeId`] for a drink type by `(name, alcoholic)`.
///
/// The name is matched case-insensitively (uppercased before hashing); the
/// `alcoholic` flag is part of the identity, so the alcoholic and
/// non-alcoholic forms of the same name get distinct IDs. Equivalent to
/// `uuid_v5(DRINK_TYPE_NAMESPACE, "{UPPERCASE(name)}\x1f{alcoholic}")` wrapped
/// in the newtype.
#[must_use]
pub fn drink_type_uuid(name: &str, alcoholic: bool) -> DrinkTypeId {
    let key = format!("{}{FIELD_SEP}{alcoholic}", name.to_uppercase());
    DrinkTypeId::new(Uuid::new_v5(&DRINK_TYPE_NAMESPACE, key.as_bytes()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_drink_type_uuid_is_deterministic() {
        let id1 = drink_type_uuid("Molson Canadian", true);
        let id2 = drink_type_uuid("Molson Canadian", true);
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_drink_type_uuid_is_case_insensitive() {
        let id1 = drink_type_uuid("Molson Canadian", true);
        let id2 = drink_type_uuid("MOLSON CANADIAN", true);
        let id3 = drink_type_uuid("molson canadian", true);
        assert_eq!(id1, id2);
        assert_eq!(id2, id3);
    }

    #[test]
    fn test_different_names_produce_different_uuids() {
        let id1 = drink_type_uuid("Labatt Blue", true);
        let id2 = drink_type_uuid("Modelo", true);
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_same_name_different_alcoholic_produce_different_uuids() {
        let alcoholic = drink_type_uuid("Punch", true);
        let non_alcoholic = drink_type_uuid("Punch", false);
        assert_ne!(
            alcoholic, non_alcoholic,
            "the alcoholic flag is part of a drink type's identity"
        );
    }
}
