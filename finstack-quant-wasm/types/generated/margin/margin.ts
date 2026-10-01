// Generated from the finstack-quant-margin JSON schemas by scripts/generate-contract-types.mjs. Do not edit.

/**
 * Built-in [`ScheduleAssetClass`] spellings.
 */
export type BuiltInScheduleAssetClass = "interest_rate" | "credit" | "equity" | "commodity" | "fx" | "other";
/**
 * CCP methodology type.
 */
export type CcpMethodology =
  | "lch_swap_clear"
  | "lch_cds_clear"
  | "cme"
  | "ice_clear_credit"
  | "ice_clear_us"
  | "jscc"
  | "eurex"
  | {
      generic_var: {
        /**
         * Confidence level (e.g., 0.99 for 99%)
         */
        confidence: number;
        /**
         * Lookback period in days
         */
        lookback_days: number;
      };
    };
/**
 * Clearing status for OTC derivatives.
 *
 * Determines whether a trade is cleared through a CCP or remains bilateral
 * under a CSA agreement.
 */
export type ClearingStatus =
  | "bilateral"
  | {
      cleared: {
        /**
         * CCP identifier (e.g., "LCH", "CME", "ICE", "JSCC")
         */
        ccp: string;
        [k: string]: unknown;
      };
    };
/**
 * Collateral asset classes per BCBS-IOSCO standards.
 *
 * Asset classes determine baseline haircuts and eligibility criteria.
 * The BCBS-IOSCO framework specifies minimum haircuts by asset class.
 *
 * # Reference
 *
 * BCBS-IOSCO "Margin requirements for non-centrally cleared derivatives" (2020)
 * Annex A: Standardized haircut schedule `docs/REFERENCES.md#bcbs-iosco-uncleared-margin`
 */
export type CollateralAssetClass =
  | "cash"
  | "government_bonds"
  | "agency_bonds"
  | "covered_bonds"
  | "corporate_bonds"
  | "equity"
  | "gold"
  | "mutual_funds";
/**
 * FRTB correlation scenario for capital charge aggregation.
 *
 * The final SBA capital charge is max(low, medium, high).
 * Low and high scenarios scale prescribed correlations per BCBS d457,
 * floored/capped by the scenario-specific Basel formulas.
 */
export type CorrelationScenario = "low" | "medium" | "high";
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
 * Initial margin calculation methodology.
 *
 * Different methodologies are used depending on regulatory requirements,
 * product type, and whether trades are cleared or bilateral.
 *
 * # BCBS-IOSCO Standards
 *
 * For bilateral (uncleared) OTC derivatives, either SIMM or the regulatory
 * schedule approach may be used. SIMM is the industry standard for large
 * dealers due to its risk-sensitivity.
 */
export type ImMethodology = "haircut" | "simm" | "schedule" | "internal_model" | "clearing_house";
/**
 * Exact decimal encoded only as a JSON string.
 */
export type DecimalWire = string;
/**
 * Margin call frequency.
 *
 * Determines how often margin calls are made and collateral is exchanged.
 * Industry standard for OTC derivatives is daily under BCBS-IOSCO rules.
 */
export type MarginTenor = "daily" | "weekly" | "monthly" | "on_demand";
/**
 * ISO 8601 calendar date encoded as a `YYYY-MM-DD` JSON string.
 */
export type DateWire = string;
/**
 * DRC asset type.
 */
export type DrcAssetType = "corporate" | "sovereign" | "local_government" | "securitization" | "equity";
/**
 * DRC sector classification.
 */
export type DrcSector = "sovereign" | "corporate" | "local_government";
/**
 * DRC seniority for LGD assignment.
 */
export type DrcSeniority = "covered_bond" | "senior_unsecured" | "subordinated" | "equity" | "securitization";
/**
 * Rounding modes supported by the library.
 *
 * The variants mirror the most common conventions found in pricing engines.
 *
 * # Examples
 * ```rust
 * use finstack_quant_core::config::{FinstackConfig, RoundingMode};
 *
 * let mut cfg = FinstackConfig::default();
 * cfg.rounding.mode = RoundingMode::TowardZero;
 * assert!(matches!(cfg.rounding.mode, RoundingMode::TowardZero));
 * ```
 */
export type RoundingMode = "bankers" | "away_from_zero" | "toward_zero" | "floor" | "ceil";
/**
 * Finite JSON number that is strictly greater than zero.
 *
 * This type is used by serde field adapters so runtime deserialization and
 * generated schemas enforce the same positive-number contract.
 */
export type PositiveF64Wire = number;
/**
 * FRTB risk classes per BCBS d457.
 */
export type FrtbRiskClass = "girr" | "csr_non_sec" | "csr_sec_ctp" | "csr_sec_non_ctp" | "equity" | "commodity" | "fx";
/**
 * Deterministic decay applied to today's SIMM IM to approximate `E[IM(t)]`.
 */
export type ImDecayProfile =
  | "constant"
  | {
      linear_to_maturity: {
        /**
         * Portfolio maturity `T` in years (must be positive and finite).
         */
        maturity_years: number;
      };
    }
  | {
      sqrt_time: {
        /**
         * Portfolio maturity `T` in years (must be positive and finite).
         */
        maturity_years: number;
      };
    };
/**
 * Type of margin call.
 *
 * Classifies the nature of a margin call for proper processing
 * and accounting treatment.
 */
export type MarginCallType =
  "initial_margin" | "variation_margin_post" | "variation_margin_collect" | "top_up" | "substitution";
/**
 * OTC derivative margin specifications including CSA terms, thresholds, and collateral eligibility. Covers ISDA CSA, BCBS-IOSCO regulatory margin, and CCP clearing requirements.
 */
export type MarginEnvelope =
  | {
      /**
       * OTC margin specification payload.
       */
      otc_margin_spec: OtcMarginSpec;
      /**
       * Required contract marker.
       */
      schema: MarginSchema;
    }
  | {
      /**
       * CSA specification payload.
       */
      csa_spec: CsaSpec;
      /**
       * Required contract marker.
       */
      schema: MarginSchema;
    }
  | {
      /**
       * Margin call payload.
       */
      margin_call: MarginCall;
      /**
       * Required contract marker.
       */
      schema: MarginSchema;
    };
/**
 * Explicit ISDA SIMM credit risk-class and bucket assignment.
 *
 * Corporate, sovereign, and index credit exposures belong to credit
 * qualifying, including high-yield sectors represented by SIMM buckets 7-12.
 * Credit non-qualifying is reserved for securitizations and other exposures
 * governed by the non-qualifying risk class.
 */
export type SimmCreditClassification =
  | {
      risk_class: "qualifying";
      /**
       * Sector bucket used for credit-qualifying delta aggregation.
       */
      sector: SimmCreditSector;
    }
  | {
      risk_class: "non_qualifying";
    };
/**
 * SIMM credit sector for bucket assignment.
 *
 * Maps reference entities to ISDA SIMM credit qualifying buckets.
 * See ISDA SIMM v2.6 Table 2.
 */
export type SimmCreditSector =
  | "sovereign"
  | "financial"
  | "basic_materials"
  | "consumer_goods"
  | "technology_media"
  | "health_care"
  | "high_yield_sovereign"
  | "high_yield_financial"
  | "high_yield_basic_materials"
  | "high_yield_consumer_goods"
  | "high_yield_technology_media"
  | "high_yield_health_care"
  | "residual";
/**
 * Typed value of the required margin schema marker.
 */
export type MarginSchema = "finstack_quant.margin/1";
/**
 * Identifies a margin netting set.
 *
 * Instruments in the same netting set can offset each other for margin
 * calculation purposes. The netting set is typically defined by the
 * CSA agreement (bilateral) or by CCP membership (cleared) — these two
 * shapes are mutually exclusive, so the type encodes them as enum
 * variants rather than as a struct with two `Option<String>` fields
 * that could in principle both be set or both be unset.
 */
export type NettingSetId =
  | {
      /**
       * Counterparty identifier
       */
      counterparty_id: string;
      /**
       * CSA identifier
       */
      csa_id: string;
      kind: "bilateral";
    }
  | {
      /**
       * CCP identifier (also used as the counterparty id)
       */
      ccp_id: string;
      kind: "cleared";
    };
/**
 * Repo margin type.
 *
 * Different margin mechanisms offer varying levels of protection
 * and operational complexity.
 */
export type RepoMarginType = "none" | "mark_to_market" | "net_exposure" | "triparty";
/**
 * SA-CCR asset class for add-on computation.
 *
 * Each derivative trade is assigned to exactly one asset class.
 * The add-on formula and supervisory parameters differ by class.
 */
export type SaCcrAssetClass = "interest_rate" | "foreign_exchange" | "credit" | "equity" | "commodity";
/**
 * SA-CCR option type for delta computation.
 */
export type SaCcrOptionType = "call_long" | "call_short" | "put_long" | "put_short";
/**
 * Supervisory classification required for credit, equity and commodity trades.
 * Parameters are prescribed by CRE52.72; this is independent of hedging-set identity.
 */
export type SaCcrSupervisoryCategory =
  | "credit_aa"
  | "credit_a"
  | "credit_bbb"
  | "credit_bb"
  | "credit_b"
  | "credit_ccc"
  | "credit_index_ig"
  | "credit_index_hy"
  | "equity_single_name"
  | "equity_index"
  | "commodity_electricity"
  | "commodity_other";
/**
 * Asset class for schedule-based IM calculation.
 */
export type ScheduleAssetClass = BuiltInScheduleAssetClass | string;
/**
 * Risk classes for SIMM categorization.
 */
export type SimmRiskClass =
  "interest_rate" | "credit_qualifying" | "credit_non_qualifying" | "equity" | "commodity" | "fx";
/**
 * SIMM version identifier.
 */
export type SimmVersion = "v2_6";

/**
 * Single collateral eligibility entry.
 *
 * Defines eligibility criteria and haircut for a specific type of collateral.
 */
export interface CollateralEligibility {
  /**
   * Asset class
   */
  asset_class: CollateralAssetClass;
  /**
   * Concentration limit as fraction of total collateral (optional)
   *
   * E.g., 0.30 means max 30% of collateral can be this type.
   */
  concentration_limit?: number | null;
  /**
   * Additional FX haircut for currency mismatch (decimal)
   *
   * Applied when collateral currency differs from settlement currency.
   */
  fx_haircut_addon?: number;
  /**
   * Haircut as decimal (e.g., 0.02 = 2%)
   */
  haircut: number;
  /**
   * Remaining maturity constraints
   */
  maturity_constraints?: MaturityConstraints | null;
  /**
   * Minimum credit rating requirement (e.g., "A-", "BBB")
   *
   * If None, no rating constraint applies.
   */
  min_rating?: string | null;
}
/**
 * Maturity constraints for eligible collateral.
 *
 * Some CSAs restrict collateral based on remaining maturity to limit
 * duration risk in the collateral portfolio.
 */
export interface MaturityConstraints {
  /**
   * Maximum remaining years to maturity (if any)
   */
  max_remaining_years?: number | null;
  /**
   * Minimum remaining years to maturity (if any)
   */
  min_remaining_years?: number | null;
}
/**
 * A breach of a collateral concentration limit.
 */
export interface ConcentrationBreach {
  /**
   * Asset class that breached
   */
  asset_class: CollateralAssetClass;
  /**
   * Excess fraction above limit
   */
  excess: number;
  /**
   * Actual fraction of total collateral
   */
  fraction: number;
  /**
   * Allowed concentration limit
   */
  limit: number;
}
/**
 * Credit Support Annex specification (ISDA standard).
 *
 * The CSA governs the exchange of collateral between counterparties
 * for OTC derivatives. This specification captures all key commercial
 * terms needed for margin calculation and management.
 *
 * # ISDA Documentation
 *
 * This type represents terms from:
 * - ISDA 2016 Credit Support Annex for Variation Margin (VM CSA)
 * - ISDA 2018 Credit Support Annex for Initial Margin (IM CSA)
 *
 * # References
 *
 * - ISDA 2016 VM CSA: `docs/REFERENCES.md#isda-vm-csa-2016`
 * - ISDA 2018 IM CSA: `docs/REFERENCES.md#isda-im-csa-2018`
 * - BCBS-IOSCO uncleared margin framework: `docs/REFERENCES.md#bcbs-iosco-uncleared-margin`
 *
 * # Example
 *
 * ```
 * use finstack_quant_margin::{
 *     CsaSpec, VmParameters, ImParameters, EligibleCollateralSchedule,
 *     MarginCallTiming, ImMethodology, MarginTenor,
 * };
 * use finstack_quant_core::currency::Currency;
 * use finstack_quant_core::money::Money;
 *
 * # fn main() -> finstack_quant_core::Result<()> {
 * let csa = CsaSpec {
 *     id: "USD-CSA-2024".to_string(),
 *     base_currency: Currency::USD,
 *     vm_params: VmParameters::regulatory_standard(Currency::USD)?,
 *     im_params: Some(ImParameters::simm_standard(Currency::USD)?),
 *     eligible_collateral: EligibleCollateralSchedule::bcbs_standard()?,
 *     call_timing: MarginCallTiming::regulatory_standard()?,
 *     collateral_curve_id: "USD-OIS".into(),
 *     calendar_id: "usny".into(),
 * };
 * # Ok(())
 * # }
 * ```
 */
export interface CsaSpec {
  /**
   * Base currency for margin calculations.
   *
   * All exposures and collateral values are converted to this currency
   * for netting and comparison with thresholds.
   */
  base_currency: Currency;
  /**
   * Contractual business-day calendar for calls and settlements.
   */
  calendar_id: string;
  /**
   * Margin call timing parameters.
   */
  call_timing: MarginCallTiming;
  /**
   * Discount curve ID for collateral valuation.
   *
   * Cash collateral is typically discounted at OIS/RFR rates.
   * This curve should match the CSA's collateral interest rate.
   */
  collateral_curve_id: Id;
  /**
   * Eligible collateral schedule.
   *
   * Defines what collateral types are acceptable and associated haircuts.
   */
  eligible_collateral: EligibleCollateralSchedule;
  /**
   * CSA identifier (e.g., "USD-CSA-STANDARD", "COUNTERPARTY-XYZ-CSA")
   */
  id: string;
  /**
   * Initial margin parameters (optional).
   *
   * If None, no IM is exchanged (either not in scope for regulations
   * or trade is cleared).
   */
  im_params?: ImParameters | null;
  /**
   * Variation margin parameters.
   *
   * Governs daily mark-to-market collateral exchange.
   */
  vm_params: VmParameters;
}
/**
 * Margin call timing parameters.
 *
 * Specifies the operational timing for margin calls including
 * notification and dispute resolution windows.
 */
export interface MarginCallTiming {
  /**
   * Grace period for collateral delivery (business days)
   */
  delivery_grace_days: number;
  /**
   * Dispute resolution window (business days)
   */
  dispute_resolution_days: number;
  /**
   * Notification deadline (hours after valuation, e.g., 13:00 local time)
   */
  notification_deadline_hours: number;
  /**
   * Response deadline (hours after notification)
   */
  response_deadline_hours: number;
}
/**
 * Eligible collateral schedule with haircuts.
 *
 * Defines the complete set of collateral types accepted under a CSA
 * or margin agreement, along with associated haircuts and constraints.
 *
 * # Example
 *
 * ```
 * use finstack_quant_margin::{CollateralEligibility, EligibleCollateralSchedule};
 *
 * // Start from a standard schedule (BCBS-IOSCO compliant)
 * let schedule = EligibleCollateralSchedule::bcbs_standard()?;
 * # let _ = schedule;
 * # Ok::<(), finstack_quant_core::Error>(())
 * ```
 */
export interface EligibleCollateralSchedule {
  /**
   * Default haircut for unlisted collateral (if accepted)
   *
   * If None, only explicitly listed collateral types are accepted.
   */
  default_haircut?: number | null;
  /**
   * List of eligible collateral types with haircuts
   */
  eligible: CollateralEligibility[];
  /**
   * Whether rehypothecation of posted collateral is permitted
   *
   * For IM under BCBS-IOSCO rules, rehypothecation is prohibited.
   * For VM, rehypothecation may be permitted by bilateral agreement.
   */
  rehypothecation_allowed?: boolean;
}
/**
 * Initial margin parameters.
 *
 * Initial margin is collateral posted to cover potential future exposure (PFE)
 * during the close-out period following a default. IM is required for
 * non-centrally cleared derivatives under BCBS-IOSCO rules.
 *
 * # Margin Period of Risk (MPOR)
 *
 * The MPOR determines the horizon over which PFE is calculated:
 * - Standard: 10 business days for bilateral derivatives
 * - Reduced: 5 days for certain liquid products
 *
 * # Example
 *
 * ```
 * use finstack_quant_margin::{ImMethodology, ImParameters};
 * use finstack_quant_core::currency::Currency;
 * use finstack_quant_core::money::Money;
 *
 * let im_params = ImParameters {
 *     methodology: ImMethodology::Simm,
 *     mpor_days: 10,
 *     threshold: Money::from((50_000_000_i64, Currency::USD)),
 *     mta: Money::from((0_i64, Currency::USD)), // Combined with VM MTA
 *     segregated: true,
 * };
 * ```
 */
export interface ImParameters {
  /**
   * IM calculation methodology.
   *
   * Options include SIMM, regulatory schedule, or CCP methodology.
   */
  methodology: ImMethodology;
  /**
   * Margin Period of Risk in business days.
   *
   * Standard is 10 days under BCBS-IOSCO. CCPs may use shorter periods.
   */
  mpor_days: number;
  /**
   * Minimum Transfer Amount for IM.
   *
   * Combined IM+VM MTA must not exceed €500,000 under BCBS-IOSCO.
   */
  mta: Money;
  /**
   * Whether IM must be held in a segregated account.
   *
   * Under BCBS-IOSCO, IM must be segregated with a third-party custodian
   * to protect it in case of the collecting party's insolvency.
   */
  segregated: boolean;
  /**
   * Group-level IM threshold allocated to this CSA by the caller.
   *
   * BCBS-IOSCO permits €50M aggregate threshold at group level.
   * Many large dealers operate with zero threshold by agreement.
   */
  threshold: Money;
}
/**
 * Currency-tagged monetary amount with safe arithmetic.
 *
 * Values retain decimal precision independently of ISO 4217 display precision.
 *
 * When you need configurable rounding during ingestion, use
 * [`Money::new_with_config`].
 *
 * # Examples
 * ```rust
 * use finstack_quant_core::money::Money;
 * use finstack_quant_core::currency::Currency;
 *
 * let notional = Money::from((1_000_000_i64, Currency::EUR));
 * assert_eq!(notional.currency(), Currency::EUR);
 * assert_eq!(notional.amount(), 1_000_000.0);
 * ```
 */
export interface Money {
  /**
   * Monetary amount, carried on the wire as an exact decimal string rather
   * than a JSON number so no precision is lost in transit. Construction with
   * configuration applies the selected ingest scale; raw construction does not.
   */
  amount: DecimalWire;
  /**
   * ISO 4217 currency of `amount`. Arithmetic between two `Money` values
   * requires this to match; there is no implicit conversion.
   */
  currency: Currency;
}
/**
 * Variation margin parameters.
 *
 * These parameters govern the daily (or periodic) exchange of variation margin
 * under a CSA agreement. VM is exchanged to eliminate mark-to-market exposure.
 *
 * # ISDA CSA Standard Terms
 *
 * The 2016 VM CSA introduced standardized terms for variation margin:
 * - Zero threshold for in-scope entities
 * - Daily exchange with T+1 settlement
 * - Cash or highly liquid securities as collateral
 *
 * # Example
 *
 * ```
 * use finstack_quant_margin::{MarginTenor, VmParameters};
 * use finstack_quant_core::currency::Currency;
 * use finstack_quant_core::money::Money;
 *
 * let vm_params = VmParameters {
 *     threshold: Money::from((10_000_000_i64, Currency::USD)),
 *     mta: Money::from((500_000_i64, Currency::USD)),
 *     rounding: Money::from((10_000_i64, Currency::USD)),
 *     independent_amount: Money::from((0_i64, Currency::USD)),
 *     frequency: MarginTenor::Daily,
 *     settlement_lag: 1,
 * };
 * ```
 */
export interface VmParameters {
  /**
   * Margin call frequency.
   *
   * Under BCBS-IOSCO, daily margin exchange is required.
   */
  frequency: MarginTenor;
  /**
   * Independent Amount (IA) / Additional Margin.
   *
   * Fixed collateral amount required regardless of exposure.
   * Often used for credit enhancement or as a buffer.
   */
  independent_amount: Money;
  /**
   * Minimum Transfer Amount (MTA).
   *
   * Margin calls below MTA are not made. BCBS-IOSCO permits combined
   * IM+VM MTA up to €500,000 equivalent.
   */
  mta: Money;
  /**
   * Rounding increment for margin amounts.
   *
   * Margin calls are typically rounded to the nearest multiple of this amount.
   */
  rounding: Money;
  /**
   * Settlement lag in business days (T+n).
   *
   * Standard is T+1 for VM under 2016 VM CSA.
   */
  settlement_lag: number;
  /**
   * Threshold amount below which no margin is exchanged.
   *
   * Under BCBS-IOSCO rules for covered entities, VM threshold must be zero.
   * Legacy bilateral CSAs may have non-zero thresholds.
   */
  threshold: Money;
}
/**
 * A position subject to the Default Risk Charge.
 *
 * Per MAR22.9, gross JTD for a position is:
 *
 * ```text
 * long:  gross = max(LGD * notional + P&L, 0)
 * short: gross = min(LGD * notional + P&L, 0)
 * ```
 *
 * where the `P&L` term captures any mark-to-market adjustment already
 * reflected in the trading-book valuation (e.g. an underwater long bond
 * has a small negative `P&L` that reduces the exposed JTD). `jtd_amount`
 * represents the signed *notional* (positive = long, negative = short);
 * [`drc_charge`](super::drc::drc_charge) multiplies by [`DrcSeniority`]
 * LGD and then applies the `pnl_adjustment` and the sign-preserving floor.
 */
export interface DrcPosition {
  /**
   * Asset sub-type: corporate, sovereign, local government, or equity.
   * Securitizations are rejected because they require a separate DRC model.
   */
  asset_type: DrcAssetType;
  /**
   * Issuer identifier.
   */
  issuer: string;
  /**
   * Signed JTD *notional* (positive = long, negative = short). Does
   * **not** include the LGD multiplier — [`super::drc::drc_charge`] applies LGD.
   */
  jtd_amount: number;
  /**
   * Residual contractual maturity in years; scaled into [0.25, 1.0] for JTD.
   * Cash equities use the elected three-month or greater-than-one-year horizon.
   */
  maturity_years: number;
  /**
   * Mark-to-market / P&L adjustment from MAR22.9. Default 0. Add a
   * negative value for a long position with unrealised loss so the
   * gross JTD is correctly floored at zero when the mark-down already
   * exceeds `LGD * notional`.
   */
  pnl_adjustment?: number;
  /**
   * Credit rating bucket (1-based per FRTB specification).
   */
  rating_bucket: number;
  /**
   * Sector for DRC bucket assignment.
   */
  sector: DrcSector;
  /**
   * Seniority for LGD determination.
   */
  seniority: DrcSeniority;
  [k: string]: unknown;
}
/**
 * SA-CCR Exposure at Default result.
 */
export interface EadResult {
  /**
   * Aggregate add-on before multiplier.
   */
  add_on_aggregate: number;
  /**
   * Add-on by asset class.
   */
  add_on_by_asset_class: {
    [k: string]: number;
  };
  /**
   * Alpha multiplier (1.4 per regulation).
   */
  alpha: number;
  /**
   * Exposure at Default: `alpha * (RC + PFE)`, capped for margined sets
   * at the same netting set calculated without a margin agreement.
   */
  ead: number;
  /**
   * Maturity factor applied.
   */
  maturity_factor: number;
  /**
   * Policy metadata stamped by the computing layer: numeric mode, active
   * rounding context, any applied FX policy, and the parallel-execution
   * flag.
   */
  meta?: ResultsMeta;
  /**
   * PFE multiplier (accounts for over-collateralization).
   */
  multiplier: number;
  /**
   * Potential future exposure component.
   */
  pfe: number;
  /**
   * Replacement cost component.
   */
  rc: number;
  [k: string]: unknown;
}
/**
 * Metadata bundle that accompanies valuation outputs.
 *
 * The metadata is intentionally small so it can be attached to reports and
 * downstream data stores for reproducibility and audit trails.
 *
 * # Examples
 * ```rust
 * use finstack_quant_core::config::{results_meta, FinstackConfig, NUMERIC_MODE_F64};
 *
 * let meta = results_meta(&FinstackConfig::default());
 * assert_eq!(meta.numeric_mode, NUMERIC_MODE_F64);
 * assert!(meta.timestamp.is_none()); // deterministic by default
 * ```
 */
export interface ResultsMeta {
  /**
   * Optional FX policy applied by the computing layer (human-readable key).
   */
  fx_policy_applied?: string | null;
  /**
   * Numeric engine mode used to produce the results.
   *
   * Always [`NUMERIC_MODE_F64`] today; a plain string so result
   * envelopes stay wire-compatible across host languages.
   */
  numeric_mode: string;
  /**
   * Whether the producing computation ran in parallel.
   *
   * Serial results omit the field entirely so existing payloads and golden
   * files stay byte-identical.
   */
  parallel?: boolean;
  /**
   * Rounding context snapshot applied to IO boundaries.
   */
  rounding: RoundingContext;
  /**
   * Timestamp when result was computed (ISO 8601 format).
   * Useful for audit trails and reproducibility.
   */
  timestamp?: string | null;
  /**
   * Finstack Quant library version used to produce the result.
   */
  version?: string | null;
  [k: string]: unknown;
}
/**
 * Snapshot of active rounding settings used for result stamping.
 *
 * Instances are typically produced via [`rounding_context_from`] and persisted
 * alongside valuation results.
 */
export interface RoundingContext {
  /**
   * Ingest scale map snapshot by currency code.
   */
  ingest_scale_by_currency: {
    [k: string]: number;
  };
  /**
   * Active rounding mode.
   */
  mode: RoundingMode;
  /**
   * Output scale map snapshot by currency code.
   */
  output_scale_by_currency: {
    [k: string]: number;
  };
  /**
   * Tolerance settings snapshot for floating-point comparisons.
   */
  tolerances?: ToleranceConfig;
  /**
   * Schema version for forward compatibility.
   */
  version: number;
  [k: string]: unknown;
}
/**
 * Numerical tolerance configuration for floating-point comparisons.
 *
 * Provides configurable epsilon values for zero-checks in rate calculations
 * and generic floating-point comparisons. These defaults are chosen to balance
 * numerical stability with practical precision requirements.
 *
 * # Examples
 * ```rust
 * use finstack_quant_core::config::ToleranceConfig;
 *
 * let mut tol = ToleranceConfig::default();
 * assert_eq!(tol.rate_epsilon, 1e-12);
 *
 * // Customize for stricter rate comparisons
 * tol.rate_epsilon = 1e-14;
 * ```
 */
export interface ToleranceConfig {
  /**
   * Epsilon for generic floating-point comparisons (default: 1e-10).
   *
   * Used for general numerical comparisons where higher tolerance is acceptable.
   */
  generic_epsilon?: PositiveF64Wire;
  /**
   * Epsilon for rate comparisons (default: 1e-12).
   *
   * Used when comparing interest rates, yields, and other small ratios.
   */
  rate_epsilon?: PositiveF64Wire;
}
/**
 * Excess collateral result.
 */
export interface ExcessCollateral {
  /**
   * Collateral value
   */
  collateral_value: Money;
  /**
   * Excess amount (positive) or shortfall (negative)
   */
  excess: Money;
  /**
   * Required value
   */
  required_value: Money;
  [k: string]: unknown;
}
/**
 * Diagnostics from exposure simulation capturing data quality metrics.
 *
 * Populated by the exposure computation engine to let callers distinguish
 * genuine zero exposure from missing data.
 */
export interface ExposureDiagnostics {
  /**
   * Number of time grid points where market data could not be rolled forward.
   */
  market_roll_failures: number;
  /**
   * Total time grid points evaluated.
   */
  total_time_points: number;
  /**
   * Total number of individual instrument valuation failures across all time points.
   */
  valuation_failures: number;
}
/**
 * Exposure profile computed at each time grid point.
 */
export interface ExposureProfile {
  /**
   * Simulation quality diagnostics (populated by the exposure engine).
   */
  diagnostics?: ExposureDiagnostics | null;
  /**
   * Expected Negative Exposure at each time point: max(-V(t), 0).
   */
  ene: number[];
  /**
   * Expected Positive Exposure at each time point: max(V(t), 0).
   */
  epe: number[];
  /**
   * Portfolio mark-to-market value at each time point (may be negative).
   */
  mtm_values: number[];
  /**
   * Nonnegative time points in years from valuation date. An explicit zero
   * node supplies opening exposure; before the first node exposure is held
   * flat at that node, consistently across CVA, DVA, FVA and MVA.
   */
  times: number[];
}
/**
 * Complete FRTB SBA capital charge result.
 */
export interface FrtbSbaResult {
  /**
   * Which correlation scenario produced the binding charge for each component.
   */
  binding_scenario: CorrelationScenario;
  /**
   * Curvature risk charge by risk class.
   */
  curvature_by_risk_class: {
    [k: string]: number;
  };
  /**
   * Delta risk charge by risk class.
   */
  delta_by_risk_class: {
    [k: string]: number;
  };
  /**
   * Default Risk Charge (credit + equity).
   */
  drc: number;
  /**
   * Policy metadata stamped by the computing layer: numeric mode, active
   * rounding context, any applied FX policy, and the parallel-execution
   * flag.
   */
  meta?: ResultsMeta;
  /**
   * Residual Risk Add-On.
   */
  rrao: number;
  /**
   * Delta+Vega+Curvature charge under each scenario (for transparency).
   */
  scenario_charges: {
    [k: string]: number;
  };
  /**
   * Total capital charge (sum of all components).
   */
  total: number;
  /**
   * Vega risk charge by risk class.
   */
  vega_by_risk_class: {
    [k: string]: number;
  };
  [k: string]: unknown;
}
/**
 * FRTB sensitivity inputs organized by risk class.
 */
export interface FrtbSensitivities {
  base_currency: Currency;
  commodity_curvature: [unknown, unknown, unknown, unknown][];
  commodity_delta: [unknown, unknown, unknown, unknown, unknown][];
  commodity_vega: [unknown, unknown, unknown, unknown][];
  csr_nonsec_curvature: [unknown, unknown, unknown, unknown][];
  csr_nonsec_delta: [unknown, unknown, unknown, unknown, unknown][];
  csr_nonsec_vega: [unknown, unknown, unknown, unknown][];
  csr_sec_ctp_curvature: [unknown, unknown, unknown, unknown][];
  csr_sec_ctp_delta: [unknown, unknown, unknown, unknown, unknown][];
  csr_sec_ctp_vega: [unknown, unknown, unknown, unknown][];
  csr_sec_nonctp_curvature: [unknown, unknown, unknown, unknown][];
  csr_sec_nonctp_delta: [unknown, unknown, unknown, unknown, unknown][];
  csr_sec_nonctp_vega: [unknown, unknown, unknown, unknown][];
  drc_positions: DrcPosition[];
  equity_curvature: [unknown, unknown, unknown, unknown][];
  equity_delta: [unknown, unknown, unknown][];
  equity_repo_delta: [unknown, unknown, unknown][];
  equity_vega: [unknown, unknown, unknown, unknown][];
  fx_curvature: [unknown, unknown, unknown, unknown][];
  fx_delta: [unknown, unknown, unknown][];
  fx_vega: [unknown, unknown, unknown, unknown][];
  girr_curvature: [unknown, unknown, unknown][];
  girr_delta: [unknown, unknown, unknown][];
  girr_inflation_delta: [unknown, unknown][];
  girr_vega: [unknown, unknown, unknown, unknown][];
  girr_xccy_basis_delta: [unknown, unknown][];
  rrao_exotic_notionals: RraoPosition[];
}
/**
 * A position subject to the Residual Risk Add-On.
 *
 * RRAO applies to exotic instruments whose risks are not adequately
 * captured by the delta/vega/curvature framework -- instruments with
 * gap risk, correlation risk, or behavioral risk.
 */
export interface RraoPosition {
  /**
   * Instrument identifier.
   */
  instrument_id: string;
  /**
   * Whether the instrument bears exotic underlying risk (1.0% weight)
   * or other residual risk (0.1% weight).
   */
  is_exotic: boolean;
  /**
   * Gross notional amount.
   */
  notional: number;
  [k: string]: unknown;
}
/**
 * Funding cost/benefit configuration for FVA and MVA calculation.
 */
export interface FundingConfig {
  /**
   * Funding benefit spread in basis points (benefit on negative exposure).
   *
   * If `None`, symmetric funding is assumed: `funding_benefit = funding_spread`.
   * In practice, the benefit spread may be lower than the cost spread
   * due to asymmetric funding conditions.
   */
  funding_benefit_bp?: number | null;
  /**
   * Funding spread in basis points (cost on positive exposure).
   *
   * This is the spread over the risk-free rate that the institution
   * pays to fund positive (out-of-the-money to counterparty) exposure.
   * Typical values: 20–100 bp depending on the institution's credit quality.
   */
  funding_spread_bp: number;
  /**
   * Expected initial-margin profile `E[IM(t)]` that drives MVA.
   *
   * When `Some`, [`crate::xva::cva::compute_bilateral_xva`] prices the
   * lifetime funding cost of posting this IM and reports it as
   * [`XvaResult::mva`]. When `None`, MVA is not computed and
   * [`XvaResult::mva`] is `None`.
   *
   * Build the profile with
   * [`crate::xva::mva::im_profile_from_simm`] (deterministic SIMM decay), or
   * supply the path-consistent mean IM from your own simulation.
   */
  im_profile?: ImProfile | null;
  /**
   * Spread in basis points applied to posted initial margin (MVA).
   *
   * If `None`, MVA uses `funding_spread_bp` — the desk's unsecured
   * funding spread, which is the standard assumption (Green 2015, ch. 10).
   * Override when IM is funded at a different (typically term or partially
   * secured) level than uncollateralized derivative exposure.
   */
  margin_funding_spread_bp?: number | null;
}
/**
 * Expected initial-margin profile `E[IM(t)]` on a time grid.
 *
 * Values are in the aggregation currency chosen when the profile was built
 * (e.g. the `currency` argument of [`im_profile_from_simm`]).
 */
export interface ImProfile {
  /**
   * Expected IM at each time point (non-negative, finite).
   */
  im_values: number[];
  /**
   * Time points in years from the valuation date (strictly increasing, positive).
   */
  times: number[];
}
/**
 * Haircut sensitivity (Haircut01) result.
 */
export interface Haircut01 {
  /**
   * Collateral value
   */
  collateral_value: Money;
  /**
   * Current haircut (decimal)
   */
  current_haircut: number;
  /**
   * PV change for +1bp haircut
   */
  pv_change: Money;
  [k: string]: unknown;
}
/**
 * One-way IM collateral account after applying the CSA's allocated threshold.
 */
export interface ImCollateralResult {
  /**
   * Existing nonnegative collateral balance in this one-way IM account.
   */
  current_collateral: Money;
  /**
   * Gross model IM after MPOR adjustment, before contractual thresholds.
   */
  gross_initial_margin: Money;
  /**
   * Target collateral balance, max(gross IM minus allocated CSA threshold, 0).
   */
  required_collateral: Money;
  /**
   * True requires separate custody with no reuse to satisfy VM obligations.
   */
  segregated: boolean;
  /**
   * Signed transfer: positive posts additional IM, negative returns excess.
   * Absolute transfers strictly below MTA are zero; equality triggers transfer.
   */
  transfer: Money;
}
/**
 * Initial margin calculation result.
 */
export interface ImResult {
  /**
   * Calculated initial margin amount
   */
  amount: Money;
  /**
   * Whether the amount is an approximation rather than
   * an exact computation under the named methodology.
   *
   * Approximations need not be conservative. Historical SIMM sets this flag
   * because its input dimensions and some aggregation stages are simplified.
   * Also set by the clearing-house and internal-model calculators when
   * they fall back to `|exposure_base| x conservative_rate` because no
   * [`ExternalImSource`](crate::calculators::im::ExternalImSource) supplied
   * a real margin amount. Portfolio-level consumers should surface this
   * flag: an approximated IM is suitable for indicative funding/capacity
   * analysis, not for reconciling actual CCP margin calls.
   */
  approximation: boolean;
  /**
   * Calculation date
   */
  as_of: string;
  /**
   * Breakdown by risk class (if available)
   *
   * Keys are methodology-specific component labels. SIMM publishes
   * `IR_Delta`, `IR_Vega`, `Credit_Qualifying_Delta`,
   * `Credit_Qualifying_Vega`, `Credit_NonQualifying_Delta`,
   * `Credit_NonQualifying_Vega`, `Equity_Delta`, `Equity_Vega`, `FX_Delta`,
   * `FX_Vega`, `Commodity_Delta`, `Commodity_Vega` and `Curvature`; the
   * schedule calculator publishes the normalised asset class (for example
   * `interest_rate`). Values are IM amounts for that component.
   */
  breakdown: {
    [k: string]: Money;
  };
  /**
   * Methodology used for calculation
   */
  methodology: ImMethodology;
  /**
   * Margin Period of Risk in business days used in calculation
   */
  mpor_days: number;
}
/**
 * Margin call event.
 *
 * Represents a single margin call with all relevant details for
 * processing and settlement.
 */
export interface MarginCall {
  /**
   * Nonnegative transfer amount; call_type specifies the desk cashflow direction.
   */
  amount: Money;
  /**
   * Date the margin call is issued
   */
  call_date: string;
  /**
   * Type of margin call
   */
  call_type: MarginCallType;
  /**
   * Specific collateral type requested (if applicable)
   */
  collateral_type?: CollateralAssetClass | null;
  /**
   * MTA applied (may have reduced the call amount)
   */
  mta_applied: Money;
  /**
   * Mark-to-market value that triggered the call
   */
  mtm_trigger: Money;
  /**
   * Settlement date for the margin transfer
   */
  settlement_date: string;
  /**
   * Threshold in effect at time of call
   */
  threshold: Money;
}
/**
 * Margin constants a host needs to interpret inputs and results.
 */
export interface MarginConstants {
  /**
   * Registry id of the BCBS-IOSCO regulatory IM schedule.
   */
  BCBS_IOSCO_SCHEDULE_ID: string;
  /**
   * [`CALENDAR_DAYS_PER_YEAR`]: days per year for ACT/365 Fixed year fractions.
   */
  CALENDAR_DAYS_PER_YEAR: number;
  /**
   * [`DURATION_APPROXIMATION_FACTOR`]: modified duration per year to maturity.
   */
  DURATION_APPROXIMATION_FACTOR: number;
  /**
   * Margin period of risk, in business days, stamped on haircut-based IM results.
   */
  HAIRCUT_MPOR_DAYS: number;
  /**
   * [`ONE_BP`]: one basis point as a decimal.
   */
  ONE_BP: number;
  /**
   * Number of SIMM commodity buckets.
   */
  SIMM_COMMODITY_BUCKET_COUNT: number;
  /**
   * SIMM tenor bucket labels accepted by the sensitivity containers, shortest first.
   */
  SIMM_TENORS: string[];
  /**
   * [`STANDARD_CDS_MATURITY_YEARS`]: CDS tenor used for SIMM bucketing, in years.
   */
  STANDARD_CDS_MATURITY_YEARS: number;
  /**
   * SIMM tenor bucket boundaries in years, keyed by the [`tenor_buckets`] constant names.
   */
  tenor_buckets: TenorBucketYears;
}
/**
 * SIMM tenor bucket boundaries in years (the [`tenor_buckets`] constants).
 */
export interface TenorBucketYears {
  /**
   * 10 year bucket threshold.
   */
  BUCKET_10Y: number;
  /**
   * 15 year bucket threshold.
   */
  BUCKET_15Y: number;
  /**
   * 1 year bucket threshold.
   */
  BUCKET_1Y: number;
  /**
   * 20 year bucket threshold.
   */
  BUCKET_20Y: number;
  /**
   * 2 year bucket threshold.
   */
  BUCKET_2Y: number;
  /**
   * 3 month bucket threshold.
   */
  BUCKET_3M: number;
  /**
   * 3 year bucket threshold.
   */
  BUCKET_3Y: number;
  /**
   * 5 year bucket threshold.
   */
  BUCKET_5Y: number;
  /**
   * 6 month bucket threshold.
   */
  BUCKET_6M: number;
}
/**
 * OTC derivative margin specification (ISDA CSA compliant).
 *
 * This is the standard margin specification for bilateral and cleared
 * OTC derivatives. It combines CSA terms with clearing-specific parameters.
 *
 * # Usage
 *
 * Attach this to any OTC derivative instrument that requires margining:
 * - Interest Rate Swaps (IRS)
 * - Credit Default Swaps (CDS)
 * - CDS Indices
 * - Total Return Swaps (TRS)
 *
 * # Example
 *
 * ```
 * use finstack_quant_margin::{
 *     OtcMarginSpec, CsaSpec, SimmCreditClassification, SimmCreditSector,
 * };
 *
 * # fn main() -> finstack_quant_core::Result<()> {
 * // Bilateral (uncleared) derivative
 * let bilateral_spec = OtcMarginSpec::bilateral_simm(CsaSpec::usd_regulatory()?);
 * let credit_spec = bilateral_spec.with_simm_credit_classification(
 *     SimmCreditClassification::Qualifying {
 *         sector: SimmCreditSector::Financial,
 *     },
 * );
 *
 * // Cleared derivative
 * let cleared_spec = OtcMarginSpec::cleared("LCH", finstack_quant_core::currency::Currency::USD)?;
 * # Ok(())
 * # }
 * ```
 */
export interface OtcMarginSpec {
  /**
   * Clearing status: bilateral or cleared through CCP
   */
  clearing_status: ClearingStatus;
  /**
   * Full CSA specification (for bilateral trades)
   *
   * For cleared trades, this represents the terms with the CCP.
   */
  csa: CsaSpec;
  /**
   * Initial margin calculation methodology
   *
   * - Bilateral: SIMM or Schedule
   * - Cleared: ClearingHouse (CCP-specific)
   */
  im_methodology: ImMethodology;
  /**
   * Settlement lag for margin transfers (business days)
   */
  settlement_lag: number;
  /**
   * Explicit SIMM credit classification for credit-sensitive instruments.
   *
   * Required when a credit product uses `ImMethodology::Simm`; leave `None`
   * for non-credit instruments and non-SIMM margin methodologies.
   */
  simm_credit_classification?: SimmCreditClassification | null;
  /**
   * Variation margin exchange frequency
   */
  vm_frequency: MarginTenor;
}
/**
 * Margin funding cost result.
 */
export interface MarginFundingCost {
  /**
   * Net funding cost (annualized)
   */
  annual_cost: Money;
  /**
   * Collateral return rate (e.g., Fed Funds)
   */
  collateral_rate: number;
  /**
   * Funding rate (annualized)
   */
  funding_rate: number;
  /**
   * Posted margin amount
   */
  margin_posted: Money;
  [k: string]: unknown;
}
/**
 * Margin utilization result.
 */
export interface MarginUtilization {
  posted: Money;
  required: Money;
}
/**
 * Result of an MVA computation.
 */
export interface MvaResult {
  /**
   * Time-weighted average IM over the profile horizon:
   * `(1/T) ∫₀ᵀ IM(t) dt` under the same trapezoid convention as `mva`.
   */
  average_im: number;
  /**
   * Echo of the IM profile used: `(time, IM(t))` pairs.
   */
  im_profile: [unknown, unknown][];
  /**
   * MVA (positive = lifetime funding cost of posting IM).
   */
  mva: number;
}
/**
 * GMRA 2011 compliant repo margin specification.
 */
export interface RepoMarginSpec {
  /**
   * Tenor of margin valuation and calls.
   */
  call_frequency: MarginTenor;
  /**
   * Eligible collateral for substitution (if allowed).
   */
  eligible_substitutes?: EligibleCollateralSchedule | null;
  /**
   * Percentage deviation that triggers a margin call.
   *
   * E.g., 0.01 = 1% deviation from margin ratio triggers a call.
   * If the current ratio falls below `margin_ratio * (1 - threshold)`,
   * a margin call is generated.
   */
  margin_call_threshold: number;
  /**
   * Margin interest rate (if applicable).
   *
   * Typically tied to overnight rates (Fed Funds, SONIA, ESTR).
   */
  margin_interest_rate?: number | null;
  /**
   * Margin ratio (e.g., 1.02 = 102% collateralization required).
   *
   * GMRA typically expresses this as the ratio of Market Value
   * of Securities to Purchase Price.
   */
  margin_ratio: number;
  /**
   * Type of margin mechanism.
   */
  margin_type: RepoMarginType;
  /**
   * Whether margin interest is paid on cash margin transfers.
   *
   * Under GMRA, the parties may agree to pay interest on
   * cash margin transfers.
   */
  pays_margin_interest: boolean;
  /**
   * Settlement lag for margin transfers (business days).
   *
   * GMRA standard is typically same-day (0) or next-day (1).
   */
  settlement_lag: number;
  /**
   * Whether collateral substitution is permitted.
   *
   * GMRA Paragraph 8 governs substitution rights.
   */
  substitution_allowed: boolean;
}
/**
 * Netting set configuration for SA-CCR.
 */
export interface SaCcrNettingSetConfig {
  /**
   * Valuation date used for forward-start and remaining-maturity calculations.
   */
  as_of: DateWire;
  /**
   * Net current collateral held (positive = bank holds collateral).
   */
  collateral: number;
  /**
   * Whether the netting set is subject to a margin agreement.
   */
  is_margined: boolean;
  /**
   * Margin period of risk in business days (default: 10 for bilateral).
   */
  mpor_days: number;
  /**
   * Minimum transfer amount (MTA).
   */
  mta: number;
  /**
   * Netting set identifier.
   */
  netting_set_id: NettingSetId;
  /**
   * Net independent collateral amount (NICA).
   */
  nica: number;
  /**
   * Threshold amount (TH) under the margin agreement.
   */
  threshold: number;
}
/**
 * A single derivative trade for SA-CCR EAD computation.
 */
export interface SaCcrTrade {
  /**
   * Asset class assignment.
   */
  asset_class: SaCcrAssetClass;
  /**
   * Long (+1.0) or short (-1.0) direction.
   */
  direction: number;
  /**
   * Underlying end date used for supervisory duration; linear trade maturity.
   */
  end_date: DateWire;
  /**
   * Hedging set identifier within the asset class.
   * Trades with the same hedging set can partially offset.
   */
  hedging_set: string;
  /**
   * Whether this trade is an option.
   */
  is_option: boolean;
  /**
   * Current mark-to-market value.
   */
  mtm: number;
  /**
   * Notional in reporting currency before supervisory duration for IR/credit.
   * FX, equity and commodity notionals must already use CRE52 adjusted notionals.
   */
  notional: number;
  /**
   * Option expiry for maturity-factor calculation, distinct from the
   * underlying start/end dates used for supervisory duration.
   */
  option_maturity_date?: DateWire | null;
  /**
   * Option exercise type if applicable.
   */
  option_type?: SaCcrOptionType | null;
  /**
   * Underlying start date used for IR/credit supervisory duration.
   */
  start_date: DateWire;
  /**
   * Explicit supervisory category; required outside IR and FX.
   */
  supervisory_category?: SaCcrSupervisoryCategory | null;
  /**
   * Supervisory delta adjustment.
   * For linear trades: +1 (long) or -1 (short).
   * For options: delta from Black-Scholes or equivalent.
   */
  supervisory_delta: number;
  /**
   * Unique trade identifier.
   */
  trade_id: string;
  /**
   * Underlier reference (e.g., currency pair, issuer, equity name, commodity).
   */
  underlier: string;
}
/**
 * One volatility-weighted vega input before SIMM curvature scaling.
 *
 * The amount is `sigma * dPV/dsigma` in the sensitivity container's base
 * currency, before HVR, vega risk weights or concentration. For equity, FX
 * and commodity, `sigma` is the paragraph 10(b) prescribed volatility proxy;
 * for rates and credit it is the matching quoted ATM volatility. Preserve
 * separate expiries until `SF(t) = 0.5 * min(1, 14/t_days)` has been applied.
 */
export interface SimmCurvatureSensitivity {
  /**
   * Currency code for IR, credit sector for qualifying credit, numeric
   * commodity bucket, `fx` for FX, or `residual` for equity/non-qualifying credit.
   */
  bucket: string;
  /**
   * Option-expiry tenor from [`SIMM_TENORS`]. Two weeks means 14 calendar
   * days; months use 365/12 days and years use 365 days.
   */
  expiry_tenor: string;
  /**
   * IR subcurve name, credit issuer, equity underlier, commodity name,
   * or an FX currency pair such as `EUR/USD`; must be nonempty.
   */
  factor: string;
  /**
   * SIMM risk class selecting the prescribed bucket and correlation rules.
   */
  risk_class: SimmRiskClass;
  /**
   * IR or credit risk-factor tenor. Required for IR and credit; absent for
   * equity, commodity and FX. This is distinct from the option expiry.
   */
  risk_tenor?: string | null;
  /**
   * Signed base-currency `sigma * dPV/dsigma` before SF, HVR, VRW or concentration.
   */
  volatility_weighted_vega: number;
}
/**
 * SIMM sensitivity inputs organized by risk class.
 */
export interface SimmSensitivities {
  /**
   * Base currency for the sensitivities.
   *
   * This is the currency context in which the sensitivity set was produced.
   * It does not force the output currency of the eventual margin result.
   */
  base_currency: Currency;
  /**
   * Commodity delta P&L per 1% relative price increase by bucket.
   *
   * Bucket labels should match the SIMM commodity bucket naming expected by
   * the calculator's registry-backed lookup table.
   */
  commodity_delta: {
    [k: string]: number;
  };
  /**
   * Commodity vega by bucket.
   *
   * Bucket labels follow the same SIMM commodity bucket naming as
   * [`commodity_delta`](Self::commodity_delta); the single commodity vega
   * risk weight replaces the per-bucket delta weights.
   */
  commodity_vega: {
    [k: string]: number;
  };
  /**
   * Credit non-qualifying delta by (issuer/index, tenor bucket).
   *
   * For securitizations and exposures explicitly classified as non-qualifying.
   */
  credit_non_qualifying_delta: {
    [k: string]: number;
  };
  /**
   * Credit non-qualifying vega by `(issuer/index, tenor bucket)`.
   *
   * Pooled like [`credit_non_qualifying_delta`](Self::credit_non_qualifying_delta)
   * and weighted by the credit-non-qualifying vega risk weight.
   */
  credit_non_qualifying_vega: {
    [k: string]: number;
  };
  /**
   * Credit qualifying delta by `(sector, issuer/index, tenor bucket)`.
   *
   * Sector assignment is mandatory so the calculator can apply ISDA SIMM
   * intra- and inter-bucket aggregation without a scalar approximation.
   */
  credit_qualifying_delta: {
    [k: string]: number;
  };
  /**
   * Credit qualifying vega by `(sector, issuer/index, tenor bucket)`.
   *
   * Bucketed exactly like [`credit_qualifying_delta`](Self::credit_qualifying_delta)
   * so ISDA SIMM applies the same intra- and inter-bucket aggregation to the
   * vega risk class, weighted by the single credit-qualifying vega risk
   * weight rather than the per-bucket delta weights.
   */
  credit_qualifying_vega: {
    [k: string]: number;
  };
  /**
   * Expiry-resolved signed `sigma * dPV/dsigma` inputs before SF, HVR,
   * vega risk weights or concentration. Entries are retained separately
   * until expiry scaling, so opposite vegas at different expiries do not
   * incorrectly cancel curvature.
   */
  curvature: SimmCurvatureSensitivity[];
  /**
   * Equity delta by underlier.
   *
   * Values are signed currency P&L per 1% relative equity-price increase.
   */
  equity_delta: {
    [k: string]: number;
  };
  /**
   * Equity vega by underlier.
   */
  equity_vega: {
    [k: string]: number;
  };
  /**
   * FX delta by currency.
   *
   * Values are signed currency P&L per 1% relative FX-price increase,
   * before risk weighting or concentration. USD is the calculation currency.
   */
  fx_delta: {
    [k: string]: number;
  };
  /**
   * FX vega by currency pair.
   */
  fx_vega: {
    [k: string]: number;
  };
  /**
   * Interest rate delta by (currency, tenor bucket).
   *
   * Tenor buckets follow SIMM specification: 2W, 1M, 3M, 6M, 1Y, 2Y, 3Y, 5Y, 10Y, 15Y, 20Y, 30Y
   */
  ir_delta: {
    [k: string]: number;
  };
  /**
   * Interest rate vega by `(currency, tenor bucket)`.
   *
   * Values are sigma times dPV/dsigma in currency before VRW or concentration.
   * This legacy two-dimensional vega input collapses underlying-maturity detail;
   * curvature has the separate full expiry-resolved input.
   */
  ir_vega: {
    [k: string]: number;
  };
  [k: string]: unknown;
}
/**
 * JSON-friendly representation of `SimmSensitivities`.
 */
export interface SimmSensitivitiesJson {
  /**
   * Base currency for the sensitivities.
   */
  base_currency: Currency;
  /**
   * Commodity delta buckets as `(bucket, amount)`.
   */
  commodity_delta?: [unknown, unknown][];
  /**
   * Commodity vega buckets as `(bucket, amount)`.
   */
  commodity_vega?: [unknown, unknown][];
  /**
   * Credit non-qualifying delta buckets as `(name, tenor, amount)`.
   */
  credit_non_qualifying_delta?: [unknown, unknown, unknown][];
  /**
   * Credit non-qualifying vega buckets as `(name, tenor, amount)`.
   */
  credit_non_qualifying_vega?: [unknown, unknown, unknown][];
  /**
   * Credit qualifying deltas as `(sector, name, tenor, amount)`.
   */
  credit_qualifying_delta?: [unknown, unknown, unknown, unknown][];
  /**
   * Credit qualifying vegas as `(sector, name, tenor, amount)`.
   */
  credit_qualifying_vega?: [unknown, unknown, unknown, unknown][];
  /**
   * Expiry-resolved volatility-weighted vega inputs before curvature scaling.
   */
  curvature?: SimmCurvatureSensitivity[];
  /**
   * Equity delta buckets as `(underlier, amount)`.
   */
  equity_delta?: [unknown, unknown][];
  /**
   * Equity vega buckets as `(underlier, amount)`.
   */
  equity_vega?: [unknown, unknown][];
  /**
   * FX delta buckets as `(currency, amount)`.
   */
  fx_delta?: [unknown, unknown][];
  /**
   * FX vega buckets as `(ccy1, ccy2, amount)`.
   */
  fx_vega?: [unknown, unknown, unknown][];
  /**
   * Interest-rate delta buckets as `(currency, tenor, amount)`.
   */
  ir_delta?: [unknown, unknown, unknown][];
  /**
   * Interest-rate vega buckets as `(currency, tenor, amount)`.
   */
  ir_vega?: [unknown, unknown, unknown][];
}
/**
 * Variation margin calculation result.
 */
export interface VmResult {
  /**
   * Amount to collect from the counterparty, including returned collateral
   */
  collect_amount: Money;
  /**
   * Calculation date
   */
  date: string;
  /**
   * Gross mark-to-market exposure
   */
  gross_exposure: Money;
  /**
   * Net exposure after applying threshold and independent amount
   */
  net_exposure: Money;
  /**
   * Amount to post to the counterparty, including returned collateral
   */
  post_amount: Money;
  /**
   * Settlement date for the margin transfer
   */
  settlement_date: string;
}
/**
 * Result of XVA calculations.
 */
export interface XvaResult {
  /**
   * Unilateral CVA (positive = cost to the desk).
   *
   * Represents the expected loss due to counterparty default,
   * discounted to present value.
   */
  cva: number;
  /**
   * DVA (Debit Valuation Adjustment): own-default benefit.
   *
   * Positive DVA represents the expected gain to the desk from
   * the institution's own default on negative-exposure positions.
   *
   * `None` when DVA is not computed (unilateral CVA only).
   */
  dva?: number | null;
  /**
   * Time-weighted average of Effective EPE (regulatory scalar metric).
   *
   * Computed as:
   * ```text
   * Effective_EPE_avg = (1 / min(1, M)) × Σₖ Effective_EPE(tₖ) × Δtₖ
   * ```
   *
   * where `M` is the portfolio maturity and `Δtₖ = tₖ - tₖ₋₁`.
   * This is the key input for EAD under SA-CCR.
   *
   * # References
   *
   * - BCBS 279 (2014). "The standardised approach for measuring
   *   counterparty credit risk exposures." `docs/REFERENCES.md#bcbs-279-saccr`
   */
  effective_epe: number;
  /**
   * Effective EPE profile: `(time, Effective_EPE(t))`.
   *
   * Non-decreasing version of EPE, per Basel III SA-CCR:
   * `Effective_EPE(t_k) = max(Effective_EPE(t_{k-1}), EPE(t_k))`
   *
   * # References
   *
   * - BCBS 279 (2014). "The standardised approach for measuring
   *   counterparty credit risk exposures." `docs/REFERENCES.md#bcbs-279-saccr`
   */
  effective_epe_profile: [unknown, unknown][];
  /**
   * Expected Negative Exposure profile: `(time, ENE(t))`.
   *
   * ENE(t) = E[max(-V(t), 0)] — the average negative mark-to-market
   * at each time point (own-default exposure).
   */
  ene_profile: [unknown, unknown][];
  /**
   * Expected Positive Exposure profile: `(time, EPE(t))`.
   *
   * EPE(t) = E[max(V(t), 0)] — the average positive mark-to-market
   * at each time point.
   */
  epe_profile: [unknown, unknown][];
  /**
   * FVA (Funding Valuation Adjustment): net funding cost/benefit.
   *
   * Positive FVA represents a net funding cost; negative FVA
   * represents a net funding benefit. Captures the cost of
   * funding uncollateralized derivative positions.
   *
   * `None` when FVA is not computed.
   */
  fva?: number | null;
  /**
   * Maximum PFE across the profile (`max_t PFE(t)`).
   *
   * In the deterministic engine this equals `max_t EPE(t)` by
   * construction (see [`Self::pfe_profile`]). Used for coarse credit
   * limit monitoring where a Monte Carlo tail quantile is not
   * available.
   */
  max_pfe: number;
  /**
   * Policy metadata stamped by the computing layer: numeric mode, active
   * rounding context, any applied FX policy, and the parallel-execution
   * flag.
   */
  meta?: ResultsMeta;
  /**
   * MVA (Margin Valuation Adjustment): funding cost of posted initial margin.
   *
   * Positive MVA represents the lifetime cost of funding the initial
   * margin the desk posts against the netting set. Uses the same sign
   * convention as CVA and FVA.
   *
   * `None` when MVA is not computed — that is, when no
   * [`FundingConfig::im_profile`] was supplied.
   *
   * # References
   *
   * - Green, A. (2015). *XVA*. Wiley. Chapter 10. `docs/REFERENCES.md#green-xva`
   */
  mva?: number | null;
  /**
   * Potential Future Exposure profile: `(time, PFE(t))`.
   *
   * **IMPORTANT** — the deterministic CVA engine has a single path,
   * so the distribution of exposures collapses to a point mass at
   * `max(V(t), 0)`. In that degenerate case every quantile (and the
   * mean) equals `EPE(t)`, and this field holds the EPE path, not a
   * tail quantile. The name is retained so downstream systems keep
   * their column bindings; supply a profile from a Monte Carlo exposure
   * simulation when a true 97.5%-quantile PFE is required for limit
   * monitoring.
   */
  pfe_profile: [unknown, unknown][];
  /**
   * All-in valuation adjustment: `CVA − DVA + FVA + MVA`.
   *
   * Uncomputed components contribute zero. This is the quantity subtracted
   * from the risk-free value of the netting set.
   *
   * # References
   *
   * - Gregory, J. (2020). *The xVA Challenge*, 4th ed. Wiley. Chapter 14. `docs/REFERENCES.md#gregory-xva-challenge`
   * - Green, A. (2015). *XVA*. Wiley. Chapters 9-10. `docs/REFERENCES.md#green-xva`
   */
  total_xva: number;
}
