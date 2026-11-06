# MODELS-AND-VALUATIONS-CREDIT-20: Exchange and liability-management arithmetic

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: Observed in the paths listed below; discrepancies are stated explicitly.
- Validation status: Evidence inspected or located, **not executed**; no external accuracy certification.
- Audit date: 2026-11-05.
- Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and rationale

Exchange-offer comparisons use supplied PVs and a fixed 2% tender hurdle; LME scenarios apply explicit balance-sheet arithmetic. These functions do not price the securities or legal priority changes.

## Scope and input domain

Nonnegative supplied exchange PVs/fees/equity sweetener; positive LME notional and acceptance fraction [0,1]. Repurchase price is a fraction of par; its meaning changes by LME type.

## Algorithm

1. Tender total=new PV+consent fee+equity sweetener; delta=tender total−old PV. Breakeven recovery=min(tender total/old PV,1), or one if old PV is zero. Recommend tender only when total exceeds 1.02×old PV.
2. Repurchase/tender retires notional×acceptance and costs retired par×price (price in (0,1.5]). Amend-and-extend retires no par and costs participating notional×fee (fee in [0,.10]). Dropdown records transferred-asset fraction in [0,1] as holder impact but no cost or debt retirement.
3. Discount capture=par retired−cost and fraction divides by retired par, zero if none. Positive optional EBITDA yields pre/post leverage and reduction from debt retirement.

## Selection and controls

ExchangeType is descriptive; no branch changes exchange arithmetic. LmeType chooses the scenario formulas. The 1.02 tender hurdle is a constant, not caller policy input.

## Conventions

PVs/fees and notional/EBITDA require consistent monetary units; price, acceptance and holder impact are fractions despite pct field names. Breakeven recovery is a ratio to old PV, not to claim par.

## Limitations, failure and fallback

Optional EBITDA is filtered only by >0, so positive infinity can produce zero ratios. Uptier/downtier labels do not model priority, dilution or participating/nonparticipating claims. Amend-and-extend discount capture is negative fee cost.

## Alternatives and tradeoffs

Pricing new/old instruments and passing their PVs is supported composition; issuer-strategy optimization and legal restructuring simulations are unimplemented.

## Evidence

Owner: [liability_management.rs](../../../../finstack-quant/models/src/credit/liability_management.rs) and [liability_management.rs tests](../../../../finstack-quant/models/tests/liability_management.rs) with formula/domain cases. No independent rationale for the 2% recommendation hurdle was established.

## Follow-up tracking

Adoption questions and implementation or validation work are tracked in the
[credit-model and valuation follow-ups](../FOLLOW_UPS.md#credit-models-and-valuations).
