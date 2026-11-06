# Composite fixed, notional, metric, volatility and expression sizing

- **Record:** `VALUATIONS-EQUITY-AND-COMPOSITION-23`.
- **Decision status:** Draft — observed implementation; adoption pending.
- **Audit date/source:** 2026-11-05, `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the owned paths listed below; shared kernels are identified explicitly.
- **Validation status:** Source and linked evidence inspected or located; no numerical tests executed in this audit. Located tests establish the stated local behavior only, not independent accuracy across the domain.

## Choice and rationale

A composite resolves quantities only during initialization/rebalance and stores those quantities as immutable pricing state between rebalances. Five explicit sizing methods support fixed units, gross notionals, anchored metric contribution, anchored unit-P&L volatility and caller expressions. Risk pricing does not silently resize the strategy.

## Scope and input domain

At least two legs, finite nonzero scores/quantities, positive capital in reporting currency; maximum nesting depth eight and total leg count 64. IDs must match embedded instruments; repeated IDs must have identical hashed definitions. Anchor quantities/inputs must exceed absolute tolerance 1e-12. Histories are strictly dated and cannot extend beyond rebalance.

## Algorithm

FixedQuantity uses scores directly. NotionalWeighted computes `qi=sign(si)×gross_notional×abs(si)/Σabs(scores)/abs(FXconverted_notionali)`; missing or near-zero instrument notional fails.

MetricWeighted prices every leg's selected unit metric. Additive monetary measures are converted to reporting currency; nonadditive measures retain their own units. Without neutralization, use raw scores. With neutralization normalize positive scores to sum+1 and negative scores to sum−1, requiring both sides. Set `qi=(si/sanchor)×qanchor×manchor/mi`; anchor quantity×metric must have its score's sign. The chosen metric's sign, conventions and applicability govern the resulting hedge.

VolatilityWeighted reprices each unit instrument over market history in reporting currency and forms `PnLt=Vt−Vprevious+cashflows_(previous,t]`, using endpoint market state for cashflow projection and date-specific FX. Use the last lookback P&Ls, require min_observations≥2 and history ending exactly on rebalance, then `σi=sample_std(PnL)×sqrt(annualization_factor)`. Set `qi=(si/sanchor)×qanchor×σanchor/σi`, rejecting zero/nonfinite volatility.

UserDefined compiles one expression per leg against scalar columns for scores, reporting values, FX, optional notionals and required metrics; when history has at least three dates it adds annualized full-history unit-P&L volatility. The first expression output is quantity; final state validation rejects zero/nonfinite quantities or stored inputs.

## Selection, controls and conventions

Caller supplies metric, anchor, lookback, minimum observations, annualization and expressions. Sizing options govern metric valuation; native unit-P&L history uses instrument.value. InitializeFixed needs no market only for fixed quantities. Rebalance rules are Manual, explicit increasing dates, or start-anchored day/week/month/year cadence with strict calendar lookup and business-day adjustment; duplicate adjusted dates are collapsed.

## Limitations, failures and alternatives

Unit-P&L volatility is currency volatility of one instrument, not percentage-return volatility or a covariance/risk-parity portfolio calculation. No covariance, transaction-cost or turnover optimizer is present. Neutralizing a nonadditive metric can be algebraically defined yet economically inappropriate. Sparse irregular observations use one supplied annualization factor rather than elapsed-time weighting. Calendar schedules are acted on at available observation dates in record 24.

## Evidence

The [composite tests](../../../../finstack-quant/valuations/src/instruments/composite/tests.rs) include gross notional normalization, delta-neutral anchoring, butterfly score normalization, unit-P&L volatility, expressions, retained quantities under metric pricing and cadence boundaries. These are algebraic/state evidence; they do not establish out-of-sample hedge effectiveness. Core expression/statistics/date algorithms are covered by their owning audits.

## Follow-up tracking

Pending decisions, implementation reconciliation and validation work are tracked in
the [equity and composition follow-ups](../FOLLOW_UPS.md#equity-and-composition).

## Owned source files

- [spec.rs](../../../../finstack-quant/valuations/src/instruments/composite/spec.rs)
- [spec_support.rs](../../../../finstack-quant/valuations/src/instruments/composite/spec_support.rs)
- [instrument.rs](../../../../finstack-quant/valuations/src/instruments/composite/instrument.rs)
