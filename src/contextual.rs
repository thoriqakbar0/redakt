use crate::Span;

#[derive(Clone, Copy)]
enum Grammar {
    Name,
    Address,
    Secret,
    Account,
    Date,
}

const CUES: &[(&str, Grammar)] = &[
    ("full name", Grammar::Name),
    ("my name", Grammar::Name),
    ("name", Grammar::Name),
    ("patient", Grammar::Name),
    ("customer", Grammar::Name),
    ("address", Grammar::Address),
    ("ship to", Grammar::Address),
    ("lives at", Grammar::Address),
    ("password", Grammar::Secret),
    ("passwd", Grammar::Secret),
    ("api_key", Grammar::Secret),
    ("api key", Grammar::Secret),
    ("access_token", Grammar::Secret),
    ("access token", Grammar::Secret),
    ("client_secret", Grammar::Secret),
    ("client secret", Grammar::Secret),
    ("bank account", Grammar::Account),
    ("account number", Grammar::Account),
    ("date of birth", Grammar::Date),
    ("birth date", Grammar::Date),
    ("dob", Grammar::Date),
];
const MAX_VALUE: usize = 256;

fn horizontal(character: char) -> bool {
    matches!(character, ' ' | '\t')
}

fn word(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}

fn matches_ascii(text: &[char], offset: usize, expected: &str) -> bool {
    let Some(candidate) = text.get(offset..offset + expected.len()) else {
        return false;
    };
    candidate
        .iter()
        .zip(expected.chars())
        .all(|(actual, expected)| actual.eq_ignore_ascii_case(&expected))
}

fn skip_spaces(text: &[char], mut offset: usize) -> usize {
    while text
        .get(offset)
        .is_some_and(|&character| horizontal(character))
    {
        offset += 1;
    }
    offset
}

fn value_start(text: &[char], offset: usize, cue: &str) -> Option<usize> {
    let mut position = offset + cue.len();
    if text.get(position).is_some_and(|&character| word(character)) {
        return None;
    }
    if offset > 0 && matches!(text[offset - 1], '\'' | '"') {
        if text.get(position) != text.get(offset - 1) {
            return None;
        }
        position += 1;
    }
    let spaced = skip_spaces(text, position);
    position = match text.get(spaced) {
        Some(':' | '=') => spaced + 1,
        _ if spaced > position
            && matches_ascii(text, spaced, "is")
            && text
                .get(spaced + 2)
                .is_some_and(|&character| horizontal(character)) =>
        {
            spaced + 2
        }
        _ if spaced > position && matches!(cue, "ship to" | "lives at") => spaced,
        _ => return None,
    };
    Some(skip_spaces(text, position))
}

fn value_end(text: &[char], start: usize, grammar: Grammar) -> Option<(usize, usize)> {
    let quote = text
        .get(start)
        .copied()
        .filter(|character| matches!(character, '\'' | '"'));
    let first = start + usize::from(quote.is_some());
    let mut end = first;
    while let Some(&character) = text.get(end) {
        if end - first > MAX_VALUE {
            return None;
        }
        if matches!(character, '\r' | '\n') {
            if quote.is_some() {
                return None;
            }
            break;
        }
        if quote == Some(character) {
            if text
                .get(end + 1)
                .is_some_and(|&next| word(next) || next == '\\')
            {
                return None;
            }
            return Some((first, end));
        }
        if quote.is_none() && matches!(grammar, Grammar::Secret) && matches!(character, '\'' | '"')
        {
            return None;
        }
        if quote.is_none() {
            let separator = matches!(character, ',' | ';' | '}' | ']' | '"');
            let period = character == '.' && !matches!(grammar, Grammar::Secret);
            let token_end =
                horizontal(character) && matches!(grammar, Grammar::Secret | Grammar::Date);
            let next_field = horizontal(character)
                && CUES.iter().any(|(cue, _)| {
                    let next = skip_spaces(text, end);
                    matches_ascii(text, next, cue) && value_start(text, next, cue).is_some()
                });
            if separator || period || token_end || next_field {
                break;
            }
        }
        end += 1;
    }
    if quote.is_some() || end - first > MAX_VALUE {
        return None;
    }
    while end > first && horizontal(text[end - 1]) {
        end -= 1;
    }
    Some((first, end))
}

fn valid_name(value: &[char]) -> bool {
    let mut words = 0;
    let mut letters = 0;
    let mut previous_letter = false;
    for (index, &character) in value.iter().enumerate() {
        if character.is_alphabetic() {
            letters += 1;
            previous_letter = true;
        } else if matches!(character, '\'' | '’' | '-') {
            let next_letter = value
                .get(index + 1)
                .is_some_and(|next| next.is_alphabetic());
            if !previous_letter || !next_letter {
                return false;
            }
            previous_letter = false;
        } else if horizontal(character) {
            if previous_letter {
                words += 1;
            }
            previous_letter = false;
        } else {
            return false;
        }
    }
    words += usize::from(previous_letter);
    if !(1..=4).contains(&words) || letters == 0 || value.len() > 100 {
        return false;
    }
    let name: String = value.iter().collect();
    !name.split_whitespace().any(|part| {
        [
            "this", "is", "the", "and", "works", "was", "has", "said", "lives",
        ]
        .iter()
        .any(|stop| part.eq_ignore_ascii_case(stop))
    })
}

fn valid_date(value: &[char]) -> bool {
    if value.len() != 10 || value[4] != '-' || value[7] != '-' {
        return false;
    }
    if !value
        .iter()
        .enumerate()
        .all(|(index, character)| [4, 7].contains(&index) || character.is_ascii_digit())
    {
        return false;
    }
    let number = |range: std::ops::Range<usize>| {
        value[range].iter().fold(0_u32, |total, character| {
            total * 10 + (*character as u32 - '0' as u32)
        })
    };
    let year = number(0..4);
    let month = number(5..7);
    let day = number(8..10);
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = match month {
        2 => 28 + u32::from(leap),
        4 | 6 | 9 | 11 => 30,
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        _ => return false,
    };
    year != 0 && (1..=days).contains(&day)
}

fn valid_address(value: &[char]) -> bool {
    let address: String = value.iter().collect();
    let parts: Vec<_> = address.split_whitespace().collect();
    (3..=8).contains(&parts.len())
        && parts[0].chars().all(|character| character.is_ascii_digit())
        && parts[1..parts.len() - 1].iter().all(|part| {
            part.chars()
                .all(|character| character.is_alphabetic() || matches!(character, '-' | '\''))
        })
        && [
            "street",
            "st",
            "road",
            "rd",
            "avenue",
            "ave",
            "lane",
            "ln",
            "drive",
            "dr",
            "boulevard",
            "blvd",
        ]
        .iter()
        .any(|suffix| parts.last().unwrap().eq_ignore_ascii_case(suffix))
}

fn valid_value(grammar: Grammar, value: &[char]) -> bool {
    if value.is_empty() {
        return false;
    }
    match grammar {
        Grammar::Name => valid_name(value),
        Grammar::Secret => {
            value
                .iter()
                .all(|character| !character.is_control() && *character != '\\')
                && value.iter().any(|character| !character.is_whitespace())
        }
        Grammar::Account => {
            let digits = value
                .iter()
                .filter(|character| character.is_ascii_digit())
                .count();
            (6..=34).contains(&digits)
                && value.first().is_some_and(char::is_ascii_digit)
                && value.last().is_some_and(char::is_ascii_digit)
                && value.iter().all(|&character| {
                    character.is_ascii_digit() || horizontal(character) || character == '-'
                })
        }
        Grammar::Date => valid_date(value),
        Grammar::Address => valid_address(value),
    }
}

pub(crate) fn scan(text: &str) -> Vec<Span> {
    let characters: Vec<_> = text.chars().collect();
    let mut spans = Vec::new();
    for offset in 0..characters.len() {
        if offset > 0 && word(characters[offset - 1]) {
            continue;
        }
        for &(cue, grammar) in CUES {
            if !matches_ascii(&characters, offset, cue) {
                continue;
            }
            let Some(start) = value_start(&characters, offset, cue) else {
                continue;
            };
            let Some((start, end)) = value_end(&characters, start, grammar) else {
                continue;
            };
            if !valid_value(grammar, &characters[start..end]) {
                continue;
            }
            let label = match grammar {
                Grammar::Name => "name",
                Grammar::Address => "address",
                Grammar::Secret => "secret",
                Grammar::Account => "account_number",
                Grammar::Date => "date",
            };
            spans.push(Span {
                start,
                end,
                label: label.into(),
            });
            break;
        }
    }
    spans
}
