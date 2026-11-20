use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Output, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

fn invoke(input: &[u8], arguments: &[&str]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_privacy-hygiene"))
        .args(arguments)
        .env("PATH", "")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    thread::scope(|scope| {
        let writer = scope.spawn(move || stdin.write_all(input));
        let output = child.wait_with_output().unwrap();
        if let Err(error) = writer.join().unwrap() {
            assert!(!output.status.success());
            assert_eq!(error.kind(), std::io::ErrorKind::BrokenPipe);
        }
        output
    })
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
fn address_boundaries() {
    let street = "12 Oak Lane West Road";
    for ending in [
        "!", "?", ":", ")", "\"", "。", " ", "\t", "\r", "\r\n", "  !",
    ] {
        let input = format!("Address: {street}{ending}");
        let output = invoke(input.as_bytes(), &[]);
        assert!(output.status.success());
        let actual: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            actual,
            json!({
                "spans": [{"start": 9, "end": 9 + street.len(), "label": "address"}],
                "redacted": format!("Address: {}{ending}", "█".repeat(street.len()))
            }),
            "ending {ending:?}"
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
    for argument in ["--weights", "--unknown", "--contextual"] {
        let output = invoke(private_input.as_bytes(), &[argument]);
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

#[test]
fn native_model_matches_frozen_rules() {
    let model = privacy_hygiene::Detector::embedded().unwrap();
    for line in include_str!("fixtures/rules-parity.jsonl").lines() {
        let expected: Value = serde_json::from_str(line).unwrap();
        let text = expected["text"].as_str().unwrap();
        let spans = model.detect(text);
        let redacted = privacy_hygiene::redact(text, &spans).unwrap();
        assert_eq!(
            json!({"spans": spans, "redacted": redacted}),
            json!({"spans": expected["spans"], "redacted": expected["redacted"]}),
            "oracle record {}",
            expected["id"]
        );
    }
}

#[test]
fn jsonl_flushes_each_record_before_eof() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_privacy-hygiene"))
        .arg("--jsonl")
        .env("PATH", "")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let (sender, receiver) = mpsc::channel();
    let reader = thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            if sender.send(line).is_err() {
                break;
            }
        }
    });
    for id in [json!("first"), json!({"batch": 2}), json!(null)] {
        let record = json!({"id": id, "text": "🦊 password: a@example.test"});
        writeln!(stdin, "{record}").unwrap();
        stdin.flush().unwrap();
        let response = receiver.recv_timeout(Duration::from_secs(10));
        if response.is_err() {
            child.kill().unwrap();
            child.wait().unwrap();
            reader.join().unwrap();
            panic!("JSONL response was not flushed before EOF");
        }
        let line = response.unwrap().unwrap();
        let actual: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(
            actual,
            json!({
                "id": id,
                "spans": [
                    {"start": 12, "end": 26, "label": "email"},
                    {"start": 12, "end": 26, "label": "secret"}
                ],
                "redacted": "🦊 password: ██████████████"
            })
        );
    }
    drop(stdin);
    assert!(child.wait_with_output().unwrap().status.success());
    reader.join().unwrap();
}

#[test]
fn rejects_private_jsonl() {
    for input in [
        "private",
        "{\"id\":1,\"text\":\"private\",\"extra\":true}",
        "{\"id\":\"private\"}",
        "{\"id\":1,\"text\":42}",
    ] {
        let output = invoke(input.as_bytes(), &["--jsonl"]);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert_eq!(
            String::from_utf8(output.stderr).unwrap(),
            "privacy-hygiene: Invalid JSONL record\n"
        );
    }
}

fn model_json() -> Value {
    serde_json::from_slice(&include_bytes!("../models/weights.bin")[10..]).unwrap()
}

fn model_bytes(model: &Value) -> Vec<u8> {
    let payload = serde_json::to_vec(model).unwrap();
    let mut bytes = b"PHYT".to_vec();
    bytes.extend(2_u16.to_le_bytes());
    bytes.extend(u32::try_from(payload.len()).unwrap().to_le_bytes());
    bytes.extend(payload);
    bytes
}

#[test]
fn weights_control_predictions() {
    let text = concat!(
        "a@example.test 212-555-0123 123-45-6789 4111111111111111 ",
        "GB82WEST12345698765432 Name: Nora Chen Address: 123 Main Street"
    );
    let baseline = privacy_hygiene::Detector::embedded().unwrap().detect(text);
    let labels: std::collections::BTreeSet<_> =
        baseline.iter().map(|span| span.label.as_str()).collect();
    assert_eq!(
        labels,
        ["email", "phone", "ssn", "card", "iban", "name", "address"]
            .into_iter()
            .collect()
    );

    let mut model = model_json();
    for mask in model["masks"].as_array_mut().unwrap() {
        for byte in mask.as_array_mut().unwrap() {
            *byte = json!(0);
        }
    }
    let detector = privacy_hygiene::Detector::from_bytes(&model_bytes(&model)).unwrap();
    assert!(detector.detect(text).is_empty());

    let mut model = model_json();
    for checksum in ["luhn", "iban"] {
        for byte in model[checksum]["accept"].as_array_mut().unwrap() {
            *byte = json!(0);
        }
    }
    let detector = privacy_hygiene::Detector::from_bytes(&model_bytes(&model)).unwrap();
    let expected: Vec<_> = baseline
        .into_iter()
        .filter(|span| span.label != "card" && span.label != "iban")
        .collect();
    assert_eq!(detector.detect(text), expected);
}

#[test]
fn malformed_model_programs_are_rejected() {
    let mut invalid_models = Vec::new();

    let mut model = model_json();
    model["heads"][0]["entry"] = json!(model["instructions"].as_array().unwrap().len());
    invalid_models.push(("Invalid head", model));

    let mut model = model_json();
    model["instructions"][0] = json!([1, 9999, 0, 0]);
    invalid_models.push(("Invalid instruction operands", model));

    let mut model = model_json();
    model["masks"][0][26] = json!(9);
    invalid_models.push(("Invalid ternary padding", model));

    let mut model = model_json();
    model["instructions"] = json!([[2, 0, 0, 0]]);
    invalid_models.push(("Cyclic zero-width program", model));

    let mut model = model_json();
    model["instructions"] = json!([[1, 0, 1, 0], [4, 0, 2, 1], [0, 0, 0, 0]]);
    invalid_models.push(("Recursive assertion program", model));

    let mut model = model_json();
    let mut instructions = vec![json!([0, 0, 0, 0])];
    for depth in 1..=65 {
        instructions.push(json!([4, depth - 1, 0, 1]));
    }
    model["instructions"] = json!(instructions);
    invalid_models.push(("Assertion nesting exceeds model limit", model));
    for (reason, model) in invalid_models {
        let error = privacy_hygiene::Detector::from_bytes(&model_bytes(&model))
            .err()
            .expect("malformed model was accepted");
        assert_eq!(error, reason);
    }
}
