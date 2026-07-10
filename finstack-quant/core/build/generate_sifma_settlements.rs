//! Generate the compiled SIFMA settlement lookup from maintained CSV data.

use std::env;
use std::fs;
use std::io;
use std::path::PathBuf;

const SOURCE_FILE: &str = "data/sifma_settlements.csv";
const OUTPUT_FILE: &str = "sifma_settlements_generated.rs";

#[derive(Debug)]
struct SettlementRow {
    year: i32,
    month: u8,
    class_days: [Option<u8>; 4],
}

fn parse_day(value: &str, line_number: usize) -> io::Result<Option<u8>> {
    if value.is_empty() {
        return Ok(None);
    }
    let day = value.parse::<u8>().map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{SOURCE_FILE}:{line_number}: invalid settlement day '{value}': {error}"),
        )
    })?;
    if !(1..=31).contains(&day) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{SOURCE_FILE}:{line_number}: settlement day must be 1-31, got {day}"),
        ));
    }
    Ok(Some(day))
}

fn parse_rows(source: &str) -> io::Result<Vec<SettlementRow>> {
    let mut lines = source.lines();
    const HEADER: &str = "year,month,class_a,class_b,class_c,class_d";
    if lines.next() != Some(HEADER) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{SOURCE_FILE}: expected header '{HEADER}'"),
        ));
    }

    let mut rows = Vec::new();
    for (index, line) in lines.enumerate() {
        let line_number = index + 2;
        let fields: Vec<_> = line.split(',').collect();
        if fields.len() != 6 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{SOURCE_FILE}:{line_number}: expected six comma-separated fields"),
            ));
        }
        let year = fields[0].parse::<i32>().map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "{SOURCE_FILE}:{line_number}: invalid year '{}': {error}",
                    fields[0]
                ),
            )
        })?;
        let month = fields[1].parse::<u8>().map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "{SOURCE_FILE}:{line_number}: invalid month '{}': {error}",
                    fields[1]
                ),
            )
        })?;
        if !(1..=12).contains(&month) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{SOURCE_FILE}:{line_number}: month must be 1-12, got {month}"),
            ));
        }
        let class_days = [
            parse_day(fields[2], line_number)?,
            parse_day(fields[3], line_number)?,
            parse_day(fields[4], line_number)?,
            parse_day(fields[5], line_number)?,
        ];
        if class_days.iter().all(Option::is_none) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{SOURCE_FILE}:{line_number}: row must publish at least one class"),
            ));
        }
        rows.push(SettlementRow {
            year,
            month,
            class_days,
        });
    }
    rows.sort_by_key(|row| (row.year, row.month));
    if rows
        .windows(2)
        .any(|pair| (pair[0].year, pair[0].month) == (pair[1].year, pair[1].month))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{SOURCE_FILE}: duplicate year/month row"),
        ));
    }
    Ok(rows)
}

fn render_day(day: Option<u8>) -> String {
    day.map_or_else(|| "None".to_string(), |value| format!("Some({value})"))
}

pub(crate) fn generate() -> io::Result<()> {
    let manifest_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set"));
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
    let source = fs::read_to_string(manifest_dir.join(SOURCE_FILE))?;
    let rows = parse_rows(&source)?;

    let mut output = String::from(
        "// Auto-generated from data/sifma_settlements.csv - DO NOT EDIT\n\
         // Each row: (year, month, [class A, class B, class C, class D] days).\n\
         pub(crate) static SIFMA_SETTLEMENTS: &[(i32, u8, [Option<u8>; 4])] = &[\n",
    );
    for row in rows {
        let [a, b, c, d] = row.class_days;
        output.push_str(&format!(
            "    ({}, {}, [{}, {}, {}, {}]),\n",
            row.year,
            row.month,
            render_day(a),
            render_day(b),
            render_day(c),
            render_day(d),
        ));
    }
    output.push_str("];\n");
    fs::write(out_dir.join(OUTPUT_FILE), output)
}
