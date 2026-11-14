# VALUATIONS-FRAMEWORK-18 — Repriced risk extraction, tenor allocation and margin identity

- Decision status: **Accepted — complete checked margin repricing and identity package, 2026-11-14**.
- Implementation status: Runtime and caller reconciliation pending. Observed behavior below describes the shared bridge and its consumers.
- Validation status: Source, published factor definitions and independent arithmetic reviewed. Runtime, replay, reporting and host qualification pending.
- Audit date / source revision: 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked margin repricing and identity

1. **One prepared valuation basis.** Select the margin method and edition,
   calculation date and information cut, market snapshot, contractual state,
   instrument and pricer dispatch, model and execution settings explicitly.
   Freeze the priced position basis, native amount currency, eligible cash and
   exact active market bindings. All base and shocked prices use that lifecycle
   and the selected held/rebuilt input rules. Reuse a result only when its
   provenance matches the requested valuation and risk basis, including the
   actual shock, replay, model and position basis. Apply the same request through
   direct and portfolio routes. Variation-margin marks and eligible settlement
   cash have their own agreement and lifecycle basis. Initial-margin sensitivity
   extraction stays separate. See [valuation dispatch](pricing-lifecycle.md)
   and [metric provenance](metric-orchestration.md).

2. **Complete, checked results.** Distinguish a product-qualified zero or
   inapplicable factor from a live unsupported calculation, missing inputs or
   rows, and failed pricing, market reconstruction or replay. Determine
   applicability from outstanding exposure and eligible cash. For example,
   completed accrual can leave future cash exposed to discount risk, and ended
   CDS protection can leave deterministic cash. Preserve failure context and
   publish a required risk set only after every required factor is accounted for
   and all amounts are finite. Remove automatic IRS/CDS duration fallbacks.
   Any product or margin approximation needs its own explicit selection,
   assumptions, eligibility and result provenance. Required model-derived risk
   keeps its original error on failure. A standalone margin calculation that
   requires the failed risk set fails with it. The portfolio owner governs
   aggregate failure and any explicitly permitted degraded results.

3. **Qualified shock and signed amount.** For ordinary IR market-yield delta
   select the signed upward price change for an exact positive 1 bp quote
   shock on the qualified replay basis. Require a separately selected and
   qualified definition for any other stencil or factor family, including
   inflation and cross-currency basis. Apply [CREDIT-13](../models-and-valuations-credit/13-cds-and-index-risk.md#accepted-decision-for-checked-cds-and-index-risk)
   to CDS and CDX/iTraxx credit delta, using
   `V(factor + 1 bp) − V(factor)` from the qualified replay center. Keep exact
   calibration bindings through the shock. A reused amount must be proven to
   reproduce the requested measure. Direct zero-rate DV01, centered quote
   CS01, and parallel risk each retain their own definitions.
   See [rate risk](finite-difference-rates.md) and
   [credit sensitivities](credit-sensitivities.md).

4. **Typed factor and exact vertex.** Extract complete rows from the
   requested grid using the resolved factor identity, including risk class,
   source and role, the applicable risk currency and IR subcurve, and the
   exact qualified vertex. Preserve distinct rows even when a curve ID or
   tenor label matches. Evaluate a shock once when the bindings establish the
   same complete economic shock. Reject malformed or out-of-domain
   coordinates. Use the selected methodology's exact target grid and approved
   subcurve and quote mappings. When a target needs a new shock, reprice it
   directly or establish equivalence to the required finite price change.
   Keep unmapped custom nodes identifiable in the source result. Remove the
   standard-label scan, maturity ceiling and automatic nearest-tenor or
   interpolation allocation. In the published SIMM specifications, IR uses
   twelve vertices including 2W, 1M and 3M, and credit uses 1Y, 2Y, 3Y, 5Y and 10Y.
   See [ISDA SIMM v2.8+2506, §C](https://www.isda.org/a/f2RgE/ISDA-SIMM_v2.82506_PUBLIC.pdf).

5. **Explicit credit identity and class.** Resolve qualifying or nonqualifying
   status and any required sector or bucket from the authoritative instrument
   or constituent classification for the selected edition. Keep issuer,
   seniority and payment currency distinct for qualifying credit. Preserve
   the issuer/tranche identity, native source coordinates and edition-specific
   grouping for nonqualifying credit. Preserve source index family, series and
   version. Reprice actual constituents where qualifying index attribution
   requires them. Remove curve-suffix entity inference, scalar-to-maturity
   allocation and the expired 5Y fallback.
   Exhausted protection supplies no credit rows after the product has
   accounted for remaining cash. Missing required identity, classification,
   attribution or requested vertex coverage fails. See
   [CREDIT-13](../models-and-valuations-credit/13-cds-and-index-risk.md#accepted-decision-for-checked-cds-and-index-risk)
   and [ISDA SIMM v2.8+2506, §C](https://www.isda.org/a/f2RgE/ISDA-SIMM_v2.82506_PUBLIC.pdf).

6. **Signed position and currency treatment.** Preserve the amount and its
   declared currency separately from risk-factor currency. Apply signed
   position quantity once against the priced basis. The designated owner
   performs any required conversion once using the qualified currency basis
   and the selected methodology's conversion rules. Preserve the original
   factor, amount currency, quantity basis and conversion provenance. Keep
   product class where the margin method groups by it. A consumer or wire
   shape that cannot express the required coordinates must return unsupported
   until the existing contract carries them.

7. **Qualified netting scope.** Use a structural tagged identity with the
   actual counterparty and applicable agreement/netting scope for bilateral
   positions. Use the exact CCP and the account, clearing-member, service
   or agreement partition where the qualified cleared scope requires it.
   Reject missing, ambiguous or conflicting required scope in an applicable
   request. CSA text, trade IDs or a synthetic identifier alone cannot
   establish scope. Preserve identity through grouping, ordering, export and
   restore. Display labels serve presentation. Keep collateral-account scope
   and regulatory applicability with their owners. See
   [Basel Framework CRE50](https://www.bis.org/committees/basel-framework/standard/cre/50/inforce/2019-12-15/published/2019-12-15)
   and [portfolio margin and netting](../portfolio-execution/16-margin-netting-and-initial-margin.md).

8. **Ownership and existing surfaces.** The valuation bridge owns prepared
   price and shock evidence, signed risk amounts and lossless attribution.
   Product owners qualify the cash and model basis and any separately selected
   approximation. The margin owners select editions, factor mappings, parameter
   tables, methods, collateral and netting authority. Reconcile IRS and CDS
   adapters, other product proxy consumers where they call this bridge,
   direct SIMM, portfolio, relevant SA-CCR identity consumers, and the
   existing Rust, Python, WASM, schema and report contracts. This record selects
   bridge behavior. Edition adoption, legal netting recognition and regulatory
   qualification remain with their owners. See
   [sensitivity input contracts](../margin-collateral-im/03-sensitivity-inputs.md),
   [agreement and netting terms](../margin-collateral-im/17-csa-selection-and-im-terms.md)
   and [portfolio margin exports](../portfolio-execution/17-margin-collateral-and-wire.md).

## Observed behavior

## Choice, algorithm and input domain

The bridge first requests an instrument's default pricing path with the desired risk metric. A pricing failure or missing scalar becomes `None`; bucketed-DV01 failure becomes an empty list. Product `Marginable` implementations decide whether to use a proxy. The helper deliberately loses the error category, so missing data and unsupported calculations are not distinguished from other failures. Selected-model overrides from an unrelated valuation request are not carried into this default request.

Bucket extraction searches only the standard DV01 tenor labels for each supplied curve ID and returns every found `(years,value)` pair. It does not discover custom grids or aggregate duplicate curve IDs. Its documentation calls the curves unique, but the loop does not deduplicate them; callers own that precondition.

Credit tenor allocation selects the first upper bound among 1, 2, 3, 5 and 10 years, otherwise 15Y. IR allocation selects 0.5, 1, 2, 3, 5, 10, 15 or 20 years, otherwise 30Y. This is ceiling allocation, not linear interpolation. Nonpositive values enter the first bucket; NaN and positive infinity reach the final bucket because no explicit finite-domain check precedes comparisons.

Reference-entity extraction rejects an empty curve ID, then strips at most two rightmost suffixes recognized as an integer followed by uppercase Y/M/W/D or a supported uppercase curve type (`CDS`, `BOND`, `CURVE`, `OIS`, `SOFR`, `ESTR`, `EURIBOR`, `LIBOR`, `HAZARD`, `SPREAD`). Either suffix kind may occur on either iteration. Unknown suffixes stop stripping. If stripping removes the entire prefix, preserve the original ID and warn. An ordinary unknown suffix returns the original text without warning despite the broader Rustdoc statement.

Credit delta requires explicit qualifying-sector or nonqualifying classification from `OtcMarginSpec`; absence or an unsupported variant errors. The bridge does not infer a sector from entity, rating or currency. Netting-set identity uses the CCP for cleared trades and repeats the CSA ID as both bilateral counterparty and agreement key otherwise.

## Alternatives, limitations and evidence

The accepted decision selects explicit reference-entity IDs, error-preserving extraction, prepared model selection, and exact qualified vertices. Margin-edition selection and production qualification remain open.

[margin.rs](../../../../finstack-quant/valuations/core/src/instruments/common_impl/margin.rs) contains every algorithm in this record. Its inline tests cover explicit qualifying/nonqualifying routing and absent-classification rejection. The [DV01 record](finite-difference-rates.md) defines upstream shock signs and units. Independent tenor-boundary, duplicated-curve and pricing-failure behavior tests were not established in this scope; no tests were run.

## Independent review evidence

- For `V(b) = 100 + 2b + 3b²` with `b` in bp, the upward one-bp change is
  5 and the centered one-bp amount is 2. At a one-year simple quote of 10%
  with redemption 100, the direct continuous-zero upward shock gives about
  −0.00909045 while raising the simple market quote by one bp gives
  −0.00826371. Shock basis matters even when base price and tenor match.
- Distinct 3M and 6M amounts of +12 and −12 collapse to zero under the
  current IR ceiling. When these amounts keep their vertices, an illustrative
  equal-weight aggregation with correlation 0.8 gives about 7.58947.
  Distinct same-vertex subcurves also require their separate keys.
- Bilateral `(A:B, C)` and `(A, B:C)` both display as `A:B:C` with the
  current separator. Structural equality distinguishes them. Opposite
  positions in separate client accounts can likewise share a CCP label
  while belonging to separate cleared scopes.

The product call sites expose the observed default basis, lost errors, expiry
and proxy behavior. The current sensitivity containers also omit required
coordinates such as IR subcurve. Production qualification needs matched
quote-replay price fixtures, distinct and duplicate-binding cases, custom
vertices, signed units and currency conversion, late settlement cash,
incomplete requests, separate netting scopes and host round trips.
[IRS adapter](../../../../finstack-quant/valuations/rates/src/irs/marginable.rs),
[CDS adapters](../../../../finstack-quant/valuations/credit/src/cds/marginable.rs),
[SIMM result types](../../../../finstack-quant/margin/src/types/simm_types.rs),
[netting types](../../../../finstack-quant/margin/src/types/netting.rs),
[portfolio aggregator](../../../../finstack-quant/portfolio/src/margin/aggregator.rs).

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#shared-valuation-framework).
