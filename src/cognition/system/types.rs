use serde::{Deserialize, Serialize};

use crate::cognition::semantic_interpreter::types::SemanticResponse;
use crate::cognition::event_translator::types::ProposedEventSequence;
use crate::cognition::policy::types::{PolicyEvaluationResult, PolicyRule};
use crate::cognition::execution_adapter::types::{ExecutionPlan, ExecutionPlanResult};
use crate::cognition::trace::types::TraceRecord;
use crate::cognition::coherence::types::CoherenceReport;
use crate::cognition::system::policy::types::PolicyDecision;
use crate::cognition::executive::types::ExecutiveDecision;
use crate::cognition::goals::types::{GoalEvaluation, GoalState};
use crate::cognition::memory::types::{CognitiveMemoryState, MemorySnapshot};
use crate::kernel::GraphState;
use crate::ontology::OntologyRegistry;

#[derive(Debug, Clone)]
pub struct CognitionSystemInput {
    pub raw_input: String,
    pub policy_rules: Vec<PolicyRule>,
    pub kernel_state: GraphState,
    pub ontology_registry: OntologyRegistry,
    pub historical_traces: Vec<TraceRecord>,
    pub previous_memory: CognitiveMemoryState,
    pub goal_state: GoalState,
}

impl CognitionSystemInput {
    pub fn new(
        raw_input: &str,
        policy_rules: Vec<PolicyRule>,
        kernel_state: GraphState,
        ontology_registry: OntologyRegistry,
    ) -> Self {
        CognitionSystemInput {
            raw_input: raw_input.to_string(),
            policy_rules,
            kernel_state,
            ontology_registry,
            historical_traces: vec![],
            previous_memory: CognitiveMemoryState::empty(),
            goal_state: GoalState { goals: std::collections::BTreeMap::new() },
        }
    }

    pub fn with_traces(
        raw_input: &str,
        policy_rules: Vec<PolicyRule>,
        kernel_state: GraphState,
        ontology_registry: OntologyRegistry,
        historical_traces: Vec<TraceRecord>,
    ) -> Self {
        CognitionSystemInput {
            raw_input: raw_input.to_string(),
            policy_rules,
            kernel_state,
            ontology_registry,
            historical_traces,
            previous_memory: CognitiveMemoryState::empty(),
            goal_state: GoalState { goals: std::collections::BTreeMap::new() },
        }
    }

    pub fn with_memory(
        raw_input: &str,
        policy_rules: Vec<PolicyRule>,
        kernel_state: GraphState,
        ontology_registry: OntologyRegistry,
        previous_memory: CognitiveMemoryState,
    ) -> Self {
        CognitionSystemInput {
            raw_input: raw_input.to_string(),
            policy_rules,
            kernel_state,
            ontology_registry,
            historical_traces: vec![],
            previous_memory,
            goal_state: GoalState { goals: std::collections::BTreeMap::new() },
        }
    }

    pub fn with_goals(
        raw_input: &str,
        policy_rules: Vec<PolicyRule>,
        kernel_state: GraphState,
        ontology_registry: OntologyRegistry,
        goal_state: GoalState,
    ) -> Self {
        CognitionSystemInput {
            raw_input: raw_input.to_string(),
            policy_rules,
            kernel_state,
            ontology_registry,
            historical_traces: vec![],
            previous_memory: CognitiveMemoryState::empty(),
            goal_state,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CognitionSystemOutput {
    pub semantic_response: SemanticResponse,
    pub event_sequence: ProposedEventSequence,
    pub policy_decision: PolicyDecision,
    pub policy_result: PolicyEvaluationResult,
    pub execution_plan: ExecutionPlan,
    pub execution_result: ExecutionPlanResult,
    pub trace_record: TraceRecord,
    pub coherence_report: CoherenceReport,
    pub memory_snapshot: MemorySnapshot,
    pub goal_evaluations: Vec<GoalEvaluation>,
    pub updated_goal_state: GoalState,
    pub executive_decision: ExecutiveDecision,
}

impl CognitionSystemOutput {
    pub fn stage_count(&self) -> usize {
        12
    }
}
