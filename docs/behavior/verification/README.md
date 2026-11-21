# Hand verification

Each checklist row ties an observable behavior to its owning section. Check the text, diagnostics, and exit status separately. The running product is the release binary built from the pinned source.

## Checklists

| File | Covers |
| --- | --- |
| [Foundations](foundations.md) | Argument handling, process behavior, numbering, and span coordinates |
| [Command journeys](commands.md) | Help and version, plain text, and JSONL |
| [Detection and weights](detection.md) | Recognition boundaries and selecting a model file |

P1 covers shared behavior and suspected defects. P2 covers ordinary cases. P3 covers limits and timings.

## How to run a pass

1. From the repository root, run `git rev-parse --short HEAD`. Use a checkout of `6ba6de0` to reproduce the pinned pass. When testing a later revision, record its commit so behavior changes can be distinguished from documentation errors.
2. Run `cargo build --release --locked` and confirm that `target/release/redakt --version` prints `redakt 0.0.5` on the pinned revision. Use Bash on Linux and synthetic inputs. Install jq to inspect structured responses.
3. Prepare an isolated scratch directory and the helper below. In the checklists, `r` sends its input argument verbatim and captures stdout, stderr, and exit status by row ID. Add a newline explicitly when a row needs one.

   ```bash
   binary="$PWD/target/release/redakt"
   scratch=$(mktemp -d)
   printf %s a@example.test > "$scratch/email"
   r() {
       local id=$1 input=$2 rc=0
       shift 2
       printf %s "$input" > "$scratch/$id.in"
       "$binary" "$@" < "$scratch/$id.in" > "$scratch/$id.out" 2> "$scratch/$id.err" || rc=$?
       printf '%s\n' "$rc" > "$scratch/$id.status"
   }
   ```

4. Read each linked section. Run P1, then P2, then P3. Unless an error is expected, check for status 0 and empty stderr. Compare exact bytes if the row mentions line endings. Use jq to compare parsed JSON values when key order and escaping are incidental.
5. Mark each row `pass`, `fail`, or `blocked`. Add conditions or evidence when they affect the result. On a suspected-bug row, a pass means the documented observation reproduced. Record whether the defect itself is confirmed in [triage](../bug-triage.md).
6. Update the [coverage table](../README.md#coverage) after a complete manual pass for that document. Each P1 and P2 claim must pass or be filed in triage. Keep checklist and triage IDs stable.

## Devices and conditions

- **Redirected streams.** Use separate scratch destinations and the helper above. For a closed-stdout case, use the existing CLI test with an already-closed pipe reader. An ordinary pipeline can race with the writer.
- **Linux terminal.** Start the executable directly. Terminal input echo is distinct from redakt's output. In Bash, Ctrl+D on an empty line ends stdin. Ctrl+C interrupts a foreground run.
- **Held pipe.** Use a fresh named pipe or coprocess and keep the write end open while checking for a response. To check signals, send one to the redakt PID after a flushed response and use `wait` to capture its status. For file weights, use a scratch copy that can be renamed while the stream is open.
- **Unwritable destination.** On Linux, redirect the specified stream to `/dev/full` to reject writes without filling a disk.
- **Mutable source file.** Use a temporary file. Change it while redakt is reading and record exactly which bytes reached stdin.
- **Rust toolchain and checkout.** Existing tests supply controlled pipe, model, or span fixtures where a simple shell invocation is impractical.

## Results so far

On 2026-11-21, the Linux x86-64 release build at `6ba6de0` passed 72 of 73 checklist rows. One file-mutation timing row remains open. Four findings in triage reproduced across five rows. The pass used shell assertions, existing Rust tests, and live terminal checks. Plain text waited for EOF, JSONL responded before EOF, and an interrupt left the flushed response visible.

The coverage table remains at `drafted`. These checks sampled the CLI on Linux and included manual observations of selected paths. A complete manual pass, startup signal races, concurrent mutation during initial model load, and other operating systems remain open. The independent accuracy report used only its selected synthetic examples.
