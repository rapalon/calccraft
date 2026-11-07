//! The unit tables against Microsoft's own: `ms_unit_data.txt` holds what
//! `UnitConverterDataLoader.cs` writes (see the file's header for the commit and how it was
//! extracted), so a typo in a factor, a unit missing from a list, a changed order or a wrong
//! default unit for a region shows up here. Nothing in it comes from running this crate.

use super::*;

const DATA: &str = include_str!("ms_unit_data.txt");

/// The data lines of one kind, split into their tab separated fields.
fn lines(kind: &str) -> impl Iterator<Item = Vec<&'static str>> + '_ {
    DATA.lines()
        .filter(|line| !line.starts_with('#'))
        .map(|line| line.split('\t').collect::<Vec<_>>())
        .filter(move |fields| fields[0] == kind)
}

fn category(name: &str) -> Category {
    Category::ALL
        .into_iter()
        .find(|category| category.key() == name)
        .unwrap_or_else(|| panic!("no category {name}"))
}

#[test]
fn every_unit_list_matches_the_data_loader() {
    // Source: UnitConverterDataLoader.cs, the AddUnit calls (the ids, resource keys and the
    // `order` argument that sorts the list) and `conversionFactors`.
    let mut checked = 0;
    for category in Category::ALL {
        let expected: Vec<_> = lines("unit")
            .filter(|fields| fields[1] == category.key())
            .collect();
        let units = category.all_units();
        let ids: Vec<_> = units.iter().map(|unit| unit.id.to_string()).collect();
        let expected_ids: Vec<_> = expected
            .iter()
            .map(|fields| fields[2].to_string())
            .collect();
        assert_eq!(ids, expected_ids, "{} units or their order", category.key());
        for (unit, fields) in units.iter().zip(&expected) {
            assert_eq!(unit.key, fields[3], "{} id {}", category.key(), fields[2]);
            if category != Category::Temperature {
                let factor: f64 = fields[5].parse().unwrap();
                assert_eq!(unit.factor, factor, "{} {}", category.key(), unit.key);
            }
            checked += 1;
        }
    }
    assert_eq!(checked, 132);
}

#[test]
fn every_comparison_unit_matches_the_data_loader() {
    // Source: UnitConverterDataLoader.cs, the AddUnit calls with `isWhimsical` set, and
    // their entries in `conversionFactors`.
    let mut checked = 0;
    for category in Category::ALL {
        let expected: Vec<_> = lines("whimsy")
            .filter(|fields| fields[1] == category.key())
            .collect();
        let comparisons = category.comparisons();
        assert_eq!(
            comparisons.len(),
            expected.len(),
            "{} comparison units",
            category.key()
        );
        for (comparison, fields) in comparisons.iter().zip(&expected) {
            assert_eq!(comparison.key, fields[3], "{}", category.key());
            let factor: f64 = fields[5].parse().unwrap();
            assert_eq!(
                comparison.factor,
                factor,
                "{} {}",
                category.key(),
                fields[3]
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 26);
}

#[test]
fn temperature_conversions_match_the_data_loader() {
    // Source: UnitConverterDataLoader.cs, `explicitConversions`.
    let names = ["DegreesCelsius", "DegreesFahrenheit", "Kelvin"];
    let mut checked = 0;
    for fields in lines("temp") {
        let row = names.iter().position(|name| *name == fields[1]).unwrap();
        let column = names.iter().position(|name| *name == fields[2]).unwrap();
        let ratio: f64 = fields[3].parse().unwrap();
        let offset: f64 = fields[4].parse().unwrap();
        let offset_first = fields[5] == "true";
        assert_eq!(
            TEMPERATURE[row][column],
            (ratio, offset, offset_first),
            "{} to {}",
            fields[1],
            fields[2]
        );
        checked += 1;
    }
    assert_eq!(checked, 9);
}

#[test]
fn default_units_follow_the_region_flags_of_load_data() {
    // Source: UnitConverterDataLoader.LoadData (`useUSCustomary`, `useFahrenheit`,
    // `useWattInsteadOfKilowatt`, `usePyeong` and the source/target arguments of AddUnit) and
    // UnitConverter::InitializeSelectedUnits, which takes the first flagged unit.
    let mut checked = 0;
    for fields in lines("default") {
        let mut converter = Converter::new(Region::new(fields[1]));
        converter.set_category(category(fields[2]));
        let units = converter.units();
        let (from, to) = (units[converter.unit1()].id, units[converter.unit2()].id);
        assert_eq!(
            (from.to_string(), to.to_string()),
            (fields[3].to_string(), fields[4].to_string()),
            "{} in {}",
            fields[2],
            fields[1]
        );
        checked += 1;
    }
    assert_eq!(checked, 19 * 12);
}
