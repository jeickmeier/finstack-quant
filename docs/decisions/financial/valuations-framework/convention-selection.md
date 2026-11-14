# VALUATIONS-FRAMEWORK-16 — Embedded convention resolution, regional fallback and construction presets

- Decision status: **Accepted — complete checked convention selection and dated sources package, 2026-11-14**.
- Implementation status: **Observed, with reconciliation pending** across registry editions, dated selection, product construction, calibration replay and hosts.
- Validation status: Source and selected primary evidence inspected. Bounded independent metadata, selection, date and tick checks executed. Full row, product, calibration and host qualification remain pending.
- Audit date / source revision: 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.

## Accepted decision for checked convention selection and dated sources

1. **Keep the embedded sources as qualified immutable editions.**
   Use the existing six typed families and fallible initialization.
   Publish a snapshot only after all its active family data and
   cross-references validate. Each loader accepts only the explicitly
   supported exact schema identifier and namespace for its family.
   Require a positive data edition separate from schema version;
   identical schema text can serve distinct namespaces. Reject major
   zero, unknown or mismatched schema and namespace pairs, missing
   required fields, unknown fields, malformed values and duplicate raw
   JSON fields before map materialization. Apply one Rust validation
   path to the existing ingest and restore surfaces. Do not introduce a
   live external resolver or compatibility reader.
   [Strict numeric wire and canonical identity](../core-numerics/numeric-wire-and-canonical.md#accepted-decision-strict-numeric-wire-and-canonical-content-identity).

2. **Use exact typed keys and deliberate aliases.** Give each entry a
   nonblank canonical typed key and admit only explicitly listed aliases.
   Use the same canonical grammar for storage and lookup. Keep textual
   IDs exact and case-sensitive, with no implicit trim or case fold;
   typed fields such as currency use their own accepted canonical
   parsing. Check duplicate canonical keys and alias collisions after
   typed decoding, including different text for the same key. A key
   can have separately qualified records in nonoverlapping intervals.
   Reject blank IDs, incomplete entries, conflicting or dangling
   references, and any first-row or last-row authority. An alias identifies the
   same qualified entry in its stated scope; it does not establish a
   venue symbol or authorize another transaction. Unknown or ambiguous
   entries error.

3. **Bind the applicable date, authority and frozen inputs.** A named
   registry selection requires an exact typed key and qualified
   edition, plus the owner's governing convention date and scope.
   Store applicable half-open effective intervals `[from,to)` with
   documented end or open-ended status, and enforce actual source
   coverage. The trade, issue, contract or rule-application date is
   selected by the owner. Valuation date, market snapshot date and
   retrieval date do not substitute for it. Admit exactly one eligible
   row for the selected key/date in its edition; reject missing
   coverage and overlaps. Preserve an existing deal's authored or
   resolved terms across valuations and replays. No automatic
   replacement with the latest row. A row records authority, exact
   rule/source version, applicable scope and coverage, and the
   referenced calendars or definitions and their versions. Qualify
   retained source content by immutable identity and record field
   provenance where authorities differ. Keep an embedded but
   unqualified row from serving as market authority.
   [Executed pricing provenance](pricing-lifecycle.md#accepted-decision-for-checked-valuation-dispatch-and-lifecycle),
   [construction and replay](../core-market/calibration-provenance.md#accepted-decision-lossless-construction-records-and-request-time-replay-qualification).

4. **Separate explicit terms, selected templates and market bindings.**
   Construction takes either complete explicit owner-validated terms
   or a deliberately named qualified template. A supported field
   override is explicit, changes the resolved terms, and is validated
   and stamped as such; a partial or conflicting request cannot
   select another profile. Keep contractual definitions and coupon
   rules distinct from projection, discount and collateral bindings.
   A row, a curve ID or currency alone cannot establish an actual
   binding. Validate exact index and currency references at selection.
   Validate dates and lags under each role's purpose and supported
   calendar coverage. Require explicit nonnegative whole-day lags
   where that role is a lag, with the declared direction, calendar,
   anchor, adjustment and bounds. An explicit weekends-only choice
   is valid where the owner admits it. Inherit applicable representability
   and resource limits instead of treating the observed 31-day reset
   cap as a general market rule. Do not silently replace a
   missing or retired calendar with weekends or another calendar,
   or treat a trading or label calendar as a settlement service.
   Keep required named legs, lag roles and model inputs separate.
   [Calendar purpose and coverage](../core-conventions/005-jurisdiction-calendars.md#accepted-decision-jurisdiction-calendar-purposes-and-coverage),
   [business-day operations](../core-conventions/006-business-day-adjustments.md#accepted-decision-rolling-and-joint-calendar-conventions).

5. **Require an exact CDS election and date regime.** Select the
   governing definitions, transaction type, exact restructuring
   election, currency and dated schedule profile or complete
   bespoke terms. Preserve CR14/MR14/MM14/XR14 as distinct authored
   elections, subject to the chosen transaction's actual terms.
   A qualified named row can supply its exact elections explicitly;
   currency and a regional name cannot infer them. Remove primary
   family selection by first row, North America on missing currency,
   the `ANY` runtime fallback and Australia/New Zealand remapping
   to Asia. Permit explicitly shared schedule terms only as qualified
   references from distinct exact rows that retain their identities.
   Keep premium accrual start, protection step-in, upfront settlement,
   contractual maturity and default payment as separate roles.
   Preserve literal maturities and use the accepted named regimes
   for tenor-based construction. Quote materialization and calibration
   replay use the full admitted election, dates and bindings; a common
   schedule or region does not make different clauses interchangeable.
   [CDS product date regimes](../models-and-valuations-credit/12-cds-legs.md#accepted-decision-named-cds-contractual-date-regimes),
   [CDS calibration materialization](../calibration-framework/07-cds-materialization.md#accepted-decision-named-cds-contractual-date-regimes),
   [ISDA restructuring examples](https://www.isda.org/a/ydiDE/isda-2014-credit-definitions-faq-v12-clean.pdf).

6. **Qualify rate-index and listed interest-rate futures profiles.**
   A term index requires its actual positive contractual tenor and no
   overnight convention. An overnight index carries no term tenor.
   Do not treat a term-style simple reset as overnight averaging.
   Where an overnight template supplies accrual conventions, require
   the exact admitted averaging/compounding, lookback, observation-shift
   and lockout rules. Keep index fixing calendar, coupon payment terms,
   and market settlement terms separate. Do not treat a deposit
   construction template as a tenorless term index. Preserve exact
   IndexId and market projection binding without copying the CurveId
   into an index lookup or ignoring a failed lookup.
   For an interest-rate future require the exact venue/product,
   contract/reference interval rule, reference index, trading and
   settlement rules and applicable contract multiplier, quote units and
   tick terms. Check branch-specific month counts or lags only for
   profiles that actually prescribe them. Tick terms must match their
   unit and contract conditions. A constant-tick profile qualifies
   only for the scope where it holds. Preserve an explicitly supplied
   finite convexity adjustment with its units and basis; omission
   cannot instantiate an estimated correction. Treat retired contracts
   as historical only within their qualified coverage. A generic alias
   does not extend an old contract or change a term index to overnight.
   [Overnight projection](overnight-projection.md#accepted-decision-for-checked-overnight-projection-and-quote-risk),
   [listed products](listed-products.md#accepted-decision-for-checked-listed-futures-and-options),
   [New York Fed SOFR](https://www.newyorkfed.org/markets/reference-rates/sofr/),
   [CME SR3 rulebook](https://www.cmegroup.com/rulebook/CME/IV/400/460.pdf),
   [CME legacy conversion notice](https://www.cmegroup.com/notices/clearing/2023/04/Chadv23-121.pdf).

7. **Resolve complete swaption, inflation and cross-currency terms.**
   For swaptions bind the exact underlying index/currency, each leg's
   frequency, day count, calendar and lag, and the actual expiry,
   settlement and underlying effective-date rule. Keep the convention
   separate from the pricing model, volatility convention and source.
   For inflation bind the exact index/currency, applicable nonnegative
   reference lag in supported whole months where the owner selects monthly
   lag, an explicit reference and interpolation rule, and actual schedule
   and settlement data. Preserve explicit no-lag and monthly anchoring
   modes where supported; alternate spellings cannot switch modes. Apply
   the observation rule once. Keep publication eligibility, base
   observations and price-index projection with their owners.
   Configuration, quote preparation,
   calibration, interpolation and repricing use the same resolved
   financial basis. Replacing selected terms requires explicit
   requalification; no hidden plan-over-row merging or rebuilding
   missing fields from another convention.
   For cross-currency swaps bind the ordered distinct currencies,
   exact leg indices and curve roles, each calendar and the selected
   joint date rule. Require explicit principal exchange and, for
   mark-to-market exchange, the exact resetting leg. No reverse-pair
   or reset-side inference. The selected EUR/USD observation uses
   Leg2 explicitly; it is not a universal default.
   [Cross-currency owner](../valuations-rates/16-cross-currency-swaps.md).

8. **Remove broad construction presets as implicit authority.** A bond
   requires exact issue terms or a qualified selected issue or market
   template with a documented applicability scope. Do not infer
   issuer class from currency or adopt a blanket stub, roll, end-of-
   month, settlement, ex-coupon, calendar or discount ID. Preserve
   explicit quote compounding where the bond owner requires it.
   For commodities require the exact contract or qualified dated venue
   specification and actual currency, settlement date or complete
   settlement rule. Remove default currency, lag and roll chosen by
   broad asset labels. Exchange-traded delivery terms do not establish
   an OTC forward settlement rule. Retain actual calendar scope;
   retired generic `cme` and the `nerc` off-peak labels cannot
   supply settlement calendars. No forced migration or silent alias.
   [Bond quote compounding](../valuations-bonds/12-spread-quotes.md#accepted-decision-required-quote-compounding),
   [UK DMO gilt conventions and exceptions](https://dmo.gov.uk/help/glossary/),
   [CME WTI contract scope](https://www.cmegroup.com/content/dam/cmegroup/rulebook/NYMEX/2/200.pdf).

9. **Require NDF quote direction and principal exchange at ingress.**
   NDF construction, including trade-date conveniences, requires the
   ordered currencies and declared BasePerSettlement or
   SettlementPerBase convention. Bind each contract, fixing and
   applicable forward quote to its actual direction, using only an
   explicit checked conversion. Remove the enum/default-direction
   shortcut. Cross-currency native builders and wire restore require
   the existing explicit exchange choice, including None, Final,
   InitialAndFinal or the named resetting leg; remove the implicit
   InitialAndFinal path. Product owners retain actual fixing, FX
   projection, settlement, reset and cashflow formulas.
   [Explicit FX quotation and value dates](../core-conventions/011-fx-date-and-quote-conventions.md#accepted-decision-explicit-fx-quote-metadata-and-value-dates),
   [NDF owner](../valuations-fx-and-exotics/ndf-fixing-settlement.md).

10. **Reconcile existing consumers and qualify retained rows.** Keep
    shared Rust lookup and validation as authority in construction,
    calibration, replay and any exposed Python/WASM lookups, typed
    inputs, declarations, schemas and parity. Return the admitted
    resolved values and provenance rather than host-specific
    defaults. Retire unsupported rows from authoritative selection or
    retain them only as clearly scoped historical/reference material.
    Qualify each retained row against the actual primary rule,
    effective period and dependencies before using it as a market
    convention. Check overlap, gap, alias and typed collision
    cases; test exact dated construction and replay. Parsing a row or
    matching one example does not establish venue, regulatory or
    historical accuracy. Product and calibration owners retain
    their financial and numerical acceptance rules.

## Observed behavior

### Choice and loading algorithm

The process loads six embedded JSON convention registries lazily and caches the successful result. Input metadata requires a nonempty schema identifier ending in `.registry.v<integer>`, schema major at most 2, nonempty namespace, and positive registry version. It does not verify that the namespace matches the requested registry or that schema major is positive. Alias identifiers are trimmed, required nonempty and unique after normalization; duplicate aliases fail rather than replacing an existing convention. Query identifiers are typed wrappers without analogous normalization. Thus insertion-time whitespace normalization is not a case-insensitive/trimmed lookup policy.

Rate-index records parse tenor/payment frequencies, forbid tenor on overnight RFRs, require their OIS compounding, and forbid OIS compounding on term indices. A term index may omit tenor at this loader boundary. Reset lag is an integer in `[0,31]` with positive T-minus semantics. Payment and settlement lags are not given the same range validation here. IR-future records require positive finite face/tick fields, positive delivery months, nonnegative settlement lag, zero settlement lag for in-arrears references, three months for IMM-quarter and one month for calendar/business-month references. The optional convexity adjustment is stored rather than estimated in the loader. Swaption, inflation and XCCY records parse their payment/fixed tenors and preserve the other supplied economics; downstream products validate additional constraints. Omitted XCCY notional exchange selects InitialAndFinal, while the current EUR/USD row explicitly selects mark-to-market resetting on Leg2.

### Lookup and CDS regional selection

Rate-index, swaption, inflation, IR-future and XCCY queries require an exact registered ID and error when absent. CDS lookup first maps documentation clauses into a schedule family: AU/NZ meta clauses use Asia; exact CR14/MR14/MM14/XR14 retain their contractual clause at the instrument while selecting the currency's explicit regional family, or North America when no currency row exists. Query the exact currency/family row, then the loader-only `ANY` row for that family. Explicit family detection similarly defaults unknown currencies to North America. This is a material fallback, not a universal strict-missing policy. The CDS loader rejects duplicate normalized text IDs, validates the single-colon ID and its regional clause against the record, and parses payment frequency. Settlement is stored as unsigned days and calendar text is not resolved here. For currencies with multiple noncustom regional rows, the first encountered row selects the primary family; differently cased ANY prefixes can map to the same family and later replace its fallback. Family fallback does not infer contract-specific restructuring economics.

### Bond and commodity presets

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

### Live differences from the original audit

- The [XCCY loader](../../../../finstack-quant/valuations/core/src/market/conventions/loaders/xccy.rs)
  requires `notional_exchange`. A test named as an omission case
  exercises the enum default; the instrument construction and restore
  paths still require reconciliation.
- The [commodity forward](../../../../finstack-quant/valuations/exotics/src/commodity/commodity_forward/types.rs)
  now requires its contract settlement calendar and rejects `nerc` in
  that role. Broad lag and roll fallbacks still remain.
- The [NDF direct path](../../../../finstack-quant/valuations/fx/src/ndf/types.rs)
  requires quote orientation, while the trade-date helper still selects
  BasePerSettlement. The [bond constructors](../../../../finstack-quant/valuations/fixed-income/src/bond/types/construction.rs)
  retain inferred fixed and floating construction presets.
- The [USD-Deposit row](../../../../finstack-quant/valuations/core/data/conventions/rate_index_conventions.json)
  has `1D`, but the scalar calibration template supplies cash at the
  quoted maturity. That synthetic template is not an observed one-day
  term benchmark.

## Alternatives, evidence

An explicit per-contract convention is the supported way to depart from a preset. Date/version-specific registries, strict CDS missing-currency failure, and issuer/exchange-specific calendars are alternatives requiring agreement. Embedded loading tests establish parser/lookup behavior and selected row values, not current venue correctness.

- [registry.rs](../../../../finstack-quant/valuations/core/src/market/conventions/registry.rs), [definitions](../../../../finstack-quant/valuations/core/src/market/conventions/defs.rs), [IDs](../../../../finstack-quant/valuations/core/src/market/conventions/ids.rs), [JSON loader](../../../../finstack-quant/valuations/core/src/market/conventions/loaders/json.rs), [rate-index loader](../../../../finstack-quant/valuations/core/src/market/conventions/loaders/rate_index.rs), [CDS loader](../../../../finstack-quant/valuations/core/src/market/conventions/loaders/cds.rs), [IR-future loader](../../../../finstack-quant/valuations/core/src/market/conventions/loaders/ir_future.rs), [swaption loader](../../../../finstack-quant/valuations/core/src/market/conventions/loaders/swaption.rs), [inflation loader](../../../../finstack-quant/valuations/core/src/market/conventions/loaders/inflation_swap.rs), and [XCCY loader](../../../../finstack-quant/valuations/core/src/market/conventions/loaders/xccy.rs) own lookup/default validation.
- The [embedded data directory](../../../../finstack-quant/valuations/core/data/conventions) contains the exact six JSON inputs read in this audit. [conventions.rs](../../../../finstack-quant/valuations/core/src/instruments/common_impl/parameters/conventions.rs) owns the bond/commodity tables. Primary convention provenance and adoption approval remain open.

### Independent review evidence

- Strict offline JSON inspection found six files, 40 payload rows and
  95 alias/ID occurrences. The inspected rate-index references resolve
  structurally, but no row supplies an effective interval and primary
  provenance. A synthetic pair of adjacent intervals selects the later
  row at the shared boundary, has no row at the exclusive final endpoint,
  and becomes ambiguous if the intervals overlap.
- The embedded SR3 row carries the term `USD-SOFR-3M` identifier and
  the `.0025` index point/`$6.25` tick. CME Chapter 460 uses daily
  overnight SOFR and states a `.005`/`$12.50` base tick with a
  specified near-expiry `.0025`/`$6.25` tier. Both pairs imply
  `$2,500` per index point; a multiplier check alone misses the incorrect
  tick scope. The exact listed rules and admitted quote styles govern.
- The inspected AUD/NZD rows under the Asia family carry `auce` and
  `nzau`, while the Asia `ANY` row carries `jpto`. A BRL request
  can lack an exact row but still pick up a North American fallback.
  Synthetic row reversals change the first family or cause a differently
  cased `ANY` entry to win. Exact selection rejects that implicit authority.
- An NDF with CNY `700,000` notional and quotes of `7.25` for the
  contract and `7.00` at fixing in CNY per USD gives
  `100000/29` USD (about `3,448.28`) on the example's long-base
  settlement basis. Reciprocating the quotes explicitly gives the
  same amount; relabeling the original numeric quotes gives
  `−175,000` USD. A separate example counts seven business days
  before the [DMO's January 31, 2025 payment](https://dmo.gov.uk/responsibilities/gilt-market/about-gilts)
  and reaches January 22. It does not qualify the rest of the gilt preset.

These bounded checks do not validate the full dataset, exact market
availability, or production construction, calibration, runtime or hosts.

## Follow-up tracking

Adoption, implementation and validation work is tracked in the
[scope follow-ups](../FOLLOW_UPS.md#shared-valuation-framework).
