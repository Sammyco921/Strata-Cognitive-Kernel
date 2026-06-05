pub mod api;
pub mod bootstrap;
pub mod cli;
pub mod describe;
pub mod kernel;
pub(crate) mod persistence;
pub(crate) mod projection;
pub mod session;
pub mod workflow;

// Re-export key types at crate root for convenience
pub use api::envelope::{TraceId, ENVELOPE_VERSION};
pub use api::dispatcher::ApiDispatcher;
pub use api::result::{CommandResultV1, ResultPayload};
pub use api::StrataEngine;
pub use api::ABI_VERSION;
pub use api::{EdgeView, EventView, ExplanationView, NodeView, SnapshotView};
pub use kernel::engine::Kernel;
pub use kernel::error::KernelError;
pub use kernel::event::{Event, EventType};
pub use kernel::graph::GraphState;
pub use kernel::hash::{log_hash, state_hash, LogHash, StateHash};
pub use kernel::replay::{detect_causal_cycle, replay};
pub use kernel::version;
pub use kernel::version::{KernelVersion, SchemaVersion, CURRENT_KERNEL_VERSION, CURRENT_SCHEMA_VERSION};
pub use projection::causal::{CausalChainLink, CausalGraph, CausalRelation, Explanation, project_default};
