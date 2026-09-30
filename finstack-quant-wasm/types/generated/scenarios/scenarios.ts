// Generated from the finstack-quant-scenarios JSON schemas by scripts/generate-contract-types.mjs. Do not edit.

/**
 * Asset class categories affected by a stress template.
 *
 * These values describe the primary risk buckets touched by a historical
 * scenario so registries can expose coarse filtering and search.
 */
export type AssetClass = "rates" | "credit" | "equity" | "fx" | "volatility" | "commodity";
/**
 * Compounding convention for interest rates.
 *
 * Used to specify how interest rates should be quoted or converted.
 * All variants produce mathematically equivalent discount factors when
 * applied consistently.
 *
 * # Relationship Between Conventions
 *
 * For a given discount factor DF at time t, the rates under different
 * conventions are related by:
 *
 * ```text
 * DF = e^(-r_cc × t)                    [Continuous]
 *    = (1 + r_ann)^(-t)                 [Annual]
 *    = (1 + r_per/n)^(-n×t)             [Periodic(n)]
 *    = 1 / (1 + r_simple × t)           [Simple]
 * ```
 *
 * # Ordering of Rates
 *
 * For positive rates and t > 0: `r_simple > r_annual > r_continuous`
 * (less frequent compounding requires a higher quoted rate for the same DF).
 */
export type Compounding =
  | "continuous"
  | "annual"
  | {
      periodic: number;
    }
  | "simple";
/**
 * ISO 4217 currency enumeration
 */
export type Currency =
  | "AED"
  | "AFN"
  | "ALL"
  | "AMD"
  | "ANG"
  | "AOA"
  | "ARS"
  | "AUD"
  | "AWG"
  | "AZN"
  | "BAM"
  | "BBD"
  | "BDT"
  | "BGN"
  | "BHD"
  | "BIF"
  | "BMD"
  | "BND"
  | "BOB"
  | "BRL"
  | "BSD"
  | "BTN"
  | "BWP"
  | "BYN"
  | "BZD"
  | "CAD"
  | "CDF"
  | "CHF"
  | "CLF"
  | "CLP"
  | "CNY"
  | "COP"
  | "CRC"
  | "CUC"
  | "CUP"
  | "CVE"
  | "CZK"
  | "DJF"
  | "DKK"
  | "DOP"
  | "DZD"
  | "EGP"
  | "ERN"
  | "ETB"
  | "EUR"
  | "FJD"
  | "FKP"
  | "GBP"
  | "GEL"
  | "GHS"
  | "GIP"
  | "GMD"
  | "GNF"
  | "GTQ"
  | "GYD"
  | "HKD"
  | "HNL"
  | "HRK"
  | "HTG"
  | "HUF"
  | "IDR"
  | "ILS"
  | "INR"
  | "IQD"
  | "IRR"
  | "ISK"
  | "JMD"
  | "JOD"
  | "JPY"
  | "KES"
  | "KGS"
  | "KHR"
  | "KMF"
  | "KPW"
  | "KRW"
  | "KWD"
  | "KYD"
  | "KZT"
  | "LAK"
  | "LBP"
  | "LKR"
  | "LRD"
  | "LSL"
  | "LYD"
  | "MAD"
  | "MDL"
  | "MGA"
  | "MKD"
  | "MMK"
  | "MNT"
  | "MOP"
  | "MRU"
  | "MUR"
  | "MVR"
  | "MWK"
  | "MXN"
  | "MYR"
  | "MZN"
  | "NAD"
  | "NGN"
  | "NIO"
  | "NOK"
  | "NPR"
  | "NZD"
  | "OMR"
  | "PAB"
  | "PEN"
  | "PGK"
  | "PHP"
  | "PKR"
  | "PLN"
  | "PYG"
  | "QAR"
  | "RON"
  | "RSD"
  | "RUB"
  | "RWF"
  | "SAR"
  | "SBD"
  | "SCR"
  | "SDG"
  | "SEK"
  | "SGD"
  | "SHP"
  | "SLE"
  | "SLL"
  | "SOS"
  | "SRD"
  | "SSP"
  | "STN"
  | "SYP"
  | "SZL"
  | "THB"
  | "TJS"
  | "TMT"
  | "TND"
  | "TOP"
  | "TRY"
  | "TTD"
  | "TWD"
  | "TZS"
  | "UAH"
  | "UGX"
  | "USD"
  | "UYU"
  | "UZS"
  | "VED"
  | "VES"
  | "VND"
  | "VUV"
  | "WST"
  | "XAF"
  | "XCD"
  | "XOF"
  | "XPF"
  | "YER"
  | "ZAR"
  | "ZMW"
  | "ZWL";
/**
 * Identifies which family of curve an operation targets.
 *
 * These variants map to the market data collections exposed by
 * `finstack_quant_core::market_data::context::MarketContext`.
 * They also determine which quoting and interpolation conventions apply when
 * downstream helpers extract rates or apply node shocks.
 *
 * # Examples
 * ```rust
 * use finstack_quant_scenarios::CurveKind;
 *
 * let kind = CurveKind::Discount;
 * assert_eq!(format!("{:?}", kind), "Discount");
 * ```
 */
export type CurveKind = "discount" | "forward" | "par_cds" | "inflation" | "commodity";
/**
 * ISO 8601 calendar date encoded as a `YYYY-MM-DD` JSON string.
 */
export type DateWire = string;
/**
 * Supported day-count conventions with industry-standard definitions.
 *
 * Each variant implements a specific day count convention as defined by
 * ISDA, ICMA, or local market conventions. The conventions determine how
 * interest accrues between payment dates.
 *
 * # Standards References
 *
 * Implementations follow:
 * - **ISDA**: 2006 ISDA Definitions, Section 4.16
 * - **ICMA**: ICMA Rule Book, Rule 251
 * - **ISO**: ISO 20022 Day Count Fraction Codes
 *
 * # Examples
 *
 * ```rust
 * use finstack_quant_core::dates::{Date, DayCount, DayCountContext};
 * use time::Month;
 *
 * let start = Date::from_calendar_date(2025, Month::January, 1).expect("Valid date");
 * let end = Date::from_calendar_date(2025, Month::July, 1).expect("Valid date");
 *
 * // Actual/360 - money market convention
 * let yf_360 = DayCount::Act360.year_fraction(start, end, DayCountContext::default()).expect("Year fraction calculation should succeed");
 *
 * // 30/360 - bond convention
 * let yf_30360 = DayCount::Thirty360.year_fraction(start, end, DayCountContext::default()).expect("Year fraction calculation should succeed");
 *
 * assert!(yf_360 > yf_30360); // Act/360 has larger denominator
 * ```
 */
export type DayCount =
  | "one_one"
  | "act_360"
  | "act_365f"
  | "act_365l"
  | "30_360"
  | "30e_360"
  | "30e_360_isda"
  | "30_360_it"
  | "nl_365"
  | "act_act"
  | "act_act_isma"
  | "act_act_afb"
  | "bus_252";
/**
 * How ParCDS operations deliver a spread shock onto a hazard curve.
 *
 * [`Self::SolveToPar`] is the production default: implied par CDS spreads are
 * shocked and the hazard is re-bootstrapped so the curve still prices those
 * quotes. [`Self::FirstOrderShift`] converts the spread shock to a hazard
 * shift using `delta_hazard = delta_spread / (1 - recovery)` and emits an
 * approximation warning. It does not reconcile the resulting par quotes.
 */
export type HazardBumpMode = "solve_to_par" | "first_order_shift";
/**
 * A predicate for filtering nodes by their tags.
 */
export type TagPredicate =
  | {
      equals: {
        /**
         * Tag key to match.
         */
        key: string;
        /**
         * Expected tag value.
         */
        value: string;
        [k: string]: unknown;
      };
    }
  | {
      in: {
        /**
         * Tag key to match.
         */
        key: string;
        /**
         * Accepted tag values.
         */
        values: string[];
        [k: string]: unknown;
      };
    }
  | {
      exists: {
        /**
         * Tag key that must be present.
         */
        key: string;
        [k: string]: unknown;
      };
    };
/**
 * A phantom-typed identifier that prevents mixing different kinds of IDs.
 *
 * This type wraps a string identifier with a phantom type parameter to ensure
 * type safety at compile time. Different `Id<T>` types with different `T`
 * cannot be compared or mixed accidentally, preventing entire classes of bugs.
 *
 * # Type Parameters
 *
 * * `T` - Phantom type tag that distinguishes this ID from IDs with different tags
 *
 * # Invariants
 *
 * - Storage uses `Arc<str>` for efficient cloning
 * - The phantom marker has zero size and runtime cost
 * - Two `Id<T>` values are equal if their string values are equal
 * - IDs with different type tags (`Id<A>` vs `Id<B>`) cannot be compared
 *
 * # Examples
 *
 * ```rust
 * use finstack_quant_core::types::{CurveId, InstrumentId};
 *
 * // Create IDs with different type tags
 * let curve = CurveId::from("USD-SOFR");
 * let bond = InstrumentId::from("ISIN:US912828XG60");
 *
 * // Can compare IDs of the same type
 * assert_eq!(curve, CurveId::from("USD-SOFR"));
 * assert_ne!(curve, CurveId::from("EUR-ESTR"));
 *
 * // Cannot compare IDs of different types (compile error):
 * // let _ = curve == bond;  // Error: mismatched types
 * ```
 *
 * # Thread Safety
 *
 * `Id<T>` is `Send + Sync` as it wraps an `Arc<str>`. Multiple threads can
 * safely share and clone IDs with minimal synchronization overhead.
 */
export type Id = string;
/**
 * Strongly-typed instrument classification for pricer dispatch.
 *
 * Each variant represents a distinct instrument type with its own pricing
 * logic and risk characteristics. Used by the pricing registry to route
 * instruments to appropriate pricer implementations.
 */
export type InstrumentType =
  | "bond"
  | "credit_default_swap"
  | "cds_index"
  | "cds_tranche"
  | "cds_option"
  | "interest_rate_swap"
  | "cap_floor"
  | "swaption"
  | "bermudan_swaption"
  | "basis_swap"
  | "basket"
  | "convertible_bond"
  | "deposit"
  | "equity_option"
  | "fx_option"
  | "fx_spot"
  | "fx_swap"
  | "xccy_swap"
  | "inflation_linked_bond"
  | "inflation_swap"
  | "yoy_inflation_swap"
  | "inflation_cap_floor"
  | "interest_rate_future"
  | "variance_swap"
  | "fx_variance_swap"
  | "equity"
  | "repo"
  | "forward_rate_agreement"
  | "structured_credit"
  | "private_markets_fund"
  | "revolving_credit"
  | "asian_option"
  | "barrier_option"
  | "lookback_option"
  | "quanto_option"
  | "autocallable"
  | "cms_option"
  | "cms_swap"
  | "cliquet_option"
  | "range_accrual"
  | "fx_barrier_option"
  | "term_loan"
  | "discounted_cash_flow"
  | "real_estate_asset"
  | "levered_real_estate_equity"
  | "trs_equity"
  | "trs_fixed_income_index"
  | "bond_future"
  | "commodity_forward"
  | "commodity_swap"
  | "commodity_option"
  | "commodity_asian_option"
  | "commodity_swaption"
  | "commodity_spread_option"
  | "volatility_index_future"
  | "fx_forward"
  | "ndf"
  | "agency_mbs_passthrough"
  | "agency_tba"
  | "dollar_roll"
  | "agency_cmo"
  | "fx_digital_option"
  | "fx_touch_option"
  | "tarn"
  | "cms_spread_option"
  | "callable_range_accrual"
  | "snowball"
  | "composite"
  | "commodity_future"
  | "fx_future"
  | "equity_future"
  | "equity_total_return_future"
  | "interest_rate_future_option"
  | "equity_future_option"
  | "fx_future_option"
  | "commodity_future_option"
  | "volatility_index_future_option"
  | "asset_backed_facility";
/**
 * Type-safe identifier for a node in a financial model.
 *
 * Serializes as a plain string and is interoperable with `&str` via
 * [`Borrow`] and [`AsRef`].
 */
export type NodeId = string;
/**
 * Individual operation within a scenario.
 *
 * Each variant represents a specific type of shock or adjustment that can be
 * applied to market data, instruments, statements, or the valuation horizon.
 * Units are encoded in the variant name and field docs:
 * - `Pct` fields use percentage points (`5.0 = +5%`)
 * - `Bp` fields use additive basis points (1 bp = 1e-4), except
 *   [`CurveKind::Commodity`] where `bp` is percent of the price-curve
 *   forward
 * - Vol-index `Pts` are **index points** (`1.0` on 18.5 → 19.5)
 * - Correlation / base-corr `Pts` are **decimal correlation** (`0.02` = +0.02)
 *
 * Hierarchy-targeted variants are resolved into direct identifiers during
 * [`crate::engine::ScenarioEngine::apply`] using the market hierarchy attached
 * to the execution context.
 */
export type OperationSpec =
  | {
      /**
       * Base currency in the FX pair.
       */
      base: Currency;
      kind: "market_fx_pct";
      /**
       * Percentage change (positive means base strengthens).
       */
      pct: number;
      /**
       * Quote currency in the FX pair.
       */
      quote: Currency;
    }
  | {
      /**
       * Equity identifiers to shock.
       */
      ids: string[];
      kind: "equity_price_pct";
      /**
       * Percentage price change.
       */
      pct: number;
    }
  | {
      /**
       * Attributes to match. Matching uses AND semantics with
       * case-insensitive key/value comparison.
       */
      attrs: {
        [k: string]: string;
      };
      kind: "instrument_price_pct_by_attr";
      /**
       * Percentage price change.
       */
      pct: number;
    }
  | {
      /**
       * Basis point shift (additive), or percent of the price-curve forward
       * when `curve_kind` is [`CurveKind::Commodity`].
       */
      bp: number;
      /**
       * Curve identifier.
       */
      curve_id: Id;
      /**
       * Type of curve (Discount, Forward, ParCDS, Inflation, Commodity).
       */
      curve_kind: CurveKind;
      /**
       * Optional explicit discount curve identifier when an operation must pair
       * `curve_id` with a discounting curve (some forward/hazard bumps need one).
       * If `None`, adapters apply engine default resolution; set this when multiple
       * curves could match or you need deterministic selection.
       */
      discount_curve_id?: Id | null;
      kind: "curve_parallel_bp";
    }
  | {
      /**
       * Curve identifier.
       */
      curve_id: Id;
      /**
       * Type of curve (Discount, Forward, ParCDS, Inflation, Commodity).
       */
      curve_kind: CurveKind;
      /**
       * Optional explicit discount curve identifier when an operation must pair
       * `curve_id` with a discounting curve (some forward/hazard bumps need one).
       * If `None`, adapters apply engine default resolution; set this when multiple
       * curves could match or you need deterministic selection.
       */
      discount_curve_id?: Id | null;
      kind: "curve_node_bp";
      /**
       * How to handle tenors not in the curve.
       */
      match_mode?: TenorMatchMode;
      /**
       * Vector of `(tenor, shift)` pairs. Shift is rate bp except for
       * [`CurveKind::Commodity`], where it is percent of the forward.
       */
      nodes: [unknown, unknown][];
    }
  | {
      /**
       * Curve identifier.
       */
      curve_id: Id;
      kind: "vol_index_parallel_pts";
      /**
       * Absolute index-point shift.
       */
      points: number;
    }
  | {
      /**
       * Curve identifier.
       */
      curve_id: Id;
      kind: "vol_index_node_pts";
      /**
       * How to handle tenors not in the curve.
       */
      match_mode?: TenorMatchMode;
      /**
       * Vector of (tenor, points) pairs in absolute vol-index points.
       */
      nodes: [unknown, unknown][];
    }
  | {
      kind: "base_corr_parallel_pts";
      /**
       * Absolute shift in decimal correlation (`0.02` = +0.02).
       */
      points: number;
      /**
       * Surface identifier.
       */
      surface_id: Id;
    }
  | {
      /**
       * Optional detachment points in basis points (e.g., 300 for 3%).
       */
      detachment_bp?: number[] | null;
      kind: "base_corr_bucket_pts";
      /**
       * Absolute shift in decimal correlation (`0.02` = +0.02).
       */
      points: number;
      /**
       * Surface identifier.
       */
      surface_id: Id;
    }
  | {
      kind: "vol_surface_parallel_pct";
      /**
       * Percentage change in volatility.
       */
      pct: number;
      /**
       * Volatility-surface identifier.
       */
      vol_surface_id: Id;
    }
  | {
      kind: "vol_surface_bucket_pct";
      /**
       * Percentage change in volatility.
       */
      pct: number;
      /**
       * Optional strike levels.
       */
      strikes?: number[] | null;
      /**
       * Optional tenor strings (e.g., "1M", "3M").
       */
      tenors?: string[] | null;
      /**
       * Volatility-surface identifier.
       */
      vol_surface_id: Id;
    }
  | {
      kind: "stmt_forecast_percent";
      /**
       * Statement node identifier.
       */
      node_id: NodeId;
      /**
       * Percentage change to apply.
       */
      pct: number;
    }
  | {
      kind: "stmt_forecast_assign";
      /**
       * Statement node identifier.
       */
      node_id: NodeId;
      /**
       * Absolute value to assign.
       */
      value: number;
    }
  | {
      /**
       * The rate binding specification to apply.
       */
      binding: RateBindingSpec;
      kind: "rate_binding";
    }
  | {
      /**
       * Attributes to match. Matching uses AND semantics with
       * case-insensitive key/value comparison.
       */
      attrs: {
        [k: string]: string;
      };
      /**
       * Basis point shift to apply.
       */
      bp: number;
      kind: "instrument_spread_bp_by_attr";
    }
  | {
      /**
       * Instrument types to shock.
       */
      instrument_types: InstrumentType[];
      kind: "instrument_price_pct_by_type";
      /**
       * Percentage price change.
       */
      pct: number;
    }
  | {
      /**
       * Basis point shift to apply.
       */
      bp: number;
      /**
       * Instrument types to shock.
       */
      instrument_types: InstrumentType[];
      kind: "instrument_spread_bp_by_type";
    }
  | {
      /**
       * Additive shock in decimal correlation (`0.02` = +0.02).
       */
      delta_pts: number;
      kind: "asset_correlation_pts";
    }
  | {
      /**
       * Additive shock in decimal correlation (`0.02` = +0.02).
       */
      delta_pts: number;
      kind: "prepay_default_correlation_pts";
    }
  | {
      /**
       * Basis point shift (additive), or percent of the price-curve forward
       * when `curve_kind` is [`CurveKind::Commodity`].
       */
      bp: number;
      /**
       * Type of curve (Discount, Forward, ParCDS, Inflation, Commodity).
       */
      curve_kind: CurveKind;
      /**
       * Optional discount curve for hazard-curve recalibration.
       *
       * Propagated to each expanded [`OperationSpec::CurveParallelBp`].
       * If `None`, the adapter applies heuristic resolution.
       */
      discount_curve_id?: Id | null;
      kind: "hierarchy_curve_parallel_bp";
      /**
       * Hierarchy target to resolve to curves.
       */
      target: HierarchyTarget;
    }
  | {
      kind: "hierarchy_vol_surface_parallel_pct";
      /**
       * Percentage change in volatility.
       */
      pct: number;
      /**
       * Hierarchy target to resolve to surfaces.
       */
      target: HierarchyTarget;
    }
  | {
      kind: "hierarchy_equity_price_pct";
      /**
       * Percentage price change.
       */
      pct: number;
      /**
       * Hierarchy target to resolve to equity IDs.
       */
      target: HierarchyTarget;
    }
  | {
      kind: "hierarchy_base_corr_parallel_pts";
      /**
       * Absolute shift in decimal correlation (`0.02` = +0.02).
       */
      points: number;
      /**
       * Hierarchy target to resolve to surfaces.
       */
      target: HierarchyTarget;
    }
  | {
      /**
       * Whether to continue applying the remaining scenario operations after the roll.
       *
       * When `false`, [`crate::engine::ScenarioEngine::apply`] returns
       * immediately after the roll-forward step.
       */
      apply_shocks?: boolean;
      kind: "time_roll_forward";
      /**
       * Period to roll forward (e.g., "1D", "1W", "1M", "1Y")
       */
      period: string;
      /**
       * Roll mode controlling calendar vs business-day behaviour.
       *
       * Defaults to `BusinessDays`, which respects calendars when provided.
       */
      roll_mode?: TimeRollMode;
    };
/**
 * Strategy for aligning requested tenor bumps with curve pillars.
 *
 * [`TenorMatchMode::Exact`] requires the requested tenor to coincide with an
 * existing pillar. [`TenorMatchMode::Interpolate`] distributes the bump
 * across the two adjacent knots, then **calibrates those pillar deltas so
 * the curve's native interpolant hits the requested shock at the requested
 * tenor** (discount zeros, inflation implied annual rates, forward rates,
 * and commodity price forwards). The initial guess is the minimum-norm
 * `1/Σw²` split; a few scale/Newton iterations adjust it when the live
 * interpolant is not linear-on-rate. On-pillar requests reduce to a single
 * full-size pillar bump. Delivery is first-order only for par-CDS
 * solve-to-par recalibration, which still emits
 * [`Warning::InterpolatedNodeBumpFirstOrder`](crate::warning::Warning::InterpolatedNodeBumpFirstOrder).
 *
 * # Examples
 * ```rust
 * use finstack_quant_scenarios::TenorMatchMode;
 *
 * let mode = TenorMatchMode::Interpolate;
 * assert_eq!(format!("{:?}", mode), "Interpolate");
 * ```
 */
export type TenorMatchMode = "exact" | "interpolate";
/**
 * Controls how time roll-forward periods are interpreted.
 *
 * Use [`TimeRollMode::BusinessDays`] when the scenario should respect holiday
 * calendars and business-day adjustment rules. Use
 * [`TimeRollMode::CalendarDays`] when the tenor should be added without a
 * business-day adjustment. [`TimeRollMode::Approximate`] uses fixed day-count
 * approximations aligned with the internal period-to-day conversion.
 *
 * # Reported day counts are always calendar days
 *
 * Every mode reports `RollForwardReport::days` (and `HorizonResult::horizon_days`)
 * as the *calendar-day* span between the old and new as-of dates. `BusinessDays`
 * adjusts the **target date**, not the unit of the count: a `1M` business-day
 * roll whose target is carried from a Saturday to the following Monday reports
 * 33 calendar days, not 31 and not a business-day tally. Downstream ACT/365F
 * annualization depends on this.
 *
 * # Non-additivity of `Approximate`
 *
 * [`TimeRollMode::Approximate`] is **not additive across composed rolls**.
 * Month periods round `months * 365 / 12` independently for each operation.
 * For example, `6M`
 * resolves to 183 days, so two `6M` approximate rolls produce 366 days while
 * one `12M` or `1Y` roll resolves to 365 days. For chained horizons or
 * scenario composition prefer
 * [`TimeRollMode::BusinessDays`] or [`TimeRollMode::CalendarDays`], which
 * both resolve the target date via [`finstack_quant_core::dates::Tenor`] and are
 * additive modulo the chosen business-day convention.
 */
export type TimeRollMode = "business_days" | "calendar_days" | "approximate";
/**
 * Controls how shocks at multiple hierarchy levels combine for a single curve.
 */
export type ResolutionMode = "most_specific_wins" | "cumulative";
/**
 * Exact schema marker accepted by [`ScenarioEnvelope`].
 */
export type ScenarioSchema = "finstack_quant.scenario/1";
/**
 * Severity classification for stress scenarios.
 *
 * This label is intended for discovery and filtering rather than for pricing
 * logic. Registries and UIs can use it to group historical events by the
 * magnitude of the modeled dislocation.
 */
export type TemplateSeverity = "mild" | "moderate" | "severe";

/**
 * A target specifying a hierarchy path with optional tag filtering.
 */
export interface HierarchyTarget {
  /**
   * Path through the hierarchy (e.g., `["Credit", "US", "IG"]`).
   */
  path: string[];
  /**
   * Optional tag filter applied to nodes in the subtree.
   */
  tag_filter?: TagFilter | null;
  [k: string]: unknown;
}
/**
 * A filter combining multiple tag predicates (AND semantics).
 */
export interface TagFilter {
  /**
   * All predicates must match (AND semantics).
   */
  predicates: TagPredicate[];
  [k: string]: unknown;
}
/**
 * Configuration for rate binding between curves and statement nodes.
 *
 * Specifies how to extract a rate from a market curve and link it to a
 * statement forecast node. This enables automatic propagation of market
 * rate shocks to financial statement projections.
 *
 * The extracted rate is written into the statement model as a decimal scalar
 * (for example `0.0525` for 5.25%). `compounding` controls the output quote
 * convention, while `day_count` optionally overrides the curve's native day
 * count when converting `tenor` into a year fraction.
 *
 * Persisted `day_count` override values are the canonical snake_case
 * [`DayCount`] labels: `act_360`, `act_365f`, `act_365l`, `nl_365`, `30_360`,
 * `30e_360`, `30e_360_isda`, `act_act`, `act_act_isma`, and `bus_252`.
 *
 * # Examples
 * ```rust
 * use finstack_quant_scenarios::spec::RateBindingSpec;
 * use finstack_quant_scenarios::spec::Compounding;
 *
 * let binding = RateBindingSpec {
 *     node_id: "InterestRate".into(),
 *     curve_id: "USD_SOFR".into(),
 *     tenor: "1Y".into(),
 *     compounding: Compounding::Continuous,
 *     day_count: None, // Use curve's day count
 * };
 * ```
 */
export interface RateBindingSpec {
  /**
   * Compounding convention for rate conversion.
   */
  compounding?: Compounding;
  /**
   * Curve ID to extract rate from.
   */
  curve_id: Id;
  /**
   * Output quote day-count convention. If `None`, uses the curve's convention.
   * Changes the annualized rate while preserving maturity dates and the
   * curve-implied accumulation factor; native curve query times are unchanged.
   */
  day_count?: DayCount | null;
  /**
   * Statement node ID to receive the rate.
   */
  node_id: NodeId;
  /**
   * Tenor for rate extraction (e.g., "3M", "1Y").
   */
  tenor: string;
}
/**
 * Versioned scenario specification with typed market, instrument, statement, and time-roll operations.
 */
export interface ScenarioEnvelope {
  /**
   * Bare scenario specification used by in-process APIs.
   */
  scenario: ScenarioSpec;
  /**
   * Exact scenario contract marker.
   */
  schema: ScenarioSchema;
}
/**
 * A complete scenario specification with metadata and ordered operations.
 *
 * # Fields
 * - `id`: Stable identifier used for persistence and reporting.
 * - `name`: Optional display name for UI or logs.
 * - `description`: Optional text describing the intent of the scenario.
 * - `operations`: Ordered list of [`OperationSpec`] values to execute.
 * - `priority`: Used by [`ScenarioSpec::compose`](crate::spec::ScenarioSpec::compose)
 *   to determine merge ordering (lower numbers run first).
 * - `resolution_mode`: Controls how hierarchy-targeted shocks at multiple tree
 *   levels combine for a single curve. Defaults to [`ResolutionMode::MostSpecificWins`].
 * - `hazard_bump_mode`: ParCDS delivery. Defaults to
 *   [`HazardBumpMode::SolveToPar`].
 *
 * # Examples
 *
 * ```rust
 * use finstack_quant_scenarios::{HazardBumpMode, ScenarioSpec, OperationSpec, CurveKind};
 * use finstack_quant_core::market_data::hierarchy::ResolutionMode;
 *
 * let scenario = ScenarioSpec {
 *     id: "stress_test".into(),
 *     name: Some("Q1 Stress".into()),
 *     description: Some("Rate shock scenario".into()),
 *     operations: vec![
 *         OperationSpec::CurveParallelBp {
 *             curve_kind: CurveKind::Discount,
 *             curve_id: "USD_SOFR".into(),
 *             discount_curve_id: None,
 *             bp: 50.0,
 *         },
 *     ],
 *     priority: 0,
 *     resolution_mode: ResolutionMode::default(),
 *     hazard_bump_mode: HazardBumpMode::default(),
 * };
 * ```
 */
export interface ScenarioSpec {
  /**
   * Optional description.
   */
  description?: string | null;
  /**
   * How ParCDS curve operations rewrite hazard curves.
   *
   * Default: [`HazardBumpMode::SolveToPar`]. Omitted from JSON when left
   * at that default so existing envelopes keep their wire shape.
   */
  hazard_bump_mode?: HazardBumpMode;
  /**
   * Unique identifier for this scenario.
   */
  id: string;
  /**
   * Optional human-readable name.
   */
  name?: string | null;
  /**
   * List of operations to apply.
   */
  operations: OperationSpec[];
  /**
   * Priority for composition (lower value = higher priority).
   */
  priority?: number;
  /**
   * Resolution mode for hierarchy-targeted operations.
   *
   * Only relevant when operations use hierarchy targeting.
   * Default: [`ResolutionMode::MostSpecificWins`].
   */
  resolution_mode?: ResolutionMode;
}
/**
 * Descriptive metadata of a built-in scenario template.
 */
export interface TemplateMetadata {
  /**
   * Asset classes materially affected by the scenario.
   */
  asset_classes: AssetClass[];
  /**
   * IDs of composable sub-component templates.
   *
   * Empty when the template is atomic rather than a composite built from
   * multiple reusable scenario fragments.
   */
  components: string[];
  /**
   * Description of the historical event and modeled effects.
   */
  description: string;
  /**
   * Primary date associated with the historical event.
   *
   * This is typically the date of the market dislocation rather than the
   * valuation date used when the template is later executed.
   */
  event_date: DateWire;
  /**
   * Stable identifier for the template.
   */
  id: string;
  /**
   * Human-readable template name.
   */
  name: string;
  /**
   * Severity classification for the template.
   */
  severity: TemplateSeverity;
  /**
   * Freeform tags used for filtering and discovery.
   */
  tags: string[];
}
