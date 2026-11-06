# FX European option valuation, implied volatility and delta conventions

- Record: `VALUATIONS-FX-AND-EXOTICS-FX-VANILLA-DELTA`
- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, limited to the source paths below and their named delegated kernels.
- Validation status: **Evidence inspected/located, not executed**. Code-level formulas and test assertions are evidence of implementation; no external numerical certification or current test pass is claimed.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice, rationale and scope

European vanilla FX calls/puts use the Black–Scholes/Garman–Kohlhagen kernel with domestic discounting and foreign carry, equivalent to Black on the covered-interest-parity forward. The implementation also exposes spot, forward and premium-adjusted deltas as separate measures. The contract requires distinct base/quote currencies, finite nonnegative strike and positive base-currency notional; premiums are returned in quote currency. The type has no American/Bermudan selector.

The implementation establishes the choice; adoption, suitability and accuracy tolerances require agreement. Any tradeoff below is a proposed evaluation criterion, not evidence of prior approval.

## Algorithm and conventions

Set `t` from instrument day count and recover `r_d=-ln(D_d)/t`, `r_f=-ln(D_f)/t` from date-based DFs. For call, `V=N[S D_f Φ(d1)-K D_d Φ(d2)]`, with `d1=[ln(S/K)+(r_d-r_f+σ²/2)t]/(σ√t)` and `d2=d1-σ√t`; put follows the signed-CDF form. Past expiry returns zero; on expiry use intrinsic. Near-zero strike (`abs(K)<1e-12`) uses the discounted-asset call limit and zero put.

Spot delta is `N D_f φsign Φ(φsign d1)`; forward delta removes `D_f`. If premium is in base currency, premium-adjusted spot delta is `N (K/S)D_d φsign Φ(φsign d2)` and premium-adjusted forward delta is `N(K/F)φsign Φ(φsign d2)`; quote-currency premium leaves unadjusted delta. Degenerate zero-time/zero-strike branches define half delta at exact ATM where applicable.

The shared kernel supplies gamma, vega, theta, domestic rho and foreign rho. Vega/rhos are cash P&L per one percentage-point move; theta uses the configured days-per-year denominator. The delta-neutral ATM helper returns `F exp(+σ²t/2)` for unadjusted spot/forward delta and `F exp(-σ²t/2)` for premium-adjusted variants. Vanna central-differences analytic delta across active volatility scenarios and normalizes per vol point. Volga central-differences already-per-point vega and multiplies by 0.01 after dividing by decimal-vol width. The half bump is capped at half resolved volatility, and both measures are zero for nonpositive volatility or expired time. Convention metrics cache a single computed Greek bundle. The implied-volatility call requires positive spot, strike and notional and returns zero at nonpositive time, with price bounds/convergence delegated to the shared solver.

## Selection and controls

The default registry key is `Black76`, implemented in mathematically equivalent Garman–Kohlhagen spot form; volatility comes from the active implied-volatility quote override or the surface at option expiry/strike. Implied volatility calls the shared Black–Scholes inverse solver after dividing target cash premium by base notional. Existing ADR 0027 records model selection; this record covers formulation and quote risk.

Input time defaults to Act365F; explicit day count is honored. Date-based discount factors retain their curves' conventions before conversion to the effective rates above. Rates and annualized volatility are decimals; spot and strike are quote currency per base currency. The baseline does not calibrate during valuation. Spot comes from the base/quote FX matrix. Premium currency is validated against the pair; volatility override selection is shared with pricing and risk. Generic DV01/key-rate risk remains distinct from analytic one-percent rhos.

## Limitations, failure and fallback

The presence of four delta measures does not select a desk quotation or hedging convention. Zero-strike/zero-volatility and ATM degenerate Greeks follow the observed branches above, without implying adoption. Surface smile movement under spot risk is the implemented fixed-surface lookup behavior, not an approved sticky-delta model.

## Alternatives and tradeoffs

A delta-coordinate smile with recalibrated delta strikes can change hedging risk relative to fixed absolute-strike surfaces. Stochastic rates and smile dynamics are unimplemented here; alternate models must be explicitly selected elsewhere.

## Evidence

- [finstack-quant/valuations/fx/src/fx_option/metrics/delta_conventions.rs](../../../../finstack-quant/valuations/fx/src/fx_option/metrics/delta_conventions.rs)
- [finstack-quant/valuations/fx/src/fx_option/metrics/mod.rs](../../../../finstack-quant/valuations/fx/src/fx_option/metrics/mod.rs)
- [finstack-quant/valuations/fx/src/fx_option/pricer.rs](../../../../finstack-quant/valuations/fx/src/fx_option/pricer.rs)
- [finstack-quant/valuations/fx/src/fx_option/types.rs](../../../../finstack-quant/valuations/fx/src/fx_option/types.rs)

- [finstack-quant/valuations/fx/tests/fx_option/test_conventions.rs](../../../../finstack-quant/valuations/fx/tests/fx_option/test_conventions.rs): Explicit delta-neutral ATM strike identities for unadjusted and premium-adjusted conventions.
- [Put-call parity tests](../../../../finstack-quant/valuations/tests/instruments/fx_option/test_put_call_parity.rs) locate formula-identity evidence. The [QuantLib-labelled FX fixture](../../../../finstack-quant/valuations/tests/golden/data/pricing/quantlib/fx_option/eurusd_atm_call_3m_quantlib.json) carries provenance and tolerances whose generation was not requalified. Neither was executed in this audit.

Canonical reference index: [Garman Kohlhagen](../../../REFERENCES.md#garman-kohlhagen-1983); [Clark FX Options](../../../REFERENCES.md#clark-fx-options). The index supplies provenance pointers; agreement on the exact convention and validation domain remains open.

## Follow-up tracking

Adoption, implementation reconciliation and validation work is tracked in the
[FX, commodity and exotic follow-ups](../FOLLOW_UPS.md#fx-commodities-and-exotics).
