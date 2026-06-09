use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Capability {
    pub id: String,
    pub name: String,
    pub layer: String,
    pub description: String,
}

impl Capability {
    pub fn new(id: &str, name: &str, layer: &str, description: &str) -> Self {
        Capability {
            id: id.to_string(),
            name: name.to_string(),
            layer: layer.to_string(),
            description: description.to_string(),
        }
    }
}
