# Covenants methodology inventory

**Decision status:** All 13 Covenants methodology records accepted 2026-11-17.
**Audit date / source revision:** 2026-11-05 / `7e82fd03ad98346727769e88dd5ac5d84b56e9fc`.
**Exact scope:** 14 production Rust files in `finstack-quant/covenants/src`, classified as 11 algorithm files and 3 support files, covered by 13 records. Embedded unit tests remain evidence within their owning production files; dedicated test files and inspected dependencies are evidence, not additions to this source count.

The audit traces metric selection, effective schedules/windows/waivers, compliance and headroom, activation/scope, breach/cure state, consequence execution, dated observations, deterministic and stochastic forecasting, package defaults and result/wire/schema contracts. The crate consumes calculation-ready borrower measures and supplies no pricing, yield solver, LTM aggregation, equity-cure cash injection or legal-action assessment. Instrument mutation and statements/calendar adapters are explicit external boundaries.

Accepted [units and signs](../conventions/0007-units-and-sign-conventions.md), [invalid and missing distinctions](../conventions/0009-missing-and-invalid-data.md) and [monetary arithmetic boundaries](../conventions/0020-numerical-representation-and-rounding.md) apply across the catalogue. The [coverage ledger](coverage.json) records every assigned file with its original audit SHA-256. The audit reviewed source and embedded tests. All 115 Covenants Rust tests passed on 2026-11-17 against the observed implementation.

## Algorithm inventory

The [covenants follow-ups](../FOLLOW_UPS.md#covenants) own pending choices,
implementation reconciliation, and qualification work. Records retain the
observed behavior, limitations, and evidence supporting this inventory.

The implementation and validation column records the original audit evidence. The decision records track later acceptance and validation.

| Calculation / supported instruments | Implemented method / material variants | Actual selection / controls | Decision record | Implementation / validation |
| --- | --- | --- | --- | --- |
| Calculation-ready borrower measures | Supplied ratios/amounts; no ratio construction, FX or LTM aggregation | Exact caller metric and reporting-period convention | [COVENANTS-01](01-terms-and-metric-selection.md) | Observed source; metric tests inspected |
| Metric resolution and missing inputs | Explicit ID → built-in default → custom/basket name; missing errors | Accepted explicit-only binding remains pending | [COVENANTS-01](01-terms-and-metric-selection.md) | Current forward explicit-missing regression passed |
| Map ingestion and finite values | Map insertion replaces; JSON requires numeric values; typed reads reject nonfinite on demand | Accepted rejection of blank IDs and duplicate input keys remains pending | [COVENANTS-01](01-terms-and-metric-selection.md), [COVENANTS-13](13-schema-contracts.md) | Current parser and source-parity tests passed |
| Spec identities and default terms | Exact nonblank labels; generic active maintenance and 30-day cure defaults | Accepted explicit operative terms remain pending | [COVENANTS-01](01-terms-and-metric-selection.md), [COVENANTS-13](13-schema-contracts.md) | Current identity and net-denominator tests passed |
| Threshold schedules | Sort unique finite date/value pairs; last date ≤ test; static fallback | Checked constructor and custom deserialization | [COVENANTS-02](02-effective-terms.md) | Current duplicate-date and schedule wire tests passed |
| Window replacement | Inclusive nonoverlapping windows replace whole spec set; outside uses base | Date selection after full-engine validation | [COVENANTS-02](02-effective-terms.md) | Current window regressions passed |
| Waiver/amendment resolution | Full waiver suppresses test; amendment > schedule > static | Exact label and inclusive dates; waiver applicability and effective reporting are accepted and pending | [COVENANTS-02](02-effective-terms.md) | Current amendment, overlap and forecast tests passed |
| Maintenance/incurrence selection | Snapshot all scopes; tracking explicit scope; batch config scope | Explicit multi-covenant scope and completed-action evidence for incurrence tracking are accepted and pending | [COVENANTS-04](04-activation-and-scope.md) | Current explicit-incurrence regression passed |
| Inactive and fully waived tests | Currently report a pass and skip metric reads | Distinct inactive and waived outcomes are accepted and pending | [COVENANTS-04](04-activation-and-scope.md) | Current inactivity and waiver tests passed |
| Springing activation | Inclusive trigger, evaluated deterministically in all forecast modes | Distinct untriggered and incomplete outcomes are accepted and pending | [COVENANTS-04](04-activation-and-scope.md) | Three focused springing tests passed |
| Negative/affirmative covenants | Current snapshot passes, batch omits and single forecast errors | Applicable qualitative tests requiring external assessment will be visible in coverage | [COVENANTS-04](04-activation-and-scope.md) | Current batch nonnumeric regression passed |
| Numeric bounds and basket use | Inclusive maxima/minima; basket compares supplied utilization | Accepted contractual precision and rounding require implementation | [COVENANTS-03](03-comparison-and-headroom.md) | Current equality and custom comparisons passed |
| Leverage and coverage validity | Negative gross or nonpositive supplied denominator currently yields a breach | Explicit denominators for all built-in ratios and distinct indeterminate outcomes are accepted and pending | [COVENANTS-01](01-terms-and-metric-selection.md), [COVENANTS-03](03-comparison-and-headroom.md) | Current net-cash and nonpositive-earnings regressions passed |
| Absolute and relative headroom | Current shared scalar uses absolute distance near zero | Separate measures, undefined relative headroom at zero and explicit overflow are accepted and pending | [COVENANTS-03](03-comparison-and-headroom.md) | Nine focused headroom tests passed against current behavior |
| Breach episode capture | First failure captures values and terms; continuing failure usually keeps one episode | Chronological tracking, idempotent replay and completed-action identity are accepted and pending | [COVENANTS-05](05-breach-and-cure.md) | Current persistent episode and window tests passed |
| Cure deadlines and recovery | Checked calendar-day addition and inclusive deadline | Explicit cure-period choice and captured cure rule with dated evidence are accepted and pending | [COVENANTS-05](05-breach-and-cure.md) | Nine focused cure tests passed against current behavior |
| Consequence eligibility and progress | Resolve history, skip future/cured/grace and resume outstanding actions | Stable episode order and agreement-based conflict resolution are accepted and pending | [COVENANTS-06](06-consequence-execution.md) | Current stale-episode and retry regressions passed |
| Consequence conversions and actions | Convert basis points and pass sweep fractions and other captured actions | Typed target inputs and checked maturity acceleration are accepted and pending | [COVENANTS-06](06-consequence-execution.md) | TermLoan boundary inspected |
| Consequence failure isolation | Clone, apply and commit per action; earlier success survives a later error | Dated application records and visible partial results are accepted and pending | [COVENANTS-06](06-consequence-execution.md) | Seven focused consequence tests passed |
| Dated metric construction | Constructor sorts rows into daily periods and rejects duplicate dates | Shared ordering and duplicate checks across dated-row paths are accepted and pending | [COVENANTS-07](07-dated-series.md) | Current sorting and duplicate-date tests passed |
| Period date lookup | Dated series reconstructs year and ordinal; model adapter supplies inclusive dates | Exact period membership and validated chronological selections are accepted and pending | [COVENANTS-07](07-dated-series.md) | Source and convenience paths reviewed |
| Series compliance evaluation | Read-only reports with on-demand metric reads | Empty-assessment validation, incomplete outcomes and source observation classification are accepted and pending | [COVENANTS-07](07-dated-series.md) | Five focused series tests passed |
| Forecast date/config validation | Strictly increasing nonempty dates with valid bounds | Explicit modes and explicit stochastic reference date and volatility are accepted and pending | [COVENANTS-08](08-forecast-composition.md), [COVENANTS-09](09-analytic-probability.md) | Current date and configuration tests passed |
| Deterministic forecast and summaries | Base-case first breach and minimum finite headroom | Separate comparable absolute and relative minima and earliest ties are accepted and pending | [COVENANTS-08](08-forecast-composition.md) | Current first-breach tests passed |
| Batch effective-term grouping | Resolve date windows then scope; forecast each effective spec/date subset | Complete test coverage and effective-term outcomes are accepted and pending | [COVENANTS-08](08-forecast-composition.md), [COVENANTS-10](10-monte-carlo.md) | Current effective-term regressions passed |
| Breach-list inclusion and ordering | Base-case breaches plus stochastic alerts, sorted by date and label | Explicit cutoff, positive probability filter, inclusion reason and Monte Carlo error are accepted and pending | [COVENANTS-08](08-forecast-composition.md) | Current base-case inclusion regression passed |
| Stochastic clock and domains | Calendar days / 365.25; positive bases shocked | Explicit reference date and volatility on the same basis are accepted | [COVENANTS-09](09-analytic-probability.md) | Current dated-anchor regression passed |
| Analytic lognormal risk | Mean-preserving overlay; current upper tail subtracts a CDF from one | Stable log ratio and direct tails are accepted and pending | [COVENANTS-09](09-analytic-probability.md) | Two focused tests passed and independent tail arithmetic showed cancellation |
| Degenerate stochastic cases | Zero sigma/time base indicator; positive stochastic base with nonpositive threshold gives constant risk | Deterministic-only outcomes outside the valid positive domain are accepted and pending | [COVENANTS-09](09-analytic-probability.md) | Current degenerate-base behavior reviewed |
| MC Brownian paths | PCG increments across eligible horizons within each current spec group | Explicit seed and one chronological path grid per exact metric binding are accepted and pending | [COVENANTS-10](10-monte-carlo.md) | Current replay and convergence fixtures passed |
| Antithetic units | Pair averages; current odd path request rounds up | Even requested count and at least two independent units are accepted | [COVENANTS-10](10-monte-carlo.md) | Current antithetic tests passed |
| MC sampling error | Standard error from plain indicators or pair averages | Counts, endpoint flags and stable comparison are accepted and pending | [COVENANTS-10](10-monte-carlo.md) | Five focused Monte Carlo and antithetic tests passed |
| LBO package | Four maintenance tests and currently embedded consequence and cure defaults | Agreement-supplied operative terms are accepted and pending | [COVENANTS-11](11-package-templates.md) | Current template tests passed |
| Cov-lite package | Two leverage incurrence tests plus a descriptive debt restriction | Explicit terms and external-assessment outcome are accepted and pending | [COVENANTS-11](11-package-templates.md) | Current template and nonnumeric tests passed |
| Real-estate package | DSCR, debt yield and LTV; currently embeds two sweep fractions | Agreement-supplied terms and explicit sweep resolution are accepted and pending | [COVENANTS-11](11-package-templates.md) | Current template tests passed |
| Project-finance package | Separate default and lockup DSCR, liquidity and net leverage | Shared DSCR definition and strictly higher lockup floor are accepted and pending | [COVENANTS-11](11-package-templates.md) | Current template and lockup regression passed |
| Reports and numeric absence | Ordered labels, optional actual and headroom, static descriptions | Explicit outcome, test date, effective terms, typed values and cure state are accepted and pending | [COVENANTS-12](12-reports-and-json.md) | Current report and nullable serialization tests passed |
| Report metadata and provenance | Default config stamp; incomplete forecast provenance | Report applied facts and preserve historical provenance on restore | [COVENANTS-12](12-reports-and-json.md) | Provenance reconciliation is pending |
| Canonical JSON validation | Checked spec and engine inputs; report structure only; sorted object keys | Accepted report consistency checks and exact monetary values remain pending | [COVENANTS-12](12-reports-and-json.md) | All 24 current serialization tests passed |
| Schema contracts and generation | Ten current roots, ISO dates and tuple schedules | Canonical bare metric maps, checked restoration and aligned host contracts are accepted and pending | [COVENANTS-13](13-schema-contracts.md) | Both current schema tests passed |

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

The [covenant follow-ups](../FOLLOW_UPS.md#covenants) track implementation reconciliation and qualification. The statements adapter and engine records document their respective contracts and evidence limits.
