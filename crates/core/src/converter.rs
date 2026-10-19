//! Unit converter logic, independent of the user interface: a port of the Windows
//! Calculator's `UnitConverter` (`CalcManager/UnitConverter.cpp`), the units and
//! factors of `UnitConverterDataLoader.cs`, and the parts of
//! `UnitConverterViewModel.cs` that decide what the two fields show.
//!
//! Windows converts with `double` and rounds the result with its own rules: at
//! most 15 digits, at least 7 significant digits where possible, and scientific
//! notation for very large and very small results. This module does the same with
//! `f64`, not with the exact `BigRational` of the calculator engine, so the digits
//! match Windows.
//!
//! Currency, the first converter in Windows, needs online exchange rates and isn't
//! here yet.

use crate::engine::{
    BASIC_PASTE_CHARACTERS, clean_paste, extract_operands, group_thousands, operand_matches,
};

#[cfg(test)]
mod ms_tables;

/// Longest number that can be typed, the minus sign included (`MAXIMUMDIGITSALLOWED`).
const MAX_DIGITS: usize = 15;
/// Results get at least this many significant digits (`OPTIMALDIGITSALLOWED`).
const OPTIMAL_DIGITS: usize = 7;
/// Results smaller than this get 15 decimals (`OPTIMALDECIMALALLOWED`).
const OPTIMAL_DECIMAL: f64 = 1e-6;
/// Results smaller than this use scientific notation (`MINIMUMDECIMALALLOWED`).
const MINIMUM_DECIMAL: f64 = 1e-14;
/// Longest number that can be pasted (`MaxConverterInputLength`).
const MAX_PASTE_LENGTH: usize = 16;

/// The converters, in the order of the Windows navigation menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Category {
    Volume,
    Length,
    Weight,
    Temperature,
    Energy,
    Area,
    Speed,
    Time,
    Power,
    Data,
    Pressure,
    Angle,
}

impl Category {
    pub const ALL: [Category; 12] = [
        Category::Volume,
        Category::Length,
        Category::Weight,
        Category::Temperature,
        Category::Energy,
        Category::Area,
        Category::Speed,
        Category::Time,
        Category::Power,
        Category::Data,
        Category::Pressure,
        Category::Angle,
    ];

    /// Whether values can be negative. The other categories hide `±`.
    pub fn supports_negative(self) -> bool {
        matches!(
            self,
            Category::Temperature | Category::Power | Category::Angle
        )
    }

    fn index(self) -> usize {
        Category::ALL.iter().position(|&c| c == self).unwrap_or(0)
    }

    /// The name saved preferences use (Windows' `FriendlyName` without spaces).
    fn key(self) -> &'static str {
        match self {
            Category::Volume => "Volume",
            Category::Length => "Length",
            Category::Weight => "Weight",
            Category::Temperature => "Temperature",
            Category::Energy => "Energy",
            Category::Area => "Area",
            Category::Speed => "Speed",
            Category::Time => "Time",
            Category::Power => "Power",
            Category::Data => "Data",
            Category::Pressure => "Pressure",
            Category::Angle => "Angle",
        }
    }

    fn all_units(self) -> &'static [Unit] {
        match self {
            Category::Volume => VOLUME,
            Category::Length => LENGTH,
            Category::Weight => WEIGHT,
            Category::Temperature => TEMPERATURE_UNITS,
            Category::Energy => ENERGY,
            Category::Area => AREA,
            Category::Speed => SPEED,
            Category::Time => TIME,
            Category::Power => POWER,
            Category::Data => DATA,
            Category::Pressure => PRESSURE,
            Category::Angle => ANGLE,
        }
    }

    fn comparisons(self) -> &'static [Comparison] {
        match self {
            Category::Volume => VOLUME_COMPARISONS,
            Category::Length => LENGTH_COMPARISONS,
            Category::Weight => WEIGHT_COMPARISONS,
            Category::Energy => ENERGY_COMPARISONS,
            Category::Area => AREA_COMPARISONS,
            Category::Speed => SPEED_COMPARISONS,
            Category::Power => POWER_COMPARISONS,
            Category::Data => DATA_COMPARISONS,
            Category::Temperature | Category::Time | Category::Pressure | Category::Angle => &[],
        }
    }
}

/// A unit of a category.
#[derive(Clone, Copy, Debug)]
pub struct Unit {
    /// Windows' unit id (`UnitConverterUnits`), used in saved preferences.
    id: u16,
    /// The unit's resource keys in Windows: `UnitName_{key}` and `UnitAbbreviation_{key}`.
    pub key: &'static str,
    /// The English name; the UI shows the translation of `UnitName_{key}`.
    #[cfg_attr(not(test), allow(dead_code))]
    pub name: &'static str,
    /// The short name in the "About equal to" results.
    pub abbreviation: &'static str,
    /// The unit's size in the category's base unit. Temperature uses `TEMPERATURE`.
    factor: f64,
}

const fn unit(
    id: u16,
    key: &'static str,
    name: &'static str,
    abbreviation: &'static str,
    factor: f64,
) -> Unit {
    Unit {
        id,
        key,
        name,
        abbreviation,
        factor,
    }
}

/// An everyday comparison unit ("2.3 bananas", Windows' whimsical units). It's never
/// in the drop-down lists; the best one ends the "About equal to" results.
#[derive(Clone, Copy, Debug)]
struct Comparison {
    /// The resource key suffix, as for `Unit::key`.
    key: &'static str,
    abbreviation: &'static str,
    /// The picture in front of the value: a Segoe UI Symbol emoji (`DelighterUnitStyles.xaml`).
    glyph: &'static str,
    /// The gap after the picture, which Windows sets per picture.
    gap: f32,
    factor: f64,
}

const fn comparison(
    key: &'static str,
    abbreviation: &'static str,
    glyph: &'static str,
    gap: f32,
    factor: f64,
) -> Comparison {
    Comparison {
        key,
        abbreviation,
        glyph,
        gap,
        factor,
    }
}

// The units of each category in the order of the drop-down list, and their factors
// exactly as `UnitConverterDataLoader.cs` writes them. The everyday comparison units
// (bananas, whales, …) follow in their own tables.

#[allow(clippy::excessive_precision)]
const VOLUME: &[Unit] = &[
    unit(75, "Milliliter", "Milliliters", "mL", 1.0),
    unit(64, "CubicCentimeter", "Cubic centimeters", "cm³", 1.0),
    unit(74, "Liter", "Liters", "L", 1000.0),
    unit(67, "CubicMeter", "Cubic meters", "m³", 1000000.0),
    unit(
        79,
        "TeaspoonUS",
        "Teaspoons (US)",
        "tsp. (US)",
        4.92892159375,
    ),
    unit(
        78,
        "TablespoonUS",
        "Tablespoons (US)",
        "tbsp. (US)",
        14.78676478125,
    ),
    unit(
        71,
        "FluidOunceUS",
        "Fluid ounces (US)",
        "fl oz (US)",
        29.5735295625,
    ),
    unit(69, "CupUS", "Cups (US)", "cup (US)", 236.588237),
    unit(77, "PintUS", "Pints (US)", "pt (US)", 473.176473),
    unit(81, "QuartUS", "Quarts (US)", "qt (US)", 946.352946),
    unit(73, "GallonUS", "Gallons (US)", "gal (US)", 3785.411784),
    unit(66, "CubicInch", "Cubic inches", "in³", 16.387064),
    unit(65, "CubicFoot", "Cubic feet", "ft³", 28316.846592),
    unit(68, "CubicYard", "Cubic yards", "yd³", 764554.857984),
    unit(
        115,
        "TeaspoonUK",
        "Teaspoons (UK)",
        "tsp. (UK)",
        5.91938802083333333333,
    ),
    unit(
        116,
        "TablespoonUK",
        "Tablespoons (UK)",
        "tbsp. (UK)",
        17.7581640625,
    ),
    unit(
        70,
        "FluidOunceUK",
        "Fluid ounces (UK)",
        "fl oz (UK)",
        28.4130625,
    ),
    unit(76, "PintUK", "Pints (UK)", "pt (UK)", 568.26125),
    unit(80, "QuartUK", "Quarts (UK)", "qt (UK)", 1136.5225),
    unit(72, "GallonUK", "Gallons (UK)", "gal (UK)", 4546.09),
];

const LENGTH: &[Unit] = &[
    unit(168, "Angstrom", "Angstroms", "A", 0.0000000001),
    unit(38, "Nanometer", "Nanometers", "nm", 0.000000001),
    unit(35, "Micron", "Microns", "µm", 0.000001),
    unit(37, "Millimeter", "Millimeters", "mm", 0.001),
    unit(30, "Centimeter", "Centimeters", "cm", 0.01),
    unit(34, "Meter", "Meters", "m", 1.0),
    unit(33, "Kilometer", "Kilometers", "km", 1000.0),
    unit(32, "Inch", "Inches", "in", 0.0254),
    unit(31, "Foot", "Feet", "ft", 0.3048),
    unit(40, "Yard", "Yards", "yd", 0.9144),
    unit(36, "Mile", "Miles", "mi", 1609.344),
    unit(39, "NauticalMile", "Nautical miles", "nmi", 1852.0),
];

const WEIGHT: &[Unit] = &[
    unit(82, "Carat", "Carats", "CD", 0.0002),
    unit(90, "Milligram", "Milligrams", "mg", 0.000001),
    unit(83, "Centigram", "Centigrams", "cg", 0.00001),
    unit(84, "Decigram", "Decigrams", "dg", 0.0001),
    unit(86, "Gram", "Grams", "g", 0.001),
    unit(85, "Decagram", "Dekagrams", "dag", 0.01),
    unit(87, "Hectogram", "Hectograms", "hg", 0.1),
    unit(88, "Kilogram", "Kilograms", "kg", 1.0),
    unit(95, "Tonne", "Metric tonnes", "t", 1000.0),
    unit(91, "Ounce", "Ounces", "oz", 0.028349523125),
    unit(92, "Pound", "Pounds", "lb", 0.45359237),
    unit(94, "Stone", "Stone", "st", 6.35029318),
    unit(93, "ShortTon", "Short tons (US)", "ton (US)", 907.18474),
    unit(89, "LongTon", "Long tons (UK)", "ton (UK)", 1016.0469088),
];

const TEMPERATURE_UNITS: &[Unit] = &[
    unit(46, "DegreesCelsius", "Celsius", "°C", 1.0),
    unit(47, "DegreesFahrenheit", "Fahrenheit", "°F", 1.0),
    unit(48, "Kelvin", "Kelvin", "K", 1.0),
];

/// `(ratio, offset, offset first)` from Celsius, Fahrenheit and Kelvin (rows) to
/// each of them (columns), from Windows' explicit temperature conversions.
#[allow(clippy::excessive_precision)]
const TEMPERATURE: [[(f64, f64, bool); 3]; 3] = [
    [(1.0, 0.0, false), (1.8, 32.0, false), (1.0, 273.15, false)],
    [
        (0.55555555555555555555555555555556, -32.0, true),
        (1.0, 0.0, false),
        (0.55555555555555555555555555555556, 459.67, true),
    ],
    [
        (1.0, -273.15, true),
        (1.8, -459.67, false),
        (1.0, 0.0, false),
    ],
];

const ENERGY: &[Unit] = &[
    unit(
        25,
        "Electron-Volt",
        "Electron volts",
        "eV",
        0.0000000000000000001602176565,
    ),
    unit(27, "Joule", "Joules", "J", 1.0),
    unit(29, "Kilojoule", "Kilojoules", "kJ", 1000.0),
    unit(24, "Calorie", "Thermal calories", "cal", 4.184),
    unit(28, "Kilocalorie", "Food calories", "kcal", 4184.0),
    unit(26, "Foot-Pound", "Foot-pounds", "ft•lb", 1.3558179483314),
    unit(
        23,
        "BritishThermalUnit",
        "British thermal units",
        "BTU",
        1055.056,
    ),
    unit(166, "Kilowatthour", "Kilowatt-hours", "kWh", 3600000.0),
];

/// Pyeong comes last, and only regions that use it show it.
const AREA: &[Unit] = &[
    unit(9, "SquareMillimeter", "Square millimeters", "mm²", 0.000001),
    unit(3, "SquareCentimeter", "Square centimeters", "cm²", 0.0001),
    unit(7, "SquareMeter", "Square meters", "m²", 1.0),
    unit(2, "Hectare", "Hectares", "ha", 10000.0),
    unit(6, "SquareKilometer", "Square kilometers", "km²", 1000000.0),
    unit(5, "SquareInch", "Square inches", "in²", 0.00064516),
    unit(4, "SquareFoot", "Square feet", "ft²", 0.09290304),
    unit(10, "SquareYard", "Square yards", "yd²", 0.83612736),
    unit(1, "Acre", "Acres", "ac", 4046.8564224),
    unit(8, "SquareMile", "Square miles", "mi²", 2589988.110336),
    unit(165, "Pyeong", "Pyeong", "Pyeong", 400.0 / 121.0),
];

/// Windows' factors: a mile per hour is 44.7 cm/s and a knot 51.44 cm/s.
#[allow(clippy::excessive_precision)]
const SPEED: &[Unit] = &[
    unit(
        57,
        "CentimetersPerSecond",
        "Centimeters per second",
        "cm/s",
        1.0,
    ),
    unit(62, "MetersPerSecond", "Meters per second", "m/s", 100.0),
    unit(
        59,
        "KilometersPerHour",
        "Kilometers per hour",
        "km/h",
        27.777777777777777777778,
    ),
    unit(58, "FeetPerSecond", "Feet per second", "ft/s", 30.48),
    unit(63, "MilesPerHour", "Miles per hour", "mph", 44.7),
    unit(60, "Knot", "Knots", "kn", 51.44),
    unit(61, "Mach", "Mach", "M", 34030.0),
];

const TIME: &[Unit] = &[
    unit(51, "Microsecond", "Microseconds", "µs", 0.000001),
    unit(52, "Millisecond", "Milliseconds", "ms", 0.001),
    unit(54, "Second", "Seconds", "s", 1.0),
    unit(53, "Minute", "Minutes", "min", 60.0),
    unit(50, "Hour", "Hours", "hr", 3600.0),
    unit(49, "Day", "Days", "d", 86400.0),
    unit(55, "Week", "Weeks", "wk", 604800.0),
    unit(56, "Year", "Years", "yr", 31557600.0),
];

#[allow(clippy::excessive_precision)]
const POWER: &[Unit] = &[
    unit(45, "Watt", "Watts", "W", 1.0),
    unit(44, "Kilowatt", "Kilowatts", "kW", 1000.0),
    unit(
        43,
        "Horsepower",
        "Horsepower (US)",
        "hp (US)",
        745.69987158227022,
    ),
    unit(
        42,
        "Foot-PoundPerMinute",
        "Foot-pounds/minute",
        "ft•lb/min",
        0.0225969658055233,
    ),
    unit(
        41,
        "BTUPerMinute",
        "BTUs/minute",
        "BTU/min",
        17.58426666666667,
    ),
];

#[allow(clippy::excessive_precision)]
const DATA: &[Unit] = &[
    unit(11, "Bit", "Bits", "b", 0.000000125),
    unit(167, "Nibble", "Nibble", "nybl", 0.0000005),
    unit(12, "Byte", "Bytes", "B", 0.000001),
    unit(15, "Kilobit", "Kilobits", "Kb", 0.000125),
    unit(149, "Kibibits", "Kibibits", "Ki", 0.000128),
    unit(16, "Kilobyte", "Kilobytes", "KB", 0.001),
    unit(150, "Kibibytes", "Kibibytes", "KiB", 0.001024),
    unit(17, "Megabit", "Megabits", "Mb", 0.125),
    unit(151, "Mebibits", "Mebibits", "Mi", 0.131072),
    unit(18, "Megabyte", "Megabytes", "MB", 1.0),
    unit(152, "Mebibytes", "Mebibytes", "MiB", 1.048576),
    unit(13, "Gigabit", "Gigabits", "Gb", 125.0),
    unit(147, "Gibibits", "Gibibits", "Gi", 134.217728),
    unit(14, "Gigabyte", "Gigabytes", "GB", 1000.0),
    unit(148, "Gibibytes", "Gibibytes", "GiB", 1073.741824),
    unit(21, "Terabit", "Terabits", "Tb", 125000.0),
    unit(155, "Tebibits", "Tebibits", "Ti", 137438.953472),
    unit(22, "Terabyte", "Terabytes", "TB", 1000000.0),
    unit(156, "Tebibytes", "Tebibytes", "TiB", 1099511.627776),
    unit(19, "Petabit", "Petabits", "Pb", 125000000.0),
    unit(153, "Pebibits", "Pebibits", "Pi", 140737488.355328),
    unit(20, "Petabyte", "Petabytes", "PB", 1000000000.0),
    unit(154, "Pebibytes", "Pebibytes", "PiB", 1125899906.842624),
    unit(143, "Exabits", "Exabits", "E", 125000000000.0),
    unit(145, "Exbibits", "Exbibits", "Ei", 144115188075.855872),
    unit(144, "Exabytes", "Exabytes", "EB", 1000000000000.0),
    unit(146, "Exbibytes", "Exbibytes", "EiB", 1152921504606.846976),
    unit(163, "Zetabits", "Zetabits", "Z", 125000000000000.0),
    unit(161, "Zebibits", "Zebibits", "Zi", 147573952589676.412928),
    unit(164, "Zetabytes", "Zetabytes", "ZB", 1000000000000000.0),
    unit(
        162,
        "Zebibytes",
        "Zebibytes",
        "ZiB",
        1180591620717411.303424,
    ),
    unit(159, "Yottabit", "Yottabits", "Y", 125000000000000000.0),
    unit(157, "Yobibits", "Yobibits", "Yi", 151115727451828646.838272),
    unit(160, "Yottabyte", "Yottabytes", "YB", 1000000000000000000.0),
    unit(
        158,
        "Yobibytes",
        "Yobibytes",
        "YiB",
        1208925819614629174.706176,
    ),
];

#[allow(clippy::excessive_precision)]
const PRESSURE: &[Unit] = &[
    unit(137, "Atmosphere", "Atmospheres", "atm", 1.0),
    unit(138, "Bar", "Bars", "ba", 0.9869232667160128),
    unit(139, "KiloPascal", "Kilopascals", "kPa", 0.0098692326671601),
    unit(
        140,
        "MillimeterOfMercury",
        "Millimeters of mercury",
        "mmHg",
        0.0013155687145324,
    ),
    unit(141, "Pascal", "Pascals", "Pa", 9.869232667160128e-6),
    unit(
        142,
        "PSI",
        "Pounds per square inch",
        "psi",
        0.068045961016531,
    ),
];

const ANGLE: &[Unit] = &[
    unit(134, "Degree", "Degrees", "deg", 1.0),
    unit(135, "Radian", "Radians", "rad", 57.29577951308233),
    unit(136, "Gradian", "Gradians", "grad", 0.9),
];

// The everyday comparison units, in Windows' order, with their pictures and gaps.

const VOLUME_COMPARISONS: &[Comparison] = &[
    comparison("CoffeeCup", "coffee cups", "☕", 8.0, 236.5882),
    comparison("Bathtub", "bathtubs", "🛀", 10.0, 378541.2),
    comparison("SwimmingPool", "swimming pools", "🏊", 8.0, 3750000000.0),
];

const LENGTH_COMPARISONS: &[Comparison] = &[
    comparison("Paperclip", "paperclips", "📎", 6.0, 0.035052),
    comparison("Hand", "hands", "👋", 8.0, 0.18669),
    comparison("JumboJet", "jumbo jets", "✈", 8.0, 76.0),
];

const WEIGHT_COMPARISONS: &[Comparison] = &[
    comparison("Snowflake", "snowflakes", "❄", 8.0, 0.000002),
    comparison("SoccerBall", "soccer balls", "⚽", 8.0, 0.4325),
    comparison("Elephant", "elephants", "🐘", 9.0, 4000.0),
    comparison("Whale", "whales", "🐳", 9.0, 90000.0),
];

const ENERGY_COMPARISONS: &[Comparison] = &[
    comparison("Battery", "batteries", "🔋", 10.0, 9000.0),
    comparison("Banana", "bananas", "🍌", 8.0, 439614.0),
    comparison("SliceOfCake", "slices of cake", "🍰", 8.0, 1046700.0),
];

const AREA_COMPARISONS: &[Comparison] = &[
    comparison("Hand", "hands", "👋", 8.0, 0.012516104),
    comparison("Paper", "sheets of paper", "📄", 8.0, 0.06032246),
    comparison("SoccerField", "soccer fields", "⚽", 10.0, 10869.66),
    comparison("Castle", "castles", "🏰", 10.0, 100000.0),
];

const SPEED_COMPARISONS: &[Comparison] = &[
    comparison("Turtle", "turtles", "🐢", 9.0, 8.94),
    comparison("Horse", "horses", "🐎", 8.0, 2011.5),
    comparison("Jet", "jets", "✈", 8.0, 24585.0),
];

#[allow(clippy::excessive_precision)]
const POWER_COMPARISONS: &[Comparison] = &[
    comparison("LightBulb", "light bulbs", "💡", 10.0, 60.0),
    comparison("Horse", "horses", "🐎", 8.0, 745.7),
    comparison("TrainEngine", "train engines", "🚂", 9.0, 2982799.486329081),
];

const DATA_COMPARISONS: &[Comparison] = &[
    comparison("FloppyDisk", "floppy disks", "💾", 9.0, 1.474560),
    comparison("CD", "CDs", "💿", 9.0, 700.0),
    comparison("DVD", "DVDs", "📀", 10.0, 4700.0),
];

/// One of the "About equal to" results: the value as shown, the unit's short name,
/// and for an everyday comparison its picture and the gap after it.
#[derive(Clone, Debug, PartialEq)]
pub struct Suggestion {
    pub value: String,
    /// The unit's resource key suffix (see `Unit::key`) and its English short name.
    pub key: &'static str,
    pub abbreviation: &'static str,
    pub glyph: Option<(&'static str, f32)>,
}

/// The region settings the units depend on, from the flags in Windows' `LoadData`.
#[derive(Clone, Copy, Debug)]
pub struct Region {
    us_customary: bool,
    fahrenheit: bool,
    /// The United Kingdom starts Power at watts instead of kilowatts.
    watts: bool,
    pyeong: bool,
}

impl Region {
    /// The settings for a region code from Windows (`US`, `CH`, …).
    pub fn new(code: &str) -> Region {
        let us = matches!(code, "US" | "FM" | "MH" | "PW");
        Region {
            us_customary: us || code == "LR",
            fahrenheit: us || matches!(code, "BS" | "KY" | "LR"),
            watts: code == "GB",
            pyeong: matches!(code, "JP" | "TW" | "KP" | "KR"),
        }
    }

    /// The units a category starts with (the first `isConversionSource` and
    /// `isConversionTarget` units). Regions with US customary units convert from
    /// metric, the others to it.
    fn default_units(self, category: Category) -> (u16, u16) {
        let metric_first = |metric, customary| {
            if self.us_customary {
                (metric, customary)
            } else {
                (customary, metric)
            }
        };
        match category {
            // Milliliters and teaspoons (US); Windows never picks the UK teaspoon.
            Category::Volume => metric_first(75, 79),
            Category::Length => metric_first(30, 32),
            Category::Weight => metric_first(88, 92),
            Category::Area => metric_first(7, 4),
            Category::Speed => metric_first(59, 63),
            Category::Temperature if self.fahrenheit => (46, 47),
            Category::Temperature => (47, 46),
            Category::Energy => (27, 28),
            Category::Time => (50, 53),
            Category::Power if self.watts => (45, 43),
            Category::Power => (44, 43),
            Category::Data => (14, 18),
            Category::Pressure => (137, 138),
            Category::Angle => (134, 135),
        }
    }
}

/// A key on the converter keypad.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConverterCommand {
    Digit(u8),
    Decimal,
    Negate,
    Backspace,
    /// `CE`: back to 0.
    Clear,
}

/// The converter's state: the category, the two units, and the number being typed
/// and its conversion as Windows keeps them, as unformatted strings.
#[derive(Clone, Debug)]
pub struct Converter {
    region: Region,
    category: Category,
    /// Units picked in each category, as (source, target) indices. Windows keeps
    /// them while the app runs.
    picked: [(usize, usize); 12],
    from: usize,
    to: usize,
    /// The number being typed (`m_currentDisplay`) and the result (`m_returnDisplay`).
    current: String,
    current_has_decimal: bool,
    result: String,
    result_has_decimal: bool,
    /// The fields or the source unit changed, so the next digit starts a new
    /// number (`m_switchedActive`).
    switched_active: bool,
    /// The top field is the one being typed into (the view model's `_value1cp`).
    value1_is_source: bool,
    /// A paste was rejected: both fields show `Invalid input` until the next change.
    paste_error: bool,
}

impl Converter {
    pub fn new(region: Region) -> Self {
        let mut picked = [(0, 0); 12];
        for category in Category::ALL {
            let (from, to) = region.default_units(category);
            let find = |id| {
                category
                    .all_units()
                    .iter()
                    .position(|u| u.id == id)
                    .unwrap_or(0)
            };
            picked[category.index()] = (find(from), find(to));
        }
        let (from, to) = picked[0];
        let mut converter = Converter {
            region,
            category: Category::Volume,
            picked,
            from,
            to,
            current: "0".into(),
            current_has_decimal: false,
            result: "0".into(),
            result_has_decimal: false,
            switched_active: false,
            value1_is_source: true,
            paste_error: false,
        };
        converter.calculate();
        converter
    }

    // --------------------------------------------------------------- queries

    pub fn category(&self) -> Category {
        self.category
    }

    /// The units of the current category, in the order of the drop-down list.
    pub fn units(&self) -> &'static [Unit] {
        let units = self.category.all_units();
        if self.category == Category::Area && !self.region.pyeong {
            &units[..units.len() - 1]
        } else {
            units
        }
    }

    /// The units of the top and bottom fields, as indices into `units`.
    pub fn unit1(&self) -> usize {
        if self.value1_is_source {
            self.from
        } else {
            self.to
        }
    }

    pub fn unit2(&self) -> usize {
        if self.value1_is_source {
            self.to
        } else {
            self.from
        }
    }

    /// Whether the top field is the one being typed into (shown in semibold).
    pub fn value1_active(&self) -> bool {
        self.value1_is_source
    }

    /// The text of the top and bottom fields, with thousands separators (unless they
    /// show the paste error: see `has_paste_error`).
    pub fn value1(&self) -> String {
        self.field(self.value1_is_source)
    }

    pub fn value2(&self) -> String {
        self.field(!self.value1_is_source)
    }

    /// Whether both fields show the paste error, Windows' `Invalid input`, instead of
    /// their values. The UI writes it in the user's language.
    pub fn has_paste_error(&self) -> bool {
        self.paste_error
    }

    fn field(&self, source: bool) -> String {
        localize(if source { &self.current } else { &self.result })
    }

    /// What `Ctrl+C` copies: the active value without separators.
    pub fn copy_text(&self) -> String {
        self.current.clone()
    }

    // --------------------------------------------------------------- changes

    /// Switches to another converter. The value being typed stays (without its
    /// minus sign where values can't be negative), and the units are the ones
    /// last used in that category.
    pub fn set_category(&mut self, category: Category) {
        if category == self.category {
            return;
        }
        self.picked[self.category.index()] = (self.from, self.to);
        self.category = category;
        if !category.supports_negative() && self.current.starts_with('-') {
            self.current.remove(0);
        }
        (self.from, self.to) = self.picked[category.index()];
        self.calculate();
    }

    pub fn set_unit1(&mut self, index: usize) {
        if self.value1_is_source {
            self.set_units(index, self.to);
        } else {
            self.set_units(self.from, index);
        }
    }

    pub fn set_unit2(&mut self, index: usize) {
        if self.value1_is_source {
            self.set_units(self.from, index);
        } else {
            self.set_units(index, self.to);
        }
    }

    /// `SetCurrentUnitTypes`: a new source unit makes the next digit start a new number.
    fn set_units(&mut self, from: usize, to: usize) {
        if from != self.from {
            self.switched_active = true;
        }
        self.from = from;
        self.to = to;
        self.picked[self.category.index()] = (from, to);
        self.calculate();
    }

    /// Makes the top (`true`) or bottom field the one being typed into. Its value
    /// stays, and the next digit replaces it.
    pub fn activate(&mut self, value1: bool) {
        if value1 == self.value1_is_source {
            return;
        }
        // `OnSwitchActive` and `SwitchActive`: the roles swap, the shown values don't.
        self.value1_is_source = value1;
        std::mem::swap(&mut self.from, &mut self.to);
        std::mem::swap(&mut self.current_has_decimal, &mut self.result_has_decimal);
        std::mem::swap(&mut self.current, &mut self.result);
        self.current_has_decimal = self.current.contains('.');
        self.switched_active = true;
    }

    /// A keypad key (`UnitConverter::SendCommand`).
    pub fn execute(&mut self, command: ConverterCommand) {
        use ConverterCommand::*;
        let (mut clear_front, mut clear_back);
        if command != Negate && self.switched_active {
            self.clear_values();
            self.switched_active = false;
            clear_front = true;
            clear_back = false;
        } else {
            clear_front = self.current == "0";
            // The limit counts the minus sign but not the decimal point.
            let length = self.current.len() - usize::from(self.current_has_decimal);
            clear_back = length >= MAX_DIGITS;
        }

        match command {
            Digit(digit) => self.current.push(char::from(b'0' + digit)),
            Decimal => {
                (clear_front, clear_back) = (false, false);
                if !self.current_has_decimal {
                    self.current.push('.');
                    self.current_has_decimal = true;
                }
            }
            Backspace => {
                (clear_front, clear_back) = (false, false);
                let length = self.current.len();
                if (!self.current.starts_with('-') && length > 1) || length > 2 {
                    if self.current.ends_with('.') {
                        self.current_has_decimal = false;
                    }
                    self.current.pop();
                } else {
                    self.current = "0".into();
                    self.current_has_decimal = false;
                }
            }
            Negate => {
                (clear_front, clear_back) = (false, false);
                if self.category.supports_negative() {
                    if self.current.starts_with('-') {
                        self.current.remove(0);
                    } else {
                        self.current.insert(0, '-');
                    }
                }
            }
            Clear => {
                (clear_front, clear_back) = (false, false);
                self.clear_values();
            }
        }

        // A digit replaces a lone 0, and one beyond the limit is dropped again.
        if clear_front {
            self.current.remove(0);
        }
        if clear_back {
            self.current.pop();
        }
        self.calculate();
    }

    fn clear_values(&mut self) {
        self.current = "0".into();
        self.current_has_decimal = false;
        self.result_has_decimal = false;
    }

    /// Pastes a number the way Windows does: the text is checked with the rules of
    /// `CopyPasteManager` and then typed in key by key after a `CE`. A `-` at the
    /// start becomes `±` after the next digit. Invalid text shows `Invalid input`.
    /// Returns whether the text was accepted.
    pub fn paste(&mut self, text: &str) -> bool {
        use ConverterCommand::*;
        if !paste_is_valid(text) {
            self.paste_error = true;
            return false;
        }
        // `UnitConverterViewModel::OnPaste`.
        let mut first = true;
        let mut negate = false;
        for c in text.chars() {
            let (command, can_negate) = match c {
                '0'..='9' => (Digit(c as u8 - b'0'), true),
                '.' => (Decimal, true),
                '-' => (Negate, false),
                // Anything else (a space, too) cancels a pending minus sign.
                _ => {
                    negate = false;
                    continue;
                }
            };
            if first {
                self.execute(Clear);
                first = false;
                negate = command == Negate;
            }
            if command != Negate {
                self.execute(command);
                if negate && can_negate {
                    self.execute(Negate);
                }
                negate = false;
            }
        }
        true
    }

    // ------------------------------------------------------------ conversion

    /// `(ratio, offset, offset first)` from the source unit to unit `to` of the category.
    fn conversion(&self, to: usize) -> (f64, f64, bool) {
        if self.category == Category::Temperature {
            TEMPERATURE[self.from][to]
        } else {
            let units = self.units();
            (units[self.from].factor / units[to].factor, 0.0, false)
        }
    }

    /// Converts the value being typed and rounds the result like Windows
    /// (`UnitConverter::Calculate`).
    fn calculate(&mut self) {
        // Windows shows the new values, which replaces a paste error.
        self.paste_error = false;
        let conversion = self.conversion(self.to);
        let (ratio, offset, _) = conversion;
        if ratio == 1.0 && offset == 0.0 {
            self.result = trim_trailing_zeros(&self.current);
            self.result_has_decimal = self.current_has_decimal;
            return;
        }

        let result = convert(self.value(), conversion);
        let whole_digits = whole_number_digits(result);
        self.result =
            if whole_digits > MAX_DIGITS || (result != 0.0 && result.abs() < MINIMUM_DECIMAL) {
                to_scientific(result)
            } else {
                // At least 7 significant digits, more when more were typed, and only as
                // many decimals as the integer part leaves room for.
                let precision = if result.abs() < OPTIMAL_DECIMAL {
                    MAX_DIGITS
                } else {
                    let digits = number_digits(&self.current).clamp(OPTIMAL_DIGITS, MAX_DIGITS);
                    digits.saturating_sub(whole_digits)
                };
                trim_trailing_zeros(&format!("{result:.precision$}"))
            };
        self.result_has_decimal = self.result.contains('.');
    }

    /// The value being typed, as a number (C++'s `stod`).
    fn value(&self) -> f64 {
        self.current.parse().unwrap_or(0.0)
    }

    /// The "About equal to" results (`UnitConverter::CalculateSuggested`): the value in
    /// the category's other units, closest to 1 first, then the best everyday comparison.
    pub fn suggestions(&self) -> Vec<Suggestion> {
        let value = self.value();
        // Windows sorts by the size of log10(value), and on a tie puts the larger value
        // first. For negative values its log10 is NaN and the order undefined; the
        // absolute value keeps it predictable here.
        let sorted = |mut values: Vec<(f64, usize)>| {
            let magnitude = |v: f64| v.abs().log10();
            values.sort_by(|&(a, _), &(b, _)| {
                let (ma, mb) = (magnitude(a), magnitude(b));
                ma.abs().total_cmp(&mb.abs()).then(mb.total_cmp(&ma))
            });
            values
        };

        let units = self.units();
        let others = (0..units.len())
            .filter(|&i| i != self.from && i != self.to)
            .map(|i| (convert(value, self.conversion(i)), i))
            .collect();
        let mut suggestions: Vec<Suggestion> = sorted(others)
            .into_iter()
            .filter_map(|(converted, i)| {
                let (value, zero) = suggested_value(converted);
                // Zeros are left out, except where values can be negative.
                (!zero || self.category.supports_negative()).then(|| Suggestion {
                    value,
                    key: units[i].key,
                    abbreviation: units[i].abbreviation,
                    glyph: None,
                })
            })
            .collect();

        let comparisons = self.category.comparisons();
        let source = units[self.from].factor;
        let everyday = comparisons
            .iter()
            .enumerate()
            .map(|(i, c)| (value * (source / c.factor), i))
            .collect();
        let best = sorted(everyday).into_iter().find_map(|(converted, i)| {
            let (value, zero) = suggested_value(converted);
            (!zero).then(|| Suggestion {
                value,
                key: comparisons[i].key,
                abbreviation: comparisons[i].abbreviation,
                glyph: Some((comparisons[i].glyph, comparisons[i].gap)),
            })
        });
        suggestions.extend(best);
        suggestions
    }

    // ----------------------------------------------------------- preferences

    /// The category and its two units, for saving (Windows saves the same in
    /// `UnitConverterPreferences`).
    pub fn preferences(&self) -> String {
        let units = self.units();
        format!(
            "{};{};{}",
            self.category.key(),
            units[self.from].id,
            units[self.to].id
        )
    }

    /// Restores what `preferences` saved. Units that don't exist (any more) are ignored.
    pub fn restore_preferences(&mut self, saved: &str) {
        let mut parts = saved.split(';');
        let (Some(key), Some(from), Some(to), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return;
        };
        let Some(category) = Category::ALL.into_iter().find(|c| c.key() == key) else {
            return;
        };
        self.set_category(category);
        let find = |id: &str| {
            let id: u16 = id.parse().ok()?;
            self.units().iter().position(|u| u.id == id)
        };
        let from = find(from).unwrap_or(self.from);
        let to = find(to).unwrap_or(self.to);
        self.from = from;
        self.to = to;
        self.picked[category.index()] = (from, to);
        self.calculate();
    }
}

// ------------------------------------------------------------- formatting

/// `value` converted with `(ratio, offset, offset first)` (`UnitConverter::Convert`).
fn convert(value: f64, (ratio, offset, offset_first): (f64, f64, bool)) -> f64 {
    if offset_first {
        (value + offset) * ratio
    } else {
        value * ratio + offset
    }
}

/// An "About equal to" value as the row shows it, and whether it rounds to 0.
fn suggested_value(value: f64) -> (String, bool) {
    let rounded = round_suggestion(value);
    let zero = rounded.parse::<f64>().unwrap_or(0.0) == 0.0;
    (localize(&trim_trailing_zeros(&rounded)), zero)
}

/// An "About equal to" value: 2 decimals below 100, 1 below 1,000, none above
/// (`RoundSignificantDigits`).
fn round_suggestion(value: f64) -> String {
    if value.abs() < 1000.0 {
        let decimals = if value.abs() < 100.0 { 2 } else { 1 };
        return format!("{value:.decimals$}");
    }
    // The view model parses the whole number again and formats it with WinRT's
    // `DecimalFormatter`, which writes 15 significant digits when they give the same
    // double back, else 17, and zeros after them. From 10^17 on, that's fewer digits than
    // the double's own (365249999999999616 shows as 365,249,999,999,999,620).
    let whole = value.round_ties_even();
    let mut scientific = format!("{whole:.14e}");
    if scientific.parse::<f64>().ok() != Some(whole) {
        scientific = format!("{whole:.16e}");
    }
    let Some((mantissa, exponent)) = scientific.split_once('e') else {
        return scientific;
    };
    let sign = if whole < 0.0 { "-" } else { "" };
    let mut digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();
    let length = exponent.parse::<usize>().unwrap_or(0) + 1;
    digits.truncate(length);
    format!("{sign}{digits:0<length$}")
}

/// Removes trailing zeros after the decimal point, and the point itself if
/// nothing follows it (`TrimTrailingZeros`).
fn trim_trailing_zeros(number: &str) -> String {
    if !number.contains('.') {
        return number.to_string();
    }
    let trimmed = number.trim_end_matches('0');
    trimmed.strip_suffix('.').unwrap_or(trimmed).to_string()
}

/// Digits in a typed number, without trailing zeros (`GetNumberDigits`).
fn number_digits(value: &str) -> usize {
    let value = trim_trailing_zeros(value);
    value.len() - usize::from(value.contains('.')) - usize::from(value.contains('-'))
}

/// Digits before the decimal point (`GetNumberDigitsWholeNumberPart`).
fn whole_number_digits(value: f64) -> usize {
    if value == 0.0 {
        1
    } else {
        (1.0 + value.abs().log10().max(0.0)) as usize
    }
}

/// C++'s `std::scientific`: six decimals and at least two exponent digits,
/// as in `1.234568e+20` (`ToScientificNumber`).
fn to_scientific(value: f64) -> String {
    let text = format!("{value:.6e}");
    let Some((mantissa, exponent)) = text.split_once('e') else {
        return text;
    };
    let exponent: i32 = exponent.parse().unwrap_or(0);
    let sign = if exponent < 0 { '-' } else { '+' };
    format!("{mantissa}e{sign}{:02}", exponent.abs())
}

/// A value as a field shows it: the integer part grouped (`1,234.5`), the
/// decimals as typed, a lone decimal point kept while typing (`3.`), and the
/// exponent of scientific notation without a leading zero
/// (`UnitConverterViewModel::ConvertToLocalizedString`).
fn localize(value: &str) -> String {
    if value.is_empty() {
        return "0".to_string();
    }
    if let Some((significand, exponent)) = value.split_once('e') {
        let (sign, digits) = exponent.split_at(exponent.len().min(1));
        return format!("{}e{sign}{}", localize(significand), localize(digits));
    }
    if value.parse::<f64>().is_err() {
        return value.to_string();
    }
    let (sign, unsigned) = match value.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", value),
    };
    let (int, fraction) = match unsigned.split_once('.') {
        Some((int, fraction)) => (int, Some(fraction)),
        None => (unsigned, None),
    };
    let int = int.trim_start_matches('0');
    let int = group_thousands(if int.is_empty() { "0" } else { int });
    match fraction {
        Some(fraction) => format!("{sign}{int}.{fraction}"),
        None => format!("{sign}{int}"),
    }
}

/// Checks pasted text like `CopyPasteManager` does for the converters: one plain
/// number (no expression, no e-notation) of at most 16 characters.
fn paste_is_valid(text: &str) -> bool {
    let Some(expression) = clean_paste(text) else {
        return false;
    };
    // Windows still splits the text to enforce the operand and exponent limits,
    // then checks it as a whole.
    if extract_operands(&expression, BASIC_PASTE_CHARACTERS).is_empty() {
        return false;
    }
    let length = expression
        .chars()
        .filter(|c| !matches!(c, '\'' | '_' | '`' | '(' | ')' | '-' | '+'))
        .count();
    operand_matches(&expression, false) && length <= MAX_PASTE_LENGTH
}

#[cfg(test)]
mod tests {
    use super::ConverterCommand::*;
    use super::*;

    /// Every unit's key names it in the Windows resources: the English strings there
    /// are the ones in the tables.
    #[test]
    fn units_have_windows_resource_keys() {
        let english: std::collections::HashMap<&str, &str> =
            include_str!("../../../locales/en-US.txt")
                .lines()
                .filter_map(|line| line.split_once('='))
                .collect();
        let string = |kind: &str, key: &str| english.get(format!("{kind}_{key}").as_str()).copied();
        for category in Category::ALL {
            for unit in category.all_units() {
                assert_eq!(string("UnitName", unit.key), Some(unit.name), "{unit:?}");
                assert_eq!(
                    string("UnitAbbreviation", unit.key),
                    Some(unit.abbreviation),
                    "{unit:?}"
                );
            }
            for comparison in category.comparisons() {
                assert_eq!(
                    string("UnitAbbreviation", comparison.key),
                    Some(comparison.abbreviation),
                    "{comparison:?}"
                );
            }
        }
    }

    /// A converter for the US region, switched to `category` with the units at
    /// the given positions of the drop-down list.
    fn converter(category: Category, from: &str, to: &str) -> Converter {
        let mut c = Converter::new(Region::new("US"));
        c.set_category(category);
        let find = |name: &str| c.units().iter().position(|u| u.name == name).unwrap();
        let (from, to) = (find(from), find(to));
        c.set_unit1(from);
        c.set_unit2(to);
        c
    }

    /// Types keys: digits, `.`, `n` = ±, `<` = ⌫, `c` = CE.
    fn press(c: &mut Converter, keys: &str) {
        for key in keys.chars() {
            let command = match key {
                '0'..='9' => Digit(key as u8 - b'0'),
                '.' => Decimal,
                'n' => Negate,
                '<' => Backspace,
                'c' => Clear,
                _ => panic!("unknown key {key}"),
            };
            c.execute(command);
        }
    }

    fn convert(category: Category, from: &str, to: &str, keys: &str) -> String {
        let mut c = converter(category, from, to);
        press(&mut c, keys);
        c.value2()
    }

    #[test]
    fn acceptance_values_from_features() {
        assert_eq!(
            convert(Category::Length, "Kilometers", "Miles", "5"),
            "3.106856"
        );
        assert_eq!(
            convert(Category::Volume, "Gallons (US)", "Liters", "1"),
            "3.785412"
        );
        assert_eq!(
            convert(Category::Weight, "Kilograms", "Pounds", "1"),
            "2.204623"
        );
        assert_eq!(
            convert(Category::Energy, "Food calories", "Joules", "1"),
            "4,184"
        );
        assert_eq!(
            convert(Category::Area, "Acres", "Square feet", "1"),
            "43,560"
        );
        assert_eq!(
            convert(Category::Speed, "Mach", "Kilometers per hour", "1"),
            "1,225.08"
        );
        assert_eq!(convert(Category::Time, "Weeks", "Hours", "1"), "168");
        assert_eq!(convert(Category::Time, "Weeks", "Minutes", "1"), "10,080");
        assert_eq!(
            convert(Category::Power, "Horsepower (US)", "Watts", "1"),
            "745.6999"
        );
        assert_eq!(convert(Category::Data, "Kilobytes", "Bytes", "1"), "1,000");
        assert_eq!(convert(Category::Data, "Kibibytes", "Bytes", "1"), "1,024");
        assert_eq!(
            convert(Category::Pressure, "Atmospheres", "Pascals", "1"),
            "101,325"
        );
        assert_eq!(
            convert(
                Category::Pressure,
                "Atmospheres",
                "Pounds per square inch",
                "1"
            ),
            "14.69595"
        );
        assert_eq!(
            convert(Category::Angle, "Degrees", "Gradians", "180"),
            "200"
        );
        assert_eq!(
            convert(Category::Angle, "Degrees", "Radians", "180"),
            "3.141593"
        );
    }

    #[test]
    fn temperature_uses_offsets() {
        assert_eq!(
            convert(Category::Temperature, "Celsius", "Fahrenheit", "0"),
            "32"
        );
        assert_eq!(
            convert(Category::Temperature, "Celsius", "Kelvin", "0"),
            "273.15"
        );
        assert_eq!(
            convert(Category::Temperature, "Fahrenheit", "Celsius", "212"),
            "100"
        );
        assert_eq!(
            convert(Category::Temperature, "Kelvin", "Fahrenheit", "0"),
            "-459.67"
        );
        assert_eq!(
            convert(Category::Temperature, "Celsius", "Fahrenheit", "40n"),
            "-40"
        );
    }

    #[test]
    fn rounding_keeps_at_least_seven_digits() {
        // 7 significant digits, or as many as were typed (up to 15).
        assert_eq!(
            convert(Category::Length, "Inches", "Centimeters", "1"),
            "2.54"
        );
        assert_eq!(
            convert(Category::Length, "Miles", "Kilometers", "1"),
            "1.609344"
        );
        assert_eq!(convert(Category::Length, "Feet", "Meters", "1"), "0.3048");
        assert_eq!(convert(Category::Length, "Meters", "Feet", "1"), "3.28084");
        assert_eq!(
            convert(Category::Length, "Meters", "Feet", "1.000000001"),
            "3.280839898"
        );
        // Below 0.000001 the result gets 15 decimals.
        assert_eq!(
            convert(Category::Length, "Nanometers", "Kilometers", "1"),
            "0.000000000001"
        );
    }

    #[test]
    fn scientific_notation_for_extremes() {
        assert_eq!(
            convert(Category::Data, "Yottabytes", "Bits", "1"),
            "8.000000e+24"
        );
        assert_eq!(
            convert(Category::Energy, "Electron volts", "Kilowatt-hours", "1"),
            "4.450490e-26"
        );
        assert_eq!(to_scientific(1.5e-5), "1.500000e-05");
        assert_eq!(localize("1.500000e-05"), "1.500000e-5");
    }

    #[test]
    fn same_factor_echoes_the_input() {
        let mut c = converter(Category::Volume, "Milliliters", "Cubic centimeters");
        press(&mut c, "12.50");
        assert_eq!((c.value1(), c.value2()), ("12.50".into(), "12.5".into()));
        press(&mut c, "c3.");
        assert_eq!((c.value1(), c.value2()), ("3.".into(), "3".into()));
    }

    #[test]
    fn typing_rules() {
        let mut c = converter(Category::Length, "Meters", "Centimeters");
        assert_eq!(c.value1(), "0");
        press(&mut c, "0001234567");
        assert_eq!(c.value1(), "1,234,567");
        press(&mut c, "..5");
        assert_eq!(c.value1(), "1,234,567.5");
        press(&mut c, "<<<");
        assert_eq!(c.value1(), "123,456");
        press(&mut c, "c");
        assert_eq!((c.value1(), c.value2()), ("0".into(), "0".into()));
        // 15 digits at most; the decimal point doesn't count.
        press(&mut c, "1234567890123456789");
        assert_eq!(c.value1(), "123,456,789,012,345");
        press(&mut c, "c.12345678901234567");
        assert_eq!(c.copy_text(), "0.12345678901234");
    }

    #[test]
    fn negate_only_where_values_can_be_negative() {
        let mut c = converter(Category::Length, "Meters", "Feet");
        press(&mut c, "5n");
        assert_eq!(c.value1(), "5");
        let mut c = converter(Category::Angle, "Degrees", "Gradians");
        press(&mut c, "9n");
        assert_eq!((c.value1(), c.value2()), ("-9".into(), "-10".into()));
        // ⌫ on the last digit drops the sign too.
        press(&mut c, "<");
        assert_eq!(c.value1(), "0");
        // Switching to a category without negatives drops the sign.
        let mut c = converter(Category::Temperature, "Celsius", "Kelvin");
        press(&mut c, "5n");
        c.set_category(Category::Length);
        assert_eq!(c.copy_text(), "5");
    }

    #[test]
    fn activating_the_other_field_swaps_roles() {
        let mut c = converter(Category::Length, "Kilometers", "Miles");
        assert!(c.value1_active());
        press(&mut c, "5");
        c.activate(false);
        assert!(!c.value1_active());
        // The values stay until the next key, which starts a new number.
        assert_eq!((c.value1(), c.value2()), ("5".into(), "3.106856".into()));
        assert_eq!(c.copy_text(), "3.106856");
        press(&mut c, "1");
        assert_eq!((c.value1(), c.value2()), ("1.609344".into(), "1".into()));
        // The units stay with their fields.
        assert_eq!(c.units()[c.unit1()].name, "Kilometers");
        assert_eq!(c.units()[c.unit2()].name, "Miles");
        c.activate(true);
        assert!(c.value1_active());
    }

    /// A new source unit makes the next digit start a new number; a new target unit
    /// just converts again.
    #[test]
    fn only_a_new_source_unit_starts_a_new_number() {
        let mut c = converter(Category::Length, "Kilometers", "Miles");
        let position =
            |c: &Converter, name: &str| c.units().iter().position(|u| u.name == name).unwrap();
        press(&mut c, "5");
        let feet = position(&c, "Feet");
        c.set_unit2(feet);
        press(&mut c, "1");
        assert_eq!(c.value1(), "51");
        let meters = position(&c, "Meters");
        c.set_unit1(meters);
        assert_eq!(c.value1(), "51");
        press(&mut c, "7");
        assert_eq!(c.value1(), "7");
        // The same source unit again changes nothing.
        c.set_unit1(meters);
        press(&mut c, "3");
        assert_eq!(c.value1(), "73");
    }

    #[test]
    fn backspace_removes_the_last_character_and_keeps_a_zero() {
        let mut c = converter(Category::Temperature, "Celsius", "Kelvin");
        press(&mut c, "123<");
        assert_eq!(c.value1(), "12");
        press(&mut c, "<<");
        assert_eq!(c.value1(), "0");
        press(&mut c, "<");
        assert_eq!(c.value1(), "0");
        // A minus sign counts as a character, and goes with the last digit.
        press(&mut c, "56n");
        assert_eq!(c.value1(), "-56");
        press(&mut c, "<");
        assert_eq!(c.value1(), "-5");
        press(&mut c, "<");
        assert_eq!(c.value1(), "0");
        // The decimal point can be removed and typed again.
        press(&mut c, "1.<");
        assert_eq!(c.value1(), "1");
        press(&mut c, ".5");
        assert_eq!(c.value1(), "1.5");
        // The last digit leaves a 0 to type on, so a point after it follows a 0.
        press(&mut c, "c7<.5");
        assert_eq!(c.value1(), "0.5");
    }

    #[test]
    fn changing_units() {
        let mut c = converter(Category::Length, "Kilometers", "Miles");
        press(&mut c, "5");
        // A new target unit converts the same value.
        let meters = c.units().iter().position(|u| u.name == "Meters").unwrap();
        c.set_unit2(meters);
        assert_eq!(c.value2(), "5,000");
        // A new source unit converts it too, but the next digit starts over.
        let feet = c.units().iter().position(|u| u.name == "Feet").unwrap();
        c.set_unit1(feet);
        assert_eq!(c.value2(), "1.524");
        press(&mut c, "2");
        assert_eq!(c.value1(), "2");
    }

    #[test]
    fn categories_remember_their_units() {
        let mut c = Converter::new(Region::new("US"));
        c.set_category(Category::Length);
        assert_eq!(c.units()[c.unit1()].name, "Centimeters");
        assert_eq!(c.units()[c.unit2()].name, "Inches");
        c.set_unit2(c.units().iter().position(|u| u.name == "Feet").unwrap());
        c.set_category(Category::Weight);
        assert_eq!(c.units()[c.unit1()].name, "Kilograms");
        c.set_category(Category::Length);
        assert_eq!(c.units()[c.unit2()].name, "Feet");
    }

    #[test]
    fn region_defaults() {
        let us = Converter::new(Region::new("US"));
        let mut ch = Converter::new(Region::new("CH"));
        assert_eq!(us.units()[us.unit1()].name, "Milliliters");
        assert_eq!(us.units()[us.unit2()].name, "Teaspoons (US)");
        assert_eq!(ch.units()[ch.unit1()].name, "Teaspoons (US)");
        assert_eq!(ch.units()[ch.unit2()].name, "Milliliters");
        ch.set_category(Category::Temperature);
        assert_eq!(ch.units()[ch.unit1()].name, "Fahrenheit");
        ch.set_category(Category::Area);
        assert!(ch.units().iter().all(|u| u.name != "Pyeong"));
        let mut kr = Converter::new(Region::new("KR"));
        kr.set_category(Category::Area);
        assert_eq!(kr.units().last().unwrap().name, "Pyeong");
        let mut gb = Converter::new(Region::new("GB"));
        gb.set_category(Category::Power);
        assert_eq!(gb.units()[gb.unit1()].name, "Watts");
    }

    #[test]
    fn paste() {
        let mut c = converter(Category::Temperature, "Celsius", "Fahrenheit");
        assert!(c.paste(" 1,234.5 "));
        assert_eq!(c.value1(), "1,234.5");
        assert!(c.paste("-40"));
        assert_eq!((c.value1(), c.value2()), ("-40".into(), "-40".into()));
        // A space after the minus sign cancels it.
        assert!(c.paste("- 3"));
        assert_eq!(c.value1(), "3");
        for invalid in ["abc", "1+2", "1e5", "12345678901234567", ""] {
            assert!(!c.paste(invalid), "{invalid:?}");
            assert!(c.has_paste_error(), "{invalid:?}");
        }
        // The next key shows the values again.
        press(&mut c, "7");
        assert!(!c.has_paste_error());
        assert_eq!(c.value1(), "37");
        // Without negatives, the minus sign is dropped.
        let mut c = converter(Category::Length, "Meters", "Feet");
        assert!(c.paste("-2"));
        assert_eq!(c.value1(), "2");
    }

    #[test]
    fn preferences_round_trip() {
        let mut c = converter(Category::Data, "Bytes", "Kibibytes");
        let saved = c.preferences();
        assert_eq!(saved, "Data;12;150");
        let mut restored = Converter::new(Region::new("US"));
        restored.restore_preferences(&saved);
        assert_eq!(restored.category(), Category::Data);
        assert_eq!(restored.units()[restored.unit1()].name, "Bytes");
        assert_eq!(restored.units()[restored.unit2()].name, "Kibibytes");
        // Unknown text is ignored.
        c.restore_preferences("Nonsense;1;2");
        c.restore_preferences("");
        assert_eq!(c.category(), Category::Data);
    }

    fn shown(c: &Converter) -> Vec<String> {
        c.suggestions()
            .iter()
            .map(|s| format!("{} {}", s.value, s.abbreviation))
            .collect()
    }

    #[test]
    fn suggestions_like_windows() {
        let mut c = converter(Category::Length, "Centimeters", "Inches");
        // Nothing for 0.
        assert!(c.suggestions().is_empty());
        press(&mut c, "100");
        // Closest to 1 first; values that round to 0 (km, mi, nmi) are left out, and the
        // closest everyday comparison comes last.
        assert_eq!(
            shown(&c),
            [
                "1 m",
                "1.09 yd",
                "3.28 ft",
                "1,000 mm",
                "1,000,000 µm",
                "1,000,000,000 nm",
                "10,000,000,000 A",
                "5.36 hands"
            ]
        );
        assert_eq!(c.suggestions()[0].glyph, None);
        assert_eq!(c.suggestions().last().unwrap().glyph, Some(("👋", 8.0)));
    }

    #[test]
    fn suggestions_keep_zeros_where_values_can_be_negative() {
        let c = converter(Category::Angle, "Degrees", "Radians");
        assert_eq!(shown(&c), ["0 grad"]);
        // No everyday comparisons for angles, temperatures, time or pressure.
        let mut c = converter(Category::Time, "Hours", "Minutes");
        press(&mut c, "1");
        assert!(c.suggestions().iter().all(|s| s.glyph.is_none()));
    }

    #[test]
    fn suggestions_round_by_size() {
        assert_eq!(round_suggestion(5.356), "5.36");
        assert_eq!(round_suggestion(123.45), "123.5");
        assert_eq!(round_suggestion(1234.5), "1234");
        let mut c = converter(Category::Energy, "Food calories", "Joules");
        press(&mut c, "100");
        // 100 kcal is 0.95 bananas; the comparison is the last result.
        assert_eq!(shown(&c).last().unwrap(), "0.95 bananas");
    }

    /// Large whole values show the digits of Windows' `DecimalFormatter` (checked on
    /// Windows 11), not every digit of the double.
    #[test]
    fn large_suggestions_show_seventeen_digits_at_most() {
        assert_eq!(round_suggestion(365249999999999616.0), "365249999999999620");
        assert_eq!(round_suggestion(999999999999998976.0), "999999999999999000");
        assert_eq!(
            round_suggestion(-999999999999998976.0),
            "-999999999999999000"
        );
        // 17 digits even where 16 would give the same double.
        assert_eq!(
            round_suggestion(1152921504606851072.0),
            "1152921504606851100"
        );
        assert_eq!(round_suggestion(72057594037927936.0), "72057594037927936");
        assert_eq!(round_suggestion(8e24), "8000000000000000000000000");

        let value = |c: &Converter, abbreviation: &str| {
            c.suggestions()
                .into_iter()
                .find(|s| s.abbreviation == abbreviation)
                .map(|s| s.value)
        };
        let mut c = converter(Category::Time, "Years", "Weeks");
        press(&mut c, "999999999999999");
        assert_eq!(value(&c, "d").as_deref(), Some("365,249,999,999,999,620"));
        let mut c = converter(Category::Data, "Yottabytes", "Bytes");
        press(&mut c, "999999999999999");
        assert_eq!(value(&c, "ZB").as_deref(), Some("999,999,999,999,999,000"));
    }

    #[test]
    fn formatting_helpers() {
        assert_eq!(trim_trailing_zeros("1.500"), "1.5");
        assert_eq!(trim_trailing_zeros("2.000"), "2");
        assert_eq!(trim_trailing_zeros("100"), "100");
        assert_eq!(number_digits("-12.50"), 3);
        assert_eq!(whole_number_digits(0.5), 1);
        assert_eq!(whole_number_digits(999.0), 3);
        assert_eq!(whole_number_digits(1000.0), 4);
        assert_eq!(localize("-05"), "-5");
        assert_eq!(localize("-0"), "-0");
        assert_eq!(localize("1234567.890"), "1,234,567.890");
    }
}
