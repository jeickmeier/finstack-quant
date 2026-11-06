# Covenants methodology inventory

**Audit status:** Draft methodology records; observed implementation, adoption pending.
**Audit date / source revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
**Exact scope:** 14 production Rust files in `finstack-quant/covenants/src`, classified as 11 algorithm files and 3 support files, covered by 13 records. Embedded unit tests remain evidence within their owning production files; dedicated test files and inspected dependencies are evidence, not additions to this source count.

The audit traces metric selection, effective schedules/windows/waivers, compliance and headroom, activation/scope, breach/cure state, consequence execution, dated observations, deterministic and stochastic forecasting, package defaults and result/wire/schema contracts. The crate consumes calculation-ready borrower measures and supplies no pricing, yield solver, LTM aggregation, equity-cure cash injection or legal-action assessment. Instrument mutation and statements/calendar adapters are explicit external boundaries.

Accepted [units/signs](../conventions/0007-units-and-sign-conventions.md), [invalid/missing distinctions](../conventions/0009-missing-and-invalid-data.md) and [monetary arithmetic boundaries](../conventions/0020-numerical-representation-and-rounding.md) remain requirements; these Draft records do not adopt template terms, stochastic assumptions or contractual precision policy. The [coverage ledger](coverage.json) records every assigned file exactly once with its supplied baseline SHA-256. Source files were inspected through their ends, including embedded tests; no runtime code was edited and no build, Rust test, schema generation or numerical experiment was run.

## Algorithm inventory

The [covenants follow-ups](../FOLLOW_UPS.md#covenants) own pending choices,
implementation reconciliation, and qualification work. Records retain the
observed behavior, limitations, and evidence supporting this inventory.

The implementation/validation column describes static evidence only. “Inspected tests” means assertions were read, not executed or independently qualified.

| Calculation / supported instruments | Implemented method / material variants | Actual selection / controls | Decision record | Implementation / validation |
| --- | --- | --- | --- | --- |
| Calculation-ready borrower measures | Supplied ratios/amounts; no ratio construction, FX or LTM aggregation | Exact caller metric and reporting-period convention | [COVENANTS-01](01-terms-and-metric-selection.md) | Observed source; metric tests inspected |
| Metric resolution and missing inputs | Explicit ID → built-in default → custom/basket name; missing errors | Explicit missing name never falls back | [COVENANTS-01](01-terms-and-metric-selection.md) | Observed; forward explicit-missing regression inspected |
| Map ingestion and finite values | Map insertion replaces; JSON requires numeric values; typed reads reject nonfinite on demand | Bare JSON parser versus prebuilt/derived wrapped map | [COVENANTS-01](01-terms-and-metric-selection.md), [COVENANTS-13](13-schema-contracts.md) | Observed; parser/source-parity tests inspected |
| Spec identities and default terms | Exact nonempty labels; active maintenance, 30-day cure, no consequences | Constructor defaults; derived serde bypasses constructors | [COVENANTS-01](01-terms-and-metric-selection.md), [COVENANTS-13](13-schema-contracts.md) | Observed; identity and net-denominator tests inspected |
| Threshold schedules | Sort unique finite date/value pairs; last date ≤ test; static fallback | Checked constructor and custom deserialization | [COVENANTS-02](02-effective-terms.md) | Observed; duplicate-date and wire tests inspected |
| Window replacement | Inclusive nonoverlapping windows replace whole spec set; outside uses base | Date selection after full-engine validation | [COVENANTS-02](02-effective-terms.md) | Observed; window regressions inspected |
| Waiver/amendment resolution | Full waiver suppresses test; amendment > schedule > static | Exact label; inclusive effective/expiry intervals | [COVENANTS-02](02-effective-terms.md) | Observed; amendment/overlap tests inspected |
| Maintenance/incurrence selection | Snapshot all scopes; tracking explicit scope; batch config scope | Maintenance batch default; single forecast supplied spec | [COVENANTS-04](04-activation-and-scope.md) | Observed; explicit-incurrence regression inspected |
| Inactive and fully waived tests | Passing report without numeric assessment; no metric reads | Active flag, then full waiver | [COVENANTS-04](04-activation-and-scope.md) | Observed; inactivity/waiver tests inspected |
| Springing activation | Inclusive minimum/maximum trigger; finite required; no metric reads if unmet | Deterministic observed trigger in all modes | [COVENANTS-04](04-activation-and-scope.md) | Observed; finite/NaN trigger tests inspected |
| Negative/affirmative covenants | No numeric assessment; snapshot pass; batch omit; single forecast errors | Type has no bound or threshold | [COVENANTS-04](04-activation-and-scope.md) | Observed; batch nonnumeric test inspected |
| Numeric bounds and basket use | Exact maxima/minima; equality passes; basket compares supplied utilization | Type comparator; custom has plain signed semantics | [COVENANTS-03](03-comparison-and-headroom.md) | Observed; equality/custom tests inspected |
| Gross/net leverage validity | Negative gross or nonpositive supplied earnings → indeterminate breach | Net denominator required; constructor default ebitda | [COVENANTS-01](01-terms-and-metric-selection.md), [COVENANTS-03](03-comparison-and-headroom.md) | Observed; net-cash/negative-earnings regressions inspected |
| Relative/absolute headroom | Signed distance / abs(threshold); denominator 1 for abs(threshold) < EPSILON | Shared point-in-time and forecast helper | [COVENANTS-03](03-comparison-and-headroom.md) | Observed; negative-threshold tests inspected |
| Breach episode capture | First failure captures values/terms; continuing failure keeps one episode | Latest uncured episode at/before test date | [COVENANTS-05](05-breach-and-cure.md) | Observed; persistent/window tests inspected |
| Cure deadlines and recovery | Checked calendar-day addition; numeric recovery only through inclusive deadline | Some(30) default; None immediate consequence and no auto-cure | [COVENANTS-05](05-breach-and-cure.md) | Observed; recovery/overflow/non-testing tests inspected |
| Consequence eligibility and progress | Resolve history key; skip future/cured/grace; replay only outstanding prefix | Caller snapshot order; current history owns state | [COVENANTS-06](06-consequence-execution.md) | Observed; stale/retry tests inspected |
| Consequence conversions and actions | bp / 10000; sweep fraction; default/block/collateral/maturity dispatch | Captured ordered consequence variants | [COVENANTS-06](06-consequence-execution.md) | Observed; retry tests and TermLoan boundary inspected |
| Consequence failure isolation | Clone/apply/commit per action; prior successes survive later error | Cloneable mutator with no external side effects | [COVENANTS-06](06-consequence-execution.md) | Observed; failed mutation regression inspected |
| Dated metric construction | Nonempty rows sorted into daily periods; duplicate dates rejected | Private series constructor; value checks deferred | [COVENANTS-07](07-dated-series.md) | Observed; dated-series unit tests inspected |
| Period date lookup | Dated series reconstructs year/ordinal; model adapter supplies inclusive dates | Generic ModelTimeSeries caller periods | [COVENANTS-07](07-dated-series.md) | Observed; convenience tests inspected |
| Series compliance evaluation | Input-order stateless reports; lazy reads; first error aborts | Both scopes; empty output skips engine validation | [COVENANTS-07](07-dated-series.md) | Observed; evaluation agreement/missing tests inspected |
| Forecast date/config validation | Strictly increasing nonempty ends; start ≤ end; explicit reference bound | Generic forecast; whole config/spec validation | [COVENANTS-08](08-forecast-composition.md), [COVENANTS-09](09-analytic-probability.md) | Observed; date/sample/config tests inspected |
| Deterministic forecast and summaries | 0/1 risk; base-case first breach and minimum finite headroom | stochastic false; same summaries also in stochastic mode | [COVENANTS-08](08-forecast-composition.md) | Observed; first-breach tests inspected |
| Batch effective-term grouping | Resolve date windows then scope; group by spec object, common anchor | Forecast each selected spec/date subset | [COVENANTS-08](08-forecast-composition.md), [COVENANTS-10](10-monte-carlo.md) | Observed; effective-term regressions inspected |
| Breach-list inclusion and ordering | Base breach always included; active stochastic p ≥ cutoff; date/id sort | Default cutoff 0.05; cutoff zero includes active zero risk | [COVENANTS-08](08-forecast-composition.md) | Observed; cutoff regression inspected |
| Stochastic clock and domains | Calendar days / 365.25; positive bases shocked; nonpositive deterministic | Explicit reference or previous day before first model start | [COVENANTS-09](09-analytic-probability.md) | Observed; dated-anchor regression inspected |
| Analytic lognormal risk | Mean-preserving GBM overlay; upper 1−Phi(z), lower Phi(z) | stochastic true and num_paths zero | [COVENANTS-09](09-analytic-probability.md) | Observed; approximate analytic fixture inspected |
| Degenerate stochastic cases | Zero sigma/time exact indicator; positive base/nonpositive bound constant risk | Shared base/threshold and activation rules | [COVENANTS-09](09-analytic-probability.md) | Observed; degenerate-base tests inspected |
| MC Brownian paths | Seed/stream PCG normal increments across eligible horizons | stochastic true and num_paths positive; absent seed 0 | [COVENANTS-10](10-monte-carlo.md) | Observed; reproducibility/convergence fixtures inspected |
| Antithetic units | Z/−Z pair averages; odd path request rounds up | ceil(paths/2), at least two independent units | [COVENANTS-10](10-monte-carlo.md) | Observed; anti/sample-count tests inspected |
| MC sampling error | Unbiased variance of units divided by unit count; clamp roundoff below zero | Plain indicators or paired average units | [COVENANTS-10](10-monte-carlo.md) | Observed; one variance-reduction fixture inspected |
| LBO package | Four maintenance tests; +200bp and distribution block; constant leverage | LboStandard; nonnegative finite thresholds | [COVENANTS-11](11-package-templates.md) | Observed; template/default tests inspected |
| Cov-lite package | Two leverage incurrence tests plus descriptive debt restriction | CovLite; no maintenance specs | [COVENANTS-11](11-package-templates.md) | Observed; template/non-numeric tests inspected |
| Real-estate package | DSCR, debt yield, LTV; separate 100%/50% sweep actions | RealEstate; quarterly metadata, 30-day inherited cures | [COVENANTS-11](11-package-templates.md) | Observed; package construction inspected |
| Project-finance package | Separate default/lockup DSCR, liquidity, net leverage | ProjectFinance; 60-day default versus 30-day other cures | [COVENANTS-11](11-package-templates.md) | Observed; lockup isolation and valid-input tests inspected |
| Reports and numeric absence | Ordered labels; optional actual/headroom; static descriptions | Builder defaults and evaluation branches | [COVENANTS-12](12-reports-and-json.md) | Observed; report/nullable-wire tests inspected |
| Report metadata and provenance | Default FinstackConfig stamp; forecast lacks resolved config/seed metadata | Report constructors and omitted-meta serde default | [COVENANTS-12](12-reports-and-json.md) | Observed; metadata code and canonical fixture inspected |
| Canonical JSON validation | Typed parse; spec/engine business checks; report structural-only; sorted keys | Core checked serializer; arrays retain order | [COVENANTS-12](12-reports-and-json.md) | Observed; canonical/strictness tests inspected |
| Schema contracts and generation | Ten roots, ISO dates, tuple schedules, serde defaults and enum tags | json-schema registry and delegated generator | [COVENANTS-13](13-schema-contracts.md) | Observed; selected schema assertions inspected |

## Records and evidence boundary

- [COVENANTS-01: Contract terms, identity and metric selection](01-terms-and-metric-selection.md)
- [COVENANTS-02: Threshold schedules, windows and waivers](02-effective-terms.md)
- [COVENANTS-03: Numeric compliance, denominator validity and headroom](03-comparison-and-headroom.md)
- [COVENANTS-04: Scope selection, springing activation and descriptive covenants](04-activation-and-scope.md)
- [COVENANTS-05: Breach episodes, cure deadlines and historical state](05-breach-and-cure.md)
- [COVENANTS-06: Consequence conversion, ordering and retry semantics](06-consequence-execution.md)
- [COVENANTS-07: Dated observations, period calendars and series evaluation](07-dated-series.md)
- [COVENANTS-08: Deterministic forecasts, effective-term grouping and breach selection](08-forecast-composition.md)
- [COVENANTS-09: Lognormal overlay and analytic marginal breach probability](09-analytic-probability.md)
- [COVENANTS-10: Path-consistent Monte Carlo, antithetic units and sampling error](10-monte-carlo.md)
- [COVENANTS-11: Deal-package assembly and embedded consequence assumptions](11-package-templates.md)
- [COVENANTS-12: Report semantics, metadata and canonical JSON boundaries](12-reports-and-json.md)
- [COVENANTS-13: Schema registry, strictness and validation boundaries](13-schema-contracts.md)

Selected evidence includes the crate's inline engine/forward/series/template/metric tests and dedicated [audit regressions](../../../../finstack-quant/covenants/tests/audit_regressions.rs), [engine conventions](../../../../finstack-quant/covenants/tests/engine_conventions.rs), [serialization](../../../../finstack-quant/covenants/tests/serialization.rs), [schema contracts](../../../../finstack-quant/covenants/tests/schema_contract.rs), and [integration assertions](../../../../finstack-quant/covenants/tests/integration.rs). Core canonical JSON, metadata, normal CDF and RNG code and the statements/TermLoan adapters were inspected at the call boundaries. Repository references identify mathematical background; they do not certify the borrower model or template provenance.

The [covenant follow-ups](../FOLLOW_UPS.md#covenants) own outstanding reconciliation, investigation, adoption, and qualification work; the records retain the observed contracts and evidence limits. The prior statements-adapter audit is complemented by this engine audit; independent engine qualification remains open.
