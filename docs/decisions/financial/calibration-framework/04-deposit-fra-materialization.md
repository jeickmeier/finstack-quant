# CALIBRATION-FRAMEWORK-004: Deposit/FRA construction and prepared quote clocks

- Decision status: **Accepted 2026-11-15 — complete deposit and FRA construction, clock, residual and preparation package**.
- Implementation status: **Observed for the stated source scope.** Reconciliation to the accepted contract is pending.
- Validation status: **Independent calendar, deposit trade PV and FRA settlement calculations checked.** Production registry, calibration, pricing and host qualification remain pending.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (see [scope and coverage](README.md)).

## Accepted decision for deposit and FRA materialization and prepared clocks

1. **Bind the dated convention and actual curve roles.** Select the
   exact index and qualified convention edition. Preserve distinct
   trade, valuation, information and target curve-base inputs.
   Require positive exact notional in the contract currency, an
   admitted annual decimal quote and exact typed curve roles.
   Require an explicit shared-evaluator election where discount
   and projection use one evaluator. Retain the calendar coverage,
   roll, lags and day-count context.
   [Accepted convention selection](../valuations-framework/convention-selection.md).

2. **Resolve spot, contract dates and fixing once.** Apply the elected
   spot business-day lag from trade, excluding the origin when
   counting, and then the selected spot roll. Deposit starts at
   resolved spot. Both FRA tenor endpoints resolve independently
   from that same spot. Authored absolute dates distinguish raw
   dates subject to elected roll from exact effective dates.
   Builders consume resolved dates directly. Derive FRA fixing
   backward from resolved start on its own qualified calendar.
   Zero fixing lag preserves start and further roll requires an
   explicit rule. Reject invalid ordering, incomplete calendar
   coverage and collapsed accrual.
   [Accepted deposits](../valuations-rates/01-deposits.md#accepted-decision-for-checked-deposit-cashflows-and-quote-inversion) ·
   [Accepted FRAs](../valuations-rates/02-fra.md#accepted-decision-for-checked-fra-fixing-settlement-and-risk).

3. **Keep benchmark, contractual and target clocks distinct.** Bind
   FRA benchmark value dates and index accrual `αx` separately
   from contract accrual `α` and upfront payment at contract start.
   The admitted reference pillar for these profiles is resolved
   accrual end. Compute time from the explicit target base date
   on its own curve clock and complete required context. Remove
   curve-clock inference from quote indices and substitution of
   trade date for the curve base. Targets own control-knot placement
   and pillar collisions.

4. **Use the deposit equation appropriate to the target.** In
   discount calibration reuse the admitted deposit's checked raw
   trade PV, `N × [(1+rα) × D(V,E) − D(V,S)]`, including funding
   and the maturity receipt. Eligible same-day funding has factor
   one. Require positive `1+rα`. A deposit already funded at the
   cut is outside the inception fit. Keep the existing per-notional
   residual basis for discount targets. A qualified projection
   deposit uses `Px(Sx)/Px(Ex) − 1 − rαx` on its benchmark
   period. Its residual is growth. Target owners set weighting
   and acceptance.
   [Bank of England Working Paper 356, appendix C](https://www.bankofengland.co.uk/-/media/boe/files/working-paper/2008/measuring-monetary-policy-expectations-from-financial-market-instruments.pdf).

5. **Use the admitted live upfront FRA as a projection constraint.**
   Require the exact term benchmark and publication status. Only
   on the qualified unpublished basis project
   `F = [Px(Sx)/Px(Ex) − 1]/αx`. Use the shared raw upfront
   formula `s × N × α × (F−K)/(1+Fα)` with `s=−1` for the
   expressly selected receive-fixed calibration profile and
   require a positive denominator. Discount using the actual
   payment-date factor, with unit factor for a qualified same-day
   forecast. Fixed or settled FRAs are outside live projection
   calibration. A discount-only change leaves par PV zero when
   projection is fixed. The fitted curve must act in the actual
   projection role.
   [OpenGamma upfront FRA formula](https://raw.githubusercontent.com/OpenGamma/Strata/main/modules/pricer/src/main/java/com/opengamma/strata/pricer/fra/DiscountingFraProductPricer.java).

6. **Share qualified preparation across fit and replay.** Freeze the
   original quote ID and form, convention, resolved dates and accruals,
   side, pricing basis, pillar and exact bindings. Use this preparation
   for preflight, trial pricing, final repricing and replay. Reject a
   quote, instrument or basis mismatch instead of generic `value_raw`
   fallback. Preserve instrument-owned cash precision and calculate
   the raw residual before final PV Money conversion. The target
   and report owners retain residual weighting, solver controls
   and fit gates. Use the same validation in Rust, Python, WASM
   and schema admission.
   [Accepted calibration provenance](../core-market/calibration-provenance.md#accepted-decision-lossless-construction-records-and-request-time-replay-qualification).

## Independent calculation checks

- On a synthetic calendar a January 31, 2025 trade with two
  business days to spot gives February 5. A 1M×3M FRA starts
  March 5 and has adjusted accrual end May 6. Using the separate
  fixing calendar and two-day lag gives February 28. Using the
  market calendar for fixing gives March 3. Contract accrual is
  62/360 and target time from January 31 is 95/365.
- On notional 1,000,000 at 6% with accrual 0.25 and factors
  0.995 to start and 0.980 to end, deposit trade PV is −300.
  The maturity receipt alone has PV 994,700.
- Growth 1.012 over benchmark accrual 0.20 gives an FRA forward
  of 6%. On 1,000,000 receive-fixed at 5%, accrual 0.25 and
  payment factor 0.99, upfront cash is −2,463.05 and PV is −2,438.42.

## Observed choice and rationale

A quote becomes the same instrument type used by valuation, with dates resolved once and retained in `PreparedQuote`. The explicit deposit NPV branch avoids using the deposit's redemption-value convention as a zero-PV calibration objective. Shared construction reduces schedule drift; it does not make same-kernel repricing an independent benchmark.

## Inputs, stages and conventions

`BuildCtx` supplies as-of/trade date, numeric notional, required named curve roles, and optional CDS/OIS overrides. Missing curve roles fail. Index ID lookup in `ConventionRegistry` selects currency, calendar, business-day convention, settlement and reset lags, and day count. `resolve_spot_date` adds business days to as-of then applies the convention adjustment.

For deposits, tenor pillars start from spot and explicit dates are adjusted on the market calendar. The built deposit uses the convention currency/day count, decimal fixed rate, and required discount curve. For FRA, start and end pillars are independently resolved from the same spot date; fixing is start minus the convention business-day reset lag. It is a Receive-side FRA with explicit discount/forward roles, fixing calendar and convention. Float-to-decimal conversion errors propagate. Date ordering is enforced through instrument construction rather than an additional bespoke date repair here.

Prepared rate quotes retain the original quote in an `Arc`, the built instrument, resolved pillar date, and target-selected year fraction from base date to pillar. Deposit/FRA pillars are their end dates. `calibration_value_raw` calls deposit `npv_raw`; all other rates call instrument `value_raw`. This preserves numeric residual precision and does not prematurely round into `Money`. Swap payment-lag and futures reference-period pillars are detailed separately.

## Limits and alternatives

Convention data, builders and pricing kernels are external dependencies of this slice; this record does not certify exchange or ISDA compliance. A standalone closed-form calibration formula would give an independent implementation but risks drifting schedules; an external date/cashflow fixture would qualify the shared builder without duplication. Curve time, accrual day count and fixing date are distinct roles and must not be interchanged.

## Source and validation evidence

- Implementation: [build/context.rs](../../../../finstack-quant/calibration/src/build/context.rs) — required curve roles and overrides.
- Implementation: [build/helpers.rs](../../../../finstack-quant/calibration/src/build/helpers.rs) — business-day spot calculation.
- Implementation: [build/rates.rs](../../../../finstack-quant/calibration/src/build/rates.rs) — deposit/FRA dates and instrument orientation.
- Implementation: [build/prepared.rs](../../../../finstack-quant/calibration/src/build/prepared.rs) — cached pillar year fraction and deposit raw NPV override.
- Test evidence: [src/build/tests/rates.rs](../../../../finstack-quant/calibration/src/build/tests/rates.rs) — deposit/FRA fields and EUR/USD construction assertions; inspected or located, not executed.
- Test evidence: [src/build/tests/quote_construction.rs](../../../../finstack-quant/calibration/src/build/tests/quote_construction.rs) — quote-family construction smoke evidence; inspected or located, not executed.
- Test evidence: [src/build/prepared.rs](../../../../finstack-quant/calibration/src/build/prepared.rs) — deposit calibration-value convention regression; inspected or located, not executed.
- Reference context: [ISDA 2021 definitions](../../../REFERENCES.md#isda-2021-definitions) — convention context; no mapping of every registry entry to a versioned source is established; a catalogue entry is not proof of implementation equivalence.

## Follow-up tracking

Implementation reconciliation and missing evidence are tracked in the
[calibration-framework follow-ups](../FOLLOW_UPS.md#calibration-orchestration-solvers-and-replay).
