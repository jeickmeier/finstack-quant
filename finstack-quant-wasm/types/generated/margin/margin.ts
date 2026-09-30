// Generated from the finstack-quant-margin JSON schemas by scripts/generate-contract-types.mjs. Do not edit.

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
