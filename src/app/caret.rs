//! Where the engine's caret (`engine::Caret`, a position in the typed characters) is in the
//! text the display shows, and the other way around for a tap. The text has characters the
//! number doesn't: the region's group separators (`1,234`, `12,34,567`, `1.234`), the
//! Programmer mode's spaces, BIN's leading zeros, a minus sign and `e+`. Pure, so it's
//! tested on every platform.

use std::ops::Range;

use crate::engine::{Caret, ExpressionPart};

/// The expression line (en-US, from `Calculator::expression_parts`) with the region's decimal
/// separator, as `localize_expression` writes it, and where its numbers are in it, in
/// characters (a tap's galley position is a character index).
pub(super) fn localize_line(
    line: &str,
    parts: &[ExpressionPart],
    decimal: &str,
) -> (String, Vec<(Range<usize>, usize)>) {
    let localize = |piece: &str| piece.replace('.', decimal);
    let mut text = String::new();
    let mut ranges = Vec::new();
    let mut at = 0;
    for part in parts {
        if part.range.start < at || part.range.end > line.len() {
            continue;
        }
        text.push_str(&localize(&line[at..part.range.start]));
        let start = text.chars().count();
        text.push_str(&localize(&line[part.range.clone()]));
        ranges.push((start..text.chars().count(), part.item));
        at = part.range.end;
    }
    text.push_str(&localize(&line[at..]));
    (text, ranges)
}

/// The caret stops of a number shown as `shown`: the character index in `shown` before each
/// character of `mantissa` (the typed digits with the decimal point) and after the last one,
/// then the same for the exponent's digits.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Stops {
    pub mantissa: Vec<usize>,
    pub exponent: Option<Vec<usize>>,
}

impl Stops {
    /// Finds the typed characters in `shown`, whose decimal separator is `decimal`.
    pub fn new(shown: &str, decimal: &str, mantissa: &str, exponent: Option<&str>) -> Stops {
        let chars: Vec<char> = shown.chars().collect();
        let decimal: Vec<char> = decimal.chars().collect();
        let e = chars.iter().position(|&c| c == 'e').unwrap_or(chars.len());
        // A caret after a character is drawn right after it, before a group separator that
        // follows; the first one before the first character.
        let mut stops = Vec::new();
        let mut at = 0;
        for c in mantissa.chars() {
            let found = (at..e).find(|&i| {
                if c == '.' {
                    chars[i..].starts_with(&decimal)
                } else {
                    chars[i].eq_ignore_ascii_case(&c)
                }
            });
            let Some(i) = found else { break };
            if stops.is_empty() {
                stops.push(i);
            }
            at = i + if c == '.' { decimal.len().max(1) } else { 1 };
            stops.push(at);
        }
        // Missing characters (which shouldn't happen) and an empty number (the display's
        // 0) put their stops at the end of the number.
        let count = mantissa.chars().count() + 1;
        if stops.len() < count {
            stops.clear();
            stops.resize(count, e);
        }
        let exponent = exponent.map(|digits| {
            // After `e` and its sign.
            let mut at = (e + 2).min(chars.len());
            let mut stops = vec![at];
            for c in digits.chars() {
                let i = (at..chars.len())
                    .find(|&i| chars[i] == c)
                    .unwrap_or(chars.len());
                at = (i + 1).min(chars.len());
                stops.push(at);
            }
            stops
        });
        Stops {
            mantissa: stops,
            exponent,
        }
    }

    /// The character index of a caret.
    pub fn index(&self, caret: Caret) -> usize {
        let stops = match (&self.exponent, caret.exponent) {
            (Some(stops), true) => stops,
            _ => &self.mantissa,
        };
        stops
            .get(caret.index)
            .or(stops.last())
            .copied()
            .unwrap_or_default()
    }

    /// The caret nearest to the character index `index` (a tap between two characters).
    pub fn nearest(&self, index: usize) -> Caret {
        let mut best = Caret::default();
        let mut distance = usize::MAX;
        let segments = [
            (false, Some(&self.mantissa)),
            (true, self.exponent.as_ref()),
        ];
        for (exponent, stops) in segments {
            for (i, &stop) in stops.into_iter().flatten().enumerate() {
                let d = stop.abs_diff(index);
                if d < distance {
                    distance = d;
                    best = Caret { exponent, index: i };
                }
            }
        }
        best
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(index: usize) -> Caret {
        Caret {
            exponent: false,
            index,
        }
    }

    #[test]
    fn group_separators_are_skipped() {
        let stops = Stops::new("1,234.5", ".", "1234.5", None);
        assert_eq!(stops.mantissa, [0, 1, 3, 4, 5, 6, 7]);
        // India's grouping and a German decimal comma with points between the groups.
        let stops = Stops::new("12,34,567", ".", "1234567", None);
        assert_eq!(stops.mantissa, [0, 1, 2, 4, 5, 7, 8, 9]);
        let stops = Stops::new("1.234,5", ",", "1234.5", None);
        assert_eq!(stops.mantissa, [0, 1, 3, 4, 5, 6, 7]);
        // A minus sign comes before the first stop.
        let stops = Stops::new("-12", ".", "12", None);
        assert_eq!(stops.mantissa, [1, 2, 3]);
    }

    #[test]
    fn programmer_groups_and_padding() {
        let stops = Stops::new("0001 0010", ".", "10010", None);
        assert_eq!(stops.mantissa, [3, 4, 6, 7, 8, 9]);
        let stops = Stops::new("F FA00", ".", "FFA00", None);
        assert_eq!(stops.mantissa, [0, 1, 3, 4, 5, 6]);
        // 0 has one stop, after it.
        let stops = Stops::new("0", ".", "", None);
        assert_eq!(stops.mantissa, [1]);
    }

    #[test]
    fn the_exponent() {
        let stops = Stops::new("1.5e-30", ".", "1.5", Some("30"));
        assert_eq!(stops.mantissa, [0, 1, 2, 3]);
        assert_eq!(stops.exponent, Some(vec![5, 6, 7]));
        // `exp` without digits shows e+0: the stop is before the 0.
        let stops = Stops::new("5.e+0", ".", "5.", Some(""));
        assert_eq!(stops.exponent, Some(vec![4]));
        let caret = Caret {
            exponent: true,
            index: 1,
        };
        let stops = Stops::new("1.5e-30", ".", "1.5", Some("30"));
        assert_eq!(stops.index(caret), 6);
        assert_eq!(stops.nearest(6), caret);
    }

    #[test]
    fn the_line_in_the_regions_format() {
        let parts = [
            ExpressionPart {
                range: 0..3,
                item: 0,
            },
            ExpressionPart {
                range: 10..14,
                item: 4,
            },
        ];
        let (text, ranges) = localize_line("1.5 + sqr(2.25) =", &parts, ",");
        assert_eq!(text, "1,5 + sqr(2,25) =");
        assert_eq!(ranges, [(0..3, 0), (10..14, 4)]);
        // A separator of another length, and characters of more than one byte before.
        let (text, ranges) = localize_line(
            "sin₀(0.5) × 2",
            &[
                ExpressionPart {
                    range: 7..10,
                    item: 1,
                },
                ExpressionPart {
                    range: 15..16,
                    item: 3,
                },
            ],
            "٫٫",
        );
        assert_eq!(text, "sin₀(0٫٫5) × 2");
        assert_eq!(ranges, [(5..9, 1), (13..14, 3)]);
    }

    #[test]
    fn a_tap_snaps_to_the_nearest_stop() {
        let stops = Stops::new("1,234", ".", "1234", None);
        // On the separator: before the 2.
        assert_eq!(stops.nearest(1), at(1));
        assert_eq!(stops.nearest(2), at(1));
        assert_eq!(stops.nearest(0), at(0));
        assert_eq!(stops.nearest(99), at(4));
        assert_eq!(stops.index(at(4)), 5);
    }
}
