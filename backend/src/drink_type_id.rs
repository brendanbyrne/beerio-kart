use uuid::Uuid;

/// Namespace UUID for deterministic drink type IDs.
/// Generated once, never changes. Used in both seed logic and API endpoints.
const DRINK_TYPE_NAMESPACE: Uuid = Uuid::from_bytes([
    0x6b, 0xa7, 0xb8, 0x10, 0x9d, 0xad, 0x11, 0xd1, 0x80, 0xb4, 0x00, 0xc0, 0x4f, 0xd4, 0x30, 0xc8,
]);

/// Compute a deterministic UUID for a drink type name (case-insensitive).
/// `uuid_v5(DRINK_TYPE_NAMESPACE, UPPERCASE(name))`
pub fn drink_type_uuid(name: &str) -> String {
    Uuid::new_v5(&DRINK_TYPE_NAMESPACE, name.to_uppercase().as_bytes()).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_drink_type_uuid_is_deterministic() {
        let id1 = drink_type_uuid("Molson Canadian");
        let id2 = drink_type_uuid("Molson Canadian");
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_drink_type_uuid_is_case_insensitive() {
        let id1 = drink_type_uuid("Molson Canadian");
        let id2 = drink_type_uuid("MOLSON CANADIAN");
        let id3 = drink_type_uuid("molson canadian");
        assert_eq!(id1, id2);
        assert_eq!(id2, id3);
    }

    #[test]
    fn test_different_names_produce_different_uuids() {
        let id1 = drink_type_uuid("Labatt Blue");
        let id2 = drink_type_uuid("Modelo");
        assert_ne!(id1, id2);
    }
}
