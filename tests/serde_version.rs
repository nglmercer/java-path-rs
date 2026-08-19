#![cfg(feature = "serde")]

use java_path::JavaVersion;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

fn hash_of(v: &JavaVersion) -> u64 {
    let mut hasher = DefaultHasher::new();
    v.hash(&mut hasher);
    hasher.finish()
}

#[test]
fn serializes_as_the_version_string() {
    let v = JavaVersion::parse("21.0.3+9").unwrap();
    assert_eq!(serde_json::to_string(&v).unwrap(), "\"21.0.3+9\"");
}

/// Regression: deriving `Deserialize` let a serialized form construct
/// component vectors the parser cannot produce (`[17, 0, 0]`). Such a value
/// compared equal to `17` but hashed differently, breaking the `Eq`/`Hash`
/// contract. Deserialization now goes through `JavaVersion::parse` only.
#[test]
fn deserialization_cannot_bypass_normalisation() {
    let parsed = JavaVersion::parse("17").unwrap();
    for text in ["\"17\"", "\"17.0.0\"", "\"17.0\""] {
        let decoded: JavaVersion = serde_json::from_str(text).unwrap();
        assert_eq!(decoded, parsed, "{text}");
        assert_eq!(
            hash_of(&decoded),
            hash_of(&parsed),
            "{text} hashes differently from an equal value"
        );
        assert_eq!(decoded.components(), parsed.components(), "{text}");
    }
}

#[test]
fn round_trips_every_supported_scheme() {
    for text in ["1.8.0_412-b08", "17.0.10.1+7", "21", "22-ea+15"] {
        let original = JavaVersion::parse(text).unwrap();
        let json = serde_json::to_string(&original).unwrap();
        let decoded: JavaVersion = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded, original, "{text}");
        assert_eq!(decoded.raw(), original.raw(), "{text}");
        assert_eq!(hash_of(&decoded), hash_of(&original), "{text}");
    }
}

/// The struct form the derived impl used to accept is no longer a version.
#[test]
fn rejects_structural_and_unparsable_forms() {
    assert!(serde_json::from_str::<JavaVersion>(
        r#"{"components":[17,0,0],"pre":null,"build":null,"raw":"17"}"#
    )
    .is_err());
    assert!(serde_json::from_str::<JavaVersion>("\"\"").is_err());
    assert!(serde_json::from_str::<JavaVersion>("\"not-a-version\"").is_err());
    assert!(serde_json::from_str::<JavaVersion>("21").is_err());
}
