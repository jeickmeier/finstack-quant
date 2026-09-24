# Changelog

## [Unreleased]

### Loans cleanup: one term-sheet vocabulary, one balance replay (2026-09-23)

#### Changed (breaking)

- **`TermLoanSpec` and its `TryFrom` are removed.** `TermLoan` is itself the
  serde-stable shape; build it with `TermLoan::builder()` (which validates) or
  deserialize it. The spec's `notional_limit: None` fallback to the DDTL
  commitment is gone: `notional_limit` is always required.
- **Term-loan DDTL step-downs use the shared `loan_terms::CommitmentStep`.**
  `CommitmentStepDown` is deleted; step JSON `{date, new_limit}` becomes
  `{date, amount, fee_bp}`. Term loans carry no reduction fee, so a non-zero
  `fee_bp` is rejected. `DdtlSpec::usage_fee_bp` / `commitment_fee_bp` are now
  `f64` basis points (finite, non-negative), matching the revolver.
- **The revolver's Monte Carlo model key is `monte_carlo_three_factor`**
  (`ModelKey::MonteCarloThreeFactor`), naming its utilization / short-rate /
  credit-spread simulation. `monte_carlo_gbm` no longer prices a revolving
  credit facility; it remains the GBM key for exotics.
- Removed dead items: `RevolvingCreditFees::flat_bp`,
  `revolving_credit::ZERO_TOLERANCE`.

### Component source registry 0.2.0 (2026-09-20)

- Prepares 150 registry items for financial forms, views and a composed pricing
  workbench using existing WASM contracts and unmodified shadcn controls. See the
  [UI changelog](finstack-quant-ui/CHANGELOG.md) and
  [upgrade instructions](docs-site/content/docs/registry/upgrading.mdx).
- No Rust, Python or WASM calculation/API additions are claimed. Typed cashflow
  rows, inferred units, full-state curve evaluation, plain volatility-surface
  off-grid evaluation, absent financial aggregates, unresolved detail contracts
  and quote-space calibration fits remain excluded. Scenario-price documentation
  drift remains upstream. The complete installation matrix and public CLI/MCP
  checks remain open; publication remains restricted to `master`.

### Requested metrics never go missing; windowed formulas work on money (2026-09-19)

#### Fixed

- **Windowed formula functions are usable on monetary nodes.** `lag`, `shift`,
  `diff`, `rolling_mean/sum/min/max/median/std`, `ewm_mean`, `ewm_std`,
  `annualize` and `fiscal_ytd` folded their trailing count, window, offset,
  smoothing-factor or month argument into the dimension check, so every one of
  them was rejected on a USD-denominated node with `Dimensional mismatch in
  lag: cannot combine scalar and USD`. A debt corkscrew written the natural way
  (`lag(closing_debt, 1) + drawdowns - repayments`) could not be expressed and
  had to be rebuilt with `cumsum`. Those functions now take their dimension
  from the series argument alone; an argument that is a count, window, offset,
  smoothing factor, digit count or month number never participates in dimension
  unification. `mean`, `sum`, `min`, `max`, `clamp` and `coalesce` are genuinely
  variadic over values and still reject a currency mismatch between them.

#### Changed (breaking)

- **A metric requested from `price_instrument` / `priceInstrument` /
  `Instrument::price_with_metrics` is now either returned or raises.**
  Previously a requested metric with no calculator for that instrument type was
  silently omitted from `measures`, so a caller could not tell "not supported"
  from "computed but missing". It now raises `MetricNotApplicable` (Python
  `ValueError`) naming both the metric and the instrument type.
  **16,509 of the 17,940 (instrument type, metric) pairs change from silence to
  an error**; 1,431 remain applicable. Callers passing a broad list across mixed
  instruments must narrow it.
- `MetricRegistry::compute` rejects unregistered and non-applicable requests
  before running any calculator, so the reported error no longer depends on
  dependency evaluation order.
- `MetricId::ThetaPeriodDays` is a registered metric that resolves on its own;
  previously it appeared only when `theta` was requested in the same call.
- **A portfolio metric list is a documented menu.** `value_portfolio` /
  `valuePortfolio` / `RequestedMetrics` take one list for a book of mixed
  instrument types and offer no way to say "`delta` for the options, `cs01` for
  the credit", so each position is now asked for exactly the entries its own
  instrument type has a calculator for. This restores mixed-book risk runs,
  which the strict single-instrument contract above would otherwise have made
  impossible for any list broader than the standard set. Narrowing covers
  structural inapplicability only: `strict_risk` still governs a metric an
  instrument type supports but fails to compute, and an unknown metric name
  still raises when the request is parsed.

#### Added

- `PositionValue.inapplicable_metrics` (Rust `Vec<MetricId>`, Python
  `list[str]`, serialized when non-empty) lists the requested metrics a
  position's instrument type has no calculator for, so the portfolio-level
  narrowing above is reported rather than silent. It is not a failure list:
  unlike `degraded_positions` and `PortfolioMetrics.skipped_metrics`, nothing in
  it could have been computed and was not, so a portfolio total is not
  understated by it.
- `MetricRegistry::applicable_subset(&[MetricId], InstrumentType)` narrows a
  cross-instrument superset to what one instrument type supports, so dropping
  the rest is a visible decision at the call site rather than a silent filter
  inside pricing. Used by composite legs, the standard option-Greek set, the
  portfolio evaluation menu and the attribution engine menu, each of which
  evaluates a fixed superset across heterogeneous instruments.
- `expected_loss` is a registered metric for `structured_credit` and `bond`,
  and `expected_shortfall` for `structured_credit`. These are published by the
  model pricers themselves — structured credit's deal Monte Carlo pass and the
  Merton-MC bond engine — rather than derived by a calculator, so the registry
  previously reported them as inapplicable to those instrument types even
  though every priced result carried them. Requesting one now returns the
  pricer's own value, unchanged and never recomputed; on structured credit
  `expected_shortfall` is the deal Monte Carlo tail loss rather than a
  historical-simulation estimate, matching what the result envelope already
  reported. Requesting either on a valuation whose model does not publish it
  (a discounted bond, say) is an error, as is requesting them on an instrument
  type that neither publishes nor calculates them.


### Structured credit: analyst remediation (2026-09-17 plan, Phases A–E)

#### Fixed

- Deal and cleanup calls realize the stub period's collateral flows; every
  "original balance" quantity (cumulative-loss triggers, cleanup-call factor,
  charge-off curves) uses `AssetPool.original_balance`; OC cures are sized for
  an interest-funded diversion (`max(0, D − N/r)`); `CoverageRules::clo_standard`
  carries performing collateral at par; Default01 / Prepayment01 / Recovery01 /
  Severity01 and DV01 reprice per tranche through `StructuredCreditTranche`;
  stochastic prices are per current tranche face (`TranchePricingResult.price_pct`).
- Card master trusts allocate collections on a fixed investor share
  (`CardPortfolioSpec.{seller_interest, fixed_allocation_pct}`); the CLO registry
  profile is the single source of CLO defaults (15/35 bp fees, 20% CPR, 2% CDR,
  60% recovery); the incentive fee shares principal proceeds above the hurdle;
  shifting interest keeps the equity residual out of pro-rata principal.
- Servicer advances are per loan: cures repay arrears (reimbursing advances
  first) and amortize the repaid principal, charge-offs write off pro rata and
  reimburse from their own proceeds; the coverage-test evaluation that gates
  reinvestment runs on the executor's cash and hedge-adjusted waterfall.
- `Tranche.frequency` sets the yield's compounding and its day count the time
  basis (a par 5% quarterly 30/360 note yields 5.00%); impaired notes price at
  zero with `TrancheValuation.ytm: None`; scenario cells quote the clean
  settlement price from one projection; the payment business-day default is
  documented as ModifiedFollowing.

#### Added

- Collateral terms: `PoolAsset.{origination_date, amortization_term_months,
  io_months, index_floor}` (per-asset seasoning on PSA/ABS/vector/SDA curves,
  level payment over the amortization term, IO windows, index floors),
  `BalloonSpec.{extension_prob, loss_prob, severity_pct, workout_months}` with
  workout claims, NPL timelines anchored on `acquisition_date`, DSCR on the live
  floating coupon and schedule.
- Deal mechanics: `LossRecognition::{AtDefault, AtLiquidation}`,
  `ShiftingInterestSpec::{mode, triggers}`, `WaterfallRules::{reserve, target_oc}`
  (`ReserveAccountSpec`, `ReserveTarget`, `TargetOcSpec`), `AfcSpec.carryover`,
  `CoverageTestSpec::{placement, divert_pct}` (`CoveragePlacement`),
  `Tranche.non_deferrable`, `AdvancingPolicy::PrincipalAndInterest.reimburse_from_collections`,
  `DealFees::{workout_fee_pct, liquidation_fee_pct}` with dynamic special
  servicing, `PrepaymentPenalty::{Lockout, StepDown, YieldMaintenance {
  discount_curve_id, floor_pct }}`, `CreditModelConfig.stochastic_recovery_spec`.
- Facilities: `EarlyAmortizationSpec.max_cumulative_loss`,
  `CallAssumption.after_early_amortization_months`, `StructuredCredit::{tranche_draws,
  tranche_readvance}`, `EligibilityRule::{max_days_past_due, exclude_non_performing}`
  on live collateral, `AssetBackedFacility::{fees, draw_schedule,
  readvance_to_borrowing_base}`, `FacilityProjection.draws`,
  `SimulationDiagnostics::{early_amortization_date, tranche_draws}`,
  `PeriodDiagnostics.servicer_advances_outstanding`.
- Analytics: `TrancheMetrics::{spread_convexity, dm_bp}` and effective
  `modified_duration` / `convexity` from ±1 bp re-projection;
  `calculate_tranche_spread_convexity`; `TranchePricingResult.paths_with_principal`
  (average life over paths that return principal).
- Python: typed `BalloonSpec`, `PrepaymentPenalty`, `SpecialServicingSpec`,
  `LiquidationSpec`, `BorrowingBaseRules`, `AdvanceRate`, `EligibilityRule`,
  `ConcentrationLimit`, `TermOutSpec`, `AmortizationEvent` (accepted wherever
  the dict form was), `TrancheCashflows.{total_deferred, total_writedown}`,
  `TrancheBuilder.non_deferrable`, `StructuredCreditBuilder.stochastic_recovery_spec`.

#### Removed

- `ReinvestmentCriteria.{maintain_credit_quality, maintain_wal}` (inert);
  `TranchePricingResult.spread` (never computed); `CoverageTestSpec.after_tranche`
  wire field (now `placement`); `BalloonSpec.default_prob` (now
  `extension_prob`); `EarlyAmortizationSpec.max_cumulative_loss_pct` (now the
  decimal `max_cumulative_loss`).

### Structured credit: collateral-vertical coverage (2026-09-15 plan, Phases A–F)

#### Added

- Coverage tests are waterfall positions (`PaymentType::CoverageTest` tiers
  carrying `CoverageTestSpec`s, default position after the tested class's own
  coupon, `after_tranche` to move it); only the interest still undistributed
  below the test is diverted, up to the binding cure.
- `LossAllocationPolicy` (`WriteDown` / `ParPreserving`) on the deal, with
  deal-type defaults; par-preserving notes realize shortfalls at legal final.
- Deal-level reinvestment: `ReinvestmentPeriod { amortizing_tranches,
  assumptions: ReinvestmentAssumptions }` books synthetic `REINVEST-{n}` rows
  (spread, price, maturity, coupon floor); notes are held flat unless listed.
- Waterfall funding sources (`WaterfallTier.funding`,
  `StructuredCredit.principal_covers_senior_interest`), hedge swaps as
  senior/junior fee recipients (`hedge_swaps: Vec<HedgeSwap>`), template
  fees with a subordinated management fee and an equity-IRR incentive fee
  (`TemplateFees`, `IncentiveFeeSpec`).
- `CoverageRules { rating_haircuts, defaulted_valuation, ccc_bucket,
  discount_obligation, borrowing_base }` for the OC numerator;
  `CoverageRules::clo_standard()`; `PoolAsset.market_price_pct`.
- Behavioral model library: `PrepaymentCurve::{Abs, Vector}`,
  `DefaultCurve::{Vector, CumulativeLoss, Timing}`,
  `RecoveryModelSpec.severity_vector`; roll-rate delinquency with servicer
  advancing and modifications (`DelinquencyModel`,
  `PoolAsset.delinquency_buckets`, `StepDownTrigger::MaxDelinquency`); card
  master trusts (`CardPortfolioSpec`, `EarlyAmortizationSpec.min_excess_spread_3m`,
  `abs_excess_spread`, `abs_payment_rate`, `abs_delinquency` metrics).
- Deal terms and analytics: `CallAssumption` (deal or tranche scope) with
  `liquidation_price_pct`, `TrancheMetrics.{wal_to_call, z_spread_to_call_bp,
  dm_to_call_bp}`; `SimulationDiagnostics.periods` (per-period pool,
  collections, accounts and coverage-test record); `calculate_equity_metrics`
  (`EquityMetrics`); CMBS terms (`BalloonSpec`, `PrepaymentPenalty`,
  `SpecialServicingSpec`, loan-level `noi` DSCR,
  `PaymentCalculation::PercentageOfSpecialServiced`).
- Attachment/detachment points are optional and derived from balances by
  payment priority (`Tranche::from_balance`, `TrancheStructure::from_balances`).
- `AssetBackedFacility` instrument (warehouse line: `BorrowingBaseRules`
  advance rates, eligibility and concentration limits, borrowing-base
  coverage test, unused fee, revolving period, early-amortization events,
  term-out) with `abf_*` metrics, JSON tag `asset_backed_facility`, typed
  Python (`AssetBackedFacility`, `FacilityProjection`) and WASM classes;
  `CoverageTestType::BorrowingBase` for any deal.
- NPL/RPL resolution timelines (`PoolAsset.liquidation: LiquidationSpec`).
- Python typed structured-credit surface: `PoolAsset` keyword constructor,
  `CallAssumption`, `CoverageRules`, `HedgeSwap`, `Waterfall`,
  `EquityMetrics`, `TrancheCashflows`, one builder setter per deal field,
  getters for every field, `SimulationDiagnostics.periods` /
  `to_dataframe` / `coverage_tests_dataframe`; typed curve-shape
  constructors on the cashflow specs.

#### Changed (BREAKING)

- Tranche prices and quotes are per CURRENT face (`price_pct`,
  `market_price_pct`, `quoted_clean_price`, `OasResult`, `ScenarioCell`).
- The two structured-credit regression goldens were re-blessed
  (`expected_loss` under par-preserving notes; hedge swap and `abs_speed`
  inputs replaced by their supported equivalents).
- Python `AssetPool.assets(value)` renamed to `with_assets`; the `assets`
  property now returns typed `PoolAsset` rows (`asset_records` keeps dicts).
- `AssetBackedFacility::example` is fallible; `InstrumentJson::AssetBackedFacility`
  is boxed.

#### Removed

- `Tranche.is_revolving`, `Tranche.can_reinvest`, `Tranche.target_balance`
  (Rust, Python, `.pyi`, JSON fixtures, WASM facade).
- `WaterfallTier.divertible`, `Waterfall.coverage_triggers`.
- `CoverageTestRules` (replaced by `CoverageRules`).
- `DealConfig`, `DealDates`, `CoverageTestConfig`, `DefaultAssumptions`.
- `Overrides.abs_speed` (use `PrepaymentModelSpec::abs`); registry
  `auto_abs` record, `default_auto_abs_speed`, `default_auto_ramp_months`,
  `seasonality`, `asset_type_defaults`, per-profile `assumptions`,
  `rmbs_standard_cpr`, `rmbs_standard_sda`, the seasonality constants.
- `hedge_npv`, `price_with_hedges`, `price_with_metrics_standalone`.
- `cleanup_call_premium`.
- `CardPortfolioSpec.purchase_rate` / `seller_interest_pct` were never added
  (the pool is the investor interest).

## [0.8.0] - 2026-09-06

### Changed (BREAKING) — post-RC tightening

- `AsianCall::new`, `AsianPut::new`, and both `with_history` constructors now
  return `Result`. A payoff requires at least one future or historical fixing;
  fully observed contracts may still have no future fixing steps.
- Portfolio margin-result JSON uses the margin crate's canonical
  `SimmSensitivitiesJson` tuple arrays for nested sensitivities. The duplicate
  portfolio-specific sensitivity object format was removed.
- FX barrier, digital, and touch options reject a monitoring start date after
  the valuation date.
- Black-Scholes and related model entry points reject non-positive spot or
  strike, negative volatility or expiry, and non-finite numerical inputs.
- `Rate` conversion, percentage construction, and rate negation return `Result`
  instead of panicking or silently wrapping invalid values.
- Callable bonds reject the `discounting` model. Use `tree` for rates-only
  optional pricing or `rates_credit` for joint rates-credit optional pricing.
- Lookback, goal-seek, covenant, carry-metric, and rolling-risk entry points
  fail closed on non-finite, out-of-range, or missing required inputs.

### Removed (BREAKING) — post-RC surface cuts

- `PvDiscountSource::Market`; callers pass an explicit discount source.
- Unused public CS01/hazard helpers and the unused hazard CS01 metric path.
- Compatibility and orphan surfaces including the option-market forwarding
  module, FX barrier payoff alias, empty ILB pricer module, reinvestment
  manager, RMBS WAL calculator, ASW configuration, basket-exposure metric,
  linear credit facade, inert ECL write-offs, formula-check alias,
  `PeriodDataFrame` export, `Bond::pricing_cashflows`, unread XVA
  config/netting wrappers, and speculative custom covenant evaluator hooks.
- Python duplicate instrument-validation helpers and unread sensitivity
  facades. Bindings take serde-owned wire shapes and typed error kinds.

### Added

- `CashFlowAccrual` records an optional `coupon_period` and
  `end_is_termination_date` so accrual day counts can honor the contractual
  coupon window and termination stub.
- `EmbeddedJsonRegistry` is public, with a document/registry split and an
  optional extension key.
- Act/Act ISMA day-count and inflation-index handling coverage.
- Documentation site under `docs-site/` with a notebook-backed curriculum.
- `finstack-quant-calibration` owns quote ingestion, market construction,
  calibration, and cached recalibration. Valuations keep instruments, pricing,
  and the recalibration port.

### Fixed

- Taylor attribution records cashflow collection failures and marks the result
  invalid instead of reporting successful theta with zero coupon income.
- Computed waterfall allocations, balances, and residual interest return
  errors when monetary conversion overflows instead of panicking.
- Taylor VaR borrows the supplied instruments directly, avoiding redundant
  instrument cloning and temporary reference collections.
- CMS option rho uses a 1 bp bump. Inflation `roll_forward` and key-rate
  bumps preserve interpolation style and extrapolation.
- Python stubs declare `PortfolioBuilder`, `PositionValue`, and
  `ReconciliationReport`, and nested credit types no longer shadow `pd` /
  `lgd` / `float` in annotations.

### Changed — model-engine consolidation (BREAKING)

- Product-independent mathematical and stochastic engines now have one owner:
  `finstack-quant-models`. Characteristic functions, DTSM, volatility
  evaluation and fitting, credit analytics, factor models and pure factor-risk
  kernels, liquidity, Hull-White equations, and structured-credit pool
  stochastic models moved out of `core`, `valuations`, `portfolio`, and the
  former standalone factor-model package.
- Core volatility surfaces and cubes are data artifacts. Model evaluation goes
  through `models::volatility::VolSource`; valuation-owned market resolution
  retains lookup and override precedence.
- Hull-White calibration remains in valuations but returns
  `models::rates::hull_white::HullWhiteParams`. Structured-credit deals,
  waterfalls, presets, and pricing remain in valuations while pool default,
  prepayment, and correlation engines live in `models::credit::pool`.
- Python model APIs now live under `finstack_quant.models`; WASM model APIs
  live under `models`. Rust and Python use snake case, while WASM retains the
  corresponding camel-case names.

### Changed — explicit recovery inputs (BREAKING)

- Calculations whose results depend on recovery now require a finite decimal
  recovery in `[0.0, 1.0]`; explicit zero remains valid. The implicit 40%
  recovery fallback and the global recovery assumption were removed.
- Because the project is pre-release, affected persisted contracts were fixed
  in place and remain version 1. This includes
  `finstack_quant.calibration/1`, `finstack_quant.credit_assumptions/1`, and
  their existing schema directories and registry keys.

### Removed — obsolete model paths (BREAKING)

- Removed the standalone `finstack-quant-factor-model` package and the old
  `finstack_quant.factor_model` host namespace.
- Removed model-bearing `core::credit`, `core::market_data::dtsm`, and
  `core::math::characteristic_function` paths.
- Removed portfolio-owned liquidity implementations, valuation-owned
  Hull-White kernels, valuation-owned structured-credit stochastic engines,
  and all related compatibility exports.

### Changed — reusable model ownership (BREAKING)

- Credit migration, PD calibration, scoring, LGD/EAD, rating-factor, recovery
  waterfall, liability-management, and assumptions-registry engines now live
  under `finstack_quant_models::credit` and `finstack_quant.models.credit`.
  `CreditRating` remains a neutral core type, but rating-factor lookup is now a
  models-owned function.
- WASM liability-management analytics moved from `core` to `models.credit`.
  The former Rust, Python, and WASM credit-model paths were removed rather than
  retained as compatibility exports.
- The credit-assumptions contract remains
  `finstack_quant.credit_assumptions/1`; its registry key is
  `models.credit_assumptions.v1`.

### Changed — credit derivative architecture (BREAKING)

- `CDSTranchePricer::with_params` now returns `Result` and rejects invalid
  copula, recovery, quadrature, bump, correlation, settlement, and convolution
  settings before constructing numerical caches. Direct tranche pricer methods
  validate instrument invariants before valuation.
- Credit-index market dependencies are distinct from direct hazard-curve
  dependencies. Portfolio factor-model orchestration resolves the aggregate
  index to its bound hazard curve when applying credit shocks.
- CDS-index bucketed CS01 and tranche Recovery01 now propagate failed
  par-spread recalibration instead of silently switching to a different risk
  definition or frozen-curve risk.
- CDS-option quadrature and synthetic-underlying modules are crate-private;
  callers use the validated instrument and metric surfaces.

### Changed — rates lifecycle, conventions, and settlement (BREAKING)

- Cash-settled swaptions now default to
  `CashSettlementMethod::CollateralizedCashPrice`; fixed and floating
  underlier legs carry independent day-count conventions, and the
  single-curve forward shortcut is restricted to economically equivalent,
  unseasoned swaps.
- Cap/floor schedules resolve currency-standard market calendars, `expiry()`
  reports the final contractual fixing date, and an optional dated premium
  is discounted as a holder outflow until settlement.
- FRA construction rejects zero accrual periods and fixed FRAs no longer
  require a forward curve. Basis swaps reject non-positive notionals.
- Remaining rates cashflows use start-of-day valuation semantics: events on
  or before `as_of` are settled. Single-curve OIS instruments no longer
  advertise redundant forward-curve dependencies.
- Rates documentation treats the ISDA 2021 Definitions as the current
  framework and labels ISDA 2006 conventions as legacy transaction terms.

### Changed — fixed-rate bond pricing and quote conventions (BREAKING)

- `Bond::fixed` now requires an explicit `StubKind`; Python and WASM expose
  the same stub choice. Existing callers must choose `None`, short/long front,
  or short/long back instead of inheriting a silent short-front assumption.
- Bond `Instrument::value` now remains an `as_of` NPV when a clean/dirty,
  yield, Z-spread, or OAS quote drives valuation. Clean, dirty, and accrued
  metrics are consistently settlement-anchored.
- Settlement is clamped to issue date and configured calendars fail closed.
- TreasuryActual long-first-coupon pricing follows 31 CFR Part 356,
  Appendix B, and its one-cashflow yield conversion now round-trips.

### Changed — scenario P&L and stress validation

- Equity and instrument price shocks reject percentages below −100%; an exact
  −100% wipeout remains valid, but scenarios can no longer create negative
  post-shock prices.
- Scenario P&L requires canonical PV pricing on both base and stressed legs
  instead of independently falling back to `Instrument::value`.
- FX scenarios conservatively reprice the full portfolio so triangulated
  native-PV dependencies cannot reuse stale base values.
- Portfolio scenario reports carry the caller's active `FinstackConfig`.
  `operations_applied` is documented as a low-level effect count, not an
  operation-coverage ratio.

## [0.7.0] - 2026-08-17

### Removed — legacy pathways, waves 1-6 (BREAKING)

Behavioral legacy: opt-in flags that restored pre-audit behavior, `Option`
fields whose `None` arm selected older semantics, and `serde(default)`s kept
only so pre-field payloads still parsed. The workspace has no `#[deprecated]`
attributes and no `serde(alias)`, so none of this was annotation-visible.

**Breaking (Rust)**

- Removed `finstack_quant_core::cashflow::NpvOptions` and `npv_with_options`.
  `npv` / `npv_with_ctx` always exclude flows dated on or before the valuation
  date. For an investment/project NPV containing the time-0 outlay, use
  `npv_amounts`, or value one day before the earliest flow.
- Removed `CDSTranchePricerConfig::validate_arbitrage_free` and
  `with_arbitrage_validation`. Base-correlation arbitrage beyond tolerance is
  now always an error; the silent zero-protection clamp is gone.
- Removed `CDSTranchePricerConfig::enforce_el_monotonicity` (its `false` branch
  was unreachable — no setter existed). EL/WD monotonicity is always enforced.
- Removed the unreachable `CreditPdError::ZeroAnnualDefaultRate` variant.
- Removed `PortfolioEclResult::from_results`; use `from_results_with_exposures`.
- Removed the dead best-effort wrapper `TrancheCoupon::current_rate_with_index`;
  use `try_current_rate_with_index`.
- Dropped unread parameters from public functions: `split_io_po`,
  `estimate_payment_window`, `allocate_pac_support`, `calculate_pay_up`,
  `project_floating_rate`, `try_rate_for_period`,
  `InterestRateFuture::calculate_convexity_adjusted_rate`.
- Removed `CreditAttributionInput::delta_spread` (write-only; the period
  decomposition already encodes it).
- `monte_carlo::rng::fbm::HybridFbm` → `WindowedConditionalFbm` (the old name
  disclaimed itself: it is not the Bennedsen-Lunde-Pakkanen scheme).
- Removed the `BarrierType` and `Position` re-export chains. Import
  `finstack_quant_core::types::BarrierType` and
  `finstack_quant_valuations::instruments::Position` directly.
- `TestContext::interest_claim_caps` is no longer `Option`; callers must supply
  the spec-derived claim map.
- Removed the one-variant `AltmanPdCalibration` enum. `altman_*_with_pd` take
  only the input struct.
- `arrow` no longer enables the `ipc` feature (no IPC code remains).

**Breaking (Rust, JSON) — waves 13-14: margin**

- A netting set with no `margin_spec` now records an `MO-16` degradation
  instead of silently reporting gross MTM as variation margin. Repo netting
  sets reach this path: they carry a `RepoMarginSpec` the aggregator does not
  consume, so their haircut terms are absent from VM. The number is unchanged;
  what changed is that it is no longer presented as a CSA-netted call amount.
- ISDA credit-qualifying bucket tables are **required** in the SIMM registry.
  The removed fallbacks fabricated them: broad weights via `unwrap_or(85.0)`,
  a flat `0.27` inter-bucket correlation across every sector pair, per-bucket
  concentration thresholds collapsed to the aggregate, and a `0.46`/`0.42`
  intra-bucket default. None correspond to any ISDA calibration.
- Removed `SimmVersion::V2_5` and its registry entry. It shipped **no** CQ
  tables, so it ran entirely on those fabricated constants. Versions are
  selectable only when their ISDA-published tables are present. `"v2_5"` no
  longer parses.

**Breaking (Rust, JSON, Python) — waves 15-16: explicit SIMM credit classification**

- CDS and CDS-index products using SIMM require
  `OtcMarginSpec.simm_credit_classification`. Qualifying exposures carry an
  explicit ISDA sector; non-qualifying is reserved for securitizations and
  designated CNQ exposures.
- Removed the scalar credit-qualifying map and approximation. The canonical
  `credit_qualifying_delta` shape is now `(sector, name, tenor, amount)` and
  always follows ISDA §3.B bucket aggregation.
- Replaced boolean `add_credit_delta` with explicit
  `add_credit_qualifying_delta` and `add_credit_non_qualifying_delta` APIs.

**Breaking (Rust, JSON, Python) — wave 12: the waterfall can now report insolvency**

- `WaterfallSpec.available_cash_node` is **required** and `impl Default for
  WaterfallSpec` is removed. With `None` the engine skipped its entire Step-5
  block: every scheduled fee, coupon and amortization was reported paid in full
  regardless of whether the model generated the cash — uses exceeded sources,
  cash was created from nothing, and no shortfall could ever be raised. The
  cash cap now always applies.
- The "every cash-consuming category must appear in `priority_of_payments`"
  rule is unconditional (it was gated on `available_cash_node` being set).
  Specs must list `Fees`, `Interest` and `Amortization`.
- Removed `CapitalStructureWarning::SweepExcessUnallocated`. It described only
  the no-equity-bucket case, which cannot arise once the cash cap is always on;
  sweep excess beyond debt capacity now falls to the equity residual.
- `default_priority_of_payments()` is public so hosts cannot drift from the
  canonical stack (fees, interest, amortization, sweep, equity).
- Python `WaterfallSpec(...)` requires `available_cash_node`.

**Breaking (Rust behavior) — wave 11: numeric defaults**

- **Bug fix**: `LatentFactorSpec::TwoFactor { correlation: 0.0 }` collapsed into
  the single-factor arm, which shares one factor — implied correlation **+1**,
  the opposite of the independence being requested. `factor_correlation` now
  returns `Some(rho)` for any two-factor spec and `None` only for
  `SingleFactor`; `MultiFactor` errors instead of silently sharing a factor.
- The callable-bond Hull-White tree no longer supplies an invented 100 bp
  short-rate vol when none is configured. It reads `model_config.hw1f_sigma`,
  then `market_quotes.implied_volatility`, and otherwise treats rates as
  deterministic (sigma = 0) rather than manufacturing a confident option value.
- CMS instruments (`CmsSwap`, `CmsOption`, `CmsSpreadOption`) derive unset leg
  conventions from the instrument's **currency** (EUR/GBP/JPY) instead of
  hard-coded USD constants. A EUR CMS previously took its calendar from the EUR
  convention but its frequencies and day counts from USD. USD is unchanged: its
  terminal constants encode the conventional fixed-vs-3M CMS underlying, which
  is a different swap from `IRSConvention::UsdSofr` and is a separate decision.
- `MetricId::FxDelta` and `MetricId::Fx01` were numerically identical with two
  implementations; both ids now share `GenericFx01Calculator` and the bespoke
  `FxDeltaCalculator` modules in `fx_swap` and `fx_spot` are deleted.

**Breaking (Rust behavior) — wave 10: silent degradation becomes an error**

- `BermudanSwaptionPricerConfig`, `CheyetteRoughConfig` and `LmmBermudanConfig`
  default `enforce_calibration` to **true**. Pricing off the generic starting
  parameters now errors; opting out is explicit.
- Hull-White swaption calibration **rejects** a malformed per-quote accrual
  schedule instead of silently substituting the synthetic constant-dt recipe,
  which calibrated to a different instrument than the caller described. The
  `schedule_fallback_count` / `schedule_fallback_quotes` report metadata is
  gone; `schedule_source` is now exactly what the caller supplied.
- `Swaption::validate` no longer has the undocumented "compatibility date"
  escape that allowed an expiry up to 5 business days **after** `swap_start`.
  Expiry must be on or before the swap start. This corrected one golden
  fixture whose expiry (2027-05-08, a Saturday) postdated its swap start:
  expiry moved to the swap start 2027-05-05 and its NPV re-pinned
  2,278,477.91 -> 2,259,795.96 (-0.82%, three fewer days of optionality).
- `CashFlowSchedule::to_period_dataframe` requires `meta.issue_date`. Inferring
  the funding anchor from the earliest flow silently anchored accrual to a
  coupon date rather than to issuance.
- `Covenant.label` is required and `Covenant::new` takes it. `None` fell back
  to the discriminant-only `covenant_id`, so two covenants of the same type
  collided in compliance reports and breach tracking. `Covenant::with_label`
  is removed.

**Breaking (Rust) — wave 9: `Instrument` trait**

- `Instrument::market_dependencies` is now a **required** method. Its old
  default returned an empty set, so emptiness could not be distinguished from
  "never declared" — the portfolio therefore treated every empty set as
  *unresolved* and repriced those positions for every factor. All 79 real
  instruments already declared their dependencies; only test mocks relied on
  the default. An empty set now means the instrument genuinely reads no market
  data, and such positions are repriced for no factor.
- `Instrument::base_value_raw_with_currency`'s default called both
  `base_value_raw` and `base_value`, pricing the instrument twice. It now
  prices once. Identical results for the 66 instruments that use the default;
  the 13 with a distinct high-precision raw kernel already override it.

**Breaking (JSON / serde)**

- `FxMatrixState.pinned_quotes` is required. A snapshot omitting it previously
  restored a matrix with no pinned fixings, silently re-deriving those dates
  from the provider.
- `SimmSensitivitiesWire.credit_qualifying_delta` requires a sector on every
  entry; the former scalar and parallel `*_bucketed` fields are removed.
- `IssuerBetaRow.level_fit_quality` is required; `FactorVolModel` now denies
  unknown fields.
- `LoadPhase::Migrate` removed (never constructed; no migration code existed).
- `NumericMode::Decimal` removed — never emitted. The enum now has one variant.
- `XccyConventions.notional_exchange` is required in the conventions registry.
- `core::wire::non_finite_f64` rejects JSON `null` instead of decoding it as
  `NaN`. Note the round trip this closes: a JS `Infinity` written by
  `restore_non_finite_ratios` stringifies to `null`, which previously decoded
  back as `NaN` — silent corruption. It now fails loudly.

**Breaking (Rust, JSON) — waves 7-8**

- Removed `DiscountedCashFlow.discount_curve_id` and `RealEstateAsset` /
  `LeveredRealEstateEquity.discount_curve_id`. These instruments discount at
  their own WACC / cap rate; the field only forced callers to load a curve no
  computation read. They now declare no market dependency
  (`no_market_dependencies = true` in the coverage manifest). Because these
  types deny unknown fields, stored payloads carrying it must drop it.
- Removed `PoolStats.weighted_avg_life` — it was assigned
  `weighted_avg_maturity` verbatim. Use
  `AssetPool::weighted_avg_life_from_cashflows` for a real WAL.
- `AssetPool.cumulative_scheduled_amortization` is now a required `Money`.
  `None` was treated as zero, understating the original-balance denominator
  and overstating `current_loss_percentage`.
- `RangeAccrual.accrual_start_date` is required. The `None` arm inferred the
  start by extrapolating one observation interval backwards.
- Removed `FloatingLegCompounding::CompoundedInArrears.observation_shift`, a
  duplicate of the `CompoundedWithObservationShift` variant whose combination
  with `lookback_days` had to be rejected on every pricing path. Use
  `CompoundedWithObservationShift { shift_days }`. The mirrored
  `RateCalibrationOisCompounding` field goes with it.
- `monte_carlo::registry::PythonBindingDefaults` → `ConvenienceDefaults` (and
  `Python{Engine,Pricer,Lsmc,Greek}Defaults` → `Convenience*Defaults`). The
  `python_bindings` key in `data/defaults/pricer_defaults.v1.json` is now
  `convenience`; the struct feeds Rust convenience pricers too, not just
  Python. Unknown keys are denied, so existing override docs must be renamed.

**Breaking (Python)**

- Removed the pre-rename aliases `FinstackValuationError`, `FinstackFxError`,
  `FinstackOptimizationError`. Use `ValuationError`, `FxError`,
  `OptimizationError`. (`FinstackError`, the base class, is unchanged.)
- `scoring.AltmanPdCalibration` removed; the `pd_calibration` argument on
  `altman_z_score` / `altman_z_prime` / `altman_z_double_prime` is now
  `with_implied_pd: bool = False`.
- `NumericMode.decimal()` removed.

**Breaking (WASM)**

- `Portfolio.validateMaterializationJson` → `Portfolio.validateMaterialization`.
  It returns a typed report object, so the `Json` wire suffix was wrong; the
  Python and WASM names now match and the rename-map entry is gone.

### Changed — result-return standardization (BREAKING)

Public APIs now hand results back the same way in Rust, Python, and WASM. The
full contract is recorded in `.claude/skills/finstack-consistency-reviewer/conventions.md`.
`to_json` / `from_json` still work everywhere — every converted entry point's
previous string output is available by calling `.to_json()` on the result.

**Rust**

- `finstack_quant_core::config::ResultsMeta` gained `parallel: bool`. It is
  omitted from JSON when false, so existing payloads and golden files are
  byte-identical for serial runs.
- `finstack_quant_statements::evaluator::ResultsMeta` → `EvalStats` (execution
  statistics, distinct from the workspace audit stamp). Serde field names are
  unchanged; the JSON Schema definition is renamed `StatementResultsMeta` →
  `StatementEvalStats`.
- `scenarios`: `ApplicationReport.rounding_context: Option<String>` →
  `meta: Option<ResultsMeta>`. `ApplicationEnvelope.market_json: String` /
  `model_json: Option<String>` → `market` / `model` as nested JSON objects.
- Typed twins are now public where only a JSON string was reachable:
  `attribute_return_contribution(&spec)` and `allocate_weights(&spec)` return
  typed results; the string forms are `*_json`.
- `margin`: three scalar/`_result` API pairs collapsed — `calculate_for_notional`,
  `calculate_netting_set_with_ngr`, and `calculate_for_collateral` now return
  `ImResult` (the scalar is the `amount` field).
- `XvaResult`, `EadResult`, `FrtbSbaResult`, `CovenantReport`, and the ECL
  results now stamp `meta: ResultsMeta`.
- Duplicate type names resolved: `CalibrationResult` → `TreeCalibrationResult`
  (short-rate tree), `ValidationReport` → `CalibrationValidationReport`
  (calibration validator), `WaterfallPeriodResult` → `CmoWaterfallPeriodResult`.
- `portfolio::performance`: `twrr_modified_dietz` and `twrr_linked` return
  `Result<_>` instead of `Option<_>`, so invalid inputs report why.

**Python**

- `attribute_pnl` returns `PnlAttribution` (its docstring already claimed this).
- `attribute_return_contribution` returns a new typed `ReturnContributionResult`
  with `to_dataframe()` and `to_series()`.
- `scenarios.apply_scenario` / `apply_scenario_to_market` return a typed
  `ApplicationResult` (`.market`, `.model`, `.report`) instead of a dict of JSON
  strings. `ApplicationReport` is a new typed class.
- `StatementResult.to_pandas_long` / `to_pandas_wide` → `to_dataframe(orient=...)`.
- `from_json` is `@staticmethod` everywhere (49 `@classmethod` conversions).
- `to_json()` emits compact JSON everywhere. Schema-document emitters
  (`*_schema`, `schema.index`) stay pretty-printed by design.
- The analytics domain and the Monte Carlo estimates gained `to_json` /
  `from_json` / `__reduce__`; they previously had no serialization at all.

**WASM**

- All 19 raw `serde_wasm_bindgen::to_value` call sites now route through
  `crate::utils::to_js_value`. Rust maps previously arrived as ES `Map`s in
  those returns, which `JSON.stringify` silently drops; they are now plain
  objects, matching `index.d.ts` and the Python dict shapes. `mise run
  wasm-lint` now fails if the raw serializer reappears.
- 47 exports converted from JSON strings to structured objects: the four
  `priceInstrument*` entry points and `calibrate`; 31 portfolio exports
  (valuation, aggregation, attribution, optimization, risk decomposition,
  liquidity); `covenants.evaluateEngine`; `statements.evaluateModel`,
  `evaluateModelWithMarket`, `runMonteCarlo`; six statements-analytics
  analyses; and `scenarios.computeHorizonReturn`.
- Wire surfaces that keep returning strings gained honest names: the seven
  covenant functions, five statements validators, two margin CSA specs, and
  `parsePortfolioSpec`/`buildPortfolioFromSpec` are now `*Json`.
- Prose-returning exports are now `*Text`: `parseFormulaText`,
  `plSummaryReportText`, `creditAssessmentReportText`. They previously shared
  the `Result<String, JsValue>` signature with ~130 real JSON exports, so
  callers had no way to tell prose from a parseable document.
- Numeric vectors cross as `Float64Array` instead of boxed-`Number` arrays
  (`correlationBounds`, `jointProbabilities`, `nearestCorrelation`,
  `generateSmile`, the two coupon profiles, `expiries`, `pillarVols`). Three
  of these were already *declared* `Float64Array` in `index.d.ts` and had been
  lying about their runtime type.
- `Portfolio.toSpecJson` → `toJson`; `margin.calculateVm` takes an ISO-8601
  date string instead of three integers; the comps functions and
  `portfolioResultGetMetric` return `undefined` rather than `null` for absent
  values.
- The JS facade is a pure namespace re-export again — `exports/valuations.js`
  no longer `JSON.parse`s `calibrate` while leaving its sibling `dryRun` alone.

### Known gap

Some entry points still return bare JSON strings under unsuffixed names in
*both* Python and WASM (parts of statements-analytics, factor-model, and the
scenario spec builders). They are consistent with each other but not with the
contract; converting one side alone would create the cross-language divergence
this work removes, so they are listed as paired follow-ups in
`.claude/skills/finstack-consistency-reviewer/conventions.md`.

### Added

- **Python:** ~90 `to_dataframe` / `to_*_dataframe` exports across portfolio,
  statements, statements-analytics, margin, scenarios, analytics, core.credit,
  core.market_data, monte_carlo, factor-model and valuations.correlation. Every
  frame documents its columns; `Money` becomes a float column plus one
  `currency` column, never a nested cell. 52 result types also gained
  `_repr_html_`, so they render as tables in Jupyter.
- **Python:** pickle support on every wrapper that round-trips through JSON, so
  results survive `multiprocessing` / `joblib` / `dask`. `copy.deepcopy` works
  through the same path. `PortfolioOptimizationResult` is the one exception: its
  Rust type has no `Deserialize`.
- **Python:** `finstack_quant.<domain>.schema` for `valuations`, `statements`,
  `factor_model` and `cashflows`, exposing the JSON Schemas compiled into the
  extension, so they can never drift from the installed wheel.
- **Python:** `finstack_quant.__version__`, sourced from the crate version.
- **Python:** `FinstackError` (`finstack_quant.core`) as the common base for the
  library's named exceptions. It derives from `ValueError`, so every existing
  `except ValueError` is unaffected. `CalibrationEnvelopeError` stays outside the
  tree because it is a `RuntimeError` and PyO3 cannot express two bases.
- **Rust, JSON:** clean-price CDS-option strikes (`CDSOptionStrike::CleanPricePct`),
  the CDX HY market convention, alongside the existing forward-spread strikes.
  Price strikes carry `strike_index_factor` (the strike's original index factor
  `f0`) and are validated as index-only, no-knockout, with an explicit positive
  coupon, factors in `(0, 1]`, `f <= f0`, and realized loss bounded by the removed
  original notional. Delta and gamma branch by strike kind: a clean price is not a
  valid argument to the Black `d₁`, so price strikes use a curve-reprice hedge
  ratio (option CS01 over underlying spread DV01 under a symmetric ±1 bp par-quote
  bump with hazard rebootstrap, sticky native strike, sticky surface volatility),
  and gamma is the change in that delta across the ±5 bp screen bump.
- **Rust:** a two-factor rates-credit lattice (`models::trees::two_factor_rates_credit`)
  for callable credit-risky bonds and term loans, with public `ModelConfig` inputs
  `hazard_volatility`, `hazard_mean_reversion` and `rate_credit_correlation`. One
  `resolve_rates_credit_config()` owns the public-config to lattice-input mapping
  for both engines. Correlation feasibility is proved against the per-node Fréchet
  bounds at calibration time rather than clamped during pricing, and
  `HazardFloorSaturation` reports how much state-price mass sat on the zero-hazard
  floor. Mean-reversion speeds above `KAPPA_MAX` are rejected — use `HullWhiteTree`.
- **Rust:** future floating-coupon resets valued at lattice nodes (`NodeCoupon`).
  The deterministic projection stays booked as before; the lattice adds only the
  node-dependent increment, which vanishes identically as `rate_vol → 0`, so an
  option-free floater's PV is invariant to rate volatility. Caps and floors on a
  floating leg are therefore priced as the caplet/floorlet strip they are. Call
  dates strictly inside a future floating period, and future floating PIK, are
  rejected rather than mispriced.
- **Rust:** `models::credit::market_anchored`, the shared fractional-to-absolute
  credit-volatility mapping (credit triangle `s = (1 − R)·λ`), used by both the
  callable lattice and the revolving-credit CIR path so they cannot drift apart.
  `CreditVolatilityConversion` carries every input and output together, so a 35%
  CDS-option quote cannot reach an additive hazard lattice without the 1.05%
  absolute figure sitting beside it. This is an explicit first-order local
  mapping, not a calibration.
- **Rust:** `market::credit_option_vol`, which resolves a CDX/iTraxx option-vol
  surface point into that additive hazard volatility. The surface is queried
  strictly at the strike's **native displayed** coordinate — a decimal spread for
  CDX IG and iTraxx, a clean price in percentage points (`107.0`, never `1.07`
  and never a spread equivalent) for CDX HY — and the index-derived *fractional*
  vol is anchored on the *target* curve's own reference hazard, so a single-name
  bond is never anchored to the index's hazard level. Selectors are `Strike`,
  `Moneyness` (against the native ATM-forward coordinate) and `Delta`.

### Changed

- **Breaking (Rust, JSON, Python, WASM):** the CDS-option strike is a typed enum
  instead of a bare decimal. `CDSOption.strike` and `CDSOptionParams.strike` are
  now `CDSOptionStrike`, whose canonical JSON is externally tagged, and **the old
  scalar wire shape is rejected with no compatibility fallback**:

  ```json
  { "strike": "0.0325" }                      // before — now rejected
  { "strike": { "spread": "0.0325" } }        // after, forward-spread strike
  { "strike": { "clean_price_pct": "107.0" } } // after, CDX HY clean-price strike
  ```

  Persisted `CDSOption` payloads must be migrated; Python and WASM reach CDS
  options through this JSON, so they are affected identically. `Spread` stays a
  decimal annual rate (`0.0325` = 325 bp) and `CleanPricePct` is quoted in
  percentage-price points (`107.0` = fraction `1.07`) — use
  `clean_price_fraction()` rather than re-dividing by 100. Spread strikes reject
  `strike_index_factor` as inert. `effective_underlying_cds_coupon` is now
  fallible, since a clean-price strike can never serve as the running coupon, and
  `settlement` is explicit on `CDSOptionParams` (default `Cash`).
- **Breaking (Rust behavior):** the callable rates-credit path reads only
  `hw1f_sigma` / `hw1f_mean_reversion` and **rejects** the legacy
  `implied_volatility` / `mean_reversion` channel when its canonical counterpart
  is absent, so a configuration cannot silently flip pricing regime; `hw1f_*` wins
  when both are set. Hazard inputs supplied without a `credit_curve_id` are
  rejected rather than ignored.
- **Breaking (Rust behavior, PV-affecting):** `RatesCreditConfig::default()` is
  now deterministic in both factors. The callable-bond path had been inheriting an
  undeclared `hazard_vol = 0.20` — worth roughly 33% of PV on the reference
  fixture — from `..Default::default()` construction. Callers that want a
  stochastic factor must now declare it explicitly, so previously-priced callable
  credit-risky bonds will change value.
- **Breaking (Rust behavior):** CDS-option volatility resolution is strict. An
  instrument-level implied-vol override wins; otherwise the surface is looked up
  at the native strike coordinate with a required `VolSurfaceAxis::Strike` and
  `VolQuoteType::BlackLognormal`, and out-of-grid coordinates error instead of
  clamping to the nearest edge. Delta and gamma screen metrics share this
  resolver. Valuing a physically-settled option at or after expiry now fails
  explicitly; the exercise/delivery lifecycle is not modelled.
- **Fixed (Rust):** bond rate vega bumped `market_quotes.implied_volatility`,
  which the rates-credit path no longer reads, producing a silently **zero** vega
  on every credit-risky callable (or an error when `hw1f_sigma` was unset). Vega
  now bumps whichever channel the instrument's own routing consumes.
- **Breaking (Python):** 40 `Performance` metrics return a `pandas.Series`
  indexed by ticker (with `.name` set to the metric) instead of `list[float]`;
  `skew_kurt` and `value_at_risk_and_es` return a tuple of two Series. Positional
  access must become `.iloc[i]` — `perf.sharpe()[0]` warns on pandas 2 and raises
  on pandas 3. `perf.sharpe()["FUND"]` is the preferred form, and
  `pd.concat([...], axis=1)` now yields correctly named columns.
- **Breaking (Python):** the pricers return typed results instead of JSON
  strings — `price_instrument` / `price_instrument_with_metrics` →
  `ValuationResult`, and `structured_credit_tranche_oas` / `_metrics` /
  `_scenario_table` → `OasResult` / `TrancheMetrics` / `ScenarioTable`. Replace
  `ValuationResult.from_json(price_instrument(...))` with the call itself, and
  `json.loads(result)` with `result.to_json()` where the wire payload is still
  wanted. `instrument_cashflows_json` is unchanged and still returns `str`.
- **Breaking (Python):** `ModelBuilder` / `MixedNodeBuilder` configuration
  methods return the builder instead of `None`, so calls chain. Statement-per-line
  code is unaffected (the object is mutated in place and the returned value *is*
  the same builder); the only visible change is that a notebook cell ending on a
  setter now echoes a builder repr. `build()` and `mixed()` remain terminal.
- **Python:** every date-valued `as_of` parameter accepts a `datetime.date` /
  `pandas.Timestamp` as well as an ISO string — the pricers, the scenario entry
  points, the `structured_credit_tranche_*` family, the portfolio sensitivity,
  stress, what-if and aggregation functions, `accrued_interest_json`,
  `evaluate_engine`, `decompose_levels` and `run_corporate_analysis`. The two
  functions that take a fiscal *period* rather than a date
  (`credit_assessment`, `credit_assessment_report`, e.g. `"2025Q4"`) still take
  a string, since a calendar date has no meaning there. A malformed date string
  is now rejected by the shared extractor, so the `ValueError` names the
  offending value and the reason and reads the same at every entry point.
- **Python:** zero-row result frames now carry their real dtypes instead of
  `object`. Concatenating an empty frame with a populated one previously
  downgraded every column, so a numeric column silently stopped being numeric
  and `groupby().sum()`, arithmetic and `to_parquet` broke — on the common path
  of iterating a book where some entries produce no rows.
- **Python:** `PnlAttribution.to_dataframe()` now raises `ValueError` when a
  factor's currency differs from `total_pnl`'s. The single-row frame carries one
  `currency` label for every factor, so a mixed-currency attribution would have
  made `df[factors].sum(axis=1)` add unlike units. Use `to_long_dataframe()`,
  which carries currency per row, for genuinely mixed input.

- Canonicalized editor and agent rules under `.agents/rules`. Cursor and
  Claude now resolve the same reviewed rule tree through checkout-relative
  `.cursor/rules` and `.claude/rules` symlinks.
- **Breaking (Rust, Python, JSON):** Removed `VolSurfaceKind` and the redundant
  `surface_kind` field from direct and hierarchy volatility-surface shocks;
  `vol_surface_id` now fully identifies the target surface.
- **Breaking (Rust, Python, JSON):** Removed the unused
  `OperationSpec::BaseCorrBucketPts.maturities` field and Python argument;
  base-correlation shocks target the supported detachment dimension only.
- **Breaking (Rust, JSON):** Removed the unused
  `IndexUnderlyingParams.convexity_id` field and its `with_convexity` builder;
  fixed-income index TRS pricing never consumed the identifier.
- **Breaking (Rust, JSON, WASM):** Removed the unwired
  `RateQuote::Futures.vol_surface_id` field and its persisted
  `RateCalibrationQuote` copy. Futures calibration continues to use the
  explicit `convexity_adjustment` value.
- **Breaking (Rust, Python):** Removed statement formula alias and fuzzy-name
  rewriting, including `ModelBuilder::with_name_normalization`. Formulas and
  `where` clauses now require exact node IDs; unknown IDs retain the dependency
  graph's nearest-name diagnostics.
- **Breaking (Python):** Removed the 13-argument `SaCcrTrade` constructor,
  which inferred supervisory delta and option classification from direction.
  Construct trades with `SaCcrTrade.from_json` and the complete canonical
  schema; deserialization now immediately applies the Rust SA-CCR regulatory
  validator.
- **Breaking (Rust, ECL policy JSON):** Simplified ECL staging and
  schedule-based calculation now use canonical `EclStageRequest` and
  `EclRequest` surfaces. The duplicate binding-default policy block, its seven
  Rust getters, and `compute_ecl_weighted_from_schedules` were removed; Python
  retains its established signatures and error types while delegating all
  policy, exposure, scenario, and configuration construction to Rust.
- **Breaking (Rust, JSON, Python):** Removed the unused
  `ScenarioDefinition.model_id` field and Python `ScenarioSet.model_ids`
  constructor input. Scenario-set JSON containing `model_id` is now rejected
  by the existing unknown-field validation.
- **Breaking (Rust, Python):** Corporate analysis now stores direct
  `CreditContextMetrics` per instrument instead of the speculative
  `CreditInstrumentAnalysis` wrapper. Non-positive DCF enterprise-value
  suppression is reported once at the top level as
  `ev_suppressed_non_positive`.
- **Breaking (Rust):** Removed the unused
  `CreditScoringError::OutOfRange` variant. Credit scoring input failures
  continue to use `NonFiniteInput` and `InvalidBinaryIndicator`.
- **Breaking (Rust):** Removed the unused `MigrationError::NotSquare` variant.
  Credit migration matrix shape failures continue to use `DimensionMismatch`.
- **Breaking (Rust):** Removed the unused
  `InputError::JointCalendarNonConvergent` variant. Active joint-calendar safety
  failures continue to use `JointCalendarIterationLimitExceeded`.
- **Breaking (Rust):** `ThresholdSchedule::new` now returns `Result` and is the
  sole threshold-schedule constructor; `try_new` was removed. Deserialization
  routes through the same finite-value and unique-date validation.
- **Breaking (Python):** Removed the module-level `price_european_call` and
  `price_european_put` functions. Use `EuropeanPricer.price_call` /
  `price_put` or `McEngine.price_european_call` / `price_european_put`.
  WASM retains `priceEuropeanCall` and `priceEuropeanPut` as
  function-oriented browser entry points.
- **Breaking (Rust, Python, WASM):** Kyle calibration now requires an explicit
  `reference_price` and returns price-space lambda. The working `*_with_mid`
  implementations now own the canonical `KyleLambdaModel::lambda_from_series`
  and `from_amihud` names; the legacy fail-closed signatures were deleted.
- **Breaking (Rust, Python, WASM):** `almgren_chriss_uniform_impact` now returns the canonical
  `ImpactEstimate`; the duplicate `AlmgrenChrissImpactView` was removed.
  Python and WASM now return the same five canonical fields, including
  `total_cost`, `cost_bp`, and `execution_risk`.
- **Breaking (Rust):** Removed the no-op `JumpEuler::with_max_jumps`
  constructor. Use `JumpEuler::new`; the aggregate jump sampler remains
  uncapped.
- **Breaking (Rust behavior):** Statement formula checks now use the canonical
  statements DSL evaluator, including time-series functions and `cs.*`
  references. `CheckSuiteSpec::resolve()` materializes formula checks directly;
  the duplicate analytics resolver was removed, and missing references or
  evaluation failures are returned instead of being silently skipped.
- Comparable-company flat field construction, named-field access, and metric
  selector parsing now share one canonical Rust implementation across scoring,
  Python, and WASM; accepted fields and scoring behavior are unchanged.
- **Breaking (Rust, Python, WASM):** Statement-model Monte Carlo now belongs
  exclusively to `finstack-quant-statements`. Import `MonteCarloConfig`,
  `MonteCarloResults`, and `run_monte_carlo` from `finstack_quant.statements`,
  or call `statements.runMonteCarlo` in WASM; the duplicate
  `statements_analytics` facade was removed without changing JSON payloads.
- **Breaking (Python):** `reporting.attribution_tearsheet` is now
  presentation-only and accepts only a precomputed `PnlAttribution` or its
  canonical JSON/dict payload. Inline instrument/market attribution parameters
  were removed; compute attribution through `finstack_quant.attribution` before
  rendering.
- **Breaking (Python):** `reporting.instrument_tearsheet` is now presentation-only
  and requires a precomputed `ValuationResult`; its `market`, `as_of`, `model`,
  and `market_price` parameters were removed. The presentation-owned
  `recommended_metrics` helper was also removed, so callers select metrics in
  the valuations API before rendering.
- **Breaking (Rust/Python):** Removed the unused SA-CCR engine
  `reporting_currency` field, builder option, and Python constructor argument.
  SA-CCR monetary inputs must already use one consistent currency; the engine
  does not perform currency conversion.
- **Breaking (Rust):** Removed the unused FRTB SBA engine
  `reporting_currency` field and builder option. Currency remains explicit on
  `FrtbSensitivities`, where it participates in the regulatory input contract.
- **Breaking (Rust):** Removed the speculative FRTB parameter-bundle and
  revision APIs, including `FrtbParams`, `FrtbRevision`, the JSON-overlay
  registry, and the corresponding `FrtbSbaEngine` builder and accessors. FRTB
  SBA continues to use the fixed BCBS d457 constants under
  `regulatory::frtb::params`.
- **Breaking (Rust):** Removed the duplicate scenario tenor and period parsing
  helpers, including their context wrappers. Use
  `finstack_quant_core::dates::Tenor` directly for parsing, simple year/day
  approximations, and calendar-aware year fractions.
- **Breaking (Rust):** `InterpolationResult` and
  `calculate_interpolation_weights` are now crate-private scenario adapter
  details and no longer part of the public or serialized API.
- **Breaking (Rust):** `TemplateRegistry` now has one validated registration
  path. Use the fallible `TemplateRegistry::with_embedded_builtins`,
  `register_json_template_str`, or `load_json_dir`; the builder-factory
  `register` / `register_with_components` methods and panicking `Default`
  implementation were removed.
- **Breaking (Rust):** Simplified the valuations surface. Use
  `schema::instrument_schema("bond")`, `TreePricer::calculate_oas`, and the
  free `solve_ytm` function in place of the removed bond wrappers and YTM
  solver objects. Tranche loss methods now use their stored balance, the
  valuations prelude no longer re-exports the core prelude, unused constants
  were removed, and rate-exotic Monte Carlo settings must be constructed as
  typed `RateExoticMcConfig` values.
- **Breaking (Rust):** Removed low-value public aliases and helpers from the
  foundational crates: use `analytics::regression::constrained_least_squares`
  and the correlation crate-root exports; construct `DatedSeries` with its
  public fields or `Default`; evaluate covenant schedules through covenant
  APIs; and import core `Error` / `Result` directly. The unused credit-calibrator
  `config` and `diagnostics` accessors were also removed.
- **Breaking (Rust, JSON, Python, WASM):** Removed the duplicate feature-operation
  names `clip_by_quantile` / `ClipByQuantile` and
  `dollar_neutral_weights` / `DollarNeutralWeights`. Use `winsorize` /
  `CrossSectionalOp::Winsorize` and `long_short_weights` /
  `CrossSectionalOp::LongShortWeights` in typed Rust calls, JSON panel specs,
  and Python or WASM string-dispatched calls.
- **Breaking (Rust):** Removed the public `CagrBasis` and
  `AnnualizationConvention` configuration types. `Performance::cagr()` keeps
  its existing date-based Act/365.25 behavior.
- **Breaking (Rust):** Factor-model configuration, covariance, envelope,
  primitive, and sensitivity types are now exposed only through their existing
  crate-root paths. The `matching`, `credit`, and `schema` modules remain public.
- **Breaking (Rust):** Removed the unused `FactorModelError` hierarchy. Factor-model
  workflows continue to return their canonical core or portfolio errors, and
  `UnmatchedPolicy` remains available at the factor-model crate root with the
  same `snake_case` JSON representation.
- **Breaking (Rust):** Renamed the serialize-only scenario outputs
  `ScenarioRevalueEnvelope` and `ScenarioPnlEnvelope` to
  `ScenarioRevalueView` and `ScenarioPnlView`, and renamed their helpers to
  `apply_and_revalue_view` and `scenario_pnl_view`. No deprecated aliases are
  provided because these pre-1.0 outputs are not round-trip persistence
  envelopes.
- **Breaking (JSON):** Attribution request deserialization now rejects missing
  or mismatched `schema` markers, matching the published schema `const` and
  existing attribution-result behavior.
- **Breaking (JSON):** Margin serialization and deserialization now enforce the
  published bounds for MPOR, collateral maturities, haircuts, concentration
  limits, default haircuts, and notification hours. Invalid publicly
  constructible values fail serialization instead of producing JSON that the
  same type cannot deserialize.

### Fixed

- Period cash / total-return carry no longer treats a deposit's opening
  notional draw (effective start) as buy-and-hold income. Bonds already
  skipped the issue-date draw; deposits now use the same rule, so a
  position opened on `as_of_t0` does not book `−notional` (FX-converted)
  into `carry` and `total_pnl`.
