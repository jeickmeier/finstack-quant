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
gate and selected finite-strip or full flat-Black wings.
[Analytic-model record 22](models-analytic/atm-volatility-conversion.md#accepted-decision-for-checked-price-equivalent-atm-volatility-conversion)
was accepted on 2026-11-10 for literal at-the-forward conversion using checked analytic
pricing, direct inversion and premium replay in the smaller boundary
component.
[Analytic-model record 23](models-analytic/hull-white-analytic-kernels.md#accepted-decision-for-checked-hullwhite-analytic-kernels-and-payment-measures)
was accepted on 2026-11-10 for checked scheduled Hull–White moments
and affine bonds, explicit payment-measure caplets and coupons,
exact simple futures bias and a price-equivalent term ATM Normal quote.
[Analytic-model record 24](models-analytic/rate-clock-and-projection.md#accepted-decision-for-checked-model-clocks-curve-fit-and-term-projection)
was accepted on 2026-11-10 for checked dated clock mapping, an exact
deterministic curve shift, contractual term-period reconstruction and
active fitted-state rules.
[Analytic-model record 25](models-analytic/hw1f-monte-carlo-discounting.md#accepted-decision-for-exact-joint-hullwhite-paths-and-sampling)
was accepted on 2026-11-10 for exact joint Hull–White state and log-bank
steps, explicit event and stream controls, and pair-based sampling
uncertainty.
[Analytic-model record 26](models-analytic/hw1f-issuer-lsmc.md#accepted-decision-for-checked-issuer-call-policy-training-and-pricing)
was accepted on 2026-11-10 for dated issuer-call cash, checked
continuation regression and independent pricing of a frozen policy.
[Analytic-model record 27](models-analytic/diebold-li-var.md#accepted-decision-for-checked-nelsonsiegel-factors-and-var-forecasts)
was accepted on 2026-11-10 for explicit observation cadence, checked
Nelson–Siegel factor extraction, stable VAR dynamics and marginal
fitted-yield forecasts.
[Analytic-model record 28](models-analytic/yield-pca.md#accepted-decision-for-checked-yield-change-pca-and-scenarios)
was accepted on 2026-11-11 for checked yield-change PCA, full-spectrum
variance accounting and fitted-basis reconstruction and scenarios.
All 28 analytic-model records are Accepted.
[Numerical-model record 001](models-numerical/equity-binomial-construction.md#accepted-decision-for-checked-crr-and-leisenreimer-lattices)
was accepted on 2026-11-11 for checked CRR and Leisen–Reimer
transitions, explicit lattice counts and exact vanilla payoff limits.
[Numerical-model record 002](models-numerical/dividends-and-lattice-exercise.md#accepted-decision-for-checked-escrowed-dividends-and-lattice-exercise)
was accepted on 2026-11-11 for dated ex-claim values, a common escrow
reserve horizon, exact alignment and product-qualified exercise phases.
[Numerical-model record 003](models-numerical/tree-discounting-and-induction.md#accepted-decision-for-checked-tree-discounting-and-rollback)
was accepted on 2026-11-11 for checked interval discounting, explicit
nodewise OAS, validated rollback and genuine terminal state.
[Numerical-model record 004](models-numerical/ho-lee-curve-fit.md#accepted-decision-for-checked-holee-curve-fitting)
was accepted on 2026-11-11 for origin-relative Ho–Lee targets,
checked additive row fits and one repricing gate.
[Numerical-model record 005](models-numerical/bdt-lognormal-curve-fit.md#accepted-decision-for-checked-bdt-curve-fitting)
was accepted on 2026-11-11 for strictly decreasing BDT targets,
unclamped log-rate fits and one inclusive replay gate.
[Numerical-model record 006](models-numerical/black-karasinski-trinomial.md#accepted-decision-for-checked-bk-trinomial-fitting)
was accepted on 2026-11-11 for exact conditional OU moments on a
capped trinomial, positive rate fitting and one inclusive replay gate.
[Numerical-model record 007](models-numerical/hull-white-event-grid-and-affine-bonds.md#accepted-decision-for-checked-hullwhite-event-lattices-and-node-bonds)
was accepted on 2026-11-11 for exact Hull–White event grids,
conditional OU moments, Continuous base fitting and explicit
affine exercise valuation.
[Numerical-model record 008](models-numerical/rates-credit-marginal-calibration.md#accepted-decision-for-checked-rates-credit-marginal-calibration)
was accepted on 2026-11-11 for centered binary marginals,
strict variance retention, local integrated-hazard fitting
and checked replay.
[Numerical-model record 009](models-numerical/rates-credit-coupling-and-sampling.md#accepted-decision-for-checked-rates-credit-coupling-and-path-sampling)
was accepted on 2026-11-11 for strict Bernoulli coupling, indexed
Philox midpoints and bound segment replay.
[Numerical-model record 010](models-numerical/rates-credit-coupon-folding.md#accepted-decision-for-checked-joint-credit-rollback-and-coupon-folding)
was accepted on 2026-11-11 for checked joint survival and recovery,
projection on exact index dates and a full coupon fold with explicit
exercise eligibility.
[Numerical-model record 011](models-numerical/pde-grids-interpolation-and-greeks.md#accepted-decision-for-checked-pde-grids-interpolation-and-coordinate-greeks)
was accepted on 2026-11-11 for validated spatial grids, closed-domain
value extraction and interpolated quadratic nodal derivative estimates.
[Numerical-model record 012](models-numerical/pde-spatial-stencils.md#accepted-decision-for-checked-directional-and-mixed-pde-stencils)
was accepted on 2026-11-11 for checked diffusion admission, an inclusive
central-to-one-sided drift switch and an explicit tensor mixed stencil.
[Numerical-model record 013](models-numerical/pde-boundary-elimination.md#accepted-decision-for-checked-pde-boundary-elimination-and-reconstruction)
was accepted on 2026-11-11 for shared checked edge relations, explicit
x-edge corner ownership and stage-consistent reconstruction and cache reuse.
[Numerical-model record 014](models-numerical/theta-time-march-and-thomas.md#accepted-decision-for-checked-theta-marching-and-tridiagonal-solves)
was accepted on 2026-11-11 for exact event-grid theta steps, full-interval
damping and one checked Thomas kernel with a componentwise residual gate.
[Numerical-model record 015](models-numerical/pde-exercise-projection.md#accepted-decision-for-checked-grid-based-pde-exercise)
was accepted on 2026-11-11 for exact post-step payoff comparison,
exact exercise knots and actual-spot immediate exercise at eligible
equity valuations.
[Numerical-model record 016](models-numerical/mcs-adi-time-march.md#accepted-decision-for-checked-modified-craigsneyd-adi)
was accepted on 2026-11-11 for fixed full MCS, checked factorized startup
and atomic stages with application-specific stability and price qualification.
[Numerical-model record 017](models-numerical/black-scholes-and-heston-pde-bridges.md#accepted-decision-for-checked-pde-coefficient-bridges)
was accepted on 2026-11-11 for checked bridge parameters and diffusion,
explicit edge laws, exact deterministic limits and qualified price and
risk boundaries. All 17 trees and finite differences records are accepted.
Runtime/caller/host/state reconciliation and independent qualification remain pending.

The first Monte Carlo package was accepted on 2026-11-11. Its evidence includes
source and primary references, plus independent arithmetic checks for event
time, correlation, discounting, antithetic counts and optional stopping.
Production execution, host parity and pricing qualification are pending.
[Monte Carlo record 01](models-monte-carlo/01-engine-and-estimation.md#accepted-decision-for-checked-monte-carlo-execution-and-estimation).

The second Monte Carlo package was accepted on 2026-11-11. Independent
checks cover Random123 words, open midpoint and complement arithmetic,
reflected normals, fill capacity, uniform offsets and FNV seed framing.
Runtime, host replay and model qualification are pending.
[Monte Carlo record 02](models-monte-carlo/02-random-streams.md#accepted-decision-for-checked-philox-streams-and-seed-derivation).

The third Monte Carlo package was accepted on 2026-11-11. Independent
checks cover dyadic budget admission, seed and address arithmetic, bridge
geometry, equal weighting and finite-grid limits. Runtime, host and price
qualification are pending.
[Monte Carlo record 03](models-monte-carlo/03-randomized-qmc.md#accepted-decision-for-whole-path-sobol-pricing-and-replicate-diagnostics).

The fourth Monte Carlo package was accepted on 2026-11-11. Independent
checks cover transition range, cumulative carry, log covariance,
dated ATM variance and local-volatility steps. Runtime, host parity
and production pricing qualification are pending.
[Monte Carlo record 04](models-monte-carlo/04-lognormal-diffusions.md#accepted-decision-for-checked-gbm-and-local-volatility-paths).

The fifth Monte Carlo package was accepted on 2026-11-11. Independent
checks cover Euler range, additive Brownian covariance, exact OU limits
and the effect of unequal reversion speeds on step covariance.
Runtime, host parity and model qualification are pending.
[Monte Carlo record 05](models-monte-carlo/05-generic-steppers.md#accepted-decision-for-checked-coordinatewise-euler-and-exact-gaussian-steps).

The sixth Monte Carlo package was accepted on 2026-11-11. Independent
checks cover delayed cash claim values, mixed jump order, adjacent
knots, small positive widths and floor-sensitive forward moments.
Runtime, host parity and financial qualification are pending.
[Monte Carlo record 06](models-monte-carlo/06-cash-dividend-jumps.md#accepted-decision-for-checked-discrete-dividend-jumps-on-exact-grid-knots).

The seventh Monte Carlo package was accepted on 2026-11-11. Independent
checks cover scheduled endpoint moments, exact stored times, extreme
mean and noise arithmetic, and the difference between mean-level fit
and discount replay. Runtime, host and product qualification are pending.
[Monte Carlo record 07](models-monte-carlo/07-hull-white.md#accepted-decision-for-checked-hullwhite-endpoints-and-curve-fit-boundaries).

The eighth Monte Carlo package was accepted on 2026-11-11. Independent
checks cover conditional moments, the switch boundary, the positive
mixture tail, log-moment range, zero atoms and signed CIR++ projection.
Runtime, host and product qualification are pending.
[Monte Carlo record 08](models-monte-carlo/08-cir-qe.md#accepted-decision-for-checked-cir-and-cir-quadratic-exponential-paths).

The ninth Monte Carlo package was accepted on 2026-11-11. Independent Heston checks
cover the QE mixture atom, both moment poles, centered leverage,
deterministic variance and exact carry endpoints. Runtime, host and
product qualification are pending.
[Monte Carlo record 09](models-monte-carlo/09-heston-qe.md#accepted-decision-for-checked-heston-qe-m-paths).

The tenth Monte Carlo package was accepted on 2026-11-11. Independent LMM
examples cover displaced support, checked step arithmetic, exact tenor and
payment-bond identities and finite-step bias in deflated bond moments.
Runtime, host and product qualification are pending.
[Monte Carlo record 10](models-monte-carlo/10-lmm.md#accepted-decision-for-checked-terminal-measure-lmm-paths).

The eleventh Monte Carlo package was accepted on 2026-11-11. Independent checks
cover clipped utilization moments, left-spread coupling, exact limits,
scaled arithmetic and latent-normal and bank covariances. Runtime,
host and product qualification are pending.
[Monte Carlo record 11](models-monte-carlo/11-credit-line.md#accepted-decision-for-checked-revolving-credit-paths).

The twelfth Monte Carlo package was accepted on 2026-11-11. Independent
Schwartz–Smith checks cover covariance and partition moments, near-singular variance,
factor initialization and one-expiry log forward pinning. Runtime, host
and product qualification are pending.
[Monte Carlo record 12](models-monte-carlo/12-schwartz-smith.md#accepted-decision-for-checked-schwartzsmith-paths-and-expectations).

The thirteenth Monte Carlo package was accepted on 2026-11-11. Independent
fractional noise checks cover window covariance, newest-cell conditioning, actual
grid geometry, sampled RL variance and Gaussian compensation. Runtime,
host and product qualification are pending.
[Monte Carlo record 13](models-monte-carlo/13-fractional-noise.md#accepted-decision-for-checked-fractional-noise-and-volterra-drivers).

The fourteenth Monte Carlo package was accepted on 2026-11-11.
Independent rough Bergomi checks cover variance break alignment, numerical scale,
finite-grid moments, parity and a two-cell option benchmark. Runtime,
host and product qualification are pending.
[Monte Carlo record 14](models-monte-carlo/14-rough-bergomi.md#accepted-decision-for-checked-rough-bergomi-spot-and-pricing).

The fifteenth Monte Carlo package was accepted on 2026-11-11.
Independent rough Heston checks cover actual-knot weights, newest-cell
covariance, the projected zero atom and two-cell European pricing.
Runtime, host and product qualification are pending.
[Monte Carlo record 15](models-monte-carlo/15-rough-heston.md#accepted-decision-for-checked-rough-heston-paths-and-pricing).

The sixteenth Monte Carlo package was accepted on 2026-11-11.
Independent rough Cheyette checks cover amplitude units, coarse-grid
positivity, Gaussian endpoint moments, within-cell covariance and the
discount integration boundary. Runtime, host, grid-convergence and
financial qualification are pending.
[Monte Carlo record 16](models-monte-carlo/16-rough-cheyette.md#accepted-decision-for-checked-rough-cheyette-paths).

The seventeenth Monte Carlo package was accepted on 2026-11-11.
Independent checks cover signed terminal settlements, strict digital
ties, scaled arithmetic, repeated Asian fixings and a tied-grid geometric
control. Runtime, host and product qualification are pending.
[Monte Carlo record 17](models-monte-carlo/17-vanilla-and-asian.md#accepted-decision-for-checked-terminal-and-asian-payoffs).

The eighteenth Monte Carlo package was accepted on 2026-11-11.
Independent barrier checks cover small crossing probabilities, exact
carry breaks, first-passage timing and discounting, and continuous GBM parity.
Runtime, host, Heston refinement and product qualification are pending.
[Monte Carlo record 18](models-monte-carlo/18-barrier-monitoring.md#accepted-decision-for-checked-monte-carlo-barrier-monitoring-and-rebates).

The nineteenth Monte Carlo package was accepted on 2026-11-11.
Independent lookback checks cover terminal eligibility, historical-only
settlement, conditional extrema, small excursions, carry breaks and
continuous GBM prices.
Runtime, host and dated product qualification are pending.
[Monte Carlo record 19](models-monte-carlo/19-lookback-monitoring.md#accepted-decision-for-checked-monte-carlo-lookback-extrema).

The twentieth Monte Carlo package was accepted on 2026-11-11.
Independent autocallable checks cover coupon and call ordering, memory,
exact discrete downside touch, all three redemption formulas and distinct
dated cash. Runtime, host and dated product qualification are pending.
[Monte Carlo record 20](models-monte-carlo/20-autocallable.md#accepted-decision-for-checked-monte-carlo-autocallable-payoffs).

The twenty-first Monte Carlo package was accepted on 2026-11-11.
Independent cliquet checks cover strike setting, both accumulation modes,
fully observed settlement, mixed-scale arithmetic and one- and two-period
prices. Runtime, host and dated product qualification are pending.
[Monte Carlo record 21](models-monte-carlo/21-cliquet.md#accepted-decision-for-checked-monte-carlo-cliquet-payoffs).

The twenty-second Monte Carlo package was accepted on 2026-11-11.
Independent range accrual checks cover tied rows, fixed counts, inclusive
boundaries, scaled coupon arithmetic, dated carry and marginal price benchmarks.
Runtime, host and dated product qualification are pending.
[Monte Carlo record 22](models-monte-carlo/22-range-accrual.md#accepted-decision-for-checked-monte-carlo-range-accrual).

The twenty-third Monte Carlo package was accepted on 2026-11-11.
Independent examples cover the exact exercise clock, antithetic group
admission, frozen origin selection, look-ahead, deterministic exercise
and an optimal two-date GBM put.
Runtime, host and financial qualification are pending.
[Monte Carlo record 23](models-monte-carlo/23-lsmc-exercise.md#accepted-decision-for-checked-holder-exercise-policy-training-and-pricing).

The twenty-fourth Monte Carlo package was accepted on 2026-11-11.
Independent examples cover basis equivalence, shifted standardization,
singular rank, frozen replay and manufactured multivariate continuation.
Runtime, host and financial qualification are pending.
[Monte Carlo record 24](models-monte-carlo/24-regression-policies.md#accepted-decision-for-checked-continuation-regression).

The twenty-fifth Monte Carlo package was accepted on 2026-11-11.
Independent examples cover rounded spot geometry, stored-PV
cancellation, antithetic group uncertainty, work counts and
European GBM finite-stencil targets.
Runtime, host and broader sensitivity qualification are pending.
[Monte Carlo record 25](models-monte-carlo/25-finite-difference-greeks.md#accepted-decision-for-checked-paired-monte-carlo-spot-greeks).

The twenty-sixth Monte Carlo package was accepted on 2026-11-11.
Independent examples cover terminal and joint scores, rounded shock
loss, scaled Greek products and antithetic weighting. Runtime, host and broader
sensitivity qualification are pending.
[Monte Carlo record 26](models-monte-carlo/26-likelihood-ratio-greeks.md#accepted-decision-for-checked-gbm-likelihood-ratio-greeks).

The twenty-seventh Monte Carlo package was accepted on 2026-11-11.
Independent examples cover same-sample bias, grouped fitting, constant
and tiny controls, and the ideal seasoned Asian control expectation
and payment discount.
Runtime, dated product and host qualification are pending.
[Monte Carlo record 27](models-monte-carlo/27-control-variate.md#accepted-decision-for-checked-single-control-monte-carlo-pricing).

The twenty-eighth Monte Carlo package was accepted on 2026-11-11.
Independent examples cover fixed-size membership, storage boundaries,
actual knots, subset quantiles and timed cash export. Runtime, host and
financial qualification are pending.
[Monte Carlo record 28](models-monte-carlo/28-capture-and-simulation-contracts.md#accepted-decision-for-checked-path-capture-and-raw-simulation).

The first credit and liquidity package was accepted on 2026-11-11.
Independent examples cover selected distress-score coefficients,
boundary classification, historical SIZE scaling, leverage basis and
numerical range. Runtime, host and empirical qualification are pending.
[Credit record 01](models-and-valuations-credit/01-financial-distress-scores.md#accepted-decision-for-checked-financial-distress-scores).

The second credit and liquidity package was accepted on 2026-11-11.
Independent examples cover the conditional Gaussian mean, endpoint
and cohort semantics, complete PD bands and exact factor selection.
Runtime, host, empirical and regulatory qualification are pending.
[Credit record 02](models-and-valuations-credit/02-pd-scales-and-rating-data.md#accepted-decision-for-checked-pd-transforms-and-explicit-rating-data).

The third credit and liquidity package was accepted on 2026-11-11.
Independent examples cover two-state incidence, generator correction and
fit residuals, a valid complex-spectrum chain and competing absorbing states.
Runtime, host, sampling and empirical qualification are pending.
[Credit record 03](models-and-valuations-credit/03-rating-migration.md#accepted-decision-for-checked-rating-migration-generator-fitting-and-paths).

The fourth credit and liquidity package was accepted on 2026-11-11.
Independent examples cover first-passage and jump tails, CreditGrades
origin conditioning, equity and PD reprice, multiple CDS proxy roots,
near-par debt loss and asset-path numerical boundaries. Runtime, host,
market and empirical qualification are pending.
[Credit record 04](models-and-valuations-credit/04-structural-default.md#accepted-decision-for-checked-structural-default-models-and-calibrated-outputs).

The fifth credit and liquidity package was accepted on 2026-11-11.
Independent examples cover leverage and range, local hazard and CIR
anchoring, sigmoid tails, origin touch, paired payoffs and discrete
monitoring. Runtime, host, market and issuer-policy qualification are pending.
[Credit record 05](models-and-valuations-credit/05-hazard-and-toggle.md#accepted-decision-for-checked-leverage-hazard-and-pik-election-models).

The sixth credit and liquidity package was accepted on 2026-11-12.
Independent examples cover dilution and EAD range, workout timing,
downturn and beta tails, and pledged-pool and priority conservation.
Runtime, host, empirical and regulatory qualification are pending.
[Credit record 06](models-and-valuations-credit/06-recovery-and-lgd.md#accepted-decision-for-checked-exposure-loss-and-recovery-allocations),
[core beta-quantile amendment](core-numerics/distribution-functions.md#accepted-amendment-checked-rust-beta-quantile).

The seventh credit and liquidity package was accepted on 2026-11-12.
Independent examples cover tiny CDR, Student-t shared scales, lognormal
intensity and capped-rate expectations, persistence, granularity and
draw alignment. Runtime, host and product qualification are pending.
[Credit record 07](models-and-valuations-credit/07-pool-defaults.md#accepted-decision-for-checked-pool-default-rates-and-period-copula-resolution).

The eighth credit and liquidity package was accepted on 2026-11-12.
Independent examples cover nonlinear SMM expectations, refinancing and
burnout, regime persistence and transition limits, and matrix shocks.
Runtime, host, market and product qualification are pending.
[Credit record 08](models-and-valuations-credit/08-prepayment-and-pool-correlation.md#accepted-decision-for-checked-prepayment-and-pool-correlation).

The ninth credit and liquidity package was accepted on 2026-11-12.
Independent examples cover probability tails, random-loading calibration,
fixed quadrature, paired Student-t integration and factor coordinates.
Runtime, host and product qualification are pending.
[Credit record 09](models-and-valuations-credit/09-copula-kernels.md#accepted-decision-for-checked-copula-laws-and-latent-factors).

The tenth credit and liquidity package was accepted on 2026-11-12.
Independent examples cover loading admission, shared scales, recovery on
default, role addressing, empirical tail mass and tranche boundaries.
Runtime, host, calibration and sampling qualification are pending.
[Credit record 10](models-and-valuations-credit/10-portfolio-loss-and-correlated-recovery.md#accepted-decision-for-checked-portfolio-loss-and-correlated-recovery).

The eleventh credit and liquidity package was accepted on 2026-11-12.
Independent examples cover quote geometry, spread and turnover estimators,
execution units and bucket scores, spread stress and liquidation capacity.
Runtime, host and empirical qualification are pending.
[Credit record 11](models-and-valuations-credit/11-liquidity.md#accepted-decision-for-checked-liquidity-and-execution-estimates).

The twelfth credit and liquidity package was accepted on 2026-11-13.
Independent examples cover included maturity, rolled coupons, mixed clocks,
default settlement, stable integration and quote/settlement algebra. Runtime,
host, calibration and external-reference qualification are pending.
[Credit record 12](models-and-valuations-credit/12-cds-legs.md#accepted-decision-for-checked-cds-legs-and-quote-basis).

The thirteenth credit and liquidity package was accepted on 2026-11-13.
Independent examples cover bump normalization, rate and recovery bases,
covered loss, default settlement and the expired margin proxy. Runtime,
host, calibration and margin qualification remain pending.
[Credit record 13](models-and-valuations-credit/13-cds-and-index-risk.md#accepted-decision-for-checked-cds-and-index-risk).

The fourteenth credit and liquidity package was accepted on 2026-11-13.
Independent examples cover original-weight allocation, clean and dirty
quotes, retained earned cash, confirmed events and default timing. Runtime,
host and external-reference qualification remain pending.
[Credit record 14](models-and-valuations-credit/14-index-valuation.md#accepted-decision-for-checked-cds-index-valuation-and-cashflows).

The fifteenth credit and liquidity package was accepted on 2026-11-13.
Independent examples cover the two-root case, the replacement fit and
parity, settlement normalization, front-end protection and strike factors.
Runtime, host and external-reference qualification remain pending.
[Credit record 15](models-and-valuations-credit/15-cds-option-quadrature.md#accepted-decision-for-checked-cds-option-valuation).

The sixteenth credit and liquidity package was accepted on 2026-11-13.
Independent examples cover matched spread stencils, clock decay and
parity, rate and recovery bases, and inversion near structural or numerical
flatness. Runtime, market replay, host and external qualification remain pending.
[Credit record 16](models-and-valuations-credit/16-cds-option-risk-and-inversion.md#accepted-decision-for-checked-cds-option-risk-and-implied-volatility).

The seventeenth credit and liquidity package was accepted on 2026-11-13.
Independent examples cover seasoned geometry, exact and split-grid
expectations, factor endpoints and inconsistent tranchelets. Runtime,
calibration, dated market and host qualification remain pending.
[Credit record 17](models-and-valuations-credit/17-tranche-loss-distribution.md#accepted-decision-for-checked-finite-pool-tranche-losses).

The eighteenth credit and liquidity package was accepted on 2026-11-13.
Independent examples cover daily cash and discount dates, heterogeneous
timing, maturity and partial settlement, and clean versus actual upfront.
Runtime, dated market, calibration and host qualification remain pending.
[Credit record 18](models-and-valuations-credit/18-tranche-projection.md#accepted-decision-for-checked-tranche-cashflows-and-settlement).

The nineteenth credit and liquidity package was accepted on 2026-11-13.
Independent examples cover correlation boundaries, credit and rate stencils,
recovery bases, economic default risk and asymmetric allowances. Runtime,
replay, dated market and host qualification remain pending.
[Credit record 19](models-and-valuations-credit/19-tranche-risk.md#accepted-decision-for-checked-tranche-risk-and-default-scenarios).

The twentieth credit and liquidity package was accepted on 2026-11-13.
Independent examples cover holder PV, hurdle ties, face-value ratios,
exact decimal gain, repurchase prices, fees and gross leverage. Runtime
and host qualification remain pending.
[Credit record 20](models-and-valuations-credit/20-liability-management.md#accepted-decision-for-checked-exchange-and-liability-management-arithmetic).

The first shared valuation framework package was accepted on 2026-11-13.
Independent examples cover composite dispatch, raw risk precision, duplicate
input keys, active volatility source selection and metric identity.
Runtime, replay and host qualification remain pending.
[Valuation framework record 01](valuations-framework/pricing-lifecycle.md#accepted-decision-for-checked-valuation-dispatch-and-lifecycle).

The second shared valuation framework package was accepted on 2026-11-13.
Independent examples cover volatility-clock conversion, Hull–White knot
precision, dated GBM carry and exact seed transport. Runtime, fit replay and
host qualification remain pending.
[Valuation framework record 02](valuations-framework/model-resolution.md#accepted-decision-for-checked-valuation-model-inputs-and-controls).

The third shared valuation framework package was accepted on 2026-11-13.
Independent examples cover reference changes, clearing cash, Black and Normal
pricing, dated lattice discounting and false Gamma stability. Runtime, venue
settlement, lattice accuracy and host qualification remain pending.
[Valuation framework record 03](valuations-framework/listed-products.md#accepted-decision-for-checked-listed-futures-and-options).

The fourth shared valuation framework package was accepted on 2026-11-13.
Independent examples cover full and clipped quote periods, holiday weights,
same-day publication changes, cutoff derivatives and numerical cancellation.
Runtime, dated fixing and host qualification remain pending.
[Valuation framework record 04](valuations-framework/overnight-projection.md#accepted-decision-for-checked-overnight-projection-and-quote-risk).

The fifth shared valuation framework package was accepted on 2026-11-13.
Independent examples cover exact dates, source interpolation, local hazard,
variance retention, recovery and floating coupon normalization. Runtime,
product-price, risk and host qualification remain pending.
[Valuation framework record 05](valuations-framework/rates-credit-grid.md#accepted-decision-for-checked-dated-rates-credit-valuation).

The sixth shared valuation framework package was accepted on 2026-11-13.
Independent examples cover lagged and same-day cash, term and overnight
accrual, signed legs and spread replacement scope. Runtime, dated product
and host qualification remain pending.
[Valuation framework record 06](valuations-framework/trs-legs.md#accepted-decision-for-checked-trs-legs-and-par-spread).

The seventh shared valuation framework package was accepted on 2026-11-13.
Independent examples cover holiday cadence, publication counts, variance
estimators, exact grid lookup and deferred barrier monitoring. Runtime,
product-price and host qualification remain pending.
[Valuation framework record 07](valuations-framework/observation-grids.md#accepted-decision-for-checked-observation-schedules-and-monitoring-grids).

The eighth shared valuation framework package was accepted on 2026-11-13.
Independent examples cover dependency order, key and reporting collisions,
sparse results, unit normalization and signed FX aggregation. Runtime,
reporting and host qualification remain pending.
[Valuation framework record 08](valuations-framework/metric-orchestration.md#accepted-decision-for-checked-metric-orchestration-and-reporting).

The ninth shared valuation framework package was accepted on 2026-11-13.
Independent examples cover represented stencils, direct and quote DV01,
nonlinear bucket sums and volatility shock direction. Runtime,
calibration-replay and host qualification remain pending.
[Valuation framework record 09](valuations-framework/finite-difference-rates.md#accepted-decision-for-checked-finite-differences-and-rate-risk).

The tenth shared valuation framework package was accepted on 2026-11-13.
Independent examples cover the quote, hazard, recovery and settlement bases.
Runtime, calibration-replay and host qualification remain pending.
[Valuation framework record 10](valuations-framework/credit-sensitivities.md#accepted-decision-for-checked-credit-sensitivities).

The eleventh shared valuation framework package was accepted on 2026-11-13.
Independent examples cover represented stencils, quote scales, FX direction,
sparse bucket reconciliation and distinct theta bases. Runtime, financial
accuracy and host qualification remain pending.
[Valuation framework record 11](valuations-framework/option-sensitivities.md#accepted-decision-for-checked-option-and-fx-sensitivities).

The twelfth shared valuation framework package was accepted on 2026-11-13.
Independent examples cover represented stencils, quote replay, relative spot
units and attribution. Runtime, calibration-replay, financial accuracy and
host qualification remain pending.
[Valuation framework record 12](valuations-framework/cross-factor-risk.md#accepted-decision-for-checked-cross-factor-risk).

The thirteenth shared valuation framework package was accepted on 2026-11-14.
Independent examples cover cash cuts, dated conditioning, carry, funding
and quote breakevens. Runtime, product and host qualification remain pending.
[Valuation framework record 13](valuations-framework/carry-theta-breakeven.md#accepted-decision-for-checked-horizon-theta-carry-and-breakeven).

The fourteenth shared valuation framework package was accepted on 2026-11-14.
Independent examples cover tail estimators, scenario FX, aggregation and
represented Taylor stencils. Runtime, replay, financial accuracy and host
qualification remain pending.
[Valuation framework record 14](valuations-framework/historical-risk.md#accepted-decision-for-checked-historical-scenario-risk).

The fifteenth shared valuation framework package was accepted on 2026-11-14.
Independent examples cover settlement, term projection, accrual, credit
probability and reconciliation. Runtime, product and host qualification
remain pending.
[Valuation framework record 15](valuations-framework/cashflow-export.md#accepted-decision-for-checked-cashflow-valuation-and-export).

The sixteenth shared valuation framework package was accepted on 2026-11-14.
Bounded checks cover embedded metadata, exact selection, dated applicability
and selected financial examples. Full row, runtime, replay and host
qualification remain pending.
[Valuation framework record 16](valuations-framework/convention-selection.md#accepted-decision-for-checked-convention-selection-and-dated-sources).

The seventeenth shared valuation framework package was accepted on 2026-11-14.
Bounded checks cover lag geometry, release eligibility, issuer references and
effective rates. Runtime, calibration, product and host qualification remain pending.
[Valuation framework record 17](valuations-framework/inflation-references.md#accepted-decision-for-checked-inflation-references-and-information-dates).

The eighteenth shared valuation framework package was accepted on 2026-11-14.
Bounded checks cover shock definitions, tenor allocation and netting identity.
Runtime, quote replay, margin reporting and host qualification remain pending.
[Valuation framework record 18](valuations-framework/margin-bridge.md#accepted-decision-for-checked-margin-repricing-and-identity).

The nineteenth shared valuation framework package was accepted on 2026-11-14.
Bounded checks cover leg dates and signs, issuer lockout, convertible triggers,
quanto orientation, credit scale, PIK allocation and estimator arithmetic.
Runtime, product, calibration and host qualification remain pending.
[Valuation framework record 19](valuations-framework/shared-contract-controls.md#accepted-decision-for-checked-shared-contract-and-stochastic-controls).

The twentieth shared valuation framework package was accepted on 2026-11-14,
completing adoption of the 20 detailed records. Isolated Rust compiler and
execution probes cover current derive behavior and proposed evaluation order.
Product, workspace and host qualification remain pending.
[Valuation framework record 20](valuations-framework/builder-validation.md#accepted-decision-for-checked-generated-financial-builders).

The first rates valuation package was accepted on 2026-11-14.
Bounded checks cover date resolution, positive simple growth, signed cash,
holder and trade cuts and quote inversion. Runtime, calibration and host
qualification remain pending.
[Rates valuation record 01](valuations-rates/01-deposits.md#accepted-decision-for-checked-deposit-cashflows-and-quote-inversion).

The second rates valuation package was accepted on 2026-11-14.
Bounded checks cover benchmark dates, fixing selection, upfront settlement
and direct risk. Runtime, calibration, reporting and host qualification remain pending.
[Rates valuation record 02](valuations-rates/02-fra.md#accepted-decision-for-checked-fra-fixing-settlement-and-risk).

The third rates valuation package was accepted on 2026-11-14.
Bounded checks cover term and overnight coupons, settlement cuts, par and
direct risk. Runtime, calibration, margin and host qualification remain pending.
[Rates valuation record 03](valuations-rates/03-interest-rate-swaps.md#accepted-decision-for-checked-interest-rate-swap-cashflows-and-risk).

The fourth rates valuation package was accepted on 2026-11-14.
Bounded checks cover leg discounting, contractual spread weights, positive
and zero spread coefficients, absolute and incremental quotes, and direct risk.
Runtime, calibration and host qualification remain pending.
[Rates valuation record 04](valuations-rates/04-basis-swaps.md#accepted-decision-for-checked-basis-swap-cashflows-and-par-spreads).

The fifth rates valuation package was accepted on 2026-11-14.
Bounded checks cover payoff and premium, quote families, Hull–White term
scaling and overnight covariance. Runtime, model, calibration and host
qualification remain pending.
[Rates valuation record 05](valuations-rates/05-cap-floor-pricing.md#accepted-decision-for-checked-cap-and-floor-optionlet-pricing).

The sixth rates valuation package was accepted on 2026-11-14.
Bounded arithmetic checks cover repeated cutoff quote derivatives,
discount risk, signed holder theta and gross premium inversion limits.
Runtime, model, calibration and host qualification remain pending.
[Rates valuation record 06](valuations-rates/06-cap-floor-risk.md#accepted-decision-for-checked-cap-and-floor-risk-and-implied-volatility).

The seventh rates valuation package was accepted on 2026-11-14.
Bounded checks cover forward cashflows, dated settlement annuity and cash,
quote Greeks, discount risk, clock conversion and inverse bounds.
Runtime, model, calibration and host qualification remain pending.
[Rates valuation record 07](valuations-rates/07-european-swaptions.md#accepted-decision-for-checked-european-swaption-pricing-and-risk).

The eighth rates valuation package was accepted on 2026-11-14.
Bounded checks cover exact coupon timing, tree actions and exercise mass,
joint discounting, a bond-option reference, held-out policy and raw risk.
Runtime, numerical, calibration and host qualification remain pending.
[Rates valuation record 08](valuations-rates/08-hull-white-swaptions.md#accepted-decision-for-checked-hull-white-swaption-exercise-and-risk).

The ninth rates valuation package was accepted on 2026-11-14.
Bounded checks cover support and retained loadings, terminal discounting,
a one-forward price reference, sparse exercise and paired risk arithmetic.
Runtime, numerical, calibration and host qualification remain pending.
[Rates valuation record 09](valuations-rates/09-lmm-bermudans.md#accepted-decision-for-checked-lmm-bermudan-pricing-and-risk).

The tenth rates valuation package was accepted on 2026-11-14.
Bounded checks cover weighted observation blocks, early term fixing,
Gaussian covariance, clearing variation and separate option risk axes.
Runtime, numerical, calibration and host qualification remain pending.
[Rates valuation record 10](valuations-rates/10-rate-futures.md#accepted-decision-for-checked-rate-futures-and-futures-option-risk).

The eleventh rates valuation package was accepted on 2026-11-14.
Bounded checks cover the reference forward, signed payment delay and
stubs, payment eligibility, local collar coherence and full mixed risk.
Runtime, model and host qualification remain pending.
[Rates valuation record 11](valuations-rates/11-cms-convexity.md#accepted-decision-for-checked-cms-convexity-pricing-and-risk).

The twelfth rates valuation package was accepted on 2026-11-14.
Bounded checks cover quote geometry, support sensitivity, payment parity
and weighted coupons. Runtime, model, market and host qualification remain pending.
[Rates valuation record 12](valuations-rates/12-cms-replication.md#accepted-decision-for-checked-cms-smile-replication-and-risk).

The thirteenth rates valuation package was accepted on 2026-11-14.
Bounded checks cover lag and index basis, leap stubs, periodic coupon cash,
par and CPI risk. Runtime, calibration and host qualification remain pending.
[Rates valuation record 13](valuations-rates/13-inflation-swaps.md#accepted-decision-for-checked-inflation-swap-cashflows-and-risk).

The fourteenth rates valuation package was accepted on 2026-11-14.
Independent arithmetic covers stub and known payoffs, publication expiry,
ratio parity, Normal vega and CPI growth risk. Runtime, market and host
qualification remain pending.
[Rates valuation record 14](valuations-rates/14-inflation-options.md#accepted-decision-for-checked-inflation-option-cashflows-expiry-and-risk).

The fifteenth rates valuation package was accepted on 2026-11-14.
Independent arithmetic covers overnight settlement and negative interest,
haircut coverage, and the funding and discount reprices. Runtime, market,
margin and host qualification remain pending.
[Rates valuation record 15](valuations-rates/15-repo.md#accepted-decision-for-checked-repo-cashflows-collateral-and-risk).

The sixteenth rates valuation package was accepted on 2026-11-14.
Independent arithmetic covers principal resets, dated FX conversion,
cashflow and covariance differences, spread and directed risk. Runtime,
market, calibration and host qualification remain pending.
[Rates valuation record 16](valuations-rates/16-cross-currency-swaps.md#accepted-decision-for-checked-cross-currency-swap-cashflows-fx-resets-and-risk).

The seventeenth rates valuation package was accepted on 2026-11-14.
Independent arithmetic covers known and reduced payoffs, the zero-strike
exchange law, a fixed-order integration discrepancy and independent
convergence. Runtime, market and host qualification remain pending.
[Rates valuation record 17](valuations-rates/17-cms-spread.md#accepted-decision-for-checked-cms-spread-cashflows-model-and-risk).

The first bond valuation package was accepted on 2026-11-14.
Independent examples cover delayed cash, PIK and principal phases,
and buyer and holder entitlement. Runtime, market and host
qualification remain pending.
[Bond valuation record 01](valuations-bonds/01-schedule-and-holder-cashflows.md#accepted-decision-for-checked-bond-schedules-and-holder-cash).

The second bond valuation package was accepted on 2026-11-14.
Independent examples cover settlement eligibility, detached and
intervening claims, signed accrued and quote carry, and yield and model
time. Runtime, market, model and host qualification remain pending.
[Bond valuation record 02](valuations-bonds/02-settlement-and-clocks.md#accepted-decision-for-checked-bond-settlement-quote-carry-and-clocks).

The third bond valuation package was accepted on 2026-11-14.
Independent examples cover quote marks, signed spread discounting,
fractional recovery, repayment lags and local hazard limits. Runtime,
market, model and host qualification remain pending.
[Bond valuation record 03](valuations-bonds/03-pv-models.md#accepted-decision-for-checked-bond-pv-selection-and-fractional-recovery-of-par).

The fourth bond valuation package was accepted on 2026-11-14.
Independent examples cover exact dated nodes, cash ordering, make-whole
accrued, call thresholds and nodewise OAS. Runtime, fit, convergence,
market and host qualification remain pending.
[Bond valuation record 04](valuations-bonds/04-rates-tree-exercise.md#accepted-decision-for-checked-bond-rates-trees-and-exercise).

The fifth bond valuation package was accepted on 2026-11-14.
Independent examples cover MOIC and XIRR thresholds, actual cash dates,
exercise-phase eligibility, payable accrued and optional par terms.
Runtime, contract, path replay and host qualification remain pending.
[Bond valuation record 05](valuations-bonds/05-return-floors.md#accepted-decision-for-checked-issue-basis-return-floors).

The sixth bond valuation package was accepted on 2026-11-14.
Independent examples cover normalized index gross, overnight blocks,
cash/PIK and entitlement, lagged repayment exposure and affine rollback.
Runtime, stochastic calibration, market, risk and host qualification remain pending.
[Bond valuation record 06](valuations-bonds/06-joint-coupon-replay.md#accepted-decision-for-checked-joint-bond-payoff-and-recovery-replay).

The seventh bond valuation package was accepted on 2026-11-14.
Independent examples cover decision-time continuation, conditioning
before a strike floor, independent-group counts and pricing uncertainty.
Runtime, economic fit, pricing, risk and host qualification remain pending.
[Bond valuation record 07](valuations-bonds/07-lsmc-policy.md#accepted-decision-for-checked-joint-bond-exercise-policy-and-make-whole).

The eighth bond valuation package was accepted on 2026-11-14.
Independent examples cover terminal and bridge default, recovery timing,
PIK phases, paired estimation, fractional loss tails and signed spreads.
Runtime, stochastic convergence, market and host qualification remain pending.
[Bond valuation record 08](valuations-bonds/08-merton-simulation.md#accepted-decision-for-checked-structural-bond-simulation-and-outputs).

The ninth bond valuation package was accepted on 2026-11-14.
Independent examples cover accrued and claim carry, consistent curve
basis, discontinuous trial prices, non-identification and independent
cash validation. Runtime, stochastic fit, market, risk and host
qualification remain pending.
[Bond valuation record 09](valuations-bonds/09-merton-calibration.md#accepted-decision-for-checked-cash-equivalent-structural-calibration).

The tenth bond valuation package was accepted on 2026-11-14.
Independent examples cover final-period formulas, detached and
co-dated cash, inverse uniqueness and the Japanese simple-yield clock.
Runtime, market-specific quote, risk and host qualification remain pending.
[Bond valuation record 10](valuations-bonds/10-quoted-yield.md#accepted-decision-for-checked-settlement-yield-and-price-inversion).

Bond valuation record 12 was accepted on 2026-11-14. Independent
examples cover the native discount clock, projected floating cash,
benchmark path consistency, buyer-claim carry and discontinuous OAS.
Runtime, market, workout, risk and host qualification remain pending.
[Bond valuation record 12](valuations-bonds/12-spread-quotes.md#accepted-decision-for-checked-bond-spreads-and-price-inversion).

Bond valuation record 13 was accepted on 2026-11-14. Independent
arithmetic covers par and proceeds cash, ex-coupon accrued and carry,
dated floating projection and singular inverses. Runtime, market,
workout and host qualification remain pending.
[Bond valuation record 13](valuations-bonds/13-asset-swaps.md#accepted-decision-for-checked-bond-asset-swaps-and-price-inversion).

Bond valuation record 14 was accepted on 2026-11-14. Exact arithmetic
and high-precision difference checks cover the quoted-yield factor laws,
with dedicated immediate-cash, detached-anchor and signed-strip examples.
Runtime, market, workout and host qualification remain pending.
[Bond valuation record 14](valuations-bonds/14-yield-risk.md#accepted-decision-for-checked-quoted-yield-bond-risk).

Bond valuation record 15 was accepted on 2026-11-14. Independent
arithmetic compares valuation and settlement anchors, parallel and
bucketed shocks, uneven stencils, exercise switches and paired groups.
Runtime, model, market and host qualification remain pending.
[Bond valuation record 15](valuations-bonds/15-curve-risk.md#accepted-decision-for-checked-bond-curve-and-effective-risk).

Bond valuation record 16 was accepted on 2026-11-14. Independent checks
cover discrete par-CDS replay, exact quote buckets, periodic forward Z,
signed continuous Z and the same-pair spread-duration normalization.
Market replay, model, runtime and host qualification remain pending.
[Bond valuation record 16](valuations-bonds/16-spread-credit-risk.md#accepted-decision-for-checked-bond-spread-and-credit-risk).

Bond valuation record 17 was accepted on 2026-11-14. Independent checks
cover recovery-mode sign reversal, a full one-sided move, repayment phase,
surviving claims, the holder quote bridge and zero-par contingent cash.
Legal-claim, market replay, model, runtime and host qualification remain pending.
[Bond valuation record 17](valuations-bonds/17-default-risk.md#accepted-decision-for-checked-bond-recovery-and-jump-to-default).

Bond valuation record 18 was accepted on 2026-11-14. Independent
checks cover interacting rights, full sigma moves and paired pricing
statistics in the elected currency units. Model, market, runtime
and host qualification remain pending.
[Bond valuation record 18](valuations-bonds/18-embedded-option-risk.md#accepted-decision-for-checked-bond-embedded-option-value-and-vega).

Bond valuation record 19 was accepted on 2026-11-14. Independent
checks cover the holder bridge, explicit funding, exact Simple and
Street closing prices, and repayment-date WAL with retired-par weights.
Product, market, runtime and host qualification remain pending.
[Bond valuation record 19](valuations-bonds/19-carry-and-life.md#accepted-decision-for-checked-bond-carry-and-repayment-life).

Bond valuation record 20 was accepted on 2026-11-14. Independent
checks cover signed returns, distinct minimizing exit paths, the
ACT/365F leap-year rate, failed XIRR admission and retained exit cash.
Historical coverage, contractual path, runtime and host qualification remain pending.
[Bond valuation record 20](valuations-bonds/20-lifetime-returns.md#accepted-decision-for-checked-bond-issue-to-exit-returns).

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
