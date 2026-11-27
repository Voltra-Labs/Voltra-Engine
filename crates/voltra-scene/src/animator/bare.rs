//! An optional field written bare: `clip: Path("a.anim.ron")`, not
//! `clip: Some(Path("a.anim.ron"))`, and left out when it is `None`.
//!
//! For a field that was once required: files written before it became
//! optional read unchanged, and new ones read the same way. Used with
//! `#[serde(default, skip_serializing_if = "Option::is_none", with = "bare")]`.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub(super) fn serialize<T: Serialize, S: Serializer>(
    value: &Option<T>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    match value {
        Some(value) => value.serialize(serializer),
        None => serializer.serialize_none(),
    }
}

pub(super) fn deserialize<'de, T: Deserialize<'de>, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<T>, D::Error> {
    T::deserialize(deserializer).map(Some)
}
