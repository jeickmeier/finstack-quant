# PORTFOLIO-EXECUTION-002 — Book graph validation and monetary rollups

- **Decision status:** Accepted 2026-11-15 — complete book assignment, bounded graph, matched valuation and checked rollup package.
- **Implementation status:** Observed. Reconciliation to the accepted package is pending.
- **Validation status:** Independent depth, signed subtotal and Money reduction-order examples verified. Production rollup, restoration and host qualification remain pending.
- **Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact source hashes are in [coverage.json](coverage.json).

## Accepted decision for checked book hierarchy and NAV rollups

1. **Use one authority for each book and holding assignment.**
   Books form an optional rooted forest. Each book has at most one
   parent and each holding at most one direct book. Interior books
   may hold positions and unassigned holdings are valid. Native
   authoring uses `Book.parent_id` and `Position.book_id`. Derive
   child and membership lists from the admitted assignments. Bulk
   materialization validates its supplied representation and
   resolves the same graph. Conflicting reciprocal claims fail.
   [Materialization record 006](06-materialization-validation.md).

2. **Validate the entire forest before reduction or publication.**
   Reject blank and duplicate IDs, mismatched embedded IDs, missing
   references, repeated memberships, self-parenting and cycles.
   Keep inclusive limits of 100,000 books and 512 levels, with
   roots at level one. Validate iteratively, independently of
   insertion order and any subtotal cache. Publish the graph,
   memberships, indices and new portfolio state together after
   a checked edit.
   [Accepted holding edits](01-position-sizing-and-validation.md#accepted-decision-for-checked-portfolio-holdings-and-signed-sizing).

3. **Bind rollup to a qualified valuation of the same portfolio.**
   Derive base currency from the admitted portfolio and require
   matching immutable portfolio state, effective date and reporting
   basis. Require exactly one qualified base-Money row per holding,
   including unassigned holdings. Reject stale, missing, extra or
   inconsistent rows. Risk-only omissions are admissible with
   qualified PV. A restored valuation requires verified state
   provenance. Use the already-sized and already-converted PVs.
   [Shared execution and reuse](../../architecture/0046-portfolio-execution-operational-contract.md#decision-and-scope) ·
   [Valuation admission record 003](03-valuation-profiles-and-failures.md).

4. **Use a fixed checked Money reduction.** Complete children
   before their parent with iterative traversal. Sum direct
   holdings in portfolio order, add child subtotals in exact
   UTF-8 book-ID order, and emit books in that ID order. An
   empty book returns base-currency zero. An arithmetic error
   aborts the rollup.
   [Money arithmetic](../core-conventions/012-money-decimal-precision.md).

5. **Report subtree totals and unassigned value separately.**
   Each book total covers its direct holdings and all descendants.
   Roots cover all assigned holdings once. Ancestors overlap
   descendants. Reconciliation to published portfolio NAV uses
   the reduction contract owned by record 004.
   [Portfolio reduction record 004](04-execution-order-and-reduction.md).

6. **Share graph admission across hosts.** Python and WASM
   construction and restoration use the same Rust graph checks.
   Record 006 owns bulk wire layout. Keep the book rollup on
   its existing Rust-only surface.
   [Host parity ledger](../../../../finstack-quant-py/parity_contract.toml).

## Independent structural and arithmetic checks

- A model of the observed cache-first path admits a 513-level
  chain when descendants are cached leaf-first and rejects it
  root-first. Whole-forest admission rejects it in either order.
- A root has USD 100 directly and a child worth USD `−30`.
  It reports USD 70. A second root of USD 25 and unassigned
  USD 5 bring covered value to USD 100. Adding every book gives
  USD 65 because the child appears twice.
- USD `9007199254740993` and USD `−9007199254740992` sum
  to USD 1 in Decimal and to zero after binary64 projection.
  Checked Money order also matters where sub-minor-unit
  representation rounding is admitted.

## Observed choice and rationale at the audit revision

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

Implementation reconciliation and production qualification are tracked
in the [scope follow-ups](../FOLLOW_UPS.md#portfolio-execution).
