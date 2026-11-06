# VALUATIONS-FX-AND-EXOTICS-FX-TOUCH: FX one-touch/no-touch probabilities and payout timing

- Record: `VALUATIONS-FX-AND-EXOTICS-FX-TOUCH`
- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, limited to the source paths below and their named delegated kernels.
- Validation status: **Evidence inspected/located, not executed**. Code-level formulas and test assertions are evidence of implementation; no external numerical certification or current test pass is claimed.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice, rationale and scope

One-touch and no-touch contracts use a constant-coefficient lognormal hitting-time formula, with separate at-hit and at-expiry payout treatment.

The implementation establishes the choice; adoption, suitability and accuracy tolerances require agreement. Any tradeoff below is a proposed evaluation criterion, not evidence of prior approval.

## Algorithm and conventions

For an unbreached contract, define `μ=(r_d-r_f-σ²/2)/σ²`. Set `λ²=μ²+2r*/σ²`, with `r*=0` for expiry touch probabilities and `r*=r_d` for one-touch paid at hit. Let `z=ln(H/S)/(σ√t)+λσ√t`, `z′=ln(H/S)/(σ√t)-λσ√t` and `η=+1` down, `-1` up. The hit expression is `(S/H)^[-(μ+λ)]Φ(ηz)+(S/H)^[-(μ-λ)]Φ(ηz′)`. Multiply by payout; expiry payment adds `D_d`. No-touch expiry value is `D_d A(1-P_hit)`.

Current spot beyond the inclusive barrier is immediately hit: at-hit pays now, at-expiry discounts the payout. An already recorded breach instead means an at-hit payment has already occurred and contributes zero residual. At expiry explicit observed state decides the remaining payout; after expiry zero. Zero volatility delegates deterministic crossing probability rather than dividing by volatility.

Spot delta/gamma use shared central differences against FX-matrix bumps; vega normally bumps the active quote or surface symmetrically, capping a positive quote-override half-bump at half its volatility. At an exact zero-volatility quote override it uses the one-sided upward difference `(PV(h)-PV(0))/(100 h)`. Domestic/foreign rhos use curve bump repricing in basis-point units. This normalization differs from the vanilla/digital analytic one-percent rho and must be interpreted via the metric contract.

## Selection and controls

The touch analytic kernel is the local selected method. Continuous monitoring must have started; future monitoring windows error. Barrier-level volatility is selected through the active surface/override route.

Require positive spot/barrier and a quote-currency payout; explicit monitoring start and observed state are mandatory once seasoned. NoTouch with AtHit payout is rejected. The negative-rate at-hit formula rejects `λ²<0` rather than silently taking a complex/invalid square root. Shared finite-difference settings control spot/volatility/rate bumps.

## Limitations, failure and fallback

The formula distinguishes a settled recorded hit from a newly hit payout and rejects the stated negative-rate domain. It is constant-volatility/constant-effective-rate; surface lookup at the barrier is a frozen-volatility choice. No independent hitting-time numerical benchmark was established for the full negative-rate domain.

## Alternatives and tradeoffs

Numerical first-passage/PDE valuation could extend the domain and allow time-dependent coefficients. Such a replacement is not implemented. MC would require explicit hit-time discount handling and convergence checks.

## Evidence

- [finstack-quant/valuations/fx/src/fx_touch_option/metrics.rs](../../../../finstack-quant/valuations/fx/src/fx_touch_option/metrics.rs)
- [finstack-quant/valuations/fx/src/fx_touch_option/pricer.rs](../../../../finstack-quant/valuations/fx/src/fx_touch_option/pricer.rs)
- [finstack-quant/valuations/fx/src/fx_touch_option/types.rs](../../../../finstack-quant/valuations/fx/src/fx_touch_option/types.rs)

No separate independent numerical benchmark was established in this slice. Inline tests in the owning implementation provide only the assertions present there; they are not proof of accuracy outside those cases.

Canonical reference index: [Reiner Rubinstein](../../../REFERENCES.md#reiner-rubinstein-1991); [Haug](../../../REFERENCES.md#haug-2007-option-pricing-formulas). The index supplies provenance pointers; agreement on the exact convention and validation domain remains open.

## Follow-up tracking

Adoption, implementation reconciliation and validation work is tracked in the
[FX, commodity and exotic follow-ups](../FOLLOW_UPS.md#fx-commodities-and-exotics).
