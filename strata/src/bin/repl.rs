use std::io::{self, BufRead};

use clap::Parser;

use strata::api::envelope::{CommandEnvelope, TraceId};
use strata::bootstrap::Bootstrap;
use strata::cli::Cli;
use strata::session::{handle_repl_command, ReplAction, SessionManager};

fn main() {
    let mut bootstrap = Bootstrap::new();
    let mut session = SessionManager::new();

    let stdin = io::stdin();
    println!("Strata REPL — type :help for commands, :exit to quit");

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                eprintln!("error reading input: {}", e);
                continue;
            }
        };

        let trimmed = line.trim().to_string();
        if trimmed.is_empty() {
            continue;
        }

        // ── REPL-local commands (not forwarded to kernel) ─────────────
        if trimmed.starts_with(':') {
            match handle_repl_command(&trimmed, &session) {
                Ok((ReplAction::Exit, _)) => break,
                Ok((_, Some(output))) => println!("{}", output),
                Ok((_, None)) => {}
                Err(msg) => eprintln!("{}", msg),
            }
            continue;
        }

        // ── Standard CLI command ──────────────────────────────────────
        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        let mut args = vec!["strata"];
        args.extend(parts);

        match Cli::try_parse_from(&args) {
            Ok(cli) => {
                let command = bootstrap.convert(cli.command);
                let trace_id = TraceId::generate();
                let envelope = CommandEnvelope {
                    version: strata::api::envelope::ENVELOPE_VERSION,
                    trace_id,
                    command,
                    session_id: Some(session.session_id()),
                };
                let result = bootstrap.execute(envelope);
                session.record_trace(trace_id);
                println!("{}", serde_json::to_string_pretty(&result).unwrap());
            }
            Err(e) => {
                eprintln!("parse error: {}", e);
            }
        }
    }
}
