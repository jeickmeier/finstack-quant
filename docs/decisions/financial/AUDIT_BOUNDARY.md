# Methodology audit boundaries and qualification

The [algorithm catalogue](README.md) covers core, cashflows,
models, valuations, calibration, analytics, statements, statements-analytics,
scenarios, portfolio, margin, factor, attribution, covenants, features and
Arrow interchange. The former deferred domains now have detailed records,
source inventories and follow-ups.

## Source boundary

The coverage ledger enumerates all Rust files under the named crates' `src/`
trees and their Rust build generators. It includes every production valuations
family, the valuations umbrella and its builder macro crate, including runtime
items after interleaved test declarations. Embedded data and defaults that
materially select a method are inspected and linked in the owning record.
The SHA-256 ledger covers Rust source; it does not certify external inputs,
registry provenance or future data freshness.

The [root package entrypoints](../../../finstack-quant/src), Python/WASM
bindings, UI adapters and downstream host workflows are separate surfaces.
Selected caller traces may cite these surfaces, but they are outside exhaustive
source enumeration and independent host qualification. Their existence or
delegation to an audited kernel does not establish complete API, serialization
or behavioral parity. Dev-only `valuations/test-support` and `test-utils`, fuzz
targets, examples, benchmarks and external tests are supporting evidence rather
than additional production algorithm owners in this ledger.

## Evidence and adoption boundary

Detailed records distinguish decision, implementation and validation status.
Source inspection documents the implemented algorithm and its controls; it
does not accept the methodology or certify its numerical accuracy. Existing
accepted policies and prior agreed directions retain their stated scope. Across the
625 detailed audited algorithm records, choices not explicitly adopted retain
Draft or Proposed status. Accepted scopes are identified in their owning records;
accepting one scope does not adopt the record's remaining choices. All 15
[analytics records](analytics/README.md#accepted-scopes-in-detailed-records)
have complete scoped methodology acceptance as of 2026-11-07, with the
2026-11-08 implementation and its bounded [validation evidence](analytics/README.md#implementation-evidence)
recorded separately from the original documentation audit.
[Core-market record01](core-market/discount-evaluation.md#accepted-decision-explicit-discount-sources-and-checked-rate-extraction),
[record02](core-market/forward-projection.md#accepted-decision-explicit-index-projection-factors-and-contractual-rates),
[record03](core-market/rate-shocks.md#accepted-decision-explicit-zero-rate-shocks-and-dated-conditioning),
[record04](core-market/hazard-survival.md#accepted-decision-explicit-intensity-sources-and-local-probability-evaluation),
[record05](core-market/hazard-quotes-and-risk.md#accepted-decision-explicit-cds-observations-hazard-risk-and-recovery),
[record06](core-market/calibration-provenance.md#accepted-decision-lossless-construction-records-and-request-time-replay-qualification),
[record07](core-market/parametric-discount.md#accepted-decision-validated-nsnss-sources-and-analytic-factor-queries),
[record08](core-market/price-curves.md#accepted-decision-explicit-level-pillars-and-parameter-shocks),
[record09](core-market/basis-spreads.md#accepted-decision-source-derived-dated-zero-rate-differences),
[record10](core-market/inflation-projection.md#accepted-decision-explicit-cpi-levels-and-anchored-annual-parameter-shocks),
[record11](core-market/inflation-observations.md#accepted-decision-exact-monthly-fixing-snapshots-and-explicit-availability),
[record12](core-market/scalar-series-and-fixings.md#accepted-decision-validated-values-literal-sampling-and-qualified-exact-fixings),
[record13](core-market/dividend-events.md#accepted-decision-identified-cash-per-share-events-with-explicit-dates),
[record14](core-market/volatility-grids-and-shocks.md#accepted-decision-explicit-annual-volatility-sources-and-exact-grid-shocks),
[record15](core-market/fx-delta-quotes.md#accepted-decision-one-explicit-fx-quote-profile-and-qualified-forward-inputs),
[record16](core-market/sabr-cube-data.md#accepted-decision-canonical-sabr-sources-and-explicit-evaluated-quote-meaning),
[record17](core-market/forward-variance.md#accepted-decision-nonnegative-variance-sources-and-explicit-model-time-semantics),
[record18](core-market/base-correlation.md#accepted-decision-exact-base-correlation-sources-and-unclipped-native-shocks),
[record19](core-market/credit-index-composition.md#accepted-decision-explicit-credit-pool-composition-and-exact-issuer-contracts),
[record20](core-market/context-lifecycle.md#accepted-decision-typed-targets-and-atomic-context-lifecycle),
[record21](core-market/hierarchy-selection.md#accepted-decision-typed-selection-and-explicit-ordered-overlap),
and [record22](core-market/market-change-measurement.md#accepted-decision-explicit-comparable-quantities-and-ordered-measurement-rows)
have complete scoped acceptance as of 2026-11-08, with runtime/caller/host
reconciliation and independent qualification pending. **All 22 detailed
core-market records are accepted**, completing this area's methodology review.
Detailed reconciliation work stays in the financial register.
All 20 [core-conventions records 001–020](core-conventions/README.md) also have
complete scoped acceptance as of 2026-11-07, with reconciliation and
qualification pending. [Core-numerics record 01](core-numerics/compounding.md#accepted-decision-explicit-compounding-conversion-and-periodic-spreads)
has complete scoped acceptance as of 2026-11-07, with reconciliation and qualification
pending. [Record 02](core-numerics/value-interpolation.md#accepted-decision-validated-scalar-interpolation-and-boundary-semantics)
also has complete scoped acceptance as of 2026-11-07, with reconciliation and
qualification pending. [Record 03](core-numerics/forward-interpolation.md#accepted-decision-natural-log-discount-and-hagan-west-interpolation)
also has complete scoped acceptance as of 2026-11-07, with reconciliation and
qualification pending. [Record 04](core-numerics/quadrature.md#accepted-decision-checked-quadrature-budgets-and-measures)
also has complete scoped acceptance as of 2026-11-07, with reconciliation and
qualification pending. [Record 05](core-numerics/brent-root.md#accepted-decision-checked-brent-solving-and-bounded-search)
has complete scoped acceptance as of 2026-11-07, with runtime/caller
reconciliation and independent qualification pending. [Record 06](core-numerics/levenberg-marquardt.md#accepted-decision-checked-damped-least-squares-and-outcomes)
has complete scoped acceptance as of 2026-11-07, with runtime/caller/host
reconciliation and independent qualification pending. [Record 07](core-numerics/matrix-factorization.md#accepted-decision-checked-matrix-factors-solves-and-correlation-admission)
has complete scoped acceptance as of 2026-11-08, with runtime/caller/host
reconciliation and independent qualification pending. [Record 08](core-numerics/nearest-correlation.md#accepted-decision-explicit-nearest-correlation-projection-and-repair-limits)
has complete scoped acceptance as of 2026-11-08, with runtime/caller/host
reconciliation and independent qualification pending. [Record 09](core-numerics/sample-statistics.md#accepted-decision-checked-sums-moments-quantiles-and-mean-uncertainty)
has complete scoped acceptance as of 2026-11-08, with runtime/caller/host and
independent numerical/statistical qualification pending. [Record 10](core-numerics/realized-variance.md#accepted-decision-explicit-log-returns-and-realized-variance)
has complete scoped acceptance as of 2026-11-08, with runtime/caller/host and
independent numerical/financial qualification pending. [Record 11](core-numerics/covariance-shrinkage.md#accepted-decision-checked-scaled-identity-covariance-shrinkage)
has complete scoped acceptance as of 2026-11-08 for centered population/scaled-identity
shrinkage, stable moment/complement, checked result/error and host/credit-owner
reconciliation, with independent numerical/statistical/financial qualification pending; [Record 12](core-numerics/distribution-functions.md#accepted-decision-checked-special-functions-and-distribution-domains)
has complete scoped acceptance as of 2026-11-08 for checked scalar/distribution
domains, normal/log-tail, bounded Student-t beta and host/caller reconciliation,
with independent numerical/statistical/financial qualification pending;
[Record 13](core-numerics/binomial-mass.md#accepted-decision-checked-full-binomial-probability-mass)
has complete scoped acceptance as of 2026-11-08 for strict binomial domains/
resources, exact-mode/FMA recurrence, normalization, scratch/errors and Rust-only/
credit-owner boundaries, with runtime and independent qualification pending; [Record 14](core-numerics/beta-gamma-sampling.md#accepted-decision-checked-beta-and-gamma-sampling)
has complete scoped acceptance as of 2026-11-08 for checked RNG/shapes, stable
bounded MT/component beta ratios, endpoint/errors and Rust-only/model ownership,
with runtime and independent qualification pending;
[Record 15](core-numerics/chi-squared-inversion.md#accepted-decision-checked-chi-squared-log-tail-inversion)
has complete scoped acceptance as of2026-11-08 for closed endpoints/interior
domain, bounded log-Gamma/Brent, actual-returned-output tail gate and Rust-only/
copula ownership, with runtime and independent qualification pending; [Record 16](core-numerics/correlated-bernoulli.md#accepted-decision-strict-correlated-bernoulli-laws)
has complete scoped acceptance as of2026-11-08 for strict two-event feasibility/
nullable correlation, stable law/conditional/half-open sampling, canonical hosts
and financial ownership, with runtime and independent qualification pending; [Record 17](core-numerics/pseudorandom-and-bridge.md#accepted-decision-checked-random-draws-bridge-and-poisson-counts)
has complete scoped acceptance as of2026-11-08 for explicit PCG/checked normals,
capped stable bridge, bounded Poisson counts and caller/financial ownership,
with runtime and independent qualification pending;
[Record 18](core-numerics/sobol-sequences.md#accepted-decision-finite-period-sobol-points-and-prefix-hash-scrambling)
has complete scoped acceptance as of 2026-11-08 for fixed directions/order,
explicit seed mode, honest prefix hash guarantees, checked whole-point state/
period/buffers, caller reconciliation and estimator ownership, with runtime and
independent qualification pending.
[Record 19](core-numerics/fractional-covariance.md#accepted-decision-checked-fractional-increment-covariance)
has complete scoped acceptance as of 2026-11-08 for typed/unit-normalized fBM,
validated intervals/grids, stable bounded increment arithmetic/cancellation refusal,
checked mirrored matrices and caller/schema/financial ownership, with runtime and
independent qualification pending.
[Record 20](core-numerics/time-grids.md#accepted-decision-checked-simulation-grids-and-exact-event-knots)
has complete scoped acceptance as of 2026-11-08 for canonical knots/actual dts,
strict exact required events, checked lookup/access, bounded geometry/resources,
honest uniform recognition and removal of generic financial snapping with explicit
caller/model/host ownership, with runtime and independent qualification pending.
[Record 21](core-numerics/piecewise-coefficients.md#accepted-decision-checked-piecewise-coefficients-and-exponential-integration)
has complete scoped acceptance as of 2026-11-08 for right-continuous checked
coefficients, stable signed-kappa analytic integration without deadbands, scaled
positive-total range/zero rules and complete model/tree/host error propagation
with explicit financial ownership, with runtime and independent qualification
pending.
[Record 22](core-numerics/expression-evaluation.md#accepted-decision-checked-scalar-expressions-missing-values-and-decimal-quantization)
has complete scoped acceptance as of 2026-11-08 for bounded eager scalar syntax,
explicit exact-shape rows, NaN-only missing/invalid-calculation errors, three-valued
logic, exact binary-input decimal quantization and checked shared helpers with
explicit caller/statements/host/financial boundaries, with runtime and independent
qualification pending.
[Record 23](core-numerics/rounding-and-config.md#accepted-decision-explicit-rounding-config-local-tolerances-and-producer-metadata)
has complete scoped acceptance as of 2026-11-08 for explicit monetary config/
snapshots, local tolerance ownership, fixed embedded validation, optional actual
producer metadata and removal of obsolete global-zero/override/overlay paths,
with coordinated caller/host/replay qualification pending. [Record 24](core-numerics/numeric-wire-and-canonical.md#accepted-decision-strict-numeric-wire-and-canonical-content-identity)
has complete scoped acceptance as of 2026-11-08 for strict symmetric count,
exact monetary/date/sentinel wire boundaries and c2 serialized identity,
including duplicate/lossy-ingress rejection, explicit UTF-8 ordering/byte profile,
bounded single-pass hashing and existing host/cache reconciliation. Runtime and
independent qualification remain pending. [Record 25](core-numerics/contracts-and-diagnostics.md#accepted-decision-bounded-contract-loading-validation-and-diagnostics)
has complete scoped acceptance as of 2026-11-08 for bounded lossless loading,
exact monetary/owner validation, independent fatal-report status, nullable/exact
table values, bounded trace/error diagnostics, ordered execution and fail-closed
locks. Runtime/caller/host/schema reconciliation and independent qualification
remain pending. [Record 26](core-numerics/schema-publication.md#accepted-decision-canonical-schemas-compact-views-and-bounded-publication)
has complete scoped acceptance as of 2026-11-08 for original-graph/offline-corpus
schemas, exact examples, bounded canonical validation, compact authoring views
and staged publication. **All 26 core-numerics records are accepted**, completing
that area's methodology review; none remain Draft or Proposed. Runtime/caller/
host/schema reconciliation and independent qualification remain pending.
[Cashflows record001](cashflows/01-schedule-periods.md#accepted-decision-contractual-periods-calendars-and-day-count-context)
has complete scoped acceptance as of2026-11-08 for explicit contractual schedule
choices, original nominal references, ICMA/365L/termination context, coincident
events and checked immutable period queries. Its narrow core015 clarification
was applied in place: metadata endpoints describe actual calculation dates,
including explicit adjustment, separately from nominal references.
[Cashflows record002](cashflows/02-program-compilation.md#accepted-decision-coupon-and-payment-program-compilation)
has complete scoped acceptance as of 2026-11-08 for independent atomic windows,
explicit settlement/exchange/cutoff, globally valid overrides, atomic fallible
configuration and exact shared build paths.
[Cashflows record003](cashflows/03-principal-and-emission-order.md#accepted-decision-dated-opening-principal-signed-cash-and-atomic-event-phases)
has complete scoped acceptance as of 2026-11-08 for explicit dated opening
stock, signed settlement cash, atomic same-date principal batches, covered
history, paired terminal dates and exact explicit exchanges.
[Cashflows record004](cashflows/04-amortization.md#accepted-decision-explicit-amortization-laws-live-balance-installments-and-monetary-boundaries)
has complete scoped acceptance as of 2026-11-08 for explicit amortization laws,
exact fractions, protected final targets, stateless live-balance installments,
par effects and monetary/payment boundaries. Its removal of the obsolete owner
installment-update slot was clarified in accepted003 without changing the relative
economic phase order.
[Cashflows record005](cashflows/05-coupon-cash-and-pik.md#accepted-decision-parent-coupon-accrual-exact-splits-and-capitalization)
has complete scoped acceptance as of 2026-11-08 for original-parent prefix
weights, once-rounded parent amounts, one exact split share and conserved
cash/PIK allocation, truthful component metadata and capitalization. Accepted003
now explicitly points to005's generated-parent PIK unit/profile, retaining signed
internal contributions and separate explicit events.
[Cashflows record006](cashflows/06-floating-rate-projection.md#accepted-decision-resolved-term-observations-exact-rate-controls-and-strict-failures)
has complete scoped acceptance as of 2026-11-08 for resolved term observations,
publication-qualified exact fixing/projection, explicit exact rate controls and
one shared strict rate path without generic failure substitutes. It inherits
accepted market02/12 and cashflows005;005 now explicitly names006's admitted
Decimal-rate handoff before its separate parent-Money stage.
[Cashflows record007](cashflows/07-overnight-compounding.md#accepted-decision-resolved-overnight-blocks-cumulative-growth-and-parent-allocation)
has complete scoped acceptance as of 2026-11-09 for actual overnight quote
intervals, paired carry/shift blocks, stable frozen-quote growth replay and an
explicit fixed-bridge compound-rate amount allocation with reference-cap limits.
005 now names that overnight contribution/admission authority explicitly.
[Cashflows record008](cashflows/08-floating-replay.md#accepted-decision-parent-bound-replay-exact-full-settlement-and-explicit-timing-ownership)
has complete scoped acceptance as of2026-11-09 for immutable parent-bound state,
independent qualified observation/exact principal capture, shared full-parent
settlement, removal of generic partial/segment monetary paths and explicit
information/event timing ownership.
[Cashflows record009](cashflows/09-fees.md#accepted-decision-explicit-fee-parents-exact-amounts-and-literal-tier-selection)
has complete scoped acceptance as of2026-11-09 for explicit limited fee grids,
opening/integrated original-parent bases, exact whole-parent amounts, literal
fixed cash and strict nullable tier selection.
[Cashflows record010](cashflows/10-prepayment-and-mortality.md#accepted-decision-checked-monthly-mortality-explicit-model-clocks-and-benchmark-boundaries)
has complete scoped acceptance as of2026-11-09 for checked survival conversion,
whole-model admission, one-based monthly assumptions and explicit PSA/ABS/
lockout/vector boundaries, with actual consumer clocks and host reconciliation.
[Cashflows record011](cashflows/11-default-and-recovery-assumptions.md#accepted-decision-explicit-default-targets-strict-capacity-and-one-recovery-authority)
has complete scoped acceptance as of2026-11-09 for explicit conditional/target
interfaces, direct original-reference demand, strict represented capacity,
normalized timing and one recovery authority with truthful source/vintage limits.
[Cashflows record012](cashflows/12-schedule-lifecycle.md#accepted-decision-validated-complete-schedules-explicit-balances-and-separate-cash-views)
has complete scoped acceptance as of2026-11-09 for complete immutable source
schedules, explicit opening/through principal scope, exact closing replay and
separate native cash views without implicit residual normalization. Restricted
composition and actual product/export/host reconstruction retain their owners.
[Cashflows record013](cashflows/13-accrued-interest.md#accepted-decision-original-parent-cash-quotation-and-inclusive-first-ex-dates)
has complete scoped acceptance as of2026-11-09 for original-parent linear cash
quotation, explicit native Money/currency, inclusive first-ex boundaries and one
final restoration, removing generic compounding/PIK/configuration and inference.
[Cashflows record014](cashflows/14-aggregation-and-life.md#accepted-decision-exact-nominal-reporting-and-explicit-repayment-life)
has complete scoped acceptance as of2026-11-09 for exact native Money/PeriodPlan
reporting, typed cash-year ladders with separately owned qualified PV, and an
explicit ACT/365F repayment-stream statistic with owner-selected weights/dates.
Its correction of universal SIFMA WAL attribution does not adopt product WAL.
[Cashflows record015](cashflows/15-credit-adjusted-pv.md#accepted-decision-product-owned-credit-valuation-and-qualified-pv-reporting)
has complete scoped acceptance as of2026-11-09 removing generic credit-kind/FIFO
recovery valuation and schedule PV shortcuts. Actual product owners supply
qualified value attribution; existing core016 discounting/014 aggregation,
explicit unsupported outcomes and canonical-price reconciliation remain.
[Cashflows record016](cashflows/16-fixing-materialization.md#accepted-decision-qualified-hypothetical-fixing-materialization)
has complete scoped acceptance as of2026-11-09 for explicit required observations,
qualified frozen-forward candidates, exact conflicts and persistent hypothetical
source/role state, replacing date-window and unqualified history insertion.
All16 cashflow records are Accepted; the whole-file cashflow review is complete.
[Analytic-model record01](models-analytic/spot-vanilla-and-greeks.md#accepted-decision-checked-spot-prices-native-derivatives-and-explicit-composite-results)
has complete scoped acceptance as of2026-11-09 for checked spot BSM pricing,
raw derivative units, explicit singular Greek availability and composite
calculation behavior. [Analytic-model record02](models-analytic/forward-normal-shifted.md#accepted-decision-unit-forward-prices-and-native-model-greek-bundles)
has complete scoped acceptance as of2026-11-09 for common unit-forward pricing and
native Greek bundles, checked exact boundaries and original displacement identity,
with external product multipliers and explicit unit-output range limits.
[Analytic-model record03](models-analytic/implied-volatility-solvers.md#accepted-decision-explicit-bounded-model-inversion-and-paired-premium-acceptance)
was accepted on 2026-11-10 with one explicit model inverse, five required controls,
strict bounds/identifiable time, bounded core Brent with stable OTM objectives and
paired original-premium/time-value acceptance. Its complete24-point package
retains caller normalization, real product objectives and truthful failures.
[Analytic-model record 04](models-analytic/asian-moments.md#accepted-decision-for-discrete-asian-pricing)
was accepted on 2026-11-10 for one discrete Asian pricing API with prepared
fixing levels and independent payment discount. The decision covers the exact
geometric law, arithmetic moment matching on the future contribution, and
seasoned equity and commodity reconciliation.
[Analytic-model record 05](models-analytic/barriers-and-rebates.md#accepted-decision-for-continuous-barrier-premiums-and-cash)
was accepted on 2026-11-10 for checked continuous barrier premiums,
independent first-passage tails and explicit hit/no-hit cash timing with
bounded numerical work.
[Analytic-model record 06](models-analytic/lookback-extrema.md#accepted-decision-for-continuous-lookback-premiums)
was accepted on 2026-11-10 for checked inclusive continuous lookbacks with
qualified extrema, independent carry and discount, positive payoff components
and exact carry limits.
[Analytic-model record 07](models-analytic/quanto-drift.md#accepted-decision-for-fixed-conversion-quanto-premiums)
was accepted on 2026-11-10 for one checked normalized quanto price,
explicit payout-per-asset FX orientation and signed covariance, with
contractual conversion and quantity at the dated owner.
[Analytic-model record 08](models-analytic/heston-characteristic-function.md#accepted-decision-for-the-checked-heston-transform)
was accepted on 2026-11-10 for checked parameters, normalized cash and
share contours, the decaying affine logarithm and exact boundary laws.
[Analytic-model record 09](models-analytic/heston-fourier-integration.md#accepted-decision-for-bounded-heston-fourier-pricing)
was accepted on 2026-11-10 for one checked Heston price, exact singular
payoffs, stable OTM inversion, conservative tail enclosure and bounded mesh work.
[Analytic-model record 10](models-analytic/cos-inversion.md#accepted-decision-for-checked-cos-pricing)
was accepted on 2026-11-10 for direct bounded COS payoffs with explicit premium tolerance,
separate window and series bounds, and capped work.
[Analytic-model record 11](models-analytic/jump-characteristic-laws.md#accepted-decision-for-checked-characteristic-laws)
was accepted on 2026-11-10 for checked forward-normalized Black, Merton and variance-gamma
laws with selected contours, exact moment domains and support.
[Analytic-model record 12](models-analytic/rough-heston-riccati.md#accepted-decision-for-checked-rough-heston-pricing)
was accepted on 2026-11-10 for checked rough Heston Lewis pricing with explicit tolerances,
an analytic tail enclosure, separate time and mesh checks, and bounded work.
[Analytic-model record 13](models-analytic/sabr-expansions.md#accepted-decision-for-explicit-checked-sabr-quotes)
was accepted on 2026-11-10 for explicit SABR quote conventions, canonical parameter admission,
exact CEV distance, stable skew and Hagan midpoint time correction.
[Analytic-model record 14](models-analytic/sabr-calibration.md#accepted-decision-for-explicit-checked-sabr-calibration)
was accepted on 2026-11-10 for explicit SABR fitting, native quote-error acceptance,
literal shifts and an observed-ATM pin with a checked alpha bracket.
[Analytic-model record 15](models-analytic/sabr-smile-repair.md#accepted-decision-for-checked-sabr-smile-and-finite-grid-repair)
was accepted on 2026-11-10 for the checked ATM delta proxy, unit call grid diagnostics
and exact down-only repair with move and repricing gates.
[Analytic-model record 16](models-analytic/volatility-source-interpolation.md#accepted-decision-for-explicit-volatility-source-evaluation-and-materialization)
was accepted on 2026-11-10 for typed source evaluation, exact support and explicit boundaries,
native sigma and total-variance interpolation, qualified SABR cube modes,
strict materialization and native-unit reference moves.
[Analytic-model record 17](models-analytic/fx-delta-smiles.md#accepted-decision-for-checked-fx-delta-smiles-and-strike-materialization)
was accepted on 2026-11-10 for checked FX delta strikes and labelled pillars, strict-expiry quote
evaluation and explicit strike wings, plus exact-union materialization
with qualified row forwards.
[Analytic-model record 18](models-analytic/svi-fit-and-density.md#accepted-decision-for-checked-raw-svi-and-slice-fit-acceptance)
was accepted on 2026-11-10 for checked raw SVI, exact shape and wing boundaries, a finite density
scan, one seeded checked fit and separate annual-volatility and sampled-density
acceptance.
[Analytic-model record 19](models-analytic/dupire-local-volatility.md#accepted-decision-for-checked-dupire-extraction-and-local-volatility-grids)
was accepted on 2026-11-10 for checked fixed-moneyness Dupire stencils, explicit extraction
support and smoothing, and checked local sigma grids and simulation
consumption.
[Analytic-model record 20](models-analytic/surface-arbitrage-diagnostics.md#accepted-decision-for-checked-surface-arbitrage-diagnostics-and-coverage)
was accepted on 2026-11-10 for checked sampled call-price, common-support calendar and Dupire
sign checks, with explicit coverage and unfiltered status.
[Analytic-model record 21](models-analytic/variance-replication.md#accepted-decision-for-checked-variance-replication-and-explicit-black-wings)
was accepted on 2026-11-10 for checked forward-split variance replication, an explicit mesh
gate and selected finite-strip or full flat-Black wings. The other
seven records are Draft.
Runtime/caller/host/state reconciliation and independent qualification remain pending. Other financial records
and convention ADRs retain their separately stated statuses; describing an
observed method does not approve it.

Selected tests and references were inspected; this does not claim review of
every test body. Numerical suites, simulation experiments and vendor comparisons
were not executed for these documentation audits. Independent semantic reviews
sample records; they do not mathematically certify every implementation or
qualify a release. The first batch's core reference-data consistency check compares
maintained data with generated tables; it does not verify official holidays.
Accounting/regulatory labels and embedded versions do not establish current
compliance. Independent provenance, convergence, domain coverage and justified
tolerances remain specific follow-ups in the
[review register](FOLLOW_UPS.md).

Maintain these boundaries under
[ADR 0026](governance/0026-methodology-decisions-and-inventory.md). Trace actual callers,
update the record and source mapping after substantive changes, and qualify
consumers separately where their units, defaults, state or failure behavior
differ from the shared kernel.
