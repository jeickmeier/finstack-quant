# PORTFOLIO-EXECUTION-001 — Holding identity, signed sizing and validation boundaries

- **Decision status:** Draft — observed implementation; adoption of the specific methodology remains pending. Existing accepted architectural and arithmetic requirements continue to apply.
- **Implementation status:** Observed for the source paths identified below; discrepancies and limits are stated explicitly.
- **Validation status:** Source and selected test evidence inspected or located; tests, builds and numerical benchmarks were not executed.
- **Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact source hashes are in [coverage.json](coverage.json).

## Choice and rationale

A position holds an already-sized instrument and a signed multiplier. This avoids rebuilding a deal for each holding. `Units`, `FaceValue` and `Notional` all multiply the instrument value by `quantity`; `Percentage` divides percentage points by 100. The name `FaceValue` does not introduce division by the instrument face, and `Notional(Some(currency))` does not convert a monetary notional into lots. These are observed contracts, not proposed reinterpretations of quantity.

## Scope and input domain

`Position::new` requires finite quantity and limits percentage positions to `[-100,100]`. Zero and short quantities are supported. Absolute quantity above `1e15` emits a warning but is not rejected for the other units. Positions retain a reference ID separately from `instrument.id()`. Instrument objects behind `Arc` are contractually immutable once inserted; the library does not fingerprint interior state at every valuation.

## Algorithm and selection

1. Construct the position after quantity validation; initialize book, attributes and metadata empty.
2. Compute `s = quantity` or `quantity/100`. `scale_value` forms `Money::new(value.amount() * s, value.currency())`. Regular valuation warns when a notional currency tag disagrees with the native PV currency and still scales; [margin VM](17-margin-collateral-and-wire.md) treats this mismatch as an error for that position.
3. The portfolio builder requires base currency and as-of date, inserts entities/books by ID, and supplies `_standalone` when a position uses that reserved entity. Repeated entity or book IDs replace the prior map entry. Repeated position IDs are rejected at build.
4. `Portfolio::from_spec` reconstructs each `InstrumentJson`, calls `Position::new`, rebuilds indices, then validates portfolio structure. A missing serializable instrument payload is fatal; `to_spec` can nevertheless emit `instrument_spec: None` for an unsupported runtime instrument.
5. `add_position` checks duplicate position ID and a referenced book, updates reciprocal book membership and both indices, and assigns a fresh state ID. `set_positions` checks IDs/books before mutation, rebuilds book memberships and indices, then assigns a fresh state ID. Neither operation performs the complete entity/instrument validation of a load boundary.

Attribute filters compare text exactly. Numeric equality uses `|a-b| <= max(1e-12, 1e-9 max(|a|,|b|))`; inequality uses the strict complement expression. Numeric ordering uses ordinary comparisons. Missing keys, text ordering and mixed text/numeric types return false. Numeric attributes themselves have no finite-value constructor check, so NaN does not behave like a normal comparable number.

## Boundaries and alternatives

`Portfolio::validate` checks entity references, position uniqueness and [book structure](02-book-hierarchy-and-rollups.md). It does **not** revalidate quantity, require `position.instrument_id == position.instrument.id()`, or check entity map key against its embedded ID. Position fields can be edited before insertion, and entities/books remain public. The [materialization boundary](06-materialization-validation.md) checks more of these conditions. Ordinary valuation does not call full portfolio validation on each request. This is a concrete native-versus-materialized validation difference; numerical consequences from deliberately malformed native objects were not reproduced.

Alternative absolute-notional/face sizing would change economic meaning and requires a separate API decision; it is not an implemented variant. Revalidating native objects at mutation/load boundaries would strengthen the current invariant while preserving multiplier semantics. [Accepted arithmetic policy](../conventions/0020-numerical-representation-and-rounding.md) governs Decimal/f64 boundaries: current `Money::new` has no ingest-scale rounding, but projecting exact stored Decimal values through `amount()` still enters f64 arithmetic.

## Evidence

- Owning sources: [position.rs](../../../../finstack-quant/portfolio/src/position.rs), [portfolio.rs](../../../../finstack-quant/portfolio/src/portfolio.rs), [builder.rs](../../../../finstack-quant/portfolio/src/builder.rs), [types.rs](../../../../finstack-quant/portfolio/src/types.rs).
- Located constructor and percentage-bound tests in `position.rs`; inspected reciprocal-mutation tests in `portfolio.rs`. [Core/builder integration tests](../../../../finstack-quant/portfolio/tests/core_portfolio_and_builder.rs) cover duplicate position rejection; [serialization tests](../../../../finstack-quant/portfolio/tests/serialization.rs) cover reconstruction. These establish intended contracts and round trips, not financial market standards.
- Attribute comparison tests in `types.rs` cover text, numeric and mismatched types. These cases do not establish non-finite attribute or bypassed-native-validation behavior.

## Follow-up tracking

Implementation, adoption and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#portfolio-execution). This record does not
authorize a runtime change or certify the observed method.
