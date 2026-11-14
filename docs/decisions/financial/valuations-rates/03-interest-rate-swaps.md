# VALUATIONS-RATES-03-INTEREST-RATE-SWAPS: IRS cashflow projection, convention resolution and sensitivity bases

- ID: VALUATIONS-RATES-03-INTEREST-RATE-SWAPS
- Decision status: **Accepted — complete checked interest-rate swap cashflows and risk package, 2026-11-14**.
- Implementation status: Runtime and caller reconciliation pending. Observed behavior below describes the current implementation.
- Validation status: Source, published references and independent arithmetic reviewed. Runtime, calibration, margin and host qualification pending.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked interest-rate swap cashflows and risk

1. **Specify one fixed-for-floating contract.** Use one native currency,
   positive exact constant Money notional, no principal exchange and one
   exact typed discount source shared by both legs. Require explicit pay or
   receive fixed, exact signed Decimal fixed rate and signed fractional
   Decimal spread in bp. Remove the magnitude floor on notional and the
   generic fixed-rate bound. Use authored terms or a deliberately selected
   qualified convention. Use one coupon settlement per accrual period in
   this profile. Permit different nominal leg windows only when
   deliberately specified and supported by the selected terms. Validate
   each leg independently and reject mismatches against a selected common
   window. Different frequencies and effective schedules can still share
   one nominal window. See [dated convention selection](../valuations-framework/convention-selection.md)
   and [shared contract controls](../valuations-framework/shared-contract-controls.md).

2. **Prepare each grid once.** Keep nominal and actual accrual dates,
   reference periods, fixing coordinates and actual payments explicit.
   Resolve the selected frequency, stubs, roll/EOM, day-count context,
   calendars, accrual adjustment and nonnegative lags. Preserve the
   contractual day-count references for ICMA and ACT/365L. Apply
   payment adjustment and lag in the accepted order and discount the
   emitted payment date directly. Compute the last actual payment over
   both grids without market data. Preserve it separately from the
   contractual accrual and fixing horizons. Remove negative lag sentinels,
   inferred EOM, fallback calendars, repeated adjustment and silently
   dropped overnight periods. Allow a qualified zero contractual
   accrual; reject invalid dates, context or schedules. See
   [schedule grids](../core-conventions/008-schedule-generation.md) and
   [contractual cashflow periods](../cashflows/01-schedule-periods.md).

3. **Use one fixed and term coupon basis.** Before trade side, fixed
   coupon cash is `N K A` and simple term coupon cash is `N A(F+s)`,
   where `A` is the applicable contractual coupon fraction and `s`
   is the converted decimal spread. Use unit gearing and no cap/floor
   in this profile. Resolve one exact benchmark observation per term
   coupon, with its own accrual, period and publication status. Use a required
   eligible fixing, or project an expressly unpublished observation
   through the qualified factor source and index accrual. A missing
   required fixing or failed projection errors. Admit the selected index
   rate through the accepted Decimal rule, check positive index growth,
   then apply exact spread and the applicable all-in-rate and whole-coupon
   Money boundaries. Preserve signed cash. A dates-only
   report identifies unresolved amounts and cannot fabricate spread-only
   cash. See [fixed coupon arithmetic](../cashflows/05-coupon-cash-and-pik.md)
   and [term observations](../cashflows/06-floating-rate-projection.md).

4. **Select direct-growth overnight cash.** Require an explicitly admitted
   compound method: arrears with its selected lookback including zero,
   observation shift, or cutoff. Keep actual accrual endpoints, qualified
   observation calendar, full quote intervals, retained partial blocks,
   and the accepted shift and cutoff rules. Freeze distinct observations
   and publication authority. No automatic combination of offsets or
   arithmetic averaging in this IRS profile. With overnight weights
   `w_i`, compute `H=Π(1+r_i w_i)−1` through the accepted positive-factor
   arithmetic and use `N(H+sA)` for cash. Keep contractual spread outside
   compounding. Annualized index growth `H/Σw_i` is a separate output.
   An equivalent growth rate on `A` is unavailable when `A=0`. Use
   one checked contractual cash boundary. See
   [shared overnight projection](../valuations-framework/overnight-projection.md),
   [overnight block rules](../cashflows/07-overnight-compounding.md),
   [ISDA overnight elections](https://www.isda.org/a/BNEgE/Key-Changes-in-the-2021-ISDA-Interest-Rate-Derivatives-Definitions-June-2021.pdf)
   and [New York Fed SOFR methodology](https://www.newyorkfed.org/markets/reference-rates/additional-information-about-reference-rates).

5. **Bind sources by actual financial role.** Require the exact typed
   projection source for unresolved observations. An explicitly qualified
   single-evaluator overnight route may use the same source for discount
   and projection and retains both role bindings. Same identifier text
   or market presence cannot select or replace a role. Fail if distinct
   required bindings cannot be represented. A fully fixed leg
   does not fetch projection. Publish actual dependencies and observation
   provenance for each operation, including known versus projected fixings.
   See [projection factor sources](../core-market/forward-projection.md).

6. **Value eligible payments and solve par on that basis.** The ordinary
   holder mark includes payments strictly after valuation. An explicit
   entitled same-day cut includes the payment at factor one. Keep
   projected amounts distinct from actual payable cash. Under an admitted
   forecast cut, an expressly unpublished observation can be projected.
   Actual same-day payable cash requires the qualified completed observations.
   Paid or otherwise excluded cash does not demand unused fixings.
   Lagged future payments stay live after accrual ends. After structural
   admission an empty eligible stream returns native zero without fetching
   unused sources. Use positive dated relative discount factors and
   checked accumulation for both leg values. The leg metrics are
   receive-direction amounts before trade side, so can be negative.
   Pay fixed nets floating minus fixed, with the opposite sign for
   receive fixed. Preserve leg and period identity for co-dated payments.
   Use one prepared ledger for price, risk and export. An adjusted headline
   uses its scenario multiplier once. Qualify any leg breakdown against
   the reported basis. The contractual par and annuity stay on their
   own unscaled basis.
   Publish raw PV before final PV Money conversion while retaining the
   admitted contractual cash. Keep full historical and contractual views
   separate from the holder filter. On eligible fixed rows calculate
   `A_fixed=Σ A_i D(V,p_i)`, and use `K_par=V_float/(N A_fixed)`
   for the common fixed-rate replacement. A zero coefficient leaves par
   unavailable, including a live float-only remainder. Remove the
   near-zero floor. Check the represented replacement by repricing on the
   same basis within a stated native-currency residual budget. See
   [cashflow valuation](../valuations-framework/cashflow-export.md) and
   [Strata swap par rate](https://strata.opengamma.io/apidocs/com/opengamma/strata/pricer/swap/DiscountingSwapProductPricer.html).

7. **Give each direct and replay risk measure a fixed meaning.** Use
   signed native currency per positive bp for direct combined parallel
   `dv01`, per-source parallel `pv01` with its checked total, and
   triangular `bucketed_dv01`. Apply the accepted represented central
   stencils to exact typed active sources. Select quote replay explicitly
   with a qualified center, exact quote scope and authorized provider.
   Missing or incomplete required replay errors. Keep the cash basis,
   contractual rates, dates and known fixings consistent across reprices.
   Combined convexity is the second divided difference along the combined
   direct direction. Discount/projection cross-gamma uses the four corners
   on distinct qualified axes. Report both in native currency per
   decimal-rate squared. A single active shared evaluator supplies
   combined convexity but no independent discount/projection cross.
   A proven absent projection contribution after fixing gives zero
   interaction on the supported two-source contract. Missing required
   inputs fail. Determine exposure by actual payment and fixing state.
   See [direct and replay risk](../valuations-framework/finite-difference-rates.md)
   and [cross-factor risk](../valuations-framework/cross-factor-risk.md).

8. **Use the accepted margin bridge and shared ingress.** For the selected
   ordinary IR market-quote margin basis require signed positive one-bp
   repricing on the qualified replay center with edition, currency,
   subcurve and vertex, complete rows and qualified netting scope.
   Remove tenor rounding, partial-success publication and the duration
   fallback. Use remaining eligible cash and exact basis, including after
   accrual end. Variation margin uses its qualified economic mark and
   settlement terms. Reuse the same checked resolved swap through tagged
   instrument, direct, calibration, replay, cash export, Python and WASM
   routes. Preserve convention edition, actual payment dates and source
   roles. Carry exact term and overnight projection intervals through
   calibration. Calibration owners choose pillar, residuals and acceptance.
   Reconcile the existing swaption and CMS annuity consumers on their own
   cuts. See [margin repricing](../valuations-framework/margin-bridge.md),
   [valuation lifecycle](../valuations-framework/pricing-lifecycle.md) and
   [swap materialization](../calibration-framework/06-swap-materialization.md).

## Observed behavior

### Choice, scope and rationale

IRS valuation discounts separately generated fixed and floating coupon schedules with one shared collateral discount curve. Term floating projection and overnight observation compounding are distinct paths. Equal forward/discount identifiers permit a single-curve compounded OIS path; otherwise projection requires the forward curve. There is no principal exchange in the swap schedules.

### Algorithm and controls

1. Resolve negative reset/payment-lag sentinels from the forward-ID convention registry; an unresolved sentinel errors. Both-endpoints-at-month-end automatically enables EOM even if the stored flag was false. Fixed/float accrual-date adjustment flags separately control schedule generation.
2. Fixed coupon is `N*K*alpha`; classify an irregular front/back stub from expected tenor stepping and EOM boundaries. The annuity sums `alpha*D(as_of,pay)` over payments strictly after valuation, using Neumaier summation.
3. Simple floating coupons use the cashflow builder with unit gearing, spread included, no cap/floor and Error fallback when a market exists. A schedule-only call without market uses SpreadOnly as a representation fallback, not a valid market forecast.
4. Compounded arrears/observation-shift/rate-cutoff schedules use the shared overnight projector. Resolve fixing calendar from explicit fixing calendar, leg calendar or supported currency convention. Adjust observation accrual boundaries and skip collapsed intervals; ACT/365L retains the original coupon reference interval. Historical observations and remaining projected observations determine the compound factor. Add spread simply outside compounding and retain projected-fixing metadata.
5. Single-curve compounded OIS uses a same-ID forward curve if present, otherwise the discount curve as overnight projection source. Normal dual-curve pricing requires the forward. Sum only future coupon payments; continue past raw maturity when payment lag leaves unpaid coupons. Pay-fixed PV is floating PV minus fixed PV; Receive reverses it.
6. ParRate is floating PV divided by `N*fixed_annuity`; near-zero annuity errors. Leg-PV metrics are unsigned leg values. SimpleAverage is rejected. Discount IDs must match across legs; mismatched raw leg date ranges warn rather than error. Notional must exceed `1e-6`; fixed decimal rate magnitude above 100 errors.

### Risk and margin algorithms

DV01 uses central quote-bump calibration replay when recipes exist, including a discount-only replay for single-curve OIS with no separate forward object; otherwise generic combined curve bumps. Per-curve PV01 and triangular bucketed DV01 use the generic curve-bump engine. IR convexity centrally bumps all existing declared rate curves and divides by squared decimal bump. IR cross-gamma uses four discount/forward bumps; equal IDs or missing participating curves returns zero.

SIMM first sums repriced bucketed DV01 into tenor buckets. If no repriced sensitivity is available, it warns and uses `abs(N)*remaining_calendar_years*DURATION_APPROXIMATION_FACTOR*1bp`, signed by payer/receiver, distributed by overlap lengths in fixed 6M–30Y buckets. Expiry for this margin path is float-leg end. A partially available nonempty set is accepted as repriced, so completeness depends on the shared margin helper. VM uses the instrument value.

### Alternatives and tradeoffs

The accepted decision keeps quote replay and direct zero-curve risk as separate measures. Required margin risk uses complete qualified repricing. Overnight spread follows the selected outside-spread formula and each leg uses its resolved contractual calendars.

## Evidence

- [IRS types/conventions](../../../../finstack-quant/valuations/rates/src/irs/types.rs), [cashflow projection](../../../../finstack-quant/valuations/rates/src/irs/cashflow.rs), [PV](../../../../finstack-quant/valuations/rates/src/irs/pricer.rs), [annuity](../../../../finstack-quant/valuations/rates/src/irs/metrics/annuity.rs), [metric selection/par](../../../../finstack-quant/valuations/rates/src/irs/metrics/mod.rs), [DV01](../../../../finstack-quant/valuations/rates/src/irs/metrics/dv01.rs), [convexity/cross-gamma](../../../../finstack-quant/valuations/rates/src/irs/metrics/ir_convexity.rs), [margin](../../../../finstack-quant/valuations/rates/src/irs/marginable.rs).
- [Compounding accuracy](../../../../finstack-quant/valuations/rates/tests/irs/test_compounding_accuracy.rs), [cashflow reconciliation](../../../../finstack-quant/valuations/rates/tests/irs/cashflows.rs), [par-rate tests](../../../../finstack-quant/valuations/rates/tests/irs/metrics/par_rate.rs), [margin tests](../../../../finstack-quant/valuations/rates/tests/irs/integration/margin.rs): formula, lifecycle and risk invariants.
- [Ametrano/Bianchetti](../../../REFERENCES.md#ametrano-bianchetti-2013), [ARRC](../../../REFERENCES.md#arrc-sofr-users-guide), [ISDA](../../../REFERENCES.md#isda-2006-definitions).

## Evidence limits

In same-ID OIS, the projection path uses an existing forward object but `market_dependencies` omits forward dependency based on equal IDs alone. No independent certification of margin proxy or partial-bucket completeness is established.

## Independent review evidence

- For index growth `1.0125`, index accrual `91/360`,
  coupon accrual 0.25, notional 1m and 10 bp spread, the term coupon is
  approximately 12,612.64. Using the coupon fraction in the projection
  denominator would give 12,750.
- Two overnight blocks at 4% for `3/360` and 5% for `1/360` give
  `H=10201/21600000`. With coupon accrual `4/365`, notional 1m and
  10 bp outside spread, direct-growth cash is approximately 483.23.
  Annualizing on the index basis and applying the coupon basis gives
  approximately 476.76.
- With floating PV 12,300, notional 1m and fixed annuity
  `0.25 × 0.98 = 0.245`, the common replacement fixed quote is
  approximately 5.020408%. A future lagged payment remains in the
  annuity even after the corresponding accrual period ends.
- With notional 1m and fixed rate 1%, take same-day fixed and float
  coupon fractions 0.4 and 0.25, paying fixed and receiving −1%
  floating. Take later 0.5 fractions on both legs with floating
  rate 2% and discount factor 0.98. The eligible same-day cut gives
  pay-fixed PV −1,600 and par about 0.820225%. The holder cut gives
  PV +4,900 and par 2%, with the later payment still included.
- In a one-period pay-fixed example with notional 1m, accrual 0.5,
  fixed 4%, projected 5%, discount 0.98, discount horizon 0.75 and
  projection interval width 0.5, PV is 4,900. Direct centered ±1 bp
  shocks give about −0.3675 on discount, +50.2250 on projection and
  +49.8575 combined per bp. The independent discount/projection
  cross-gamma is about −376,687.50 per decimal-rate squared. A quote
  reprice that raises the forward by exactly 1 bp with discount
  unchanged gives +49. The two experiments report different quantities.

These examples bound the reviewed arithmetic. Existing cashflow, par and
risk fixtures still require qualification against resolved schedules, the
complete live market basis, the accepted replay and margin contracts,
and the host surfaces.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#rates-valuations).
