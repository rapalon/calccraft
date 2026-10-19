//! The date functions against what Microsoft's `DateCalculationEngine` computes, see the
//! header of `ms_date_cases.txt` for where the expected values come from.

use super::*;

const CASES: &str = include_str!("ms_date_cases.txt");

fn date(fields: &[&str]) -> NaiveDate {
    let [y, m, d] = fields[..3] else {
        panic!("bad date {fields:?}")
    };
    NaiveDate::from_ymd_opt(y.parse().unwrap(), m.parse().unwrap(), d.parse().unwrap()).unwrap()
}

fn shown(date: Option<NaiveDate>) -> String {
    match date {
        Some(d) => format!("{} {} {}", d.year(), d.month(), d.day()),
        None => "none".to_string(),
    }
}

#[test]
fn dates_match_microsofts_date_calculation_engine() {
    let mut checked = 0;
    let mut failures = Vec::new();
    for line in CASES
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
    {
        let (query, expected) = line.split_once('\t').unwrap();
        let w: Vec<&str> = query.split(' ').collect();
        let got = match w[0] {
            "D" => {
                let d = difference(date(&w[1..4]), date(&w[4..7]));
                format!("{} {} {} {}", d.years, d.months, d.weeks, d.days)
            }
            kind => {
                let start = date(&w[1..4]);
                let n: Vec<u32> = w[4..7].iter().map(|x| x.parse().unwrap()).collect();
                shown(if kind == "A" {
                    add(start, n[0], n[1], n[2])
                } else {
                    subtract(start, n[0], n[1], n[2])
                })
            }
        };
        checked += 1;
        if got != expected {
            failures.push(format!("{query}: {got}, Microsoft {expected}"));
        }
    }
    assert!(checked >= 2000, "only {checked} cases");
    assert!(
        failures.is_empty(),
        "{} of {checked}:\n{}",
        failures.len(),
        failures[..failures.len().min(20)].join("\n")
    );
}
