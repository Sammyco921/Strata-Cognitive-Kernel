use serde::Serialize;
use uuid::Uuid;

use crate::api::command::{Command, CommandResult};

/// Current envelope schema version.
pub const ENVELOPE_VERSION: u32 = 1;

/// A versioned command envelope for CLI → kernel communication.
///
/// Every request carries a schema `version`, a unique `request_id`, and a
/// `command` payload.  No command may be sent to the kernel without an
/// envelope.
#[derive(Debug, Clone, Serialize)]
pub struct CommandEnvelope {
    pub version: u32,
    pub request_id: Uuid,
    pub command: Command,
}

impl CommandEnvelope {
    pub fn new(request_id: Uuid, command: Command) -> Self {
        CommandEnvelope {
            version: ENVELOPE_VERSION,
            request_id,
            command,
        }
    }
}

/// A versioned result envelope returned by the kernel.
///
/// Mirrors `CommandEnvelope` with the same `version` and paired
/// `request_id` so every response can be matched to its request.
#[derive(Debug, Clone, Serialize)]
pub struct ResultEnvelope {
    pub version: u32,
    pub request_id: Uuid,
    pub result: CommandResult,
}

impl ResultEnvelope {
    pub fn new(request_id: Uuid, result: CommandResult) -> Self {
        ResultEnvelope {
            version: ENVELOPE_VERSION,
            request_id,
            result,
        }
    }
}
