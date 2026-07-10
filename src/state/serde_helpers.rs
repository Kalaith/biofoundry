//! Serde adapters for session state.
//!
//! JSON object keys must be strings, so `HashMap<TilePos, T>` fields are
//! round-tripped as a sequence of `(TilePos, T)` pairs instead.

pub mod tile_key_map {
    use macroquad_toolkit::grid::TilePos;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::collections::HashMap;

    pub fn serialize<S, T>(map: &HashMap<TilePos, T>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
        T: Serialize,
    {
        let mut pairs: Vec<(&TilePos, &T)> = map.iter().collect();
        // Deterministic output ordering keeps saves diff-friendly.
        pairs.sort_by_key(|(pos, _)| (pos.x, pos.y));
        pairs.serialize(serializer)
    }

    pub fn deserialize<'de, D, T>(deserializer: D) -> Result<HashMap<TilePos, T>, D::Error>
    where
        D: Deserializer<'de>,
        T: Deserialize<'de>,
    {
        let pairs: Vec<(TilePos, T)> = Vec::deserialize(deserializer)?;
        Ok(pairs.into_iter().collect())
    }
}
