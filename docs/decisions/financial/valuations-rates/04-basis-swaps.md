# VALUATIONS-RATES-04-BASIS-SWAPS: Basis swap floating legs and absolute par spread

- ID: VALUATIONS-RATES-04-BASIS-SWAPS
- Decision status: **Accepted — complete checked basis swap cashflows and par spreads package, 2026-11-14**.
- Implementation status: Runtime and caller reconciliation pending. Observed behavior below describes the current implementation.
- Validation status: Source, published references and independent arithmetic reviewed. Runtime, calibration and host qualification pending.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked basis swap cashflows and par spreads

1. **Fix the contract and quote leg.** Use one native currency and positive
   exact constant Money notional. The product value receives primary and
   pays reference. Exchange coupon cash without principal movements.
   Require explicit signed fractional Decimal bp spreads on both legs
   and identify the primary leg as the spread replacement target.
   Use authored terms or a deliberately selected qualified convention.
   Keep intentionally different leg dates and frequencies when supported.
   For this profile each original accrual period has one coupon payment.
   Reject unsupported compounding or multiple-reset arrangements.
   See [ISDA rates disclosure](https://www.isda.org/a/ORiDE/isda-rates.pdf)
   and [dated conventions](../valuations-framework/convention-selection.md).

2. **Resolve each schedule once.** Keep nominal and actual accrual dates,
   tenor, stubs, roll/EOM, day-count context, reset and payment conventions
   distinct. Apply explicit nonnegative lags and the selected calendar for
   each purpose. Remove missing-calendar substitution and its flag. An
   explicitly selected weekends-only calendar remains valid. Preserve
   the admitted actual accrual boundaries through observation processing.
   Do not roll them again on the observation calendar.
   Reject invalid geometry, collapsed periods or unsupported terms. Get
   the final payment from the two resolved payment grids without market
   lookup and keep it distinct from accrual or fixing maturity.
   See [schedule grids](../core-conventions/008-schedule-generation.md)
   and [cashflow periods](../cashflows/01-schedule-periods.md).

3. **Bind each benchmark and discount role exactly.** Identify the
   benchmark, tenor, currency, observation convention and projection
   source on each leg. Replace the bare-ID shared-curve flag with an
   explicit qualified shared binding. A shared evaluator must support
   every benchmark role it serves. The legs retain their own benchmark
   observations and periods on a shared source. A common collateral discount basis
   uses the common qualified source. Distinct per-leg discount sources
   require explicit terms and a qualified common native-currency PV basis.
   Equal ID text or mere market presence cannot choose a source, prove
   shared identity or substitute for collateral terms.
   See [projection factors](../core-market/forward-projection.md)
   and [Ametrano and Bianchetti, §2.2](https://www.bianchetti.org/Finance/BootstrappingTheIlliquidity-v1.0.pdf).

4. **Use checked term or compounded overnight cash.** Let `A` be the
   contractual coupon accrual and `s` the exact spread converted from bp.
   A term coupon uses `N A(F+s)` with unit gearing and no cap or floor.
   Use the exact benchmark interval and index accrual for `F` and the
   accepted rate admission and whole-coupon Money boundary. For either
   term or overnight, select an exact eligible fixing or an expressly
   unpublished observation's qualified forecast. Missing required
   observations and failed projections error. For overnight select
   compounded arrears with explicit lookback, observation shift or cutoff
   under the accepted block, partial-weight and publication rules.
   Compute `H=Π(1+r_i w_i)−1` with checked positive factors and use
   `N(H+sA)` with spread outside compounding. An admitted `A=0`
   contributes zero spread weight and may retain overnight growth cash.
   An equivalent rate on zero `A` is unavailable. Arithmetic averaging
   and inside-spread compounding are outside this profile. See
   [term projection](../cashflows/06-floating-rate-projection.md),
   [overnight projection](../valuations-framework/overnight-projection.md)
   and [IRS coupon rules](03-interest-rate-swaps.md).

5. **Use one prepared cash basis.** Ordinary holder PV excludes same-day
   payments. An explicit entitled same-day cut uses factor one. Actual
   payable amounts require eligible observations. A qualified forecast
   uses its own label. Future delayed payments stay live after accrual
   end. Discount each eligible payment on its actual date using its
   leg's positive dated relative factor. Preserve signed receive-perspective
   leg PVs and compute net PV as primary minus reference. Admit the
   contract before zero shortcuts. Skip unused market lookups for
   exhausted or fully fixed scope, while a known unpaid amount can still
   need discounting. Preserve signed leg and period identities, co-dated
   rows, exact contractual notional and observation provenance on export.
   Preserve each leg's accrual context and discount attribution.
   Use checked raw PV before final PV Money restoration. Distinguish
   a full contractual or historical view from eligible holder cash.
   See [dated NPV](../core-conventions/016-discounting.md) and
   [cashflow valuation](../valuations-framework/cashflow-export.md).

6. **Define the annuity and both spread measures.** A leg annuity
   `A_j=Σ A_ji D_j(V,p_i)` uses contractual accrual on eligible
   rows whose amount changes with that spread, with the selected
   same-day identity. It can be
   zero and does not need projection. A known fixing can still leave an
   unpaid spread-bearing row. The primary PV weight per bp is
   `W_p=N A_p/10,000`. Hold the reference spread, dates, observations and
   selected sources fixed. With `V_p0` the primary PV at zero primary
   spread, report absolute replacement `(V_reference−V_p0)/W_p` in bp,
   and incremental spread as that value minus the existing primary
   spread. Remove the annuity and one-cent notional floors and generic
   500/5,000 bp warning and rejection bands. Use checked scaled
   arithmetic for positive representable weights. A zero weight
   leaves par unavailable even if net PV is zero or reference cash
   remains live. Verify the represented returned quote by replacing the
   primary spread on the same cash basis and testing an explicit
   native-currency residual budget. Preserve annuities and spread quotes
   on their unscaled basis when the headline uses a scenario adjustment.
   See [Strata swap market quote and spread](https://strata.opengamma.io/apidocs/com/opengamma/strata/pricer/swap/DiscountingSwapProductPricer.html).

7. **Name the direct risk experiments by their shock.** Align the product
   with the IRS metric meanings. Use `dv01` for combined parallel direct
   risk and `pv01` for per-source parallel rows and their checked sum.
   The current basis `dv01` is per-source. Move that experiment and its
   row keys to `pv01`. Use `bucketed_dv01` for the triangular key-rate
   experiment. Report signed native currency per positive bp on the
   accepted represented central basis. Shock a shared primitive once
   across all its roles. Preserve exact typed active bindings, fixings,
   dates, economics and price cut. Return structural zero once eligible
   payments are exhausted and require qualified coordinates for requested
   bucket rows. Missing required sources or failed endpoints fail
   the experiment. The per-source sum and combined finite
   repricing remain distinct. See [shared rate risk](../valuations-framework/finite-difference-rates.md).

8. **Require explicit replay and consistent public admission.** Market
   quote replay requires a separate qualified request, exact quote
   bindings, an admitted original-market center and an authorized
   calibration provider. A stored Basis quote alone does not establish
   replay support. Keep calibration objective, pillar, weights and
   solver choices with the calibration owner. Use the same validated terms,
   resolved observations and cash basis across native construction, restore,
   pricing, risk, cash export and the existing Python/WASM tagged routes.
   Reconcile generated schemas and existing metric reports and fixtures.
   See [valuation lifecycle](../valuations-framework/pricing-lifecycle.md)
   and [shared builders](../valuations-framework/builder-validation.md).

## Observed behavior

### Choice, scope and rationale

Basis swaps value receive-primary/pay-reference floating coupons, each with its own projection and discount curve. The par-spread metric solves the primary leg's total quoted spread; incremental spread subtracts its existing quote. Simple and compounded overnight legs share the IRS/overnight kernels but retain basis-specific calendar and validation rules.

### Algorithm and controls

1. Require positive notional and positive leg tenors. Identical forward IDs error unless `allow_same_curve=true`; missing/invalid calendars error unless `allow_calendar_fallback=true`, which selects weekends-only. Negative reset/payment lags and SimpleAverage are rejected; simple overnight-ID misuse is rejected by the shared convention guard.
2. For term coupons, build unadjusted accrual boundaries and adjusted payments. Derive resets using the separate fixing calendar when provided. Select exact observed reset or period forward through shared fixing/projection helpers. Coupon is `N*(L+spread_bp/10000)*alpha`; discount with that leg's curve.
3. Compounded coupons use adjusted observation boundaries, the shared overnight engine, simple spread outside compounding and projected-fixing metadata. Degenerate adjusted intervals are skipped. PV excludes payments on or before valuation and uses Neumaier summation.
4. Annuity sums discounted accrual factors on unpaid periods. For overnight legs it reuses the projector's effective accrual rather than assuming the nominal schedule alpha. Reject annuity below `1e-12`.
5. `BasisParSpread = 10000*(PV_reference-PV_primary_with_zero_spread)/(N*A_primary)`. Keep the reference spread unchanged. Reject near-zero annuity and notional below 0.01 for this inverse metric. `IncrementalParSpread=BasisParSpread-current_primary_spread_bp`.
6. DV01 selects generic **per-curve** parallel sensitivities; BucketedDv01 selects triangular key-rate bumps. Annuity/PV metrics expose both legs separately. Model choice is deterministic discounted cashflow; there is no local quote-calibration replay branch.

### Conventions, failures and alternatives

The leg spread is bp, not decimal. Absolute spread above 5000 bp errors and above 500 bp warns. Separate leg discount curves are accepted here, unlike IRS validation. Zero-spread linearity makes algebraic par solving exact for the implemented simple additive spread; compounding spread inside daily factors would require a different solve and is not implemented. Explicit same-index spread trades are allowed only through the opt-in flag.

## Evidence

- [Basis implementation](../../../../finstack-quant/valuations/rates/src/basis_swap/types.rs) and [metrics](../../../../finstack-quant/valuations/rates/src/basis_swap/metrics.rs).
- [Par-spread tests](../../../../finstack-quant/valuations/rates/tests/basis_swap/test_basis_swap_par_spread.rs), [overnight tests](../../../../finstack-quant/valuations/rates/tests/basis_swap/test_basis_swap_overnight.rs), [sensitivities](../../../../finstack-quant/valuations/rates/tests/basis_swap/test_basis_swap_sensitivities.rs): algebraic solve, projection and bump evidence, not executed.
- [Ametrano/Bianchetti](../../../REFERENCES.md#ametrano-bianchetti-2013), [Sadr](../../../REFERENCES.md#sadr-2009-interest-rate-swaps).

## Independent review evidence

- For notional 1m, identical 0.25 accruals and projections at 5%,
  zero spreads, and separately qualified discount factors 0.99 for
  primary and 0.98 for reference, net PV is +125. The replacement
  primary spread is approximately −5.050505 bp and clears PV.
- For notional 1m, primary accrual 0.25, factor 0.98, forward 4% and
  current spread +12.5 bp, the zero-spread primary PV is 9,800 and the
  spread weight is 24.50 per bp. With reference accrual 0.5, factor
  0.97, forward 2.5% and spread −2.5 bp, reference PV is 12,003.75.
  Absolute replacement is about 89.94898 bp and the incremental
  amount is 77.44898 bp. Only the primary spread changes at par.
- For notional 1m and an overnight coupon with `H=0.001` and
  contractual `A=0`, the coupon pays 1,000 and its spread weight is
  exactly zero. If this is the only eligible primary row, par is
  unavailable. At the other boundary, `N=0.0001`, `A_p=1e−14`
  and a PV gap of `2e−20` give a positive weight of `1e−22` per bp
  and a 200 bp replacement when those amounts are representable.
- For two one-period legs with 0.5 accrual, notional 1m, primary
  forward 5% plus 10 bp and reference forward 4% less 5 bp, use discount
  factors 0.98 and 0.96 at source times 0.75 and 1.25. With projection
  intervals of width 0.5, base PV is 6,030. The ±1 bp central per-source
  sum is approximately +1.760750004942 and the simultaneous direct
  parallel result is +1.760749822528 in native currency per bp.

The examples check selected cash, spread and direct risk definitions.
Existing basis swap fixtures and the [tagged instrument loader](../../../../finstack-quant/valuations/src/instruments/json_loader.rs)
remain qualification entry points. The generic [calibration recipe builder](../../../../finstack-quant/calibration/src/targets/rate_recipe.rs)
currently rejects Basis quotes on its generic reconstruction path.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#rates-valuations).
