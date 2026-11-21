use std::collections::HashMap;

pub(crate) const WIDTH: usize = 1 << 18;

#[derive(Default)]
pub(crate) struct Token {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) word: u32,
    shape: u32,
    pattern: u32,
    prefix: u32,
    suffix: u32,
    chunk: u32,
    field: u32,
}

fn fold(left: u32, right: u32) -> u32 {
    (left ^ right).wrapping_mul(16777619)
}

fn kind(ch: char) -> u32 {
    if ch.is_uppercase() {
        1
    } else if ch.is_alphabetic() {
        2
    } else if ch.is_numeric() {
        3
    } else {
        ch as u32 + 4
    }
}

fn make_token(chars: &[char], start: usize, end: usize, chunk: u32, field: u32) -> Token {
    let span = &chars[start..end];
    let mut word = 2166136261;
    let mut prefix = word;
    let mut suffix = word;
    let mut shape = 0;
    let mut previous_kind = 0;
    for (offset, &ch) in span.iter().enumerate() {
        let lower = ch.to_lowercase().next().unwrap_or(ch) as u32;
        word = fold(word, lower);
        if offset < 3 {
            prefix = fold(prefix, lower);
        }
        if offset + 3 >= span.len() {
            suffix = fold(suffix, lower);
        }
        let current = kind(ch);
        if previous_kind != current {
            shape = fold(shape, current);
        }
        previous_kind = current;
    }
    Token {
        start,
        end,
        word,
        shape,
        prefix,
        suffix,
        chunk,
        field,
        pattern: fold(shape, span.len().min(32) as u32),
    }
}

fn chunk_shape(chars: &[char]) -> u32 {
    let mut shape = 0;
    let mut previous = 0;
    let mut email = false;
    for &ch in chars {
        let group = if ch.is_alphabetic() { 1 } else { kind(ch) };
        if group != previous {
            shape = fold(shape, group);
        }
        previous = group;
        email |= ch == '@';
    }
    if email {
        u32::MAX
    } else {
        shape
    }
}

fn token_end(chars: &[char], start: usize, end: usize) -> usize {
    let first = chars[start];
    let mut next = start + 1;
    if first.is_alphabetic() {
        while next < end && chars[next].is_alphabetic() {
            next += 1;
        }
    } else if first.is_numeric() {
        while next < end && chars[next].is_numeric() {
            next += 1;
        }
    }
    next
}

pub(crate) fn tokenize(text: &str) -> Vec<Token> {
    let chars: Vec<_> = text.chars().collect();
    let mut tokens: Vec<Token> = Vec::new();
    let mut cursor = 0;
    let mut field = 0;
    while cursor < chars.len() {
        if chars[cursor].is_whitespace() {
            if matches!(chars[cursor], '\n' | '\r') {
                field = 0;
            }
            cursor += 1;
            continue;
        }
        let end = chars[cursor..]
            .iter()
            .position(|c| c.is_whitespace())
            .map_or(chars.len(), |offset| cursor + offset);
        let chunk = chunk_shape(&chars[cursor..end]);
        while cursor < end {
            let start = cursor;
            let ch = chars[start];
            cursor = token_end(&chars, start, end);
            if matches!(ch, ':' | '=') {
                field = tokens.last().map_or(0, |token| token.word);
            }
            tokens.push(make_token(&chars, start, cursor, chunk, field));
        }
    }
    tokens
}

pub(crate) const FEATURE_COUNT: usize = 49;

pub(crate) fn features(
    tokens: &[Token],
    index: usize,
    prior: u8,
    vocab: &HashMap<u32, u8>,
) -> [usize; FEATURE_COUNT] {
    let pad = Token::default();
    let at = |delta: isize| {
        index
            .checked_add_signed(delta)
            .and_then(|i| tokens.get(i))
            .unwrap_or(&pad)
    };
    let word_class = |t: &Token| u32::from(*vocab.get(&t.word).unwrap_or(&0));

    let token = &tokens[index];
    let left = at(-1);
    let right = at(1);
    let left2 = at(-2);
    let right2 = at(2);

    let lex = word_class(token);
    let previous = u32::from(prior);
    let joined_left = u32::from(index > 0 && left.end == token.start);
    let joined_right = u32::from(token.end == right.start);
    let case = token.shape;

    let values = [
        // Token spelling and character classes.
        1,
        token.word,
        case,
        token.pattern,
        token.chunk,
        token.prefix,
        token.suffix,
        // Local word and shape context.
        left.word,
        right.word,
        left2.word,
        right2.word,
        at(-3).word,
        at(3).word,
        fold(left2.word, left.word),
        fold(right.word, right2.word),
        fold(left.word, case),
        fold(right.word, case),
        fold(left2.word, case),
        fold(right2.word, case),
        fold(left.word, token.word),
        fold(token.word, right.word),
        fold(left.shape, case),
        fold(case, right.shape),
        fold(left.pattern, token.pattern),
        fold(token.pattern, right.pattern),
        // Field cues and classes learned from annotated words.
        token.field,
        fold(token.field, case),
        lex,
        fold(lex, case),
        fold(word_class(left), lex),
        fold(lex, word_class(right)),
        fold(word_class(left), case),
        fold(word_class(right), case),
        // Sequence history and token boundaries.
        previous,
        fold(previous, case),
        fold(previous, token.word),
        fold(previous, lex),
        fold(previous, token.pattern),
        fold(previous, token.chunk),
        fold(previous, joined_left),
        fold(joined_left, case),
        fold(joined_right, case),
        // Shapes of whitespace-delimited strings preserve formats such as email.
        fold(left.chunk, token.chunk),
        fold(token.chunk, right.chunk),
        fold(at(-3).word, left2.word),
        fold(token.field, token.chunk),
        fold(previous, right.word),
        fold(previous, fold(joined_left, case)),
        fold(previous, token.suffix),
    ];
    std::array::from_fn(|i| {
        let bits = fold(i as u32 + 1, values[i]).wrapping_mul(0x85ebca6b);
        ((bits ^ (bits >> 16)) as usize) % WIDTH
    })
}
