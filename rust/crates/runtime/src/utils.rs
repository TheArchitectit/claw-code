//! Internal utility functions and helpers.

/// Serialization helpers for HashMap with non-string keys.
pub mod serde_hashmap {
    use std::collections::HashMap;

    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<K, V, S>(map: &HashMap<K, V>, serializer: S) -> Result<S::Ok, S::Error>
    where
        K: Serialize + AsRef<str>,
        V: Serialize,
        S: Serializer,
    {
        let string_map: HashMap<&str, &V> =
            map.iter().map(|(k, v)| (k.as_ref(), v)).collect();
        string_map.serialize(serializer)
    }

    pub fn deserialize<'de, K, V, D>(deserializer: D) -> Result<HashMap<K, V>, D::Error>
    where
        K: Deserialize<'de> + std::hash::Hash + Eq + From<String>,
        V: Deserialize<'de>,
        D: Deserializer<'de>,
    {
        let string_map: HashMap<String, V> = HashMap::deserialize(deserializer)?;
        Ok(string_map
            .into_iter()
            .map(|(k, v)| (K::from(k), v))
            .collect())
    }
}

/// Serialization helpers for Vec<T> to handle optional fields.
pub mod serde_vec {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    #[allow(dead_code)]
    pub fn serialize<T, S>(vec: &Option<Vec<T>>, serializer: S) -> Result<S::Ok, S::Error>
    where
        T: Serialize,
        S: Serializer,
    {
        match vec {
            Some(v) => v.serialize(serializer),
            None => [(); 0].serialize(serializer),
        }
    }

    #[allow(dead_code)]
    pub fn deserialize<'de, T, D>(deserializer: D) -> Result<Option<Vec<T>>, D::Error>
    where
        T: Deserialize<'de>,
        D: Deserializer<'de>,
    {
        let vec: Vec<T> = Vec::deserialize(deserializer)?;
        if vec.is_empty() {
            Ok(None)
        } else {
            Ok(Some(vec))
        }
    }
}
