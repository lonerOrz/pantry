use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize, Clone, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum DisplayMode {
    #[default]
    Text,
    Picture,
}

#[derive(Debug, Clone)]
pub struct Category {
    pub display: Option<DisplayMode>,
    pub entries: HashMap<String, String>,
}

impl<'de> Deserialize<'de> for Category {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{MapAccess, Visitor};
        use std::fmt;

        struct CategoryVisitor;

        impl<'de> Visitor<'de> for CategoryVisitor {
            type Value = Category;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a map")
            }

            fn visit_map<V>(self, mut map: V) -> Result<Category, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut display = None;
                let mut entries = HashMap::new();

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "display" => {
                            if display.is_some() {
                                return Err(serde::de::Error::duplicate_field("display"));
                            }
                            display = Some(map.next_value()?);
                        }
                        _ => {
                            let value: String = map.next_value()?;
                            entries.insert(key, value);
                        }
                    }
                }

                Ok(Category { display, entries })
            }
        }

        deserializer.deserialize_map(CategoryVisitor)
    }
}

#[derive(Debug)]
pub struct Config {
    pub display: DisplayMode,
    pub categories: HashMap<String, Category>,
}

impl<'de> Deserialize<'de> for Config {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{MapAccess, Visitor};
        use std::fmt;

        struct ConfigVisitor;

        impl<'de> Visitor<'de> for ConfigVisitor {
            type Value = Config;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a map")
            }

            fn visit_map<V>(self, mut map: V) -> Result<Config, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut display = None;
                let mut categories = HashMap::new();

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "display" => {
                            if display.is_some() {
                                return Err(serde::de::Error::duplicate_field("display"));
                            }
                            display = Some(map.next_value()?);
                        }
                        _ => {
                            let category: Category = map.next_value()?;
                            categories.insert(key, category);
                        }
                    }
                }

                Ok(Config {
                    display: display.unwrap_or_default(),
                    categories,
                })
            }
        }

        deserializer.deserialize_map(ConfigVisitor)
    }
}
