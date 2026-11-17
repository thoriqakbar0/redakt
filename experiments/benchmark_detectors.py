"""Compare warm, persistent JSONL detectors on identical individual documents."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import selectors
import statistics
import subprocess
import sys
import time

from detector import redact
from experiments.evaluate import parse_spans

ROOT = Path(__file__).resolve().parent.parent
LABELS = {"email", "phone", "name", "address", "ssn", "card", "iban",
          "account_number", "private_date", "private_url", "secret"}
WARMUP = {"id": "warmup", "text": (
    "Name: Mira Vale; Address: 12 Oak Street; mira@example.test; 212-555-1234; "
    "SSN 123-45-6789; 4111 1111 1111 1111; GB82 WEST 1234 5698 7654 32.")}


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def cpu_seconds(pid):
    fields = Path(f"/proc/{pid}/stat").read_text().rsplit(") ", 1)[1].split()
    return (int(fields[11]) + int(fields[12])) / os.sysconf("SC_CLK_TCK")


def peak_rss(pid):
    for line in Path(f"/proc/{pid}/status").read_text().splitlines():
        if line.startswith("VmHWM:"):
            return int(line.split()[1])
    raise RuntimeError(f"Missing VmHWM for worker {pid}")


class Worker:
    def __init__(self, command, timeout):
        self.timeout = timeout
        started = time.perf_counter()
        self.process = subprocess.Popen(command, cwd=ROOT, stdin=subprocess.PIPE,
                                        stdout=subprocess.PIPE)
        self.selector = selectors.DefaultSelector()
        self.selector.register(self.process.stdout, selectors.EVENT_READ)
        self.pending = b""
        try:
            self.request(WARMUP)
        except BaseException:
            self.close(force=True)
            raise
        self.startup_warmup_seconds = time.perf_counter() - started

    def request(self, row):
        payload = json.dumps(row, ensure_ascii=False).encode() + b"\n"
        self.process.stdin.write(payload)
        self.process.stdin.flush()
        deadline = time.monotonic() + self.timeout
        while b"\n" not in self.pending:
            remaining = deadline - time.monotonic()
            if remaining <= 0 or not self.selector.select(remaining):
                raise TimeoutError(f"Worker timed out on {row['id']}")
            chunk = os.read(self.process.stdout.fileno(), 65536)
            if not chunk:
                raise RuntimeError(f"Worker exited on {row['id']}: {self.process.poll()}")
            self.pending += chunk
        line, self.pending = self.pending.split(b"\n", 1)
        response = json.loads(line)
        validate(row, response)
        return response

    def close(self, force=False):
        if force and self.process.poll() is None:
            self.process.kill()
        try:
            self.process.stdin.close()
        except BrokenPipeError:
            pass
        try:
            code = self.process.wait(timeout=self.timeout)
        except subprocess.TimeoutExpired:
            self.process.kill()
            self.process.wait()
            raise TimeoutError("Worker failed to exit")
        finally:
            self.selector.close()
            self.process.stdout.close()
        if code and not force:
            raise RuntimeError(f"Worker exited with status {code}")


def validate(row, response):
    if response.keys() != {"id", "spans", "redacted"} or response["id"] != row["id"]:
        raise ValueError(f"Invalid response for {row['id']}")
    spans = parse_spans(response, row["text"])
    if any(span.label not in LABELS for span in spans):
        raise ValueError(f"Unknown span label for {row['id']}")
    if response["redacted"] != redact(row["text"], spans):
        raise ValueError(f"Invalid redaction for {row['id']}")


def pass_rows(worker, rows):
    return [worker.request({"id": row["id"], "text": row["text"]}) for row in rows]


def scalability_rows(corpus):
    return [
        {"id": "mixed", "text": "\n".join(row["text"] for row in corpus) * 100},
        {"id": "clean", "text": "Plain ordinary sentences without personal information. " * 5000},
        {"id": "invalid", "text": "4111111111111112;DE90370400440532013000;000-12-1234;" * 1000},
        {"id": "unbounded", "text": (
            "a" * 10000 + "@" + "b" * 1000 + ".test; Name: A" + "a" * 5000 +
            " Chen. Address: 12 " + "Long " * 800 + "Road.")},
    ]


def check_stable(name, actual, expected, repeat):
    if actual != expected:
        raise ValueError(f"{name} output changed in trial {repeat + 1}")


def package_versions(python):
    script = "import importlib.metadata as m, json; "
    script += "print(json.dumps({n: m.version(n) for n in "
    script += "['torch', 'numpy', 'opf', 'tiktoken', 'safetensors']}))"
    return json.loads(subprocess.check_output([python, "-c", script], text=True))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--corpus", type=Path, default=ROOT / "data/synthetic.jsonl")
    parser.add_argument("--scalability", action="store_true",
                        help="Compare only rules/native on generated long-document workloads")
    parser.add_argument("--native", type=Path, default=ROOT / "target/release/privacy-hygiene")
    parser.add_argument("--baseline-python", default=sys.executable)
    parser.add_argument("--privacy-filter", action="store_true")
    parser.add_argument("--checkpoint", type=Path)
    parser.add_argument("--threads", type=int, default=4)
    parser.add_argument("--grouped-cpu", action="store_true")

    parser.add_argument("--repeats", type=int, default=5)
    parser.add_argument("--timeout", type=float, default=300)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--predictions", type=Path)

    args = parser.parse_args()
    if args.repeats < 5 or args.threads < 1 or args.timeout <= 0:
        parser.error("requires at least five repeats, positive threads and timeout")

    if args.privacy_filter and (not args.checkpoint or not args.checkpoint.is_dir()):
        parser.error("--privacy-filter requires an existing local --checkpoint")
    if args.predictions and not args.privacy_filter:
        parser.error("--predictions requires --privacy-filter")
    if args.grouped_cpu and not args.privacy_filter:
        parser.error("--grouped-cpu requires --privacy-filter")
    if args.scalability and args.privacy_filter:
        parser.error("--scalability is a separate rules/native track")
    rows = [json.loads(line) for line in args.corpus.read_text().splitlines() if line.strip()]
    if not rows or len({row["id"] for row in rows}) != len(rows):
        parser.error("corpus must contain documents with unique ids")
    if args.scalability:
        rows = scalability_rows(rows)
    native = args.native.resolve()
    commands = {
        "rules": [sys.executable, "-m", "experiments.benchmark_worker", "rules"],
        "ternary": [str(native), "--jsonl"],
    }
    if args.privacy_filter:
        commands["privacy-filter"] = [args.baseline_python, "-m", "experiments.benchmark_worker",
                                      "privacy-filter", "--checkpoint", str(args.checkpoint.resolve()),
                                      "--threads", str(args.threads)]
        if args.grouped_cpu:
            commands["privacy-filter"].append("--grouped-cpu")
    env = {name: os.environ.get(name) for name in
           ("OMP_NUM_THREADS", "MKL_NUM_THREADS", "OPENBLAS_NUM_THREADS")}
    sources = [
        ROOT / "detector.py", ROOT / "src/lib.rs", ROOT / "src/main.rs",
        ROOT / "experiments/compile_ternary.py", ROOT / "experiments/ternary.py",
        ROOT / "experiments/benchmark_worker.py", Path(__file__),
        ROOT / "experiments/run_baseline.py", ROOT / "experiments/optimized_opf.py",
    ]
    inputs = sources + [native, ROOT / "models/weights.bin", args.corpus]
    initial_hashes = {path: sha256(path) for path in inputs}
    load_start = os.getloadavg()
    workers = {}
    try:
        for name, command in commands.items():
            workers[name] = Worker(command, args.timeout)
        expected = {name: pass_rows(worker, rows) for name, worker in workers.items()}
        if expected["rules"] != expected["ternary"]:
            raise ValueError("Rules and ternary outputs differ before timing")
        samples = {name: [] for name in workers}
        cpu = {name: [] for name in workers}
        names = list(workers)
        orders = []
        for repeat in range(args.repeats):
            offset = repeat % len(names)
            order = names[offset:] + names[:offset]
            if (repeat // len(names)) % 2:
                order.reverse()
            orders.append(order.copy())
            trial = {}
            for name in order:
                worker = workers[name]

                cpu_before = cpu_seconds(worker.process.pid)
                started = time.perf_counter()
                trial[name] = pass_rows(worker, rows)
                samples[name].append(time.perf_counter() - started)
                cpu[name].append(cpu_seconds(worker.process.pid) - cpu_before)

                check_stable(name, trial[name], expected[name], repeat)

            if trial["rules"] != trial["ternary"]:
                raise ValueError(f"Rules and ternary differ in trial {repeat + 1}")
            print(f"Completed trial {repeat + 1}/{args.repeats}", file=sys.stderr, flush=True)
        rss = {name: peak_rss(worker.process.pid) for name, worker in workers.items()}
        startup = {name: worker.startup_warmup_seconds for name, worker in workers.items()}
    finally:
        failed = sys.exc_info()[0] is not None
        for worker in workers.values():
            worker.close(force=failed)
    if any(sha256(path) != digest for path, digest in initial_hashes.items()):
        raise RuntimeError("Benchmark source, executable, model, or corpus changed during the run")
    checkpoint_hashes = None
    if args.checkpoint:
        checkpoint_hashes = {}
        for path in args.checkpoint.iterdir():
            if path.is_file():
                checkpoint_hashes[path.name] = sha256(path)

    packages = package_versions(args.baseline_python) if args.privacy_filter else None

    report = {
        "description": "Cross-language deployed implementations, warm persistent JSONL, "
                       "one document per request; wall times include transport and parent validation, "
                       "not pure detector kernel time.",
        "corpus_sha256": sha256(args.corpus), "documents_per_trial": len(rows),
        "workload": "scalability" if args.scalability else "corpus",
        "characters_per_trial": sum(len(row["text"]) for row in rows),
        "request_sha256": hashlib.sha256(json.dumps(
            [{"id": row["id"], "text": row["text"]} for row in rows],
            ensure_ascii=False, sort_keys=True,
        ).encode()).hexdigest(),
        "source_sha256": {str(path.relative_to(ROOT)): sha256(path) for path in sources},
        "native_executable_sha256": sha256(native), "commands": commands,
        "artifact_bytes": {
            "native_executable_including_model": native.stat().st_size,
            "native_model": (ROOT / "models/weights.bin").stat().st_size,
            "python_reference_archive": (ROOT / "models/weights.npz").stat().st_size,
            "rules_source_excluding_python": (ROOT / "detector.py").stat().st_size,
            "privacy_filter_checkpoint_excluding_runtime": sum(
                path.stat().st_size for path in args.checkpoint.iterdir() if path.is_file()
            ) if args.checkpoint else None,
        },
        "native_model_sha256": sha256(ROOT / "models/weights.bin"),
        "python": sys.version, "baseline_python": subprocess.check_output(
            [args.baseline_python, "--version"], text=True).strip(),
        "cpu_model": next(line.split(":", 1)[1].strip() for line in
                          Path("/proc/cpuinfo").read_text().splitlines()
                          if line.startswith("model name")),
        "platform": platform.platform(), "cpu_affinity": sorted(os.sched_getaffinity(0)),
        "load_average_start": load_start, "load_average_end": os.getloadavg(),
        "environment": env, "threads": args.threads, "grouped_cpu": args.grouped_cpu,
        "checkpoint": str(args.checkpoint.resolve()) if args.checkpoint else None,
        "checkpoint_sha256": checkpoint_hashes, "baseline_packages": packages,
        "warmup": WARMUP, "startup_and_warmup_seconds": startup, "trial_order": orders,
        "trials": {name: {"wall_seconds": values, "median_seconds": statistics.median(values),
                          "range_seconds": [min(values), max(values)], "cpu_seconds": cpu[name],
                          "worker_peak_rss_kib": rss[name]} for name, values in samples.items()},
        "errors": 0,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n")
    if args.predictions:
        args.predictions.parent.mkdir(parents=True, exist_ok=True)
        predictions = []
        for row, result in zip(rows, expected["privacy-filter"]):
            predictions.append({"id": row["id"], "text": row["text"], "spans": result["spans"]})

        lines = [json.dumps(row, ensure_ascii=False) + "\n" for row in predictions]
        args.predictions.write_text("".join(lines))


if __name__ == "__main__":
    main()
