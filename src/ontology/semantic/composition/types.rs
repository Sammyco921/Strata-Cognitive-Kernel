use std::collections::{BTreeMap, BTreeSet};

use crate::ontology::semantic::query::types::{QuerySpec, ResultSet};
use crate::ontology::semantic::rules::types::RuleSpec;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PureTransform {
    pub name: String,
    pub parameters: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PipelineStep {
    QueryStep(QuerySpec),
    RuleStep(BTreeSet<RuleSpec>),
    TransformStep(PureTransform),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PipelineSpec {
    pub steps: Vec<PipelineStep>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct StepResult {
    pub index: usize,
    pub step_type: String,
    pub output: ResultSet,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct PipelineResult {
    pub steps: Vec<StepResult>,
    pub final_output: ResultSet,
}

// ── Serialization utilities ─────────────────────────────────────────────────

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

fn serialize_btreemap(map: &BTreeMap<String, String>) -> String {
    if map.is_empty() {
        return "{}".to_string();
    }
    let parts: Vec<String> = map.iter()
        .map(|(k, v)| format!("{}:{}", escape_json(k), escape_json(v)))
        .collect();
    format!("{{{}}}", parts.join(","))
}

fn serialize_ruleset(rules: &BTreeSet<RuleSpec>) -> String {
    let items: Vec<String> = rules.iter().map(|r| r.to_deterministic_string()).collect();
    format!("[{}]", items.join(","))
}

fn serialize_step_vec(steps: &[PipelineStep]) -> String {
    let items: Vec<String> = steps.iter().map(|s| s.to_deterministic_string()).collect();
    format!("[{}]", items.join(","))
}

// ── PipelineStep ────────────────────────────────────────────────────────────

impl PipelineStep {
    pub fn to_deterministic_string(&self) -> String {
        match self {
            PipelineStep::QueryStep(spec) => {
                format!("{{\"type\":\"query\",\"spec\":{}}}", spec.to_deterministic_string())
            }
            PipelineStep::RuleStep(rules) => {
                format!("{{\"type\":\"rule\",\"rules\":{}}}", serialize_ruleset(rules))
            }
            PipelineStep::TransformStep(t) => {
                format!(
                    "{{\"type\":\"transform\",\"name\":{},\"parameters\":{}}}",
                    escape_json(&t.name),
                    serialize_btreemap(&t.parameters),
                )
            }
        }
    }

    pub fn from_deterministic_string(s: &str) -> Option<Self> {
        let s = s.trim();
        let obj = s.strip_prefix('{')?.strip_suffix('}')?;
        let step_type = extract_string_field(obj, "type")?;
        match step_type.as_str() {
            "query" => {
                let spec_str = crate::ontology::semantic::types::extract_field_raw(obj, "spec")?;
                let spec = QuerySpec::from_deterministic_string(spec_str)?;
                Some(PipelineStep::QueryStep(spec))
            }
            "rule" => {
                let rules_str = crate::ontology::semantic::types::extract_field_raw(obj, "rules")?;
                let rules = parse_ruleset(rules_str)?;
                Some(PipelineStep::RuleStep(rules))
            }
            "transform" => {
                let name = extract_string_field(obj, "name")?;
                let params_str = crate::ontology::semantic::types::extract_field_raw(obj, "parameters")?;
                let parameters = parse_btreemap(params_str)?;
                Some(PipelineStep::TransformStep(PureTransform { name, parameters }))
            }
            _ => None,
        }
    }
}

// ── PipelineSpec ────────────────────────────────────────────────────────────

impl PipelineSpec {
    pub fn to_deterministic_string(&self) -> String {
        format!("{{\"steps\":{}}}", serialize_step_vec(&self.steps))
    }

    pub fn from_deterministic_string(s: &str) -> Option<Self> {
        let s = s.trim();
        let obj = s.strip_prefix('{')?.strip_suffix('}')?;
        let steps_str = crate::ontology::semantic::types::extract_field_raw(obj, "steps")?;
        let steps = parse_step_vec(steps_str)?;
        Some(PipelineSpec { steps })
    }
}

// ── StepResult ──────────────────────────────────────────────────────────────

impl StepResult {
    pub fn to_deterministic_string(&self) -> String {
        format!(
            "{{\"index\":{},\"step_type\":{},\"output\":{}}}",
            self.index,
            escape_json(&self.step_type),
            self.output.to_deterministic_string(),
        )
    }

    pub fn from_deterministic_string(s: &str) -> Option<Self> {
        let s = s.trim();
        let obj = s.strip_prefix('{')?.strip_suffix('}')?;
        let index = extract_usize_field(obj, "index")?;
        let step_type = extract_string_field(obj, "step_type")?;
        let output_str = crate::ontology::semantic::types::extract_field_raw(obj, "output")?;
        let output = ResultSet::from_deterministic_string(output_str)?;
        Some(StepResult { index, step_type, output })
    }
}

// ── PipelineResult ──────────────────────────────────────────────────────────

impl PipelineResult {
    pub fn to_deterministic_string(&self) -> String {
        let steps_str: Vec<String> = self.steps.iter().map(|s| s.to_deterministic_string()).collect();
        format!(
            "{{\"steps\":[{}],\"final_output\":{}}}",
            steps_str.join(","),
            self.final_output.to_deterministic_string(),
        )
    }

    pub fn from_deterministic_string(s: &str) -> Option<Self> {
        let s = s.trim();
        let obj = s.strip_prefix('{')?.strip_suffix('}')?;
        let steps_str = crate::ontology::semantic::types::extract_field_raw(obj, "steps")?;
        let final_output_str = crate::ontology::semantic::types::extract_field_raw(obj, "final_output")?;
        let steps = parse_step_result_vec(steps_str)?;
        let final_output = ResultSet::from_deterministic_string(final_output_str)?;
        Some(PipelineResult { steps, final_output })
    }
}

// ── JSON parsing helpers ────────────────────────────────────────────────────

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

fn extract_usize_field(obj: &str, field: &str) -> Option<usize> {
    let pattern = format!("\"{}\":", field);
    let start = obj.find(&pattern)?;
    let rest = &obj[start + pattern.len()..];
    let end = rest.find(|c: char| c == ',' || c == '}').unwrap_or(rest.len());
    rest[..end].trim().parse().ok()
}

fn parse_ruleset(s: &str) -> Option<BTreeSet<RuleSpec>> {
    let s = s.trim();
    if !s.starts_with('[') || !s.ends_with(']') {
        return None;
    }
    let inner = s[1..s.len()-1].trim();
    if inner.is_empty() {
        return Some(BTreeSet::new());
    }
    let mut result = BTreeSet::new();
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    let mut current = String::new();
    for ch in inner.chars() {
        if escaped { escaped = false; current.push(ch); continue; }
        if ch == '\\' { escaped = true; current.push(ch); continue; }
        if ch == '"' { in_string = !in_string; current.push(ch); continue; }
        if in_string { current.push(ch); continue; }
        match ch {
            '{' => { depth += 1; current.push(ch); }
            '}' => {
                depth -= 1;
                current.push('}');
                if depth == 0 {
                    if let Some(rule) = RuleSpec::from_deterministic_string(&current) {
                        result.insert(rule);
                    }
                    current.clear();
                }
            }
            ',' => { if depth == 0 { } else { current.push(ch); } }
            _ => { current.push(ch); }
        }
    }
    Some(result)
}

fn parse_btreemap(s: &str) -> Option<BTreeMap<String, String>> {
    let s = s.trim();
    if s.is_empty() || s == "{}" {
        return Some(BTreeMap::new());
    }
    let inner = s.strip_prefix('{')?.strip_suffix('}')?;
    let inner = inner.trim();
    if inner.is_empty() {
        return Some(BTreeMap::new());
    }
    let mut map = BTreeMap::new();
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
        if ch == ':' && parsing_key {
            current_key = Some(current_val.clone());
            current_val.clear();
            parsing_key = false;
            continue;
        }
        if ch == ',' || ch == '}' {
            if let Some(ref key) = current_key {
                map.insert(key.clone(), current_val.trim().trim_matches('"').to_string());
            }
            current_val.clear();
            current_key = None;
            parsing_key = true;
            if ch == '}' { break; }
            continue;
        }
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

fn parse_step_vec(s: &str) -> Option<Vec<PipelineStep>> {
    let s = s.trim();
    if !s.starts_with('[') || !s.ends_with(']') {
        return None;
    }
    let inner = s[1..s.len()-1].trim();
    if inner.is_empty() {
        return Some(Vec::new());
    }
    let mut result = Vec::new();
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    let mut current = String::new();
    let mut in_bracket = false;
    for ch in inner.chars() {
        if escaped { escaped = false; current.push(ch); continue; }
        if ch == '\\' { escaped = true; current.push(ch); continue; }
        if ch == '"' { in_string = !in_string; current.push(ch); continue; }
        if in_string { current.push(ch); continue; }
        match ch {
            '{' => { depth += 1; current.push(ch); }
            '}' => {
                depth -= 1;
                current.push('}');
                if depth == 0 {
                    if let Some(step) = PipelineStep::from_deterministic_string(&current) {
                        result.push(step);
                    }
                    current.clear();
                }
            }
            '[' => { in_bracket = true; current.push(ch); }
            ']' => { in_bracket = false; current.push(ch); }
            ',' => { if depth == 0 && !in_bracket { } else { current.push(ch); } }
            _ => { current.push(ch); }
        }
    }
    Some(result)
}

pub fn abi_contract() -> crate::abi::registry::AbiContract {
    crate::abi::registry::AbiContract::new(
        "CompositionTypes",
        "1.0.0",
        &[
            "steps", "final_output", "type", "spec", "rules",
            "name", "parameters", "index", "step_type", "output",
        ],
        &["RuleTypes", "QueryTypes"],
    )
}

fn parse_step_result_vec(s: &str) -> Option<Vec<StepResult>> {
    let s = s.trim();
    if !s.starts_with('[') || !s.ends_with(']') {
        return None;
    }
    let inner = s[1..s.len()-1].trim();
    if inner.is_empty() {
        return Some(Vec::new());
    }
    let mut result = Vec::new();
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    let mut current = String::new();
    for ch in inner.chars() {
        if escaped { escaped = false; current.push(ch); continue; }
        if ch == '\\' { escaped = true; current.push(ch); continue; }
        if ch == '"' { in_string = !in_string; current.push(ch); continue; }
        if in_string { current.push(ch); continue; }
        match ch {
            '{' => { depth += 1; current.push(ch); }
            '}' => {
                depth -= 1;
                current.push('}');
                if depth == 0 {
                    if let Some(sr) = StepResult::from_deterministic_string(&current) {
                        result.push(sr);
                    }
                    current.clear();
                }
            }
            ',' => { if depth == 0 { } else { current.push(ch); } }
            _ => { current.push(ch); }
        }
    }
    Some(result)
}
