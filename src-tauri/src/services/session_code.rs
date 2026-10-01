//! Session code generation. 31-symbol alphabet without look-alikes (0/O, 1/I/L).

use crate::config::SESSION_CODE_LENGTH;

pub const ALPHABET: &[u8] = b"23456789ABCDEFGHJKMNPQRSTUVWXYZ";
/// Largest multiple of the alphabet size that fits in a byte; larger bytes are rejected to avoid modulo bias.
const ACCEPT_BELOW: u8 = (256 / ALPHABET.len() * ALPHABET.len()) as u8;

/// Random bytes from UUIDv4s, skipping bytes 6 and 8 which carry fixed version/variant bits.
fn random_bytes() -> impl Iterator<Item = u8> {
    std::iter::repeat_with(|| uuid::Uuid::new_v4().into_bytes())
        .flat_map(|b| b.into_iter().enumerate().filter(|(i, _)| *i != 6 && *i != 8).map(|(_, v)| v).collect::<Vec<_>>())
}

pub fn generate_with_len(len: usize) -> String {
    random_bytes()
        .filter(|b| *b < ACCEPT_BELOW)
        .map(|b| ALPHABET[(b as usize) % ALPHABET.len()] as char)
        .take(len)
        .collect()
}

pub fn generate() -> String {
    generate_with_len(SESSION_CODE_LENGTH)
}

/// Normalises user input (case, whitespace); returns None if it cannot be a valid code.
pub fn normalize(input: &str) -> Option<String> {
    let s: String = input.trim().to_ascii_uppercase();
    (s.len() == SESSION_CODE_LENGTH && s.bytes().all(|b| ALPHABET.contains(&b))).then_some(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn has_expected_length_and_alphabet() {
        for _ in 0..200 {
            let c = generate();
            assert_eq!(c.len(), SESSION_CODE_LENGTH);
            assert!(c.bytes().all(|b| ALPHABET.contains(&b)));
        }
    }

    #[test]
    fn avoids_ambiguous_characters() {
        for ch in ['0', 'O', '1', 'I', 'L'] {
            assert!(!ALPHABET.contains(&(ch as u8)));
        }
    }

    #[test]
    fn codes_vary() {
        let set: std::collections::HashSet<_> = (0..200).map(|_| generate()).collect();
        assert!(set.len() > 190);
    }

    #[test]
    fn normalize_accepts_lowercase_and_rejects_garbage() {
        assert_eq!(normalize(" 8f4k2 ").as_deref(), Some("8F4K2"));
        assert_eq!(normalize("8F4K"), None);
        assert_eq!(normalize("8F4K0"), None); // 0 is not in the alphabet
        assert_eq!(normalize("'; DROP"), None);
    }
}
