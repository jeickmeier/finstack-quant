# FEATURES-001: Panel ordering, timestamp keys and partition membership

**Decision status:** Accepted 2026-11-17.

**Implementation status:** Row ordering, alignment, and partitioning are implemented. Checked timestamp conversion and consistent native key-column admission require reconciliation.

**Validation status:** Independent arithmetic checked a tied-key rolling example. Rust fixtures were inspected. Timestamp boundaries and host behavior still require qualification.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See [scope and source ledger](README.md).

## Accepted decision

1. Keep each row as a separate observation. Within each entity, sort by exact order key and original input position, and return results in input order. Trailing windows include the current row and count missing rows toward membership. Each operation defines observation eligibility. Values 8, 2, and 4 at keys 02, 01, and 01 give two-row rolling means of 6, missing, and 3 in input order.
2. Partition cross-sections by exact time key or exact time-key and group tuple. Repeated entities remain separate rows and contribute separately to the statistic.
3. Treat caller-supplied strings as opaque lexical keys. When converting native values, require a single representation per key column. Normalize aware timestamps to UTC, preserve naive wall time, and format nine fractional digits. Check that timestamp years after applicable normalization are in 0001–9999. Reject offsets that cannot be represented exactly.

## Observed scope and actual selection

Every time-series path, including pairwise statistics and rolling regression, calls `sorted_indices`, then visits contiguous entity runs. Cross-sectional operations and neutralization partition by exact `time_key`; grouped cross-sectional operations partition by the actual `(time_key, groups)` tuple. No calendar, exchange session, currency, entity normalization or duplicate-row aggregation is supplied.

## Ordered algorithm and conventions

1. Entry points check all required row counts before indexing. Sort the indices by entity string, order string, then original row index. Duplicate entity/order pairs remain separate observations in input-index order.
2. Operate on one entity run at a time, placing each result back at its original index. Output length and row alignment are preserved; sorting does not reorder the returned array.
3. A trailing window at position `p` starts at `p.saturating_sub(window-1)` and includes `p`. Windows count rows, including missing rows; each algorithm separately counts eligible finite observations. There is no date-based duration or regular-spacing check.
4. Cross-sections use a `BTreeMap`, so key visitation is lexical and rows inside a partition retain input order. Pair grouping avoids separator-concatenation collisions. Duplicate entities within a cross-section are separate, equally represented rows.
5. `naive_datetime_order_key` formats wall-clock components as `YYYY-MM-DDTHH:MM:SS.fffffffff`; `datetime_order_key` first converts to UTC and appends `+00:00`. No timezone is assumed for naive values. Rust accepts strings unchanged; it does not detect mixed timestamp spellings, awareness or precision.

The year formatter is `{:04}` and accepts the `time` datetime's year without a four-digit/nonnegative restriction. Fixed-width nonnegative four-digit years support the stated lexical chronology; negative years and any enabled expanded-year range require qualification. The broad chronology claim in the formatter documentation lacks this domain restriction. Offset conversion at the supported datetime boundaries also has no additional feature-layer check.

## Rationale, alternatives and limitations

Documented rationale is stable lexical ordering and host-shared timestamp spelling. Proposed rationale for row windows is reproducible panel alignment, but adopting it also accepts unequal calendar intervals and input-order sensitivity at duplicate timestamps. Calendar-duration windows, duplicate rejection or prior aggregation are distinct alternatives, not implemented options. Rust's typed timestamps do not make arbitrary key strings chronological.

## Evidence

- Owning [index helpers](../../../../finstack-quant/features/src/index.rs): `sorted_indices`, `try_for_each_entity`, `try_for_each_trailing_window`, `partition_by_key`, `partition_by_pair`; [timestamp formatting](../../../../finstack-quant/features/src/keys.rs): both public key functions.
- Inspected [transform fixtures](../../../../finstack-quant/features/tests/transforms.rs): `transform_timeseries_aligns_unsorted_returns_lag_and_rolling_std` checks restored row positions; `grouped_cross_sectional_transform_does_not_collide_composite_keys` checks tuple separation. Inline key tests cover fixed nanos and equal UTC instants, only for ordinary positive years.
- Inspected [audit regressions](../../../../finstack-quant/features/tests/audit_regressions.rs): `rolling_windows_and_slope_preserve_row_gaps` establishes row-window and finite-lag distinctions. These are authored behavioral examples, not independent calendar validation.
- Selected [Python key conversion](../../../../finstack-quant-py/src/bindings/features/mod.rs), `extract_keys`, rejects mixed aware/naive datetime objects and delegates formatting; this is caller evidence, not a complete binding audit.

No runtime boundary case was executed.

## Follow-up tracking

Pending choices, implementation reconciliation, and qualification work are tracked
in the [feature follow-ups](../FOLLOW_UPS.md#features).
