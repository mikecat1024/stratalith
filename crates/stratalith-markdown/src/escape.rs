mod data;

pub(super) fn punctuation(c: char) -> bool {
    c.is_ascii_punctuation()
        || data::PUNCTUATION
            .binary_search_by(|&(start, end)| {
                if end < c {
                    std::cmp::Ordering::Less
                } else if start > c {
                    std::cmp::Ordering::Greater
                } else {
                    std::cmp::Ordering::Equal
                }
            })
            .is_ok()
}

pub(super) fn entity(input: &str) -> Option<(String, usize)> {
    let rest = input.strip_prefix('&')?;
    let end = rest.bytes().take(33).position(|b| b == b';')?;
    let name = &rest[..end];
    let value = if let Some(number) = name.strip_prefix('#') {
        let (digits, radix, limit) = if let Some(hex) = number.strip_prefix(['x', 'X']) {
            (hex, 16, 6)
        } else {
            (number, 10, 7)
        };
        if digits.is_empty() || digits.len() > limit || !digits.chars().all(|c| c.is_digit(radix)) {
            return None;
        }
        let n = u32::from_str_radix(digits, radix).ok()?;
        char::from_u32(n)
            .filter(|&c| c != '\0')
            .unwrap_or('\u{fffd}')
            .into()
    } else {
        let i = data::ENTITIES
            .binary_search_by_key(&name, |&(key, _)| key)
            .ok()?;
        data::ENTITIES[i].1.into()
    };
    Some((value, end + 2))
}

pub(super) fn decode(mut s: &str) -> String {
    let mut out = String::new();
    while let Some(i) = s.find(['\\', '&']) {
        out.push_str(&s[..i]);
        s = &s[i..];
        if s.starts_with('\\') && s[1..].starts_with(|c: char| c.is_ascii_punctuation()) {
            out.push(s.as_bytes()[1] as char);
            s = &s[2..];
        } else if let Some((value, n)) = entity(s) {
            out.push_str(&value);
            s = &s[n..];
        } else {
            out.push(s.as_bytes()[0] as char);
            s = &s[1..];
        }
    }
    out.push_str(s);
    out
}

pub(super) fn fold(c: char, out: &mut String) {
    if let Ok(i) = data::FOLDS.binary_search_by_key(&c, |&(key, _)| key) {
        out.push_str(data::FOLDS[i].1);
    } else {
        out.extend(c.to_lowercase());
    }
}
