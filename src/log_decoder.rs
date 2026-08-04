/// Build byte substitution table (LUT) for Kuro Games Client.log
pub(crate) fn build_lut() -> [Option<char>; 256] {
    let mut lut = [None; 256];

    // Punctuation & Symbols
    lut[0xb4] = Some('[');
    lut[0xb2] = Some(']');
    lut[0x8b] = Some('.');
    lut[0x9f] = Some(':');
    lut[0xc2] = Some('-');
    lut[0xaf] = Some('\n');
    lut[0xe2] = Some('\r');
    lut[0x8d] = Some('(');
    lut[0xc6] = Some(')');
    lut[0xd4] = Some(',');
    lut[0x9e] = Some('q');
    lut[0xc0] = Some('\\');
    lut[0x2f] = Some('/');
    lut[0x3a] = Some(':');
    lut[0xfd] = Some(' ');
    lut[0x85] = Some(' ');
    lut[0xa0] = Some('O');

    // Digits 0 - 9
    lut[0x95] = Some('0');
    lut[0xde] = Some('1');
    lut[0x97] = Some('2');
    lut[0xdc] = Some('3');
    lut[0x91] = Some('4');
    lut[0xda] = Some('5');
    lut[0x93] = Some('6');
    lut[0xd8] = Some('7');
    lut[0x9d] = Some('8');
    lut[0xd6] = Some('9');

    // Uppercase Letters
    lut[0xae] = Some('A');
    lut[0xac] = Some('C');
    lut[0xb8] = Some('W');
    lut[0xe9] = Some('L');
    lut[0xa4] = Some('K');
    lut[0x9c] = Some('S');
    lut[0xf7] = Some('R');
    lut[0xf1] = Some('T');
    lut[0xa2] = Some('M');
    lut[0xaa] = Some('E');
    lut[0xe3] = Some('F');
    lut[0xe1] = Some('V');
    lut[0xf5] = Some('P');
    lut[0xed] = Some('H');
    lut[0xeb] = Some('N');
    lut[0xa8] = Some('G');
    lut[0xbe] = Some('Q');
    lut[0xe7] = Some('B');
    lut[0xd2] = Some('|');

    // Lowercase Letters
    lut[0x8e] = Some('a');
    lut[0x8c] = Some('c');
    lut[0xc1] = Some('d');
    lut[0x8a] = Some('e');
    lut[0xc3] = Some('f');
    lut[0x88] = Some('g');
    lut[0xcd] = Some('h');
    lut[0x86] = Some('i');
    lut[0xc9] = Some('l');
    lut[0x82] = Some('m');
    lut[0xcb] = Some('n');
    lut[0x80] = Some('o');
    lut[0xd5] = Some('p');
    lut[0xd7] = Some('r');
    lut[0xbc] = Some('s');
    lut[0xd1] = Some('t');
    lut[0x9a] = Some('u');
    lut[0xd3] = Some('v');
    lut[0x98] = Some('w');
    lut[0xdd] = Some('x');
    lut[0x96] = Some('y');
    lut[0xdf] = Some('z');

    lut
}

/// Decode raw bytes using substitution LUT.
pub(crate) fn decode_bytes(bytes: &[u8], lut: &[Option<char>; 256]) -> String {
    let mut decoded = String::with_capacity(bytes.len());
    for &b in bytes {
        if let Some(c) = lut[b as usize] {
            decoded.push(c);
        } else {
            decoded.push(b as char);
        }
    }
    decoded
}

/// Only explicit launcher intent indicates a hotfix restart.
/// Generic exit and patch-module messages also occur during normal shutdown.
const RESTART_MARKERS: [&str; 1] = ["hotfixrestarttocompletehotfix"];

fn retain_restart_marker_suffix(text: &mut String) {
    let keep_chars = RESTART_MARKERS
        .iter()
        .map(|marker| marker.chars().count())
        .max()
        .unwrap_or(1)
        .saturating_sub(1);
    let char_count = text.chars().count();
    if char_count > keep_chars {
        let start = text
            .char_indices()
            .nth(char_count - keep_chars)
            .map(|(index, _)| index)
            .unwrap_or(text.len());
        text.drain(..start);
    }
}

pub(crate) fn update_restart_marker_tail(tail: &mut String, decoded_text: &str) -> bool {
    tail.push_str(decoded_text);
    let lower_tail = tail.to_lowercase();
    if RESTART_MARKERS.iter().any(|marker| lower_tail.contains(marker)) {
        tail.clear();
        true
    } else {
        retain_restart_marker_suffix(tail);
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hotfix_restart_marker_detected_across_log_reads() {
        let mut tail = String::new();

        assert!(!update_restart_marker_tail(
            &mut tail,
            "HotFixRestartToComplete"
        ));
        assert_eq!(tail, "HotFixRestartToComplete");
        assert!(update_restart_marker_tail(&mut tail, "HotFix"));
        assert!(tail.is_empty());
    }

    #[test]
    fn test_normal_exit_markers_do_not_request_restart() {
        let mut tail = String::new();

        assert!(!update_restart_marker_tail(
            &mut tail,
            "Engine exit requested (reason: EngineExit())"
        ));
        assert!(!update_restart_marker_tail(
            &mut tail,
            "KuroHotPatch module shutdown"
        ));
        assert!(!update_restart_marker_tail(&mut tail, "RequestExitWithStatus"));
        assert!(!update_restart_marker_tail(&mut tail, "NeedRestart"));
    }

    #[test]
    fn test_restart_marker_case_insensitive() {
        let mut tail = String::new();
        assert!(update_restart_marker_tail(
            &mut tail,
            "hotfixrestarttocompletehotfix"
        ));
        assert!(tail.is_empty());
    }

    #[test]
    fn test_hotfix_marker_decodes_uppercase_f_bytes() {
        let lut = build_lut();
        let bytes = [
            0xed, 0x80, 0xd1, 0xe3, 0x86, 0xdd, 0xf7, 0x8a, 0x9c, 0xd1, 0x8e, 0xd7, 0xd1,
            0xf1, 0x80, 0xac, 0x80, 0x82, 0xd5, 0xc9, 0x8a, 0xd1, 0x8a, 0xed, 0x80, 0xd1,
            0xe3, 0x86, 0xdd,
        ];

        let decoded = decode_bytes(&bytes, &lut);
        assert_eq!(decoded, "HotFixReStartToCompleteHotFix");

        let mut tail = String::new();
        assert!(update_restart_marker_tail(&mut tail, &decoded));
    }

    #[test]
    fn test_restart_marker_tail_is_bounded_without_marker() {
        let mut tail = String::new();
        let long_text = "x".repeat(256);

        assert!(!update_restart_marker_tail(&mut tail, &long_text));
        let max_keep = RESTART_MARKERS
            .iter()
            .map(|m| m.chars().count())
            .max()
            .unwrap()
            - 1;
        assert_eq!(tail.chars().count(), max_keep);
    }
}
