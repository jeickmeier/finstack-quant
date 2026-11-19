//! Tests for SimpleContext.
//!
//! This module tests SimpleContext:
//! - Basic column lookup
//! - Missing columns
//! - Edge cases (empty, single column, duplicates)
//! - Special characters and unicode names

use finstack_quant_core::expr::SimpleContext;

#[test]
fn simple_context_basic_usage() {
    let ctx = SimpleContext::new(["price", "volume", "timestamp"]).expect("unique columns");

    assert_eq!(ctx.get_index("price"), Some(0));
    assert_eq!(ctx.get_index("volume"), Some(1));
    assert_eq!(ctx.get_index("timestamp"), Some(2));
}

#[test]
fn simple_context_missing_column() {
    let ctx = SimpleContext::new(["price", "volume"]).expect("unique columns");

    assert_eq!(ctx.get_index("unknown"), None);
    assert_eq!(ctx.get_index(""), None);
}

#[test]
fn simple_context_empty() {
    let empty: Vec<&str> = vec![];
    let ctx = SimpleContext::new(empty).expect("unique columns");

    assert_eq!(ctx.get_index("anything"), None);
}

#[test]
fn simple_context_single_column() {
    let ctx = SimpleContext::new(["value"]).expect("unique columns");

    assert_eq!(ctx.get_index("value"), Some(0));
    assert_eq!(ctx.get_index("other"), None);
}

#[test]
fn simple_context_duplicate_names() {
    let result = SimpleContext::new(["col", "col"]);
    assert!(result.is_err(), "duplicate column names must be rejected");
}

#[test]
fn simple_context_case_sensitive() {
    let ctx = SimpleContext::new(["Price", "price", "PRICE"]).expect("unique columns");

    // Should be case-sensitive
    assert!(ctx.get_index("Price").is_some());
    assert!(ctx.get_index("price").is_some());
    assert!(ctx.get_index("PRICE").is_some());
    assert_eq!(ctx.get_index("pRiCe"), None);
}

#[test]
fn simple_context_special_characters() {
    let ctx = SimpleContext::new(["col_1", "col-2", "col.3", "col$4"]).expect("unique columns");

    assert_eq!(ctx.get_index("col_1"), Some(0));
    assert_eq!(ctx.get_index("col-2"), Some(1));
    assert_eq!(ctx.get_index("col.3"), Some(2));
    assert_eq!(ctx.get_index("col$4"), Some(3));
}

#[test]
fn simple_context_numeric_names() {
    let ctx = SimpleContext::new(["123", "456", "789"]).expect("unique columns");

    assert_eq!(ctx.get_index("123"), Some(0));
    assert_eq!(ctx.get_index("456"), Some(1));
    assert_eq!(ctx.get_index("789"), Some(2));
}

#[test]
fn simple_context_whitespace_names() {
    let ctx =
        SimpleContext::new(["col with spaces", "  leading", "trailing  "]).expect("unique columns");

    // Exact match required
    assert_eq!(ctx.get_index("col with spaces"), Some(0));
    assert_eq!(ctx.get_index("  leading"), Some(1));
    assert_eq!(ctx.get_index("trailing  "), Some(2));
    assert_eq!(ctx.get_index("col with spaces "), None); // Extra space
}

#[test]
fn simple_context_resolve_index_method() {
    let ctx = SimpleContext::new(["a", "b", "c"]).expect("unique columns");

    assert_eq!(ctx.get_index("a"), Some(0));
    assert_eq!(ctx.get_index("b"), Some(1));
    assert_eq!(ctx.get_index("c"), Some(2));
    assert_eq!(ctx.get_index("d"), None);
}

#[test]
fn simple_context_from_vec_string() {
    let columns = vec!["col1".to_string(), "col2".to_string(), "col3".to_string()];
    let ctx = SimpleContext::new(columns).expect("unique columns");

    assert_eq!(ctx.get_index("col1"), Some(0));
    assert_eq!(ctx.get_index("col2"), Some(1));
    assert_eq!(ctx.get_index("col3"), Some(2));
}

#[test]
fn simple_context_from_iterator() {
    let columns = ["x", "y", "z"];
    let ctx = SimpleContext::new(columns.iter().copied()).expect("unique columns");

    assert_eq!(ctx.get_index("x"), Some(0));
    assert_eq!(ctx.get_index("y"), Some(1));
    assert_eq!(ctx.get_index("z"), Some(2));
}

#[test]
fn simple_context_many_columns() {
    let columns: Vec<String> = (0..100).map(|i| format!("col_{}", i)).collect();
    let ctx = SimpleContext::new(columns.clone()).expect("unique columns");

    // Verify all columns are indexed
    for (i, col) in columns.iter().enumerate() {
        assert_eq!(ctx.get_index(col), Some(i));
    }
}

#[test]
fn simple_context_empty_string_column() {
    let ctx = SimpleContext::new([""]).expect("unique columns");

    assert_eq!(ctx.get_index(""), Some(0));
}

#[test]
fn simple_context_unicode_names() {
    let ctx = SimpleContext::new(["價格", "数量", "タイムスタンプ"]).expect("unique columns");

    assert_eq!(ctx.get_index("價格"), Some(0));
    assert_eq!(ctx.get_index("数量"), Some(1));
    assert_eq!(ctx.get_index("タイムスタンプ"), Some(2));
}
