#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliCommand {
    Run(String),
    Replay(String),
    Inspect(String),
    Verify,
    Goals,
    Memory,
}

impl CliCommand {
    pub fn name(&self) -> &'static str {
        match self {
            CliCommand::Run(_) => "run",
            CliCommand::Replay(_) => "replay",
            CliCommand::Inspect(_) => "inspect",
            CliCommand::Verify => "verify",
            CliCommand::Goals => "goals",
            CliCommand::Memory => "memory",
        }
    }
}

pub fn parse_command(args: &[String]) -> Result<CliCommand, String> {
    if args.is_empty() {
        return Err("No command specified. Usage: strata <command> [args]".to_string());
    }
    let command = args[0].as_str();
    match command {
        "run" => {
            if args.len() < 2 {
                return Err("Usage: strata run <input>".to_string());
            }
            Ok(CliCommand::Run(args[1..].join(" ")))
        }
        "replay" => {
            if args.len() < 2 {
                return Err("Usage: strata replay <trace_file>".to_string());
            }
            Ok(CliCommand::Replay(args[1].clone()))
        }
        "inspect" => {
            if args.len() < 2 {
                return Err("Usage: strata inspect <trace_id>".to_string());
            }
            Ok(CliCommand::Inspect(args[1].clone()))
        }
        "verify" => Ok(CliCommand::Verify),
        "goals" => Ok(CliCommand::Goals),
        "memory" => Ok(CliCommand::Memory),
        _ => Err(format!("Unknown command: {}. Available: run, replay, inspect, verify, goals, memory", command)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_run() {
        let cmd = parse_command(&["run".to_string(), "find nodes".to_string()]).unwrap();
        assert_eq!(cmd, CliCommand::Run("find nodes".to_string()));
    }

    #[test]
    fn test_parse_run_multi_word() {
        let cmd = parse_command(&["run".to_string(), "describe".to_string(), "node".to_string(), "42".to_string()]).unwrap();
        assert_eq!(cmd, CliCommand::Run("describe node 42".to_string()));
    }

    #[test]
    fn test_parse_replay() {
        let cmd = parse_command(&["replay".to_string(), "trace.json".to_string()]).unwrap();
        assert_eq!(cmd, CliCommand::Replay("trace.json".to_string()));
    }

    #[test]
    fn test_parse_inspect() {
        let cmd = parse_command(&["inspect".to_string(), "int_abc123".to_string()]).unwrap();
        assert_eq!(cmd, CliCommand::Inspect("int_abc123".to_string()));
    }

    #[test]
    fn test_parse_verify() {
        let cmd = parse_command(&["verify".to_string()]).unwrap();
        assert_eq!(cmd, CliCommand::Verify);
    }

    #[test]
    fn test_parse_goals() {
        let cmd = parse_command(&["goals".to_string()]).unwrap();
        assert_eq!(cmd, CliCommand::Goals);
    }

    #[test]
    fn test_parse_memory() {
        let cmd = parse_command(&["memory".to_string()]).unwrap();
        assert_eq!(cmd, CliCommand::Memory);
    }

    #[test]
    fn test_parse_empty_args() {
        let result = parse_command(&[]);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("No command specified"));
    }

    #[test]
    fn test_parse_unknown_command() {
        let result = parse_command(&["foobar".to_string()]);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown command"));
    }

    #[test]
    fn test_parse_run_missing_input() {
        let result = parse_command(&["run".to_string()]);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Usage: strata run"));
    }

    #[test]
    fn test_parse_replay_missing_file() {
        let result = parse_command(&["replay".to_string()]);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Usage: strata replay"));
    }

    #[test]
    fn test_parse_inspect_missing_id() {
        let result = parse_command(&["inspect".to_string()]);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Usage: strata inspect"));
    }

    #[test]
    fn test_command_name_run() {
        assert_eq!(CliCommand::Run("test".to_string()).name(), "run");
    }

    #[test]
    fn test_command_name_verify() {
        assert_eq!(CliCommand::Verify.name(), "verify");
    }
}
