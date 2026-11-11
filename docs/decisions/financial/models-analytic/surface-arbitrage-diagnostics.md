# MODELS-ANALYTIC-20: Surface arbitrage scans, severity and pass policy

**Decision status:** Accepted (complete package adopted 2026-11-10).

**Implementation status:** Observed in the source scope identified below; discrepancies are identified explicitly.

**Validation status:** Source and caller paths inspected. Independent scalar and rational checks support the proposed distinctions. Production, financial and host qualification remain pending.

**Audit date:** 2026-11-05. **Source revision:** `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked surface-arbitrage diagnostics and coverage

All 27 decisions were accepted on **2026-11-10**. Implementation reconciliation
and independent complete-path qualification remain open.

1. **Use one checked surface diagnostic entry point.** Consume a
   canonical validated unshifted Black annual-volatility surface with
   positive expiries and positive absolute strikes, using
   [accepted record 16](volatility-source-interpolation.md#accepted-decision-for-explicit-volatility-source-evaluation-and-materialization)
   for source lookup. Nonnegative stored sigma, including zero, is
   valid. Retire unchecked public checker structs and the parallel
   raw-grid and violation-vector entry points.

2. **Require actual row forward inputs.** Bind one finite positive
   forward to each exact expiry and its underlying and strike units.
   Validate the full association and source clock for dated use under
   ADR-0045. Keep numeric source clocks optional. No scalar broadcast,
   repeat-last fallback or inferred forward is applied.

3. **Select the checks explicitly.** Require at least one of the
   sampled call-price, fixed-moneyness calendar and Dupire sign checks.
   Only the selected checks contribute to the report. Validate the
   entire request before evaluating probes. A supported thin surface
   can yield partial diagnostics even when a selected check needs
   more points.

4. **Give each comparison its own threshold and units.** Require a
   finite nonnegative `price_abs_tol` for sampled call-price checks,
   in undiscounted premium per underlying unit. Require a finite
   nonnegative `total_variance_abs_tol` for calendar comparisons.
   The Dupire check uses the sign rules in decisions 16–18.
   Remove shared numeric thresholds and automatic check defaults.

5. **Price the original strike rows.** When the price check is selected,
   evaluate the checked Black unit call at every original knot with the
   row's actual forward and the exact stored sigma. Zero sigma uses
   the intrinsic limit. Preserve the represented premium and propagate
   pricing errors.
   [Record 02](forward-normal-shifted.md#accepted-decision-unit-forward-prices-and-native-model-greek-bundles)

6. **Check bounds and include the exact Black anchor.** At observed
   strikes compare `max(F-K,0) ≤ C ≤ F`. Prepend the mathematical
   point `(K,C) = (0,F)` to the ordered row for spread checks.
   This anchor comes from the positive-underlying law and needs
   no implied-volatility lookup.

7. **Check both vertical-spread inequalities.** For every adjacent
   pair including the anchor require
   `0 ≤ C_i - C_(i+1) ≤ K_(i+1) - K_i` within the native
   premium tolerance. Report separately the breached inequality
   and its actual premium excess.

8. **Check unequal-strike convexity.** For every adjacent triple
   including the anchor, let h_L and h_R be its left and right
   strike gaps. Compare the middle call with
   `(h_R C_left + h_L C_right)/(h_L+h_R)`. Report any excess
   above the chord. Use actual strike spacing throughout.

9. **Reuse the accepted price-grid arithmetic.** Compare represented
   strikes, forwards and unit premiums with the exact arithmetic
   from record 15. A positive constraint excess breaches when it is
   strictly greater than the price tolerance. Compare before
   upward display rounding. Use the price constraints as the
   sampled strike-slice check, with the finite-difference density
   factor handled by the Dupire check.
   [Record 15](sabr-smile-repair.md#accepted-decision-for-checked-sabr-smile-and-finite-grid-repair)

10. **State the price check's actual coverage.** Count observed
    bounds, adjacent pairs and convexity triples and identify
    where the anchor contributed. At least two observed strikes
    are required for complete coverage, with two supplying the
    anchor triple only. On a single-strike row still check the
    available bounds and anchor vertical and report missing
    convexity coverage.

11. **Use fixed forward moneyness for calendar comparisons.** Use
    adjacent expiries of the same underlying with qualified
    deterministic discounting and proportional carry. Compare
    normalized calls in the coordinates `K/F`, equivalently
    their total variance `w = T sigma²` at fixed
    `k = log(K/F)`. The selected comparison is in total
    variance, under the stated normalized-price model.

12. **Probe both rows' knots on common support.** For each adjacent
    pair intersect the two closed normalized-strike supports.
    Use the ascending exact union of both rows' normalized
    knots within the intersection, including its endpoints.
    Preserve distinct coordinates by comparing the original
    positive ratios without prematurely rounding them. Deduplicate
    exact equality. Exclude out-of-overlap wings from the declared
    calendar scope and disclose each pair's overlap.

13. **Resolve the actual matched pair strictly.** At each probe
    evaluate each original expiry with its own matched cash
    strike and declared source strike interpolation. Use record
    19's complete ratio remap and record 16's checked lookup.
    Preserve exact original knots. Retain the exact normalized
    probe identity and both actually evaluated represented strikes.
    Reject unresolved geometry. No flat wings or interpolated
    forwards enter these comparisons.

14. **Compare calendar changes in native total variance.** Compute
    the signed complete `w_later - w_earlier` with scale retained
    through subtraction. Breach when it is strictly below minus
    the supplied total-variance tolerance. A pair with a
    positive-width overlap and all union probes evaluated is
    complete for that scope. A singleton overlap can still be
    tested and reported, with incomplete coverage. Empty overlap
    and absence of adjacent pairs are incomplete.

15. **Select the Dupire probe region explicitly.** Use a nonempty
    rectangular range of original expiry and strike indices,
    including the whole grid if selected. Use the full original
    source for stencils. Require the same qualified forward and
    proportional-carry basis as
    [record 19](dupire-local-volatility.md#accepted-decision-for-checked-dupire-extraction-and-local-volatility-grids).
    The diagnostic uses
    the unsmoothed source and strict matched support.

16. **Check the density factor where it is defined.** Evaluate
    record 19's checked log-moneyness derivatives and Durrleman
    g with positive center total variance and sufficient strike
    support. Report computed g≤0 as a failure of the positive
    density requirement for local inversion. A zero g has that
    domain meaning. With a zero center or insufficient strike
    nodes, the density probe is unavailable.

17. **Check the time slope independently.** Use record 19's
    matched-moneyness time stencil and report computed w_T<0
    in total variance per model year. A computed zero slope is
    allowed. A missing time stencil or strict matched strike
    outside support makes the slope unavailable. Keep available
    findings from either quantity when the other is unavailable.

18. **Keep the Dupire decision in native quantities.** Use the
    actual computed signs of g and w_T. Remove the shared
    tolerance and the extra ratio test on w_T/g. A sampled
    sign result describes the stencil. Actual local-sigma
    extraction, including representability of the ratio and
    root, remains governed by record 19.

19. **Distinguish missing coverage from failure to compute.**
    A declared thin grid, zero center or unsupported matched
    point can limit the relevant check. Preserve its typed
    reason and location. Invalid source/configuration, failed
    pricing, unresolved arithmetic or unavailable required
    output representation produces a typed error for the
    operation. Do not convert errors to empty findings.

20. **Return typed findings in their own units.** Preserve the
    tested inequality, relevant source expiry and strike or
    paired calendar coordinates, native signed metric or
    exact-compared premium excess, threshold and units.
    The Durrleman density factor is dimensionless. Distinguish total
    variance from its time derivative. Preserve report scope and source,
    forward and clock association.

21. **Track coverage independently of findings.** For every
    selected family report the actual sampled domain, checked
    counts and unavailable required probes or missing support,
    with reasons. Count density and time-slope probes separately.
    Distinguish absent test families from selected checks with
    incomplete coverage. Retain known findings on partially
    covered scopes.

22. **Derive one status from the complete report.** `Failed`
    means at least one selected criterion breached, including
    when coverage is incomplete. `Incomplete` means no breach
    was found and at least one selected check lacked complete
    coverage of its declared scope. `Passed` requires complete
    coverage and zero breaches. Use the same rule for each
    selected family. A typed operation error is separate.

23. **Keep presentation independent of the verdict.** Retire
    universal severity bands, `min_severity`, and the redundant
    filtered pass boolean. Preserve every breach and its native
    amount. Any display-only filtering leaves the native
    status, counts and coverage intact. Remove automatic
    suggested fixes and unused SVI-specific report variants.
    Record 18 owns the SVI fit diagnostics.

24. **Validate report state and deterministic ordering.** Use
    fixed check-kind, source-order and probe-coordinate ordering.
    Check work, counts and output sizes before publication.
    Restore state through validation of native fields, explicit
    scope and complete findings and coverage. Derive summary
    status and counts from canonical contents, or verify any
    serialized summaries against them.

25. **Reconcile the existing hosts and UI.** Use the same
    Rust report and errors in Python and WASM. Update request
    and report schemas, stubs, declarations, facade, parity,
    serialization and DataFrame views. The pricing workbench,
    worker and surface badge supply explicit controls and
    display actual selected scope, units and status, including
    the distinction between a local-inversion domain failure
    and a sampled price breach. Keep
    request absence and operation errors distinct from a
    completed report.

26. **Limit the financial and publication claims.** The report
    assesses sampled price constraints, overlap calendar probes
    and local stencil signs. Continuous between-node behavior,
    wings, distribution existence, local-volatility repricing
    and product eligibility require separate qualification.
    CALIBRATION-FRAMEWORK-017 owns validation and publication
    disposition of calibrated outputs. This diagnostic makes
    no automatic changes to the surface.

27. **Require independent and end-to-end evidence.** Cover
    exact anchor and unequal gaps, zero-sigma prices, the two
    row calendars including point-only overlap, interpolation
    limits, both independent Dupire quantities, partial
    coverage, tolerance ties and error precedence. Check
    native report restoration and actual Python, WASM and
    UI rendering. See the
    [surface diagnostic references](../../../REFERENCES.md#checked-surface-arbitrage-diagnostics-and-coverage).

## Independent arithmetic checks

| Fixture | Result |
|---|---|
| F=T=1, strikes (0.94,1.29,1.40), sigmas (0.24,0.31,0.19) | The middle call exceeds the unequal-strike chord by about 0.00778334. The existing interior density stencil gives g≈+0.04900. |
| F=100, strikes (80,100), calls (30,10) | Observed bounds and observed vertical spread pass. Including the exact anchor gives a chord value 28 at K=80 and convexity excess 2. |
| K=(80,100,120), F=(100,110), early w=(0.04,0.04,0.04), later w=(0.06,0.03,0.06), TotalVariance mode | Common normalized knots are (4/5,10/11,1,12/11). The signed differences are (0.008,−0.01,0.005,0.02). Earlier-row knots alone miss the later-row breach. |
| Two source rows with w_i(K)=0.1+0.1 K/F_i, F=(100,121), TotalVariance time interpolation | Matched endpoints give 0.20 at k=0. Halfway with F=110, the published fixed-strike interpolation gives 441/2200≈0.20045455. This produces a subsequent fixed-moneyness decrease. |
| Positive final calls (0.25,0.25) at strikes (1,2) with F=1 | The sampled constraints pass with the anchor. A convex decreasing continuation with final zero slope cannot reach a vanishing call tail. |
| A total-variance drop of 2e−6 with the current default tolerance | The current Minor classification is removed by a Major filter before the current pass calculation. |

These scalar checks compare the selected inequalities, actual sampled
support and the current report predicate. Production, host and full
financial qualification remain pending.

## Observed implementation and evidence

### Scope and input domain

Black-lognormal strike surfaces. Top-level check validates convention/tolerance and requires explicit forwards when any check is enabled; a single forward expands across expiries.

### Choice and rationale

Discrete butterfly, fixed-log-moneyness calendar and Dupire-density scans produce diagnostics. Severity/filtering controls the report’s pass boolean; a passed report is not a proof of global absence of arbitrage.

### Algorithm

Butterfly uses unequal-log-strike derivatives of w and Durrleman g at interior strikes. Calendar compares adjacent-expiry w at fixed k, with clamped second-expiry strike. Density uses shared Dupire kernel, strict matched-k time lookup, flags negative denominator or negative local variance when denominator>tol. All comparisons use configured tolerance (default1e−10). Magnitudes classify negligible<1e−8, minor<1e−5, major<1e−3, critical otherwise. Apply min_severity filter first, sort descending severity, then passed=true if no retained severity above Negligible.

### Selection and controls

All three checks default enabled; default config has no forward and therefore errors until populated. Low-level checker structs are public and weaker: empty/short grids return no violations; butterfly/calendar repeat last forward for missing expiry entries and skip nonpositive forwards; density requires exact count and positive finite forwards.

### Conventions

Magnitudes have different units: density g dimensionless, calendar total-variance decrease, local-variance negativity. Shared fixed thresholds do not normalize these to a single economic loss.

### Limitations, failure and fallback

Filtering can make passed true after removing real lower-severity violations. Thin grids/near-zero total variance are skipped; nonfinite low-level calculations can evade comparisons. Wings outside sampled points and interpolation between points are not certified. Declared SVI violation enum variants are reporting vocabulary, not checks automatically executed by check_surface.

### Alternatives and tradeoffs

Analytic parameter constraints/global price convexity tests would be alternatives. Top-level strict configuration is safer than direct low-level checker construction but both remain exposed.

### Implementation and validation evidence

[checks.rs](../../../../finstack-quant/models/src/volatility/arbitrage/checks.rs), [mod.rs](../../../../finstack-quant/models/src/volatility/arbitrage/mod.rs) and [types.rs](../../../../finstack-quant/models/src/volatility/arbitrage/types.rs). Inline constructed-violation and missing-forward tests establish reporting behavior; local-vol analytic SSVI evidence covers a related kernel, not all report classifications. References: [Static arbitrage](../../../REFERENCES.md#gatheral-jacquier-2014-svi), [Dupire](../../../REFERENCES.md#dupire-1994).

The pricing workbench currently supplies forwards and relies on default
checks and tolerance. The badge renders violation magnitudes as total
variance and summarizes enabled and disabled checks together. The
calibration validator uses separate sampled calendar and call constraints
for its own publication policy. See the
[pricing workbench](../../../../finstack-quant-ui/registry/valuations/blocks/pricing-workbench/pricing-workbench.tsx),
[surface badge](../../../../finstack-quant-ui/registry/models/components/surface-arbitrage-badge/surface-arbitrage-badge.tsx),
[calibration validator](../../../../finstack-quant/calibration/src/validation/surfaces.rs)
and [calibration validation record](../calibration-framework/17-surface-validation.md).

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[analytic-model follow-ups](../FOLLOW_UPS.md#analytic-models).
