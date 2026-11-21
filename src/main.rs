use recovery::Recovery;
use redakt::{redact, Detector, PiiTagger, Span};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{self, BufRead, Read, Write};
use std::path::PathBuf;

mod recovery;
mod update;

const HELP: &str = concat!(
    "redakt [--weights PATH] [--jsonl]\n",
    "Read UTF-8 stdin and write plain redacted text to stdout.\n",
    "Saves encrypted recovery data and a separate code file by default.\n",
    "redakt --reverse [--recovery PATH] [--code-file PATH]\n",
    "--recovery PATH selects or creates a mapping for one conversation.\n",
    "--code-file PATH selects the recovery code file.\n",
    "--irreversible uses numbered labels and writes no recovery files.\n",
    "Detects names, contact details, identifiers and other PII using trained and rule-based detectors.\n",
    "--weights runs the supplied native rules model with cue-based detection.\n",
    "--jsonl reads one {id, text} record per line and flushes each JSON response.\n",
    "JSONL span offsets count Unicode code points.\n\n",
    "redakt update [--check]\n",
    "Check GitHub Releases or update a standalone Linux or macOS binary.\n",
    "Set REDAKT_UPDATE_NOTIFY=0 to disable daily update notices."
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

fn detect(model: &Detector, tagger: Option<&PiiTagger>, text: &str) -> Vec<Span> {
    match tagger {
        Some(tagger) => tagger.detect_with_rules(model, text),
        None => model.detect_contextual(text),
    }
}

fn stream(model: &Detector, tagger: Option<&PiiTagger>) -> Result<(), String> {
    let input = io::stdin();
    let output = io::stdout();
    let mut output = output.lock();
    for line in input.lock().lines() {
        let line = line.map_err(|_| "Could not read UTF-8 JSONL")?;
        if line.trim().is_empty() {
            continue;
        }
        let record: Record = serde_json::from_str(&line).map_err(|_| "Invalid JSONL record")?;
        let spans = detect(model, tagger, &record.text);
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
    let mut arguments = std::env::args_os().skip(1).peekable();
    if arguments.next_if_eq(&"update").is_some() {
        let check_only = match arguments.next() {
            None => false,
            Some(flag) if flag == "--check" && arguments.next().is_none() => true,
            _ => return Err("Usage: redakt update [--check]".into()),
        };
        return write_info(&update::run(check_only)?);
    }
    let mut weights = None;
    let mut jsonl = false;

    let mut reverse = false;
    let mut irreversible = false;

    let mut recovery_path = None;
    let mut code_path = None;
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
            Some("--reverse") if !reverse => reverse = true,
            Some("--irreversible") if !irreversible => irreversible = true,
            Some("--recovery") if recovery_path.is_none() => {
                recovery_path = Some(PathBuf::from(
                    arguments.next().ok_or("--recovery requires a path")?,
                ));
            }
            Some("--code-file") if code_path.is_none() => {
                code_path = Some(PathBuf::from(
                    arguments.next().ok_or("--code-file requires a path")?,
                ));
            }
            _ => return Err("Unknown or repeated argument; use --help".into()),
        }
    }

    if (jsonl || irreversible) && (reverse || recovery_path.is_some() || code_path.is_some()) {
        return Err("Recovery options require reversible plain text".into());
    }
    if reverse {
        if weights.is_some() {
            return Err("--reverse does not use weights".into());
        }
        let recovery = Recovery::select(recovery_path, code_path, true)?;
        return write_text(&recovery.reverse(&read_text()?)?);
    }

    let (model, tagger) = if let Some(path) = weights {
        let bytes = std::fs::read(path).map_err(|_| "Could not read native model")?;
        (Detector::from_bytes(&bytes)?, None)
    } else {
        (Detector::embedded()?, Some(PiiTagger::embedded()?))
    };
    if jsonl {
        stream(&model, tagger.as_ref())?;
    } else {
        let text = read_text()?;
        let spans = detect(&model, tagger.as_ref(), &text);
        let masked = if irreversible {
            redact(&text, &spans)?
        } else {
            Recovery::select(recovery_path, code_path, false)?.mask(&text, spans)?
        };
        write_text(&masked)?;
    }
    update::notify();
    Ok(())
}

fn read_text() -> Result<String, String> {
    let mut text = String::new();
    io::stdin()
        .read_to_string(&mut text)
        .map_err(|_| "Could not read UTF-8 stdin")?;
    Ok(text)
}

fn write_text(text: &str) -> Result<(), String> {
    let mut output = io::stdout().lock();
    output
        .write_all(text.as_bytes())
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
