/// Command descriptor — used by `list-commands` and `describe` for
/// self-describing CLI introspection.

/// Describes a single CLI command: its name, category, purpose, inputs, and output.
pub struct CommandDescriptor {
    pub name: &'static str,
    pub category: &'static str,
    pub summary: &'static str,
    pub inputs: &'static [InputField],
    pub output: &'static str,
}

pub struct InputField {
    pub name: &'static str,
    pub kind: &'static str,
    pub optional: bool,
    pub description: &'static str,
}

/// All commands, grouped by category, in display order.
pub fn all_commands() -> &'static [CommandDescriptor] {
    &ALL_COMMANDS
}

static ALL_COMMANDS: &[CommandDescriptor] = &[
        // ── State operations ─────────────────────────────────────────────
        CommandDescriptor {
            name: "create-node",
            category: "State",
            summary: "Create a new node in the graph",
            inputs: &[InputField {
                name: "id",
                kind: "string",
                optional: false,
                description: "Unique node identifier",
            }],
            output: "Confirmation message on success; error if node already exists",
        },
        CommandDescriptor {
            name: "create-edge",
            category: "State",
            summary: "Create a directed edge between two nodes",
            inputs: &[
                InputField {
                    name: "id",
                    kind: "string",
                    optional: false,
                    description: "Unique edge identifier",
                },
                InputField {
                    name: "from",
                    kind: "string",
                    optional: false,
                    description: "Source node ID",
                },
                InputField {
                    name: "to",
                    kind: "string",
                    optional: false,
                    description: "Target node ID",
                },
                InputField {
                    name: "type",
                    kind: "string",
                    optional: false,
                    description: "Edge type label",
                },
            ],
            output: "Confirmation message on success; error if node missing or edge exists",
        },
        CommandDescriptor {
            name: "delete-node",
            category: "State",
            summary: "Delete a node and all its incident edges",
            inputs: &[InputField {
                name: "id",
                kind: "string",
                optional: false,
                description: "Node ID to delete",
            }],
            output: "Confirmation message; error if node does not exist",
        },
        CommandDescriptor {
            name: "delete-edge",
            category: "State",
            summary: "Delete a single edge",
            inputs: &[InputField {
                name: "id",
                kind: "string",
                optional: false,
                description: "Edge ID to delete",
            }],
            output: "Confirmation message; error if edge does not exist",
        },
        CommandDescriptor {
            name: "set-property",
            category: "State",
            summary: "Set a property key-value pair on a node or edge",
            inputs: &[
                InputField {
                    name: "target",
                    kind: "string",
                    optional: false,
                    description: "Node or edge ID",
                },
                InputField {
                    name: "key",
                    kind: "string",
                    optional: false,
                    description: "Property key name",
                },
                InputField {
                    name: "value",
                    kind: "string | JSON",
                    optional: false,
                    description: "Property value (auto-parsed as JSON; falls back to string)",
                },
            ],
            output: "Confirmation message; error if target does not exist",
        },

        // ── Query operations ─────────────────────────────────────────────
        CommandDescriptor {
            name: "show-state",
            category: "Query",
            summary: "Display the full current graph state (nodes + edges + properties)",
            inputs: &[],
            output: "List of nodes (with properties) and edges (with type + properties); summary counts",
        },

        // ── Causal / explanation operations ──────────────────────────────
        CommandDescriptor {
            name: "explain",
            category: "Causal",
            summary: "Trace the causal chain behind a node's property value",
            inputs: &[
                InputField {
                    name: "node-id",
                    kind: "string",
                    optional: false,
                    description: "Node ID to explain",
                },
                InputField {
                    name: "property-key",
                    kind: "string",
                    optional: true,
                    description: "Specific property to explain (omit for all)",
                },
            ],
            output: "Current value + ordered list of events (event ID, type, timestamp, reason)",
        },
        CommandDescriptor {
            name: "trace",
            category: "Causal",
            summary: "Trace the causal predecessors of a specific event",
            inputs: &[InputField {
                name: "event-id",
                kind: "string",
                optional: false,
                description: "Event ID to trace",
            }],
            output: "Ordered list of causally linked events leading to the given event",
        },

        // ── Replay / temporal operations ─────────────────────────────────
        CommandDescriptor {
            name: "replay",
            category: "Replay",
            summary: "Replay the full event log from scratch and display resulting state",
            inputs: &[],
            output: "Replayed graph state (nodes + edges) with event count",
        },
        CommandDescriptor {
            name: "save-snapshot",
            category: "Replay",
            summary: "Export a snapshot of current kernel state to disk",
            inputs: &[],
            output: "Confirmation with snapshot byte size",
        },
        CommandDescriptor {
            name: "replay-check",
            category: "Replay",
            summary: "Replay event log and compare result against saved snapshot",
            inputs: &[],
            output: "Replay result + MATCH/MISMATCH against snapshot (or 'none available')",
        },

        // ── System / diagnostic operations ───────────────────────────────
        CommandDescriptor {
            name: "version",
            category: "System",
            summary: "Display kernel version",
            inputs: &[],
            output: "Semantic version string (e.g. 1.0)",
        },
        CommandDescriptor {
            name: "schema-version",
            category: "System",
            summary: "Display event schema version",
            inputs: &[],
            output: "Semantic version string (e.g. 1.0)",
        },
        CommandDescriptor {
            name: "validate-log",
            category: "System",
            summary: "Check event log for timestamp monotonicity and parsing errors",
            inputs: &[],
            output: "Event count + timestamp validation report",
        },
        CommandDescriptor {
            name: "list-commands",
            category: "System",
            summary: "List all available CLI commands with one-line descriptions",
            inputs: &[],
            output: "Grouped list of commands and their summaries",
        },
        CommandDescriptor {
            name: "describe",
            category: "System",
            summary: "Show detailed information about a specific command",
            inputs: &[InputField {
                name: "command",
                kind: "string",
                optional: false,
                description: "Command name to describe (e.g. create-node, explain)",
            }],
            output: "Full description including inputs, types, and output",
        },
        CommandDescriptor {
            name: "workflow-list",
            category: "System",
            summary: "List available built-in verification workflows",
            inputs: &[],
            output: "List of workflow names",
        },
        CommandDescriptor {
            name: "workflow-run",
            category: "System",
            summary: "Run a named verification workflow",
            inputs: &[InputField {
                name: "name",
                kind: "string",
                optional: false,
                description: "Workflow name from workflow-list",
            }],
            output: "PASS or FAIL result",
        },
        CommandDescriptor {
            name: "workflow-validate",
            category: "System",
            summary: "Run all verification workflows and report results",
            inputs: &[],
            output: "Per-workflow PASS/FAIL + summary",
        },
    ];

/// Look up a single command descriptor by name (case-insensitive).
pub fn find_command(name: &str) -> Option<&'static CommandDescriptor> {
    all_commands().iter().find(|c| c.name.eq_ignore_ascii_case(name))
}
