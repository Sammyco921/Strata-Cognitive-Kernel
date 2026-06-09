use std::collections::BTreeMap;

use crate::ontology::semantic::query::types::ResultSet;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PropertyMatch {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RuleSpec {
    pub id: String,
    pub tag: String,
    pub node_type_match: Option<String>,
    pub node_property_matches: Vec<PropertyMatch>,
    pub specific_node_id: Option<u64>,
    pub edge_type_match: Option<String>,
    pub edge_property_matches: Vec<PropertyMatch>,
    pub specific_edge_id: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct AnnotatedResultSet {
    pub result_set: ResultSet,
    pub node_tags: BTreeMap<u64, Vec<String>>,
    pub edge_tags: BTreeMap<u64, Vec<String>>,
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

fn serialize_optional_string(val: &Option<String>) -> String {
    match val {
        Some(s) => escape_json(s),
        None => "null".to_string(),
    }
}

fn serialize_optional_u64(val: &Option<u64>) -> String {
    match val {
        Some(n) => n.to_string(),
        None => "null".to_string(),
    }
}

fn serialize_property_match_vec(vec: &[PropertyMatch]) -> String {
    if vec.is_empty() {
        return "[]".to_string();
    }
    let items: Vec<String> = vec.iter().map(|pm| pm.to_deterministic_string()).collect();
    format!("[{}]", items.join(","))
}

fn serialize_tag_map(tags: &BTreeMap<u64, Vec<String>>) -> String {
    if tags.is_empty() {
        return "{}".to_string();
    }
    let mut parts: Vec<String> = Vec::new();
    for (id, tag_vec) in tags {
        let tags_str: Vec<String> = tag_vec.iter().map(|t| escape_json(t)).collect();
        parts.push(format!("\"{}\":[{}]", id, tags_str.join(",")));
    }
    format!("{{{}}}", parts.join(","))
}

impl PropertyMatch {
    pub fn to_deterministic_string(&self) -> String {
        format!("{{\"key\":{},\"value\":{}}}", escape_json(&self.key), escape_json(&self.value))
    }

    pub fn from_deterministic_string(s: &str) -> Option<Self> {
        let s = s.trim();
        let obj = s.strip_prefix('{')?.strip_suffix('}')?;
        let key = extract_string_field(obj, "key")?;
        let value = extract_string_field(obj, "value")?;
        Some(PropertyMatch { key, value })
    }
}

impl RuleSpec {
    pub fn to_deterministic_string(&self) -> String {
        format!(
            "{{\"id\":{},\"tag\":{},\"node_type_match\":{},\"node_property_matches\":{},\"specific_node_id\":{},\"edge_type_match\":{},\"edge_property_matches\":{},\"specific_edge_id\":{}}}",
            escape_json(&self.id),
            escape_json(&self.tag),
            serialize_optional_string(&self.node_type_match),
            serialize_property_match_vec(&self.node_property_matches),
            serialize_optional_u64(&self.specific_node_id),
            serialize_optional_string(&self.edge_type_match),
            serialize_property_match_vec(&self.edge_property_matches),
            serialize_optional_u64(&self.specific_edge_id),
        )
    }

    pub fn from_deterministic_string(s: &str) -> Option<Self> {
        let s = s.trim();
        let obj = s.strip_prefix('{')?.strip_suffix('}')?;
        let id = extract_string_field(obj, "id")?;
        let tag = extract_string_field(obj, "tag")?;
        let node_type_match = extract_optional_string_field(obj, "node_type_match");
        let node_property_matches = extract_property_match_vec(obj, "node_property_matches");
        let specific_node_id = extract_optional_u64_field(obj, "specific_node_id");
        let edge_type_match = extract_optional_string_field(obj, "edge_type_match");
        let edge_property_matches = extract_property_match_vec(obj, "edge_property_matches");
        let specific_edge_id = extract_optional_u64_field(obj, "specific_edge_id");
        Some(RuleSpec { id, tag, node_type_match, node_property_matches, specific_node_id, edge_type_match, edge_property_matches, specific_edge_id })
    }
}

impl AnnotatedResultSet {
    pub fn to_deterministic_string(&self) -> String {
        format!(
            "{{\"result_set\":{},\"node_tags\":{},\"edge_tags\":{}}}",
            self.result_set.to_deterministic_string(),
            serialize_tag_map(&self.node_tags),
            serialize_tag_map(&self.edge_tags),
        )
    }

    pub fn from_deterministic_string(s: &str) -> Option<Self> {
        let s = s.trim();
        let obj = s.strip_prefix('{')?.strip_suffix('}')?;
        let result_set_str = crate::ontology::semantic::types::extract_field_raw(obj, "result_set")?;
        let node_tags_str = crate::ontology::semantic::types::extract_field_raw(obj, "node_tags")?;
        let edge_tags_str = crate::ontology::semantic::types::extract_field_raw(obj, "edge_tags")?;
        let result_set = ResultSet::from_deterministic_string(result_set_str)?;
        let node_tags = parse_tag_map(node_tags_str)?;
        let edge_tags = parse_tag_map(edge_tags_str)?;
        Some(AnnotatedResultSet { result_set, node_tags, edge_tags })
    }
}

// ── JSON parsing utilities ─────────────────────────────────────────────────

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

fn extract_optional_string_field(obj: &str, field: &str) -> Option<String> {
    let pattern = format!("\"{}\":", field);
    let start = obj.find(&pattern)?;
    let after_key = &obj[start + pattern.len()..];
    if after_key.trim_start().starts_with("null") {
        return None;
    }
    extract_string_field(obj, field)
}

fn extract_optional_u64_field(obj: &str, field: &str) -> Option<u64> {
    let pattern = format!("\"{}\":", field);
    let start = obj.find(&pattern)?;
    let after_key = &obj[start + pattern.len()..];
    if after_key.trim_start().starts_with("null") {
        return None;
    }
    let end = after_key.find(|c: char| c == ',' || c == '}').unwrap_or(after_key.len());
    after_key[..end].trim().parse().ok()
}

fn extract_property_match_vec(obj: &str, field: &str) -> Vec<PropertyMatch> {
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
            '}' => {
                depth -= 1;
                current.push(ch);
                if depth == 0 && !current.is_empty() {
                    if let Some(pm) = PropertyMatch::from_deterministic_string(&current) {
                        result.push(pm);
                    }
                    current.clear();
                }
            }
            ',' => { if depth == 0 { } else { current.push(ch); } }
            _ => { current.push(ch); }
        }
    }
    result
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

pub fn abi_contract() -> crate::abi::registry::AbiContract {
    crate::abi::registry::AbiContract::new(
        "RuleTypes",
        "1.0.0",
        &[
            "key", "value",
            "id", "tag", "node_type_match", "node_property_matches",
            "specific_node_id", "edge_type_match", "edge_property_matches",
            "specific_edge_id", "result_set", "node_tags", "edge_tags",
        ],
        &["QueryTypes"],
    )
}

fn parse_tag_map(s: &str) -> Option<BTreeMap<u64, Vec<String>>> {
    let s = s.trim();
    if s.is_empty() || s == "{}" {
        return Some(BTreeMap::new());
    }
    let inner = s.strip_prefix('{')?.strip_suffix('}')?;
    let inner = inner.trim();
    if inner.is_empty() {
        return Some(BTreeMap::new());
    }

    let mut result: BTreeMap<u64, Vec<String>> = BTreeMap::new();
    let bytes = inner.as_bytes();
    let len = inner.len();
    let mut pos = 0;

    while pos < len {
        // Skip whitespace and commas
        while pos < len && (bytes[pos] == b' ' || bytes[pos] == b',' || bytes[pos] == b'\t' || bytes[pos] == b'\n' || bytes[pos] == b'\r') {
            pos += 1;
        }
        if pos >= len { break; }

        // Expect '"' for key
        if bytes[pos] != b'"' { return None; }
        pos += 1;

        // Read key (node ID as string)
        let key_start = pos;
        while pos < len && bytes[pos] != b'"' {
            pos += 1;
        }
        if pos >= len { return None; }
        let key_str = std::str::from_utf8(&bytes[key_start..pos]).ok()?;
        pos += 1;

        // Expect ':'
        while pos < len && bytes[pos] == b' ' { pos += 1; }
        if pos >= len || bytes[pos] != b':' { return None; }
        pos += 1;

        // Skip whitespace
        while pos < len && bytes[pos] == b' ' { pos += 1; }

        // Expect '['
        if pos >= len || bytes[pos] != b'[' { return None; }
        pos += 1;

        // Parse array of strings
        let mut tags = Vec::new();
        while pos < len && bytes[pos] != b']' {
            // Skip whitespace and commas
            while pos < len && (bytes[pos] == b' ' || bytes[pos] == b',' || bytes[pos] == b'\t' || bytes[pos] == b'\n' || bytes[pos] == b'\r') {
                pos += 1;
            }
            if pos >= len { return None; }
            if bytes[pos] == b']' { break; }

            // Expect '"'
            if bytes[pos] != b'"' { return None; }
            pos += 1;

            // Read tag value with escape handling
            let mut tag = String::new();
            while pos < len && bytes[pos] != b'"' {
                if bytes[pos] == b'\\' && pos + 1 < len {
                    pos += 1;
                    match bytes[pos] {
                        b'"' => tag.push('"'),
                        b'\\' => tag.push('\\'),
                        b'n' => tag.push('\n'),
                        b't' => tag.push('\t'),
                        b'r' => tag.push('\r'),
                        c => { tag.push('\\'); tag.push(c as char); }
                    }
                } else {
                    tag.push(bytes[pos] as char);
                }
                pos += 1;
            }
            if pos >= len { return None; }
            pos += 1;
            tags.push(tag);
        }
        if pos >= len { return None; }
        pos += 1;

        let node_id: u64 = match key_str.parse() {
            Ok(n) => n,
            Err(_) => return None,
        };
        result.insert(node_id, tags);
    }

    Some(result)
}
