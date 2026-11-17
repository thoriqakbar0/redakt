use privacy_hygiene::{redact, Detector};
use serde::Deserialize;
use serde_json::{json, Value};
use std::io::{self, BufRead, Read, Write};

const HELP: &str = concat!(
    "privacy-hygiene [--weights PATH] [--jsonl]\n",
    "Read UTF-8 stdin and emit JSON spans and redacted text.\n",
    "Offsets count Unicode code points.\n",
    "Detects email, phone, SSN, card, IBAN, cued name and address.\n",
    "--jsonl reads one {id, text} record per line and flushes each response."
);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    id: Value,
    text: String,
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
        let spans = model.detect(&record.text);
        let masked = redact(&record.text, &spans)?;
        let response = json!({"id": record.id, "spans": spans, "redacted": masked});
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
    let spans = model.detect(&text);
    let masked = redact(&text, &spans)?;
    println!("{}", json!({"spans": spans, "redacted": masked}));
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("privacy-hygiene: {error}");
        std::process::exit(1);
    }
}
