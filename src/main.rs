mod cli;
mod client;
mod config;
mod error;
mod output;

use clap::{CommandFactory, Parser};
use cli::{Auth, Cli, Command, Search, Work};
use error::Result;
use reqwest::Method;
use serde_json::{json, Value};

fn main() {
    if let Err(error) = run(Cli::parse()) {
        eprintln!("error: {error}");
        std::process::exit(error.code);
    }
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Completions { shell } => {
            clap_complete::generate(shell, &mut Cli::command(), "mlc", &mut std::io::stdout());
            Ok(())
        }
        Command::Auth {
            command:
                Auth::Setup {
                    username_ref,
                    password_ref,
                },
        } => {
            config::setup(username_ref, password_ref)?;
            output::print(
                &json!({"configured":true,"source":"config file","secrets_saved":true}),
                cli.json,
            )
        }
        Command::Auth {
            command: Auth::Status,
        } => output::print(
            &json!({"source":config::source()?,"verified":false}),
            cli.json,
        ),
        Command::Doctor => {
            let source = config::source()?;
            let client = client::Client::login(config::credentials()?)?;
            client.request(
                Method::POST,
                "search/songcode",
                Some(json!({"title":"Yesterday", "writers":[{"writerLastName":"McCartney"}]})),
            )?;
            output::print(
                &json!({"authenticated":true,"data_access":true,"source":source,"version":env!("CARGO_PKG_VERSION")}),
                cli.json,
            )
        }
        command => {
            let client = client::Client::login(config::credentials()?)?;
            output::print(&query(&client, command)?, cli.json)
        }
    }
}

fn query(client: &client::Client, command: Command) -> Result<Value> {
    match command {
        Command::Search {
            command: Search::Works(args),
        } => client.request(Method::POST, "search/songcode", Some(args.body())),
        Command::Search {
            command: Search::Recordings(args),
        } => client.request(Method::POST, "search/recordings", Some(args.body())),
        Command::Work {
            command: Work::Get { id },
        } => client.work(&id),
        Command::Work {
            command: Work::Batch { ids },
        } => {
            let body = ids
                .iter()
                .map(|id| json!({"mlcsongCode":id}))
                .collect::<Vec<_>>();
            client.request(Method::POST, "works", Some(json!(body)))
        }
        _ => unreachable!(),
    }
}
