# VALUATIONS-FX-AND-EXOTICS-FX-VANILLA-DELTA: FX European option valuation, implied volatility and delta conventions

- Record: `VALUATIONS-FX-AND-EXOTICS-FX-VANILLA-DELTA`
- Decision status: **Accepted 2026-11-14** — complete European deliverable FX option and delta package.
- Implementation status: **Observed** in the owning paths below. Reconciliation remains pending.
- Validation status: Independent dated Black prices, parity, hedge amounts, delta-neutral strikes and risk examples checked. Runtime valuation, market data, lifecycle and host qualification remain pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked FX vanilla pricing and delta conventions

1. **Price the actual delivery forward under Black-76.**
   Select physically delivered European calls and puts as
   the baseline. Require the ordered pair, explicit long
   or short side, positive base notional and strictly
   positive strike. Expiry `E` determines the volatility
   clock. Delivery `L` determines the underlying forward
   and quote discount. Use the selected cash FX or the
   actual market spot value date `P` in the accepted
   spot-to-cash bridge. Gross quote PV is signed base
   notional times `D_Q(V,L)` times the unit Black price
   on `F_L`. Cash-settled profiles require their own
   qualified benchmark and payment rules.
   [Deliverable FX basis](fx-delivery-cip.md#accepted-decision-for-checked-deliverable-fx-and-covered-interest-parity) ·
   [Unit Black rules](../models-analytic/forward-normal-shifted.md) ·
   [Strata FX option pricer](https://raw.githubusercontent.com/OpenGamma/Strata/main/modules/pricer/src/main/java/com/opengamma/strata/pricer/fxopt/BlackFxVanillaOptionProductPricer.java).

2. **Resolve exercise and outstanding premium separately.**
   Require an authoritative exercise or nonexercise
   outcome at the expiry cutoff. A confirmed exercise
   leaves the fixed signed exchange in both native
   currencies at delivery. An unpaid booked premium
   retains its amount, currency and due date after
   either outcome. Resolve explicit same-day entitlement
   and value the surviving obligations on their qualified
   dated bases. Return zero residual once all obligations
   are discharged.
   [Strata FX option trade terms](https://strata.opengamma.io/trade_loader_fx_vanilla_option/).

3. **Separate PV delta from the four hedge conventions.**
   `delta` is the PV derivative per unit of the active
   spot quote with model volatility held fixed.
   `delta_spot` is the base amount of the spot hedge.
   Keep forward and both premium-adjusted hedge amounts
   separately identified. Use hedge date `H=P` for
   spot or `H=L` for forward. Divide the relevant PV
   derivative by `D_Q(V,H)` to obtain the hedge amount.
   A qualified base-premium adjustment subtracts
   signed gross option PV divided by `X_V * D_B(V,H)`
   from the hedge amount. Use the model premium in
   that adjustment and value actual booked premium
   cash separately. Select the reporting convention
   explicitly and preserve the source smile's quote
   convention.
   [Reiswich and Wystup](https://doi.org/10.1002/WILM.10132).

4. **Apply exact boundaries and the bounded IV inverse.**
   Remove the near-zero-strike shortcut. Exact zero
   volatility gives discounted forward intrinsic and
   the live analytic Greek bundle is unavailable.
   For positive time and volatility, with
   `w = sigma^2 * t_E`, the one-volatility
   delta-neutral straddle strike is `F_L * exp(w/2)`
   unadjusted and `F_L * exp(-w/2)` premium-adjusted.
   Invert the standalone gross quote-currency premium
   on its matching delivery basis under the accepted
   explicit solver controls and paired premium checks.
   Zero IV requires exact lower-bound equality at
   positive time. An expired claim has no identifiable IV.
   [IV solver rules](../models-analytic/implied-volatility-solvers.md).

5. **Keep local Greeks, market shocks and horizon risk on
   their own bases.** Report gamma per squared unit of
   the active spot quote and model vega per volatility
   point. Use the checked shared vanna and volga
   stencils with full admissible moves. Report local
   base and quote rhos per positive basis point with
   the actual spot-date and delivery-date effects.
   Full curve DV01 uses its own matched reprices.
   Use a dated horizon with eligible cash for theta.
   Hold a selected scalar volatility or absolute-strike
   grid fixed under market FX and rate shocks. For
   a native ATM/RR/BF source hold the raw quotes and
   rebuild the smile on each shocked delivery forward.
   Use one Rust lifecycle for pricing, risk and cash
   export through the existing typed Python and WASM
   routes.
   [Shared option risk](../valuations-framework/option-sensitivities.md) ·
   [Dated theta](../valuations-framework/carry-theta-breakeven.md).

## Observed behavior

### Choice, rationale and scope

European vanilla FX calls/puts use the Black–Scholes/Garman–Kohlhagen kernel with domestic discounting and foreign carry, equivalent to Black on the covered-interest-parity forward. The implementation also exposes spot, forward and premium-adjusted deltas as separate measures. The contract requires distinct base/quote currencies, finite nonnegative strike and positive base-currency notional; premiums are returned in quote currency. The type has no American/Bermudan selector.

The implementation establishes the choice; adoption, suitability and accuracy tolerances require agreement. Any tradeoff below is a proposed evaluation criterion, not evidence of prior approval.

### Algorithm and conventions

Set `t` from instrument day count and recover `r_d=-ln(D_d)/t`, `r_f=-ln(D_f)/t` from date-based DFs. For call, `V=N[S D_f Φ(d1)-K D_d Φ(d2)]`, with `d1=[ln(S/K)+(r_d-r_f+σ²/2)t]/(σ√t)` and `d2=d1-σ√t`; put follows the signed-CDF form. Past expiry returns zero; on expiry use intrinsic. Near-zero strike (`abs(K)<1e-12`) uses the discounted-asset call limit and zero put.

Spot delta is `N D_f φsign Φ(φsign d1)`; forward delta removes `D_f`. If premium is in base currency, premium-adjusted spot delta is `N (K/S)D_d φsign Φ(φsign d2)` and premium-adjusted forward delta is `N(K/F)φsign Φ(φsign d2)`; quote-currency premium leaves unadjusted delta. Degenerate zero-time/zero-strike branches define half delta at exact ATM where applicable.

The shared kernel supplies gamma, vega, theta, domestic rho and foreign rho. Vega/rhos are cash P&L per one percentage-point move; theta uses the configured days-per-year denominator. The delta-neutral ATM helper returns `F exp(+σ²t/2)` for unadjusted spot/forward delta and `F exp(-σ²t/2)` for premium-adjusted variants. Vanna central-differences analytic delta across active volatility scenarios and normalizes per vol point. Volga central-differences already-per-point vega and multiplies by 0.01 after dividing by decimal-vol width. The half bump is capped at half resolved volatility, and both measures are zero for nonpositive volatility or expired time. Convention metrics cache a single computed Greek bundle. The implied-volatility call requires positive spot, strike and notional and returns zero at nonpositive time, with price bounds/convergence delegated to the shared solver.

### Selection and controls

The default registry key is `Black76`, implemented in mathematically equivalent Garman–Kohlhagen spot form; volatility comes from the active implied-volatility quote override or the surface at option expiry/strike. Implied volatility calls the shared Black–Scholes inverse solver after dividing target cash premium by base notional. Existing ADR 0027 records model selection; this record covers formulation and quote risk.

Input time defaults to Act365F; explicit day count is honored. Date-based discount factors retain their curves' conventions before conversion to the effective rates above. Rates and annualized volatility are decimals; spot and strike are quote currency per base currency. The baseline does not calibrate during valuation. Spot comes from the base/quote FX matrix. Premium currency is validated against the pair; volatility override selection is shared with pricing and risk. Generic DV01/key-rate risk remains distinct from analytic one-percent rhos.

### Limitations, failure and fallback

The presence of four delta measures does not select a desk quotation or hedging convention. Zero-strike/zero-volatility and ATM degenerate Greeks follow the observed branches above, without implying adoption. Surface smile movement under spot risk is the implemented fixed-surface lookup behavior, not an approved sticky-delta model.

### Alternatives and tradeoffs

A delta-coordinate smile with recalibrated delta strikes can change hedging risk relative to fixed absolute-strike surfaces. Stochastic rates and smile dynamics are unimplemented here; alternate models must be explicitly selected elsewhere.

### Evidence

- [finstack-quant/valuations/fx/src/fx_option/metrics/delta_conventions.rs](../../../../finstack-quant/valuations/fx/src/fx_option/metrics/delta_conventions.rs)
- [finstack-quant/valuations/fx/src/fx_option/metrics/mod.rs](../../../../finstack-quant/valuations/fx/src/fx_option/metrics/mod.rs)
- [finstack-quant/valuations/fx/src/fx_option/pricer.rs](../../../../finstack-quant/valuations/fx/src/fx_option/pricer.rs)
- [finstack-quant/valuations/fx/src/fx_option/types.rs](../../../../finstack-quant/valuations/fx/src/fx_option/types.rs)

- [finstack-quant/valuations/fx/tests/fx_option/test_conventions.rs](../../../../finstack-quant/valuations/fx/tests/fx_option/test_conventions.rs): Explicit delta-neutral ATM strike identities for unadjusted and premium-adjusted conventions.
- [Put-call parity tests](../../../../finstack-quant/valuations/fx/tests/fx_option/test_put_call_parity.rs) locate formula-identity evidence. The [QuantLib-labelled FX fixture](../../../../finstack-quant/valuations/tests/golden/data/pricing/quantlib/fx_option/eurusd_atm_call_3m_quantlib.json) carries provenance and tolerances whose generation was not requalified. Neither was executed in this audit.

References for the original audit are [Garman and Kohlhagen](../../../REFERENCES.md#garman-kohlhagen-1983) and [Clark FX Options](../../../REFERENCES.md#clark-fx-options).

## Independent numerical checks

- For EUR 1 million, USD per EUR spot 1.20,
  quote/base factors `(.98,.99)` at market spot and
  `(.94,.96)` at delivery, strike 1.21, expiry time .5
  and volatility .20, the delivery forward is
  1.213152804642. Long call and put PVs are
  USD 65,693.688967 and 62,730.052603. Their difference
  of USD 2,963.636364 matches the value of the fixed
  delivery exchange. An independently unpaid EUR 2,500
  premium at base factor .997 has USD PV −2,960.787879.
- The same call's spot PV derivative is 508,890.365658
  per unit of the spot quote. Its spot and forward hedge
  amounts are EUR 519,275.883324 and 535,503.254678.
  The premium-adjusted amounts are EUR 463,413.902910
  and 477,895.587376. The unadjusted and adjusted
  delta-neutral strikes are 1.225345193027 and
  1.201081732548.

## Relationship to the accepted FX quote-source boundary

[CORE-MARKET-15's complete accepted package](../core-market/fx-delta-quotes.md#accepted-decision-one-explicit-fx-quote-profile-and-qualified-forward-inputs)
requires an explicit ordered pair and fixed forward-delta/DNS/arithmetic-smile-BF
profile for native FX quote sources. These quotation definitions are distinct
from the product's selected reporting/hedge delta. A premium-adjusted reported
delta can use a correctly interpreted unadjusted source; it does not convert or
retag its quotes. Active native-source lookup must use the actual compatible
delivery forward and source expiry clock. Reconcile the shared FX input collector
and volatility request rather than forwarding a zero tenor or a flat-rate
expiry substitute. Reject wrong-pair/ambiguous source association; reciprocal
FX requires qualified repricing/relabeling. Overrides and inactive branches
do not require unused source dependencies. The FX quote-source decision was
accepted on **2026-11-08**. This package resolves the product's date, risk
and lifecycle basis for reconciliation.

## Follow-up tracking

Implementation reconciliation and validation work is tracked in the
[FX, commodity and exotic follow-ups](../FOLLOW_UPS.md#fx-commodities-and-exotics).
