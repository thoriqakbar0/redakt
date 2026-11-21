use serde_json::{json, Value};
use std::io::Write;
use std::process::{Command, Output, Stdio};

fn invoke(text: &str, arguments: &[&str]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_redakt"))
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(text.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

const RULE_WEIGHTS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/models/weights.bin");

fn json_response(text: &str, arguments: &[&str]) -> Value {
    let input = format!("{}\n", json!({"id":1,"text":text}));
    let output = invoke(&input, arguments);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn rule_response(text: &str) -> Value {
    json_response(text, &["--jsonl", "--weights", RULE_WEIGHTS])
}

#[test]
fn plain_stdin_replaces_cued_values() {
    let output = invoke("🦊 DOB: 2000-02-29\r\npassword: abc", &[]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(
        output.stdout,
        "🦊 DOB: [DATE_1]\r\npassword: [SECRET_1]".as_bytes()
    );
}

#[test]
fn contextual_values_and_unicode_offsets() {
    for (text, value, label) in [
        (
            "🦊 FULL NAME: élise o’neill; hello",
            "élise o’neill",
            "name",
        ),
        ("patient is 李 明\r\nnext", "李 明", "name"),
        ("customer = ana-maría cruz, next", "ana-maría cruz", "name"),
        ("my name is ada lovelace.", "ada lovelace", "name"),
        (
            "\"api_key\": \"abc-123_xyz\", next",
            "abc-123_xyz",
            "secret",
        ),
        ("password is hunter2", "hunter2", "secret"),
        (
            "bank account: 1234 5678 9012; next",
            "1234 5678 9012",
            "account_number",
        ),
        ("DOB=29/02/2000", "29/02/2000", "date"),
        ("Birth date: 02/29/2000", "02/29/2000", "date"),
        ("DOB=2000-02-29", "2000-02-29", "date"),
        ("address: 12 lower road; next", "12 lower road", "address"),
        ("SHIP TO: 42 Maple Street.", "42 Maple Street", "address"),
        ("lives at 42 Maple Street", "42 Maple Street", "address"),
    ] {
        let actual = json_response(text, &["--jsonl"]);
        let byte_start = text.find(value).unwrap();
        let start = text[..byte_start].chars().count();
        let end = start + value.chars().count();
        assert!(
            actual["spans"]
                .as_array()
                .unwrap()
                .contains(&json!({"start":start,"end":end,"label":label})),
            "{text}: {actual}"
        );
    }
}

#[test]
fn rejects_invalid_values() {
    for text in [
        "ada lovelace 2000-02-29",
        "DOB: 29/02/1900",
        "DOB: 12/0x/1990",
        "Release date: 12/04/1990",
        "DOB: 1900-02-29",
        "DOB: 2024-04-31",
        "DOB: 0000-01-01",
        "DOB: 2024-01-011",
        "password:\r\nprivate",
        "username: ada lovelace",
        "name: this is unrelated prose",
        "api_key: \"unterminated",
        "account number: 12345",
        "account number: 12345678901234567890123456789012345",
    ] {
        assert_eq!(rule_response(text)["spans"], json!([]), "{text}");
    }
    assert_eq!(
        rule_response(&format!("password: {}", "x".repeat(300)))["spans"],
        json!([])
    );
}

#[test]
fn contextual_jsonl_with_file_weights() {
    let model = concat!(env!("CARGO_MANIFEST_DIR"), "/models/weights.bin");
    let input = format!("{}\n", json!({"id":1,"text":"password: abc"}));
    let output = invoke(&input, &["--jsonl", "--weights", model]);
    assert!(output.status.success());
    let actual: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        actual,
        json!({
            "id": 1,
            "spans": [{"start": 10, "end": 13, "label": "secret"}],
            "redacted": "password: [SECRET_1]"
        })
    );
}

#[test]
fn aliases_quotes_and_adjacent_fields() {
    for cue in [
        "password",
        "passwd",
        "api_key",
        "api key",
        "access_token",
        "access token",
        "client_secret",
        "client secret",
    ] {
        for separator in [":", "=", " is "] {
            let text = format!("\"{cue}\"{separator}\"abc.def+/==\"");
            let actual = rule_response(&text);
            let start = text.find("abc").unwrap();
            assert_eq!(
                actual["spans"],
                json!([{"start":start,"end":start+11,"label":"secret"}])
            );
        }
    }
    for cue in ["DOB", "date of birth", "birth date"] {
        assert_eq!(
            rule_response(&format!("{cue} is 2024-02-29"))["spans"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }
    let text = "name: ada lovelace password: abc.def== DOB: 2024-02-29\r\nname: 李 明  \r\n";
    let actual = rule_response(text);
    assert_eq!(actual["spans"].as_array().unwrap().len(), 4);
    assert_eq!(
        actual["redacted"],
        "name: [NAME_1] password: [SECRET_1] DOB: [DATE_1]\r\nname: [NAME_2]  \r\n"
    );
    let spans = rule_response("Name: Nora Chen")["spans"]
        .as_array()
        .unwrap()
        .clone();
    assert_eq!(spans.len(), 1);
}

#[test]
fn disabled_weights() {
    let mut model: Value =
        serde_json::from_slice(&include_bytes!("../models/weights.bin")[10..]).unwrap();
    for mask in model["masks"].as_array_mut().unwrap() {
        for byte in mask.as_array_mut().unwrap() {
            *byte = json!(0);
        }
    }
    let payload = serde_json::to_vec(&model).unwrap();
    let mut bytes = b"PHYT".to_vec();
    bytes.extend(2_u16.to_le_bytes());
    bytes.extend(u32::try_from(payload.len()).unwrap().to_le_bytes());
    bytes.extend(payload);
    let path = std::env::temp_dir().join(format!("privacy-contextual-{}.bin", std::process::id()));
    std::fs::write(&path, bytes).unwrap();
    let input = format!(
        "{}\n",
        json!({"id":1,"text":"password: abc a@example.test"})
    );
    let output = invoke(&input, &["--jsonl", "--weights", path.to_str().unwrap()]);
    std::fs::remove_file(path).unwrap();
    assert!(output.status.success());
    let actual: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        actual["spans"],
        json!([{"start":10,"end":13,"label":"secret"}])
    );
}

#[test]
fn complete_secret_values() {
    for (text, secret) in [
        ("api_key=abc.def==", "abc.def=="),
        ("access token: abc:def==", "abc:def=="),
        ("password='some pass phrase'", "some pass phrase"),
        ("password=\"don't share\"", "don't share"),
        (
            "\"password\": \"some pass phrase\", next",
            "some pass phrase",
        ),
    ] {
        let start = text.find(secret).unwrap();
        let end = start + secret.len();
        let actual = rule_response(text);
        assert_eq!(
            actual["spans"],
            json!([{"start":start,"end":end,"label":"secret"}])
        );
        let redacted = format!("{}[SECRET_1]{}", &text[..start], &text[end..]);
        assert_eq!(actual["redacted"], redacted);
    }
    for text in [
        "password='some pass phrase",
        "password='   '",
        "api_key=abc\\def",
        "api_key=abc\"def",
        "password=\"abc\"def",
    ] {
        assert_eq!(rule_response(text)["spans"], json!([]));
    }
}

#[test]
fn bounded_values_and_overlapping_labels() {
    for length in [256, 257] {
        let text = format!("password: {}", "x".repeat(length));
        let actual = rule_response(&text);
        let expected = if length == 256 {
            json!([{"start":10,"end":266,"label":"secret"}])
        } else {
            json!([])
        };
        assert_eq!(actual["spans"], expected);
    }
    let actual = rule_response("password: a@example.test");
    assert_eq!(
        actual["spans"],
        json!([
            {"start":10,"end":24,"label":"email"},
            {"start":10,"end":24,"label":"secret"}
        ])
    );
}
