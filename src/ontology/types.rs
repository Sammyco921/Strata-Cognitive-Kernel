use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum OntologyEventType {
    CreateEntityType,
    CreateRelationshipType,
    CreatePropertyType,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct EntityTypeDef {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RelationshipTypeDef {
    pub name: String,
    pub from_entity: String,
    pub to_entity: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PropertyTypeDef {
    pub name: String,
    pub value_type: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum OntologyPayload {
    EntityType(EntityTypeDef),
    RelationshipType(RelationshipTypeDef),
    PropertyType(PropertyTypeDef),
}

impl OntologyPayload {
    pub fn name(&self) -> &str {
        match self {
            OntologyPayload::EntityType(d) => &d.name,
            OntologyPayload::RelationshipType(d) => &d.name,
            OntologyPayload::PropertyType(d) => &d.name,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OntologyEvent {
    pub id: String,
    pub timestamp: u64,
    pub event_type: OntologyEventType,
    pub payload: OntologyPayload,
    pub causes: Vec<String>,
}

impl OntologyEvent {
    pub fn new(
        event_type: OntologyEventType,
        payload: OntologyPayload,
        timestamp: u64,
    ) -> Self {
        let id = compute_event_id(&event_type, &payload);
        OntologyEvent {
            id,
            timestamp,
            event_type,
            payload,
            causes: Vec::new(),
        }
    }

    pub fn with_causes(
        event_type: OntologyEventType,
        payload: OntologyPayload,
        timestamp: u64,
        causes: Vec<String>,
    ) -> Self {
        let id = compute_event_id(&event_type, &payload);
        OntologyEvent {
            id,
            timestamp,
            event_type,
            payload,
            causes,
        }
    }
}

fn compute_event_id(event_type: &OntologyEventType, payload: &OntologyPayload) -> String {
    let tag = match event_type {
        OntologyEventType::CreateEntityType => "et",
        OntologyEventType::CreateRelationshipType => "rt",
        OntologyEventType::CreatePropertyType => "pt",
    };
    format!("ont:{}:{}", tag, payload.name())
}

// ── Serialization ──────────────────────────────────────────────────────────

impl OntologyEvent {
    pub fn to_deterministic_string(&self) -> String {
        let payload_json = payload_to_json(&self.payload);
        let causes_json = if self.causes.is_empty() {
            "[]".to_string()
        } else {
            let items: Vec<String> = self.causes.iter().map(|c| format!("\"{}\"", c)).collect();
            format!("[{}]", items.join(","))
        };
        format!(
            "{{\"id\":\"{}\",\"timestamp\":{},\"event_type\":\"{}\",\"payload\":{},\"causes\":{}}}",
            self.id,
            self.timestamp,
            event_type_to_str(&self.event_type),
            payload_json,
            causes_json,
        )
    }

    pub fn from_deterministic_string(s: &str) -> Option<Self> {
        let s = s.trim();
        if !s.starts_with('{') || !s.ends_with('}') {
            return None;
        }
        let inner = &s[1..s.len()-1];
        let mut fields: BTreeMap<String, String> = BTreeMap::new();
        let mut depth = 0i32;
        let mut current_key: Option<String> = None;
        let mut current_val = String::new();
        let mut in_key = true;
        let mut in_string = false;
        let mut escaped = false;

        for ch in inner.chars() {
            if escaped {
                current_val.push(ch);
                escaped = false;
                continue;
            }
            if ch == '\\' {
                escaped = true;
                current_val.push(ch);
                continue;
            }
            if ch == '"' {
                in_string = !in_string;
                if in_string && in_key {
                    current_val.clear();
                }
                current_val.push(ch);
                continue;
            }
            if in_string {
                current_val.push(ch);
                continue;
            }
            match ch {
                '{' | '[' => {
                    depth += 1;
                    current_val.push(ch);
                }
                '}' | ']' => {
                    depth -= 1;
                    current_val.push(ch);
                }
                ':' if depth == 0 && in_key => {
                    let raw_key = current_val.trim().trim_matches('"');
                    current_key = Some(raw_key.to_string());
                    current_val.clear();
                    in_key = false;
                }
                ',' if depth == 0 => {
                    if let Some(ref key) = current_key {
                        fields.insert(key.clone(), current_val.trim().to_string());
                    }
                    current_val.clear();
                    current_key = None;
                    in_key = true;
                }
                _ => {
                    current_val.push(ch);
                }
            }
        }
        if let Some(ref key) = current_key {
            fields.insert(key.clone(), current_val.trim().to_string());
        }

        let id = fields.get("id")?.trim_matches('"').to_string();
        let timestamp: u64 = fields.get("timestamp")?.parse().ok()?;
        let event_type_str = fields.get("event_type")?.trim_matches('"');
        let event_type = str_to_event_type(event_type_str)?;
        let payload_str = fields.get("payload")?;
        let payload = payload_from_json(payload_str)?;
        let causes_str = fields.get("causes")?;
        let causes = parse_string_array(causes_str);

        Some(OntologyEvent { id, timestamp, event_type, payload, causes })
    }
}

fn event_type_to_str(et: &OntologyEventType) -> &'static str {
    match et {
        OntologyEventType::CreateEntityType => "CreateEntityType",
        OntologyEventType::CreateRelationshipType => "CreateRelationshipType",
        OntologyEventType::CreatePropertyType => "CreatePropertyType",
    }
}

fn str_to_event_type(s: &str) -> Option<OntologyEventType> {
    match s {
        "CreateEntityType" => Some(OntologyEventType::CreateEntityType),
        "CreateRelationshipType" => Some(OntologyEventType::CreateRelationshipType),
        "CreatePropertyType" => Some(OntologyEventType::CreatePropertyType),
        _ => None,
    }
}

fn payload_to_json(payload: &OntologyPayload) -> String {
    match payload {
        OntologyPayload::EntityType(def) => {
            let desc = def.description.as_ref().map(|d| format!("\"{}\"", d)).unwrap_or_else(|| "null".to_string());
            format!("{{\"EntityType\":{{\"name\":\"{}\",\"description\":{}}}}}", def.name, desc)
        }
        OntologyPayload::RelationshipType(def) => {
            let desc = def.description.as_ref().map(|d| format!("\"{}\"", d)).unwrap_or_else(|| "null".to_string());
            format!(
                "{{\"RelationshipType\":{{\"name\":\"{}\",\"from_entity\":\"{}\",\"to_entity\":\"{}\",\"description\":{}}}}}",
                def.name, def.from_entity, def.to_entity, desc
            )
        }
        OntologyPayload::PropertyType(def) => {
            let desc = def.description.as_ref().map(|d| format!("\"{}\"", d)).unwrap_or_else(|| "null".to_string());
            format!("{{\"PropertyType\":{{\"name\":\"{}\",\"value_type\":\"{}\",\"description\":{}}}}}", def.name, def.value_type, desc)
        }
    }
}

fn payload_from_json(s: &str) -> Option<OntologyPayload> {
    let s = s.trim();
    if !s.starts_with('{') || !s.ends_with('}') {
        return None;
    }
    let inner = s[1..s.len()-1].trim();

    if let Some(et_inner) = inner.strip_prefix("\"EntityType\":") {
        let obj = extract_json_object(et_inner)?;
        let name = extract_string_field(obj, "name")?;
        let description = extract_optional_string_field(obj, "description");
        Some(OntologyPayload::EntityType(EntityTypeDef { name, description }))
    } else if let Some(rt_inner) = inner.strip_prefix("\"RelationshipType\":") {
        let obj = extract_json_object(rt_inner)?;
        let name = extract_string_field(obj, "name")?;
        let from_entity = extract_string_field(obj, "from_entity")?;
        let to_entity = extract_string_field(obj, "to_entity")?;
        let description = extract_optional_string_field(obj, "description");
        Some(OntologyPayload::RelationshipType(RelationshipTypeDef { name, from_entity, to_entity, description }))
    } else if let Some(pt_inner) = inner.strip_prefix("\"PropertyType\":") {
        let obj = extract_json_object(pt_inner)?;
        let name = extract_string_field(obj, "name")?;
        let value_type = extract_string_field(obj, "value_type")?;
        let description = extract_optional_string_field(obj, "description");
        Some(OntologyPayload::PropertyType(PropertyTypeDef { name, value_type, description }))
    } else {
        None
    }
}

fn extract_json_object(s: &str) -> Option<&str> {
    let s = s.trim();
    if !s.starts_with('{') || !s.ends_with('}') {
        return None;
    }
    Some(&s[1..s.len()-1])
}

fn extract_string_field(obj: &str, field: &str) -> Option<String> {
    let pattern = format!("\"{}\":\"", field);
    let start = obj.find(&pattern)?;
    let val_start = start + pattern.len();
    let mut val = String::new();
    let mut chars = obj[val_start..].chars();
    loop {
        match chars.next()? {
            '\\' => {
                val.push(chars.next()?);
            }
            '"' => break,
            c => val.push(c),
        }
    }
    Some(val)
}

fn extract_optional_string_field(obj: &str, field: &str) -> Option<String> {
    let pattern = format!("\"{}\":", field);
    let start = obj.find(&pattern)?;
    let after_key = &obj[start + pattern.len()..];
    if after_key.trim_start().starts_with("null") {
        return None;
    }
    extract_string_field(obj, field)
}

fn parse_string_array(s: &str) -> Vec<String> {
    let s = s.trim();
    if !s.starts_with('[') || !s.ends_with(']') {
        return Vec::new();
    }
    let inner = s[1..s.len()-1].trim();
    if inner.is_empty() {
        return Vec::new();
    }
    let mut result = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    let mut escaped = false;
    for ch in inner.chars() {
        if escaped {
            if ch != '"' { current.push('\\'); }
            current.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '"' {
            in_string = !in_string;
            continue;
        }
        if ch == ',' && !in_string {
            result.push(current.trim().trim_matches('"').to_string());
            current.clear();
            continue;
        }
        if in_string {
            current.push(ch);
        }
    }
    if !current.is_empty() {
        result.push(current.trim().trim_matches('"').to_string());
    }
    result
}

// ── Registry ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityType {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationshipType {
    pub name: String,
    pub from_entity: String,
    pub to_entity: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropertyType {
    pub name: String,
    pub value_type: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OntologyRegistry {
    pub entity_types: BTreeMap<String, EntityType>,
    pub relationship_types: BTreeMap<String, RelationshipType>,
    pub property_types: BTreeMap<String, PropertyType>,
}

impl OntologyRegistry {
    pub fn empty() -> Self {
        OntologyRegistry {
            entity_types: BTreeMap::new(),
            relationship_types: BTreeMap::new(),
            property_types: BTreeMap::new(),
        }
    }

    pub fn apply_event(&mut self, event: &OntologyEvent) {
        match &event.payload {
            OntologyPayload::EntityType(def) => {
                self.entity_types.insert(def.name.clone(), EntityType {
                    name: def.name.clone(),
                    description: def.description.clone(),
                });
            }
            OntologyPayload::RelationshipType(def) => {
                self.relationship_types.insert(def.name.clone(), RelationshipType {
                    name: def.name.clone(),
                    from_entity: def.from_entity.clone(),
                    to_entity: def.to_entity.clone(),
                    description: def.description.clone(),
                });
            }
            OntologyPayload::PropertyType(def) => {
                self.property_types.insert(def.name.clone(), PropertyType {
                    name: def.name.clone(),
                    value_type: def.value_type.clone(),
                    description: def.description.clone(),
                });
            }
        }
    }
}

pub fn abi_contract() -> crate::abi::registry::AbiContract {
    crate::abi::registry::AbiContract::new(
        "OntologyTypes",
        "1.0.0",
        &[
            "id", "timestamp", "event_type", "payload", "causes",
            "EntityType", "RelationshipType", "PropertyType",
            "name", "description", "from_entity", "to_entity", "value_type",
        ],
        &[],
    )
}
