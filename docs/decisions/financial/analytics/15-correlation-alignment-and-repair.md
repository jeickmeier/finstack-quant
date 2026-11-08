# ANALYTICS-015: Common-sample Pearson correlation, provenance and numerical validation

**Decision status:** Accepted for the complete common-sample Pearson, availability, numerical-validation and result-provenance package below (2026-11-07).

**Implementation status:** Implemented in the analytics decision reconciliation dated 2026-11-08. The accepted sections below define the current contract.

**Validation status:** Scoped implementation checks and their limits are recorded in the [analytics inventory](README.md#implementation-evidence). A passing numerical fixture establishes its stated case; it does not certify a population estimator or every supported input.

**Audit provenance:** The original source audit was performed on 2026-11-05 at `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. Its historical discrepancies are superseded by the accepted contracts and this implementation. The [source ledger](coverage.json) retains source ownership and dated inspection fingerprints.

## Accepted decision: common-sample Pearson correlation

**Accepted:** 2026-11-07.

This package covers the complete analytics
sample-selection, Pearson estimator, availability, numerical normalization,
validation/repair permission and result-provenance contract. Record 01 keeps
general panel/window ownership, record 02 keeps investment-history ownership,
and core-numerics keeps its separate matrix-validator and explicit projection
algorithms.

### One common observation sample

Use the exact intersection of all loaded tickers' valid active return spans
and the selected analysis range, on the panel's ordered date grid. Every
coefficient, mean and dispersion uses those same `N` observed return-ending
dates with equal weights. Do not borrow independent pair history, truncate
arrays, fill/interpolate gaps or automatically change sample policy. Keep all
loaded tickers in their supplied order; do not silently drop a difficult
ticker or introduce a new selection/configuration mode here.

Remove the pairwise fallback. Valid panel observations are contiguous within
each ticker's span. If their common interval has fewer than two observations,
the ticker starting latest and the ticker ending earliest also have fewer
than two common observations. Thus a pairwise fallback cannot rescue a
successful multi-ticker matrix under this supported domain. A genuinely
pairwise estimator would be a separate statistical choice; it is not needed
to implement this complete-case contract.

Adding a ticker or changing the active range may shorten the common sample
and change every matrix entry. Report the actual shared sample, rather than
suggesting each pair used its longest available history. Irregular dates
remain actual observations; no regular spacing or observation frequency is
inferred. The benchmark selection does not select or reweight this matrix.

Validate the full supplied investment histories under record 02 before
selecting the common span. A valid terminal `-1` return participates normally
if included in that span. Do not accept invalid post-wipeout history or add
future zero returns. Original return provenance, matched measurement periods,
currency and fee conventions remain caller/panel responsibilities.

### Pearson definition and whole-matrix availability

For ticker `j`, center its common-sample return column and normalize its
Euclidean length:

```text
c_ij = r_ij - mean(r_j)
z_j = c_j / ||c_j||_2
rho_jk = z_j' * z_k
R = Z' * Z
```

This is equal-weight Pearson correlation, equivalently sample covariance
divided by the two sample SDs. The common `N - 1` factors cancel. It is not
uncentered cosine similarity, a rank statistic or correlation of cumulative
wealth. Correlation is dimensionless and unannualized; no cash, MAR,
confidence or frequency argument is required solely for this calculation.

| Requested common matrix | Accepted result |
| --- | --- |
| At least one ticker, `N >= 2`, every common-sample column genuinely varies | Return the complete finite matrix. |
| No shared history or `N < 2` | Explicit unavailable-history error for the matrix call; no empty/partial matrix or fabricated identity. |
| Any genuinely constant common-sample column | Explicit undefined-correlation error for the matrix call; no silent ticker removal, zero correlation or diagonal-one exception. |
| One ticker | Apply the same observation/variation checks; return `[[1]]` only after eligibility is established. |
| Duplicate/proportional or inverse columns, or more tickers than `N - 1` | Valid singular PSD matrix, subject to numerical validation; no positive-definiteness or full-rank requirement. |

With exactly two observations, varying columns have correlations `+1` or
`-1`; this is computability, not reliable evidence about population exposure.
A constant cash-like series has undefined correlation rather than correlation
zero. Genuine zero and negative correlations between varying columns remain
valid. Whole-matrix availability is this API's explicit atomic-result choice;
other analytics retain their own per-field/per-ticker availability rules.

### Stable computation, rounding and PSD validation

- Use the existing numerical infrastructure to compute adequately scaled,
  centered unit-length columns and a symmetric Gram matrix. Compute each
  off-diagonal once and mirror it; emit exact unit diagonals only for eligible
  columns. Centering must preserve small representable variation at high
  offsets, and norm/dot calculations must avoid avoidable overflow/underflow.
- Detect a constant column from genuine equality of the observations, not a
  variance floor or a squared intermediate that rounded to zero. Distinct
  tiny returns remain statistically eligible even when their original-unit
  variance is not representable. For example, two identical varying columns
  `[1e-200, 2e-200, 3e-200]` have correlation one.
- Do not multiply original-unit variances before taking a square root. The
  zero-denominator fallback or unconditional clamp cannot establish correct
  analytics outputs. Preserve
  a representable requested coefficient through adequate same-estimator
  reconstruction, or report a calculation error if unresolved.
- Permit only explicit endpoint roundoff normalization: after adequate
  evaluation, a finite coefficient outside `[-1, 1]` by at most `1e-12` may
  be set to its nearest endpoint. Do not snap an interior near-perfect
  correlation to `+1`/`-1`, clip a larger violation, erase small correlations
  or use clipping as a substitute for input/moment validation.
- Validate the actual final returned matrix after any permitted endpoint
  normalization: exact square shape, finite bounded entries, exact mirrored
  symmetry/unit diagonals and PSD under the core validator's declared
  numerical contract. Singular PSD is valid. If numerical validation fails,
  adequately reconstruct this same common-sample Pearson estimate, then
  fail explicitly if unresolved. Do not return the pre-cleanup validator's
  result as though it validated a subsequently changed matrix.

The exact common-sample matrix is PSD because it is a normalized Gram
matrix, with rank at most `min(ticker_count, N - 1)`. Floating computation may
produce tiny roundoff deviations; numerical PSD acceptance is not a demand
for a positive smallest eigenvalue. Keep shared PSD/factorization tolerances
in the [core numerical owner](../core-numerics/matrix-factorization.md), rather
than inventing a second analytics eigensolver or adopting every core policy
through this record. The scalar `1e-12` endpoint allowance is an explicit
analytics numerical choice requiring independent qualification.

### Remove automatic repair and the repair flag

Remove automatic Higham/nearest-correlation projection from this method,
along with its `repaired` boolean and repair dispatcher. Do not add a repair
switch, shrinkage, eigenvalue clipping, jitter or a positive-definite fallback.
A material invalidity in this common-sample estimate is a computation failure,
not permission to change empirical relationships until a matrix passes.

The core's explicit nearest-correlation operation remains separately owned.
Choosing to repair or regularize a matrix for a downstream use requires that
caller's explicit operation and interpretation. This package does not adopt
the core projection's 200-iteration/`1e-10` controls or guarantee suitability
for inversion, Cholesky methods requiring positive definiteness, portfolio
optimization or out-of-sample forecasting.

The result propagates through Rust, Python, WASM, schemas/declarations,
fixtures and correlation heatmap adapters without a repair stamp or
compatibility flag.

### Compact result provenance and restoration

Return one canonical `CorrelationResult` with:

| Field | Accepted meaning |
| --- | --- |
| `tickers` | Ordered unique ticker labels for both matrix axes, preserving panel order. |
| `matrix` | Complete `M x M` finite Pearson matrix in that order. |
| `n_obs` | Actual common return-observation count `N`, not price count or elapsed calendar days. |
| `start_date`, `end_date` | First and last included return-ending dates of that common sample; not the initial wealth date or wider active-panel bounds. |

All fields are required on success, with `M >= 1`, `N >= 2` and
`start_date < end_date`. The count and bounds summarize provenance without
duplicating the full observation grid. They do not reconstruct irregular
intermediate dates or certify return-source identity; reproducibility still
requires the source panel. No redundant sample-policy enum, repair status,
pair-count matrix or generic diagnostics carrier is needed for this one
estimator.

Keep Rust canonical and propagate the same result semantics through Python,
WASM and JSON, retaining existing date/number conversion conventions. Validate
construction/serialization and restoration: a nonempty ordered collection of
unique ticker labels under the panel's identity convention, matching square
dimensions, finite `[-1, 1]` coefficients, exact symmetry/
unit diagonal, numerical PSD, integer sample count and chronological date
bounds. Since the grid has one observation per unique `Date`, require
`2 <= n_obs <= (end_date - start_date in calendar days) + 1`; this is a
cardinality bound, not an inference of regular sampling. Keep the mathematical
sample-rank relation in estimator qualification rather than adding a second
numerical-rank restoration solver. Reject malformed/unknown fields, nulls, NaN/infinity sentinels and
incompatible metadata without truncation, sorting or matrix repair. Restored
metadata cannot establish source observation values, actual variation or the
historical fit from the summary alone.

Unavailable history/undefined correlations fail the matrix call with a clear
domain outcome; malformed inputs/restoration are validation errors.
Unresolved numerical/matrix validation failure is a calculation error.
Success contains no partial cells or nonfinite values. Independent
qualification must cover common-span selection, adding/removing tickers,
one-ticker boundaries, two-point/singular matrices, terminal events, tiny/high-
scale variation, endpoint normalization, final PSD, carrier restoration and
cross-host/UI behavior. No confidence interval or sampling-quality guarantee
is implied by a successful matrix.

[R's correlation documentation](https://stat.ethz.ch/R-manual/R-devel/library/stats/html/cor.html)
distinguishes common complete observations from pairwise estimation and warns
that the latter can be non-PSD. [NumPy's Pearson documentation](https://numpy.org/doc/stable/reference/generated/numpy.corrcoef.html)
documents covariance normalization and floating-point symmetry/bound issues;
its implementation is not an independent oracle or a requirement to adopt
unconditional clipping. These references do not qualify this implementation's
numerical accuracy or make automatic projection an empirical estimator.

## Implementation evidence

[correlation.rs](../../../../finstack-quant/analytics/src/correlation.rs), [performance/aggregation.rs](../../../../finstack-quant/analytics/src/performance/aggregation.rs). The [crate integration tests](../../../../finstack-quant/analytics/tests/main.rs), inline numerical regressions and [schema contracts](../../../../finstack-quant/analytics/tests/schema_contract.rs) exercise this scope. See the [inventory validation record](README.md#implementation-evidence) for executed checks and evidence limits.
