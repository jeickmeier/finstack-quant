//! Column name resolution context for expression evaluation.
//!
//! Provides [`SimpleContext`], the single concrete context type used by
//! expression evaluators to resolve column references to array indices.

use crate::collections::HashMap;

/// Column name → index context for expression evaluation.
///
/// This is the single concrete context type accepted by
/// [`CompiledExpr::eval`](crate::expr::CompiledExpr::eval).
/// Construct it from any ordered iterator of column names; the index of each
/// name in the iterator becomes its column index in the data arrays passed to
/// `eval`.
///
/// Deserialization is strict (`deny_unknown_fields`): unknown fields on
/// inbound payloads are rejected rather than silently ignored.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SimpleContext {
    /// Column name to index mapping for O(1) resolution.
    column_indices: HashMap<String, usize>,
}

impl SimpleContext {
    /// Construct from an iterator of column names.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Validation`](crate::Error::Validation) if any column
    /// name appears more than once.
    pub fn new(columns: impl IntoIterator<Item = impl Into<String>>) -> crate::Result<Self> {
        let mut column_indices = HashMap::default();
        for (idx, name) in columns.into_iter().enumerate() {
            let name = name.into();
            if column_indices.contains_key(&name) {
                return Err(crate::Error::Validation(format!(
                    "duplicate column name: {name}"
                )));
            }
            column_indices.insert(name, idx);
        }
        Ok(Self { column_indices })
    }
    /// Find the index of a column by name.
    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.column_indices.get(name).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_context_maps_columns_in_insertion_order() {
        let ctx = SimpleContext::new(["price", "volume", "flag"]).expect("unique columns");
        assert_eq!(ctx.index_of("price"), Some(0));
        assert_eq!(ctx.index_of("volume"), Some(1));
        assert_eq!(ctx.index_of("flag"), Some(2));
        assert_eq!(ctx.index_of("missing"), None);
    }

    #[test]
    fn simple_context_rejects_duplicate_column_names() {
        let err = SimpleContext::new(["a", "b", "a"]);
        assert!(err.is_err());
    }
}
