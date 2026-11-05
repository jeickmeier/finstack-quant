# Unreleased

## Itemized changes

### WASM: TypeScript declarations come from the Rust bindings

The hand-written `index.d.ts` is gone. Every `#[wasm_bindgen]` export now types
its `JsValue` parameters and returns (`unchecked_param_type`,
`unchecked_optional_param_type`, `unchecked_return_type`), so wasm-bindgen's
`pkg/finstack_quant_wasm.d.ts` publishes no `any`, and the Rustdoc above each
export is its JSDoc. `index.d.ts` is generated over it by
`scripts/gen-facade-dts.mjs`. TypeScript-visible changes:

- Namespace interfaces are named by their facade path
  (`ValuationsInstrumentsNamespace`, `ModelsVolatilityNamespace`, ...); the
  `*Constructor` interfaces are gone (use `typeof valuations.instruments.Bond`).
- Parameters are declared under the camelCase names the runtime errors use
  (`instrumentJson`, `marketJson`, `envelopeJson`, `varValue`, ...).
- Classes declare `[Symbol.dispose]()`; the package declares `Symbol.dispose`
  itself, so ES2020 consumers still compile without `esnext.disposable`.
- Members the facade adds (`MarketContext.insert`, `CashFlowBuilder.build(market?)`,
  `Performance.lookbackReturns`/`periodStats` with a `FiscalConfig`, the chained
  builder methods) are declared by augmenting the package's classes.

### Statements: forecasts run on typed parameters; covenant bridge reads the result calendar

`ForecastSpec::parameters` reads a forecast's parameter map once into `ForecastParameters`, and every method runs on that typed form; `ForecastSpec::validate` is that read. A missing required parameter (`rate`, `season_length`, `mode`, ...) now fails when the model is built rather than when it is evaluated, and errors name the offending key (`Invalid GrowthPct forecast parameter 'rate': ...`, or the unknown key with the accepted ones); `null` is rejected rather than read as absent. Values do not move.

Renamed or reshaped public APIs:

- `RollupSpec::default_for` is `RollupSpec::declared_methods` (Python `RollupSpec.declared_methods`, WASM `statements.rollupSpecDeclaredMethods`).
- `EvaluationContext::new` and `PeriodHistory::from_named_maps` take the model's periods explicitly; `ttm()` no longer derives Gregorian periods from history keys.
- `evaluate_covenants(results, covenants)` and `forecast_breaches(results, covenants, config)` drop the `model` argument and test every period of `StatementResult::periods` (Python and WASM `statements_analytics.evaluateCovenants` / `forecastBreaches` likewise; the UI covenant requests drop `modelJson`). `StatementsAdapter::new` takes only the results. `CreditAssessment::compute` also spans `StatementResult::periods`.
- `finstack_quant_covenants::evaluate_series_generic` is `evaluate_series_from_model`.
- `PeerSet::from_universe` returns `(PeerSet, excluded_count)`; the Python and WASM constructors still return the peer set.
- `PnlBucketShare::field` is the `PnlBucket` enum and `MertonReport::errors` is keyed by `MertonReportMeasure`; both serialize to the same snake-case strings as before.

### Python: relative value report keeps typed extractors and takes universe JSON

`RelativeValueDimension.y` and `.x` return the `MetricExtractor` document (`{"named": "oas_bp"}`, `{"custom": ...}`, `{"multiple": "ev_ebitda"}`) instead of a flattened `"name"` / `"multiple:<name>"` label that could not tell a custom key from a canonical metric of the same name; they now equal the WASM report's `y` and `x`. `relative_value_report` also takes the whole `universe` as one JSON array text, as WASM does. No value moves.

### Statements: LTM covers twelve calendar months on daily models

`ttm()`, `capital_structure_summary` and the credit assessment's trailing sums (leverage, interest coverage and the leverage check) now share one window, `finstack_quant_core::dates::ltm_window`: the twelve calendar months ending at the reporting period's end. Monthly, quarterly, semi-annual, annual and weekly models read the same 12, 4, 2, 1 and 52 periods as before, so their values do not move. A daily model used to read a fixed 252 periods, about 8.3 calendar months of a calendar-day grid, so LTM EBITDA read low and leverage about 1.45x high; it now sums every model day in the trailing year, 365 or 366 on a calendar-day grid and the trading days (about 252) on a business-day grid, whose missing weekends and holidays are not missing data. A daily window is short, and its LTM figures are NaN or `None`, while the model's daily grid starts after the window's first day. The credit assessment's daily window previously demanded every calendar day, so a business-day model had no trailing sum at all; weekly and fiscal daily results now have one.

### UI: facade-coverage ledger counts three more computations

`core.SifmaSettlementClass` (`fromAgencyTerm` classifies a term), `models.credit.lgd.WorkoutCosts` (`totalRate` computes and `standard` is a preset) and `margin.excessCollateral` (computes collateral less required) are no longer excluded as data constructors. The `data-constructor` definition now also covers a handle class whose other members only read back what it was built from, which is what `DayCountContext`, `VolCubeExpirySlice` and `LatentSingleFactor` are. Coverage reads 176 of 639 units.

### UI: attribution fixture with a book Rust does not reconcile

The portfolio attribution fixture adds a native `metrics_based` attribution at a host tolerance of zero, where `portfolioAttributionReconciliationCheck` reports the book not reconciled (the bucket sum leaves about 5e-13 USD). Tests now render the "Not reconciled" badge and a position whose residual is outside Rust's tolerance. At the host's one-cent tolerance the book always reconciles, because Rust's buckets include the residual.

### UI: portfolio returns keep NaN and Infinity apart

`@finstack/use-portfolio-returns` keys its three queries with `requestSnapshot`, as `use-correlation-matrix` does, instead of putting the cloned request in the TanStack key, whose JSON hashing turns NaN and Infinity both into `null`. A blank horizon followed by an overflowing one now makes two native `twrrLinked` calls and shows each native error.

### UI: correlation heatmap always draws Rust's matrix

`@finstack/correlation-heatmap` no longer checks coefficients against [-1, 1] and drops its chart for one outside; Rust now returns every coefficient inside the bound, so the heatmap and the table always show. The Repaired-stamp test says plainly that no native panel reaches Rust's repair today.

### Portfolio: attribution explain text and bucket shares agree on a zero total

`PortfolioAttribution::explain` treated `total_pnl` as zero only below `1e-10`, while `pct_of_total_by_bucket` uses the default `RoundingContext` (half a unit of the base currency's output scale). A total of 0.003 USD printed percentage shares in the text while the table had none. Both now use one rule, so the text prints `0.0%` exactly where the shares are `None`. Python and WASM explain text follow. No UI fixture moves.

### Analytics: correlation matrices stay inside [-1, 1]

`Performance::correlation_matrix` now returns a diagonal of exactly 1 and clamps every off-diagonal to `[-1, 1]`, on the clean path and after Higham repair. The validator it applies first tolerates `1e-10` on the diagonal and `1e-12` past each bound, so rounding in the Pearson estimate or the repair could return a coefficient such as `1.0000000000000002`, which a correlation cannot take. Python `Performance.correlation_matrix` and WASM `correlationMatrix` document the bound. No UI fixture moves.

### Portfolio: a linked return rejects a non-finite horizon

`twrr_linked` now returns a validation error when `horizon_years` is NaN or infinite. A blank horizon field (NaN) used to come back as an unannualised cumulative return and an overflowing one (`1e999`, infinity) as an annualised return of zero. Zero and negative horizons still skip annualisation. Python `twrr_linked` raises `ValueError`; WASM `portfolio.twrrLinked` throws a `validation` `FinstackError`. The returns UI fixture records both native errors. No existing value moves.

### UI: facade-coverage ledger lists what a screen never wires

`finstack-quant-ui/facade-coverage.json` now excludes 108 more facade units, each with a reason from a closed set in `scripts/coverage.mjs` that `coverage --check` enforces: constructors and label parsers (`data-constructor`), enum-variant constructors (`schema-form`), copy-with-field setters (`setter`), stored-field lookups (`getter`), shape validators (`validator`) and the four `analytics` series functions that repeat a `Performance` method (`performance-duplicate`). The six covenant `with_*` setters move from `schema-form` to `setter`. Coverage reads 176 of 636 units instead of 176 of 744; computations, numeric checks, presets and later-phase targets stay counted.

### UI: correlation heatmap on the workbench Risk tab

`@finstack/correlation-heatmap` draws the Pearson correlation matrix of a host return panel, one native `analytics.Performance` `correlationMatrix` chain through `@finstack/use-correlation-matrix`, as a heatmap and a table in Rust's ticker order, stamped Repaired only when Rust's `repaired` flag says it projected the estimate to the nearest valid correlation matrix. `portfolio-workbench` shows it on the Risk tab when the host supplies a `correlation` panel.

### UI: exposure report on the workbench Risk tab

`@finstack/exposure-report` presents a book's primitive exposures, one native `portfolio.primitiveExposures` call through `@finstack/use-exposure-report`: each primitive's net and gross quantity and value in the base currency with a chart of the two values, each requested additive metric's net and gross amount, and the position-aware paths, all as returned; and, when the host supplies factor definitions, the book's `portfolio.computeFactorSensitivities` matrix. Rust returns both net and gross; nothing is summed or netted in JavaScript. `portfolio-workbench` shows it on the Risk tab over its book and market, with the host's `exposures` metric ids and factors.

### UI: Brinson report and the workbench Brinson tab

`@finstack/brinson-report` presents multi-period Brinson-Fachler attribution of the host's sector weights and returns, one native `portfolio.brinsonLinked` call through `@finstack/use-brinson-linked`: the compounded portfolio and benchmark returns, each sector's Carino-linked allocation, selection, interaction and total with Rust's linked total of each effect, a chart of the sectors' linked totals, and every period's returns, sector effects and totals, all as returned. Nothing is summed, differenced or linked in JavaScript. `portfolio-workbench` adds a Brinson tab when the host supplies `brinson` periods.

### UI: portfolio returns and the workbench Returns tab

`@finstack/portfolio-returns` answers how the book has performed from the host's performance data in three forms, each one native call through `@finstack/use-portfolio-returns`: `twrrLinked` of sub-period decimal returns over a horizon in 365-day years (cumulative, annualised, `is_annualised` and the period count as returned, saying when Rust did not annualise), `twrrModifiedDietz` of one period's values and flows (a decimal for the period; contributions positive) and `mwrXirr` of dated investor flows (an annualised decimal on Act/365F; contributions negative). Nothing is defaulted, derived from dates or sign-flipped, and each form shows its native error. `portfolio-workbench` adds a Returns tab when the host supplies `returns`.

### Portfolio: linked returns say whether they were annualised

`LinkedReturn` gains `is_annualised`: true when `twrr_linked`'s `horizon_years` is at least one year, false for shorter or non-positive horizons, where `annualised` mirrors `cumulative` (GIPS 2020 reports them cumulatively). A report can now say which figure it shows without restating that rule. Python `LinkedReturn.is_annualised` (and a `to_dataframe` column); WASM `portfolio.twrrLinked` returns the field. The host parity fixture pins a two-year and a half-year link for both hosts. No existing value moves.

### UI: portfolio attribution report and the workbench Attribution tab

`@finstack/portfolio-attribution-report` shows why the book moved between two snapshots as Rust returns it: the book total, the reconciliation verdict, gap and tolerance from `portfolioAttributionReconciliationCheck`, the base-currency buckets with their shares from `portfolioAttributionPctOfTotalByBucket` as a table and a signed chart, a row per position with one column per bucket from that position's own Rust buckets, the selected position through `pnl-attribution-report` (which now exports its factor table and chart as `PnlFactorBuckets`), and `portfolioAttributionExplainText`. `@finstack/use-attribute-portfolio-pnl` makes the one `attributePortfolioPnl` call. `portfolio-workbench` adds an Attribution tab when the host supplies `attribution` (closing market, closing date and reconciliation tolerance, none defaulted); every Rust method is offered with none preselected, and a failing method shows the native error.

### Portfolio: attribution buckets and their shares from Rust

`PortfolioAttribution::pct_of_total_by_bucket()` lists the book's eleven base-currency factor buckets (`fx_translation_pnl` included) and `residual`, the amounts `reconciliation_check` adds up, as `PnlBucketShare` records with each share of `total_pnl` (`None` when the total is effectively zero), so a report reads the buckets from Rust instead of naming them. Bound as Python `PortfolioAttribution.pct_of_total_by_bucket` and WASM `portfolio.portfolioAttributionPctOfTotalByBucket`; the host parity fixture pins both. No existing value moves.

### UI: portfolio workbench

`@finstack/portfolio-workbench` puts the portfolio components under tabs over a host-supplied book, market and margin date: Positions (summary and positions), Risk (aggregated metrics and a bucketed series), Scenarios (scenario P&L, shown when the host supplies scenarios), Margin and Book (a `PortfolioSpec` validated natively before it is valued). It adds no native call; the Scenarios and Margin runs start when their tab opens. No value moves.

### Portfolio: multi-period Brinson in one call

`brinson_linked(periods)` attributes each period with Brinson-Fachler and Carino-links the results, returning the existing `CarinoLinkedAttribution`: every period's sector allocation, selection and interaction, the linked effects by sector and by effect, and the compounded portfolio and benchmark returns, whose difference the linked effects sum to. It is identical to `carino_link` over each period's `brinson_fachler` result, so a host no longer loops over periods; a period that fails Brinson-Fachler is named by its zero-based index. Bound in Python (`brinson_linked`) and WASM (`portfolio.brinsonLinked`). No value moves.

### UI: recovery-waterfall fields from the contracts; DCF WACC default

`recovery-waterfall` labels its claim, collateral and workout fields through `labelFor` and takes their help from the generated contracts (the new `recovery_claim` contract root, and `workout_lgd`), so "Direct costs" reads "Direct cost rate", "Time to resolution" reads "Workout years" and "Exposure at default" reads "EAD" (`labelFor` upper-cases `ead`). "Add claim" starts the accrued, penalties and haircut fields empty instead of at 0, so an untyped amount is rejected by Rust rather than sent as zero. The statements workbench's valuation panel leaves the DCF form's WACC unset until the calculator or an edit sets it, so a form given `defaultAssumptions` shows its WACC instead of an empty field.

### Comps: each scatter point says whether it is the subject

`RelativeValuePoint` carries `is_subject`, true on the subject's point (the first point of a dimension whose subject has both metrics) and false on every peer's, as `RelativeValueCompany` already does for the comps table. Python `RelativeValuePoint.is_subject`, the WASM `relativeValueReport` result and the generated schema and TypeScript types include it; `RelativeValuePoint.from_json` now requires it. `relative-value-table` marks the subject on its scatter from it instead of comparing `company_id` with `subject_id`. No value moves.

### Comps and capital structure: only the conditions they name

`capital_structure_summary` lists the hedges it leaves out (interest rate swaps, cap/floors and swaptions) instead of skipping every instrument that is not one of the four debt kinds; an instrument that is neither is an error naming its type, as the statements crate's capital structure already does. Every instrument a model can hold today is one or the other, so no value moves. `relative_value_report` reports `non_finite_score` only when the composite or confidence is non-finite; any other scoring error is returned instead of being relabelled. No value moves.

### UI: capital structure and recovery tables fit the gallery width

`capital-structure-report` and `recovery-waterfall` now show returned numbers as `relative-value-table` does: ratios and rates to four decimals (never "-0") and the share of debt as a percent to two decimals, each cell keeping every returned digit in its title. The debt stack names its currency once in the caption ("Debt stack at 2025Q4 (USD)") instead of in three column headers. Both tables now show every column at gallery width, the debt stack's maturity and the recovery table's deficiency included. `formatRatio`, `formatShare` and `formatValue` still override the display.

### UI: id-combobox keeps typed free text

The `id-combobox` registry primitive no longer empties its value when the suggestions close. Typing an id that is not among the suggestions and then pressing Tab, Escape or clicking elsewhere left the field (and every SchemaForm node, metric or curve id built on it) as `''`, so Rust reported for example "Target node '' not found". Typed text is now the value until the user types again or picks a suggestion.

### Comps: relative value table in the UI

The `relative-value-table` registry component, with the `use-relative-value-report` hook, takes a host-supplied subject and universe of `CompanyMetrics`, edits a `RelativeValueReportConfig` (peer filter, period basis, scoring dimensions, multiples) with a schema form over the generated contract, and draws `statements_analytics.relativeValueReport` as returned: the comps table with the subject marked, each multiple's subject value, percentile bar, z-score and peer statistics, each dimension's regression scatter through Rust's fitted values with its signal, z-score and exclusion reason, and the composite score or Rust's reason for none. `labelFor` upper-cases `ev`, `fcf`, `ntm`, `pb`, `pe` and `ptbv`, so a multiple reads "EV EBITDA" or "PE".

### Comps: partial and strict peer filters

Every `PeerFilter` field may now be omitted from its serde document, an omitted list or bound screening on nothing, so `{"gics_sectors": ["Energy"]}` is a complete filter and `{}` accepts every company; until now a filter had to list all nine fields. `PeerFilter` and `CompanyMetrics` reject unknown keys, so a misspelt criterion (`gics_sector`) or metric (`ebidta`) is a validation error instead of being dropped. This applies to every host that passes either as JSON (`relative_value_report`, `PeerSet`, `PeerFilter.from_json`, `CompanyMetrics.from_json`, WASM `relativeValueReport` and `peerSetFromUniverse`); the flat-dict constructors, which file an unknown name under `custom`, are unchanged. No value moves.

### Statements: capital structure report in the UI

The `capital-structure-report` registry component, with the `use-capital-structure-summary` hook, edits a `CapitalStructureSummaryConfig` (period, EBITDA node, optional cash node) with a schema form and draws `statements_analytics.capitalStructureSummary` as returned: headline debt, cash, net debt, LTM EBITDA with the LTM periods found, leverage and coverage; the debt stack in Rust's tranche order with the figures through each payment class; and the maturity wall by year. Ratios Rust leaves undefined show as Undefined. In the statements workbench's Metrics & ratios tab it sits in a capital structure disclosure. `labelFor` upper-cases `ebitda` and `ltm`, so labels such as "EBITDA bridge" and "LTM EBITDA" change case.

### Statements: normalization editor in the UI

The `normalization-editor` registry component, with the `use-normalize` and `use-normalization-config-validator` hooks, edits a `NormalizationConfig` (add-backs, deductions and caps) with a schema form validated by `statements.validateNormalizationConfigJson`, and draws `statements.normalize` as returned: a base-to-adjusted bridge for a chosen period, each step between Rust's `adjusted_value`s, and a table of the adjustments by period. In the statements workbench's Metrics & ratios tab it sits in an Adjusted EBITDA disclosure over the reported result.

### Statements: adjusted value through each normalization step

`AppliedAdjustment` gains `adjusted_value` (Python `AppliedAdjustment.adjusted_value`, WASM and JSON `adjusted_value`): the target's value through that adjustment, the period's `base_value` plus the capped amounts of this and every earlier adjustment in declaration order. The last step equals `final_value` exactly, so a base-to-adjusted bridge reads each step's position from the result instead of summing. The field is required in `NormalizationResult` JSON; no existing value moves.

### Models: recovery waterfall in the UI

The `recovery-waterfall` registry component and `use-recovery-waterfall` hooks draw `models.credit.recoveryWaterfall.allocateRecovery` (Rust's per-claim allocations with recovery rates and deficiencies, and the estate's distributed and undistributed totals), `models.credit.lgd.workoutLgd` and `seniorityRecoveryStats`. Collateral types and seniority classes come from the generated `workout_lgd` and `seniority_calibration` contracts, now UI schema roots, and the calibration choice from `seniorityCalibrationIds`.

### Models: seniority calibration ids

`seniority_calibration_ids()` (Python `models.credit.lgd.seniority_calibration_ids`, WASM `models.credit.lgd.seniorityCalibrationIds`) lists the seniority recovery calibrations of the embedded credit registry, the default first, so a host can offer the exact ids `seniority_recovery_stats` accepts without holding its own list.

### Statements: table and CSV export in the UI

The `statement-export` registry component and `useStatementResultTable` hook draw `statements.statementResultToTableWide` and `statementResultToTableLong`, with a CSV download of exactly those cells (RFC 4180; a cell with no value is an empty field; numbers are unrounded). The statements workbench offers it below the grids, over the reported or annual result they show. `TableEnvelope` gains an optional `missingCell`.

### Statements: WACC calculator in the UI

The `wacc-calculator` registry component and `useWacc` hook draw `statements_analytics.wacc`: five decimal inputs, the two capital weights separate, and the rate Rust returns or its rejection. In the statements workbench's Valuation tab, Use as WACC sets the DCF form's WACC field on the user's click; `DcfAssumptionsForm` gains optional `wacc` and `onWaccChange` props for that.

### Comps: relative value report

`relative_value_report(subject, universe, config)` filters the universe and returns what a comps screen draws, so a host computes nothing: `companies` (the subject first, then peers in universe order, with one value per configured multiple), `multiples` (each with the subject's value, peer statistics, percentile and z-score) and `dimensions` (one per configured dimension in config order, excluded ones included, with subject values, peer statistics, percentile, z-score, regression, the direction-adjusted `signal`, `excluded_reason`, and scatter `points` that carry their fitted Y). `composite` is the result of `score_relative_value` on the same peers and dimensions, which now shares one per-dimension evaluation with the report; when it cannot be produced it is null, `composite_excluded_reason` says why and the table still renders. Empty `dimensions` and `multiples` together, or a negative or non-finite weight, are errors. Metrics carry no currency, so normalizing them is the caller's job.

Bound in Python (`relative_value_report`, `RelativeValueReport`, `RelativeValueCompany`, `RelativeValueMultiple`, `RelativeValueDimension`, `RelativeValuePoint`) and WASM (`statements_analytics.relativeValueReport`), with schemas for the request and the result. No value moves.

### Statements: debt stack by seniority

`capital_structure_summary(results, model, config)` answers how much debt sits ahead of each tranche and what leverage and coverage look like through it. It gives one row per bond, convertible, term loan and revolver at its balance at `config.period`, with a revolver at its drawn balance and swaps, cap/floors and swaptions left out; rank and class come from the waterfall's payment classes, or one implicit class when there are none. Tranches in a class are pari passu, so cumulative debt, leverage and coverage are figures through the whole class. It also gives total debt, cash, net debt, LTM EBITDA, total and net leverage, total coverage, and a maturity wall by calendar year of final maturity, which uses the balance at the period and ignores later amortization.

`ebitda_node` is required and never guessed; `cash_node` is optional. LTM EBITDA and interest sum the trailing year of calendar periods, the slots `ttm()` reads. A window short of a year, or with an EBITDA value missing, gives null ratios and `ltm_periods_found` says how many periods held: a quarterly figure is never annualised. Zero or negative LTM EBITDA also gives null ratios. Mixed currencies are an error, since no FX is applied.

Bound in Python (`capital_structure_summary`, `CapitalStructureSummary`, `CapitalStructureTranche`, `MaturityWallBucket`) and WASM (`statements_analytics.capitalStructureSummary`), with schemas for the request and the result. No value moves.

### Convertibles: theta carry no longer counts the issue draw or a same-day coupon

`theta_carry`, and so `theta`, on a `ConvertibleBond` counted the -notional issue draw when valued on its issue date, and a coupon paid on the valuation date, which the tree's PV already leaves out. A bond issued on the valuation date showed theta near -1,000,000. Both now follow `Bond`: carry excludes the draw and receipts fall in `(as_of, horizon]`. Values change on those two dates only.

P&L attribution of a convertible from one of those dates reads the same carry, so it changes too: the UI's fixture, one day from a 1,000 issue on 2025-01-01, moves from total P&L -1,060.42 and carry -999.90 to -60.42 and 0.10.

### Convertibles: value across share prices

`convertible_spot_ladder(bond, market, as_of, spots)` answers how a convertible moves with its stock. Each row reprices on the market with only the share price moved and gives price, delta, hedge ratio, regime, conversion value, parity, conversion premium and the bond floor, in ascending share price. Omit `spots` for 0.25 to 3 times the market's share price. Spots must be finite and positive; they are sorted and duplicates dropped. Price, delta, parity, conversion value, premium and floor are the definitions of the metrics of those names, so the market row equals them.

Regime is new and defined once in Rust, from `hedge_ratio`, delta over the effective conversion ratio: `busted` below 0.40, `balanced` to 0.80, `equity_sensitive` above. The cutoffs are one market convention, the bands Putnam and Lord Abbett publish; MSCI uses thirds (`docs/REFERENCES.md#convertible-delta-bands`). A mandatory-variable bond delivers a moving share count, so its hedge ratio and regime are null.

Bound in Python (`convertible_spot_ladder`, `ConvertibleSpotLadder`, `ConvertibleSpotRow`) and WASM (`valuations.instruments.convertibleSpotLadder`). `metricMetadata` now declares the units of `parity` (dimensionless), `conversion_premium` (decimal), `conversion_value` and `bond_floor` (currency), which were `unknown`. No value moves.

`@finstack/convertible-card` draws it: price, conversion value, floor and regime by share price, parity and premium, one row per share price and metrics by id. A priced convertible in the pricing workbench adds it as an Analytics tab, priced on opening; the ladder and all 17 metric ids take milliseconds. Share prices come from the user or Rust's default ladder.

### UI: bond and loan analytics in the pricing workbench

A priced bond now adds an Analytics result tab with `bond-credit-card`, `yield-workout-ladder` and `quote-converter`, each opening on its own over the completed request; a priced term loan adds one with `loan-yield-card`. No value moved.

### UI: bond credit card, shared metric table, loan default risk

`bond-credit-card` shows a bond's spreads, option value, durations, default risk, carry and breakeven by metric id, each id with its own native error, and offers the schema's form for `theta_period` and `breakeven_config`. `metric-values-table` is the table it shares with `loan-yield-card`; `applicableLoanYieldMetrics` and `LoanYieldRow` give way to its `applicableMetrics` and `MetricValueRow`. `loan-yield-card` adds `recovery01` and `jump_to_default`. No value moved.

### Bonds and loans: recovery01 and jump to default on a hazard curve

`recovery01` and `jump_to_default` are now registered on `Bond`, `TermLoan` and `RevolvingCredit`, in currency, for the whole notional, from the holder's perspective.

`recovery01` is the PV change per +1% (0.01 absolute) recovery move, by central difference of the full PV. It bumps the recovery each pricer reads: the hazard curve's on a bond or term loan, the facility's `recovery_rate` on a revolver. As for a CDS, a curve carrying its calibration recipe is re-bootstrapped to the same par spreads at the bumped recovery, and a hand-built curve keeps its hazard rates. The two differ greatly: at 250bp flat spreads and 40% recovery, a 5% five-year bond on 1mm has 8.03 recalibrated against 1,767.89 frozen, and a premium bond's recalibrated figure is negative. A quoted bond or loan is held at its calibrated spread, as CS01 is.

`jump_to_default` is value on immediate default less current value, and it follows the fractional recovery of par each pricer uses, without accrued interest: `R` times outstanding principal on a bond, `R` times funded balance on a term loan, and `R × drawn + (R − 1) × (leq × undrawn + LC draw)` on a revolver. Notional must be positive on these instruments, so a short is a position with negative quantity.

A bond, term loan or revolver not priced on a hazard curve gets a `Validation` error for either id, never a zero: no `credit_curve_id`, a bond under a model other than `hazard_rate` or `rates_credit`, a term loan under a model other than `tree`, or a revolver with stochastic draws, which takes survival from its simulated spread path. `metricsForInstrument` reads the type, so it lists both for every such instrument; nothing requests them by default. No existing value moves.

### Bonds: G-spread over a named government curve

The new `g_spread` metric is the bond's yield minus the par yield of a named government discount curve, in decimals. It uses the same yield and horizon as `i_spread`, through one shared helper: the worst exit on a bond with exercise rights and a quoted clean price, maturity otherwise. Over the bond's own discount curve it equals `i_spread` bit for bit. This is a working convention, pending the desk's decision on the benchmark: it interpolates the named curve at the horizon and selects no on-the-run bond.

Name the curve in `instrument_pricing_overrides.model_config.government_curve_id`, where `asw_forward_curve_id` already lives. No curve named is a `Validation` error (Python `ValueError`, WASM `kind: "validation"`), never a fallback to the discount curve, and a named curve missing from the market is the usual not-found error. `metricsForInstrument` lists `g_spread` for every bond because it reads the type, so a bond with no curve gets that error only when it asks for the id. Nothing requests it by default: `compute_quotes` and every default metric set are unchanged.

A named curve joins the bond's `market_dependencies` as a discount curve. No price or parallel DV01 moves, with the curve in the market or not, and `bucketed_dv01` gains zero rows for that curve.

### Bonds: an asset-swap quote no longer prices a bond with exercise rights

**Behaviour change.** A bond with exercise rights and `quoted_asw_market`, or `BondQuoteInput::AswMarket` on one, is now an error at every spread, as a Z-spread quote already is. It used to return a price: the inverse runs the swap to maturity, while the `asw_market` metric runs it to the worst exit, which depends on the price, so the returned `asw_market` could differ from the quote (a callable at 108 came back at 119.1566). This reaches every route through the quote path: `value`, `price_instrument`, the price metrics, OAS from a quote and `compute_quotes`. The `asw_market` metric and output field on such a bond are unchanged, and so is the quote on a plain fixed-coupon bond. Quote a clean price, or use OAS.

### Bonds: the whole quote set from any one quote

`compute_quotes` is now bound in Python (`compute_quotes`, `BondQuoteSet`) and WASM (`valuations.instruments.computeQuotes`), with JSON Schemas for `BondQuoteSet` and `BondQuoteInput`. Each yield and spread equals the metric of the same name at that clean price. A field is null when it does not apply to the bond or its metric has no finite value, never infinity or NaN; no finite value moves. A quote kind the bond does not have is an error: `discount_margin` with no floating coupons, `asw_market` or `japanese_simple_yield` on a floating coupon, `z_spread` or `asw_market` with exercise rights. One failed metric fails the whole set. `BondQuoteInput` adds `deny_unknown_fields`; it already rejected an unknown or second key.

`bond_quote_set_field_units(currency)` (Python) and `valuations.instruments.bondQuoteSetFieldUnits` (WASM) state the unit of every field in struct order, including `clean_price_pct` as `% of par`.

The `quote-converter` registry component and `useComputeQuotes` and `useBondQuoteSetFieldUnits` hooks draw it: enter any one quote, read the whole set Rust returns.

### Bonds: yield and spread to every exit

`bond_workout_ladder(bond, market, as_of)` answers which call date is the worst workout and what the yield to each is. It returns one row per admissible call, put and maturity exit: date, kind, redemption price and amount, whether the redemption is make-whole, Street yield, I-spread and whether the row is the worst. The ladder and the `ytw` metric run one walk (`solve_workout_exits`), so the worst row holds that metric's yield and the `i_spread` metric's spread. As that metric does, the spread's par rate runs to the exit date when the bond has an exit other than maturity, and to contractual maturity when it has none. It needs a quoted clean price on a bond with exercise rights and lowers a return floor into calls, as `ytw` does. Yield and spread are null on a path with no finite yield: an exit on the quote date, or same-day cash at or above the price. A worst row can be null, where `ytw` is negative infinity. Z-spread is not reported per exit: no per-exit convention exists.

Bound in Python (`bond_workout_ladder`, `BondWorkoutLadder`, `BondWorkoutRow`) and WASM (`valuations.instruments.bondWorkoutLadder`). No existing value moves.

The `yield-workout-ladder` registry component and `useBondWorkoutLadder` hook draw it, with the row Rust marks as the worst workout.

### Structured credit: a custom waterfall pays a debt class only what a note is owed

Per tranche and date, `interest_flows` plus `principal_flows` now equals `cashflows` with no exception. A note is owed interest at its rate and the return of its balance. A fixed or percentage amount is a fee formula with no contractual basis as either: it cannot be an extra coupon, and treating it as principal would silently retire the notional of what is usually an interest-only strip or a misrouted fee. So these waterfalls are now refused rather than reclassified.

#### Changed

- Breaking: `validate_custom_waterfall` and `validate_tiers` reject a `RecipientType::Tranche` paid by `FixedAmount`, `PercentageOfCollateral`, `PercentageOfSpecialServiced`, `IncentiveFee` or `ReserveReplenishment` (`ValidationError::NonNoteCalculation`), or by an interest, principal or carryover calculation naming another tranche (`ValidationError::TrancheMismatch`). `run_simulation` raises the same error. A fixed periodic paydown has no representation for now: `TranchePrincipal.target_balance` is a single target, not a schedule. Model a class that takes excess cash with `TrancheSeniority::Equity`.
- Breaking: a custom `NetWacCarryover` recipient must be named `net_wac_carryover_<tranche_id>` and the tranche must accrue carryover under `waterfall_rules.afc`.
- A debt class's non-principal cash is booked in full as interest instead of being capped at the claim. Only a coupon rounded above its claim moves: with `RoundingConvention::Ceiling` on a 7,000,000 `CLASS_A`, 2024-04-01 interest gains the 0.0078 that used to reach `cashflows` alone.
- What each rejected case booked as neither interest nor principal over the life of a 10,000,000 CLO with a 2,000,000 `CLASS_B`, in a residual tier ahead of equity: `FixedAmount` 50,000 per period 1,000,000.00 (lifetime cash 3,000,000.00); `PercentageOfCollateral` 1% annualized 446,980.89, 1% per period 1,762,221.97; `PercentageOfSpecialServiced` 10% per period 1,803,433.16; `IncentiveFee` 200,809.17; `ReserveReplenishment` 1,948,016.20; a misnamed `NetWacCarryover` of 10,000 per period 200,000.00. No template waterfall or checked-in fixture used these, so no standard deal's values move.

### Structured credit: residual cash paid to a debt class is a turbo

Moves values for any custom waterfall that pays `PaymentCalculation::ResidualCash` to a debt class. A note is owed its coupon and the return of its balance, so remaining cash directed at one is the turbo feature of ABS and CLO interest waterfalls. An extra coupon has no contractual basis and would not retire the balance the coverage tests read. Model income or subordinated notes that truly take the residual with `TrancheSeniority::Equity`, whose treatment does not change.

#### Changed

- Breaking: residual cash paid to a debt class is now principal. It is booked to `principal_flows` and `total_principal`, retires the balance, is capped at the balance the class has left in the period, and passes the excess to the next recipient or tier. It does not repay deferred interest first. It used to reach `cashflows` only, uncapped, with the balance untouched.
- Values on a 10,000,000 CLO with a 2,000,000 `CLASS_B` that takes the residual: first-period (2024-04-01) `CLASS_B` cash stays 110,769.19, with principal moving from 0 to 110,769.19. Lifetime `CLASS_B` cash moves from 4,004,045.88 to 2,000,000.00 and `total_principal` stays 2,000,000.00. The 2,004,045.88 that was booked as neither interest nor principal now flows on past the class.

### Structured credit: the standard deal families come from Rust

#### Added

- `standard_deal_families()` (`StandardDealFamily`; Python `standard_deal_families`, WASM `valuations.instruments.standardDealFamilies`) returns each family `new_standard` accepts with its registry profile id and an example `StandardDealParams` whose `id`, tranche ids and `pool.deal_type` name the family.

#### Changed

- Breaking: `StructuredCredit::new_standard` returns a validation error when `pool.deal_type` contradicts the family. It must equal the family, and the `abs` family also takes `auto` and `card` pools, which have no constructor of their own.

### Structured credit: absent values are null, not zero or infinity

Wire changes. Each field below is now nullable; every value that exists is unchanged. An engine's pass, fail and pricing logic does not move.

#### Changed

- `CoverageTestDiagnostic.ratio` and `.cushion` are `null` (Python `None`) when the test has no claim outstanding: the notes it covers are retired (OC) or owe no interest (IC). They were `Infinity` in JavaScript and `None` in Python. `passing` is still `true` there. Python `coverage_tests_dataframe` gives `NaN`.
- `AssetPool::wac`, `weighted_avg_spread_bp`, `weighted_avg_maturity` and `diversity_score` (Python alike, WASM `assetPool*` `number | undefined`), `PoolStats` of the same names, `PoolReport.warf` and `PeriodDiagnostics.wac`, `weighted_avg_spread_bp` and `warf` are `null` when their population is empty, so a floating-only pool has no WAC rather than a WAC of zero. `SimulationDiagnostics::to_table` makes those three columns nullable (Python `to_dataframe`: pandas `Float64`, `<NA>`). The `clo_was`, `clo_warf` and `wam` metrics still return `0.0` there, and `clo_warf` is unchanged for any pool with performing balance; RMBS `enable_stochastic` still passes a WAC of `0.0` to the prepayment model.
- `EquityMetrics.moic` and each `cash_on_cash` share are `null` when nothing is invested. A class that receives nothing on a positive investment keeps a real MOIC of `0.0`.
- `pool_report_field_units`: `wac` is `annual decimal, fixed-rate only`, `num_obligors` and `num_industries` are `named obligors` and `named industries`, as an asset naming neither is not counted.
- `scenario_table_field_units`: `price` is `clean, % of current balance`.

#### Added

- `EquityMetricsFieldUnits.purchase_price` states the unit of the `purchase_price_pct` argument and that omitting it means par.

### Structured credit: units of an equity tranche's metrics

#### Added

- `equity_metrics_field_units(currency)` (`EquityMetricsFieldUnits`; WASM `equityMetricsFieldUnits`) states the unit of each `EquityMetrics` number.

#### Changed

- `EquityMetrics` now documents its decimal, percent and multiple conventions and what a zero investment returns.

#### Fixed

- WASM `StructuredCredit.equityMetrics` documented an omitted `purchasePricePct` as investing at model NAV. It invests at par, as Rust and Python state.

### Structured credit: units of a simulation's charted series

#### Added

- `simulation_field_units(currency)` (`SimulationFieldUnits`; WASM `simulationFieldUnits`) states the unit of each `CoverageTestDiagnostic` number, `diverted_cash`, `credit_enhancement` and `excess_spread`.

#### Changed

- `PeriodDiagnostics.excess_spread` now documents its definition; its value is unchanged.

### Structured credit: scenario table currency and units

#### Added

- `ScenarioTable.currency` names the currency of each cell's `writedown`, as `TrancheMetrics.currency` does for its result.
- `scenario_table_field_units(currency)` (`ScenarioTableFieldUnits`; WASM `scenarioTableFieldUnits`) states the unit of every `ScenarioCell` field, split into swept `axes` and reported `measures`.

#### Changed

- `ScenarioTable` JSON now requires `currency`.

### Structured credit: a tranche's cash flows by date in WASM

#### Added

- WASM `valuations.instruments.trancheCashflowsTable(cashflows)` returns Rust `TrancheCashflows::to_table` as a `TableEnvelope`: one row per payment date with cashflow, interest, principal, PIK, deferred and writedown columns. Python already reaches the same table through `TrancheCashflows.to_dataframe`.

### Structured credit: one report for a collateral pool

#### Added

- `pool_report(pool, as_of)` returns a `PoolReport`: `calculate_pool_stats`, Moody's WARF of the performing collateral (the number `clo_warf` and `PeriodDiagnostics.warf` report) and concentration by obligor, industry and rating. Each `PoolConcentrationEntry` holds the key (`null` for assets with no value on the dimension), balance, `balance_pct` in percent points and asset count; tables cover defaulted assets, sum to 100% and are ordered by balance descending, then key ascending. Python `pool_report`, WASM `poolReport`.
- `pool_report_field_units(currency)` (`PoolReportFieldUnits`) states the unit of every report field. Python `pool_report_field_units`, WASM `poolReportFieldUnits`.

#### Changed

- All structured-credit WARF numbers share one function, so `CloWarfCalculator` now skips non-positive-balance rows, as `PeriodDiagnostics.warf` already did. Pools with positive balances are unaffected.

### Structured credit: who got paid each period

#### Added

- `PeriodDiagnostics.waterfall` (`PeriodWaterfall`): each period's tier allocations, `PaymentRecord`s, diversions and carried cash, on the `SimulationDiagnostics.periods` that `StructuredCredit::run_simulation` returns (Python `run_simulation`, WASM `runSimulation`). `PaymentRecord` gains `is_principal`.
- `PeriodDiagnostics.non_waterfall_payments` (`NonWaterfallPayment`, `NonWaterfallSource`): cash paid to a tranche outside the waterfall (routed reserve interest, redemption, terminal releases). Per tranche and date, payment records plus these sum to the tranche's cash flows.
- `PeriodDiagnostics.credit_enhancement` (`TrancheCreditEnhancement`): each tranche's end-of-period share of collateral beneath it, generalising the function the `MinCreditEnhancement` step-down trigger reads.
- `SimulationDiagnostics.first_breaches` (`CoverageTestBreach`; Python `SimulationDiagnostics.first_breaches`): each coverage test's first failing payment date, including tranche triggers that never reach `coverage_tests`.

#### Changed

- A deal with no payment period left that still settles cash now reports one record in `SimulationDiagnostics.periods` holding that settlement.

### UI boundary: rules decided in Rust

`check-worker-compose` (`ui-check`) now flags a worker service that decides from a native result; pure composition takes `// finstack-compose: <reason>`.

#### Changed

- `Performance::lookback_returns` takes `Option<Date>` (Python `ref_date=None`, WASM optional `refDate`) and defaults to the last active date, erroring when none is active; it returns `Result`. `LookbackReturns` gains a required `ref_date` stating the date the windows ended on.
- `generate_tornado_entries` (Python `generate_tornado_entries`, WASM `statements_analytics.generateTornadoEntries`) returns `None` / `null` for a full-grid result instead of an error. A full grid has no tornado by construction, so a host no longer checks the mode first.

#### Removed

- The `rate-input` registry item and the UI's `rateColumn`, `rateToWire`, `convertRate`, `formatRate` and `RatePresentation`. Nothing in the registry used them, and they scaled rate units in JavaScript. Display percent is now `formatPercent`.

#### Added

- `models::credit::merton_report` (Python `models.credit.merton_report`, WASM `models.credit.mertonReport`) returns the full Merton report in one call. It owns the KMV default point of an equity source and the default master scale, so hosts cannot differ on them. A non-positive horizon now fails the whole report, and a measure rejected for the model is null with its message in `errors` rather than a structured error.
- `MarketQuoteOverrides::price_driver_fields()` (Python and WASM `valuations.instruments.price_driver_fields` / `priceDriverFields`) publishes the mutually exclusive price-driving quote fields. `price_driver_count` reads the same definition, and `InstrumentForm` takes the list through `priceDriverFields` (connect `usePriceDriverFields`) instead of holding a copy.
- `pricer::tranche_metric_field_units(currency)` (Python `tranche_metric_field_units`, WASM `valuations.instruments.trancheMetricFieldUnits`) publishes the unit and order of the scalar `TrancheMetrics` fields. `TrancheAnalytics` requires them as `metricUnits` (connect `useTrancheMetricFieldUnits`).
- `MetricMetadata.coordinates`: for each entry of `components`, the `group` and numeric `value` (tenor in years or decimal strike) that order it on its axis. The UI sorts by these instead of parsing labels.
- `PnlAttribution::pct_of_total_by_bucket()` (Python `PnlAttribution.pct_of_total_by_bucket`, WASM `attribution.pnlAttributionPctOfTotalByBucket`) lists every money bucket with its amount and share of total. `PnlAttributionReport` requires it as `buckets` and always shows the share column; `HorizonReturnReport` requires it as `attributionBuckets`. A bucket added to `PnlAttribution` now reaches the report, which previously hid it behind two JavaScript field lists.
- `risk::evaluate_risk_budget_from_decomposition(decomposition, targets, threshold)` (Python `evaluate_risk_budget_from_decomposition`, WASM `models.factor.risk.evaluateRiskBudgetFromDecomposition`) evaluates a budget against a decomposition in one call, joining component VaR and portfolio VaR in Rust. `usePositionRisk` calls it.
- `ApplicationReport::effective_as_of(as_of)` (Python `ApplicationReport.effective_as_of`, WASM `scenarios.applicationReportEffectiveAsOf`) returns the valuation date after a scenario: the time roll's new date, else `as_of`.
- `InstrumentCashflowEnvelope::calendar_year_ladder_by_currency()` (Python `InstrumentCashflowEnvelope.calendar_year_ladder_by_currency`, WASM `valuations.instruments.calendarYearLadderByCurrency`) returns one `CurrencyLadder` per row currency over `native_pv`, so a host no longer partitions the flows or picks the PV column. `useCashflowLadder` calls it.
- `pricer::tranche_solve_field_units()` (Python `tranche_solve_field_units`, WASM `valuations.instruments.trancheSolveFieldUnits`) publishes the units of the break-even CDR, discount margin and `OasResult` fields. `TrancheAnalytics` requires them as `solveUnits` (connect `useTrancheSolveFieldUnits`); `num_paths` now reads "paths".
- `statements::types::ForecastParameters`, published as the `forecast_parameters` schema, states each forecast method's parameters in the wire shape of `ForecastSpec`. The workbench forecast editor renders it with `SchemaForm` instead of a per-method field table, so its field labels are now the Rust parameter names and optional fields appear behind Add.

### UI: schema forms hold errors until an edit or a submit

#### Changed

- `SchemaForm` holds a field's validation issue until an edit touches its path or submit is attempted, and the first attempt shows them all. Choosing a union branch, adding an optional term or adding an array row no longer counts as an edit, so these actions no longer mark the field dirty. A form with `actions={false}` has no submit to attempt and shows its issues as before.

### Statements: rollup methods declared per line

#### Changed

- `StatementResult` has a required `periods` field (Python `StatementResult.periods`): the model's periods for an evaluated result and the target periods for a rollup, each actual only when every source period is. Empty only for hand-built results. JSON without it no longer deserializes.

#### Added

- `NodeSpec.rollup` (Rust `NodeSpec::with_rollup`, Python `NodeSpec(..., rollup=...)`, WASM through the JSON model): each statement line declares its rollup method on the node. `StatementResult::rollup` resolves a line as its entry in `RollupSpec::methods`, then its declaration, then `sum`, so an empty spec gives every host the same numbers. A line resolving to `formula` with no formula is an error naming the node. `RollupMethod` moved from `statements::evaluator` to `statements::types`.
- `ModelBuilder::rollup(node_id, method)` and `MixedNodeBuilder::rollup(method)` (Python `rollup`, WASM `rollup`) declare a line's method from the public builder, so no line has to drop to `NodeSpec` and `insert_node`.
- `RollupSpec::default_for(&model, frequency)` (Python `RollupSpec.default_for`, WASM `statements.rollupSpecDefaultFor`) lists the declared methods so a host can seed its choices. The tag rule it briefly carried is gone: `balance_sheet`, `ratio` and `metric` tags no longer choose a method anywhere, so a model that relied on them sums until its lines declare `rollup`. The statements workbench no longer decides in JavaScript, and it now sends an explicit `sum` so choosing Sum overrides a line's declaration.

### Analytics: one path per capability

#### Changed

- Python and WASM: an unknown `benchmark_ticker` in the `Performance` constructors (Python `Performance()`, `from_arrays`, `from_returns`, `from_returns_arrays`; WASM `new Performance`, `fromReturns`) and in `reset_bench_ticker` / `resetBenchTicker` raises `KeyError` (WASM kind `not_found`), not `AnalyticsError` (kind `validation`). A failed `reset_bench_ticker` leaves the benchmark unchanged.
- `gain_to_pain` and the `PeriodStats` payoff and profit ratios return `NaN` when a sum or a compounded bucket growth overflows `f64`; they previously returned `+inf` or `0.0`.
- The `multi_factor_greeks` tracing span and events carry the target `finstack_quant_analytics::multi_factor` instead of `finstack_quant_analytics::benchmark`.
- Python `Performance.lookback_returns` and `period_stats`, and WASM `lookbackReturns` and `periodStats`, take the core `FiscalConfig` handle (`fiscal_config` / `fiscalConfig`) instead of the `fiscal_year_start_month` / `fiscal_year_start_day` integers. An omitted config is Rust `None`: a calendar-year FYTD window, and Gregorian periods with ISO weeks for `period_stats`. An out-of-range fiscal start is now a `ValueError` from the `FiscalConfig` constructor, not an `AnalyticsError`.

#### Removed

- Core `FiscalConfig::from_parts(start_month, start_day)`, which filled an omitted half of the fiscal start. The bindings take the `FiscalConfig` handle now; build one with `FiscalConfig::new` or use `FiscalConfig::default()` (calendar year).

### Valuations: one path per capability

#### Changed

- Structured credit has one per-tranche API: `tranche_metrics(deal, tranche_id, market, as_of, market_price_pct, metrics)` (was `calculate_tranche_metrics`, and absorbs `value_tranche_with_metrics` / `TrancheValuation`). `TrancheMetrics` renames `price_pct` → `clean_price_pct`, `modified_duration` → `effective_duration`, `convexity` → `effective_convexity`, and gains `dirty_price_pct`, `accrued`, `ytm` and requested registry `metrics`. The other tranche analytics are `tranche_oas`, `tranche_discount_margin`, `tranche_breakeven_cdr` and `tranche_scenario_table` in Rust and Python (was `structured_credit_tranche_*`) and `trancheMetrics` etc. in WASM (was `structuredCreditTranche*`).
- WASM: the `valuations.fx` namespace is folded into `valuations.instruments`, and FX TypeScript types take their Rust names (`FxForward`, `FxOption`, ...).
- `X::builder()` is the one construction entry point: `#[derive(FinancialBuilder)]` no longer emits `XBuilder::new()`, and the `*FutureOption::new`, `EquityOption::new`, `FxSpot::new` and `TrancheBuilder::new` constructors are gone. `PrivateMarketsFund::new` returns `Result`; `CdsTranche::new` runs the full validation.
- `EquityOptionParams::new` defaults settlement to Cash (was Physical), matching the instrument's serde and builder default.
- `CashflowSpec::fixed_rate`, `floating_bp` and `floating_with_reset_lag` are gone; use `fixed` and `floating(.., reset_lag_days: Option<i32>)`.
- Commodity forwards pay on maturity plus the convention's settlement days on a registered exchange calendar (NYMEX, COMEX and CBOT on `cme`, ICE Brent and LME on `gblo`, power on the new `nerc` calendar), so PV and cashflows discount to that payment date.
- MBS Monte Carlo OAS takes its path count and seed from `model_config` (`mc_paths`, `mc_seed`) like the other MC pricers.
- Ten instruments (FX spot, forward, swap, NDF, quanto option; equity TRS, DCF, cliquet, autocallable; real-estate asset) deserialize with `#[serde(remote = "Self")]` and `validate_on_deserialize!` instead of hand-written `*Unchecked` mirrors; their published schema property docs now come from the real structs.
- `BrentSolver::solve_fallible` propagates objective errors; YTM, Z-spread and DM now report the pricer's error instead of the solver's.
- Missing-metric errors use the metric's wire id (`metric:z_spread`).
- Python docstrings come from the `.pyi` stubs for the converted modules (`valuations.market`, `valuations.composite`, the typed instruments and every domain's `schema`); `finstack_quant.valuations.{instruments, composite, market}` are the compiled modules (the `.py` re-export shims are gone).
- Stricter edge cases where duplicate code paths disagreed: an FX variance swap with no return interval is an error, commodity Black-76 rejects degenerate inputs, a configured but missing FI-TRS index yield is NotFound (Python `KeyError`), out-of-range SMM/MDR conversions error, `FxOption::validate` rejects a non-finite or negative strike, and a quanto whose `fx_spot_id` scalar is missing fails instead of using the FX matrix.

#### Removed

- Wire fields and variants that changed no number: `ClawbackSpec.settle_on` and `ClawbackSettle` (clawback settles at fund end), `CdsOption.exercise_style` and `premium_settlement_date`, `CommoditySpotOption.{convention, premium_settlement_days}`, `VolatilityIndexFuture.discount_curve_id`, `BermudanSchedule.notice_days`, and `YieldCompounding::TreasuryActual`. JSON carrying these fields is rejected.
- Always-zero metrics: repo `collateral_haircut01` / `collateral_price01` and CDS tranche `tail_dependence`.
- Setters that shadowed builders or public fields (swaption, cap/floor, IR future, Bermudan schedule, basis swap, structured credit, pool asset, tranche, FX spot, CDS index and option, `ValuationResult::with_covenant(s)`).
- Dead public API with no callers, including `extract_risk_factors`, `ListedDeliveryObligation`, `CdsPricerConfig`, the CDS index `*_detailed` results, `StructuredCredit::{generate_cashflows, apply_deal_defaults}`, `BasketCalculator`, `CompositeInstrument::new`, `instrument_cashflows_json` and `format_bucket_label_cow` (`format_bucket_label` returns `Cow`).
- Python `ValuationResult.{price_decimal, metric_keys, metric_count}`, the composite JSON twins `execution_trades_json` / `trades_json` / `instrument_json` (`CompositeHistoryResult.row` is the typed twin of `row_json`), and WASM `weightingMethodFixedQuantity`, `weightingMethodNotionalWeighted`, `rebalanceRuleManual` and the FX bare-spec constructor.

### Statements: one path per capability

#### Changed

- Gaussian Monte Carlo forecasts retain their sampled shocks directly, preserving correlation when small increments round away at large levels. Zero-volatility nodes remain ineligible as stochastic correlation peers.

- Capital-structure residual scaling warnings use `ResidualScaleMismatch { applied_ratio }` (`residual_scale_mismatch` in JSON), replacing the inaccurate `ScaleClamped` warning and its obsolete fields. Cashflow scaling is unchanged.

- Capital-structure runtime state, dated principal claims, and waterfall execution are crate-private. The evaluator owns their lifecycle; the unused cumulative-principal accumulator and redundant waterfall period argument are removed.

- `PikToggleSpec.target_instrument_ids` is a required list across Rust, Python, and JSON. Missing and null target lists are rejected; validation requires at least one borrowing-debt target.

- Waterfall failures leave capital-structure runtime state unchanged. Successful allocations commit balances, PIK state, and carried claims together.

- Accounting identity checks report period-level error findings when sums, differences, tolerances, or materiality calculations overflow. They publish no non-finite comparison payload for that period and continue checking later periods.

- `CapitalStructureCashflows::get_component(component, instrument_or_total, period_id)` replaces the 16 per-component and total getters in Rust and Python; WASM exposes `capitalStructureCashflowsGetComponent`. Instrument lookups retain native currency and `total` reads the reporting-currency aggregate.

- `Evaluator::evaluate_prepared` evaluates a model's capital structure exactly as `Evaluator::evaluate` does (an empty market as of the first period's start), so prepared evaluation now yields `cs_cashflows` and `cs.*` values. Before, it skipped the capital structure.
- `ModelBuilder::with_builtin_metrics` no longer overwrites a node the model already defines with the same id.
- `ModelBuilder` has one mutable configuration API: setters return `&mut Self` or `Result<&mut Self>` and rejected setters preserve the accumulated model. `build` and `mixed` consume the builder; `MixedNodeBuilder::build` returns its parent builder. The consuming setter and `try_*` twins are removed. `ModelBuilder::new` is the construction entry point, and `build` rejects a model without a timeline.
- The statements-analytics entry points take the names the bindings already used: `run_sensitivity`, `run_variance`, `variance_bridge`, `explain_formula`, `render_check_report_text`/`_html`, `evaluate_dcf` (was `evaluate_dcf_with_market`). The engine structs behind them are private.
- `dcf_sensitivity` takes no market argument (Rust, Python and WASM).
- Each statements and statements-analytics item has one public path: internal modules (`checks::{suite,traits,types,helpers}`, `dsl::{ast,compiler,parser}`, `capital_structure::waterfall`, `registry::dynamic`, `evaluator::formula`, `adjustments::{engine,types}`, `forecast`, and the analysis submodules) are private, with their public items re-exported at the parent. `NormalizationEngine` is replaced by `adjustments::normalize` and `adjustments::merge_into_results`.

#### Removed

- Wire fields, variants and types that had no effect: `DcfOptions.discount_curve_id`, `VarianceRow.driver_contribution`, `EvalWarning::NaNPropagated`, `EclRequest`, `EclStageRequest`, `ExposureEclResult` and the `ecl::portfolio` module, and the toggled-PIK interest path in `calculate_period_flows`.
- Forwarders and dead analysis API: `EclEngine`, the `config()` accessors, the comps `extract_metric`/`subject_metric` and `Multiple::is_*`/`label`, the analysis `forecast_covenant` forwarder, `VarianceReport::to_table`/`is_empty`, and `statements::formula`.
- Python `normalize_json` and WASM `normalizeJson`; Python `explain_formula_text`, `pl_summary_report_text` and `validate_scorecard_config`.
- The 20 WASM constructor functions that only built a plain object (`forecastSpec*`, `adjustment*`, `terminalValueSpec*`, `sensitivityConfigAddParameter`). Pass the object literal instead, e.g. `{ method: 'growth_pct', params: { rate: 0.05 } }`.

### Cashflows: one path per capability

#### Changed

- `CFKind`, `CashFlow` and `CashFlowAccrual` bind where Rust defines them: Python `finstack_quant.core.cashflow` (was `finstack_quant.cashflows.primitives`) and WASM `core.cfKindParse` / `core.cfKindIs*` / `core.cashFlowGetBalanceDate` / `core.cashFlowValidate` (were under `cashflows`).
- `CashFlowSchedule::builder()` is the only builder entry point. `schedule_from_dated_flows` / `schedule_from_classified_flows` validate their rows and return `Result`. Python and WASM schedules built from host parts are validated.
- `pv_by_period` drops its unused day-count argument (Rust, Python, WASM); the PV path rejects flows in a currency other than the requested base.
- `weighted_average_life_from_principal` moves from `builder` to `aggregation`. `CFKind::is_principal_repayment()` names the repayment kind set used by schedule WAL, principal-repaid aggregation and statements.
- Accrual items and `PeriodAggregation` are reached through `accrual::` and `aggregation::` only; `PrincipalEventSpec` is exported from `builder`, and `MarginStepSpec` / `PaymentStepSpec` from the crate root. `WEEKENDS_ONLY_ID` lives in `core::dates`.
- `FloatingRateSpec::DEFAULT_GEARING`, `DEFAULT_GEARING_INCLUDES_SPREAD` and `DEFAULT_RESET_LAG_DAYS` are public; the Python constructor uses them. Fee specs default their business-day convention through serde like `ScheduleParams`.
- Term-loan call exercise builds its outstanding path and accrual indexes once per schedule, not on every tree step.

#### Removed

- `finstack_quant_cashflows::primitives` (use `finstack_quant_core::cashflow`).
- `CashFlowSchedule::npv` (use `finstack_quant_core::cashflow::npv` over `dated_flows()`), the `IntoIterator` impls on `CashFlowSchedule` (use `into_flows()` / `get_flows()`), `PeriodAggregation::into_inner`, and the six `CashFlowBuilder::get_*` getters (the builder implements `Display`).
- `builder::emit_inflation_coupons` (inlined at the inflation-linked bond) and `AssetPool::wal_from_cashflows`.
- `clamped_smm_to_cpr`, `clamped_mdr_to_cdr`, `psa_to_cpr` and `cdr_2pct` (Rust, Python, WASM), and the inert SDA registry record (`SDA_PEAK_CDR` replaces it).
- Host `from_parts` constructors, `DateContext`, `valuations::__private`, the valuations re-exports of cashflows types, and the cashflows crate-root re-exports of accrual items.

### Statements: scenario forecast overrides, rollups, capital-structure scenarios

#### Added

- `ScenarioDefinition.forecast_overrides` replaces a node's whole `ForecastSpec` in a scenario: the method and every parameter (growth rates, curves, seeds, volatilities, external series, bounds). Child scenarios replace a parent's spec for the same node, and explicit values still win (`Value > Forecast > Formula`). Python `ScenarioSet` accepts a `ForecastSpec` as an override value.
- `StatementResult::rollup(&model, &RollupSpec)` rolls results up to a coarser frequency (monthly into quarterly, semi-annual or annual; quarterly into semi-annual or annual; semi-annual into annual; fiscal periods keep their `FY` labels). Nodes sum by default; `period_end` nodes take the last source value and `formula` nodes re-evaluate their formula on the rolled-up values. Every target period must be complete. Python `StatementResult.rollup` / `RollupSpec`; WASM `statements.statementResultRollup`.
- `ScenarioSet::evaluate_all_with_market(&model, &market, as_of)` evaluates every scenario like `Evaluator::evaluate_with_market`. Python `evaluate_scenario_set_with_market`; WASM `statements_analytics.evaluateScenarioSetWithMarket`.

#### Changed

- `Evaluator::evaluate` (and `ScenarioSet::evaluate_all`, Python `evaluate_scenario_set`, WASM `evaluateScenarioSet`) evaluates a model's capital structure from an empty market as of the first period's start, so fixed-rate debt yields `cs_cashflows` and `cs.*` values. Instruments that need market data (floating coupons, swaps, caps, swaptions) now fail with a missing-data error naming the instrument (Python `KeyError`) instead of being skipped silently; evaluate those with `evaluate_with_market`. `evaluate_prepared` still skips the capital structure.
- Formula checks report a period whose same-period input is missing or non-finite, or whose formula consumes a non-finite value (a `lag` before the first period), as a `warning` "insufficient data" finding that does not fail the check. Such periods previously failed with the configured severity, or aborted the suite when a value was missing. A division by zero over finite inputs still fails.

### Valuations: verified pricing and risk fixes

#### Fixed

- Rates: the default Black cap/floor pricer and the Hull-White cap/floor pricer now subtract an unsettled premium, as `CapFloor::base_value` does; registry prices were gross before. CMS option `Vanna` no longer errors on seasoned options and uses the ATM vol in the convexity adjustment, as the pricer does. `XccySwap::validate` rejects same-currency legs for every swap, not only MtM-resetting ones. Bermudan `ExpectedExerciseTime` is published by the pricer: under the LMM pricer it now errors instead of silently building a Hull-White tree.
- Fixed income: `Bond::metrics_equivalent` converts PIK coupons to cash for step-up, floating and amortizing step-up bonds (it handled only fixed), so their spread and yield metrics are on a cash basis. Convertible `parity`, `conversion_premium` and the conversion-value metric reject a spot price quoted in a currency other than the bond's.
- Credit: the `CdsIndex`, `CdsOption` and `CdsTranche` registry pricers report a missing curve as missing market data (Python `KeyError`) rather than a model failure.
- FX: `FxBarrierOption` and `QuantoOption` vega, vanna and volga bump the implied-volatility override when one is set (they returned 0 or failed). `FxBarrierOption` delta, gamma and vanna work when spot comes from the FX matrix (no `fx_spot_id`). `QuantoOption` vega is a central difference (was one-sided).
- Exotics: a missing curve on `AsianOption`, `BarrierOption`, `LookbackOption` and `QuantoOption` raises `KeyError` instead of `ValueError`. Lookback and expired-barrier spot reads reject a spot price in another currency. Lookback and range-accrual `Rho` reprice with the requested model (a non-default model previously mixed with the default) and are central discount-curve bumps.

### Structured credit: scenario-shock consistency and dead-code removal

#### Fixed

- Pricing a `StructuredCredit` through the registry with `ModelKey::StructuredCreditStochastic` and a `scenario_price_shock_decimal` now shocks the `ValuationDetails::StructuredCreditStochastic` payload (npv, clean/dirty price, standard error, confidence interval, tranche PVs) and the `pv_std_error` / `tranche_npv::<id>` measures once, as `price_stochastic_with_mode` does. Before, only the headline value was shocked.

#### Changed

- `calculate_tranche_metrics` takes a floating tranche's discount margin from the z-spread it already solved instead of re-running the deal waterfall; the ±5000 bp bound still applies. `Severity01` is computed as `-Recovery01` from the same two repricings (bit-identical to before), so requesting both reprices twice instead of four times.

#### Removed

- Unused public structured-credit items: `cmo::tranches::pac_support::allocate_pac_support`, `structured_credit::PaymentMode`, and the constants `MONTHS_PER_YEAR`, `QUARTERLY_PERIODS_PER_YEAR`, `PERCENTAGE_MULTIPLIER`, `MIN_PREPAYMENT_RATE` and the function `pool_balance_cleanup_threshold()` (the simulation reads the threshold from the assumptions registry). `BASIS_POINTS_DIVISOR` stays.

### Valuations metric registration cleanup

#### Changed

- `register_metrics!` takes an optional `custom: [("id", calculator), ...]` list registered under `MetricId::custom`, after the `metrics` list. Metric ids, applicability and values are unchanged.
- `finstack_quant_valuations_rates::{cms_common, forward_swap_rate, cms_swap::pricer}` are `pub(crate)`; nothing outside the crate used them. `cms_option::pricer` and `cms_option::replication_pricer` stay public.

#### Removed

- `FxSpot::new_t1` (use `FxSpot::new(..).with_settlement_days(1)`), `Ndf::effective_fixing_source` and `Ndf::with_quote_convention` (use the builder's `quote_convention`).
- `InstrumentPricingOverrides::with_bk_sigma` and `with_bk_mean_reversion`; set `model_config.bk_sigma` / `model_config.bk_mean_reversion` directly.
