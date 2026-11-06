# PORTFOLIO-EXECUTION-002 — Book graph validation and monetary rollups

- **Decision status:** Draft — observed implementation; adoption of the specific methodology remains pending. Existing accepted architectural and arithmetic requirements continue to apply.
- **Implementation status:** Observed for the source paths identified below; discrepancies and limits are stated explicitly.
- **Validation status:** Source and selected test evidence inspected or located; tests, builds and numerical benchmarks were not executed.
- **Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact source hashes are in [coverage.json](coverage.json).

## Choice and rationale

Books form a parent/child forest with each position assigned to at most one direct book. Ancestor totals include descendant positions. Reciprocal membership validation prevents implicit double counting in a valid portfolio; memoized rollup avoids recalculating shared lookup work. These are operational aggregation choices, not asset-allocation weights.

## Scope and algorithm

The shared validator accepts at most 100,000 books and hierarchy depth 512. It checks map key versus book ID, existing parents/children/positions, duplicate direct position membership, duplicate child listings, and parent/child reciprocity. It then traverses parent chains iteratively with cached depths and a visiting set to detect cycles and excessive depth. Native validation reports its first issue; materialization collects bounded structured diagnostics. Position `book_id` must agree with the derived membership map in native portfolios. Materialization instead derives each position's book from the validated book lists.

The builder fills parent/child links regardless of parent insertion order, and appends missing memberships implied by position `book_id`. It does not delete conflicting explicit memberships; final validation rejects them.

`aggregate_by_book` creates a position-ID-to-base-Money lookup, then recursively calculates each book in input map order:

`book_total = sum(direct position base values) + sum(child book totals)`.

The reducer uses currency-checked `Money::checked_add`, memoizes completed totals, and rejects a cycle, missing book, missing valued position or recursion depth `>=512` when starting the root at depth zero. Currency mismatch against the supplied base currency also errors. Empty books return zero in that currency; unassigned positions are outside book totals.

## Selection, limitations and alternatives

Rollup accepts an arbitrary book map and valuation, rather than a validated `Portfolio`. It guards cycles/missing references/depth but does not rerun the validator's unique-membership or reciprocal-edge checks. A manually supplied duplicate position or child listing can therefore be counted repeatedly. The intended safe path is the validated portfolio forest. Existing hierarchy validation is not proof that every public rollup caller supplies that forest.

The recursive implementation has a bounded depth; an iterative postorder reducer is an unimplemented alternative with the same economics. Fractional membership in several books would require explicit allocation weights and a different contract, not relaxation of uniqueness checks. A direct book subtotal and an ancestor subtotal intentionally overlap, so summing every returned book total does not produce portfolio NAV.

## Evidence

[book.rs](../../../../finstack-quant/portfolio/src/book.rs), [builder.rs](../../../../finstack-quant/portfolio/src/builder.rs), [portfolio.rs](../../../../finstack-quant/portfolio/src/portfolio.rs) and [grouping.rs](../../../../finstack-quant/portfolio/src/grouping.rs) own the behavior. Inline native tests cover dangling/nonreciprocal references and replacement membership. [Book hierarchy tests](../../../../finstack-quant/portfolio/tests/book_hierarchy_test.rs) locate three-level, multiple-root, unassigned and insertion-order cases; [aggregation/grouping tests](../../../../finstack-quant/portfolio/tests/aggregation_grouping_and_df.rs) locate value reconciliation. These are structural and arithmetic evidence, not independent product-pricing validation.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#portfolio-execution). This record does not
authorize a runtime change or certify the observed method.
