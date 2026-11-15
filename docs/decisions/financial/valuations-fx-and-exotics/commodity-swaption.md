# VALUATIONS-FX-AND-EXOTICS-COMMODITY-SWAPTION: Commodity swaption discounted-forward averaging and annuity Black-76

- Record: `VALUATIONS-FX-AND-EXOTICS-COMMODITY-SWAPTION`
- Decision status: **Accepted 2026-11-14** — complete commodity swaption forward, option, exercise, settlement and risk package.
- Implementation status: **Observed** in the owning paths below. Reconciliation to the accepted contract is pending.
- Validation status: Independent quantity-weighting, Black premium and parity, dated cash and curve-risk calculations checked. Runtime, market data, exercise and host qualification remain pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for commodity swaption pricing and exercise

1. **Book the option on the exact averaging swap.** Require a European
   exercise cut, holder long or short, payer or receiver right, strike, and
   explicit entry into the swap or cash settlement. Payer is a call on the
   commodity swap forward and receiver is a put. Reuse the accepted swap's
   booked period quantities, observation slots, benchmark and roll, and
   payment dates. The all-projected live profile requires the first actual
   observation after the exercise cut. A lookback can put it before the
   nominal swap start. Require qualified ordering for same-day phases.
   A booked premium is a separate trade cashflow.
   [Commodity averaging swap](commodity-average-swap.md#accepted-decision-for-commodity-averaging-swap-pricing-and-settlement).

2. **Weight the forward by discounted booked quantities.** For period
   quantity `Q_i`, dated payment factor `D_i` and qualified projected average
   `F_i`, use `A=ΣQ_iD_i` and `F=ΣQ_iD_iF_i/A`. Require finite positive
   weights and annuity with checked arithmetic. Invalid weights and required
   source failures error. Apply the annuity and holder sign once to the unit
   option premium. For the same underlying basis, the pay-fixed swap PV is
   `A(F−K)`.

3. **Use checked European Black-76 on the aggregate forward.** Require
   positive live `F` and `K`, a qualified positive exercise horizon and
   an explicit scalar volatility or matched swaption quote for the exact
   underlying schedule, expiry and strike. Preserve total variance in quote
   clock mapping. Exact zero volatility gives annuity-scaled intrinsic.
   This law models the aggregate forward as lognormal under the matching
   annuity measure. Signed period prices can contribute to a positive
   aggregate forward. A nonpositive aggregate forward or strike is outside
   this live model's domain.
   [Checked forward kernels](../models-analytic/forward-normal-shifted.md),
   [Järvinen and Toivonen](https://epub.lib.aalto.fi/pdf/diss/a238.pdf).

4. **Bind the cash payoff and resolve exercise separately.** Admit live cash
   settlement when the contractual closeout fixes the positive part of the
   elected swap's exercise-date value at the cut. For exercise date `E` and
   later payment `P` without accrual, multiply the live annuity price once by
   the deterministic exercise-to-payment factor `D(E,P)` on the coherent
   dated basis. Other cash formulas require a qualified model. At and after
   the cut require authorized exercise, assignment or lapse
   under the booked notice or automatic rules. Contract settlement activates
   the exact averaging swap with the executed direction and transfers it once.
   Cash settlement retains the authorized signed native amount on the actual
   due date through discharge. Eligible same-day unpaid cash uses factor one.
   Partial, failed or overdue cash requires a qualified residual basis.
   [ISDA Commodity Definitions, §§8.1 and 8.7](https://www.sc.com/en/uploads/sites/66/content/docs/2005-ISDA-Commodity-Definitions-and-Users-Guide-to-the-2005-ISDA-Commodity-Definitions.pdf).

5. **Separate local option Greeks and source risk.** Analytic delta and
   gamma are per aggregate forward-price unit and unit squared with annuity
   and evaluated volatility fixed. Vega is per `+0.01` annual volatility on
   the native quote clock, including its model-clock conversion. The live
   Greek bundle is unavailable at zero volatility. Commodity pillar scenarios
   have their own reprice basis. Signed parallel and bucketed discount DV01
   rebuild both the annuity and weighted forward. Known unpaid cash retains
   discount risk. The entered swap owns its commodity risk. Horizon theta
   across exercise requires a qualified closing state.
   [Dated curve risk](../valuations-framework/finite-difference-rates.md),
   [dated theta](../valuations-framework/carry-theta-breakeven.md).

6. **Use one resolved state in pricing and export.** Export authorized fixed
   cash as `DerivativeSettlement`. Delegate the entered swap's net cash to
   its owner. Complete cash export is unavailable while the option payoff is
   contingent. Confirmed lapse, discharge or completed handoff permits an
   empty option residual. Use canonical Rust pricing, state and cashflows
   through the existing Python and WASM instrument envelopes.
   [Cashflow export](../valuations-framework/cashflow-export.md).

## Independent calculations

- With quantities `1,000` and `250`, projected averages `64` and `92`,
  and payment factors `0.99` and `0.95`, the annuity is `1,227.50` and
  the weighted forward `69.4175152749`. Discount-only weighting gives
  `77.7113402062`. At strike `70`, volatility `0.32` and exercise time
  `0.75`, payer and receiver PV are USD `9,076.75595424` and
  `9,791.75595424`. Their difference equals USD `−715.00`, the underlying
  pay-fixed PV.
- With the same quantities, payment factors `0.98` and `0.95`, averages
  `65` and `88`, strike `71` and quote vol `0.28` on `0.8` quote years,
  payer PV is USD `7,632.33362217`. For rate-factor times `0.9` and `1.6`,
  signed centered parallel DV01 is USD `−0.94983286` per bp when both
  annuity and forward move, versus USD `−0.79112978` with the forward held.
- At an authorized payer exercise with quantities `1,000` and `250`,
  exercise-basis payment factors `0.98` and `0.95`, swap averages `71`
  and `74`, and strike `70`, a matched cash closeout fixes USD `1,930.00`.
  Its residual PV at a dated factor `0.996` is USD `1,922.28`. Eligible
  same-day unpaid cash uses the full fixed amount.

## Observed behavior

### Choice, rationale and scope

The inspected swaption prices a European option on one discount-weighted
commodity swap forward under Black-76.

### Algorithm and conventions

For underlying payment periods compute projected business-day average price `F_i`, payment DF `D_i`, annuity `A=ΣD_i`, and swap forward `F=ΣD_i F_i/A`. Quantities are per period, so no interest-style accrual factor multiplies annuity. Return arithmetic mean if the weight sum is nonpositive, but the Black input validator subsequently rejects nonpositive annuity when pricing.

At expiry the value is `Q A max(±(F-K),0)`. Before expiry Black-76 uses `A` in place of a single payment DF: `Q A [FΦ(d1)-KΦ(d2)]` for call. After expiry value is zero. Dates require expiry no later than underlying start, and underlying start before maturity.

Forward delta is `Q A Φ(d1)` (put signed form); at zero time/volatility use a strict intrinsic derivative. Vega is `0.01 Q A F n(d1)√t`; gamma uses percentage bumps to the whole price curve divided by `(F×bump)²`. Curve DV01 includes effects on payment annuity and weighted forward, through generic repricing.

### Selection and controls

Default/family selection is `Black76`. The same business-day average-period schedule as commodity swaps supplies the forward; the expiry/strike volatility is a single surface quote or active override.

Forward, strike and annuity must be positive and finite; volatility nonnegative, time nonnegative. Quantity positive; default time Act365F; calendar/BDC controls inherited averaging. Negative forwards error with guidance to use normal/shifted pricing, but those models are not actually implemented in this local type.

### Limitations, failure and fallback

One scalar swap volatility does not model individual forward-volatility/correlation structure. The annuity weights equal quantities per period. The arithmetic-mean fallback in the helper is not an accepted way to price invalid discount factors; pricing rejects the annuity afterward.

### Alternatives and tradeoffs

A normal or shifted-lognormal swap-rate model would cover nonpositive prices but is absent here. A joint forward-curve simulation would model delivery-period correlation at materially greater calibration cost.

### Implementation evidence

- [finstack-quant/valuations/exotics/src/commodity/commodity_swaption/metrics.rs](../../../../finstack-quant/valuations/exotics/src/commodity/commodity_swaption/metrics.rs)
- [finstack-quant/valuations/exotics/src/commodity/commodity_swaption/types.rs](../../../../finstack-quant/valuations/exotics/src/commodity/commodity_swaption/types.rs)

The original audit inspected source and inline assertions. The independent calculations above cover the stated examples. Production qualification is pending.

Canonical reference index. [Black 1976](../../../REFERENCES.md#black-1976).

## Follow-up tracking

Implementation reconciliation and qualification are tracked in the
[FX, commodity and exotic follow-ups](../FOLLOW_UPS.md#fx-commodities-and-exotics).
