# MARGIN-COLLATERAL-IM-001 — Registry composition and contractual defaults

**Decision status:** Accepted 2026-11-16 for explicit margin parameter snapshots and contractual terms.

**Implementation status:** Existing behavior is described below. Registry, calculator, agreement, portfolio and host reconciliation with the accepted contract are pending.

**Validation status:** Live loader and caller paths inspected. An independent Decimal example checked three gross schedule IM amounts from bundled rates. Evidence for historical sources is partial. Runtime and host qualification remain pending. Audit date **2026-11-05** and source revision `7e82fd03ad98346727769e88dd5ac5d84b56e9fc` (scope and coverage in [README](README.md)).

## Accepted decision for explicit margin parameter snapshots and contractual terms

1. Elect one immutable parameter snapshot for a run that uses registry data. Require an explicit choice of the bundled reference snapshot or a complete typed custom snapshot for the profiles the run uses. Retire the recursive `margin.registry.v1` overlay. Resolve exact profiles once and preserve that selection through calculator and agreement construction and later term changes. Custom profiles supply all their required values without borrowing embedded defaults.
2. Use one checked validation path for bundled, custom and restored profiles. Require recognized schema and version tags. Reject unknown fields, duplicate raw JSON keys at text ingress and duplicate interpreted keys such as schedule cells, asset-class entries and normalized correlation pairs. Validate every supplied profile for its units, ranges, required entries and ordered boundaries. Enforce the schedule rate range `[0, 1]` at load and use. Publish only validated snapshots. Use one fallible embedded load that caches either the validated snapshot or the typed error. [Accepted configuration and restoration contract](../core-numerics/rounding-and-config.md).
3. Require an explicit methodology and its inputs. For SIMM, select a supported edition and exact parameter profile. For schedule IM, supply the applicable asset class, maturity where it matters and notional. Retire the inferred five-year interest-rate input and generic rate. For collateral, use the elected eligibility schedule and actual asset classification. For cleared IM, identify the exact clearing entity, service and account and elect sourced CCP IM or a named indicative proxy. Missing data on the elected route fail. Keep the model reference margin period of risk and the agreement's required horizon distinct. [Accepted portfolio IM contract](../portfolio-execution/16-margin-netting-and-initial-margin.md#accepted-decision-for-checked-portfolio-netting-and-initial-margin).
4. Supply operative terms from the dated agreement or clearing arrangement. Require applicable settlement currency and FX terms, allocated thresholds and minimum transfer amounts, rounding, independent amounts, segregation, calendars, call cadence, settlement lag and collateral remuneration terms. Preserve the elected method through construction and term changes. Distinguish explicit zero, explicit no IM and missing information. The Basel EUR 50 million consolidated-group IM threshold and EUR 500,000 transfer ceiling are framework limits. Use actual allocated currency-bearing agreement amounts. [Basel MGN20.5–20.6](https://www.bis.org/committees/bcbs/basel-framework/standard/mgn/20/inforce/2019-12-15/published/2019-12-15).
5. Where applicable, capture the agreed notification, response, dispute and grace terms with their clock, units, time zone, calendar and authority. Report these as metadata until the corresponding operation implements the rules. The [CSA](17-csa-selection-and-im-terms.md), [VM](18-variation-margin.md) and [replay](19-margin-dates-and-replay.md#accepted-decision-for-checked-vm-schedules-and-settlement-replay) records own calculation and lifecycle details.
6. Preserve effective provenance and replay through Rust. Distinguish wire version, methodology edition, selected profile and exact effective data identity. Retain source, applicable dates, currency and units, horizon, coverage and approximation status. Give custom data its own identity and retain its source lineage. Treat the bundled USD SIMM v2.6 profile as historical and approximate. The bundled collateral reference covers cash and qualified government or central-bank securities under record 015. [Collateral eligibility and credited value](15-collateral-eligibility-and-haircuts.md#accepted-decision-for-checked-collateral-eligibility-and-credited-value). Record 014 treats the embedded CCP rates as historical evidence and requires an elected local profile for any proxy calculation. [Cleared IM routes](14-ccp-external-and-proxy.md#accepted-decision-for-elected-cleared-initial-margin-routes). Python and WASM use the same validated Rust state, result and restoration path. [Bundled data provenance](../../../../finstack-quant/margin/data/margin/README.md) · [ISDA v2.6 publication](https://www.isda.org/?p=1067617) · [BCBS–IOSCO schedule and haircut tables](https://www.bis.org/bcbs/publ/d499.pdf).

The bundled interest-rate schedule rates of 1%, 2% and 4% give gross IM of USD 100, 200 and 400 on USD 10,000 of notional in the corresponding maturity buckets. This checks the rate application on three cells. Schedule bucketing and netting are covered by [records 012](12-schedule-notional.md) and [013](13-schedule-net-gross.md).

## Choice and rationale

Observed: five embedded JSON assets supply schedules, collateral, defaults, CCP proxies and historical SIMM parameters. Configuration is composed before typed parsing. Proposed rationale: one explicit parameter source makes financial defaults inspectable; it does not establish that a named regulatory profile applies to a user or transaction.

## Input domain and conventions

The overlay is a JSON value in `FinstackConfig.extensions["margin.registry.v1"]`. Schedule and haircut rates are decimal fractions; SIMM weights have class-specific bp/percentage-point units, despite the registry module's broad “rates are decimal” comment. Raw monetary defaults become the same numeric amount in the caller's chosen currency; no EUR-equivalent conversion occurs.

## Ordered algorithm

1. Parse the five `include_str!` files into one root. Recursively merge objects; replace arrays, scalars and null values wholesale. Supplying an `entries` array replaces that section's entire array, not just named entries.
2. Parse every section even when the requested calculator uses only one. Missing required sections or unknown fields in typed records fail. The live loader checks present section `schema` and `version` tags after the merge. The wire fields remain optional.
3. Expand each record's `ids` into map entries. Duplicate schedule/collateral/CCP/SIMM IDs fail; schedule rate duplicates, asset-class default duplicates and canonical correlation duplicates overwrite earlier entries. CCP requires exactly one flagged default with a nonempty IDs list.
4. Embedded data are cached with `OnceLock`; the fallible accessor returns errors. The infallible accessor used by selected collateral/CCP conveniences panics if the embedded asset cannot load. Configured builds are separately resolved and do not mutate the cache.
5. Defaults are copied, not calibrated: VM threshold 0, MTA 500,000, rounding 10,000, IA 0, daily calls and T+1; SIMM/schedule IM 10 days, threshold 50,000,000, MTA 0 and segregation; cleared IM 5 days, zero threshold, no segregation; haircut IM 2 days. Cleared VM overrides rounding to 1 and lag to 0.

## Selection, controls and limitations

`ScheduleImCalculator::from_finstack_config`, `SimmCalculator::from_finstack_config`, configured collateral/CSA/OTC constructors consume the overlay. `CsaSpec::with_im` resolves embedded methodology defaults again; `ClearingHouseImCalculator` uses embedded data or a direct `CcpParams` override. The standard/regulatory/CCP notification-response-dispute-grace timing values are metadata: respectively (13h,2h,2d,1d), (13h,2h,1d,0d), (10h,1h,1d,0d); VM generation does not implement intraday deadlines or disputes.

Registry parsing and direct constructors have different validation boundaries; parsed configuration does not establish uniform consumer validation.

## Alternatives and tradeoffs

Complete custom snapshots make the parameters reproducible and require callers to supply each used profile in full. The existing merge can combine entries from different sources inside one effective section. The accepted agreement contract assigns thresholds and transfer amounts to their dated source.

## Source and validation evidence

Embedded assets and all parser/merge paths were inspected. Existing registry tests exercise loading and rejected overlays. The local data README is a provenance lead, not independent proof of parameter identity or regulatory applicability.

Implementation and data: [src/registry/mod.rs](../../../../finstack-quant/margin/src/registry/mod.rs), [src/registry/embedded.rs](../../../../finstack-quant/margin/src/registry/embedded.rs), [src/registry/merge.rs](../../../../finstack-quant/margin/src/registry/merge.rs), [src/registry/wire.rs](../../../../finstack-quant/margin/src/registry/wire.rs), [src/registry/validation.rs](../../../../finstack-quant/margin/src/registry/validation.rs), [data/margin/defaults.v1.json](../../../../finstack-quant/margin/data/margin/defaults.v1.json), [data/margin/README.md](../../../../finstack-quant/margin/data/margin/README.md).

Existing evidence: [src/registry/mod.rs](../../../../finstack-quant/margin/src/registry/mod.rs), [tests/simplification_contracts.rs](../../../../finstack-quant/margin/tests/simplification_contracts.rs). These references distinguish parameter/formula checks, invariants and roundtrips from independently validated portfolio outputs; no pass claim is made.

## Follow-up tracking

Implementation reconciliation and qualification work is tracked in the
[collateral and initial-margin register](../FOLLOW_UPS.md#margin-collateral-and-initial-margin).
