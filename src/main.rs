use privacy_hygiene::{redact, Detector};
use serde_json::json;
use std::io::{self, Read};

const HELP: &str = concat!(
    "privacy-hygiene [--weights PATH]\n",
    "Read UTF-8 stdin and emit JSON spans and redacted text.\n",
    "Offsets count Unicode code points.\n",
    "Detects SSNs and separated North American phones."
);

fn run() -> Result<(), String> {
    let mut arguments = std::env::args().skip(1);
    let mut weights = None;
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
            _ => return Err("Unknown or repeated argument; use --help".into()),
        }
    }
    let model = if let Some(path) = weights {
        let bytes = std::fs::read(path).map_err(|_| "Could not read native model")?;
        Detector::from_bytes(&bytes)?
    } else {
        Detector::embedded()?
    };
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
