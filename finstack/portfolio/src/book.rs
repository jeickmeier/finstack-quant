//! Book hierarchy for portfolio organization.
//!
//! Books provide an optional hierarchical organization structure for portfolios,
//! allowing positions to be grouped into folders/books with parent-child relationships.
//! This enables multi-level aggregation and reporting (e.g., Americas > Credit > IG).

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::types::PositionId;

define_string_id! {
    /// Book identifier.
    pub struct BookId;
}

/// A book represents a folder-like organizational unit within a portfolio.
///
/// Books can contain positions and/or child books, forming a hierarchical tree.
/// This allows multi-level aggregation (e.g., Americas > Credit > Investment Grade).
///
/// # Design
///
/// - Flat position list is default; books are optional
/// - Parent-child relationships tracked via `parent_id` field
/// - Positions reference books via optional `book_id` field
/// - Positions without `book_id` are not in any book
/// - Book hierarchies are expected to be acyclic trees or forests; aggregation
///   helpers reject cycles and excessively deep nesting instead of recursing
///   indefinitely
/// - [`Book::child_book_ids`] drives rollup in [`crate::grouping::aggregate_by_book`];
///   [`crate::portfolio::Portfolio::validate`] checks parent/child consistency
///   between child lists and [`Book::parent_id`]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Book {
    /// Unique identifier for this book
    pub id: BookId,

    /// Human-readable name
    pub name: Option<String>,

    /// Parent book identifier (None for root books)
    pub parent_id: Option<BookId>,

    /// Position IDs directly assigned to this book (non-recursive)
    pub position_ids: Vec<PositionId>,

    /// Child book IDs (for hierarchical organization)
    pub child_book_ids: Vec<BookId>,

    /// Book-level tags for grouping and filtering
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub tags: IndexMap<String, String>,

    /// Additional metadata
    #[serde(default, skip_serializing_if = "IndexMap::is_empty")]
    pub meta: IndexMap<String, serde_json::Value>,
}

impl Book {
    /// Create a new book with no parent (root book).
    ///
    /// # Arguments
    ///
    /// * `id` - Unique book identifier.
    /// * `name` - Optional human-readable name.
    ///
    /// # Returns
    ///
    /// A root book with no parent, no child books, and no assigned positions.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use finstack_portfolio::book::Book;
    ///
    /// let book = Book::new("credit", Some("Credit".to_string()));
    /// assert!(book.is_root());
    /// assert_eq!(book.name.as_deref(), Some("Credit"));
    /// ```
    pub fn new(id: impl Into<BookId>, name: Option<String>) -> Self {
        Self {
            id: id.into(),
            name,
            parent_id: None,
            position_ids: Vec::new(),
            child_book_ids: Vec::new(),
            tags: IndexMap::new(),
            meta: IndexMap::new(),
        }
    }

    /// Set the parent book, returning self for chaining.
    ///
    /// # Arguments
    ///
    /// * `parent_id` - Parent book identifier.
    ///
    /// # Returns
    ///
    /// The updated book for fluent chaining.
    pub fn with_parent(mut self, parent_id: impl Into<BookId>) -> Self {
        self.parent_id = Some(parent_id.into());
        self
    }

    /// Check if this is a root book (no parent).
    ///
    /// # Returns
    ///
    /// `true` when the book has no parent reference.
    pub fn is_root(&self) -> bool {
        self.parent_id.is_none()
    }

    /// Check if this book contains a specific position.
    ///
    /// # Arguments
    ///
    /// * `position_id` - Position identifier to check.
    ///
    /// # Returns
    ///
    /// `true` if the position is directly assigned to this book.
    pub fn contains_position(&self, position_id: &PositionId) -> bool {
        self.position_ids.contains(position_id)
    }

    /// Check if this book contains a specific child book.
    ///
    /// # Arguments
    ///
    /// * `child_id` - Child book identifier to check.
    ///
    /// # Returns
    ///
    /// `true` if the child is directly listed under this book.
    pub fn contains_child(&self, child_id: &BookId) -> bool {
        self.child_book_ids.contains(child_id)
    }

    /// Add a position to this book.
    ///
    /// # Arguments
    ///
    /// * `position_id` - Position identifier to add.
    pub fn add_position(&mut self, position_id: PositionId) {
        if !self.contains_position(&position_id) {
            self.position_ids.push(position_id);
        }
    }

    /// Add a child book to this book.
    ///
    /// # Arguments
    ///
    /// * `child_id` - Child book identifier to add.
    pub fn add_child(&mut self, child_id: BookId) {
        if !self.contains_child(&child_id) {
            self.child_book_ids.push(child_id);
        }
    }

    /// Remove a position from this book.
    ///
    /// # Arguments
    ///
    /// * `position_id` - Position identifier to remove.
    pub fn remove_position(&mut self, position_id: &PositionId) {
        self.position_ids.retain(|id| id != position_id);
    }

    /// Remove a child book from this book.
    ///
    /// # Arguments
    ///
    /// * `child_id` - Child book identifier to remove.
    pub fn remove_child(&mut self, child_id: &BookId) {
        self.child_book_ids.retain(|id| id != child_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_book_id_creation() {
        let id = BookId::new("americas");
        assert_eq!(id.as_str(), "americas");
        assert_eq!(id.to_string(), "americas");
    }

    #[test]
    fn test_book_id_conversions() {
        let id1: BookId = "americas".into();
        let id2: BookId = "americas".to_string().into();
        assert_eq!(id1, id2);
        assert_eq!(id1, "americas");
    }

    #[test]
    fn test_book_creation_root() {
        let book = Book::new("americas", Some("Americas".to_string()));
        assert_eq!(book.id, BookId::new("americas"));
        assert_eq!(book.name, Some("Americas".to_string()));
        assert!(book.is_root());
        assert!(book.position_ids.is_empty());
        assert!(book.child_book_ids.is_empty());
    }

    #[test]
    fn test_book_creation_with_parent() {
        let book = Book::new("credit", Some("Credit".to_string())).with_parent("americas");
        assert_eq!(book.id, BookId::new("credit"));
        assert_eq!(book.parent_id, Some(BookId::new("americas")));
        assert!(!book.is_root());
    }

    #[test]
    fn test_book_add_position() {
        let mut book = Book::new("ig", Some("Investment Grade".to_string()));
        let pos_id = PositionId::new("pos1");

        book.add_position(pos_id.clone());
        assert!(book.contains_position(&pos_id));
        assert_eq!(book.position_ids.len(), 1);

        // Adding again should not duplicate
        book.add_position(pos_id);
        assert_eq!(book.position_ids.len(), 1);
    }

    #[test]
    fn test_book_add_child() {
        let mut book = Book::new("americas", Some("Americas".to_string()));
        let child_id = BookId::new("credit");

        book.add_child(child_id.clone());
        assert!(book.contains_child(&child_id));
        assert_eq!(book.child_book_ids.len(), 1);

        // Adding again should not duplicate
        book.add_child(child_id);
        assert_eq!(book.child_book_ids.len(), 1);
    }

    #[test]
    fn test_book_remove_position() {
        let mut book = Book::new("ig", Some("Investment Grade".to_string()));
        let pos_id = PositionId::new("pos1");

        book.add_position(pos_id.clone());
        assert!(book.contains_position(&pos_id));

        book.remove_position(&pos_id);
        assert!(!book.contains_position(&pos_id));
        assert!(book.position_ids.is_empty());
    }

    #[test]
    fn test_book_remove_child() {
        let mut book = Book::new("americas", Some("Americas".to_string()));
        let child_id = BookId::new("credit");

        book.add_child(child_id.clone());
        assert!(book.contains_child(&child_id));

        book.remove_child(&child_id);
        assert!(!book.contains_child(&child_id));
        assert!(book.child_book_ids.is_empty());
    }
}
