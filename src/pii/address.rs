use crate::Span;
use std::collections::HashMap;

pub(super) struct AddressEnds(HashMap<usize, usize>);

impl AddressEnds {
    pub(super) fn from_rules(spans: &[Span]) -> Self {
        let mut ends = HashMap::<usize, usize>::new();
        for span in spans.iter().filter(|span| span.label == "address") {
            let end = ends.entry(span.start).or_default();
            *end = (*end).max(span.end);
        }
        Self(ends)
    }

    pub(super) fn clip(&self, text: &str, spans: &mut [Span]) {
        if self.0.is_empty() {
            return;
        }
        let chars: Vec<_> = text.chars().collect();
        for span in spans.iter_mut().filter(|span| span.label == "address") {
            let Some(&end) = self.0.get(&span.start) else {
                continue;
            };
            if end < span.end && crosses_terminator(&chars[end..span.end]) {
                span.end = end;
            }
        }
    }
}

fn crosses_terminator(suffix: &[char]) -> bool {
    let next = suffix.iter().find(|ch| !matches!(ch, ' ' | '\t'));
    next.is_some_and(|ch| ".,;!?:)\"\r\n。！？".contains(*ch))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn address(start: usize, end: usize) -> Span {
        Span {
            start,
            end,
            label: "address".to_owned(),
        }
    }

    #[test]
    fn address_delimiters() {
        let rule = address(2, 12);
        let boundaries = AddressEnds::from_rules(std::slice::from_ref(&rule));
        for suffix in [". Next", "。", ";", ")", "\nNext", "  \r\nNext"] {
            let text = format!("🦊 1 Oak Lane{suffix}");
            let mut spans = [address(2, text.chars().count())];
            boundaries.clip(&text, &mut spans);
            assert_eq!(spans[0], rule);
        }
    }

    #[test]
    fn address_extensions() {
        let text = "🦊 1 Oak Lane Boston 02108";
        let boundaries = AddressEnds::from_rules(&[address(2, 12)]);
        let complete = address(2, text.chars().count());
        let mut spans = [complete.clone()];
        boundaries.clip(text, &mut spans);
        assert_eq!(spans[0], complete);
    }
}
