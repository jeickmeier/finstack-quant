# MARGIN-REGULATORY-XVA-008: GIRR vega on option-expiry and underlying-tenor axes

- Decision status: **Draft — observed implementation; adoption pending.**
- Implementation status: **Observed** in the stated source scope.
- Validation status: **Evidence inspected/located, not executed.** Selected test bodies were read; no runtime test, build or numerical experiment was run.
- Audit date: 2026-11-05. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- Scope and hashes: [inventory](README.md), [coverage ledger](coverage.json).

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

There is no curve-type dimension and no surface-to-regulatory-grid rebinning. The accepted label set spans more than a dedicated five-point option grid; this audit records validation behavior without asserting external compliance. Finite inputs do not protect quadratic products from overflow.

## Alternatives and tradeoffs

Collapsing either maturity axis would alter diversification. A runtime lookup from a versioned external schedule could support multiple rulebooks but would require stronger provenance/control than the fixed implementation.

## Evidence and role

Runtime: [src/regulatory/frtb/vega.rs](../../../../finstack-quant/margin/src/regulatory/frtb/vega.rs); weight and tenor mapping: [src/regulatory/frtb/params/girr.rs](../../../../finstack-quant/margin/src/regulatory/frtb/params/girr.rs). The located `girr_vega_applies_weight_and_mar21_89_maturity_correlation` test in vega.rs targets two expiry/underlying combinations; `params/mod.rs` recomputes the weight from its stated liquidity horizon. These verify selected formulas, not a surface calibration. [FRTB reference index](../../../REFERENCES.md#bcbs-frtb-minimum-capital-requirements).

The implementation establishes observed behavior, not financial adoption or regulatory compliance. Actionable work is tracked in the [regulatory-margin/XVA follow-ups](../FOLLOW_UPS.md#margin-regulatory-capital-and-xva).
