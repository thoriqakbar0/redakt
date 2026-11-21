use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Output, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

fn invoke(input: &[u8], arguments: &[&str]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_redakt"))
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
fn update_uses_a_separate_argument_list() {
    let help = invoke(b"", &["--help"]);
    assert!(help.status.success());
    let help = String::from_utf8(help.stdout).unwrap();
    assert!(help.contains("redakt update [--check]"));
    assert!(help.contains("REDAKT_UPDATE_NOTIFY=0"));

    let invalid = invoke(b"", &["update", "--check", "--jsonl"]);
    assert!(!invalid.status.success());
    assert!(invalid.stdout.is_empty());
    assert_eq!(invalid.stderr, b"redakt: Usage: redakt update [--check]\n");
}

#[test]
fn pii_redacts_prose() {
    let input = concat!(
        "🦊 Sarah called this morning.\n",
        "Email: sarah@example.test\nPhone: 212-555-0123\nAddress: 12 Oak Lane\n"
    );
    let plain = invoke(input.as_bytes(), &[]);
    assert!(plain.status.success());
    let redacted = String::from_utf8(plain.stdout).unwrap();
    for value in ["Sarah", "sarah@example.test", "212-555-0123", "12 Oak Lane"] {
        assert!(!redacted.contains(value), "unmasked value {value}");
    }
    let request = json!({"id": "record", "text": input}).to_string();
    let output = invoke(request.as_bytes(), &["--jsonl"]);
    assert!(output.status.success());
    let record: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(record["redacted"], redacted);
    assert_eq!(record["id"], "record");
    let spans = record["spans"].as_array().unwrap();
    for (label, value) in [
        ("name", "Sarah"),
        ("email", "sarah@example.test"),
        ("phone", "212-555-0123"),
        ("address", "12 Oak Lane"),
    ] {
        let byte_start = input.find(value).unwrap();
        let start = input[..byte_start].chars().count();
        let end = start + value.chars().count();
        assert!(spans.contains(&json!({"start": start, "end": end, "label": label})));
    }
}

#[test]
fn detects_supported_layouts() {
    for (sample, placeholder) in [
        ("123-45-6789", "[SSN_1]"),
        ("212-555-0123", "[PHONE_1]"),
        ("+1212-555-0123", "[PHONE_1]"),
        ("+1 212-555-0123", "[PHONE_1]"),
        ("(212) 555-0123", "[PHONE_1]"),
        ("+1(212) 555-0123", "[PHONE_1]"),
        ("+1 (212) 555-0123", "[PHONE_1]"),
    ] {
        let input = format!("🦊 {sample}\r\n");
        let output = invoke(input.as_bytes(), &[]);
        assert!(output.status.success());
        assert_eq!(output.stdout, format!("🦊 {placeholder}\r\n").as_bytes());
    }
}

#[test]
fn numbers_distinct_values_by_category() {
    let input = concat!(
        "🦊 Name: Nora Chen\r\n",
        "Emails: nora@example.test, ada@example.test, nora@example.test, jo@example.test\r\n",
        "Name: Ada Green\r\nName: Nora Chen\r\n",
        "Address: 12 Oak Lane\r\nAddress: 48 Cedar Road\r\nAddress: 12 Oak Lane\r\n",
        "password: demo-first\r\napi_key: demo-second\r\npassword: demo-first\r\n",
        "Phone: 212-555-0123\r\nSSN: 123-45-6789\r\nCard: 4111111111111111\r\n",
        "IBAN: GB82WEST12345698765432\r\nBank account: 1234 5678 9012\r\nDOB: 2000-02-29"
    );
    let output = invoke(input.as_bytes(), &[]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        output.stdout,
        concat!(
            "🦊 Name: [NAME_1]\r\n",
            "Emails: [EMAIL_1], [EMAIL_2], [EMAIL_1], [EMAIL_3]\r\n",
            "Name: [NAME_2]\r\nName: [NAME_1]\r\n",
            "Address: [ADDRESS_1]\r\nAddress: [ADDRESS_2]\r\nAddress: [ADDRESS_1]\r\n",
            "password: [SECRET_1]\r\napi_key: [SECRET_2]\r\npassword: [SECRET_1]\r\n",
            "Phone: [PHONE_1]\r\nSSN: [SSN_1]\r\nCard: [CARD_1]\r\n",
            "IBAN: [IBAN_1]\r\nBank account: [ACCOUNT_NUMBER_1]\r\nDOB: [DATE_1]"
        )
        .as_bytes()
    );
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
        assert_eq!(
            output.stdout,
            format!("Address: [ADDRESS_1]{ending}").as_bytes(),
            "ending {ending:?}"
        );
    }
}

#[test]
fn exclusions_and_boundaries() {
    let model = concat!(env!("CARGO_MANIFEST_DIR"), "/models/weights.bin");
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
        let output = invoke(input.as_bytes(), &["--weights", model]);
        assert!(output.status.success());
        assert_eq!(output.stdout, input.as_bytes());
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

#[cfg(unix)]
#[test]
fn non_utf8_weights_path() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let filename = OsStr::from_bytes(b"synthetic-\xff");
    let run = |path: &std::path::Path| {
        Command::new(env!("CARGO_BIN_EXE_redakt"))
            .arg("--weights")
            .arg(path)
            .output()
            .unwrap()
    };

    let missing = run(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(filename));
    assert_eq!(missing.status.code(), Some(1));
    assert_eq!(missing.stderr, b"redakt: Could not read native model\n");
    assert!(missing.stdout.is_empty());

    // macOS filesystems can reject non-UTF-8 filenames.
    #[cfg(target_os = "linux")]
    {
        let dir = std::env::temp_dir().join(format!("redakt-weights-{}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join(filename);
        std::os::unix::fs::symlink(
            concat!(env!("CARGO_MANIFEST_DIR"), "/models/weights.bin"),
            &path,
        )
        .unwrap();
        let valid = run(&path);
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(dir).unwrap();

        assert!(valid.status.success());
        assert!(valid.stderr.is_empty());
    }
}

#[test]
fn information_commands_report_closed_stdout() {
    for flag in ["--help", "-h", "--version", "-V"] {
        let (reader, writer) = std::io::pipe().unwrap();
        drop(reader);
        let output = Command::new(env!("CARGO_BIN_EXE_redakt"))
            .arg(flag)
            .stdout(writer)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1), "{flag}");
        assert_eq!(
            output.stderr,
            b"redakt: Could not write command information\n"
        );
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
        assert!(redakt::Detector::from_bytes(&malformed).is_err());
    }
    let mut invalid_magic = valid.to_vec();
    invalid_magic[..4].copy_from_slice(b"BAD!");
    assert!(redakt::Detector::from_bytes(&invalid_magic).is_err());
}

#[test]
fn native_model_matches_frozen_rules() {
    let model = redakt::Detector::embedded().unwrap();
    for line in include_str!("fixtures/rules-parity.jsonl").lines() {
        let expected: Value = serde_json::from_str(line).unwrap();
        let text = expected["text"].as_str().unwrap();
        let spans = model.detect(text);
        assert_eq!(
            json!(spans),
            expected["spans"],
            "oracle record {}",
            expected["id"]
        );
    }
}

#[test]
fn jsonl_flushes_each_record_before_eof() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_redakt"))
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
        let record = json!({"id": id, "text": "🦊 password: a@example.test b@example.test"});
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
        assert_eq!(actual["id"], id);
        assert_eq!(actual["redacted"], "🦊 password: [EMAIL_1] [EMAIL_2]");
        let spans = actual["spans"].as_array().unwrap();
        for expected in [
            json!({"start": 12, "end": 26, "label": "email"}),
            json!({"start": 12, "end": 26, "label": "secret"}),
            json!({"start": 27, "end": 41, "label": "email"}),
        ] {
            assert!(spans.contains(&expected));
        }
        let ordered: Vec<_> = spans
            .iter()
            .map(|span| {
                (
                    span["start"].as_u64().unwrap(),
                    span["end"].as_u64().unwrap(),
                    span["label"].as_str().unwrap(),
                )
            })
            .collect();
        assert!(ordered.windows(2).all(|pair| pair[0] < pair[1]));
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
            "redakt: Invalid JSONL record\n"
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
    let baseline = redakt::Detector::embedded().unwrap().detect(text);
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
    let detector = redakt::Detector::from_bytes(&model_bytes(&model)).unwrap();
    assert!(detector.detect(text).is_empty());

    let mut model = model_json();
    for checksum in ["luhn", "iban"] {
        for byte in model[checksum]["accept"].as_array_mut().unwrap() {
            *byte = json!(0);
        }
    }
    let detector = redakt::Detector::from_bytes(&model_bytes(&model)).unwrap();
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
        let error = redakt::Detector::from_bytes(&model_bytes(&model))
            .err()
            .expect("malformed model was accepted");
        assert_eq!(error, reason);
    }
}
