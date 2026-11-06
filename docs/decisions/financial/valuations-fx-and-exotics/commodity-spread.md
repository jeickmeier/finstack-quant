# Commodity spread option Kirk approximation and leg risk

- Record: `VALUATIONS-FX-AND-EXOTICS-COMMODITY-SPREAD`
- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, limited to the source paths below and their named delegated kernels.
- Validation status: **Evidence inspected/located, not executed**. Code-level formulas and test assertions are evidence of implementation; no external numerical certification or current test pass is claimed.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice, rationale and scope

A two-leg commodity spread call/put uses Kirk's effective-volatility approximation with expiry forwards and each leg's ATM-forward volatility.

The implementation establishes the choice; adoption, suitability and accuracy tolerances require agreement. Any tradeoff below is a proposed evaluation criterion, not evidence of prior approval.

## Algorithm and conventions

Let expiry forwards `F1,F2`, spread strike `K`, correlation `ρ`, and each leg volatility sampled at `(t,F_leg)`. Define `K_adj=F2+K`, `w=F2/K_adj` and `σ_K²=σ1²-2ρσ1σ2w+(σ2w)²`. Price `DF×Black(F1,K_adj,σ_K,t)×quantity`. Nonpositive effective variance becomes deterministic spread intrinsic. At expiry use `max(±(F1-F2-K),0)`; after expiry zero.

Reject nonpositive/nonfinite forwards, wrong volatility quote/axis, and `F2+K<=1e-10`. Correlation is validated in [-1,1]. Strike may represent a positive or negative spread but must satisfy the denominator domain at valuation.

Default delta is leg-one forward delta. Explicit `delta::leg1` and `delta::leg2` separately bump each price curve by ±relative bump, dividing by actual forward bump. Vega bumps both vol surfaces once (deduplicating a shared surface ID); leg-specific vegas bump only the named surface. Normalize by decimal-vol width×100 for one-vol-point P&L. Generic discount DV01/key-rate risk reprices the discounted value.

## Selection and controls

The default/family key is `Black76`, while the local algorithm is Kirk, not a single-underlying Black option. There is no automatic normal/MC fallback outside Kirk's domain.

Both price curves and strike-axis Black surfaces are required, plus one scalar correlation. Curve lookup is at expiry and volatility lookup at each leg's forward, so forward bumps can also move the sampled smile strike. Quantity positive; no quantity ratio beyond the supplied curve price units is inferred.

## Limitations, failure and fallback

Kirk freezes the effective-weight approximation and cannot handle `F2+K` near zero or negative lognormal forwards. Correlation is supplied externally; forward bumps move the sampled smile strike. Default delta is not net spread delta; simultaneous leg movements are a separate scenario.

## Alternatives and tradeoffs

Bivariate normal or two-factor Monte Carlo models could support negative prices and fuller distributions; they are named by the error message but not available through this local pricer.

## Evidence

- [finstack-quant/valuations/exotics/src/commodity/commodity_spread_option/metrics.rs](../../../../finstack-quant/valuations/exotics/src/commodity/commodity_spread_option/metrics.rs)
- [finstack-quant/valuations/exotics/src/commodity/commodity_spread_option/pricer.rs](../../../../finstack-quant/valuations/exotics/src/commodity/commodity_spread_option/pricer.rs)
- [finstack-quant/valuations/exotics/src/commodity/commodity_spread_option/types.rs](../../../../finstack-quant/valuations/exotics/src/commodity/commodity_spread_option/types.rs)

No separate independent numerical benchmark was established in this slice. Inline tests in the owning implementation provide only the assertions present there; they are not proof of accuracy outside those cases.

Canonical reference index: [Kirk](../../../REFERENCES.md#kirk-1995); [Haug](../../../REFERENCES.md#haug-2007-option-pricing-formulas). The index supplies provenance pointers; agreement on the exact convention and validation domain remains open.

## Follow-up tracking

Adoption, implementation reconciliation and validation work is tracked in the
[FX, commodity and exotic follow-ups](../FOLLOW_UPS.md#fx-commodities-and-exotics).
