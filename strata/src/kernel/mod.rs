pub mod engine;
pub use engine::{KernelConfig, TestConfig};
pub(crate) mod error;
pub(crate) mod event;
pub(crate) mod compatibility;
pub(crate) mod graph;
pub mod hash;
pub(crate) mod replay;
pub mod version;

/// Abstraction for persisting and loading events and snapshots.
///
/// The kernel depends ONLY on this trait — never on a concrete persistence
/// implementation.  This ensures the kernel module has no dependency on
/// any crate::persistence or external I/O.
pub trait EventPersister {
    fn append_event(&mut self, event: &Event) -> Result<(), KernelError>;
    fn load_events(&self) -> Vec<Event>;
    fn load_snapshot(&self) -> Option<(GraphState, u64)>;
    fn save_snapshot(&self, state: &GraphState, clock: u64) -> Result<(), KernelError>;
}

/// A persister that does nothing.  Used when Kernel is created from an
/// explicit event list (tests, from_events), where no I/O is needed.
pub(crate) struct NullPersister;

impl EventPersister for NullPersister {
    fn append_event(&mut self, _event: &Event) -> Result<(), KernelError> {
        Ok(())
    }
    fn load_events(&self) -> Vec<Event> {
        Vec::new()
    }
    fn load_snapshot(&self) -> Option<(GraphState, u64)> {
        None
    }
    fn save_snapshot(&self, _state: &GraphState, _clock: u64) -> Result<(), KernelError> {
        Ok(())
    }
}

use crate::kernel::error::KernelError;
use crate::kernel::event::Event;
use crate::kernel::graph::GraphState;
