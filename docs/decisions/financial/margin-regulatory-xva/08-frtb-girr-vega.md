# MARGIN-REGULATORY-XVA-008: GIRR vega on option-expiry and underlying-tenor axes

- Decision status: **Accepted 2026-11-17 for checked GIRR vega.**
- Implementation status: Existing behavior is described below. Checked five-vertex admission on both axes, mapped source evidence, expiry-only coverage and checked aggregation and result paths remain pending.
- Validation status: Primary references, grid, published weight and correlation rules checked. Independent examples covered volatility scaling, both maturity axes and all three scenario transforms. Runtime, schema, host and regulatory qualification remain pending.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and hashes: [inventory](README.md), [coverage ledger](coverage.json).

## Accepted decision for checked GIRR vega

1. **Require prepared, volatility-scaled vega.** Admit signed
   reporting-currency `σ × ∂PV/∂σ` from a qualified normal or lognormal
   model. Use matching volatility units for `σ` and its derivative.
   An implied volatility of `0.20` and vega of `50` per `0.01` of
   volatility give a prepared amount of `1,000`. Retain the source
   model and volatility convention.
   [FRTB input contract](01-frtb-inputs-and-wire.md#accepted-decision-for-checked-frtb-inputs-and-provenance) ·
   [Basel MAR21.25–28](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23).
2. **Use the five-point grid on both ordinary axes.** Admit `0.5Y`,
   `1Y`, `3Y`, `5Y` and `10Y` for option expiry and for the underlying's
   residual maturity measured from option expiry. Net matching risk
   currency, expiry vertex and underlying vertex. Retain source curves
   in provenance.
   [Basel MAR21.8(4)](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23).
3. **Require documented mapping from the sensitivity producer.** Use
   linear interpolation or a method consistent with the qualified pricing
   model. An option may map to one or more expiry vertices and one or
   two underlying vertices. Map caplets using their individual expiries
   and underlying periods. Assign `10Y` to an option without a specified
   maturity. Carry the applicable RRAO requirement for that case and the
   specified strike or barrier cases.
   [Basel MAR21.8 and MAR21.25–26](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23) ·
   [Residual risk record](13-frtb-residual-risk.md).
4. **Treat inflation and cross-currency basis vega as expiry-only.**
   Require a qualified aggregation rule under the elected profile for
   those exposures and their interactions with ordinary vega. Required
   expiry-only exposure makes the affected GIRR vega charge unavailable
   when the profile lacks that treatment.
   [Basel MAR21.8 FAQ 4 and MAR21.93](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23).
5. **Apply the `1.00` vega weight and the two-axis correlation.**
   The 60-day liquidity horizon gives the published 100% weight. For each
   axis use `r(x,y) = exp(−0.01 × |x−y| / min(x,y))`. Multiply the two
   coefficients, cap the product at one and transform it once per
   scenario. A `1Y × 5Y` factor paired with `5Y × 10Y` has Medium
   correlation `exp(−0.05)`, approximately `0.95122942`.
   [Basel MAR21.92–93](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23) ·
   [Formula in OSFI paragraph 205](https://www.osfi-bsif.gc.ca/en/guidance/guidance-library/capital-adequacy-requirements-car-2026-chapter-9-market-risk).
6. **Use checked currency-bucket aggregation.** Transform the base
   `0.50` cross-currency correlation to `0.375`, `0.50` and `0.625` for
   Low, Medium and High. Keep raw signed bucket sums for the first
   pass and cap them only on a negative-quadratic retry. Preserve mapped
   factors, coverage, weights and scenario evidence through Rust, wire,
   Python and WASM.
   [FRTB aggregation](02-frtb-scenarios-and-aggregation.md#accepted-decision-for-checked-frtb-scenarios-and-aggregation) ·
   [Basel MAR21.4, MAR21.6 and MAR21.95](https://www.bis.org/committees/bcbs/basel-framework/standard/mar/21/inforce/2023-01-01/published/2026-03-23).

## Choice and domain

This describes the observed design. No historical adoption rationale is inferred; the alternatives and tradeoffs below are the audit's assessment.

GIRR vega preserves two maturity dimensions within each currency. It accepts volatility-scaled monetary vega `sigma×dV/dsigma`; it does not multiply ordinary vega by market volatility internally.

## Algorithm and conventions

1. Multiply each `(currency, option_expiry, underlying_tenor)` input by fixed vega weight `1.00`.
2. For each axis define `r(x,y)=exp(−0.01×|x−y|/max(min(x,y),1/365))`. Multiply option-expiry and underlying-tenor correlations, cap that product at one, then apply the chosen scenario transform.
3. Use those pair correlations to calculate each currency's K and raw S.
4. Cross-currency gamma is scenario-transformed `0.50`; use the ordinary standard/conditional capped-S aggregator.

## Selection and controls

Selected by the GIRR vega branch, returning zero for an empty map. Both axes use the shared exact ten-label GIRR tenor lookup; hence all accepted points are at least 0.25 years and the 1/365 guard is not active for validated inputs. The weight is a fixed stored value; source provenance derives its cap from the labelled liquidity-horizon rule.

## Limitations and failure behavior

Current vega factors require both maturity labels and draw them from the ten-point GIRR delta grid. Upstream callers perform surface mapping. Expiry-only exposures need qualified treatment. Finite inputs can still overflow during quadratic aggregation.

## Alternatives and tradeoffs

Both maturity axes affect ordinary diversification. The accepted calculation uses prepared contributions at the prescribed vertices and the weight and correlations of the elected profile.

## Evidence and role

The inspected test covers two expiries at a shared underlying tenor, and the parameter test derives the weight from its liquidity horizon. Independent arithmetic checked the preparation scale, differing vertices on both axes and the Low, Medium and High transforms. Runtime verification and qualification of source mappings remain pending.
[Runtime and vega tests](../../../../finstack-quant/margin/src/regulatory/frtb/vega.rs) ·
[Input validation](../../../../finstack-quant/margin/src/regulatory/frtb/types.rs) ·
[GIRR parameters](../../../../finstack-quant/margin/src/regulatory/frtb/params/girr.rs) ·
[Parameter tests](../../../../finstack-quant/margin/src/regulatory/frtb/params/mod.rs) ·
[FRTB reference index](../../../REFERENCES.md#bcbs-frtb-minimum-capital-requirements).

Reconciliation and qualification work is tracked in the [regulatory-margin/XVA follow-ups](../FOLLOW_UPS.md#margin-regulatory-capital-and-xva).
