use serde_json::{json, Value};
use std::io::{ErrorKind, Write};
use std::process::{Command, Output, Stdio};

fn invoke(input: &[u8], arguments: &[&str]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_privacy-hygiene"))
        .args(arguments)
        .env("PATH", "")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    if let Err(error) = child.stdin.take().unwrap().write_all(input) {
        assert_eq!(error.kind(), ErrorKind::BrokenPipe, "{error}");
    }
    child.wait_with_output().unwrap()
}

#[test]
fn detects_supported_layouts() {
    for (index, sample) in [
        "123-45-6789",
        "212-555-0123",
        "+1212-555-0123",
        "+1 212-555-0123",
        "(212) 555-0123",
        "+1(212) 555-0123",
        "+1 (212) 555-0123",
    ]
    .iter()
    .enumerate()
    {
        let input = format!("🦊 {sample}\r\n");
        let output = invoke(input.as_bytes(), &[]);
        assert!(output.status.success());
        let actual: Value = serde_json::from_slice(&output.stdout).unwrap();
        let label = if index == 0 { "ssn" } else { "phone" };
        assert_eq!(
            actual,
            json!({
                "spans": [{"start": 2, "end": 2 + sample.len(),
                           "label": label}],
                "redacted": format!("🦊 {}\r\n", "█".repeat(sample.len()))
            })
        );
    }
}

#[test]
fn exclusions_and_boundaries() {
    for input in [
        "",
        "000-12-1234",
        "666-12-1234",
        "900-12-1234",
        "123-00-1234",
        "123-45-0000",
        "é123-45-6789",
        "123-45-6789_",
        "-123-45-6789",
        "123-45-6789-",
        "112-555-0123",
        "212-155-0123",
        "Ⅷ212-555-0123",
    ] {
        let output = invoke(input.as_bytes(), &[]);
        assert!(output.status.success());
        let actual: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(actual, json!({"spans": [], "redacted": input}));
    }
}

#[test]
fn alternate_model_and_private_errors() {
    let model = concat!(env!("CARGO_MANIFEST_DIR"), "/models/weights.bin");
    let output = invoke(b"123-45-6789", &["--weights", model]);
    assert!(output.status.success());
    assert_eq!(output.stdout, invoke(b"123-45-6789", &[]).stdout);

    let private_input = "private".repeat(1024 * 1024);
    for arguments in [vec!["--weights"], vec!["--unknown"]] {
        let output = invoke(private_input.as_bytes(), &arguments);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(!String::from_utf8_lossy(&output.stderr).contains("private"));
    }
    let output = invoke(b"private\xff", &[]);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(!String::from_utf8_lossy(&output.stderr).contains("private"));
    for argument in ["--help", "--version"] {
        assert!(invoke(b"", &[argument]).status.success());
    }
}

#[test]
fn malformed_models_are_rejected() {
    let valid = include_bytes!("../models/weights.bin");
    for malformed in [
        vec![],
        valid[..20].to_vec(),
        [valid.as_slice(), b"extra"].concat(),
    ] {
        assert!(privacy_hygiene::Detector::from_bytes(&malformed).is_err());
    }
    let mut invalid_magic = valid.to_vec();
    invalid_magic[..4].copy_from_slice(b"BAD!");
    assert!(privacy_hygiene::Detector::from_bytes(&invalid_magic).is_err());
}
