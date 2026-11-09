# CORE-NUMERICS-10: Close and OHLC realized variance estimators

- Decision status: **Accepted — complete positive-price/log-return/realized-variance package, 2026-11-08**.
- Implementation status: **Observed; reconciliation pending**, within the path scope below.
- Validation status: **Source inspected; independent high-precision/algebraic examples and semantic reviews passed**. Production log/estimator/host/caller and financial qualification remain pending; this record is not a correctness certification.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`. [Slice scope and coverage](README.md).

## Scope and input domain

This record owns math/stats.rs positive-price log_returns, standalone close
realized_variance and the complete-OHLC estimator dispatcher. Outputs are
annualized decimal squared-log variance estimates, not volatility or accumulated
horizon quadratic variation. [Accepted record 09](sample-statistics.md#accepted-decision-checked-sums-moments-quantiles-and-mean-uncertainty)
owns shared checked sums, scaled moments and availability; analytics return,
frequency and terminal-loss definitions and product sampling/payoff remain separate.

## Accepted decision: explicit log returns and realized variance

Accepted on **2026-11-08** after whole-record review. Runtime,
caller/host reconciliation and independent numerical/financial qualification
remain pending; observed source behavior is retained separately below.

1. **Keep a strict positive-price domain and complete bar validation.** Every
   supplied price must be finite and strictly positive, including singleton/
   unused-for-estimation values before availability shortcuts. OHLC arrays have
   equal lengths and each bar satisfies L<=min(O,C)<=max(O,C)<=H. No filtering,
   partial bars, sorting, imputation, tolerance-based geometry repair or adjusted
   price inference. Prices/anchors share the caller's currency/adjustment basis;
   bars are ordered observations on one common clock. Zero/negative prices and
   terminal-zero log transforms error; this does not change accepted analytics
   simple-return terminal -1 semantics.

2. **Make annualization explicit and separate from dating/inference.** Require
   finite strictly positive annualization_factor A in return intervals/bars per
   year; remove Option/omission, the Daily/252 helper and frequency/date inference.
   A may be nonintegral. The result is A times the selected per-observation
   estimator, in annual decimal variance units; square root gives decimal
   volatility only where the caller requests it. There is no hidden percent/bp
   scaling, sample-calendar/calendar-day conversion or actual elapsed-time sum.
   Linear annual reporting does not prove temporal aggregation under dependence,
   stationarity or a product's contractual sampling schedule.

3. **Require an explicit OHLC method with one canonical parameter bundle.**
   Retain the five RealizedVarMethod variants and canonical snake_case labels;
   remove Default, OHLC_DEFAULT and missing/null-method selection. The standalone
   realized_variance(prices,A) remains the close-series entry point.
   realized_variance_ohlc takes one OhlcParams request containing open, high, low,
   close, method, annualization_factor and previous_close. This is one input
   bundle validated by the function, not mutable validated state, a method
   registry or new host class. No old flattened aliases/fallbacks. The dispatcher
   always validates the complete supplied bars, including CloseToClose; genuinely
   close-only data uses the standalone entry point. requires_ohlc remains a
   mathematical data-needs predicate, not permission for malformed dispatcher data.

4. **Make log_returns a checked adjacent-price transformation.** Return
   `Result<Vec<f64>>` with exactly N-1 ordered logs for N>=2 prices; valid empty/
   singleton input returns an empty vector after full price validation. Use one
   shared stable log-ratio ell(x,y)=ln(x/y): preserve equal-price+0, use a suitable
   log1p relative-difference path near equality and checked logarithm/exponent
   handling across wide scales. Do not form an overflowing/underflowing raw price
   ratio or erase adjacent-price changes through subtracting rounded standalone
   logs. All finite positive binary64 price pairs have representable mathematical
   logs (about +/-1454 at the extremes); fail only for a genuine required numerical
   failure, not an unnecessary ratio intermediate. No NaN holes or skipped returns.

5. **Use all target OHLC bars with an explicit preceding close where needed.**
   For CloseToClose and YangZhang require Some(previous_close), finite positive,
   representing the close immediately before bar0. It may lie outside bar0's
   [L,H] because an opening gap is allowed. Never infer it from the first open or
   close. For Parkinson/GarmanKlass/RogersSatchell require None and reject an
   irrelevant supplied anchor. Validate that conditional input even when n=0.
   All OHLC methods target the same n supplied bars; do not discard bar0 as a
   hidden anchor. Standalone N close levels intentionally cover N-1 internal
   intervals and therefore have a different window unless an anchor is prepended.

6. **Retain close-to-close as an uncentered squared-log second moment.**
   Standalone m=N-1 returns are r_j=ell(P_(j+1),P_j), and the estimate is
   A*sum(r_j²)/m. OHLC CloseToClose uses r_0=ell(C_0,previous_close) and
   r_i=ell(C_i,C_(i-1)) thereafter, returning A*sum(r_i²)/n over all target bars.
   Do not demean or apply a sample-variance/Bessel correction. This is the chosen
   realized-variance naming convention, distinct from statistical sample variance,
   the de-meaned classical benchmark in the Yang–Zhang paper, and the accumulated
   sum of squared returns for a particular horizon.

7. **Specify the three intrabar range estimators and nonnegative algebra.**
   For each bar set U=max(O,C), D=min(O,C),
   a=ell(H,U)>=0, b=ell(D,L)>=0, d=ell(U,D)>=0 and s=a+b.
   Parkinson's per-bar value is (s+d)²/(4*ln2).
   GarmanKlass is the paper's simplified practical Eq.19a,
   .5*ln(H/L)²-(2*ln2-1)*ln(C/O)², evaluated equivalently as
   .5*s²+s*d+(1.5-2*ln2)*d².
   RogersSatchell's ln(H/C)*ln(H/O)+ln(L/C)*ln(L/O) is equivalently
   a²+b²+d*(a+b). Each output is A times the mean of its per-bar values over all n
   bars, including bar0. Use these stable same-estimator nonnegative forms, not
   subtract nearly cancelling squares. GK is not the paper's full optimized
   .511/.019/.383 quadratic or an opening-gap extension; no alternate variant.

8. **Select the full-period practical Yang–Zhang definition explicitly.**
   For all i=0..n-1 form overnight o_0=ell(O_0,previous_close),
   o_i=ell(O_i,C_(i-1)) thereafter and intraday c_i=ell(C_i,O_i).
   Let V_o,V_c be their sample variances with means over n and divisor n-1,
   and R be the mean of all n RS per-bar values in decision7. Return
   A*(V_o+k*V_c+(1-k)*R), using accepted09 scaled centered moments.
   Select fixed practical k=.34/[1.34+(n+1)/(n-1)], evaluated stably as
   .34/[2.34+2/(n-1)] for n>=2. This is the paper's practical alpha=1.34
   specialization, not a universal minimum-variance weight for every drift/model.
   n counts complete target periods, not n-1 after dropping the first bar.

9. **Adopt explicit sample availability and genuine zero estimates.**
   Variance estimators return `Result<Option<f64>>`, with all data/control admission
   before None. Standalone closes need N>=2; OHLC CloseToClose and each range
   estimator need n>=1; YangZhang needs n>=2 plus its valid anchor. Short histories
   are unavailable, not zero or a different estimator. A genuine available zero
   remains Some(0). Close second moment zero means its observed returns are zero;
   Park/GK zero requires flat bars. RS can be zero on a moving monotone bar with
   a=b=0, and YZ can be zero with constant nonzero overnight/intraday logs and RS0.
   Do not transfer the ordinary-statistics all-prices-identical test to these
   different estimators, or claim estimated zero proves no risk/population variance.

10. **Evaluate the requested annual result stably without clipping.** Use
    checked compensated/scaled averaging, products and annualization, combining
    A, counts, weights and components before materializing an unnecessary overflow/
    underflow. Keep accepted09 moment/count conventions and cap actual estimator
    observation counts at min(usize::MAX,2^53); raw price transforms need only
    checked shape/allocation because they do not divide by a count. Shared
    log-derived components are bounded by the finite-price domain; do not justify
    stability with impossible 1e200 log-return fixtures. Final Some values must be
    finite nonnegative, with genuine zero normalized to+0. Negative computed
    components/results or detectably lost positive final variance are computation
    errors. Remove the -1e-14 allowance and max(0) floor; no epsilon risk cutoff,
    output clipping or price-error sentinel. Ordinary rounding remains qualified,
    not generally detectable or covered by a universal accuracy bound.

11. **Keep estimator assumptions separate from calculation and selection.**
    Parkinson and the selected GK benchmark rely on their zero-drift continuous-
    diffusion/extrema assumptions; neither includes an omitted opening gap.
    RS removes drift dependence under its Brownian log-price model but does not
    add previous-close/open variance. YZ includes its specified opening/intraday
    model components; it is not universally robust to intrabar jumps, microstructure,
    bad sampling or nonstationarity. Discrete recorded highs/lows are accepted
    observations but do not inherit continuous-extrema unbiasedness/efficiency.
    No universal best-method, bias correction, forecast or covariance claim is
    selected, and no diagnostics automatically substitute an estimator.

12. **Preserve truthful errors, resources and immutable request/output behavior.**
    Bad method/frequency, missing/irrelevant anchor, invalid price/bar/shape or
    count admission are input/domain errors. Required log, moment, arithmetic/
    range/output failure is computation, retaining context/index/method rather
    than labelling every failure bad prices. Validate before allocation/availability
    where possible; check lengths, n-1/workspace/capacity arithmetic and use fallible
    boundaries where available, without universal dependency/process out-of-memory
    recovery claims. Inputs are immutable; return one complete vector or optional
    scalar through existing error mechanisms, never partial NaN returns/results.
    No new persisted result/provenance/state or report framework.

13. **Reconcile canonical Rust/host callers and schema defaults together.**
    Keep Rust canonical names and method labels, Python snake_case and WASM
    camelCase. Hosts take explicit A for close-series calls and one required
    OHLC params object with all seven fields, previous_close=None/null for
    unanchored methods. Reject unknown/missing/retired keys, invalid method/null,
    wrong types, boolean numeric controls and numeric-string coercion. Convert
    once to canonical Rust admission without new host classes/calculations.
    Preserve supported sequence/typed-array forms and map availability to None/
    null, input failures to ValueError/domain and computation to RuntimeError,
    with matching existing WASM categories. Update stubs/exports/generated types,
    examples/tests and parity alongside all runtime signature changes. Reconcile
    serde/builder defaults using the shared method enum rather than restoring its
    removed Default through hidden literals or compatibility adapters.

14. **Keep product, data and financial risk policies at their owners.**
    Equity/FX variance-swap callers must supply the correct preceding observation,
    full sampling window and explicit method/A; generic adoption does not accept
    their contractual252 defaults, realized/forecast split, remaining variance,
    strike/payoff, observation weighting or corporate-action rules. Accepted
    analytics frequency, return availability and terminal-loss contracts stay
    unchanged. Owners establish common period length, source adjustments,
    chronological completeness and financial error budgets. Mathematical formulas,
    bounded reference cases and finite output do not certify market fitness,
    all-domain ULP/relative accuracy, bitwise library/host equivalence or a
    statistical confidence interval for the estimate. Acceptance does not execute
    or qualify a historical-volatility report or variance-swap valuation.

## Boundary examples for the accepted package

Here eta=2^-1074 is the smallest positive binary64 subnormal, and MAX is the
largest finite binary64 value.

| Input / situation | Required accepted outcome |
| --- | --- |
| Invalid A/method/shape/bar/price or conditional anchor, even with no eligible observations | Input error before availability; no omitted daily252/method default. |
| log_returns empty /one valid positive price | Empty vector after full input validation. |
| log_returns singleton0/NaN or prices ending at0 | Input error, no NaN hole/skip or simple-return terminal-loss reinterpretation. |
| Standalone closes with fewer than2 levels | None; two valid equal levels give Some(0). |
| Anchored OHLC CloseToClose one bar | One close/previous-close return is available; no hidden loss of bar0. |
| One complete range bar | Park/GK/RS available; YZ None after valid anchor admission. |
| previous_close outside first[L,H] | Allowed for anchored methods; opening gaps need not lie within the trading range. |
| Anchor omitted for CTC/YZ or supplied for Park/GK/RS | Input error; never infer or silently ignore it. |
| OHLC CloseToClose with empty malformed other legs | Input error; use standalone closes for close-only data. |
| Prices[minimum_subnormal,MAX], reversed | Finite logs about+/-1454.2227848147652 despite unusable raw ratios. |
| Adjacent large/near-binade prices | Preserve the nonzero tiny log change; no rounded-ratio doubling or equal-rounded-log false0. |
| Monotone bar O=L=1,C=H=2 | RS0, but GK/Park positive; zero is estimator-specific. |
| O=L=(1,2,4), C=H=(2,4,8), preceding close1 | YZ0 from constant logs/RS0; OHLC CTC=A*ln(2)^2. |
| Two flat bars100 then110, preceding close100 | YZ=A*ln(1.1)^2/2 over both bars; no old three-bar minimum. |
| Standalone closes[1,4,4,4,4],A=MAX | Finite MAX*ln(2)^2; do not annualize the largest term before averaging. |
| Standalone closes[1,16,16,16,16],A=eta | Positive result about1.921812eta, rounding to2eta; A/4 first must not erase it. |
| One Parkinson bar H16,L1,A=eta | Positive result about2.772589eta, rounding to3eta; no early annualization-factor underflow. |
| Negative computed variance or positive result collapsed to0 | Computation error, no -1e-14 allowance/floor. |
| Constant nonzero log returns | Close second moment is positive; do not substitute de-meaned sample variance. |
| Missing or irregular observation timing | Caller reconciliation; no date-index, resampling or frequency inference. |

## Observed choice and rationale

Five explicit estimators capture different information in close and range observations. The enum default is CloseToClose; the OHLC function's omitted method is YangZhang. These are distinct entry-point defaults, not a universal estimator choice. Formula names identify implementations and do not establish suitability for a given market microstructure.

## Observed algorithm

Let A be annualization periods per year, n the number of bars, and logs use price ratios.

1. Close-to-close forms `r_i=ln(C_i/C_(i−1))` and returns `A Σr_i²/(n−1)` for n prices. It does **not** subtract mean return or apply sample-variance correction.
2. Parkinson uses all n bars: `A mean[ln(H/L)²/(4 ln 2)]`.
3. Garman–Klass uses `A mean[0.5 ln(H/L)²−(2 ln 2−1)ln(C/O)²]` over all n bars.
4. Rogers–Satchell uses `A mean[ln(H/C)ln(H/O)+ln(L/C)ln(L/O)]` over all n bars.
5. Yang–Zhang requires n≥3, pairs bars i=1..n−1 with previous closes, and sets m=n−1. Compute sample variances of overnight `ln(O_i/C_(i−1))` and open-close `ln(C_i/O_i)`, plus mean Rogers–Satchell over the same m bars. Return `A[Var_overnight+k Var_openclose+(1−k)Mean_RS]`, where `k=0.34/[1.34+(m+1)/(m−1)]`.
6. `log_returns` returns an empty vector without checking fewer than two prices;
   otherwise it forms raw adjacent ratios, inserting NaN for unusable/nonpositive
   windows. The variance wrappers separately validate positive prices, but raw
   ratios still overflow/underflow and rounded log arithmetic loses small changes.

## Observed selection, controls and conventions

The caller supplies method and optional A; omitted A comes from `PeriodKind::Daily.periods_per_year()` (252 in this revision). OHLC validates all four arrays even when CloseToClose is selected. Decimal annualized volatility would be the square root of the returned variance; the helper does not take it.

## Observed limitations, failure and fallback

A must be finite and positive; every input price must be finite/positive. Bars must satisfy `L≤min(O,C)≤max(O,C)≤H`. Empty arrays return 0; close-to-close with fewer than two prices and Yang–Zhang with fewer than three bars return 0. Final variance below −1e-14 or nonfinite errors; tiny negative values are floored to 0. Price ratios can overflow even when individual prices are finite and positive, producing an error. No automatic estimator substitution exists.

## Observed alternatives and tradeoffs

All five alternatives are implemented. Range estimators consume intraperiod information, while Yang–Zhang includes overnight jumps and sample demeaning. A squared-return realized-variance contract differs from estimating variance after removing drift. No estimator is selected based on data diagnostics.

## Evidence

- [Owning implementation and inline tests](../../../../finstack-quant/core/src/math/stats.rs).
- `range_estimators_use_each_observed_bar_from_the_first` reconstructs the same formulas (1e-12 absolute) and checks one-bar inclusion; it is a convention regression, not independent market validation. `realized_variance_rejects_invalid_ohlc_and_annualization` and `realized_variance_defaults_are_rust_owned` cover errors and defaults.
- Dedicated [Parkinson](../../../REFERENCES.md#parkinson-1980-range-variance),
  [Garman–Klass](../../../REFERENCES.md#garman-klass-1980-range-variance),
  [Rogers–Satchell](../../../REFERENCES.md#rogers-satchell-1991-range-variance) and
  [Yang–Zhang](../../../REFERENCES.md#yang-zhang-2000-range-variance) entries now
  establish exact variants and primary provenance. GK's manuscript also states
  the Parkinson normalization. Primary GK/RS/YZ texts and publication metadata
  were consulted on 2026-11-08. The uncentered close definition, explicit all-bar
  request/anchor controls and strict failure/availability rules are scoped project
  choices, not all publication conventions or universal statistical guarantees.
- The YZ source consistently uses m=n-1 after dropping bar0; the selected package
  changes the request to include its own preceding close and all n target bars.
  Its practical .34 weight corresponds to paper (18), while (10) gives the
  general alpha-dependent expression. The source's 477–491 citation ends one page
  early; the published article spans 477–492. Reconcile citation/default claims
  alongside the new coverage rather than claim the current loop is internally
  inconsistent.
- [Python statistics](../../../../finstack-quant-py/src/bindings/core/math/stats.rs)
  and [WASM math](../../../../finstack-quant-wasm/src/api/core/math.rs)
  currently flatten optional method/A controls; [parity](../../../../finstack-quant-py/parity_contract.toml)
  needs matching canonical request/result/enum updates. The source OHLC Rustdoc
  says unused legs may be empty, but the actual body validates all four arrays;
  the proposal retains complete-bar validation and corrects that wording.
- [Equity swap types](../../../../finstack-quant/valuations/equity/src/variance_swap/types.rs)
  and [FX swap types](../../../../finstack-quant/valuations/fx/src/fx_variance_swap/types.rs)
  derive method defaults from the shared enum and separately default contractual
  trading-days-per-year. [Equity pricing](../../../../finstack-quant/valuations/equity/src/variance_swap/pricer.rs)
  and [FX pricing](../../../../finstack-quant/valuations/fx/src/fx_variance_swap/pricer.rs)
  already pass method/A explicitly. Reconcile schemas, anchors, window/availability
  and actual financial contracts; these observations do not adopt their defaults
  or reproduce a wrong swap value.

Independent 110-digit Decimal and exact-rational checks on 2026-11-08 confirmed
the nonnegative algebra against original formulas, both price directions, practical
YZ weight equivalence at n=2/3/10/2^53, moving zero cases and anchored two-flat-bar
target. A coherent O100/H120/L90/C110 bar gives per-bar Park/GK/RS about
.0298497119844/.0378713776952/.0370068176628. For previous close100 and flat100/110
bars, YZ at A1 is .004542015187166367. Wide-pair log is1454.2227848147652;
near-binade/adjacent-MAX log targets are about1.1102230246251565e-16, exposing
ratio-first doubling and equal-rounded-log loss. Full annual expression examples
give8.6370708474465935e307 and positive subnormal results2eta/3eta, rather than
false overflow/zero from unsafe operation order. These bounded formula/reference
checks (1e-100 high-precision algebra threshold) do not execute the proposed Rust
helper/params or certify an all-domain error bound. Independent semantic reviews passed.

Production Rust/host/product tests were located or inspected, not executed.
Log/annualization range and cancellation, full-bar anchor/parsing/error/availability,
schema/host and actual product reconciliation remain pending. Independent market
benchmarks, discrete-extrema bias, small-sample and financial precision assessment
are not established by formula-reconstruction or these bounded reference examples.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-numerics follow-ups](../FOLLOW_UPS.md#core-numerics).
