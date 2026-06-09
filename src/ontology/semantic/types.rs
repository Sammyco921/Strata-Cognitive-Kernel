use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TypedNode {
    pub id: u64,
    pub node_type: String,
    pub properties: BTreeMap<String, String>,
    pub semantic_type_name: Option<String>,
    pub semantic_type_description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TypedEdge {
    pub id: u64,
    pub from_node: u64,
    pub to_node: u64,
    pub edge_type: String,
    pub properties: BTreeMap<String, String>,
    pub semantic_type_name: Option<String>,
    pub semantic_type_description: Option<String>,
    pub from_node_type: Option<String>,
    pub to_node_type: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SemanticGraph {
    pub nodes: BTreeMap<u64, TypedNode>,
    pub edges: BTreeMap<u64, TypedEdge>,
}

// ── Serialization ──────────────────────────────────────────────────────────

fn escape_json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn serialize_properties(props: &BTreeMap<String, String>) -> String {
    let mut parts: Vec<String> = Vec::new();
    for (k, v) in props {
        parts.push(format!("{}:{}", escape_json_string(k), escape_json_string(v)));
    }
    format!("{{{}}}", parts.join(","))
}

fn serialize_optional_string(val: &Option<String>) -> String {
    match val {
        Some(s) => escape_json_string(s),
        None => "null".to_string(),
    }
}

impl TypedNode {
    pub fn to_deterministic_string(&self) -> String {
        format!(
            "{{\"id\":{},\"node_type\":{},\"properties\":{},\"semantic_type_name\":{},\"semantic_type_description\":{}}}",
            self.id,
            escape_json_string(&self.node_type),
            serialize_properties(&self.properties),
            serialize_optional_string(&self.semantic_type_name),
            serialize_optional_string(&self.semantic_type_description),
        )
    }

    pub fn from_deterministic_string(s: &str) -> Option<Self> {
        let s = s.trim();
        let obj = extract_json_object(s)?;
        let id = extract_u64_field(obj, "id")?;
        let node_type = extract_string_field(obj, "node_type")?;
        let properties = extract_properties(obj, "properties")?;
        let semantic_type_name = extract_optional_string_field(obj, "semantic_type_name");
        let semantic_type_description = extract_optional_string_field(obj, "semantic_type_description");
        Some(TypedNode { id, node_type, properties, semantic_type_name, semantic_type_description })
    }
}

impl TypedEdge {
    pub fn to_deterministic_string(&self) -> String {
        format!(
            "{{\"id\":{},\"from_node\":{},\"to_node\":{},\"edge_type\":{},\"properties\":{},\"semantic_type_name\":{},\"semantic_type_description\":{},\"from_node_type\":{},\"to_node_type\":{}}}",
            self.id,
            self.from_node,
            self.to_node,
            escape_json_string(&self.edge_type),
            serialize_properties(&self.properties),
            serialize_optional_string(&self.semantic_type_name),
            serialize_optional_string(&self.semantic_type_description),
            serialize_optional_string(&self.from_node_type),
            serialize_optional_string(&self.to_node_type),
        )
    }

    pub fn from_deterministic_string(s: &str) -> Option<Self> {
        let s = s.trim();
        let obj = extract_json_object(s)?;
        let id = extract_u64_field(obj, "id")?;
        let from_node = extract_u64_field(obj, "from_node")?;
        let to_node = extract_u64_field(obj, "to_node")?;
        let edge_type = extract_string_field(obj, "edge_type")?;
        let properties = extract_properties(obj, "properties")?;
        let semantic_type_name = extract_optional_string_field(obj, "semantic_type_name");
        let semantic_type_description = extract_optional_string_field(obj, "semantic_type_description");
        let from_node_type = extract_optional_string_field(obj, "from_node_type");
        let to_node_type = extract_optional_string_field(obj, "to_node_type");
        Some(TypedEdge { id, from_node, to_node, edge_type, properties, semantic_type_name, semantic_type_description, from_node_type, to_node_type })
    }
}

impl SemanticGraph {
    pub fn to_deterministic_string(&self) -> String {
        let mut node_parts: Vec<String> = Vec::new();
        for (id, node) in &self.nodes {
            node_parts.push(format!("\"{}\":{}", id, node.to_deterministic_string()));
        }
        let mut edge_parts: Vec<String> = Vec::new();
        for (id, edge) in &self.edges {
            edge_parts.push(format!("\"{}\":{}", id, edge.to_deterministic_string()));
        }
        format!("{{\"nodes\":{{{}}},\"edges\":{{{}}}}}", node_parts.join(","), edge_parts.join(","))
    }

    pub fn from_deterministic_string(s: &str) -> Option<Self> {
        let s = s.trim();
        let obj = extract_json_object(s)?;
        let nodes_str = extract_field_raw(obj, "nodes")?;
        let edges_str = extract_field_raw(obj, "edges")?;
        let nodes = parse_u64_keyed_objects::<TypedNode>(nodes_str)?;
        let edges = parse_u64_keyed_objects::<TypedEdge>(edges_str)?;
        Some(SemanticGraph { nodes, edges })
    }
}

// ── JSON parsing utilities ─────────────────────────────────────────────────

fn extract_json_object(s: &str) -> Option<&str> {
    let s = s.trim();
    if !s.starts_with('{') || !s.ends_with('}') {
        return None;
    }
    Some(&s[1..s.len()-1])
}

pub(crate) fn extract_field_raw<'a>(obj: &'a str, field: &str) -> Option<&'a str> {
    let pattern = format!("\"{}\":", field);
    let start = obj.find(&pattern)?;
    let rest = &obj[start + pattern.len()..];
    let trimmed = rest.trim_start();
    if trimmed.is_empty() {
        return None;
    }
    let first = trimmed.chars().next()?;
    let end = match first {
        '{' => find_matching_brace(trimmed)?,
        '[' => find_matching_bracket(trimmed)?,
        '"' => {
            let mut i = 1;
            let bytes = trimmed.as_bytes();
            while i < trimmed.len() {
                if bytes[i] == b'"' && bytes[i-1] != b'\\' {
                    return Some(&trimmed[..=i]);
                }
                i += 1;
            }
            return None;
        }
        _ => {
            // Number or keyword
            let end = trimmed.find(|c: char| c == ',' || c == '}' || c == ']').unwrap_or(trimmed.len());
            return Some(&trimmed[..end]);
        }
    };
    Some(&trimmed[..=end])
}

fn find_matching_brace(s: &str) -> Option<usize> {
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    for (i, ch) in s.char_indices() {
        if escaped { escaped = false; continue; }
        if ch == '\\' { escaped = true; continue; }
        if ch == '"' { in_string = !in_string; continue; }
        if in_string { continue; }
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 { return Some(i); }
            }
            _ => {}
        }
    }
    None
}

fn find_matching_bracket(s: &str) -> Option<usize> {
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    for (i, ch) in s.char_indices() {
        if escaped { escaped = false; continue; }
        if ch == '\\' { escaped = true; continue; }
        if ch == '"' { in_string = !in_string; continue; }
        if in_string { continue; }
        match ch {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 { return Some(i); }
            }
            _ => {}
        }
    }
    None
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
                let next = chars.next()?;
                match next {
                    '"' => val.push('"'),
                    'n' => val.push('\n'),
                    't' => val.push('\t'),
                    'r' => val.push('\r'),
                    '\\' => val.push('\\'),
                    c => { val.push('\\'); val.push(c); }
                }
            }
            '"' => break,
            c => val.push(c),
        }
    }
    Some(val)
}

fn extract_u64_field(obj: &str, field: &str) -> Option<u64> {
    let pattern = format!("\"{}\":", field);
    let start = obj.find(&pattern)?;
    let rest = &obj[start + pattern.len()..];
    let end = rest.find(|c: char| c == ',' || c == '}').unwrap_or(rest.len());
    rest[..end].trim().parse().ok()
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

fn extract_properties(obj: &str, field: &str) -> Option<BTreeMap<String, String>> {
    let pattern = format!("\"{}\":", field);
    let start = obj.find(&pattern)?;
    let rest = &obj[start + pattern.len()..];
    let trimmed = rest.trim_start();
    if !trimmed.starts_with('{') {
        return None;
    }
    let end = find_matching_brace(trimmed)?;
    let inner = &trimmed[1..end];
    let inner = inner.trim();
    if inner.is_empty() {
        return Some(BTreeMap::new());
    }
    let mut map = BTreeMap::new();
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    let mut current_key: Option<String> = None;
    let mut current_val = String::new();
    let mut parsing_key = true;

    for ch in inner.chars() {
        if escaped { escaped = false; continue; }
        if ch == '\\' { escaped = true; continue; }
        if ch == '"' {
            in_string = !in_string;
            if in_string && parsing_key { current_val.clear(); }
            continue;
        }
        if in_string { current_val.push(ch); continue; }
        if ch == ':' && parsing_key && depth == 0 {
            current_key = Some(current_val.clone());
            current_val.clear();
            parsing_key = false;
            continue;
        }
        if (ch == ',' || ch == '}') && depth == 0 {
            if let Some(ref key) = current_key {
                map.insert(key.clone(), current_val.trim().trim_matches('"').to_string());
            }
            current_val.clear();
            current_key = None;
            parsing_key = true;
            continue;
        }
        if ch == '{' || ch == '[' { depth += 1; current_val.push(ch); continue; }
        if ch == '}' || ch == ']' { depth -= 1; current_val.push(ch); continue; }
        current_val.push(ch);
    }
    if let Some(ref key) = current_key {
        let trimmed = current_val.trim().trim_matches('"');
        if !key.is_empty() || !trimmed.is_empty() {
            map.insert(key.clone(), trimmed.to_string());
        }
    }
    Some(map)
}

pub(crate) fn parse_u64_keyed_objects<T: FromJsonObject>(s: &str) -> Option<BTreeMap<u64, T>> {
    let s = s.trim();
    if !s.starts_with('{') || !s.ends_with('}') {
        return None;
    }
    let inner = s[1..s.len()-1].trim();
    if inner.is_empty() {
        return Some(BTreeMap::new());
    }
    let mut map = BTreeMap::new();
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    let mut current_key: Option<u64> = None;
    let mut current_val = String::new();
    let mut parsing_key = true;

    for ch in inner.chars() {
        if escaped { escaped = false; continue; }
        if ch == '\\' { escaped = true; continue; }
        if ch == '"' {
            in_string = !in_string;
            if in_string && parsing_key { current_val.clear(); }
            current_val.push(ch);
            continue;
        }
        if in_string { current_val.push(ch); continue; }
        if ch == ':' && parsing_key && depth == 0 {
            current_key = Some(current_val.trim().trim_matches('"').parse::<u64>().ok()?);
            current_val.clear();
            parsing_key = false;
            continue;
        }
        if ch == '{' { depth += 1; if depth == 1 { current_val.clear(); } current_val.push(ch); continue; }
        if ch == '}' {
            depth -= 1;
            if depth == 0 {
                current_val.push('}');
                if let Some(key) = current_key {
                    let obj = T::from_deterministic_string(&current_val)?;
                    map.insert(key, obj);
                }
                current_val.clear();
                current_key = None;
                parsing_key = true;
                continue;
            }
            current_val.push(ch);
            continue;
        }
        if ch == ',' && depth == 0 {
            parsing_key = true;
            current_key = None;
            current_val.clear();
            continue;
        }
        if !parsing_key {
            current_val.push(ch);
        }
    }
    Some(map)
}

pub(crate) trait FromJsonObject: Sized {
    fn from_deterministic_string(s: &str) -> Option<Self>;
}

impl FromJsonObject for TypedNode {
    fn from_deterministic_string(s: &str) -> Option<Self> {
        TypedNode::from_deterministic_string(s)
    }
}

impl FromJsonObject for TypedEdge {
    fn from_deterministic_string(s: &str) -> Option<Self> {
        TypedEdge::from_deterministic_string(s)
    }
}

pub fn abi_contract() -> crate::abi::registry::AbiContract {
    crate::abi::registry::AbiContract::new(
        "SemanticGraphTypes",
        "1.0.0",
        &[
            "id", "node_type", "properties",
            "semantic_type_name", "semantic_type_description",
            "from_node", "to_node", "edge_type",
            "from_node_type", "to_node_type",
            "nodes", "edges",
        ],
        &["OntologyTypes"],
    )
}


