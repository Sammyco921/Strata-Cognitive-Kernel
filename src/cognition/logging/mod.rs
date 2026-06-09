use std::sync::atomic::{AtomicU8, Ordering};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Error = 0,
    Info = 1,
    Debug = 2,
    Trace = 3,
}

impl LogLevel {
    pub fn name(self) -> &'static str {
        match self {
            LogLevel::Error => "ERROR",
            LogLevel::Info => "INFO",
            LogLevel::Debug => "DEBUG",
            LogLevel::Trace => "TRACE",
        }
    }

}

const DEFAULT_LOG_LEVEL: u8 = 1; // Info

static CURRENT_LOG_LEVEL: AtomicU8 = AtomicU8::new(DEFAULT_LOG_LEVEL);

pub fn set_log_level(level: LogLevel) {
    CURRENT_LOG_LEVEL.store(level as u8, Ordering::SeqCst);
}

pub fn reset_to_default() {
    CURRENT_LOG_LEVEL.store(DEFAULT_LOG_LEVEL, Ordering::SeqCst);
}

pub fn get_log_level() -> LogLevel {
    match CURRENT_LOG_LEVEL.load(Ordering::SeqCst) {
        0 => LogLevel::Error,
        1 => LogLevel::Info,
        2 => LogLevel::Debug,
        3 => LogLevel::Trace,
        _ => LogLevel::Info,
    }
}

pub fn should_log(level: LogLevel) -> bool {
    level as u8 <= get_log_level() as u8
}

pub fn is_minimal() -> bool {
    get_log_level() <= LogLevel::Info
}

pub fn is_verbose() -> bool {
    get_log_level() >= LogLevel::Debug
}

// Log suppression policy:
//   ERROR  — always emitted regardless of config
//   INFO   — emitted by default (production mode)
//   DEBUG  — suppressed unless --debug or --trace is passed
//   TRACE  — suppressed by default; requires --trace to enable
// Atomics are used to allow runtime reconfiguration via CLI flags
// without introducing panics, locks, or mutable globals.

#[allow(unused_macros)]
macro_rules! log_error {
    ($($arg:tt)*) => {
        if $crate::cognition::logging::should_log($crate::cognition::logging::LogLevel::Error) {
            eprintln!("[ERROR] {}", format!($($arg)*));
        }
    };
}

#[allow(unused_macros)]
macro_rules! log_info {
    ($($arg:tt)*) => {
        if $crate::cognition::logging::should_log($crate::cognition::logging::LogLevel::Info) {
            eprintln!("[INFO] {}", format!($($arg)*));
        }
    };
}

#[allow(unused_macros)]
macro_rules! log_debug {
    ($($arg:tt)*) => {
        if $crate::cognition::logging::should_log($crate::cognition::logging::LogLevel::Debug) {
            eprintln!("[DEBUG] {}", format!($($arg)*));
        }
    };
}

#[allow(unused_macros)]
macro_rules! log_trace {
    ($($arg:tt)*) => {
        if $crate::cognition::logging::should_log($crate::cognition::logging::LogLevel::Trace) {
            eprintln!("[TRACE] {}", format!($($arg)*));
        }
    };
}

#[allow(unused_imports)]
pub(crate) use log_debug;
#[allow(unused_imports)]
pub(crate) use log_error;
#[allow(unused_imports)]
pub(crate) use log_info;
#[allow(unused_imports)]
pub(crate) use log_trace;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_log_level() {
        reset_to_default();
        assert_eq!(get_log_level(), LogLevel::Info);
    }

    #[test]
    fn test_set_log_level() {
        reset_to_default();
        set_log_level(LogLevel::Debug);
        assert_eq!(get_log_level(), LogLevel::Debug);
        reset_to_default();
    }

    #[test]
    fn test_should_log() {
        reset_to_default();
        assert!(should_log(LogLevel::Error));
        assert!(should_log(LogLevel::Info));
        assert!(!should_log(LogLevel::Debug));
        assert!(!should_log(LogLevel::Trace));
        reset_to_default();
    }

    #[test]
    fn test_trace_suppressed_in_info() {
        reset_to_default();
        assert!(!should_log(LogLevel::Trace));
        set_log_level(LogLevel::Trace);
        let effective = cfg!(debug_assertions);
        if effective {
            assert!(should_log(LogLevel::Trace));
        }
        reset_to_default();
    }

    #[test]
    fn test_debug_suppressed_in_error() {
        reset_to_default();
        set_log_level(LogLevel::Error);
        assert!(!should_log(LogLevel::Debug));
        reset_to_default();
    }

    #[test]
    fn test_is_minimal_default() {
        reset_to_default();
        assert!(is_minimal());
        set_log_level(LogLevel::Debug);
        if cfg!(debug_assertions) {
            assert!(!is_minimal());
        }
        reset_to_default();
    }

    #[test]
    fn test_is_verbose() {
        reset_to_default();
        assert!(!is_verbose());
        set_log_level(LogLevel::Debug);
        if cfg!(debug_assertions) {
            assert!(is_verbose());
        }
        reset_to_default();
    }

    #[test]
    fn test_log_level_name() {
        assert_eq!(LogLevel::Error.name(), "ERROR");
        assert_eq!(LogLevel::Info.name(), "INFO");
        assert_eq!(LogLevel::Debug.name(), "DEBUG");
        assert_eq!(LogLevel::Trace.name(), "TRACE");
    }

    #[test]
    fn test_reset_to_default() {
        set_log_level(LogLevel::Error);
        reset_to_default();
        assert_eq!(get_log_level(), LogLevel::Info);
    }

    #[test]
    fn test_identical_execution_all_log_levels() {
        let input = crate::cognition::system::types::CognitionSystemInput::new(
            "find nodes",
            vec![crate::cognition::policy::types::PolicyRule::new("R001")],
            crate::kernel::GraphState::empty(),
            crate::ontology::OntologyRegistry::empty(),
        );

        let levels = [LogLevel::Error, LogLevel::Info, LogLevel::Debug, LogLevel::Trace];
        let mut outputs = Vec::new();

        for &level in &levels {
            set_log_level(level);
            outputs.push(crate::cognition::system::engine::run_cognition_system(input.clone()));
        }

        reset_to_default();

        for i in 1..outputs.len() {
            assert_eq!(outputs[0], outputs[i],
                "Output differs between log levels {:?} and {:?}",
                levels[0], levels[i]);
        }
    }

    #[test]
    fn test_reset_is_deterministic() {
        let input = crate::cognition::system::types::CognitionSystemInput::new(
            "find nodes",
            vec![crate::cognition::policy::types::PolicyRule::new("R001")],
            crate::kernel::GraphState::empty(),
            crate::ontology::OntologyRegistry::empty(),
        );

        let mut results = Vec::new();
        for level in &[LogLevel::Error, LogLevel::Debug, LogLevel::Trace, LogLevel::Info] {
            set_log_level(*level);
            results.push(crate::cognition::system::engine::run_cognition_system(input.clone()));
            reset_to_default();
        }
        reset_to_default();
        for i in 1..results.len() {
            assert_eq!(results[0], results[i]);
        }
    }

    #[test]
    fn test_logging_does_not_affect_event_log() {
        let input = crate::cognition::system::types::CognitionSystemInput::new(
            "find nodes",
            vec![crate::cognition::policy::types::PolicyRule::new("R001")],
            crate::kernel::GraphState::empty(),
            crate::ontology::OntologyRegistry::empty(),
        );

        set_log_level(LogLevel::Error);
        let out_error = crate::cognition::system::engine::run_cognition_system(input.clone());

        set_log_level(LogLevel::Trace);
        let out_trace = crate::cognition::system::engine::run_cognition_system(input.clone());

        reset_to_default();
        assert_eq!(out_error.event_sequence, out_trace.event_sequence);
        assert_eq!(out_error.trace_record, out_trace.trace_record);
    }

    #[test]
    fn test_100_run_stability_with_logging_resets() {
        let input = crate::cognition::system::types::CognitionSystemInput::new(
            "find nodes",
            vec![crate::cognition::policy::types::PolicyRule::new("R001")],
            crate::kernel::GraphState::empty(),
            crate::ontology::OntologyRegistry::empty(),
        );

        let first = crate::cognition::system::engine::run_cognition_system(input.clone());
        for i in 0..100 {
            let level = match i % 4 {
                0 => LogLevel::Error,
                1 => LogLevel::Info,
                2 => LogLevel::Debug,
                3 => LogLevel::Trace,
                _ => LogLevel::Info,
            };
            set_log_level(level);
            let next = crate::cognition::system::engine::run_cognition_system(input.clone());
            assert_eq!(first, next, "Failed at iteration {} with level {:?}", i, level);
        }
        reset_to_default();
    }
}
