use privacy_hygiene::{redact, Detector, Span};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{self, BufRead, Read, Write};

const HELP: &str = concat!(
    "privacy-hygiene [--weights PATH] [--jsonl] [--contextual]\n",
    "Read UTF-8 stdin and emit JSON spans and redacted text.\n",
    "Offsets count Unicode code points.\n",
    "Detects email, phone, SSN, card, IBAN, cued name and address.\n",
    "--contextual adds cued Unicode names, addresses, secrets, accounts and birth dates.\n",
    "--jsonl reads one {id, text} record per line and flushes each response."
);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    id: Value,
    text: String,
}

#[derive(Serialize)]
struct Response {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<Value>,
    redacted: String,
    spans: Vec<Span>,
}

fn stream(detect: impl Fn(&str) -> Vec<Span>) -> Result<(), String> {
    let input = io::stdin();
    let output = io::stdout();
    let mut output = output.lock();
    for line in input.lock().lines() {
        let line = line.map_err(|_| "Could not read UTF-8 JSONL")?;
        if line.trim().is_empty() {
            continue;
        }
        let record: Record = serde_json::from_str(&line).map_err(|_| "Invalid JSONL record")?;
        let spans = detect(&record.text);
        let masked = redact(&record.text, &spans)?;
        let response = Response {
            id: Some(record.id),
            redacted: masked,
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

fn run() -> Result<(), String> {
    let mut arguments = std::env::args().skip(1);
    let mut weights = None;
    let mut jsonl = false;
    let mut contextual = false;
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--help" | "-h" => {
                println!("{HELP}");
                return Ok(());
            }
            "--version" | "-V" => {
                println!("privacy-hygiene {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            "--weights" if weights.is_none() => {
                weights = Some(arguments.next().ok_or("--weights requires a path")?);
            }
            "--jsonl" if !jsonl => jsonl = true,
            "--contextual" if !contextual => contextual = true,
            _ => return Err("Unknown or repeated argument; use --help".into()),
        }
    }
    let model = if let Some(path) = weights {
        let bytes = std::fs::read(path).map_err(|_| "Could not read native model")?;
        Detector::from_bytes(&bytes)?
    } else {
        Detector::embedded()?
    };
    let detect = |text: &str| {
        if contextual {
            model.detect_contextual(text)
        } else {
            model.detect(text)
        }
    };
    if jsonl {
        return stream(detect);
    }
    let mut text = String::new();
    io::stdin()
        .read_to_string(&mut text)
        .map_err(|_| "Could not read UTF-8 stdin")?;
    let spans = detect(&text);
    let masked = redact(&text, &spans)?;
    let response = Response {
        id: None,
        redacted: masked,
        spans,
    };
    let response = serde_json::to_string(&response).map_err(|_| "Could not serialize response")?;
    println!("{response}");
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("privacy-hygiene: {error}");
        std::process::exit(1);
    }
}
