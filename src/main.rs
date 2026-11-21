use redakt::{redact, Detector, Span};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{self, BufRead, Read, Write};

const HELP: &str = concat!(
    "redakt [--weights PATH] [--jsonl]\n",
    "Read UTF-8 stdin and write plain redacted text to stdout.\n",
    "Replaces private values with numbered labels such as [EMAIL_1].\n",
    "Detects email, phone, SSN, card, IBAN, and cued private values.\n",
    "Cued values include names, addresses, secrets, accounts and birth dates.\n",
    "--jsonl reads one {id, text} record per line and flushes each JSON response.\n",
    "JSONL span offsets count Unicode code points."
);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    id: Value,
    text: String,
}

#[derive(Serialize)]
struct Response {
    id: Value,
    redacted: String,
    spans: Vec<Span>,
}

fn stream(model: &Detector) -> Result<(), String> {
    let input = io::stdin();
    let output = io::stdout();
    let mut output = output.lock();
    for line in input.lock().lines() {
        let line = line.map_err(|_| "Could not read UTF-8 JSONL")?;
        if line.trim().is_empty() {
            continue;
        }
        let record: Record = serde_json::from_str(&line).map_err(|_| "Invalid JSONL record")?;
        let spans = model.detect_contextual(&record.text);
        let redacted = redact(&record.text, &spans)?;
        let response = Response {
            id: record.id,
            redacted,
            spans,
        };
        let response =
            serde_json::to_string(&response).map_err(|_| "Could not serialize JSONL response")?;
        writeln!(output, "{response}").map_err(|_| "Could not write JSONL response")?;
        output
            .flush()
            .map_err(|_| "Could not flush JSONL response")?;
    }
    Ok(())
}

fn write_info(message: &str) -> Result<(), String> {
    let mut output = io::stdout().lock();
    writeln!(output, "{message}")
        .and_then(|()| output.flush())
        .map_err(|_| "Could not write command information".into())
}

fn run() -> Result<(), String> {
    let mut arguments = std::env::args_os().skip(1);
    let mut weights = None;
    let mut jsonl = false;
    while let Some(argument) = arguments.next() {
        match argument.to_str() {
            Some("--help" | "-h") => return write_info(HELP),
            Some("--version" | "-V") => {
                return write_info(concat!("redakt ", env!("CARGO_PKG_VERSION")));
            }
            Some("--weights") if weights.is_none() => {
                weights = Some(arguments.next().ok_or("--weights requires a path")?);
            }
            Some("--jsonl") if !jsonl => jsonl = true,
            _ => return Err("Unknown or repeated argument; use --help".into()),
        }
    }
    let model = if let Some(path) = weights {
        let bytes = std::fs::read(path).map_err(|_| "Could not read native model")?;
        Detector::from_bytes(&bytes)?
    } else {
        Detector::embedded()?
    };
    if jsonl {
        return stream(&model);
    }
    let mut text = String::new();
    io::stdin()
        .read_to_string(&mut text)
        .map_err(|_| "Could not read UTF-8 stdin")?;
    let spans = model.detect_contextual(&text);
    let redacted = redact(&text, &spans)?;
    let mut output = io::stdout().lock();
    output
        .write_all(redacted.as_bytes())
        .map_err(|_| "Could not write redacted text")?;
    output
        .flush()
        .map_err(|_| "Could not flush redacted text")?;
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("redakt: {error}");
        std::process::exit(1);
    }
}
