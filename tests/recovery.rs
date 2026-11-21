use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use tempfile::tempdir;

fn command(state: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_redakt"));
    command
        .env("REDAKT_STATE_DIR", state)
        .env("REDAKT_UPDATE_NOTIFY", "0")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}

fn run(state: &Path, arguments: &[&str], input: &str) -> Output {
    let mut child = command(state).args(arguments).spawn().unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

fn succeeded(output: Output) -> String {
    assert!(output.status.success(), "{:?}", output.stderr);
    assert!(output.stderr.is_empty());
    String::from_utf8(output.stdout).unwrap()
}

fn failed(output: Output) {
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(!output.stderr.is_empty());
    assert!(!String::from_utf8_lossy(&output.stderr).contains("ops@example.test"));
}

#[test]
fn restore_after_transfer() {
    let first = tempdir().unwrap();
    let second = tempdir().unwrap();
    let source = "🦊 ops@example.test\r\n";
    let masked = succeeded(run(first.path(), &[], source));
    assert!(!masked.contains("ops@example.test"));
    assert_eq!(
        succeeded(run(first.path(), &["--reverse"], &masked)),
        source
    );

    for entry in fs::read_dir(first.path()).unwrap() {
        let entry = entry.unwrap();
        if entry.path().extension().is_some_and(|part| part == "lock") {
            continue;
        }
        let bytes = fs::read(entry.path()).unwrap();
        assert!(!bytes.windows(16).any(|part| part == b"ops@example.test"));
        fs::copy(entry.path(), second.path().join(entry.file_name())).unwrap();
    }

    let token = masked.strip_prefix("🦊 ").unwrap().trim();
    let arbitrary = format!("Edited {token} [EMAIL_1] {token} [REDAKT_unknown] 🦊");
    drop(first);
    assert_eq!(
        succeeded(run(second.path(), &["--reverse"], &arbitrary)),
        "Edited ops@example.test [EMAIL_1] ops@example.test [REDAKT_unknown] 🦊"
    );
}

#[test]
fn recovery_scope_is_explicit_and_stable() {
    let directory = tempdir().unwrap();
    let first = directory.path().join("first.recovery");
    let second = directory.path().join("second.recovery");
    let first_args = ["--recovery", first.to_str().unwrap()];
    let second_args = ["--recovery", second.to_str().unwrap()];
    let original = "ops@example.test";
    let masked = succeeded(run(directory.path(), &first_args, original));
    assert_eq!(
        succeeded(run(directory.path(), &first_args, original)),
        masked
    );
    assert_ne!(
        succeeded(run(directory.path(), &second_args, original)),
        masked
    );
    failed(run(directory.path(), &["--reverse"], &masked));

    let scoped = ["--reverse", "--recovery", first.to_str().unwrap()];
    let literal = succeeded(run(directory.path(), &first_args, &masked));
    assert_ne!(literal, masked);
    assert_eq!(succeeded(run(directory.path(), &scoped, &literal)), masked);
    let unrelated = ["--reverse", "--recovery", second.to_str().unwrap()];
    assert_eq!(
        succeeded(run(directory.path(), &unrelated, &masked)),
        masked
    );
}

#[test]
fn recovery_failures_hide_output() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("one.recovery");
    let code = path.with_added_extension("code");
    let mask = ["--recovery", path.to_str().unwrap()];
    let reverse = ["--reverse", "--recovery", path.to_str().unwrap()];
    let masked = succeeded(run(directory.path(), &mask, "ops@example.test"));
    let valid_code = fs::read(&code).unwrap();
    let valid_map = fs::read(&path).unwrap();
    fs::write(&code, "00".repeat(32)).unwrap();
    failed(run(directory.path(), &reverse, &masked));
    failed(run(directory.path(), &mask, "ops@example.test"));
    assert_eq!(fs::read(&path).unwrap(), valid_map);
    fs::write(&code, valid_code).unwrap();

    let mut corrupt = valid_map.clone();
    *corrupt.last_mut().unwrap() ^= 1;
    for damaged in [&corrupt[..], &valid_map[..valid_map.len() - 1]] {
        fs::write(&path, damaged).unwrap();
        failed(run(directory.path(), &reverse, &masked));
    }
    fs::write(&path, valid_map).unwrap();

    let missing = directory.path().join("missing/other.recovery");
    let unavailable = ["--recovery", missing.to_str().unwrap()];
    failed(run(directory.path(), &unavailable, "ops@example.test"));
    assert_eq!(
        succeeded(run(directory.path(), &reverse, &masked)),
        "ops@example.test"
    );
}

#[test]
fn stop_waiting_writer() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("shared.recovery");
    let mask = ["--recovery", path.to_str().unwrap()];
    let masked = succeeded(run(directory.path(), &mask, "ops@example.test"));
    let committed = fs::read(&path).unwrap();
    let lock = fs::OpenOptions::new()
        .write(true)
        .open(path.with_added_extension("lock"))
        .unwrap();
    lock.lock().unwrap();

    let mut child = command(directory.path()).args(mask).spawn().unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"another@example.test")
        .unwrap();
    child.kill().unwrap();
    let stopped = child.wait_with_output().unwrap();
    assert!(!stopped.status.success());
    assert!(stopped.stdout.is_empty());
    assert_eq!(fs::read(&path).unwrap(), committed);
    drop(lock);

    let updated = succeeded(run(directory.path(), &mask, "another@example.test"));
    let reverse = ["--reverse", "--recovery", path.to_str().unwrap()];
    let both = format!("{masked} {updated}");
    assert_eq!(
        succeeded(run(directory.path(), &reverse, &both)),
        "ops@example.test another@example.test"
    );
}
