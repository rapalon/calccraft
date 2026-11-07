//! Windows' English texts for the engines' resource keys (the analysis' parity and
//! monotonicity, syntax errors, …), from the app's `locales/en-US.txt`.

use std::collections::HashMap;
use std::sync::OnceLock;

/// The English text of a Windows resource key, or the key itself if there is none.
pub fn text(key: &str) -> &str {
    static STRINGS: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    let strings = STRINGS.get_or_init(|| {
        include_str!("../../../locales/en-US.txt")
            .lines()
            .filter(|line| !line.starts_with('#'))
            .filter_map(|line| line.split_once('='))
            .collect()
    });
    strings.get(key).copied().unwrap_or(key)
}

#[cfg(test)]
mod tests {
    #[test]
    fn looks_up_windows_texts() {
        assert_eq!(super::text("KGFParityOdd"), "The function is odd.");
        assert_eq!(super::text("no-such-key"), "no-such-key");
    }
}
