# CALIBRATION-FRAMEWORK-005: Listed rate futures and reference periods

- Decision status: **Accepted 2026-11-15 — complete listed rate-futures terms, reference windows, convexity, residual and preparation package**.
- Implementation status: **Observed for the stated source scope.** Reconciliation to the accepted contract is pending.
- Validation status: **Independent date-role, observation-block, quote and sizing checks completed.** Production venue data, projection, calibration and host qualification remain pending.
- Audit date: **2026-11-05**; source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (see [scope and coverage](README.md)).

## Accepted decision for listed rate-futures calibration and reference periods

1. **Resolve the listed identity before deriving dates.** Bind venue,
   product, contract identity and its month role, index, currency and
   calendars from a qualified dated edition or complete authored terms.
   Resolve reference interval, fixing or observation schedule,
   last trading session, expiration, final mark and clearing payment
   as distinct terms. Reject conflicts. Remove the generic expiry roll,
   month inference from rolled expiry and compounding-based payment date.
   [Accepted rate-futures terms](../valuations-rates/10-rate-futures.md#accepted-decision-for-checked-rate-futures-and-futures-option-risk).

2. **Retain the four expressly selected reference-period profiles.**
   - Forward-starting uses the qualified venue and index value-date
     rule, tenor and roll. A venue-defined last trading or fixing
     session supplies that event's date.
     [ICE three-month Euribor](https://www.ice.com/products/38527986/Three-Month-Euribor).
   - IMM-quarter in arrears uses actual third-Wednesday endpoints
     under the qualified month role. CME SR3 names the delivery
     month in which the reference quarter ends. Derive last trading
     under the venue rule.
     [CME SR3 rulebook](https://www.cmegroup.com/rulebook/CME/IV/400/460.pdf).
   - Calendar-month in arrears uses first calendar day through first
     calendar day of the following month. CME SR1 uses arithmetic
     SOFR and ends trading on the last business day of its month.
     [CME SR1 rulebook](https://www.cmegroup.com/rulebook/CME/IV/400/461.pdf).
   - Business-month in arrears uses first business day through first
     business day of the next month on the qualified reference calendar.
     Montréal COA compounds CORRA on that interval.
     [Montréal Exchange COA terms](https://www.m-x.ca/en/markets/interest-rate-derivatives/coa).

3. **Use qualified benchmark observations at the information cut.**
   Elect term simple, weighted overnight arithmetic or overnight
   compound under the contract. Preserve eligible published observations
   and project unresolved observations with the exact active source.
   Use actual benchmark business-day blocks and overlapping weights,
   including weekends and holidays. Remove the synthetic fixing at
   reference end minus one calendar day. Fully fixed, final or settled
   contracts are ineligible as live projection constraints.
   [Accepted observation basis](../valuations-rates/10-rate-futures.md#accepted-decision-for-checked-rate-futures-and-futures-option-risk).

4. **Fit theoretical points to the observed price.** Retain original
   observed IMM points `P_obs` and quoted rate `(100−P_obs)/100`.
   Require explicit finite signed convexity adjustment `CA` in annual
   decimal units, including literal zero when selected. Subtract it once
   in the projected-rate seed. Freeze it across trials. Compute unrounded
   `100 × (1−R_projected−CA)` using the final annualized benchmark,
   including partly realized contracts. Remove sensitivity scaling,
   default-zero restoration and generic valuation fallback. Require the
   candidate curve to supply actual live projection. The undiscounted
   live mark has no discount-only calibration exposure.

5. **Use exact point difference and elected cash scaling.** Use the
   authorized cash-per-point multiplier. Where defined by the venue's
   tick cash and tick size, check their ratio on the actual applicable
   regime. Preserve `P_model−P_obs` in points. For the existing cash
   scale multiply by point value and positive synthetic long quantity
   `calibration notional / declared face normalization`. Divide by
   calibration notional for the elected per-notional residual. Fractional
   synthetic quantity is fit scaling. Trading lots, tick rounding and
   clearing cash retain their venue rules. Targets own weights and
   fit acceptance.

6. **Use reference end as the prepared pillar.** Evaluate time on the
   target's explicit base date and full clock context. Preserve both
   endpoints for target-owned solver geometry. Freeze exact quote identity,
   original price and adjustment, venue edition, observations, multiplier
   and typed bindings across preflight, trial, final repricing and replay.
   Match replay by exact identity and terms. Remove nearest-expiry
   lookup and use the same Rust admission across Python, WASM and schemas.
   [Accepted calibration provenance](../core-market/calibration-provenance.md#accepted-decision-lossless-construction-records-and-request-time-replay-qualification).

## Independent calculation checks

- July 2026 Montréal COA has reference `[July 2, August 4)` and
  last trading July 31. The final overnight block spans four calendar
  days. The synthetic end-minus-one shortcut selects the August 3
  holiday.
  [Trading calendar](https://www.m-x.ca/en/trading/data/trading-calendar?cat=0&month=7) ·
  [2026 exchange holiday circular](https://www.m-x.ca/f_circulaires_en/145-25_en.pdf).
- Observed price 98.50 and convexity +2.5 bp give projected seed
  1.475%. Trial projection 1.480% gives 98.495 points. With 2.5
  synthetic units at 2,500 currency units per point, residual is
  −31.25, or −0.0000125 per unit of the 2.5 million calibration notional.

## Observed choice and rationale

Futures retain listed price and contract economics rather than being reduced to a generic FRA. Reference period, expiry, fixing and settlement are constructed separately. The code's rationale is observable in explicit variant handling and date checks; no claim of current exchange compliance follows.

## Ordered construction

1. Resolve the futures contract and its rate index. Adjust supplied expiry on the contract calendar using the index business-day convention.
2. ForwardStarting: add contract settlement business days, adjust, then add delivery months. ImmQuarterInArrears: period end is the third Wednesday of expiry month; expiry must equal its preceding business day; period start is the third Wednesday delivery-months earlier. CalendarMonthInArrears: use first calendar day to next month first, with expiry the last business day of the reference month. BusinessMonthInArrears: enforce the same expiry, but adjust both month boundaries Following.
3. Simple-compounding fixing is period start minus index reset-lag business days. Other compounding uses period end minus one calendar day. Settlement is expiry for simple futures and period end otherwise.
4. Build `ListedFutureTerms` long, with contracts = calibration notional / face value, multiplier = tick value / tick size, convention currency, and the original quoted price. Face value, tick size, delivery months and supplied convexity adjustment are retained in `FutureContractSpecs`; quote authoring defaults convexity to zero. Explicit discount/forward IDs and index/compounding metadata are preserved.
5. The prepared pillar is **reference-period end**, not expiry. Its year fraction uses the target curve clock. Pricing and quote-bump conversion are owned by the instrument/quote modules, not recomputed here.

## Limits and alternatives

Wrong expiry for an arrears contract fails; the builder does not silently infer a new contract month. The calendar-month computation is performed after expiry adjustment. Alternatives are synthetic FRA approximation (simpler but drops listed economics) or direct exchange settlement schedules (stronger provenance, larger data contract).

## Source and validation evidence

- Implementation: [build/rates.rs](../../../../finstack-quant/calibration/src/build/rates.rs) — four period variants, fixing/settlement dates and listed economics.
- Implementation: [build/prepared.rs](../../../../finstack-quant/calibration/src/build/prepared.rs) — future pillar selection.
- Implementation: [api/authoring.rs](../../../../finstack-quant/calibration/src/api/authoring.rs) — default zero convexity.
- Test evidence: [src/build/rates.rs](../../../../finstack-quant/calibration/src/build/rates.rs) — contract reference-period regressions located; inspected or located, not executed.
- Test evidence: [src/build/tests/rates.rs](../../../../finstack-quant/calibration/src/build/tests/rates.rs) — SOFR and EURIBOR future construction fields; inspected or located, not executed.
- The cited venue rules and dated calendar support the selected profiles
  and independent checks. Production data and external repricing evidence
  remain to be qualified.

## Follow-up tracking

Implementation reconciliation and missing evidence are tracked in the
[calibration-framework follow-ups](../FOLLOW_UPS.md#calibration-orchestration-solvers-and-replay).
