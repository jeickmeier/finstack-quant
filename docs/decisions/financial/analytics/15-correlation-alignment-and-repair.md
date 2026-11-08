# ANALYTICS-015: Common-sample Pearson correlation, provenance and numerical validation

**Decision status:** Accepted for the complete common-sample Pearson, availability, numerical-validation and result-provenance package below (2026-11-07). Implementation reconciliation and independent qualification remain open; core numerical algorithms retain their separate scope.

**Implementation status:** Observed in the source paths below, including the stated discrepancies and fallback behavior.

**Validation status:** Source inspected; test evidence inspected or located as identified below, not executed. No financial accuracy certification or runtime change is implied.

**Audit date / revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. See the [scope and exact source manifest](README.md).

## Choice and domain

`Performance.correlation_matrix` estimates equal-weight sample correlations and automatically projects an invalid assembled matrix to a valid correlation matrix. Date alignment is chosen at the matrix level: use one all-ticker common active span when it has at least two observations, otherwise use each pair's overlap. This mixes a statistical selection rule with a numerical repair rule; neither is established merely by registering the API.

## Accepted decision: common-sample Pearson correlation

**Accepted:** 2026-11-07. **Implementation:** Partial; selected source
inspected at `827ebb75b9`. Common-sample-only dispatch, all-column eligibility,
scale-safe arithmetic, bounded endpoint normalization, final validation,
repair/flag removal and the compact result carrier require reconciliation.
No runtime correlation, serialization or matrix tests were executed.

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
than two common observations. Thus the current fallback cannot rescue a
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
  current shared correlation helper's zero-denominator fallback and
  unconditional clamp do not establish correct analytics outputs. Preserve
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

The current repair flag has actual binding/UI consumers. Its removal must
propagate through Rust, Python, WASM, schemas/declarations, fixtures and the
correlation heatmap's repair stamp and adapters in one later implementation
slice; it is not an unused field or an invitation to retain an always-false
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

## Algorithm and controls

Initialize the diagonal to one. For each pair, choose the common span if available or its pairwise intersection otherwise. Fewer than two paired observations errors; either exactly zero sample variance errors; nonfinite correlation errors. No missing observations are filled and no pair is silently replaced by zero. A one-ticker matrix has no pairs to check and returns self-correlation one even if its active window is empty or degenerate.

Flatten and run the shared correlation-matrix validator. If already valid, force diagonal entries to exactly one and clamp off-diagonals to [−1,1], returning `(matrix,false)`. If validation fails, invoke core `nearest_correlation` with its default 200 iterations and Frobenius-change tolerance 1e−10, then validate the repaired result again. Failure to obtain a valid matrix errors. Unflatten, clamp bounds/diagonal and return `(matrix,true)`.

The core projection is the shared Higham/Dykstra PSD/unit-diagonal routine, whose kernel details are owned by the core-numerics methodology scope. The analytics caller does not expose repair options or opt-out. The boolean is true only for the projection path: small bound-clamping after a successful validation can alter entries while still returning false. It does not report which observation spans were used, adjustment magnitude or iteration diagnostics.

These are observed source behaviors. The accepted package requires removing
the pairwise branch, automatic projection and flag, enforcing one-ticker
eligibility, replacing unsafe moment arithmetic, bounding numerical endpoint
normalization, validating the actual final matrix and returning compact
common-sample provenance.

## Conventions, limitations and alternatives

Correlation is dimensionless and not annualized. Pairwise-overlap assembly can be indefinite because different coefficients summarize different samples; projection yields a mathematically usable matrix but changes empirical estimates. A common span of only two points is preferred over longer pairwise spans, so sample-size and precision can change discontinuously when another ticker's history changes. No minimum-history policy beyond two observations, shrinkage intensity or sampling-error model is included.

Always-complete-case estimation without automatic repair is accepted above.
Genuine explicit pairwise estimation, shrinkage covariance and caller-
controlled projection remain different unimplemented analytics methods.
A valid/repaired matrix is not proof of a suitable portfolio-risk model.

## Implementation and validation evidence

- [Alignment/assembly/repair caller](../../../../finstack-quant/analytics/src/performance/aggregation.rs), [active-span logic](../../../../finstack-quant/analytics/src/performance/mod.rs), [shared moments](../../../../finstack-quant/core/src/math/stats.rs), and [core projection/defaults](../../../../finstack-quant/core/src/math/linalg/nearest_correlation.rs).
- [Contiguous panel contract](../../../../finstack-quant/analytics/src/performance/panel.rs), [Python result tuple](../../../../finstack-quant-py/src/bindings/analytics/performance.rs) and [WASM matrix/flag conversion](../../../../finstack-quant-wasm/src/api/analytics/performance.rs) establish the current sample/output boundary. The UI's [correlation heatmap](../../../../finstack-quant-ui/registry/analytics/components/correlation-heatmap/correlation-heatmap.tsx) and related adapters consume the repair flag.
- Located inline `correlation_matrix_repairs_indefinite_pairwise_fixture`, clamp-to-unit-bounds and two-asset cases establish dispatch/invariants. [Correctness regressions](../../../../finstack-quant/analytics/tests/correctness_regressions.rs) reject degenerate pairs. They do not independently validate the repaired empirical risk model or justify mixed-span estimation.
- [Core numerical inventory](../core-numerics/README.md) owns the projection algorithm's detailed numerical evidence. [Ledoit–Wolf](../../../REFERENCES.md#ledoitwolf2004) is context for a distinct shrinkage alternative, not a claim that shrinkage is implemented here.

The indefinite fixture feeds a hand-built matrix directly to the private
finalizer. It does not reproduce a successful public pairwise-fallback matrix
under valid contiguous panel spans. Distinguish that helper-dispatch evidence
from public sample selection and empirical-model qualification.

No matrix simulations or numerical tests were executed.

## Follow-up tracking

Implementation reconciliation and missing evidence are tracked in the
[common-sample correlation follow-up](../FOLLOW_UPS.md#analytics-common-sample-correlation).
