//! Book hierarchy for portfolio organization.
//!
//! Books provide an optional hierarchical organization structure for portfolios,
//! allowing positions to be grouped into folders/books with parent-child relationships.
//! This enables multi-level aggregation and reporting (e.g., Americas > Credit > IG).

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::types::PositionId;

/// Maximum supported number of organizational books in a portfolio.
pub(crate) const MAX_BOOKS: usize = 100_000;
/// Maximum supported ancestry depth, including the root book.
pub(crate) const MAX_BOOK_DEPTH: usize = 512;

/// Validate the canonical book forest and return its position memberships.
///
/// Both native construction and materialization use this walk so their
/// membership, referential-integrity, and resource limits cannot diverge.
pub(crate) fn validate_books<'a>(
    books: &'a IndexMap<BookId, Book>,
    positions: &finstack_quant_core::HashSet<&PositionId>,
    mut emit: impl FnMut(finstack_quant_core::contract::Diagnostic),
) -> finstack_quant_core::HashMap<&'a PositionId, &'a BookId> {
    use finstack_quant_core::contract::{Diagnostic, LoadPhase, Severity};
    use finstack_quant_core::{HashMap, HashSet};

    let issue = |code, message, pointer| {
        Diagnostic::new(code, LoadPhase::Semantic, Severity::Error, message).with_pointer(pointer)
    };
    let mut memberships = HashMap::default();
    if books.len() > MAX_BOOKS {
        emit(issue(
            "portfolio/book-limit",
            format!(
                "Portfolio has {} books; maximum is {MAX_BOOKS}",
                books.len()
            ),
            "/portfolio/books".to_string(),
        ));
        return memberships;
    }
    let mut child_parents = HashMap::default();
    for (book_id, book) in books {
        if book_id != &book.id {
            emit(issue(
                "portfolio/book-id-mismatch",
                format!(
                    "book map key '{book_id}' does not match embedded id '{}'",
                    book.id
                ),
                format!("/portfolio/books/{book_id}/id"),
            ));
        }
        if let Some(parent_id) = &book.parent_id {
            if !books.contains_key(parent_id) {
                emit(issue(
                    "portfolio/book-missing-parent",
                    format!("book '{book_id}' references missing parent '{parent_id}'"),
                    format!("/portfolio/books/{book_id}/parent_id"),
                ));
            }
        }
        for (index, position_id) in book.position_ids.iter().enumerate() {
            let pointer = format!("/portfolio/books/{book_id}/position_ids/{index}");
            if !positions.contains(position_id) {
                emit(
                    issue(
                        "portfolio/book-missing-position",
                        format!(
                            "book '{book_id}' references non-existent position '{position_id}'"
                        ),
                        pointer.clone(),
                    )
                    .with_position_id(position_id.to_string()),
                );
            }
            if let Some(first_book) = memberships.insert(position_id, book_id) {
                emit(issue(
                    "portfolio/position-multiple-books",
                    format!("position '{position_id}' is assigned more than once, in books '{first_book}' and '{book_id}'"),
                    pointer,
                ).with_position_id(position_id.to_string()));
            }
        }
        for (index, child_id) in book.child_book_ids.iter().enumerate() {
            let pointer = format!("/portfolio/books/{book_id}/child_book_ids/{index}");
            let Some(child) = books.get(child_id) else {
                emit(issue(
                    "portfolio/book-missing-child",
                    format!("book '{book_id}' references non-existent child book '{child_id}'"),
                    pointer,
                ));
                continue;
            };
            if let Some(first_parent) = child_parents.insert(child_id, book_id) {
                emit(issue(
                    "portfolio/book-multiple-parents",
                    format!("book '{child_id}' is listed more than once by parents '{first_parent}' and '{book_id}'"),
                    pointer.clone(),
                ));
            }
            if child.parent_id.as_ref() != Some(book_id) {
                emit(issue(
                    "portfolio/book-parent-child-mismatch",
                    format!(
                        "book '{book_id}' lists '{child_id}' as child, but its parent is {:?}",
                        child.parent_id
                    ),
                    pointer,
                ));
            }
        }
    }
    for (book_id, book) in books {
        if let Some(parent_id) = &book.parent_id {
            if books.contains_key(parent_id)
                && child_parents.get(book_id).copied() != Some(parent_id)
            {
                emit(issue(
                    "portfolio/book-parent-child-mismatch",
                    format!("book '{book_id}' names parent '{parent_id}', but the parent does not list it as a child"),
                    format!("/portfolio/books/{book_id}/parent_id"),
                ));
            }
        }
    }

    // Completed paths cache their depth; each parent edge is traversed once.
    let mut depths: HashMap<&BookId, usize> = HashMap::default();
    let mut visiting = HashSet::default();
    for book_id in books.keys() {
        if depths.contains_key(book_id) {
            continue;
        }
        let mut path = Vec::new();
        let mut current = Some(book_id);
        let mut depth = 0;
        while let Some(id) = current {
            if let Some(cached) = depths.get(id) {
                depth = *cached;
                break;
            }
            if !visiting.insert(id) {
                emit(issue(
                    "portfolio/book-cycle",
                    format!("Cycle detected in book hierarchy at book '{id}'"),
                    format!("/portfolio/books/{id}/parent_id"),
                ));
                break;
            }
            path.push(id);
            current = books
                .get(id)
                .and_then(|book| book.parent_id.as_ref())
                .filter(|parent| books.contains_key(*parent));
        }
        for id in path.into_iter().rev() {
            depth += 1;
            depths.insert(id, depth);
            visiting.remove(id);
            if depth > MAX_BOOK_DEPTH {
                emit(issue(
                    "portfolio/book-depth-limit",
                    format!("Book '{id}' exceeds maximum hierarchy depth of {MAX_BOOK_DEPTH}"),
                    format!("/portfolio/books/{id}/parent_id"),
                ));
            }
        }
    }
    memberships
}

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
///   indefinitely. A portfolio supports at most 100,000 books and 512 levels
///   of ancestry, including the root.
/// - [`Book::child_book_ids`] drives rollup in [`crate::grouping::aggregate_by_book`];
///   [`crate::portfolio::Portfolio::validate`] checks parent/child consistency
///   between child lists and [`Book::parent_id`]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "json-schema", derive(schemars::JsonSchema))]
#[serde(deny_unknown_fields)]
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
    /// use finstack_quant_portfolio::book::Book;
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
    #[must_use]
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
