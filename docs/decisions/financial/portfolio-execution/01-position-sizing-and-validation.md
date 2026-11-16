# PORTFOLIO-EXECUTION-001 — Holding identity, signed sizing and validation boundaries

- **Decision status:** Accepted 2026-11-15 — complete holding identity, signed sizing, atomic validation and persistence package.
- **Implementation status:** Observed. Reconciliation to the accepted package is pending.
- **Validation status:** Independent signed sizing, exact Money and attribute comparison examples verified. Runtime construction, valuation, restoration and host qualification remain pending.
- **Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; exact source hashes are in [coverage.json](coverage.json).

## Accepted decision for checked portfolio holdings and signed sizing

1. **Give each ordered holding a unique exact identity.** Require a
   nonblank `PositionId` unique within the portfolio. Derive the
   instrument ID from the instrument and validate any repeated wire ID
   against the decoded definition. Admit several holdings of the same
   instrument. Pricing reuse depends on the admitted immutable
   instrument state and effective pricing inputs.
   [Shared executor and reuse](../../architecture/0046-portfolio-execution-operational-contract.md#decision-and-scope).

2. **Keep two explicit sizing inputs.** `Units` is a signed
   multiplier of the instrument as built. `Percentage` accepts
   signed percentage points in `[-100,100]` and divides by 100
   once. Admit zero and shorts, normalize signed zero and reject
   nonfinite or unrepresentable scales. Retire `FaceValue`,
   `Notional`, the optional notional currency tag and the arbitrary
   `10¹⁵` warning. Actual face or notional sizing requires a
   product-qualified conversion to units.

3. **Apply the signed scale once to each additive amount.** Use
   checked Decimal multiplication for Money in its own currency,
   with the percentage conversion computed in Decimal. Use checked
   binary64 arithmetic for raw additive sensitivities. Scale PV,
   P&L, settlement cash and additive risk. Yields and other
   nonadditive measures retain their product basis. Gross exposure
   and margin retain their owners' rules.
   [Money arithmetic](../core-conventions/012-money-decimal-precision.md).

4. **Require explicit portfolio and ownership inputs.** Require a
   base currency, as-of date, exact entity references and valid
   optional book references. Entity and book map keys agree with
   their embedded IDs. Reject duplicate definitions. An explicitly
   selected `_standalone` resolves to the canonical synthetic
   entity. Book hierarchy, product lifecycle and FX resolution
   stay with their owners.
   [Book hierarchy record 002](02-book-hierarchy-and-rollups.md).

5. **Use one atomic Rust admission path.** Apply the same holding
   and reference checks in builders, native edits, inline restore
   and bulk materialization. Admit immutable holdings and publish
   checked replacements with reciprocal memberships, indices and a
   new state identity. A failed edit preserves the opening state.
   [Materialization record 006](06-materialization-validation.md).

6. **Compare attributes exactly.** Require nonblank keys and
   finite numeric values and thresholds. Use literal case-sensitive
   text equality and exact numeric comparisons. Missing keys,
   mixed types and ordering on text produce no match. Retire the
   implicit equality tolerance.

7. **Share admission across hosts and persistence.** Require explicit
   sizing and entity selection in Rust, Python and WASM. Inline
   restorable export requires complete canonical instrument
   definitions. Admitted in-memory custom instruments may participate
   in valuation and report a typed error if export is unsupported.
   Bulk artifact layout and caching stay with their owners.
   [Bulk artifact layout](06-materialization-validation.md) ·
   [Artifact caching](07-artifact-cache-and-load-reporting.md).

## Independent calculation checks

- Two holdings of `+2` and `−0.5` in one instrument with a USD
  `10` mark produce distinct rows of USD `20` and USD `−5`.
- A `−50%` holding scales a USD `100` mark to USD `−50`
  and a EUR `−7` payment to EUR `+3.50`. Scaling USD
  `9007199254740993` by one preserves the exact amount in Decimal.
  The binary64 detour gives `9007199254740992`.
- For an attribute of `99.99999995`, the old tolerance allows both
  equality with `100` and less than `100`. The exact comparison
  admits only the less-than predicate.

## Observed choice and rationale at the audit revision

A position holds an already-sized instrument and a signed multiplier. This avoids rebuilding a deal for each holding. `Units`, `FaceValue` and `Notional` all multiply the instrument value by `quantity`; `Percentage` divides percentage points by 100. The name `FaceValue` does not introduce division by the instrument face, and `Notional(Some(currency))` does not convert a monetary notional into lots. These are observed contracts, not proposed reinterpretations of quantity.

## Scope and input domain

`Position::new` requires finite quantity and limits percentage positions to `[-100,100]`. Zero and short quantities are supported. Absolute quantity above `1e15` emits a warning but is not rejected for the other units. Positions retain a reference ID separately from `instrument.id()`. Instrument objects behind `Arc` are contractually immutable once inserted; the library does not fingerprint interior state at every valuation.

## Algorithm and selection

1. Construct the position after quantity validation; initialize book, attributes and metadata empty.
2. Compute `s = quantity` or `quantity/100`. At the audit revision, `scale_value` formed `Money::new(value.amount() * s, value.currency())`. The current path uses `Money::from_model_value` after the same binary64 product. Regular valuation warns when a notional currency tag disagrees with the native PV currency and still scales; [margin VM](17-margin-collateral-and-wire.md) treats this mismatch as an error for that position.
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

Implementation reconciliation and production qualification are tracked
in the [scope follow-ups](../FOLLOW_UPS.md#portfolio-execution).
