//! IR Future Position enum tests.

use finstack_quant_valuations::instruments::rates::ir_future::Position;
use std::str::FromStr;

#[test]
fn test_position_display() {
    assert_eq!(Position::Long.to_string(), "long");
    assert_eq!(Position::Short.to_string(), "short");
}

#[test]
fn test_position_from_str() {
    assert_eq!(Position::from_str("long").unwrap(), Position::Long);
    assert_eq!(Position::from_str("short").unwrap(), Position::Short);
    assert_eq!(Position::from_str("buy").unwrap(), Position::Long);
    assert_eq!(Position::from_str("buyer").unwrap(), Position::Long);
    assert_eq!(Position::from_str("sell").unwrap(), Position::Short);
    assert_eq!(Position::from_str("seller").unwrap(), Position::Short);
    assert_eq!(Position::from_str("Long").unwrap(), Position::Long);
    assert_eq!(Position::from_str("SHORT").unwrap(), Position::Short);
    assert_eq!(Position::from_str("LONG").unwrap(), Position::Long);
}

#[test]
fn test_position_from_str_mixed_case() {
    assert_eq!(Position::from_str("LoNg").unwrap(), Position::Long);
    assert_eq!(Position::from_str("ShOrT").unwrap(), Position::Short);
}

#[test]
fn test_position_from_str_invalid() {
    assert!(Position::from_str("flat").is_err());
    assert!(Position::from_str("invalid").is_err());
    assert!(Position::from_str("").is_err());
}

#[test]
fn test_position_equality() {
    assert_eq!(Position::Long, Position::Long);
    assert_eq!(Position::Short, Position::Short);
    assert_ne!(Position::Long, Position::Short);
}

#[test]
fn test_position_clone() {
    let pos = Position::Long;
    let cloned = pos;
    assert_eq!(pos, cloned);
}

#[test]
fn test_position_serde() {
    let long = Position::Long;
    let serialized = serde_json::to_string(&long).unwrap();
    let deserialized: Position = serde_json::from_str(&serialized).unwrap();
    assert_eq!(long, deserialized);

    let short = Position::Short;
    let serialized = serde_json::to_string(&short).unwrap();
    let deserialized: Position = serde_json::from_str(&serialized).unwrap();
    assert_eq!(short, deserialized);
}
