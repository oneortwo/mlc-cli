use clap::{Args, Parser, Subcommand};
use serde_json::{json, Value};

#[derive(Parser)]
#[command(version, about = "Unofficial CLI for The MLC Public Search API")]
pub struct Cli {
    #[arg(
        long,
        global = true,
        help = "Always output JSON (also the default when piped)"
    )]
    pub json: bool,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Search musical works or sound recordings
    Search {
        #[command(subcommand)]
        command: Search,
    },
    /// Retrieve musical works and their writers/publishers
    Work {
        #[command(subcommand)]
        command: Work,
    },
    /// Configure credential references or check authentication
    Auth {
        #[command(subcommand)]
        command: Auth,
    },
    /// Verify login and data access with a small read-only work search
    Doctor,
    /// Generate shell completions
    Completions { shell: clap_complete::Shell },
}

#[derive(Subcommand)]
pub enum Search {
    /// Search by title and writer (both required by the live API)
    Works(WorkSearch),
    /// Search by ISRC, recording title and/or artist
    Recordings(RecordingSearch),
}

#[derive(Args)]
#[command(group(clap::ArgGroup::new("writer").required(true).multiple(true)
    .args(["writer_first_name", "writer_last_name", "writer_ipi"])))]
pub struct WorkSearch {
    pub title: String,
    #[arg(long)]
    pub writer_first_name: Option<String>,
    #[arg(long)]
    pub writer_last_name: Option<String>,
    #[arg(long)]
    pub writer_ipi: Option<String>,
}

#[derive(Args)]
#[command(group(clap::ArgGroup::new("criteria").required(true).multiple(true)
    .args(["title", "artist", "isrc"])))]
pub struct RecordingSearch {
    #[arg(long)]
    pub title: Option<String>,
    #[arg(long)]
    pub artist: Option<String>,
    #[arg(long)]
    pub isrc: Option<String>,
}

#[derive(Subcommand)]
pub enum Work {
    /// Get one work by MLC song code
    Get { id: String },
    /// Get multiple works by MLC song code in a single request
    Batch {
        #[arg(required = true, num_args = 1..)]
        ids: Vec<String>,
    },
}

#[derive(Subcommand)]
pub enum Auth {
    /// Save only 1Password references, never credential values
    Setup {
        #[arg(long)]
        username_ref: String,
        #[arg(long)]
        password_ref: String,
    },
    /// Show credential source without resolving or displaying secrets
    Status,
}

impl WorkSearch {
    pub fn body(&self) -> Value {
        let mut body = json!({"title":self.title});
        let mut writer = json!({});
        add(&mut writer, "writerFirstName", &self.writer_first_name);
        add(&mut writer, "writerLastName", &self.writer_last_name);
        add(&mut writer, "writerIPI", &self.writer_ipi);
        if writer != json!({}) {
            body["writers"] = json!([writer]);
        }
        body
    }
}

impl RecordingSearch {
    pub fn body(&self) -> Value {
        let mut body = json!({});
        add(&mut body, "title", &self.title);
        add(&mut body, "artist", &self.artist);
        add(&mut body, "isrc", &self.isrc);
        body
    }
}

fn add(body: &mut Value, key: &str, value: &Option<String>) {
    if let Some(value) = value {
        body[key] = json!(value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn work_search_combines_title_and_writer_with_api_field_names() {
        let cli = Cli::try_parse_from([
            "mlc",
            "search",
            "works",
            "Example",
            "--writer-first-name",
            "Jane",
            "--writer-last-name",
            "Doe",
            "--writer-ipi",
            "00001234567",
        ])
        .unwrap();
        let Command::Search {
            command: Search::Works(args),
        } = cli.command
        else {
            panic!()
        };
        assert_eq!(
            args.body(),
            json!({"title":"Example", "writers":[{"writerFirstName":"Jane", "writerLastName":"Doe", "writerIPI":"00001234567"}]})
        );
    }

    #[test]
    fn work_search_requires_title_and_writer_before_authentication() {
        assert!(Cli::try_parse_from(["mlc", "search", "works", "Example"]).is_err());
        assert!(
            Cli::try_parse_from(["mlc", "search", "works", "--writer-last-name", "Doe"]).is_err()
        );
    }

    #[test]
    fn recording_filters_match_api_schema() {
        let cli = Cli::try_parse_from([
            "mlc",
            "search",
            "recordings",
            "--isrc",
            "AAABC1200001",
            "--artist",
            "Example Artist",
            "--title",
            "Example",
        ])
        .unwrap();
        let Command::Search {
            command: Search::Recordings(args),
        } = cli.command
        else {
            panic!()
        };
        assert_eq!(
            args.body(),
            json!({"isrc":"AAABC1200001", "artist":"Example Artist", "title":"Example"})
        );
    }
}
