# VALUATIONS-RATES-16-CROSS-CURRENCY-SWAPS: Cross-currency coupons and resetting notionals

- ID: VALUATIONS-RATES-16-CROSS-CURRENCY-SWAPS
- Decision status: **Accepted — complete checked cross-currency swap cashflows, FX resets and risk package, 2026-11-14**.
- Implementation status: Runtime and caller reconciliation pending. Observed behavior below describes the current implementation.
- Validation status: Source, published references and independent arithmetic reviewed. Runtime, market, calibration and host qualification pending.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked cross-currency swap cashflows, FX resets and risk

1. **State both legs and the exchange election.** Admit two opposing
   floating legs in distinct currencies. Constant-notional modes use
   two positive exact Money notionals. Resetting mode uses one
   positive exact constant reference amount and the resetting leg's
   payment currency and explicit initial-notional election. Require
   opposite contractual holder directions, exact signed Decimal bp
   spreads, and expressly select None, Final, InitialAndFinal or
   MtMResetting. For MtM also select the resetting leg. Remove
   constructor and wire exchange defaults. Keep fixed-fixed,
   fixed-floating, and unsupported optional or amortizing variants
   outside this package. Require complete authored terms or a
   deliberately selected dated convention. The two legs can have
   distinct coupon grids.
   [Dated convention selection](../valuations-framework/convention-selection.md)
   and [Strata FX reset terms](https://strata.opengamma.io/apidocs/com/opengamma/strata/product/swap/FxResetCalculation.html).

2. **Prepare separate accrual, payment and observation terms.**
   Resolve each leg's exact tenor, grid, nominal and actual dates,
   stub, roll/EOM, day-count context, separate calendar roles,
   adjustments and nonnegative payment and fixing lags. An explicitly
   named weekends-only calendar remains usable. Remove fallback to
   it, remove the flag, and support an expressly different fixing
   calendar under the qualified convention. Reject unresolved terms
   or collapsed periods. Preserve prepared actual accrual boundaries
   without a second adjustment. Bind each benchmark's exact published
   source, identity and interval independently from the projection
   source. For term coupons hold the index accrual distinct from
   contractual accrual `A` and pay `ε N A(F+s)`. For compounded
   overnight coupons use the authorized arrears observation, block,
   lookback/shift and cutoff rules and pay `ε N[H+sA]`, where
   `H=Π(1+r_i w_i)−1`. Here `ε` is holder direction and `s` is
   decimal spread. Check each compound factor and cash boundary.
   A qualified zero coupon accrual contributes zero spread weight
   and may retain overnight growth cash. Arithmetic averaging and
   compounding the spread require another contract. Remove the
   generic negative-forward warning.
   [Accepted basis swap cash rules](04-basis-swaps.md#accepted-decision-for-checked-basis-swap-cashflows-and-par-spreads)
   and [overnight projection](../valuations-framework/overnight-projection.md).

3. **Resolve observation authority and cash entitlement separately.**
   Use an immutable explicit knowledge cut `K` with sufficient date
   and phase resolution, separate from valuation date `V`. For rates
   and FX use the exact eligible benchmark observations. When an
   observation is expressly pending, use only its qualified projection.
   A due missing observation errors, including same-date observations
   after publication. Missing inputs or an invalid quote do not select
   another date, series, currency orientation or source authority.
   Ordinary holder valuation excludes same-day cash. The explicitly
   entitled same-day view includes only known payable amounts at
   factor one. Prospective projected cash retains that status. Paid
   events need no inactive market inputs. Due but unsettled cash
   requires qualified handling. Determine the last payment from the
   actual schedule of potential coupon and principal claims, including
   delayed cash, without projecting them to learn the date.
   [Exact observed fixings](../core-market/scalar-series-and-fixings.md)
   and [dated cash valuation](../valuations-framework/cashflow-export.md).

4. **Bind one coherent valuation and FX basis.** Elect a qualified
   common collateral/funding basis `κ` and exact effective native
   discount factors `D_i(V,T)` for each currency on that basis. Bind
   live coupon projection to the exact index and the model's actual
   payment-date assumptions. Two available OIS curves alone do not
   establish the same collateral basis or a market-consistent FX
   forward. Any basis incorporated in the effective curves remains
   part of the valuation. Use the ordered conversion `X` in constant
   currency `C` per one unit of other currency `R`. For fixed
   notionals select the same ordered roles explicitly. Require
   positive valuation-date cash FX `X_V`, or convert an authorized
   later-value spot `S_P` at its actual value date `P` on the same
   basis using `X_V=S_P D_C(V,P)/D_R(V,P)`. An FX lookup, date
   policy or screen quote alone does not establish a cash FX basis.
   The coherent forward for actual FX value date `L` is
   `X_L=X_V D_R(V,L)/D_C(V,L)`. An alternative representation
   must reproduce that same model and dated factor basis. Require
   exact quote identity, orientation, range and source coverage.
   [Ordered FX and value dates](../core-conventions/011-fx-date-and-quote-conventions.md),
   [Fujii and Takahashi, section 5.1](https://arxiv.org/pdf/1508.06339)
   and [BIS on covered interest parity and basis](https://www.bis.org/publ/qtrpdf/r_qt1609e.pdf).

5. **Bind each FX reset to its actual contract dates.** On MtM keep
   the constant reference amount fixed. Require the common
   contractual term and explicitly qualified joint initial and
   final principal settlement dates. For each resetting period
   prepare its notional effective date, fixing date and authority,
   FX reference value date and actual exchange payment date under
   the declared calendars and lags. Forecast an expressly pending
   reset at that FX value date using decision 4. Use the positive
   exact admissible fixing when it is eligible. Elect either a
   contractually fixed initial resetting amount or the same FX
   reset law as later periods. Remove the first-date missing-fixing
   default, implicit pair-derived fixing series, raw/adjusted
   schedule date substitutions and placeholder notionals for
   omitted history. Compute `N_j=N_C/X_j` as checked positive
   Money on the actual notional rounding basis. Resolve both
   adjacent notionals whenever they feed an eligible rebalance,
   and each notional needed by an unpaid coupon or redemption.
   A delayed coupon keeps its own period notional. Require the
   necessary rate and FX observations to be available by the
   contractual determination cutoff. This is a deliberately
   selected deterministic FX and rates approximation. Reciprocal
   forward projection and multiplication by a projected rate
   omit stochastic FX/rate and payment timing effects. Model the
   full joint expectation only under a separately qualified law.
   [FX index observation and value date](https://strata.opengamma.io/apidocs/com/opengamma/strata/basics/index/FxIndexObservation.html)
   and [Fujii and Takahashi on MtM swaps](https://arxiv.org/pdf/1508.06339).

6. **Emit principal once on its actual dates.** For each leg of
   direction `ε`, constant cash is `−εN` at initial exchange
   and `+εN` at final exchange, only where elected. MtM emits
   `−ε_R N_0`, then `−ε_R(N_j−N_{j−1})` for each interior
   exchange and `+ε_R N_last` at final exchange. The constant
   leg has only its initial and final exchange. Apply the frozen
   holder sign to cash once. Maintain principal effects per leg
   and native currency, independent of cash sign. For a tracked
   leg with zero opening balance before funding, initial delta is
   `+N_0`, interior deltas are `N_j−N_{j−1}`, and final
   delta is `−N_last`. If the represented opening balance already
   contains principal, the funding row's added delta is zero.
   An opening tracked balance requires actual authority. Keep
   same-date and zero rows and their explicit principal dates.
   Keep complete contractual or historical views and eligible
   valuation views distinct. A slice without established opening
   stock leaves it unavailable. Remove the mixed-currency
   aggregate principal, notional and representative day count.
   [Strata FX reset exchanges](https://strata.opengamma.io/apidocs/com/opengamma/strata/product/swap/FxResetNotionalExchange.html)
   and [principal and cash semantics](../core-conventions/015-cashflow-semantics.md).

7. **Price and export the same native ledger.** Stage one
   prepared signed stream with per-row market dependency and
   discount authority. Discount eligible amounts at their actual
   payment dates in native currency. Translate the discounted
   `R` amounts into pricing currency `C` at valuation-date cash FX.
   The equivalent check for a fixed or prepared `R` payment
   amount `q` at `T` is `q D_R(V,T) X_V=q X_T D_C(V,T)`.
   For a different selected output currency convert the net PV
   once. Use the checked reciprocal of `X_V` for `R`, or the
   authorized coherent cash quote for a third currency. Specify
   that quote's shock treatment for risk. Accumulate with checked scaled
   arithmetic and preserve raw PV before final Money conversion.
   Honor actual contract cash rounding and preserve native
   row identity, factor, conversion quote, projected versus
   observed status, calendar dates and cut. Use the selected
   decomposition for both standalone export and price tracing.
   Reconcile totals against the selected raw and finalized PV
   with explicit budgets in the claimed pricing and output currencies.
   An unqualified decomposition stays unsupported.
   [Checked discounting](../core-conventions/016-discounting.md)
   and [cashflow export](../valuations-framework/cashflow-export.md).

8. **Use the quoted spread leg and the same calibration basis.**
   An authorized replacement quote experiment moves one explicit
   spread leg. Keep the other spread, cash dates, raw observations,
   rates, FX reset notionals and conversion fixed. On the affine
   cash law, the signed reporting-currency coefficient per bp
   is `W=ε X_leg Σ_j[N_j A_j D_leg(V,p_j)]/10000` across
   eligible spread-bearing coupons. Here `X_leg` is the selected
   valuation-date conversion to the quote reporting basis, or one
   for the same currency. Principal and settled rows carry no
   spread weight. Absolute replacement is `−V(0)/W`, and
   the incremental move is the replacement less the booked
   spread. A zero weight gives no identified quote. Use checked
   scaling for a finite nonzero weight. Test the
   represented result by actual repricing within an explicit
   reporting-currency residual. Contractual rounding can change
   the affine law and requires a qualified inversion or an
   unavailable result. Use this basis for the existing calibration
   path and keep the native product metric surface focused on
   price and risk. Align quote materialization, inception
   entitlement, signed quote-leg mapping and prepared raw residual.
   Separate trade-date cash FX from a later-settlement quote used
   to size principal. Quote inputs, calibration clocks,
   pillar and fit acceptance belong to their owners. Coupon basis
   and derived zero-rate differences retain their own definitions.
   [XCCY quote materialization](../calibration-framework/09-xccy-materialization.md),
   [XCCY target](../calibration-targets/07-xccy-calibration.md)
   and [derived basis diagnostics](../core-market/basis-spreads.md).

9. **Keep the price risk experiments distinct.** For `fx01`,
   apply the fixed directional ±1% experiment on the exact
   active ordered primitive quote or admitted direction set.
   Report the half difference in the stated PV currency.
   Rebuild affected forward resets
   and translation together from the same shocked source,
   holding realized fixings, contracted initial amounts and an
   expressly frozen presentation conversion fixed. A responsive
   presentation quote requires its own coherent directed shock.
   Treat reciprocal orientation as the same economic factor.
   MtM by itself proves
   no zero exposure. Combined `dv01`, typed per-source `pv01`
   and triangular `bucketed_dv01` shock active discount/projection
   sources with the live reset law reapplied. For a cash-FX input
   hold that quote fixed under rate risk. For a later-value spot
   input hold the actual spot quote fixed and reapply the dated
   bridge. Report per positive bp on actual represented steps.
   Qualify the FX×rates experiment on independent axes and the
   same four-corner raw reprices, per relative percentage point
   times bp. Require both finite positive calculation steps
   explicitly and reject failed sides. A quote replay or margin
   method requires its own authorization and qualification.
   [FX01 and volatility risk framework](../valuations-framework/option-sensitivities.md),
   [direct rate risk](../valuations-framework/finite-difference-rates.md)
   and [cross-factor risk](../valuations-framework/cross-factor-risk.md).

10. **Reconcile existing public consumers and qualify the model.**
    Keep canonical Rust owner validation through restore, valuation,
    risk and export, and through current generic Python and WASM
    routes and generated schemas. Reconcile portfolio, attribution
    and margin with the exact source directions, PV currency,
    principal and settlement cuts. Both observed and projected
    ledger paths need independent qualification through mixed
    calendars, same-day publication and delayed payments, nonzero
    collateral basis, reset precision, quote repricing and
    source-bound scenarios. Keep stochastic convexity and broad
    dealer convention coverage as separate qualifications.
    [XCCY wire](../../../../finstack-quant/valuations/schemas/instruments/1/rates/xccy_swap.schema.json)
    and [agreement-qualified margin](../valuations-framework/margin-bridge.md).

## Observed behavior

### Choice, scope and rationale

Cross-currency swaps project each floating leg in its native currency, discount with that leg's curve, then convert PV at valuation-date FX. Principal exchange is None, Final, InitialAndFinal, or MtMResetting; the constructor selects InitialAndFinal. Resetting notionals use deterministic covered-interest-parity forwards. Stochastic FX/rate covariance is not calculated.

### Algorithm and controls

1. Require different leg currencies, finite positive notionals and nonnegative payment/reset lags. Separate fixing calendars and simple-average compounding are unsupported. Explicit `allow_calendar_fallback=true` permits missing/unresolvable calendars to become weekends-only; otherwise they error. Simple overnight identifiers are rejected by shared validation.
2. Build each leg schedule from its actual tenor, stub, day count, payment and reset lags. For simple coupons, reset strictly before valuation requires the exact fixing; same-day/future resets project from the forward curve. Compounded coupons delegate daily observations, lookback/lockout handling and exposures to the shared overnight projector, after adjusted accrual boundaries.
3. Coupon is `side*N*((compound_factor-1)+spread_decimal*alpha)`; the spread is simple outside compounding. Constant-notional principal uses the opposite sign at start and coupon-side sign at end. Include only payments strictly after valuation, discount native cashflows and convert their PV at as-of FX. Neumaier accumulation reduces cancellation.
4. MtM requires common raw start/end dates but permits distinct coupon schedules. Orient FX as constant currency per unit resetting currency. Future FX is `X(t)=X(as_of)*D_reset(as_of,t)/D_constant(as_of,t)` and resetting notional `N_reset(t)=N_constant/X(t)`.
5. Historical resets needed by an unpaid coupon, future rebalancing or terminal redemption require exact FX observations. At initial start equal to valuation an absent fixing uses the supplied resetting notional. Irrelevant settled periods may use the supplied notional only as bookkeeping. Later valuation-date resets use the forward formula. Future notional changes create principal rebalancing flows with the initial-principal sign; the final flow returns the last resetting notional.
6. Initial-reset historical/same-day selection and fixing lookup use the raw common contractual start. A future initial CIP reset instead uses the constant leg's adjusted start, even if the resetting leg adjusts differently. Subsequent resetting notionals/rebalancing use the resetting period's raw accrual start. MtM PV discounts each resulting native flow then converts at as-of FX. Projected FX and rate observations are included in cashflow metadata.

### Risk and limitations

Registered risks are generic FX01, combined rates DV01, per-curve PV01, triangular bucketed DV01 and mixed FX/rates cross-gamma; their finite-difference kernels belong to valuations-core. The actual curve IDs and currency FX dependencies determine the factors. The constant-notional PV branch warns on forwards below -5% without rejection; this warning is absent from MtM projection. MtM discount factors and FX resets must be finite positive. Missing FX paths error. The common helper's curve conventions remain material dependencies.

### Alternatives and tradeoffs

Native discount plus spot conversion is the implemented valuation basis; payment-date conversion with a consistent reporting discount curve is an equivalent reconciliation only under matching curve/FX assumptions. A stochastic reset adjustment or collateral-currency/basis model is not selected here. Exact historical resets avoid replacing contractual state with current spot.

## Evidence

- [XCCY projection and PV](../../../../finstack-quant/valuations/rates/src/xccy_swap/types.rs), [MtM reset engine](../../../../finstack-quant/valuations/rates/src/xccy_swap/pricing_mtm.rs), [metric selection](../../../../finstack-quant/valuations/rates/src/xccy_swap/metrics.rs).
- [CIP invariance](../../../../finstack-quant/valuations/rates/tests/xccy_swap/cip_invariance.rs), [cashflow reconciliation](../../../../finstack-quant/valuations/rates/tests/xccy_swap/reconciliation.rs), [pricing tests](../../../../finstack-quant/valuations/rates/tests/xccy_swap/pricing.rs): independent formula/invariant checks where specified by each test; not a broad market benchmark.
- [MtM-reset reference](../../../REFERENCES.md#mtm-resetting-cross-currency-swaps) and [Fujii, Shimada and Takahashi](../../../REFERENCES.md#fujii-shimada-takahashi-2010).

## Evidence limits

No stochastic FX/rate reset-convexity validation is established.

### Independent review evidence

- Receive USD on a constant notional of `120) and pay EUR on
  the resetting leg. Fixings of`1.20` and `1.50` USD per
  EUR give EUR notionals `100,80`. USD principal cash is
  `−120,+120` and EUR principal cash is `+100,−20,−80`.
  At accrual`.25`, rates`.03,.05` and spread `.01`,
  pay EUR coupons`1,1.20`. A first coupon paid after the
  second reset still uses notional`100`. A supported same-day
  cut at the rebalance can include the known`−20` at factor
  one while the ordinary holder cut excludes it.
- USD per EUR spot `1.224` for later settlement at factors
  `.98` USD and `.99` EUR gives valuation-date cash FX
  `1.211636363636…`. At a payment with factors `.95` USD
  and `.97` EUR, the coherent forward is `1.237144497608…`.
  EUR `100` is worth USD `117.528727272727…` under either
  native discount and cash FX or forward conversion and USD
  discount. Using later-value spot as cash FX would give
  `118.728`.
- Coherent FX forwards guarantee conversion parity for the
  same cashflows. With zero coupons, USD constant notional
  `120`, cash FX `1.20`, initial known EUR notional
  `100` and an EUR pay leg, interim discount factors
  `.90` USD and `.95` EUR give reset FX `19/15` and
  EUR notional `1800/19`. With final EUR factor `.90`,
  the MtM principal PV is lower by `6/19` USD than the
  corresponding fixed-principal trade. The difference follows
  from discounted intermediate cash and changed redemption.
- Two equally likely payment-measure states with reset
  conversions in resetting currency per constant unit and
  coupon rates `(.8,.01),(1.2,.03)` give native coupon
  expectation `5,500` on one million reference notional and
  accrual `.25`. A product of the two means gives `5,000`.
  At factor `.98` the difference is `490` in native PV.
- On two coupon-only legs, receive USD one million at `.03`
  and pay EUR `800,000` at `.02` with accrual `.5`,
  factors `.96,.95` respectively and cash FX `1.20`.
  Zero pay-leg spread gives USD PV `+5,280` and signed
  weight `−45.6` per bp. Replacement pay-leg spread is
  `2200/19` bp, about `115.789473684`. With a booked
  `40` bp, the incremental move is `75.789473684` bp.
- With one-year native cash `+104` EUR and `−110` USD,
  factors `.95,.97` and cash FX `1.20`, USD PV is
  `11.86`. The ordered EUR-to-USD FX01 is `+1.1856`.
  The combined continuous-zero shock gives centered DV01
  about `−.001186`. The independent FX×rates stencil
  at ±1% and ±1 bp gives about `−.00011856` USD per
  percentage point times bp.

## Follow-up tracking

Adoption, reconciliation and validation work is tracked in the
[scoped follow-up register](../FOLLOW_UPS.md#rates-valuations).
