//! Complete a structured value when a detector labeled only part of it.
use crate::Span;

fn telephone(label: &str) -> bool {
    matches!(label, "phone" | "fax_number")
}

fn identifier(label: &str) -> bool {
    matches!(
        label,
        "customer_id"
            | "employee_id"
            | "national_id"
            | "tax_id"
            | "medical_record_number"
            | "vehicle_identifier"
            | "biometric_identifier"
            | "device_identifier"
            | "certificate_license_number"
            | "health_plan_beneficiary_number"
            | "unique_id"
            | "swift_bic"
            | "license_plate"
    )
}

fn word(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

fn phone_cue(prefix: &[char]) -> Option<&'static str> {
    let text: String = prefix.iter().collect();
    let stripped = text.trim_end_matches(|ch: char| ch.is_whitespace() || ":*=#-".contains(ch));
    for token in stripped.rsplit(|ch: char| !ch.is_alphabetic()).take(5) {
        match token.to_ascii_lowercase().as_str() {
            "fax" => return Some("fax_number"),
            "phone" | "telephone" | "mobile" | "cell" | "call" | "contact" | "reach" => {
                return Some("phone")
            }
            "number" | "is" | "at" | "us" | "me" | "him" | "her" | "them" | "on" | "via" | "by"
            | "to" | "our" | "of" | "can" | "be" => {}
            _ => break,
        }
    }
    None
}

fn number_end(chars: &[char], start: usize) -> Option<usize> {
    let mut index = start;
    if chars[index] == '+' {
        index += 1;
        if chars.get(index) == Some(&' ') {
            index += 1;
        }
    }
    if chars.get(index) == Some(&'(') {
        index += 1;
    }
    if !chars.get(index)?.is_ascii_digit() {
        return None;
    }

    let mut end = None;
    while let Some(&ch) = chars.get(index) {
        if ch.is_ascii_digit() {
            if chars.get(index + 1).is_none_or(|&next| !word(next)) {
                end = Some(index + 1);
            }
        } else if !" ()-.".contains(ch) || (ch == '.' && chars.get(index + 1) == Some(&' ')) {
            break;
        }
        index += 1;
    }
    end
}

struct Repairs<'a> {
    original: &'a [Span],
    prefix_ends: Vec<usize>,
    removed: Vec<bool>,
    added: Vec<Span>,
}

impl Repairs<'_> {
    fn contained(&self, start: usize, end: usize) -> impl Iterator<Item = (usize, &Span)> {
        let first = self.original.partition_point(|s| s.start < start);
        self.original[first..]
            .iter()
            .enumerate()
            .take_while(move |(_, s)| s.start < end)
            .filter(move |(_, s)| s.end <= end)
            .map(move |(offset, s)| (first + offset, s))
    }

    fn enclosed(&self, start: usize, end: usize) -> bool {
        let before = self.original.partition_point(|s| s.start < start);
        let through = self.original.partition_point(|s| s.start <= start);
        self.prefix_ends[before] >= end || self.prefix_ends[through] > end
    }

    fn phone(&mut self, chars: &[char], start: usize, end: usize) {
        if self.enclosed(start, end) {
            return;
        }
        let parts: Vec<_> = self.contained(start, end).collect();
        let Some(first) = parts.iter().find(|(_, s)| telephone(&s.label)) else {
            return;
        };
        let cue = phone_cue(&chars[start.saturating_sub(42)..start]);
        let whole = parts.iter().any(|(_, s)| s.start == start && s.end == end);
        let mixed = parts.iter().any(|(_, s)| !telephone(&s.label));
        if cue.is_none() && (whole || (chars[start] != '+' && mixed)) {
            return;
        }
        let label = cue.unwrap_or(&first.1.label).to_owned();
        let obsolete: Vec<_> = parts
            .iter()
            .filter(|(_, s)| {
                telephone(&s.label) && (s.start != start || s.end != end || s.label != label)
            })
            .map(|(index, _)| *index)
            .collect();
        self.replace(start, end, label, obsolete);
    }

    fn id(&mut self, start: usize, end: usize) {
        let parts: Vec<_> = self
            .contained(start, end)
            .filter(|(_, s)| identifier(&s.label))
            .collect();
        let Some((_, first)) = parts.first() else {
            return;
        };
        if parts.iter().any(|(_, s)| s.start == start && s.end == end) {
            return;
        }
        let label = first.label.clone();
        let obsolete = parts.iter().map(|(index, _)| *index).collect();
        self.replace(start, end, label, obsolete);
    }

    fn replace(&mut self, start: usize, end: usize, label: String, obsolete: Vec<usize>) {
        for index in obsolete {
            self.removed[index] = true;
        }
        self.added.push(Span { start, end, label });
    }
}

fn scan_phones(chars: &[char], repairs: &mut Repairs<'_>) {
    let mut start = 0;
    while start < chars.len() {
        let left_ok = start == 0 || (!word(chars[start - 1]) && chars[start - 1] != '+');
        let Some(end) = left_ok.then(|| number_end(chars, start)).flatten() else {
            start += 1;
            continue;
        };
        let value = &chars[start..end];
        if value.len() <= 40
            && (7..=15).contains(&value.iter().filter(|c| c.is_ascii_digit()).count())
        {
            repairs.phone(chars, start, end);
        }
        start = end;
    }
}

fn scan_ids(chars: &[char], repairs: &mut Repairs<'_>) {
    let mut index = 0;
    while index < chars.len() {
        if !word(chars[index]) {
            index += 1;
            continue;
        }
        let start = index;
        while index < chars.len() && word(chars[index]) {
            index += 1;
        }
        let token = &chars[start..index];
        if (6..=40).contains(&token.len())
            && token.iter().all(char::is_ascii_alphanumeric)
            && token.iter().any(char::is_ascii_digit)
            && token.iter().any(char::is_ascii_alphabetic)
        {
            repairs.id(start, index);
        }
    }
}

pub(crate) fn repair(text: &str, spans: &mut Vec<Span>) {
    if spans.is_empty() {
        return;
    }
    spans.sort_unstable();
    let chars: Vec<_> = text.chars().collect();
    let mut prefix_ends = vec![0];
    for span in spans.iter() {
        prefix_ends.push(span.end.max(*prefix_ends.last().unwrap()));
    }
    let mut repairs = Repairs {
        original: spans,
        prefix_ends,
        removed: vec![false; spans.len()],
        added: Vec::new(),
    };
    scan_phones(&chars, &mut repairs);
    scan_ids(&chars, &mut repairs);
    let mut keep = repairs.removed.into_iter();
    let added = repairs.added;
    spans.retain(|_| !keep.next().unwrap());
    spans.extend(added);
    spans.sort_unstable();
    spans.dedup();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(start: usize, end: usize, label: &str) -> Span {
        Span {
            start,
            end,
            label: label.into(),
        }
    }

    #[test]
    fn international_fax() {
        let text = "🦊 Fax: +49 351 437 8296. Next call.";
        let mut spans = vec![span(7, 10, "fax_number"), span(11, 23, "phone")];
        repair(text, &mut spans);
        assert_eq!(spans, [span(7, 23, "fax_number")]);
    }

    #[test]
    fn identifier_boundaries() {
        let mut spans = vec![span(4, 5, "employee_id"), span(13, 19, "employee_id")];
        repair("ID: A123456, 123456.", &mut spans);
        assert_eq!(
            spans,
            [span(4, 11, "employee_id"), span(13, 19, "employee_id")]
        );
    }
}
