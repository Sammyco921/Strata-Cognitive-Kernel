use uuid::Uuid;

use strata::api::envelope::CommandEnvelope;
use strata::bootstrap::Bootstrap;
use strata::cli::Cli;

fn main() {
    let cli: Cli = clap::Parser::parse();
    let mut bootstrap = Bootstrap::new();
    let request_id = Uuid::new_v4();

    let command = bootstrap.convert(cli.command);
    let envelope = CommandEnvelope::new(request_id, command);
    let result = bootstrap.execute(envelope);

    println!("{}", serde_json::to_string_pretty(&result).unwrap());
}
