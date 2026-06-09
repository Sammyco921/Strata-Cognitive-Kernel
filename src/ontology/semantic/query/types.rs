use std::collections::BTreeMap;



#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PropertyFilter {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct QuerySpec {
    pub node_type_filter: Option<Vec<String>>,
    pub edge_type_filter: Option<Vec<String>>,
    pub property_filters: Vec<PropertyFilter>,
    pub traversal_depth: Option<usize>,
    pub source_node_ids: Option<Vec<u64>>,
    pub target_node_ids: Option<Vec<u64>>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct ResultSet {
    pub nodes: BTreeMap<u64, TypedNode>,
    pub edges: BTreeMap<u64, TypedEdge>,
}

// ── Serialization ──────────────────────────────────────────────────────────

fn escape_json(s: &str) -> String {
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

fn serialize_str_vec(vec: &[String]) -> String {
    if vec.is_empty() {
        return "[]".to_string();
    }
    let items: Vec<String> = vec.iter().map(|s| escape_json(s)).collect();
    format!("[{}]", items.join(","))
}

fn serialize_u64_vec(vec: &[u64]) -> String {
    if vec.is_empty() {
        return "[]".to_string();
    }
    let items: Vec<String> = vec.iter().map(|n| n.to_string()).collect();
    format!("[{}]", items.join(","))
}

fn serialize_option_vec<T>(opt: &Option<Vec<T>>, f: fn(&[T]) -> String) -> String {
    match opt {
        Some(v) => f(v),
        None => "null".to_string(),
    }
}

impl PropertyFilter {
    pub fn to_deterministic_string(&self) -> String {
        format!("{{\"key\":{},\"value\":{}}}", escape_json(&self.key), escape_json(&self.value))
    }

    pub fn from_deterministic_string(s: &str) -> Option<Self> {
        let s = s.trim();
        let obj = s.strip_prefix('{')?.strip_suffix('}')?;
        let key = extract_string_field(obj, "key")?;
        let value = extract_string_field(obj, "value")?;
        Some(PropertyFilter { key, value })
    }
}

impl QuerySpec {
    pub fn to_deterministic_string(&self) -> String {
        let nf = serialize_option_vec(&self.node_type_filter, serialize_str_vec);
        let ef = serialize_option_vec(&self.edge_type_filter, serialize_str_vec);
        let pf: String = if self.property_filters.is_empty() {
            "[]".to_string()
        } else {
            let items: Vec<String> = self.property_filters.iter()
                .map(|f| f.to_deterministic_string())
                .collect();
            format!("[{}]", items.join(","))
        };
        let td = match self.traversal_depth {
            Some(d) => d.to_string(),
            None => "null".to_string(),
        };
        let sn = serialize_option_vec(&self.source_node_ids, serialize_u64_vec);
        let tn = serialize_option_vec(&self.target_node_ids, serialize_u64_vec);
        format!(
            "{{\"node_type_filter\":{},\"edge_type_filter\":{},\"property_filters\":{},\"traversal_depth\":{},\"source_node_ids\":{},\"target_node_ids\":{}}}",
            nf, ef, pf, td, sn, tn
        )
    }

    pub fn from_deterministic_string(s: &str) -> Option<Self> {
        let s = s.trim();
        let obj = s.strip_prefix('{')?.strip_suffix('}')?;
        let node_type_filter = extract_optional_str_vec(obj, "node_type_filter");
        let edge_type_filter = extract_optional_str_vec(obj, "edge_type_filter");
        let property_filters = extract_property_filter_vec(obj, "property_filters");
        let traversal_depth = extract_optional_usize(obj, "traversal_depth");
        let source_node_ids = extract_optional_u64_vec(obj, "source_node_ids");
        let target_node_ids = extract_optional_u64_vec(obj, "target_node_ids");
        Some(QuerySpec { node_type_filter, edge_type_filter, property_filters, traversal_depth, source_node_ids, target_node_ids })
    }
}

impl ResultSet {
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
        let obj = s.strip_prefix('{')?.strip_suffix('}')?;
        let nodes_str = extract_field_raw(obj, "nodes")?;
        let edges_str = extract_field_raw(obj, "edges")?;
        let nodes = parse_u64_keyed_objects::<TypedNode>(nodes_str)?;
        let edges = parse_u64_keyed_objects::<TypedEdge>(edges_str)?;
        Some(ResultSet { nodes, edges })
    }
}

// ── JSON parsing utilities (adapted from semantic types.rs) ────────────────

fn extract_field_raw<'a>(obj: &'a str, field: &str) -> Option<&'a str> {
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
            '}' => { depth -= 1; if depth == 0 { return Some(i); } }
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
            ']' => { depth -= 1; if depth == 0 { return Some(i); } }
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

fn extract_optional_str_vec(obj: &str, field: &str) -> Option<Vec<String>> {
    let pattern = format!("\"{}\":", field);
    let start = obj.find(&pattern)?;
    let after = &obj[start + pattern.len()..];
    let trimmed = after.trim_start();
    if trimmed.starts_with("null") {
        return None;
    }
    if !trimmed.starts_with('[') {
        return None;
    }
    let end = find_matching_bracket(trimmed)?;
    let inner = &trimmed[1..end];
    let inner = inner.trim();
    if inner.is_empty() {
        return Some(Vec::new());
    }
    let mut result = Vec::new();
    let mut current = String::new();
    let mut in_str = false;
    let mut escaped = false;
    for ch in inner.chars() {
        if escaped { escaped = false; current.push(ch); continue; }
        if ch == '\\' { escaped = true; continue; }
        if ch == '"' { in_str = !in_str; continue; }
        if ch == ',' && !in_str {
            result.push(current.clone());
            current.clear();
            continue;
        }
        if in_str { current.push(ch); }
    }
    if !current.is_empty() {
        result.push(current);
    }
    Some(result)
}

fn extract_optional_u64_vec(obj: &str, field: &str) -> Option<Vec<u64>> {
    let pattern = format!("\"{}\":", field);
    let start = obj.find(&pattern)?;
    let after = &obj[start + pattern.len()..];
    let trimmed = after.trim_start();
    if trimmed.starts_with("null") {
        return None;
    }
    if !trimmed.starts_with('[') {
        return None;
    }
    let end = find_matching_bracket(trimmed)?;
    let inner = &trimmed[1..end];
    let inner = inner.trim();
    if inner.is_empty() {
        return Some(Vec::new());
    }
    let result: Vec<u64> = inner.split(',')
        .filter_map(|s| s.trim().parse().ok())
        .collect();
    Some(result)
}

fn extract_optional_usize(obj: &str, field: &str) -> Option<usize> {
    let pattern = format!("\"{}\":", field);
    let start = obj.find(&pattern)?;
    let after = &obj[start + pattern.len()..];
    let trimmed = after.trim_start();
    if trimmed.starts_with("null") {
        return None;
    }
    let end = trimmed.find(|c: char| c == ',' || c == '}').unwrap_or(trimmed.len());
    trimmed[..end].trim().parse().ok()
}

fn extract_property_filter_vec(obj: &str, field: &str) -> Vec<PropertyFilter> {
    let pattern = format!("\"{}\":", field);
    let start = match obj.find(&pattern) {
        Some(pos) => pos,
        None => return Vec::new(),
    };
    let after = &obj[start + pattern.len()..];
    let trimmed = after.trim_start();
    if !trimmed.starts_with('[') {
        return Vec::new();
    }
    let end = match find_matching_bracket(trimmed) {
        Some(pos) => pos,
        None => return Vec::new(),
    };
    let inner = &trimmed[1..end];
    let inner = inner.trim();
    if inner.is_empty() {
        return Vec::new();
    }
    // Parse each object in the array
    let mut result = Vec::new();
    let mut depth = 0i32;
    let mut in_str = false;
    let mut escaped = false;
    let mut current = String::new();
    for ch in inner.chars() {
        if escaped { escaped = false; current.push(ch); continue; }
        if ch == '\\' { escaped = true; current.push(ch); continue; }
        if ch == '"' { in_str = !in_str; current.push(ch); continue; }
        if in_str { current.push(ch); continue; }
        match ch {
            '{' => { depth += 1; current.push(ch); }
            '}' => { depth -= 1; current.push(ch); if depth == 0 && !current.is_empty() { if let Some(pf) = PropertyFilter::from_deterministic_string(&current) { result.push(pf); } current.clear(); } }
            ',' => { if depth == 0 { /* skip array-level commas */ } else { current.push(ch); } }
            _ => { current.push(ch); }
        }
    }
    result
}

// Re-export parse_u64_keyed_objects from parent types
use crate::ontology::semantic::types::{parse_u64_keyed_objects, TypedNode, TypedEdge};

pub fn abi_contract() -> crate::abi::registry::AbiContract {
    crate::abi::registry::AbiContract::new(
        "QueryTypes",
        "1.0.0",
        &[
            "key", "value",
            "node_type_filter", "edge_type_filter", "property_filters",
            "traversal_depth", "source_node_ids", "target_node_ids",
            "nodes", "edges",
        ],
        &["SemanticGraphTypes"],
    )
}
