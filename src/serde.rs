use serde::de;
use serde::Deserialize;

pub fn nested_opt<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: de::Deserializer<'de>,
    T: de::Deserialize<'de>,
{
    Ok(Some(Deserialize::deserialize(deserializer)?))
}
