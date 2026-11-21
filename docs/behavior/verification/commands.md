# Command journeys

Use the [verification setup](README.md#how-to-run-a-pass). Run each row in a fresh invocation. The command snippets assume Bash from the repository root and a writable scratch directory.

## cli/information.md

| ID | P | Device | Claim | Setup | Steps | Expected | Result |
| --- | --- | --- | --- | --- | --- | --- | --- |
| INFO-01 | P1 | Linux, redirected streams | Help aliases agree ([spec](../cli/information.md#invoke)). | Release binary and scratch directory. | 1. Run `r INFO-01 '' --help; r INFO-01b '' -h; cmp -s "$scratch/INFO-01.out" "$scratch/INFO-01b.out" && head -1 "$scratch/INFO-01.out" \| grep -Fxq 'redakt [--weights PATH] [--jsonl]'`. | Both exit 0 and start with the synopsis. | pass |
| INFO-02 | P1 | Linux, redirected streams | Version aliases agree ([spec](../cli/information.md#invoke)). | Release binary and scratch directory. | 1. Run `r INFO-02 '' --version; r INFO-02b '' -V; cmp -s "$scratch/INFO-02.out" "$scratch/INFO-02b.out" && grep -Fxq 'redakt 0.0.5' "$scratch/INFO-02.out"`. | Both exit 0 and print `redakt 0.0.5`. | pass |
| INFO-03 | P1 | Linux, redirected streams | Ignore tokens after the reached information flag ([spec](../cli/information.md#exit-immediately)). | Release binary and scratch directory. | 1. Run `r INFO-03 '' --version --unknown`. | Version, status 0, no diagnostic. | pass |
| INFO-04 | P1 | Linux, redirected streams | Reject an earlier unknown token ([spec](../cli/information.md#exit-immediately)). | Release binary and scratch directory. | 1. Run `r INFO-04 '' --unknown --version`. | Status 1, empty stdout. | pass |
| INFO-05 | P1 | Linux, redirected streams | Skip model loading and input validation ([spec](../cli/information.md#begin-running)). | Release binary and scratch directory. | 1. Run `r INFO-05 $'\xff' --weights "$scratch/missing" --jsonl --version`. | Version, status 0, empty stderr. | pass |
| INFO-06 | P2 | Linux, redirected streams | First information flag wins ([spec](../cli/information.md#edge-cases)). | Release binary and scratch directory. | 1. Run `r INFO-06 '' --version --help`. | Only the version is printed. | pass |
| INFO-07 | P2 | Linux, redirected streams | Consume option-looking weights paths ([spec](../cli/information.md#edge-cases)). | Release binary and scratch directory. | 1. Run `(cd "$scratch" && r INFO-07 '' --weights --help)`. | Missing-model diagnostic, status 1. | pass |
| INFO-08 | P2 | Linux, redirected streams | Reject repeated options before information ([spec](../cli/information.md#edge-cases)). | Release binary and scratch directory. | 1. Run `r INFO-08 '' --jsonl --jsonl --help`. | Status 1 with an argument diagnostic. | pass |
| INFO-09 | P1 | Linux, closed pipe | Report failed information writes ([spec](../cli/information.md#finish)). | Rust toolchain and checkout. | 1. Run `cargo test --locked --test cli information_commands_report_closed_stdout` | Each alias exits 1 with the information-write diagnostic when stdout is closed. | pass (integration test) |
| INFO-10 | P2 | Linux, /dev/full | A full output destination fails the command ([spec](../cli/information.md#finish)). | Release binary and scratch directory. | 1. Run `"$binary" --version > /dev/full 2> "$scratch/err"; rc=$?; test "$rc" = 1 && grep -Fxq 'redakt: Could not write command information' "$scratch/err"`. | Status 1 and information-write diagnostic. | pass |

## cli/plain-text.md

| ID | P | Device | Claim | Setup | Steps | Expected | Result |
| --- | --- | --- | --- | --- | --- | --- | --- |
| PLAIN-01 | P1 | Linux, redirected streams | Preserve original CRLF and missing final newline ([spec](../cli/plain-text.md#while-running)). | Release binary and scratch directory. | 1. Run `r PLAIN-01 $'🦊 123-45-6789\r\nDOB: 2000-02-29'`. | Exact two-line result `🦊 [SSN_1]` + CRLF + `DOB: [DATE_1]`. | pass |
| PLAIN-02 | P1 | Linux terminal | Wait for EOF to output the document ([spec](../cli/plain-text.md#begin-running)). | Fresh terminal session. | 1. Launch redakt in a terminal, enter an email and Enter, then press Ctrl+D | Waits through Enter. Prints `[EMAIL_1]` on EOF. | pass (terminal) |
| PLAIN-03 | P1 | Linux, redirected streams | Reject invalid UTF-8 without partial stdout ([spec](../cli/plain-text.md#while-running)). | Release binary and scratch directory. | 1. Run `r PLAIN-03 $'a@example.test\n\xff'`. | Status 1, empty stdout, UTF-8 stdin diagnostic. | pass |
| PLAIN-04 | P2 | Linux, redirected streams | Treat empty input as success ([spec](../cli/plain-text.md#finish)). | Release binary and scratch directory. | 1. Run `r PLAIN-04 ''`. | Status 0, empty stdout and stderr. | pass |
| PLAIN-05 | P2 | Linux, redirected streams | Copy unmatched content exactly ([spec](../cli/plain-text.md#finish)). | Release binary and scratch directory. | 1. Run `r PLAIN-05 $'  sample\t🦊\r\n'`. | Status 0 and original bytes. | pass |
| PLAIN-06 | P2 | Linux, /dev/full | Report an output write error ([spec](../cli/plain-text.md#finish)). | Release binary and scratch directory. | 1. Run `"$binary" < "$scratch/email" > /dev/full 2> "$scratch/err"; rc=$?; test "$rc" = 1 && grep -Eq '^redakt: Could not (write\|flush) redacted text$' "$scratch/err"`. | Status 1 and a redacted-text output diagnostic. | pass |
| PLAIN-07 | P2 | Linux, redirected streams | Number across all physical lines ([spec](../cli/plain-text.md#edge-cases)). | Release binary and scratch directory. | 1. Run `r PLAIN-07 $'a@example.test\nb@example.test\na@example.test'`. | `[EMAIL_1]`, `[EMAIL_2]`, `[EMAIL_1]` on separate lines. | pass |

## cli/jsonl.md

| ID | P | Device | Claim | Setup | Steps | Expected | Result |
| --- | --- | --- | --- | --- | --- | --- | --- |
| JSONL-01 | P1 | Linux, redirected streams | Accept compound and null IDs ([spec](../cli/jsonl.md#invoke)). | Release binary and scratch directory. | 1. Run `r JSONL-01 $'{"text":"","id":{"run":[1]}}\n{"id":null,"text":""}' --jsonl`. | Two responses echo the object and null IDs, respectively. | pass |
| JSONL-02 | P1 | Linux, redirected streams | Reject extra, missing, duplicate, or mistyped fields ([spec](../cli/jsonl.md#invoke)). | Release binary and scratch directory. | 1. Run the five example inputs separately with `--jsonl` using `r`, with IDs `JSONL-02a` through `JSONL-02e`: `{"id":1,"text":"x","extra":1}`, `{"text":"x"}`, `{"id":1}`, `{"id":1,"id":2,"text":"x"}`, `{"id":1,"text":2}`. | Each exits 1 with the invalid record diagnostic and empty stdout. | pass |
| JSONL-03 | P2 | Linux, redirected streams | Empty stream gives no responses ([spec](../cli/jsonl.md#exit-immediately)). | Release binary and scratch directory. | 1. Run `r JSONL-03 '' --jsonl`. | Status 0, empty stdout and stderr. | pass |
| JSONL-04 | P2 | Linux, redirected streams | Ignore empty and Unicode whitespace lines ([spec](../cli/jsonl.md#begin-running)). | Release binary and scratch directory. | 1. Run `r JSONL-04 $'\n\u2003\r\n{"id":1,"text":""}\r\n' --jsonl`. | Exactly one empty document response. | pass |
| JSONL-05 | P2 | Linux, redirected streams | Decode escaped document line breaks ([spec](../cli/jsonl.md#begin-running)). | Release binary and scratch directory. | 1. Run `r JSONL-05 '{"id":1,"text":"hi\na@example.test"}' --jsonl`. | Redacted value has one decoded newline and an email placeholder. | pass |
| JSONL-06 | P1 | Linux terminal, open input | Deliver a response while stdin is open ([spec](../cli/jsonl.md#while-running)). | Fresh terminal session. | 1. Run `cargo test --locked --test cli jsonl_flushes_each_record_before_eof`. In a terminal, enter a complete record and press Enter | A full response is available before EOF. | pass (test and terminal) |
| JSONL-07 | P2 | Linux, redirected streams | IDs stay visible as supplied ([spec](../cli/jsonl.md#while-running)). | Release binary and scratch directory. | 1. Run `r JSONL-07 '{"id":"a@example.test","text":"a@example.test"}' --jsonl`. | ID is the original email. Text is redacted. | pass |
| JSONL-08 | P1 | Linux, redirected streams | Repeat IDs in order with fresh numbering ([spec](../cli/jsonl.md#while-running)). | Release binary and scratch directory. | 1. Run `r JSONL-08 $'{"id":1,"text":"a@example.test b@example.test"}\n{"id":1,"text":"b@example.test"}' --jsonl`. | IDs are `[1,1]`. Redacted text is `[EMAIL_1] [EMAIL_2]` and `[EMAIL_1]`. | pass |
| JSONL-09 | P1 | Linux, redirected streams | Keep earlier responses and stop on invalid JSON ([spec](../cli/jsonl.md#finish)). | Release binary and scratch directory. | 1. Run `r JSONL-09 $'{"id":1,"text":"a@example.test"}\ninvalid\n{"id":2,"text":"x"}' --jsonl`. | Status 1, one response for ID 1, invalid record diagnostic. | pass |
| JSONL-10 | P1 | Linux, redirected streams | Keep earlier responses and stop on invalid UTF-8 ([spec](../cli/jsonl.md#finish)). | Release binary and scratch directory. | 1. Run `r JSONL-10 $'{"id":1,"text":"a@example.test"}\n\xff\n{"id":2,"text":"x"}' --jsonl`. | Status 1, one response for ID 1, UTF-8 JSONL diagnostic. | pass |
| JSONL-11 | P2 | Linux, redirected streams | Process an unterminated final input line ([spec](../cli/jsonl.md#finish)). | Release binary and scratch directory. | 1. Run `r JSONL-11 '{"id":1,"text":"a@example.test"}' --jsonl`. | One complete newline-terminated response, status 0. | pass |
| JSONL-12 | P2 | Linux terminal | Interrupt after a flushed response ([spec](../cli/jsonl.md#cancel-and-interrupt)). | Fresh terminal session. | 1. Launch redakt --jsonl in a terminal, send one record, wait for output, and press Ctrl+C | Earlier response remains visible and the process stops. | pass (terminal) |
| JSONL-13 | P1 | Linux, redirected streams | Accept positional arrays (suspected bug) ([spec](../cli/jsonl.md#begin-running)). | Release binary and scratch directory. | 1. Run `r JSONL-13 '[1,"a@example.test"]' --jsonl`. | Observed status 0 and a normal response for ID 1. | pass |
| JSONL-14 | P2 | Linux, /dev/full | Stop on response output errors ([spec](../cli/jsonl.md#finish)). | Release binary and scratch directory. | 1. Run `printf '%s\n' '{"id":1,"text":""}' > "$scratch/in"; "$binary" --jsonl < "$scratch/in" > /dev/full 2> "$scratch/err"; rc=$?; test "$rc" = 1 && grep -Eq '^redakt: Could not (write\|flush) JSONL response$' "$scratch/err"`. | Status 1 and a JSONL output diagnostic. | pass |

## Restore

These checks used the release binary from `78fb4c7` on Linux with temporary
paths and a synthetic email. The `LOCALAPPDATA` row exercises environment
variable handling on Linux.

| ID | Priority | Environment | Claim | Setup | Steps | Expected result | Result |
| --- | --- | --- | --- | --- | --- | --- | --- |
| RESTORE-01 | P1 | Linux | [Default pair](../cli/restore.md#default-locations) | Set a temporary `HOME` and unset the three override variables. | 1. Mask a synthetic email. 2. List the default directory. 3. Pass the masked text to `redakt --reverse`. | One matching file pair under `$HOME/.local/state/redakt`. Reverse restores the original bytes. | pass |
| RESTORE-02 | P1 | Linux | [Directory precedence](../cli/restore.md#default-locations) | Use separate temporary directories for the three overrides. | 1. Mask with only `LOCALAPPDATA`. 2. Mask with `XDG_STATE_HOME` also set. 3. Mask with all three overrides set. | The pair goes under the local application data directory, then the XDG directory, then the exact redakt override path. | pass |
| RESTORE-03 | P1 | Linux | [Automatic selection failures](../cli/restore.md#choose-files-explicitly) | Prepare one directory with two mappings and point a second run at a missing directory. | 1. Run reverse without `--recovery` in each setup. | Both exit with an error and empty stdout. Diagnostics distinguish multiple candidates from no candidate. | pass |
| RESTORE-04 | P1 | Linux | [Explicit paths](../cli/restore.md#choose-files-explicitly) | Choose separate mapping and code paths. | 1. Mask with both flags. 2. Reverse with the same flags. | Files use the selected paths. Reverse restores the original bytes. | pass |
