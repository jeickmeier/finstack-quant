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

    assert_eq!(ctx.index_of("price"), Some(0));
    assert_eq!(ctx.index_of("volume"), Some(1));
    assert_eq!(ctx.index_of("timestamp"), Some(2));
}

#[test]
fn simple_context_missing_column() {
    let ctx = SimpleContext::new(["price", "volume"]).expect("unique columns");

    assert_eq!(ctx.index_of("unknown"), None);
    assert_eq!(ctx.index_of(""), None);
}

#[test]
fn simple_context_empty() {
    let empty: Vec<&str> = vec![];
    let ctx = SimpleContext::new(empty).expect("unique columns");

    assert_eq!(ctx.index_of("anything"), None);
}

#[test]
fn simple_context_single_column() {
    let ctx = SimpleContext::new(["value"]).expect("unique columns");

    assert_eq!(ctx.index_of("value"), Some(0));
    assert_eq!(ctx.index_of("other"), None);
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
    assert!(ctx.index_of("Price").is_some());
    assert!(ctx.index_of("price").is_some());
    assert!(ctx.index_of("PRICE").is_some());
    assert_eq!(ctx.index_of("pRiCe"), None);
}

#[test]
fn simple_context_special_characters() {
    let ctx = SimpleContext::new(["col_1", "col-2", "col.3", "col$4"]).expect("unique columns");

    assert_eq!(ctx.index_of("col_1"), Some(0));
    assert_eq!(ctx.index_of("col-2"), Some(1));
    assert_eq!(ctx.index_of("col.3"), Some(2));
    assert_eq!(ctx.index_of("col$4"), Some(3));
}

#[test]
fn simple_context_numeric_names() {
    let ctx = SimpleContext::new(["123", "456", "789"]).expect("unique columns");

    assert_eq!(ctx.index_of("123"), Some(0));
    assert_eq!(ctx.index_of("456"), Some(1));
    assert_eq!(ctx.index_of("789"), Some(2));
}

#[test]
fn simple_context_whitespace_names() {
    let ctx =
        SimpleContext::new(["col with spaces", "  leading", "trailing  "]).expect("unique columns");

    // Exact match required
    assert_eq!(ctx.index_of("col with spaces"), Some(0));
    assert_eq!(ctx.index_of("  leading"), Some(1));
    assert_eq!(ctx.index_of("trailing  "), Some(2));
    assert_eq!(ctx.index_of("col with spaces "), None); // Extra space
}

#[test]
fn simple_context_resolve_index_method() {
    let ctx = SimpleContext::new(["a", "b", "c"]).expect("unique columns");

    assert_eq!(ctx.index_of("a"), Some(0));
    assert_eq!(ctx.index_of("b"), Some(1));
    assert_eq!(ctx.index_of("c"), Some(2));
    assert_eq!(ctx.index_of("d"), None);
}

#[test]
fn simple_context_from_vec_string() {
    let columns = vec!["col1".to_string(), "col2".to_string(), "col3".to_string()];
    let ctx = SimpleContext::new(columns).expect("unique columns");

    assert_eq!(ctx.index_of("col1"), Some(0));
    assert_eq!(ctx.index_of("col2"), Some(1));
    assert_eq!(ctx.index_of("col3"), Some(2));
}

#[test]
fn simple_context_from_iterator() {
    let columns = ["x", "y", "z"];
    let ctx = SimpleContext::new(columns.iter().copied()).expect("unique columns");

    assert_eq!(ctx.index_of("x"), Some(0));
    assert_eq!(ctx.index_of("y"), Some(1));
    assert_eq!(ctx.index_of("z"), Some(2));
}

#[test]
fn simple_context_many_columns() {
    let columns: Vec<String> = (0..100).map(|i| format!("col_{}", i)).collect();
    let ctx = SimpleContext::new(columns.clone()).expect("unique columns");

    // Verify all columns are indexed
    for (i, col) in columns.iter().enumerate() {
        assert_eq!(ctx.index_of(col), Some(i));
    }
}

#[test]
fn simple_context_empty_string_column() {
    let ctx = SimpleContext::new([""]).expect("unique columns");

    assert_eq!(ctx.index_of(""), Some(0));
}

#[test]
fn simple_context_unicode_names() {
    let ctx = SimpleContext::new(["價格", "数量", "タイムスタンプ"]).expect("unique columns");

    assert_eq!(ctx.index_of("價格"), Some(0));
    assert_eq!(ctx.index_of("数量"), Some(1));
    assert_eq!(ctx.index_of("タイムスタンプ"), Some(2));
}
