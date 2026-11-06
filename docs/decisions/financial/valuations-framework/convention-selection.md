# VALUATIONS-FRAMEWORK-16 — Embedded convention resolution, regional fallback and construction presets

- Decision status: Draft — observed implementation; adoption pending.
- Implementation status: Observed in convention loaders, lookup algorithms and parameter presets; current legal/exchange correctness is not established by this audit.
- Validation status: Evidence inspected/located, not executed; no live external convention verification performed.
- Audit date / source revision: 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Choice and loading algorithm

The process loads six embedded JSON convention registries lazily and caches the successful result. Input metadata requires a nonempty schema identifier ending in `.registry.v<integer>`, schema major at most 2, nonempty namespace, and positive registry version. It does not verify that the namespace matches the requested registry or that schema major is positive. Alias identifiers are trimmed, required nonempty and unique after normalization; duplicate aliases fail rather than replacing an existing convention. Query identifiers are typed wrappers without analogous normalization. Thus insertion-time whitespace normalization is not a case-insensitive/trimmed lookup policy.

Rate-index records parse tenor/payment frequencies, forbid tenor on overnight RFRs, require their OIS compounding, and forbid OIS compounding on term indices. A term index may omit tenor at this loader boundary. Reset lag is an integer in `[0,31]` with positive T-minus semantics. Payment and settlement lags are not given the same range validation here. IR-future records require positive finite face/tick fields, positive delivery months, nonnegative settlement lag, zero settlement lag for in-arrears references, three months for IMM-quarter and one month for calendar/business-month references. The optional convexity adjustment is stored rather than estimated in the loader. Swaption, inflation and XCCY records parse their payment/fixed tenors and preserve the other supplied economics; downstream products validate additional constraints. Omitted XCCY notional exchange selects InitialAndFinal, while the current EUR/USD row explicitly selects mark-to-market resetting on Leg2.

## Lookup and CDS regional selection

Rate-index, swaption, inflation, IR-future and XCCY queries require an exact registered ID and error when absent. CDS lookup first maps documentation clauses into a schedule family: AU/NZ meta clauses use Asia; exact CR14/MR14/MM14/XR14 retain their contractual clause at the instrument while selecting the currency's explicit regional family, or North America when no currency row exists. Query the exact currency/family row, then the loader-only `ANY` row for that family. Explicit family detection similarly defaults unknown currencies to North America. This is a material fallback, not a universal strict-missing policy. The CDS loader rejects duplicate normalized text IDs, validates the single-colon ID and its regional clause against the record, and parses payment frequency. Settlement is stored as unsigned days and calendar text is not resolved here. For currencies with multiple noncustom regional rows, the first encountered row selects the primary family; differently cased ANY prefixes can map to the same family and later replace its fallback. Family fallback does not infer contract-specific restructuring economics.

## Bond and commodity presets

These are observed construction defaults, not a claim that the table is currently authoritative for every issue or venue. All bond presets use ShortFront stub. Government presets use Following; agency/corporate presets use ModifiedFollowing.

| Bond preset | Day count; frequency | Settlement; ex-coupon | Discount ID; calendar |
|---|---|---|---|
| US Treasury | ACT/ACT ICMA; semiannual | T+1; none | USD-TREASURY; sifma |
| US Agency | 30/360; semiannual | T+1; none | USD-OIS; sifma |
| US Corporate | 30/360; semiannual | T+1; none | USD-OIS; usny |
| EUR Corporate | ACT/ACT ICMA; annual | T+2; none | EUR-OIS; target2 |
| German Bund | ACT/ACT ICMA; annual | T+2; none | EUR-BUND; target2 |
| French OAT | ACT/ACT ICMA; annual | T+2; none | EUR-BUND; target2 |
| UK Gilt | ACT/ACT ICMA; semiannual | T+1; seven business days | GBP-GILT; gblo |
| JGB | ACT/365F; semiannual | T+2; none | JPY-JGB; jpto |

Commodity presets all return USD. Power uses T+1/ModifiedFollowing/nerc; gold and silver use T+2/ModifiedFollowing/cme; WTI, natural gas and agriculture use T+2/Following/cme; Brent and copper use T+2/Following/gblo. These broad calendar mappings are a modeling choice; named exchange rules are not reverified here. Enum parsers accept exact snake-case names. Shared `NotionalExchange` defaults to InitialAndFinal; NDF quote direction defaults to BasePerSettlement (base units per settlement-currency unit) and is explicitly parsed, not inferred from currency-name order.

## Alternatives, evidence

An explicit per-contract convention is the supported way to depart from a preset. Date/version-specific registries, strict CDS missing-currency failure, and issuer/exchange-specific calendars are alternatives requiring agreement. Embedded loading tests establish parser/lookup behavior and selected row values, not current venue correctness.

- [registry.rs](../../../../finstack-quant/valuations/core/src/market/conventions/registry.rs), [definitions](../../../../finstack-quant/valuations/core/src/market/conventions/defs.rs), [IDs](../../../../finstack-quant/valuations/core/src/market/conventions/ids.rs), [JSON loader](../../../../finstack-quant/valuations/core/src/market/conventions/loaders/json.rs), [rate-index loader](../../../../finstack-quant/valuations/core/src/market/conventions/loaders/rate_index.rs), [CDS loader](../../../../finstack-quant/valuations/core/src/market/conventions/loaders/cds.rs), [IR-future loader](../../../../finstack-quant/valuations/core/src/market/conventions/loaders/ir_future.rs), [swaption loader](../../../../finstack-quant/valuations/core/src/market/conventions/loaders/swaption.rs), [inflation loader](../../../../finstack-quant/valuations/core/src/market/conventions/loaders/inflation_swap.rs), and [XCCY loader](../../../../finstack-quant/valuations/core/src/market/conventions/loaders/xccy.rs) own lookup/default validation.
- The [embedded data directory](../../../../finstack-quant/valuations/core/data/conventions) contains the exact six JSON inputs read in this audit. [conventions.rs](../../../../finstack-quant/valuations/core/src/instruments/common_impl/parameters/conventions.rs) owns the bond/commodity tables. Primary convention provenance and adoption approval remain open.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#shared-valuation-framework).
