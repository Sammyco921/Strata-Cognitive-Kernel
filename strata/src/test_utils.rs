use crate::api::command::{Command, EnvelopeExecutor};
use crate::api::envelope::CommandEnvelope;
use crate::api::result::CommandResultV1;
use crate::api::{Engine, StrataEngine};
use crate::bootstrap::Bootstrap;
use crate::cli::CliCommand;
use crate::kernel::engine::{Kernel, TestConfig};
use crate::kernel::event::Event;
use crate::kernel::NullPersister;
use crate::observability::AuditLog;

/// A test-friendly wrapper around `Bootstrap<NullPersister>`.
///
/// Provides the same API as `Bootstrap` but with a concrete non-generic type
/// that hides the internal `NullPersister` type parameter from public API
/// surfaces.
pub struct TestBootstrap {
    inner: Bootstrap<NullPersister>,
}

/// Create a test bootstrap from an explicit event list.
///
/// The returned bootstrap uses a null persistence backend (no I/O).
pub fn test_bootstrap(events: Vec<Event>) -> TestBootstrap {
    let kernel = Kernel::test(TestConfig { seed_events: events });
    let engine = StrataEngine::from_kernel(kernel);
    TestBootstrap {
        inner: Bootstrap::from_parts(engine, 0, false, AuditLog::new()),
    }
}

impl TestBootstrap {
    /// Execute a CLI command through the canonical pipeline.
    pub fn run(&mut self, cli_cmd: CliCommand) -> CommandResultV1 {
        self.inner.run(cli_cmd)
    }

    /// Convert a CLI command into a domain Command.
    pub fn convert(&mut self, cli: CliCommand) -> Command {
        self.inner.convert(cli)
    }

    /// Return a reference to the underlying engine (for testing only).
    #[doc(hidden)]
    pub fn engine(&self) -> &impl Engine {
        self.inner.engine()
    }

    /// Return a reference to the audit log.
    pub fn audit_log(&self) -> &AuditLog {
        &self.inner.audit_log
    }

    /// Enable or disable trace logging.
    pub fn set_trace(&mut self, enabled: bool) {
        self.inner.set_trace(enabled);
    }

    /// Execute a command envelope through the canonical pipeline.
    pub fn execute(&mut self, envelope: CommandEnvelope) -> CommandResultV1 {
        self.inner.execute(envelope)
    }
}

impl EnvelopeExecutor for TestBootstrap {
    fn execute(&mut self, envelope: CommandEnvelope) -> CommandResultV1 {
        self.inner.execute(envelope)
    }
}

/// Create a test engine from an explicit event list.
///
/// The returned engine uses a null persistence backend (no I/O).  This is the
/// recommended way to construct an engine for testing or transient use.
pub fn test_engine(events: Vec<Event>) -> impl Engine {
    let kernel = Kernel::test(TestConfig { seed_events: events });
    StrataEngine::from_kernel(kernel)
}
