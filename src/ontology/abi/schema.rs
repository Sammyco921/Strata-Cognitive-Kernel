#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaDescriptor {
    pub schema_id: &'static str,
    pub version: &'static str,
    pub field_count: usize,
    pub field_names: &'static [&'static str],
}

pub const QUERY_SPEC_SCHEMA: SchemaDescriptor = SchemaDescriptor {
    schema_id: "QuerySpec",
    version: "1.0.0",
    field_count: 6,
    field_names: &[
        "node_type_filter",
        "edge_type_filter",
        "property_filters",
        "traversal_depth",
        "source_node_ids",
        "target_node_ids",
    ],
};

pub const RESULT_SET_SCHEMA: SchemaDescriptor = SchemaDescriptor {
    schema_id: "ResultSet",
    version: "1.0.0",
    field_count: 2,
    field_names: &["nodes", "edges"],
};

pub const RULE_SPEC_SCHEMA: SchemaDescriptor = SchemaDescriptor {
    schema_id: "RuleSpec",
    version: "1.0.0",
    field_count: 8,
    field_names: &[
        "id",
        "tag",
        "node_type_match",
        "node_property_matches",
        "specific_node_id",
        "edge_type_match",
        "edge_property_matches",
        "specific_edge_id",
    ],
};

pub const ANNOTATED_RESULT_SET_SCHEMA: SchemaDescriptor = SchemaDescriptor {
    schema_id: "AnnotatedResultSet",
    version: "1.0.0",
    field_count: 3,
    field_names: &["result_set", "node_tags", "edge_tags"],
};

pub const PIPELINE_SPEC_SCHEMA: SchemaDescriptor = SchemaDescriptor {
    schema_id: "PipelineSpec",
    version: "1.0.0",
    field_count: 1,
    field_names: &["steps"],
};

pub const PIPELINE_RESULT_SCHEMA: SchemaDescriptor = SchemaDescriptor {
    schema_id: "PipelineResult",
    version: "1.0.0",
    field_count: 2,
    field_names: &["steps", "final_output"],
};

pub fn get_schema(schema_id: &str) -> Option<&'static SchemaDescriptor> {
    match schema_id {
        "QuerySpec" => Some(&QUERY_SPEC_SCHEMA),
        "ResultSet" => Some(&RESULT_SET_SCHEMA),
        "RuleSpec" => Some(&RULE_SPEC_SCHEMA),
        "AnnotatedResultSet" => Some(&ANNOTATED_RESULT_SET_SCHEMA),
        "PipelineSpec" => Some(&PIPELINE_SPEC_SCHEMA),
        "PipelineResult" => Some(&PIPELINE_RESULT_SCHEMA),
        _ => None,
    }
}

pub fn validate_schema_id(schema_id: &str) -> Result<&'static SchemaDescriptor, String> {
    get_schema(schema_id).ok_or_else(|| format!("Unknown schema ID: {}", schema_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_known_schemas() {
        assert!(get_schema("QuerySpec").is_some());
        assert!(get_schema("ResultSet").is_some());
        assert!(get_schema("RuleSpec").is_some());
        assert!(get_schema("AnnotatedResultSet").is_some());
        assert!(get_schema("PipelineSpec").is_some());
        assert!(get_schema("PipelineResult").is_some());
    }

    #[test]
    fn test_unknown_schema() {
        assert!(get_schema("UnknownType").is_none());
        assert!(get_schema("").is_none());
    }

    #[test]
    fn test_all_schema_ids_match_descriptor() {
        for sid in &[
            "QuerySpec", "ResultSet", "RuleSpec",
            "AnnotatedResultSet", "PipelineSpec", "PipelineResult",
        ] {
            let schema = get_schema(sid).unwrap();
            assert_eq!(schema.schema_id, *sid);
        }
    }

    #[test]
    fn test_field_counts_consistent() {
        assert_eq!(QUERY_SPEC_SCHEMA.field_count, QUERY_SPEC_SCHEMA.field_names.len());
        assert_eq!(RESULT_SET_SCHEMA.field_count, RESULT_SET_SCHEMA.field_names.len());
        assert_eq!(RULE_SPEC_SCHEMA.field_count, RULE_SPEC_SCHEMA.field_names.len());
        assert_eq!(ANNOTATED_RESULT_SET_SCHEMA.field_count, ANNOTATED_RESULT_SET_SCHEMA.field_names.len());
        assert_eq!(PIPELINE_SPEC_SCHEMA.field_count, PIPELINE_SPEC_SCHEMA.field_names.len());
        assert_eq!(PIPELINE_RESULT_SCHEMA.field_count, PIPELINE_RESULT_SCHEMA.field_names.len());
    }

    #[test]
    fn test_validate_schema_id_known() {
        assert!(validate_schema_id("QuerySpec").is_ok());
    }

    #[test]
    fn test_validate_schema_id_unknown() {
        assert!(validate_schema_id("Nonexistent").is_err());
    }
}
