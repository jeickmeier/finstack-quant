# CORE-CONVENTIONS-011: Explicit FX quote metadata and value-date conventions

- Decision status: **Accepted — complete FX quote/date package, 2026-11-07**.
- Implementation status: **Observed; reconciliation pending**, scoped to `dates/fx.rs` and `money/fx/conventions.rs`.
- Validation status: **Evidence inspected/located; independent qualification pending**. Documentation and independent quote/unit/reciprocal/calendar examples passed; production Rust/host tests, pricing and current pair/venue qualification were not executed for this review.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; see [scope and inventory](README.md).

## Choice, scope and rationale

The observed metadata infers pair orientation, pip size and spot lag; the spot-date helper adds currency-specific intermediate/final-day rules. Those heuristics are not independently verified current conventions for every pair/product/venue. The accepted package below retains explicit quote units and checked arithmetic, removes generic market inference and defines a narrow forward-value-date operation with explicit calendars and business-EOM policy.

## Accepted decision: explicit FX quote metadata and value dates

Accepted on **2026-11-07** after whole-record review. This complete FX
quote/date package requires runtime/caller reconciliation and independent
qualification. Inherit the accepted
[004 calendar-data](004-holiday-engine.md#accepted-decision-holiday-engine-and-data-boundaries),
[005 calendar-purpose](005-jurisdiction-calendars.md#accepted-decision-jurisdiction-calendar-purposes-and-coverage),
[006 adjustment/error](006-business-day-adjustments.md#accepted-decision-rolling-and-joint-calendar-conventions)
and [007 tenor arithmetic](007-tenor-date-arithmetic.md#accepted-decision-tenor-and-date-arithmetic-conventions)
contracts. The observations below remain separate from these accepted choices.

1. **Preserve the supplied ordered pair and explicit quote units.** An
   outright rate `r` means quote-currency units per one base-currency unit;
   multiplying a base amount by `r` gives quote units. Preserve the declared
   order rather than reordering by a market hierarchy or alphabetic code.
   [CME's quotation explanation](https://www.cmegroup.com/education/courses/introduction-to-fx/understanding-fx-quote-conventions)
   distinguishes base/quote and notes that spot/futures orientation can differ.
   Require distinct currencies for quote-convention metadata and FX trades,
   before identity shortcuts. This does not redefine same-currency money/
   FX-matrix conversion policy owned by [013](013-fx-resolution-and-shocks.md).

2. **Keep validated explicit quote metadata, removing inference.** Retain
   `FxPairConvention` as the existing quote-metadata home, with the supplied
   ordered base/quote and a caller-supplied finite, strictly positive
   `pip_size`. Reuse the existing currency-pair representation; add no
   parallel pair registry. Make construction and any permitted restoration
   validate those invariants rather than exposing mutable bypasses. Remove
   `fx_pair_convention`, private ranking/alphabetic fallback and either-leg
   JPY/KRW/HUF pip inference. Remove `FxQuoteConvention` and inherited cross
   Direct/Indirect labels, plus `usd_quotation` and `settlement_days` fields.
   Explicit ordered units and product date policy replace those ambiguous
   labels/scalar hints; do not add renamed heuristic factories or aliases.

3. **Define pips in the actual ordered quote's units.** `pip_size` is
   the outright-rate increment per pip, not ISO minor-unit scale, display
   precision or a venue's minimum executable tick. Signed finite forward
   pips convert to rate points as `pips * pip_size`; a supplied convention
   must match the instrument's stored base/quote order. Reconcile
   `FxForward::with_forward_pips` to receive matching explicit metadata or
   its explicit pip size rather than canonicalizing an unrelated order.
   Preserve required finite/positive outright-rate validation in the owning
   forward-points operation. Inverting an outright does not preserve its
   pip increment or negate its forward points; a quote in the reversed
   direction needs its own explicitly selected pip convention.

4. **Retain the checked scalar reciprocal.** `invert_fx_rate(rate)`
   accepts only finite strictly positive input and returns `1/rate` only
   when that result is finite and strictly positive. Zero, negatives,
   nonfinite values and unrepresentable reciprocals error; no clipping,
   fabricated zero or infinity success. The scalar helper has no currency,
   bid/ask or convention-selection semantics. Inverting twice need not
   recover identical floating-point bits, and a spread-aware inverse would
   require the opposite bid/ask side at its owner.

5. **Remove the inferred generic spot-date recipe.** Remove
   `fx_spot_date` and `fx_standard_settlement_days`, their per-currency lag
   table, seven-currency USD-intermediate list, implicit `usny`, None-to-
   weekends fallback and arbitrary 31-day restriction. The
   [January 2020 EBS specification](https://www.cmegroup.com/content/dam/cmegroup/documents/ebs_value-date-calendar.pdf),
   sections 3.1–3.3, has venue/product exceptions absent from the source,
   including a specifically NDF-only USD/PHP T+1 entry. It does not certify
   a universal present-day currency-enum rule. Do not replace the helper
   with a generic public multi-clock rules engine, global currency registry
   or another unsourced default-T+2 table.

6. **Require actual spot dates or an explicitly owned derivation.**
   Product/calibration workflows either receive the actual selected
   spot/settlement date or derive it from their applicable pair/product/
   venue convention using existing checked day offsets, joint calendars
   and adjustments. That owner selects trade-date/cutoff meaning, requested
   lag, each intermediate-day counting calendar and final eligible-date
   calendars, including any required USD gate. A weekday minimum, per-leg
   lag and a final USD closure are separate constraints; a single lag and
   two currency labels do not determine them. Shortened/extended lags,
   T+0, deliverable/NDF status, nonstandard weekends and historical versions
   need explicit owner selection. Missing policy/data error rather than
   copying the old heuristic into a local fallback. Core offset bounds and
   required-data errors remain those of 006, not the removed 31-day rule.

7. **Retain one explicitly named forward-value-date operation.** Replace
   `add_fx_standard_tenor` with canonical `fx_forward_date`, without an
   alias. Require the actual `spot_date`, a positive 007 `tenor`, an explicit
   business-day convention, explicit `preserve_business_eom` choice and one
   explicit final settlement calendar/provider. It may already be a strict
   composite containing every contractually required member. Do not infer
   calendars from currencies or include/drop USD merely because the pair
   is a cross. Remove optional-calendar/weekends-only and host/product
   ModifiedFollowing/EOM omission defaults on paths invoking this operation.
   Product owners decide when this exact recipe applies; it is not a
   universal trade-to-maturity, fixing or NDF settlement selector.

8. **Define the Natural forward branch precisely.** Compute one raw
   target directly from spot using 007's D/W calendar-day or M/Y clamped
   addition, then apply the selected 006 convention once on the explicit
   final calendar. Do not iterate from a clamped predecessor or silently
   substitute a joint-currency/US calendar. With preservation disabled,
   perform no EOM classification or snapping. With preservation enabled
   and a non-EOM business spot, use this same Natural branch.

9. **Make business-EOM preservation conditional and M/Y-only.** If
   `preserve_business_eom` is true, reject D/W tenors. For M/Y, determine
   whether the supplied open spot is the final business day of its calendar
   month under the selected final calendar. If yes, return the final business
   day of the target calendar month obtained by the exact tenor month/year
   offset. If not, use Natural plus the explicit convention. The business
   end-to-end distinction is described in the
   [ANZ July 2002 guide, PDF pages 20–21](https://www.anz.com/Documents/FXOnline/Intro%20to%20FX%20Update.pdf)
   and EBS's dated NDF end-end rule; those examples do not adopt every
   product's current policy. Remove the actual-calendar-EOM-only trigger.
   This conditional FX policy differs from 008's strict EOM schedule grid.

10. **Require eligible value dates and necessary calendar proofs.** In
    this forward-value-date operation, require spot to be open on the explicit
    final calendar; never pre-roll a closed supplied spot. Require a returned
    date to be open and strictly later than spot. If Natural plus Unadjusted
    leaves a closed target, or Preceding collapses it back onto spot, error
    rather than rewriting the requested convention. These are selected FX
    operation preconditions, not changes to 006's identity operation or
    007's general calendar arithmetic. To prove business-EOM, inspect only
    required dates later in the spot month; one later open date disproves
    EOM. Search the target month's last open date strictly within that month.
    In the preserved-EOM branch, a fully closed target month errors rather
    than returning a neighboring month; an already-open EOM result is not
    rolled again. Required
    member/coverage/data/date failures propagate; missing data are not closed
    days or reasons to fall back to Natural. Use checked year/month/date
    arithmetic across the actual supported Date domain, without arbitrary
    search guards or automatic current-date assumptions.

11. **Reconcile production callers as a working slice.** The old spot
    helper has real FX spot/forward/swap/NDF and XCCY calibration callers.
    Update each to an actual date or its explicitly selected owner policy in
    the same implementation slice as removal; do not leave broken products,
    compatibility wrappers or private copies of the same unsourced tables.
    Reconcile forward-date and pip callers together, preserving declared
    quote order. Product parameter objects shared by the hosts currently
    infer lag/ModifiedFollowing/EOM; their financial defaults need explicit
    resolution at their owner, not accidental survival through a wrapper.
    Actual pricing, NDF fixing/payment relations, overnight/tom-next legs,
    market-data authority, FX triangulation and option delta conventions
    retain their separate records. No new product convention is adopted by
    simply removing its shared heuristic.

12. **Keep host/state validation and provenance aligned.** All retained
    financial arithmetic/validation stays in Rust. Propagate the validated
    quote metadata, removed factories/enums/fields/spot helpers and canonical
    forward-date inputs through Rust, Python/WASM, product parameter payloads,
    schemas where affected, parity contracts and examples. Reject malformed
    pairs, Boolean/nonfinite numeric inputs, nonpositive pip/rate values and
    invalid dates; honor native-host date limits with explicit conversion
    errors. Use existing error mapping and calendar identity/edition rules.
    Product convention provenance identifies applicable pair, product/venue,
    source/version, dates and purpose; a dated venue PDF is not a universal
    currency-default pin. Independent quote/unit/reciprocal, calendar/EOM,
    caller and host qualification remain with the
    [existing follow-ups](../FOLLOW_UPS.md#core-conventions). Runtime changes
    and independent current-market qualification remain pending.

## Boundary examples for the accepted package

These accepted examples use explicit quote inputs and synthetic full-day
calendars. They do not execute production Rust/host tests or certify a current
pair's venue/product convention. Date examples use explicit weekends-only
as the final calendar unless stated otherwise.

| Request | Required result |
| --- | --- |
| Explicit USD/EUR metadata versus EUR/USD metadata | Preserve each supplied order; no market-rank normalization or inherited USD label. |
| Same-currency quote/trade metadata, or pip size zero/negative/nonfinite | Input error before date/rate shortcuts. |
| EUR/USD spot `1.10`, `+50` pips, explicit pip size `0.0001` | Rate points `0.005`; outright `1.105`, subject to the owner's checked rate construction. |
| USD/JPY rate `150`, pip size `0.01`; invert the scalar | `1/150` USD per JPY; the reverse pip size is not inferred as `0.01` or its reciprocal. |
| `invert_fx_rate(0)`, negative, NaN/infinity or tiny positive input whose reciprocal overflows | Explicit validation/calculation error. |
| Missing spot-date policy or unsupported pair/venue exception | Require actual date or explicit owner policy; no default lag/calendar or generic fallback. |
| Open spot `2025-08-29`, `1M`, Following, preserve-business-EOM false / true | September 29 / September 30. August 29 is the last business day but not calendar month-end. |
| Open non-EOM spot `2025-08-28`, `1M`, Following, preservation true | Raw September 28 rolls to September 29 through the Natural branch. |
| Spot `2025-04-30`, `1D` or `1W`, preservation true | Input error; do not snap the result to the end of May. |
| Spot `2025-04-30`, `1D`, Following, preservation false | May 1. The observed calendar-EOM snap with preservation true instead reaches May 31 and rolls to June 2. |
| Spot `2025-01-03` Friday, `1D`, Preceding, preservation false | Error: adjusted Saturday target collapses onto spot. General 006 adjustment itself still returns January 3. |
| A closed supplied spot; Natural Unadjusted with a closed target | Error under this value-date helper's explicit eligibility requirements; no silent pre-roll or convention change. |
| Synthetic joint-calendar closures on both September 29–30, 2025; spot August 29, `1M`, preservation true | September 26, the last open target-month date. |
| A preserved-EOM target month wholly closed or with needed coverage unavailable | Error; no neighboring-month or Natural fallback. |
| Natural Following with every date in the target month closed, but a later month open | The 006 roll may reach that later month; the preserved-EOM same-month requirement does not apply to Natural. |
| A required extra USD final calendar, or one deliberately absent under an adopted product convention | Caller supplies the exact final composite; no currency-label inference adds or removes it. |
| Invalid calendar ID, missing required data, unrepresentable target or invalid host date | Explicit error through the canonical mapping. |

## Observed algorithm: quotation and pip conventions

Canonical market base ranks EUR, GBP, AUD, NZD, USD, then all other currencies; ties sort by alphabetic code. USD as base is labeled Indirect; USD as quote Direct; a cross inherits the label of the canonical base's USD leg. Pip size is 0.01 if **either** currency is JPY/KRW/HUF, otherwise 0.0001. `fx_pair_convention` normalizes orientation regardless of the input order and attaches the standard lag. Reciprocal conversion requires both input and reciprocal to be finite and strictly positive.

## Observed algorithm: spot dates

`fx_standard_settlement_days` returns 1 for USD paired with CAD/TRY/PHP/RUB in either direction, 2 otherwise. A different internal per-currency rule assigns 1 to CAD/TRY and 2 to everyone else. `fx_spot_date` resolves optional base/quote calendars (None -> weekends-only); lag zero returns the trade date after that resolution. Positive lag is limited to 31.

Choose USD calendar from the supplied USD leg if present; otherwise `usny`, including non-USD crosses. Let standard pair lag be S, requested lag L, capped=min(L,S), extension=max(L-S,0). Each leg's lag is min(currency_lag,capped)+extension. Non-USD legs count business days on their own calendar. USD intermediate days normally count weekdays only, but if either currency is MXN/ARS/BRL/CLP/COP/PEN/UYU, count USD business days instead.

Take the maximum of both leg results and trade date plus L weekdays. For a non-USD cross containing one of those seven currencies, also require at least L USD business days. Finally roll Following on the joint base, quote and USD calendars. Thus intermediate USD holidays and final USD settlement closures have different effects.

## Observed algorithm: forward maturity

`add_fx_standard_tenor` adds unadjusted calendar tenor to spot. If `end_of_month` is true **and spot is actual calendar month-end**, snap maturity to calendar month-end. Adjust with the supplied convention on the base/quote joint calendar. This helper does not add a third USD calendar unless the caller includes it in the supplied IDs, and it does not recognize last-business-day as EOM.

## Observed selection, limitations and failure

Caller supplies currencies, requested lag and optional calendars; there is no registry that automatically selects each currency's settlement calendar. An explicit unknown ID errors even for lag zero. The final joint roll uses the bounded business-day search. Same-currency inputs are not rejected by pair metadata/spot helpers. Quote-orientation order and spot-date rules do not establish bid/ask side or forward points scaling beyond pip metadata.

## Alternatives and tradeoffs

The accepted package removes unsourced generic spot and quote inference rather than adding a new convention registry or public rules engine. Explicit quote metadata and a bounded, fully specified forward-date recipe retain reusable calculations; actual pair/product/venue spot policies and published data remain with their financial owners. All affected callers must be reconciled in the same working implementation slice.

## Evidence

[Spot/tenor code](../../../../finstack-quant/core/src/dates/fx.rs), [quotation code](../../../../finstack-quant/core/src/money/fx/conventions.rs). Inline tests cover intermediate/final US holidays, USD-gated crosses, T+1 PHP/RUB pairs, shortened/extended lags and Latin-American USD business-day counting; they are constructed-date assertions. Tests for `fx_market_pair_iso_tie_break` and `fx_pip_size_jpy_krw_huf` encode the heuristics. [Clark FX Options](../../../REFERENCES.md#clark-fx-options) and [Wystup FX Options](../../../REFERENCES.md#wystup-fx-options) are general references, not a pair-by-pair confirmation of the constants.

No live exchange/settlement-source refresh or pair-level accuracy certification was performed.

The 2026-11-07 review inspected CME quotation material and the dated EBS
January 2020 / ANZ July 2002 documents for formula and scope evidence, not
a present-day pair/venue convention-data refresh. Independent quote-unit,
reciprocal and synthetic-calendar boundary examples passed; production
financial/host tests and current-market qualification were not executed.

The [production forward pip conversion](../../../../finstack-quant/valuations/fx/src/fx_forward/types.rs)
uses reordered convention metadata for the instrument's stored pair. The same
file's trade-date parameter payload and the [spot](../../../../finstack-quant/valuations/fx/src/fx_spot/types.rs),
[swap](../../../../finstack-quant/valuations/fx/src/fx_swap/types.rs),
[NDF](../../../../finstack-quant/valuations/fx/src/ndf/types.rs) and
[XCCY builder](../../../../finstack-quant/calibration/src/build/xccy.rs)
are material caller/default reconciliation surfaces. These source observations
are not executed pricing-defect reproductions or approval of their product rules.

## Follow-up tracking

Implementation/caller reconciliation and independent qualification are tracked in the
[core-conventions follow-ups](../FOLLOW_UPS.md#core-conventions).
