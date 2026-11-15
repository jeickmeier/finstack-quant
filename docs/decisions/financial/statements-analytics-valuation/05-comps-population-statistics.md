# STATEMENTS-ANALYTICS-VALUATION-05: Comparable-company population, multiples and descriptive statistics

- **Decision status:** Accepted for the full comparable-company population, multiples and statistics contract (2026-11-15).
- **Implementation status:** Reconciliation pending.
- **Validation status:** Existing evidence mapped. Independent qualification pending.
- **Audit date/source revision:** 2026-11-05; `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
- **Scope:** [Inventory](README.md); [source coverage](coverage.json).

## Accepted decision for comparable-company populations and statistics

1. Use one validated peer population with unique company IDs and exclude the subject on every entry path. Apply all active filters, with alternatives allowed within each filter. Require finite, nonnegative, ordered market-cap bounds in a declared currency. Report reasons for exclusion, including missing data needed by an active filter.
2. Record the analysis date, source vintage, actual period bounds and financial basis. Match accounting periods and valuation scope, and use the same currency for both monetary amounts in a ratio. Comparisons of monetary levels across companies and market-cap screens require explicitly normalized amounts in a common currency. Identify the share class or debt security for each market quote. Reserve canonical metric names.
3. Keep all eleven definitions. Pair enterprise value with EBITDA, revenue, EBIT or unlevered FCF. Pair market capitalization with net income, book value, tangible book value or levered FCF on a common-equity basis. Dividend yield is dividends per share divided by share price, in decimal units. Divide OAS in basis points by leverage and express the result in basis points per leverage turn. Divide yield in percentage points by interest coverage and express the result in percentage points per coverage turn. Require finite operands and a positive denominator. Keep valid signed enterprise values, spreads and yields. Report usable peer counts and reasons for excluded observations.
4. Use equal-weight means, sample standard deviation and R-7 quantiles. A singleton supports location statistics and has unavailable sample dispersion. A z-score requires positive sample dispersion. Inclusive percentile rank is the count of usable peer values less than or equal to the subject value divided by the usable peer count, with ties counted in full.
5. Fit equal-weight ordinary least squares with an intercept on at least three matched peers and a varying predictor. Report the fitted value, actual-minus-fitted residual, fit population and whether the subject value extrapolates beyond that population's predictor range. A constant response gives a flat fit and unavailable R².

For example, EV/EBITDA values of 6×, 8×, 8× and 12× give a median of 8×. A subject at 8× has an inclusive percentile rank of 75%. If a fifth peer lacks EBITDA, show coverage of four out of five peers for that metric.

## Audited choice, input domain and selection

[Company metrics](../../../../finstack-quant/statements-analytics/src/analysis/comps/types.rs) are optional untyped `f64` fields plus custom names and attributes. `PeriodBasis` (LTM, NTM or custom) is a label, not a transformation. No currency, accounting-policy, fiscal-period or capital-structure harmonization is performed. Named getters prefer a populated named field; named scoring extractors do not substitute custom fields for a missing named value.

[Peer selection](../../../../finstack-quant/statements-analytics/src/analysis/comps/peer_set.rs) ANDs configured exact metadata filters and market-cap bounds. `from_universe` excludes the subject ID but does not deduplicate peer IDs; direct `PeerSet::new` keeps the supplied list, including any subject. Missing market cap fails a configured cap bound. Comparisons do not validate finiteness: a NaN cap or bound can evade ordinary `<`/`>` rejection. This is a confirmed static branch property, not an executed market-data example.

## Audited formulas and numerical controls

[Multiples](../../../../finstack-quant/statements-analytics/src/analysis/comps/multiples.rs) divide EV by EBITDA/revenue/EBIT/UFCF; market cap by net income/book/tangible book/levered FCF; dividends per share by share price; OAS in bp by leverage; and yield in percentage points by interest coverage. Denominators must be finite and strictly positive; numerators may have either sign but must be finite. Nonfinite resulting ratios are unavailable. Thus dividend yield is a decimal, spread-per-turn retains bp units, and yield-per-coverage retains percentage-point units.

[Descriptive statistics](../../../../finstack-quant/statements-analytics/src/analysis/comps/stats.rs) discard nonfinite values, use compensated sums for means and shared online sample variance (`n−1`) for dispersion, and use R7 linearly interpolated quartiles. Empty finite sets return unavailable statistics. Percentile rank is `count(peer <= subject)/n`, so all ties count in full; it is neither midrank nor interpolated percentile. Z-score requires at least two finite observations and standard deviation at least `1e−15`, and rejects nonfinite outcomes.

Regression helpers require equal-length all-finite paired inputs, at least three observations and positive predictor variance. OLS slope is `Cov(x,y)/Var(x)`, intercept `mean(y)−slope×mean(x)`, and R² is squared correlation. The subject residual is actual response less fitted response. Score construction and residual normalization are separately recorded in [06](06-comps-scoring.md).

## Audited alternatives and evidence

Negative numerator multiples, tie-full percentile ranking, finite-only descriptive populations and pairwise regression populations are explicit conventions. Winsorization, robust/MAD dispersion, sector accounting normalization, FX conversion and peer deduplication are not performed. They would change the selected population and must be adopted explicitly.

Colocated `finite_extreme_inputs_cannot_return_non_finite_statistics` establishes a numerical output guard; filter deserialization cases check defaults/unknown keys, not economic population quality. [Kahan](../../../REFERENCES.md#kahan-1965) and [Welford](../../../REFERENCES.md#welford-1962) contextualize numerical accumulation, not peer comparability.

Independent ranked-data and regression reference vectors were not established by this audit.

## Follow-up tracking

Outstanding decisions, implementation reconciliation and qualification work are tracked in the [scoped follow-up register](../FOLLOW_UPS.md#statements-analytics-valuation-comparisons-and-templates).
