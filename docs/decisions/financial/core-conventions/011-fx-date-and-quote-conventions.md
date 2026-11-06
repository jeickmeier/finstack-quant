# CORE-CONVENTIONS-011: FX market orientation, pip sizes, spot lag and forward maturity dates

- Decision status: **Draft — observed implementation, adoption pending**.
- Implementation status: **Observed**, scoped to `dates/fx.rs` and `money/fx/conventions.rs`.
- Validation status: **Evidence inspected/located, not executed**. The linked tests establish available evidence, not a passing result or methodology approval.
- Audit date: **2026-11-05**. Source revision: `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`; see [scope and inventory](README.md).

## Choice, scope and rationale

FX pair metadata uses an internal market-orientation heuristic; FX settlement uses distinct per-currency/intermediate-day/final-day rules. These are observed defaults, not verified current market conventions for every pair. Rates mean units of quote currency per one base currency. Negative or zero rates are unsupported.

## Algorithm: quotation and pip conventions

Canonical market base ranks EUR, GBP, AUD, NZD, USD, then all other currencies; ties sort by alphabetic code. USD as base is labeled Indirect; USD as quote Direct; a cross inherits the label of the canonical base's USD leg. Pip size is 0.01 if **either** currency is JPY/KRW/HUF, otherwise 0.0001. `fx_pair_convention` normalizes orientation regardless of the input order and attaches the standard lag. Reciprocal conversion requires both input and reciprocal to be finite and strictly positive.

## Algorithm: spot dates

`fx_standard_settlement_days` returns 1 for USD paired with CAD/TRY/PHP/RUB in either direction, 2 otherwise. A different internal per-currency rule assigns 1 to CAD/TRY and 2 to everyone else. `fx_spot_date` resolves optional base/quote calendars (None -> weekends-only); lag zero returns the trade date after that resolution. Positive lag is limited to 31.

Choose USD calendar from the supplied USD leg if present; otherwise `usny`, including non-USD crosses. Let standard pair lag be S, requested lag L, capped=min(L,S), extension=max(L-S,0). Each leg's lag is min(currency_lag,capped)+extension. Non-USD legs count business days on their own calendar. USD intermediate days normally count weekdays only, but if either currency is MXN/ARS/BRL/CLP/COP/PEN/UYU, count USD business days instead.

Take the maximum of both leg results and trade date plus L weekdays. For a non-USD cross containing one of those seven currencies, also require at least L USD business days. Finally roll Following on the joint base, quote and USD calendars. Thus intermediate USD holidays and final USD settlement closures have different effects.

## Algorithm: forward maturity

`add_fx_standard_tenor` adds unadjusted calendar tenor to spot. If `end_of_month` is true **and spot is actual calendar month-end**, snap maturity to calendar month-end. Adjust with the supplied convention on the base/quote joint calendar. This helper does not add a third USD calendar unless the caller includes it in the supplied IDs, and it does not recognize last-business-day as EOM.

## Selection, limitations and failure

Caller supplies currencies, requested lag and optional calendars; there is no registry that automatically selects each currency's settlement calendar. An explicit unknown ID errors even for lag zero. The final joint roll uses the bounded business-day search. Same-currency inputs are not rejected by pair metadata/spot helpers. Quote-orientation order and spot-date rules do not establish bid/ask side or forward points scaling beyond pip metadata.

## Alternatives and tradeoffs

Caller-supplied lag/calendars are supported overrides. Business-EOM, product-specific spot conventions, non-USD-final-gated crosses and quote-specific pip sizes would be different policies. Broad heuristics simplify metadata but require primary reference confirmation before adoption for unusual currencies or instruments.

## Evidence

[Spot/tenor code](../../../../finstack-quant/core/src/dates/fx.rs), [quotation code](../../../../finstack-quant/core/src/money/fx/conventions.rs). Inline tests cover intermediate/final US holidays, USD-gated crosses, T+1 PHP/RUB pairs, shortened/extended lags and Latin-American USD business-day counting; they are constructed-date assertions. Tests for `fx_market_pair_iso_tie_break` and `fx_pip_size_jpy_krw_huf` encode the heuristics. [Clark FX Options](../../../REFERENCES.md#clark-fx-options) and [Wystup FX Options](../../../REFERENCES.md#wystup-fx-options) are general references, not a pair-by-pair confirmation of the constants.

No live exchange/settlement-source refresh or pair-level accuracy certification was performed.

## Follow-up tracking

Adoption questions, implementation reconciliation and missing evidence are tracked in the
[core-conventions follow-ups](../FOLLOW_UPS.md#core-conventions).
