# CORE-MARKET-09: Dated zero-rate differences and quoted basis boundaries

- Decision status: **Accepted — complete derived-diagnostic/removal package adopted 2026-11-08.**
- Implementation status: **Observed** in the owning paths identified below; the accepted source-pair diagnostic and standalone-curve removal are unimplemented.
- Validation status: **Primary/source evidence and independent financial, clock and consumer reviews inspected; standalone rational/high-precision counterexamples executed 2026-11-08; production qualification pending.** Existing Rust/host tests were inspected or located, not executed. Expression checks do not qualify the production query, XCCY calibration or a universal accuracy bound.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope: the implementation listed here; the [slice inventory](README.md) and [coverage ledger](coverage.json) identify exact file coverage.

## Scope and accepted authority

This decision owns the meaning, calculation and lifecycle of the quantity now
called BasisSpreadCurve. It preserves accepted [discount-source admission](discount-evaluation.md#accepted-decision-explicit-discount-sources-and-checked-rate-extraction),
[dated factor conditioning](rate-shocks.md#accepted-decision-explicit-zero-rate-shocks-and-dated-conditioning),
[parametric factor sources](parametric-discount.md#accepted-decision-validated-nsnss-sources-and-analytic-factor-queries),
[ADR0045 clocks](../conventions/0045-financial-clocks-and-date-boundaries.md)
and [annual decimal/bp units](../core-conventions/018-rate-units.md).

Actual signed swap-basis observations, contractual leg conventions, FX spot and
settlement, collateral/funding associations, calibration and quote risk retain
their existing owners. [XCCY materialization](../calibration-framework/09-xccy-materialization.md)
and [foreign discount calibration](../calibration-targets/07-xccy-calibration.md)
remain Draft. This diagnostic does not approve those pricing/calibration models,
infer covered-interest-parity consistency or identify a pure funding adjustment.

## Foundational findings

The optional XCCY output has the documented sign: foreign minus domestic. It
compares dated discount factors on one common reporting horizon. That full
zero-rate difference includes ordinary differences between currency interest
rates. It is distinct from a quoted coupon spread on a specified swap leg and
from a deviation from covered interest parity. Existing XccyQuote already
retains a convention, quote ID, far pillar and signed basis points on the
convention's base-currency floating leg; those observations must remain.

The output fabricates (0,0). Unit factors at an anchor identify neither zero
rate nor their difference. Independent interpolation then invents values that
need not equal the difference of the actual factor sources. Frozen translation
of sampled levels also fails to condition those sources at a new reference date.

A workspace search found the optional calibration byproduct as the numerical
producer and storage/restore/lifecycle uses, with no production pricing or risk
reader of BasisSpreadCurve::spread. That supports removing an unnecessary
financial source rather than adding roll modes and basis-risk machinery. The
search is source evidence, not a runtime reachability proof.

## Accepted decision: source-derived dated zero-rate differences

Accepted on **2026-11-08** after independent financial, clock and consumer
review. Runtime/caller/host reconciliation and independent production
qualification remain pending; observed implementation stays separate below.

1. **Remove the standalone interpolated source.** Retire BasisSpreadCurve,
   its builder and its market identity. Discount factors remain canonical for
   pricing; existing typed market quotes remain canonical for observations.
   Do not replace it with a generic scalar basis source, a basis-kind selector
   or another independently interpolated calibrated byproduct.

2. **Provide one explicit dated continuous-zero diagnostic.** Use a checked
   Rust zero_rate_difference query over two selected Discounting providers,
   with ordered domestic and foreign roles, anchor V, maturity T and an
   explicit common reporting day count plus its complete required context.
   Require T>V and finite positive alpha=tau_report(V,T). Report

   ```text
   ell_D = ln D_D(T) - ln D_D(V)
   ell_F = ln D_F(T) - ln D_F(V)
   s_F-minus-D(V,T) = (ell_D - ell_F) / alpha.
   ```

   D_j is the actually selected evaluator, including accepted retained views/
   shocks where present. The result is a signed annual decimal continuous
   rate. A positive result means the foreign rate exceeds the domestic rate
   when both are annualized on that same alpha. A bp display explicitly
   multiplies by10000 once, with checked units/range. Do not subtract own-clock
   zero quotes or infer a reporting clock, compounding, tenor or quote basis.

3. **Keep clocks and economic association explicit.** Each provider maps V/T
   using its own stored base, clock/calendar and complete evaluator. Different
   bases/day counts are eligible; querying both at the same numeric t does not
   align dates. The reporting clock supplies alpha; its required calendar or
   reference-period context must be present and valid. Require compatible
   resolved roles at the caller: exact source IDs, payout currencies and
   applicable collateral/funding assumptions. Reuse existing market/convention
   ownership, not ID substrings or a new core product registry. A report retains
   that ordered association, V/T and full reporting clock/context with its value.
   Different currency rates, or one payout currency under different collateral
   regimes, must be labelled as that actual comparison; neither automatically
   supplies a quoted XCCY basis or a funding decomposition.

4. **Admit factors before arithmetic and identities.** Admit all four absolute
   factor endpoints as finite and strictly positive under each selected
   source's actual domain/tails. Preserve accepted01/03/07 source/view admission;
   cancellation must not rescue an invalid absolute factor. Validate dates,
   reporting context and alpha before a same-source/equal-factor zero shortcut.
   Distinct dates with alpha=0 cannot identify a zero rate. T=V errors instead
   of returning an invented origin, tiny-tenor proxy or instantaneous rate.
   A separately requested short-rate comparison retains its analytic owners'
   derivative/clock meanings and is a distinct quantity.

5. **Compute the requested difference directly.** Reuse accepted01's private
   checked positive-factor log arithmetic. Evaluate the four-factor log
   cross-ratio with scaling and cancellation-aware arithmetic before dividing
   once by alpha; use nearby relative differences/log1p where appropriate.
   Do not first construct relative factors, multiply unscaled factors, subtract
   rounded cached logs or separately annualize enormous zero rates. Those
   unrequested intermediates can overflow, underflow or erase a representable
   difference. Preserve cross-source cancellation with scaled/compensated
   product differences when needed, not a fixed equality tolerance or a
   rounded-product-equals-one shortcut. Exact equality of admitted endpoint
   relationships may give0. Required nonzero log/rate results lost to zero or
   nonfinite arithmetic must be stably reformulated or fail; rounded source
   factors may already have lost an economic signal. Return Result with
   existing input versus computation error categories, propagating source
   failures. No public log-factor framework, arbitrary-precision runtime or
   all-finite-input/uniform-accuracy guarantee is added.

6. **Recompute when the anchor or sources change.** Frozen-source comparison
   at a new V uses the retained actual sources and the formula with the new
   explicit reporting horizon. It has no independent scalar translation, roll
   state, expired-knot removal, refit or (0,0) insertion. On an additive common
   source/report clock, with old differential s_B(t), a=tau_B(V), b=tau_B(T),

   ```text
   s_V(T) = [b*s_B(b) - a*s_B(a)] / (b-a).
   ```

   This explains the failure of frozen scalar levels; runtime uses the factor
   formula, not a sampled spread curve. For a nonadditive clock the denominator
   is actual tau_report(V,T), not an assumed difference of old coordinates.
   Reading old-view factors at V/T is distinct from constructing a new selected
   view with accepted03's absolute-factor admission. Do not bypass that boundary
   or rebuild a source on a new clock.

7. **Keep source risk and quote recalibration at their owners.** The diagnostic
   has no independent parameter pillars, bump target or stored market-change/
   attribution source. Direct discount-source shocks use accepted03 and
   recompute the comparison. Quoted-basis risk changes exact typed observations
   and recalibrates through the full selected instrument/dependency/fit contract;
   adding bp to the diagnostic or using its sign cannot identify that risk.
   [Record06 provenance](calibration-provenance.md) still distinguishes history
   from qualified current replay. This record does not adopt Draft XCCY solver,
   quote-risk or attribution methodology.

8. **Remove the byproduct and its transport paths together.** Delete
   basis_spread_curve_id, the Option<BasisSpreadCurve> output and its declared
   output, context/prior-market variant, storage/state/snapshot/restore and
   attribution collection. Remove old wire/roll state, re-exports, tests/examples
   and deliberately-unbound parity exclusions with those paths; no aliases,
   migrations or compatibility readers. A caller may explicitly request dated
   values from the completed foreign/domestic sources and retain labelled report
   rows. Rows have no market ID or subsequent interpolation. Calibration does
   not automatically evaluate extra maturities or construct a shadow curve.
   An explicitly requested diagnostic fails visibly on its own admission or
   arithmetic error; it cannot retroactively change the recorded fit result or
   fabricate a value. Preserve established structured calibration-envelope errors
   for calibration execution. Reconcile existing Rust/host report/context/schema
   surfaces; no standalone host basis-curve class or new reporting framework.

## Observed implementation and discrepancies

The builder requires base date and nonempty input, defaults to Act/365F,
Linear and FlatZero, and delegates admission to shared interpolation. Input is
not sorted; kernels ultimately require enough points. Only MonotoneConvex is
explicitly blocked, leaving PiecewiseQuadraticForward selectable for signed
levels. Logarithmic styles need positive levels. Builder knots replaces points.
There is no source-pair/currency/leg/unit association.

spread(t) evaluates the interpolator or retained original at t+offset. Invalid
queries/Error tails can return NaN. Roll changes base with a signed-day shift,
discards nonpositive remaining maturities and fails when all expire. Negative
cumulative offsets drop retained state and refit surviving points. Generic
context bumping returns UnsupportedBump. These behaviors are observed, not adopted.

The optional XCCY producer uses actual pillar dates and a common foreign-curve
reporting horizon, separately computes both zero rates, prepends (0,0), then
linearly interpolates with requested foreign-curve extrapolation. Date alignment
is intentional and the sign is correct. Artificial anchoring, independent
interpolation, unchecked results and scalar roll are the discrepancies.
Context/prior-market/attribution transport the object; attribution grouping under
FORWARD does not establish a projection role.

## Independent expression evidence and qualification limits

On **2026-11-08**, nine standalone cases were evaluated using standard-library
Fraction and 200-digit Decimal, without importing production code. These verify
expressions/counterexamples, not the production kernel:

- Flat common-clock domestic2%/foreign3% sources imply1%=100bp at every positive
  horizon. Linear (0,0)/(1,.01) instead gives50bp at half a year.
- For s_B(t)=.01*t^2, samples at1/2years are1%/4%; the linear midpoint is2.5%,
  while the source-derived value at1.5years is2.25%: a25bp error.
- For s_B(t)=.01*t, conditioning a=1 to b=2 gives3%; frozen evaluation gives2%.
- Over365 actual days, domestic2% on Act/365F and foreign3% on Act/360, reported
  on Act/360, give .03-.02*(360/365), approximately102.739726bp. Subtracting
  native-clock zero quotes instead gives100bp.
- Reversing the ordered pair negates the result; identical admitted endpoint
  relationships give0. Flat-source base normalizations cancel when the same
  actual V/T dates are queried. These were checked as three separate cases.
- With alpha=1e-308 and interval logs -2 and -2-2^-50, the combined rate is
  approximately8.881784197001252e292 although each individual annualized
  magnitude exceeds binary64 MAX. This is a log-expression range example,
  not a guarantee that nominal logs survive factor evaluation unchanged.
- Four admitted binary64 factors with cross-product (1+u)*(1-u), u=2^-52,
  have log cross-ratio ln(1-u^2), approximately-4.930380657631324e-32 at alpha1.
  Binary64 multiplication rounds to1 and falsely gives0. The high-precision
  result uses the exact represented factor values.

Independent production qualification must cover ordinary/negative rates,
resolved roles, bases/clocks, calendars and nonadditive/synthetic-zero reporting
horizons; Error tails, pre-origin/conditioned/shocked/parametric providers; equal
and near-cancelling endpoint relationships, subnormal/MAX factors, unrequested-
ratio range and tiny alpha; strict failures, bp conversion and source errors.
Qualify state removal, report metadata and actual Rust/host/calibration consumers.
Algebra, round trips and same-pricer calibration tests do not independently
certify market basis, collateral assumptions or repricing.

## Evidence and alternatives

Inspected owners: [basis_spread_curve.rs](../../../../finstack-quant/core/src/market_data/term_structures/basis_spread_curve.rs),
[shared roll machinery](../../../../finstack-quant/core/src/market_data/term_structures/common/roll.rs),
[context storage](../../../../finstack-quant/core/src/market_data/context/curve_storage.rs),
[XCCY producer](../../../../finstack-quant/calibration/src/targets/xccy_basis.rs),
[typed quotes](../../../../finstack-quant/calibration/src/quotes/xccy.rs),
[calibration schema](../../../../finstack-quant/calibration/src/api/schema.rs),
[prior-market wire](../../../../finstack-quant/calibration/src/api/prior_market.rs),
[attribution snapshots](../../../../finstack-quant/attribution/src/factors.rs)
and [host parity ledger](../../../../finstack-quant-py/parity_contract.toml).
Inline and [heterogeneous lifecycle tests](../../../../finstack-quant/core/tests/market_data/lifecycle.rs)
are implementation invariants; [XCCY tests](../../../../finstack-quant/calibration/tests/calibration/xccy_basis.rs)
use the production path. None was executed here.

The [BIS CIP/basis explanation](https://www.bis.org/publications/cip-fx-swaps-cross-currency-swaps-and-factors-move-basis)
and [CME CIP/basis example](https://www.cmegroup.com/articles/whitepapers/covered-interest-parity-implied-forward-foreign-exchange-swaps-cross-currency-basis-and-cme-estr-futures.html)
distinguish ordinary currency rate differences from a basis adjustment on a
specified leg. [Fujii–Takahashi, section5.1, equations5.2–5.4](https://arxiv.org/html/1508.06339)
retain payout/collateral currencies and distinguish OIS discount ratios from a
funding factor. Removing this source is this decision's architectural inference,
not a prescription from those references.

An independent supplied spread model would need a concrete pricing use,
explicit original-anchor meaning and financial parameterization. None was
located, so that alternative is not selected. Par-basis calibration, index basis
and currency funding models retain their owners rather than becoming modes of
this diagnostic.

## Follow-up tracking

Implementation reconciliation and independent qualification
are tracked in the [core-market follow-ups](../FOLLOW_UPS.md#core-market-data).
