use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct KernelCommand {
    pub id: String,
    pub command_type: String,
    pub payload: BTreeMap<String, String>,
    pub trace_id: String,
    pub event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ExecutionPlan {
    pub sequence_id: String,
    pub commands: Vec<KernelCommand>,
    pub explanation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub command_id: String,
    pub success: bool,
    pub kernel_result: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ExecutionPlanResult {
    pub plan: ExecutionPlan,
    pub results: Vec<ExecutionResult>,
    pub explanation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CommandEnvelope {
    pub command: KernelCommand,
    pub abi_version: String,
    pub schema: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CommandResultV1 {
    pub command_id: String,
    pub success: bool,
    pub result: String,
    pub error: Option<String>,
}

impl KernelCommand {
    pub fn new(
        id: &str,
        command_type: &str,
        payload: BTreeMap<String, String>,
        trace_id: &str,
        event_id: &str,
    ) -> Self {
        KernelCommand {
            id: id.to_string(),
            command_type: command_type.to_string(),
            payload,
            trace_id: trace_id.to_string(),
            event_id: event_id.to_string(),
        }
    }
}

impl ExecutionPlan {
    pub fn new(sequence_id: &str, commands: Vec<KernelCommand>) -> Self {
        let explanation = format!("commands={}", commands.len());
        ExecutionPlan {
            sequence_id: sequence_id.to_string(),
            commands,
            explanation,
        }
    }
}

impl ExecutionResult {
    pub fn new(command_id: &str, success: bool, kernel_result: &str, error: Option<String>) -> Self {
        ExecutionResult {
            command_id: command_id.to_string(),
            success,
            kernel_result: kernel_result.to_string(),
            error,
        }
    }
}

impl ExecutionPlanResult {
    pub fn new(plan: ExecutionPlan, results: Vec<ExecutionResult>) -> Self {
        let executed = results.iter().filter(|r| r.success).count();
        let failed = results.iter().filter(|r| !r.success).count();
        let explanation = format!(
            "commands={}; executed={}; failed={}",
            results.len(),
            executed,
            failed,
        );
        ExecutionPlanResult {
            plan,
            results,
            explanation,
        }
    }
}

impl CommandEnvelope {
    pub fn new(command: KernelCommand) -> Self {
        CommandEnvelope {
            command,
            abi_version: "1.0.0".to_string(),
            schema: "KernelCommand".to_string(),
        }
    }
}

impl CommandResultV1 {
    pub fn success(command_id: &str, result: &str) -> Self {
        CommandResultV1 {
            command_id: command_id.to_string(),
            success: true,
            result: result.to_string(),
            error: None,
        }
    }

    pub fn failure(command_id: &str, error: &str) -> Self {
        CommandResultV1 {
            command_id: command_id.to_string(),
            success: false,
            result: String::new(),
            error: Some(error.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_command() -> KernelCommand {
        let mut payload = BTreeMap::new();
        payload.insert("node_id".to_string(), "42".to_string());
        KernelCommand::new("cmd:evt_0", "QueryGraph", payload, "int_abc", "evt_0")
    }

    #[test]
    fn test_kernel_command_creation() {
        let cmd = sample_command();
        assert!(cmd.id.starts_with("cmd:"));
        assert_eq!(cmd.command_type, "QueryGraph");
        assert_eq!(cmd.payload.get("node_id").unwrap(), "42");
        assert_eq!(cmd.trace_id, "int_abc");
        assert_eq!(cmd.event_id, "evt_0");
    }

    #[test]
    fn test_kernel_command_id_format() {
        let cmd = sample_command();
        assert!(cmd.id == "cmd:evt_0");
    }

    #[test]
    fn test_execution_plan_creation() {
        let cmds = vec![sample_command()];
        let plan = ExecutionPlan::new("int_abc", cmds);
        assert_eq!(plan.sequence_id, "int_abc");
        assert_eq!(plan.commands.len(), 1);
        assert!(plan.explanation.contains("commands=1"));
    }

    #[test]
    fn test_execution_plan_empty() {
        let plan = ExecutionPlan::new("int_empty", vec![]);
        assert_eq!(plan.commands.len(), 0);
        assert!(plan.explanation.contains("commands=0"));
    }

    #[test]
    fn test_execution_result_creation() {
        let r = ExecutionResult::new("cmd:evt_0", true, "ok", None);
        assert_eq!(r.command_id, "cmd:evt_0");
        assert!(r.success);
        assert_eq!(r.kernel_result, "ok");
        assert!(r.error.is_none());
    }

    #[test]
    fn test_execution_result_with_error() {
        let r = ExecutionResult::new("cmd:evt_0", false, "", Some("fail".to_string()));
        assert!(!r.success);
        assert_eq!(r.error.unwrap(), "fail");
    }

    #[test]
    fn test_execution_plan_result_creation() {
        let plan = ExecutionPlan::new("int_abc", vec![sample_command()]);
        let results = vec![ExecutionResult::new("cmd:evt_0", true, "ok", None)];
        let r = ExecutionPlanResult::new(plan.clone(), results);
        assert_eq!(r.plan.sequence_id, plan.sequence_id);
        assert_eq!(r.results.len(), 1);
        assert!(r.explanation.contains("commands=1"));
        assert!(r.explanation.contains("executed=1"));
        assert!(r.explanation.contains("failed=0"));
    }

    #[test]
    fn test_command_envelope_creation() {
        let cmd = sample_command();
        let envelope = CommandEnvelope::new(cmd.clone());
        assert_eq!(envelope.command.id, cmd.id);
        assert_eq!(envelope.abi_version, "1.0.0");
        assert_eq!(envelope.schema, "KernelCommand");
    }

    #[test]
    fn test_command_result_v1_success() {
        let r = CommandResultV1::success("cmd:evt_0", "completed");
        assert!(r.success);
        assert_eq!(r.result, "completed");
        assert!(r.error.is_none());
    }

    #[test]
    fn test_command_result_v1_failure() {
        let r = CommandResultV1::failure("cmd:evt_0", "error msg");
        assert!(!r.success);
        assert!(r.error.is_some());
        assert_eq!(r.error.unwrap(), "error msg");
    }

    #[test]
    fn test_kernel_command_eq() {
        let mut p1 = BTreeMap::new();
        p1.insert("k".to_string(), "v".to_string());
        let mut p2 = BTreeMap::new();
        p2.insert("k".to_string(), "v".to_string());
        let a = KernelCommand::new("cmd:0", "QueryGraph", p1, "t1", "evt_0");
        let b = KernelCommand::new("cmd:0", "QueryGraph", p2, "t1", "evt_0");
        assert_eq!(a, b);
    }

    #[test]
    fn test_kernel_command_ordering_by_id() {
        let a = KernelCommand::new("cmd:a", "A", BTreeMap::new(), "t", "e1");
        let b = KernelCommand::new("cmd:b", "B", BTreeMap::new(), "t", "e2");
        assert!(a < b);
    }

    #[test]
    fn test_execution_plan_result_explanation_with_failures() {
        let plan = ExecutionPlan::new("int", vec![sample_command(), sample_command()]);
        let results = vec![
            ExecutionResult::new("cmd:0", true, "ok", None),
            ExecutionResult::new("cmd:1", false, "", Some("err".to_string())),
        ];
        let r = ExecutionPlanResult::new(plan, results);
        assert!(r.explanation.contains("commands=2"));
        assert!(r.explanation.contains("executed=1"));
        assert!(r.explanation.contains("failed=1"));
    }

    #[test]
    fn test_roundtrip_serialization_kernel_command() {
        let cmd = sample_command();
        let json = serde_json::to_string(&cmd).unwrap();
        let parsed: KernelCommand = serde_json::from_str(&json).unwrap();
        assert_eq!(cmd, parsed);
    }

    #[test]
    fn test_roundtrip_serialization_execution_plan() {
        let plan = ExecutionPlan::new("int", vec![sample_command()]);
        let json = serde_json::to_string(&plan).unwrap();
        let parsed: ExecutionPlan = serde_json::from_str(&json).unwrap();
        assert_eq!(plan, parsed);
    }

    #[test]
    fn test_roundtrip_serialization_command_envelope() {
        let envelope = CommandEnvelope::new(sample_command());
        let json = serde_json::to_string(&envelope).unwrap();
        let parsed: CommandEnvelope = serde_json::from_str(&json).unwrap();
        assert_eq!(envelope, parsed);
    }

    #[test]
    fn test_roundtrip_serialization_command_result_v1() {
        let r = CommandResultV1::success("cmd:0", "done");
        let json = serde_json::to_string(&r).unwrap();
        let parsed: CommandResultV1 = serde_json::from_str(&json).unwrap();
        assert_eq!(r, parsed);
    }
}
