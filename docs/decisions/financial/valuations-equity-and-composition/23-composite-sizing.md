# VALUATIONS-EQUITY-AND-COMPOSITION-23: Composite fixed, notional, metric, volatility and expression sizing

- **Record:** `VALUATIONS-EQUITY-AND-COMPOSITION-23`.
- **Decision status:** Accepted 2026-11-14 — complete composite sizing, resolved state and rebalance date package.
- **Audit date/source:** 2026-11-05, `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Implementation status:** Observed in the owning paths below. Reconciliation to the accepted contract is pending.
- **Validation status:** Independent examples cover gross weighting, matched metric contributions, cross-currency unit-P&L volatility and calendar dates. Runtime sizing, historical data and host qualification remain pending.

## Accepted decision for checked composite sizing and rebalance dates

1. **Use authored fixed quantities.** Take the signed
   quantity of each declared leg lot exactly as supplied.
   Fixed initialization requires no market data. Require
   finite nonzero quantities. Across the sizing methods,
   check inputs, divisions, scaling and results, and remove
   the universal `1e-12` cutoff.

2. **Allocate a separate gross notional budget.** For a
   positive reporting-currency budget `G`, signed score
   `s_i` and positive owner-qualified notional magnitude
   `N_i` for the declared lot, use
   `q_i = G * (s_i / sum(abs(s))) / N_i`. Translate the
   notional at the rebalance cut using explicit FX on its
   declared currency and basis. Require a usable lot
   notional, keep strategy capital separate, and report
   the gross exposure achieved by the resolved quantities.
   [FX rules](../core-conventions/013-fx-resolution-and-shocks.md).

3. **Match metric direction, units and basis across legs.**
   Admit a signed owner-qualified measure that scales
   linearly with the declared leg lot. Match the shock
   direction, denominator, horizon and valuation and
   information cut. Translate monetary measures from
   their declared currencies. Standalone duration and
   yield ratios are ineligible. Keep the supplied
   anchor quantity `q_a` exactly and use
   `q_i = (s_i / s_a) * q_a * m_a / m_i` for other
   legs. A metric used as a divisor must be nonzero
   and usable. For neutrality, require both score sides
   and normalize positive scores to +1 and negative
   scores to −1. The sign of the anchor contribution
   `q_a * m_a` must match its score. This admits a
   quantity whose sign differs from the score when the
   metric is negative. Report the residual on the
   selected shock at the rebalance cut.
   [Valuation metric rules](../valuations-framework/metric-orchestration.md) ·
   [CME hedge-ratio example](https://www.cmegroup.com/education/courses/introduction-to-treasuries/treasuries-hedging-and-risk-management).

4. **Size to fixed-lot total-P&L volatility.** Use
   matched dated marks and entitled cash actually
   settled in each interval `(previous,current]`.
   Align marks with their entitlement and payment phase
   to count cash once. Translate marks at the observation
   dates and cash at its settlement date. Require
   complete qualified cash or confirmed zero cash.
   Use a common explicit sampling grid ending at
   rebalance and a contiguous tail of at most the
   lookback, meeting the stated minimum of at least
   two increments. Compute sample standard deviation
   times the square root of explicit observations per
   year. Keep the anchor quantity exactly and use
   `q_i = (s_i / s_a) * q_a * sigma_a / sigma_i`
   for other legs. Require usable positive sizing
   volatilities. The estimator measures the leg's own
   reporting-currency P&L dispersion. Portfolio
   covariance remains outside this method.
   [Sample statistics](../core-numerics/sample-statistics.md).

5. **Resolve exactly the scalar inputs requested by
   one expression per leg.** Use the checked core
   evaluator with declared date, score, valuation,
   ordered FX, qualified notional, metric and
   volatility inputs as referenced. Volatility uses
   the same explicit sampling and settlement rules.
   A genuine zero volatility can be an expression
   input. Unknown references, unavailable required
   inputs and unusable quantities fail the entire
   sizing request.
   [Scalar expression decision](../core-numerics/expression-evaluation.md).

6. **Freeze checked state and require exact rebalance dates.**
   Bind quantities to the canonical spec, ordered leg IDs,
   effective date and sizing inputs. Keep at least two
   legs per composite, at most eight composite levels
   including the root and at most 64 leg occurrences.
   Reused IDs across branches must match their canonical
   definition. Check owned state at construction and
   restore, including nested effective dates. Pricing
   and scenarios hold quantities fixed and reject
   future-effective state. Initialization establishes
   the first effective date. Manual rebalance requires
   an explicitly chosen later date. A scheduled
   rebalance requires an exact eligible later date.
   Authored eligible dates increase strictly. For a
   calendar schedule use a positive day, week, month or
   year cadence from the original anchor to an explicit
   inclusive raw end, with the selected calendar and
   adjustment convention. Roll before filtering on the
   adjusted query horizon. Coalesce equal adjusted dates
   into one event and reject decreasing results. Cap
   explicit dates and raw cadence candidates at 100,000.
   At schedule end the last holdings persist. Rust
   owns validation for the existing Python and WASM
   routes. Historical execution and financing stay with
   [record 24](24-composite-valuation-history.md).
   [Business-day adjustment conventions](https://strata.opengamma.io/apidocs/com/opengamma/strata/basics/date/BusinessDayConventions.html).

## Observed behavior

### Choice and rationale

A composite resolves quantities only during initialization/rebalance and stores those quantities as immutable pricing state between rebalances. Five explicit sizing methods support fixed units, gross notionals, anchored metric contribution, anchored unit-P&L volatility and caller expressions. Risk pricing does not silently resize the strategy.

### Scope and input domain

At least two legs, finite nonzero scores/quantities, positive capital in reporting currency; maximum nesting depth eight and total leg count 64. IDs must match embedded instruments; repeated IDs must have identical hashed definitions. Anchor quantities/inputs must exceed absolute tolerance 1e-12. Histories are strictly dated and cannot extend beyond rebalance.

### Algorithm

FixedQuantity uses scores directly. NotionalWeighted computes `qi=sign(si)×gross_notional×abs(si)/Σabs(scores)/abs(FXconverted_notionali)`; missing or near-zero instrument notional fails.

MetricWeighted prices every leg's selected unit metric. Additive monetary measures are converted to reporting currency; nonadditive measures retain their own units. Without neutralization, use raw scores. With neutralization normalize positive scores to sum+1 and negative scores to sum−1, requiring both sides. Set `qi=(si/sanchor)×qanchor×manchor/mi`; anchor quantity×metric must have its score's sign. The chosen metric's sign, conventions and applicability govern the resulting hedge.

VolatilityWeighted reprices each unit instrument over market history in reporting currency and forms `PnLt=Vt−Vprevious+cashflows_(previous,t]`, using endpoint market state for cashflow projection and date-specific FX. Use the last lookback P&Ls, require min_observations≥2 and history ending exactly on rebalance, then `σi=sample_std(PnL)×sqrt(annualization_factor)`. Set `qi=(si/sanchor)×qanchor×σanchor/σi`, rejecting zero/nonfinite volatility.

UserDefined compiles one expression per leg against scalar columns for scores, reporting values, FX, optional notionals and required metrics; when history has at least three dates it adds annualized full-history unit-P&L volatility. The first expression output is quantity; final state validation rejects zero/nonfinite quantities or stored inputs.

### Selection, controls and conventions

Caller supplies metric, anchor, lookback, minimum observations, annualization and expressions. Sizing options govern metric valuation; native unit-P&L history uses instrument.value. InitializeFixed needs no market only for fixed quantities. Rebalance rules are Manual, explicit increasing dates, or start-anchored day/week/month/year cadence with strict calendar lookup and business-day adjustment; duplicate adjusted dates are collapsed.

### Limitations, failures and alternatives

Unit-P&L volatility is currency volatility of one instrument, not percentage-return volatility or a covariance/risk-parity portfolio calculation. No covariance, transaction-cost or turnover optimizer is present. Neutralizing a nonadditive metric can be algebraically defined yet economically inappropriate. Sparse irregular observations use one supplied annualization factor rather than elapsed-time weighting. Calendar schedules are acted on at available observation dates in record 24.

### Evidence

The [composite tests](../../../../finstack-quant/valuations/src/instruments/composite/tests.rs) include gross notional normalization, delta-neutral anchoring, butterfly score normalization, unit-P&L volatility, expressions, retained quantities under metric pricing and cadence boundaries. These are algebraic/state evidence; they do not establish out-of-sample hedge effectiveness. Core expression/statistics/date algorithms are covered by their owning audits.

## Independent numerical checks

- Gross budget USD 9,000 with scores +2 and −1 and lot
  notionals USD 600 and EUR 250 at 1.20 USD per EUR
  gives quantities +10 and −10. The signed notionals are
  USD +6,000 and −3,000.
- Neutral scores −1, .25 and .75, measures −4, +2.5
  and +1 USD per common bump and anchor quantity +2
  give quantities 2, .8 and 6. Contributions are
  −8, +2 and +6 USD per bump.
- A USD leg marked at 100, 99, 96 and 100 with USD 3
  cash settled in the first interval gives P&Ls
  `[2,−3,4]`. A EUR leg marked at 160, 203, 157.6
  and 205 with mark FX 1.25, 1, 1.25 and 1 USD per
  EUR and EUR .8 cash at first-interval settlement FX
  1.25 gives USD P&Ls `[4,−6,8]`. At 52
  observations per year the annualized sample
  volatilities are 26 and 52. Opposite equal scores
  and anchor quantity +2 give the second quantity −1.
- An anchored monthly cadence from January 31 through
  April 15, 2025 produces raw dates January 31, February
  28 and March 31. With a weekend-only calendar,
  Preceding rolls Saturday March 1 into a Friday
  February 28 query. Following rolls February 1 and 2
  onto one February 3 event.

## Follow-up tracking

Implementation reconciliation and validation work are tracked in
the [equity and composition follow-ups](../FOLLOW_UPS.md#equity-and-composition).

## Owned source files

- [spec.rs](../../../../finstack-quant/valuations/src/instruments/composite/spec.rs)
- [spec_support.rs](../../../../finstack-quant/valuations/src/instruments/composite/spec_support.rs)
- [instrument.rs](../../../../finstack-quant/valuations/src/instruments/composite/instrument.rs)
