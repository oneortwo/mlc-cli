use crate::error::{Error, Result};
use comfy_table::Table;
use serde_json::Value;
use std::io::{self, IsTerminal, Write};

pub fn print(value: &Value, json: bool) -> Result<()> {
    let mut output = io::stdout().lock();
    let result = if !json && io::stdout().is_terminal() && value.is_array() {
        let mut table = Table::new();
        table.set_header(["MLC code", "Title", "ISWC / ISRC", "Artist / Writers"]);
        for item in value.as_array().unwrap() {
            table.add_row([
                field(item, &["mlcSongCode", "mlcsongCode", "id"]),
                field(item, &["workTitle", "primaryTitle", "title"]),
                field(item, &["iswc", "isrc"]),
                people(item),
            ]);
        }
        writeln!(
            output,
            "{table}\n{} result(s)",
            value.as_array().unwrap().len()
        )
    } else {
        writeln!(output, "{}", serde_json::to_string_pretty(value).unwrap())
    };
    match result {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
        Err(_) => Err(Error::new(1, "Cannot write output")),
    }
}

fn field(value: &Value, keys: &[&str]) -> String {
    keys.iter()
        .find_map(|key| value.get(key).and_then(Value::as_str))
        .unwrap_or("")
        .chars()
        .filter(|c| !c.is_control())
        .collect()
}

fn people(value: &Value) -> String {
    if let Some(writers) = value.get("writers").and_then(Value::as_array) {
        return writers
            .iter()
            .map(|writer| {
                format!(
                    "{} {}",
                    field(writer, &["writerFirstName"]),
                    field(writer, &["writerLastName"])
                )
                .trim()
                .to_owned()
            })
            .collect::<Vec<_>>()
            .join(", ");
    }
    field(value, &["artist", "artists"])
}
