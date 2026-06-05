use strata::api::dispatcher::ApiDispatcher;
use strata::api::envelope::CommandEnvelope;
use strata::bootstrap::Bootstrap;
use strata::cli::{Cli, CliCommand};

fn main() {
    let cli: Cli = clap::Parser::parse();

    match cli.command {
        CliCommand::Api { input } => {
            let mut dispatcher = ApiDispatcher::new();
            match dispatcher.dispatch(&input) {
                Ok(result) => {
                    println!("{}", serde_json::to_string_pretty(&result).unwrap());
                }
                Err(err) => {
                    println!("{}", serde_json::to_string_pretty(&err).unwrap());
                }
            }
        }
        other => {
            let mut bootstrap = Bootstrap::new();
            let command = bootstrap.convert(other);
            let envelope = CommandEnvelope::new(command);
            let result = bootstrap.execute(envelope);

            println!("{}", serde_json::to_string_pretty(&result).unwrap());
        }
    }
}
