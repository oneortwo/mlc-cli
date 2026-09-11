mod cli;
mod client;
mod config;
mod error;
mod output;
mod update;

use clap::{CommandFactory, Parser};
use cli::{Auth, Cli, Command, Search, Work};
use error::Result;
use reqwest::Method;
use serde_json::{json, Value};
use std::io::IsTerminal;

fn main() {
    if let Err(error) = run(Cli::parse()) {
        eprintln!("error: {error}");
        std::process::exit(error.code);
    }
}

fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Update => update::run(cli.json),
        Command::Completions { shell } => {
            clap_complete::generate(shell, &mut Cli::command(), "mlc", &mut std::io::stdout());
            Ok(())
        }
        Command::Auth {
            command: Auth::Setup { no_input },
        } => {
            let credentials = match config::from_environment()? {
                Some(credentials) => credentials,
                None if no_input || !std::io::stdin().is_terminal() => {
                    return Err(error::Error::new(
                        2,
                        "Set MLC_USERNAME and MLC_PASSWORD, or run `mlc auth setup` in a terminal",
                    ))
                }
                None => config::prompt()?,
            };
            eprintln!("Verifying credentials...");
            client::Client::login(credentials.clone())?;
            let path = config::save(&credentials)?;
            eprintln!("Credentials saved to {}", path.display());
            output::print(
                &json!({"configured":true,"verified":true,"path":path}),
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
