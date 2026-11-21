use serde::Deserialize;
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::process::{Command, Stdio};
use std::thread;

const LABELS: &[&str] = &[
    "email",
    "phone",
    "ssn",
    "card",
    "iban",
    "name",
    "address",
    "secret",
    "account_number",
    "date",
    "url",
];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Annotation {
    value: String,
    label: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: String,
    text: String,
    expected: Vec<Annotation>,
}

#[derive(Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
struct Span {
    start: usize,
    end: usize,
    label: String,
}

#[derive(Deserialize)]
struct Response {
    id: String,
    spans: Vec<Span>,
}

#[derive(Default, Debug, PartialEq, Eq)]
struct Counts {
    correct: usize,
    extra: usize,
    missed: usize,
}

impl Case {
    fn spans(&self) -> BTreeSet<Span> {
        let mut spans = BTreeSet::new();
        for annotation in &self.expected {
            assert!(LABELS.contains(&annotation.label.as_str()), "{}", self.id);
            assert!(!annotation.value.is_empty(), "{}", self.id);
            let positions: Vec<_> = self
                .text
                .char_indices()
                .filter(|&(position, _)| self.text[position..].starts_with(&annotation.value))
                .collect();
            assert_eq!(positions.len(), 1, "ambiguous annotation in {}", self.id);
            let start = self.text[..positions[0].0].chars().count();
            assert!(
                spans.insert(Span {
                    start,
                    end: start + annotation.value.chars().count(),
                    label: annotation.label.clone(),
                }),
                "duplicate annotation in {}",
                self.id
            );
        }
        spans
    }
}

fn corpus() -> Vec<Case> {
    include_str!("fixtures/accuracy.jsonl")
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn score(expected: &BTreeSet<Span>, actual: &BTreeSet<Span>) -> BTreeMap<String, Counts> {
    let mut counts: BTreeMap<String, Counts> = LABELS
        .iter()
        .map(|label| (label.to_string(), Counts::default()))
        .collect();
    for span in expected {
        let count = counts.get_mut(&span.label).unwrap();
        if actual.contains(span) {
            count.correct += 1;
        } else {
            count.missed += 1;
        }
    }
    for span in actual.difference(expected) {
        counts.get_mut(&span.label).unwrap().extra += 1;
    }
    counts
}

fn ratio(numerator: usize, denominator: usize) -> String {
    if denominator == 0 {
        "n/a".into()
    } else {
        format!("{:.3}", numerator as f64 / denominator as f64)
    }
}

fn print_counts(label: &str, count: &Counts) {
    println!(
        "{label}: TP={} FP={} FN={} precision={} recall={}",
        count.correct,
        count.extra,
        count.missed,
        ratio(count.correct, count.correct + count.extra),
        ratio(count.correct, count.correct + count.missed),
    );
}

#[test]
fn valid_annotations() {
    let cases = corpus();
    let mut identifiers = BTreeSet::new();
    let mut labels = BTreeSet::new();
    let mut negatives = 0;
    for case in &cases {
        assert!(!case.id.is_empty());
        assert!(identifiers.insert(&case.id));
        let spans = case.spans();
        negatives += usize::from(spans.is_empty());
        labels.extend(spans.into_iter().map(|span| span.label));
    }
    assert!(cases.len() >= 40);
    assert!(negatives >= 10);
    assert_eq!(
        labels,
        LABELS.iter().map(|label| label.to_string()).collect()
    );
    let unicode: Case = serde_json::from_value(json!({
        "id": "unicode", "text": "🦊 élise", "expected": [{"value": "élise", "label": "name"}]
    }))
    .unwrap();
    assert_eq!(
        unicode.spans(),
        BTreeSet::from([Span {
            start: 2,
            end: 7,
            label: "name".into()
        }])
    );
}

#[test]
fn exact_span_scoring() {
    let span = |start, end, label: &str| Span {
        start,
        end,
        label: label.into(),
    };
    let expected = BTreeSet::from([
        span(0, 4, "name"),
        span(6, 10, "name"),
        span(12, 16, "secret"),
    ]);
    let actual = BTreeSet::from([span(0, 4, "name"), span(6, 9, "name"), span(12, 16, "name")]);
    let counts = score(&expected, &actual);
    assert_eq!(
        counts["name"],
        Counts {
            correct: 1,
            extra: 2,
            missed: 1
        }
    );
    assert_eq!(
        counts["secret"],
        Counts {
            correct: 0,
            extra: 0,
            missed: 1
        }
    );
    assert_eq!(ratio(0, 0), "n/a");
    assert_eq!(ratio(1, 2), "0.500");
}

#[test]
#[ignore = "reports detection quality, including known gaps; run with --ignored --nocapture"]
fn evaluate_accuracy() {
    let cases = corpus();
    let input: String = cases
        .iter()
        .map(|case| format!("{}\n", json!({"id": case.id, "text": case.text})))
        .collect();

    let mut child = Command::new(env!("CARGO_BIN_EXE_redakt"))
        .arg("--jsonl")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    let mut stdin = child.stdin.take().unwrap();
    let input_bytes = input.as_bytes();
    let output = thread::scope(|scope| {
        let writer = scope.spawn(move || stdin.write_all(input_bytes));
        let output = child.wait_with_output().unwrap();
        writer.join().unwrap().unwrap();
        output
    });
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let responses: Vec<Response> = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(responses.len(), cases.len());
    let mut totals = score(&BTreeSet::new(), &BTreeSet::new());
    println!("documents={}", cases.len());
    for (case, response) in cases.iter().zip(responses) {
        assert_eq!(case.id, response.id);
        let expected = case.spans();
        let span_count = response.spans.len();
        let actual: BTreeSet<_> = response.spans.into_iter().collect();
        assert_eq!(
            actual.len(),
            span_count,
            "duplicate prediction in {}",
            case.id
        );
        for span in &actual {
            assert!(LABELS.contains(&span.label.as_str()));
            assert!(span.start < span.end && span.end <= case.text.chars().count());
        }
        for span in expected.difference(&actual) {
            println!("{} MISS {span:?}", case.id);
        }
        for span in actual.difference(&expected) {
            println!("{} EXTRA {span:?}", case.id);
        }
        for (label, count) in score(&expected, &actual) {
            let total = totals.get_mut(&label).unwrap();
            total.correct += count.correct;
            total.extra += count.extra;
            total.missed += count.missed;
        }
    }
    let mut micro = Counts::default();
    for (label, count) in totals {
        print_counts(&label, &count);
        micro.correct += count.correct;
        micro.extra += count.extra;
        micro.missed += count.missed;
    }
    print_counts("micro", &micro);
}
