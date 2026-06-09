use crate::ontology::abi::version::{ABI_VERSION, validate_version};
use crate::ontology::abi::schema::{validate_schema_id};

use crate::ontology::semantic::query::types::{QuerySpec, ResultSet};
use crate::ontology::semantic::rules::types::{RuleSpec, AnnotatedResultSet};
use crate::ontology::semantic::composition::types::{PipelineSpec, PipelineResult};

// ── Error type ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AbiError {
    VersionMismatch(String),
    UnknownSchema(String),
    ParseError(String),
    ValidationError(String),
}

// ── Serialization traits ────────────────────────────────────────────────────

pub trait ToAbiString {
    fn to_abi_string(&self) -> String;
}

pub trait FromAbiString: Sized {
    fn from_abi_string(s: &str) -> Option<Self>;
}

// ── Trait implementations for all ABI-bound types ───────────────────────────

impl ToAbiString for QuerySpec {
    fn to_abi_string(&self) -> String {
        self.to_deterministic_string()
    }
}

impl FromAbiString for QuerySpec {
    fn from_abi_string(s: &str) -> Option<Self> {
        QuerySpec::from_deterministic_string(s)
    }
}

impl ToAbiString for ResultSet {
    fn to_abi_string(&self) -> String {
        self.to_deterministic_string()
    }
}

impl FromAbiString for ResultSet {
    fn from_abi_string(s: &str) -> Option<Self> {
        ResultSet::from_deterministic_string(s)
    }
}

impl ToAbiString for RuleSpec {
    fn to_abi_string(&self) -> String {
        self.to_deterministic_string()
    }
}

impl FromAbiString for RuleSpec {
    fn from_abi_string(s: &str) -> Option<Self> {
        RuleSpec::from_deterministic_string(s)
    }
}

impl ToAbiString for AnnotatedResultSet {
    fn to_abi_string(&self) -> String {
        self.to_deterministic_string()
    }
}

impl FromAbiString for AnnotatedResultSet {
    fn from_abi_string(s: &str) -> Option<Self> {
        AnnotatedResultSet::from_deterministic_string(s)
    }
}

impl ToAbiString for PipelineSpec {
    fn to_abi_string(&self) -> String {
        self.to_deterministic_string()
    }
}

impl FromAbiString for PipelineSpec {
    fn from_abi_string(s: &str) -> Option<Self> {
        PipelineSpec::from_deterministic_string(s)
    }
}

impl ToAbiString for PipelineResult {
    fn to_abi_string(&self) -> String {
        self.to_deterministic_string()
    }
}

impl FromAbiString for PipelineResult {
    fn from_abi_string(s: &str) -> Option<Self> {
        PipelineResult::from_deterministic_string(s)
    }
}

// ── ABI Envelope ────────────────────────────────────────────────────────────

pub struct AbiEnvelope;

impl AbiEnvelope {
    /// Serialize any ABI-bound type with ABI metadata.
    /// Returns a JSON string with abi_version, schema_id, and data fields.
    pub fn serialize<T: ToAbiString>(data: &T, schema_id: &str) -> String {
        format!(
            "{{\"abi_version\":{},\"schema_id\":{},\"data\":{}}}",
            Self::escape_json(ABI_VERSION),
            Self::escape_json(schema_id),
            data.to_abi_string(),
        )
    }

    /// Deserialize and validate an ABI-bound type.
    /// Checks version compatibility and schema_id match.
    pub fn deserialize<T: FromAbiString>(s: &str, expected_schema_id: &str) -> Result<T, AbiError> {
        Self::validate_envelope(s, expected_schema_id)?;

        let obj = Self::extract_object(s)
            .ok_or_else(|| AbiError::ParseError("Invalid JSON envelope".into()))?;

        let data_str = Self::extract_field_raw(obj, "data")
            .ok_or_else(|| AbiError::ParseError("Missing 'data' field in envelope".into()))?;

        T::from_abi_string(data_str)
            .ok_or_else(|| AbiError::ParseError("Failed to parse data payload".into()))
    }

    /// Validate ABI compatibility without deserializing the payload.
    /// Checks: JSON structure, known schema, version compat, schema_id match.
    pub fn validate_envelope(s: &str, expected_schema_id: &str) -> Result<(), AbiError> {
        let obj = Self::extract_object(s)
            .ok_or_else(|| AbiError::ParseError("Invalid JSON envelope".into()))?;

        let abi_version = Self::extract_string_field(obj, "abi_version")
            .ok_or_else(|| AbiError::ParseError("Missing 'abi_version' in envelope".into()))?;
        let schema_id = Self::extract_string_field(obj, "schema_id")
            .ok_or_else(|| AbiError::ParseError("Missing 'schema_id' in envelope".into()))?;

        // First: check schema_id is known
        let schema = validate_schema_id(&schema_id)
            .map_err(|e| AbiError::UnknownSchema(e))?;

        // Second: check version compatibility
        validate_version(schema.version, &abi_version)
            .map_err(|e| AbiError::VersionMismatch(e))?;

        // Third: check schema_id matches expected
        if schema_id != expected_schema_id {
            return Err(AbiError::ValidationError(format!(
                "Schema ID mismatch: expected '{}', got '{}'",
                expected_schema_id, schema_id
            )));
        }

        Ok(())
    }

    /// Check whether a version string is compatible with the current ABI major.
    pub fn is_abi_compatible(version: &str) -> bool {
        crate::ontology::abi::version::is_compatible(ABI_VERSION, version)
    }

    // ── Convenience methods ──────────────────────────────────────────────

    pub fn serialize_query_spec(spec: &QuerySpec) -> String {
        Self::serialize(spec, "QuerySpec")
    }

    pub fn deserialize_query_spec(s: &str) -> Result<QuerySpec, AbiError> {
        Self::deserialize(s, "QuerySpec")
    }

    pub fn serialize_result_set(rs: &ResultSet) -> String {
        Self::serialize(rs, "ResultSet")
    }

    pub fn deserialize_result_set(s: &str) -> Result<ResultSet, AbiError> {
        Self::deserialize(s, "ResultSet")
    }

    pub fn serialize_rule_spec(spec: &RuleSpec) -> String {
        Self::serialize(spec, "RuleSpec")
    }

    pub fn deserialize_rule_spec(s: &str) -> Result<RuleSpec, AbiError> {
        Self::deserialize(s, "RuleSpec")
    }

    pub fn serialize_annotated_result_set(ars: &AnnotatedResultSet) -> String {
        Self::serialize(ars, "AnnotatedResultSet")
    }

    pub fn deserialize_annotated_result_set(s: &str) -> Result<AnnotatedResultSet, AbiError> {
        Self::deserialize(s, "AnnotatedResultSet")
    }

    pub fn serialize_pipeline_spec(spec: &PipelineSpec) -> String {
        Self::serialize(spec, "PipelineSpec")
    }

    pub fn deserialize_pipeline_spec(s: &str) -> Result<PipelineSpec, AbiError> {
        Self::deserialize(s, "PipelineSpec")
    }

    pub fn serialize_pipeline_result(pr: &PipelineResult) -> String {
        Self::serialize(pr, "PipelineResult")
    }

    pub fn deserialize_pipeline_result(s: &str) -> Result<PipelineResult, AbiError> {
        Self::deserialize(s, "PipelineResult")
    }

    // ── JSON utilities ──────────────────────────────────────────────────

    fn extract_object(s: &str) -> Option<&str> {
        let s = s.trim();
        s.strip_prefix('{').and_then(|o| o.strip_suffix('}'))
    }

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
            '{' => Self::find_matching_brace(trimmed)?,
            '[' => Self::find_matching_bracket(trimmed)?,
            '"' => {
                let mut i = 1;
                let bytes = trimmed.as_bytes();
                while i < trimmed.len() {
                    if bytes[i] == b'"' && bytes[i - 1] != b'\\' {
                        return Some(&trimmed[..=i]);
                    }
                    i += 1;
                }
                return None;
            }
            _ => {
                let end = trimmed.find(|c: char| c == ',' || c == '}' || c == ']')
                    .unwrap_or(trimmed.len());
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ontology::abi::schema::SchemaDescriptor;
    use std::collections::{BTreeMap, BTreeSet};

    // ── Helper: build minimal QuerySpec ──────────────────────────────────

    fn make_query_spec() -> QuerySpec {
        QuerySpec {
            node_type_filter: Some(vec!["person".into()]),
            edge_type_filter: None,
            property_filters: Vec::new(),
            traversal_depth: None,
            source_node_ids: None,
            target_node_ids: None,
        }
    }

    fn make_result_set() -> ResultSet {
        ResultSet { nodes: BTreeMap::new(), edges: BTreeMap::new() }
    }

    fn make_rule_spec() -> RuleSpec {
        RuleSpec {
            id: "r1".into(), tag: "t".into(),
            node_type_match: Some("person".into()),
            node_property_matches: Vec::new(),
            specific_node_id: None,
            edge_type_match: None,
            edge_property_matches: Vec::new(),
            specific_edge_id: None,
        }
    }

    fn make_annotated_result_set() -> AnnotatedResultSet {
        AnnotatedResultSet {
            result_set: make_result_set(),
            node_tags: BTreeMap::new(),
            edge_tags: BTreeMap::new(),
        }
    }

    fn make_pipeline_spec() -> PipelineSpec {
        PipelineSpec { steps: Vec::new() }
    }

    fn make_pipeline_result() -> PipelineResult {
        PipelineResult {
            steps: Vec::new(),
            final_output: make_result_set(),
        }
    }

    // ── Test 1: ABI version embedding correctness ────────────────────────

    #[test]
    fn test_abi_version_embedded() {
        let json = AbiEnvelope::serialize_query_spec(&make_query_spec());
        assert!(json.contains("\"abi_version\":\"1.0.0\""));
        assert!(json.contains("\"schema_id\":\"QuerySpec\""));
    }

    #[test]
    fn test_all_types_embed_abi() {
        for (json, expected_sid) in [
            (AbiEnvelope::serialize_query_spec(&make_query_spec()), "QuerySpec"),
            (AbiEnvelope::serialize_result_set(&make_result_set()), "ResultSet"),
            (AbiEnvelope::serialize_rule_spec(&make_rule_spec()), "RuleSpec"),
            (AbiEnvelope::serialize_annotated_result_set(&make_annotated_result_set()), "AnnotatedResultSet"),
            (AbiEnvelope::serialize_pipeline_spec(&make_pipeline_spec()), "PipelineSpec"),
            (AbiEnvelope::serialize_pipeline_result(&make_pipeline_result()), "PipelineResult"),
        ].iter() {
            assert!(json.contains("\"abi_version\":\"1.0.0\""));
            assert!(json.contains(&format!("\"schema_id\":\"{}\"", expected_sid)));
        }
    }

    // ── Test 2: Deterministic serialization includes ABI fields ──────────

    #[test]
    fn test_deterministic_serialization_with_abi() {
        let spec = make_query_spec();
        let s1 = AbiEnvelope::serialize_query_spec(&spec);
        let s2 = AbiEnvelope::serialize_query_spec(&spec);
        assert_eq!(s1, s2);
    }

    // ── Test 3: Version mismatch rejection ───────────────────────────────

    #[test]
    fn test_version_mismatch_rejected() {
        // Create an envelope with a wrong version
        let obj = format!(
            "{{\"abi_version\":\"2.0.0\",\"schema_id\":\"QuerySpec\",\"data\":{}}}",
            make_query_spec().to_deterministic_string()
        );
        let result = AbiEnvelope::deserialize_query_spec(&obj);
        assert!(result.is_err());
        match result {
            Err(AbiError::VersionMismatch(_)) => {} // expected
            Err(e) => panic!("Expected VersionMismatch, got: {:?}", e),
            Ok(_) => panic!("Expected error, got Ok"),
        }
    }

    #[test]
    fn test_missing_abi_version_rejected() {
        let obj = format!(
            "{{\"schema_id\":\"QuerySpec\",\"data\":{}}}",
            make_query_spec().to_deterministic_string()
        );
        let result = AbiEnvelope::deserialize_query_spec(&obj);
        assert!(result.is_err());
    }

    // ── Test 4: Unknown schema rejection ─────────────────────────────────

    #[test]
    fn test_unknown_schema_rejected() {
        let obj = format!(
            "{{\"abi_version\":\"1.0.0\",\"schema_id\":\"UnknownType\",\"data\":{{}}}}"
        );
        let result: Result<QuerySpec, AbiError> = AbiEnvelope::deserialize(&obj, "QuerySpec");
        assert!(result.is_err());
        match result {
            Err(AbiError::UnknownSchema(_)) => {} // expected
            _ => panic!("Expected UnknownSchema error"),
        }
    }

    // ── Test 5: Schema ID mismatch rejection ─────────────────────────────

    #[test]
    fn test_schema_id_mismatch_rejected() {
        let obj = format!(
            "{{\"abi_version\":\"1.0.0\",\"schema_id\":\"ResultSet\",\"data\":{{}}}}"
        );
        let result: Result<QuerySpec, AbiError> = AbiEnvelope::deserialize(&obj, "QuerySpec");
        assert!(result.is_err());
        match result {
            Err(AbiError::ValidationError(_)) => {} // expected
            _ => panic!("Expected ValidationError"),
        }
    }

    // ── Test 6: Roundtrip stability under identical ABI ──────────────────

    #[test]
    fn test_roundtrip_query_spec() {
        let spec = make_query_spec();
        let json = AbiEnvelope::serialize_query_spec(&spec);
        let back = AbiEnvelope::deserialize_query_spec(&json).unwrap();
        assert_eq!(spec, back);
    }

    #[test]
    fn test_roundtrip_result_set() {
        let rs = make_result_set();
        let json = AbiEnvelope::serialize_result_set(&rs);
        let back = AbiEnvelope::deserialize_result_set(&json).unwrap();
        assert_eq!(rs, back);
    }

    #[test]
    fn test_roundtrip_rule_spec() {
        let spec = make_rule_spec();
        let json = AbiEnvelope::serialize_rule_spec(&spec);
        let back = AbiEnvelope::deserialize_rule_spec(&json).unwrap();
        assert_eq!(spec, back);
    }

    #[test]
    fn test_roundtrip_annotated_result_set() {
        let ars = make_annotated_result_set();
        let json = AbiEnvelope::serialize_annotated_result_set(&ars);
        let back = AbiEnvelope::deserialize_annotated_result_set(&json).unwrap();
        assert_eq!(ars, back);
    }

    #[test]
    fn test_roundtrip_pipeline_spec() {
        let spec = make_pipeline_spec();
        let json = AbiEnvelope::serialize_pipeline_spec(&spec);
        let back = AbiEnvelope::deserialize_pipeline_spec(&json).unwrap();
        assert_eq!(spec, back);
    }

    #[test]
    fn test_roundtrip_pipeline_result() {
        let pr = make_pipeline_result();
        let json = AbiEnvelope::serialize_pipeline_result(&pr);
        let back = AbiEnvelope::deserialize_pipeline_result(&json).unwrap();
        assert_eq!(pr, back);
    }

    // ── Test 7: Minor version compatibility ──────────────────────────────

    #[test]
    fn test_minor_version_accepted() {
        // Create an envelope with version 1.5.0 (same major, different minor)
        let obj = format!(
            "{{\"abi_version\":\"1.5.0\",\"schema_id\":\"QuerySpec\",\"data\":{}}}",
            make_query_spec().to_deterministic_string()
        );
        let result = AbiEnvelope::deserialize_query_spec(&obj);
        assert!(result.is_ok());
    }

    // ── Test 8: Major version incompatibility ────────────────────────────

    #[test]
    fn test_major_version_incompatibility() {
        let obj = format!(
            "{{\"abi_version\":\"2.0.0\",\"schema_id\":\"QuerySpec\",\"data\":{}}}",
            make_query_spec().to_deterministic_string()
        );
        let result = AbiEnvelope::deserialize_query_spec(&obj);
        assert!(result.is_err());
    }

    #[test]
    fn test_major_zero_incompatible_with_major_one() {
        let obj = format!(
            "{{\"abi_version\":\"0.9.0\",\"schema_id\":\"QuerySpec\",\"data\":{}}}",
            make_query_spec().to_deterministic_string()
        );
        let result = AbiEnvelope::deserialize_query_spec(&obj);
        assert!(result.is_err());
    }

    // ── Test 9: Cross-type ABI consistency ───────────────────────────────

    #[test]
    fn test_all_types_use_same_abi_version() {
        let specs = [
            AbiEnvelope::serialize_query_spec(&make_query_spec()),
            AbiEnvelope::serialize_result_set(&make_result_set()),
            AbiEnvelope::serialize_rule_spec(&make_rule_spec()),
            AbiEnvelope::serialize_annotated_result_set(&make_annotated_result_set()),
            AbiEnvelope::serialize_pipeline_spec(&make_pipeline_spec()),
            AbiEnvelope::serialize_pipeline_result(&make_pipeline_result()),
        ];

        for json in &specs {
            assert!(json.contains("\"abi_version\":\"1.0.0\""));
        }
    }

    // ── Test 10: Validate envelope (without deserializing) ───────────────

    #[test]
    fn test_validate_envelope_ok() {
        let json = AbiEnvelope::serialize_query_spec(&make_query_spec());
        assert!(AbiEnvelope::validate_envelope(&json, "QuerySpec").is_ok());
    }

    #[test]
    fn test_validate_envelope_wrong_schema() {
        let json = AbiEnvelope::serialize_query_spec(&make_query_spec());
        assert!(AbiEnvelope::validate_envelope(&json, "ResultSet").is_err());
    }

    #[test]
    fn test_validate_envelope_wrong_version() {
        let obj = format!(
            "{{\"abi_version\":\"3.0.0\",\"schema_id\":\"QuerySpec\",\"data\":{{}}}}"
        );
        assert!(AbiEnvelope::validate_envelope(&obj, "QuerySpec").is_err());
    }

    // ── Test 11: is_abi_compatible ───────────────────────────────────────

    #[test]
    fn test_is_abi_compatible() {
        assert!(AbiEnvelope::is_abi_compatible("1.99.99"));
        assert!(!AbiEnvelope::is_abi_compatible("2.0.0"));
        assert!(!AbiEnvelope::is_abi_compatible("0.0.1"));
    }

    // ── Test 12: 100+ run stability ──────────────────────────────────────

    #[test]
    fn test_stability_100_runs() {
        let spec = make_query_spec();
        let first = AbiEnvelope::serialize_query_spec(&spec);
        for _ in 0..100 {
            let json = AbiEnvelope::serialize_query_spec(&spec);
            assert_eq!(first, json);
            let back: QuerySpec = AbiEnvelope::deserialize_query_spec(&json).unwrap();
            assert_eq!(spec, back);
        }
    }

    // ── Test 13: Invalid JSON envelope ───────────────────────────────────

    #[test]
    fn test_invalid_json_rejected() {
        let result = AbiEnvelope::deserialize_query_spec("not json");
        assert!(result.is_err());
    }

    #[test]
    fn test_empty_string_rejected() {
        let result = AbiEnvelope::deserialize_query_spec("");
        assert!(result.is_err());
    }

    // ── Schema drift detection ─────────────────────────────────────────────
    // These tests detect when a type's serialization drifts from its schema
    // descriptor (e.g., fields added/removed/renamed without updating the
    // descriptor). They extract top-level field names from the serialized
    // output and verify all descriptor field_names are present.

    fn extract_json_field_names(s: &str) -> BTreeSet<String> {
        let s = s.trim();
        let body = s.strip_prefix('{').and_then(|s| s.strip_suffix('}')).unwrap_or(s);
        let mut fields = BTreeSet::new();
        let mut depth: i32 = 0;
        let mut in_string = false;
        let mut escaped = false;
        let mut key = String::new();
        let mut parsing_key = true;

        for ch in body.chars() {
            if escaped {
                escaped = false;
                if parsing_key { key.push(ch); }
                continue;
            }
            if ch == '\\' { escaped = true; continue; }
            if ch == '"' {
                in_string = !in_string;
                if in_string && parsing_key { key.clear(); }
                continue;
            }
            if in_string {
                if parsing_key { key.push(ch); }
                continue;
            }
            match ch {
                ':' if parsing_key && depth == 0 => {
                    if !key.is_empty() { fields.insert(key.clone()); }
                    parsing_key = false;
                }
                ',' if depth == 0 => { parsing_key = true; key.clear(); }
                '{' | '[' => depth += 1,
                '}' | ']' => depth -= 1,
                _ => {}
            }
        }
        fields
    }

    fn check_schema_drift(json: &str, descriptor: &SchemaDescriptor) {
        let fields = extract_json_field_names(json);
        for expected in descriptor.field_names {
            assert!(
                fields.contains(*expected),
                "Schema drift detected for '{}': field '{}' not found in serialized output. \
                 Descriptor fields: {:?}, actual fields: {:?}",
                descriptor.schema_id, expected, descriptor.field_names, fields
            );
        }
        assert_eq!(
            fields.len(),
            descriptor.field_names.len(),
            "Schema drift detected for '{}': field count mismatch. Expected {} fields ({:?}), \
             got {} fields ({:?})",
            descriptor.schema_id,
            descriptor.field_names.len(),
            descriptor.field_names,
            fields.len(),
            fields
        );
    }

    #[test]
    fn test_query_spec_schema_drift() {
        check_schema_drift(
            &make_query_spec().to_deterministic_string(),
            &crate::ontology::abi::schema::QUERY_SPEC_SCHEMA,
        );
    }

    #[test]
    fn test_result_set_schema_drift() {
        check_schema_drift(
            &make_result_set().to_deterministic_string(),
            &crate::ontology::abi::schema::RESULT_SET_SCHEMA,
        );
    }

    #[test]
    fn test_rule_spec_schema_drift() {
        check_schema_drift(
            &make_rule_spec().to_deterministic_string(),
            &crate::ontology::abi::schema::RULE_SPEC_SCHEMA,
        );
    }

    #[test]
    fn test_annotated_result_set_schema_drift() {
        check_schema_drift(
            &make_annotated_result_set().to_deterministic_string(),
            &crate::ontology::abi::schema::ANNOTATED_RESULT_SET_SCHEMA,
        );
    }

    #[test]
    fn test_pipeline_spec_schema_drift() {
        check_schema_drift(
            &make_pipeline_spec().to_deterministic_string(),
            &crate::ontology::abi::schema::PIPELINE_SPEC_SCHEMA,
        );
    }

    #[test]
    fn test_pipeline_result_schema_drift() {
        check_schema_drift(
            &make_pipeline_result().to_deterministic_string(),
            &crate::ontology::abi::schema::PIPELINE_RESULT_SCHEMA,
        );
    }
}
