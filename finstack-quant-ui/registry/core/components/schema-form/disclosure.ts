/** Collapsed groups, in sheet order. Common deal terms stay outside these groups. */
export const DISCLOSURE_ORDER = [
  "Schedule conventions",
  "Market links",
  "Pricing overrides",
  "Attributes",
  "Deal structure",
  "Facility terms",
  "Less common terms",
] as const;

const SCHEDULE = new Set([
  "day_count",
  "calendar_id",
  "payment_calendar_id",
  "fixing_calendar_id",
  "ex_coupon_calendar_id",
  "base_calendar_id",
  "quote_calendar_id",
  "business_day_convention",
  "payment_business_day_convention",
  "end_of_month",
  "stub",
  "payment_lag_days",
  "settlement_days",
  "settlement_delay",
  "ex_coupon_days",
  "reset_lag_days",
  "index_lag_days",
  "compounding_simple",
  "accrual_method",
  "theta_day_basis",
  "standard_imm_dates",
  "protection_start_convention",
  "delta_convention",
  "underlying_convention",
  "valuation_convention",
]);

const PRICING = new Set([
  "instrument_pricing_overrides",
  "metric_pricing_overrides",
  "scenario_pricing_overrides",
  "vol_shift",
  "vol_type",
  "vol_model",
  "path_model",
  "sabr_params",
  "margin_spec",
]);

const LESS_COMMON = new Set([
  "recovery_rate",
  "settlement",
  "discrete_dividends",
  "index_floor_bp",
  "knockout",
]);

/** Spec-root keys whose filled examples are still uncommon for a desk user. */
const SPEC_GROUPS: Readonly<Record<string, Readonly<Record<string, string>>>> =
  {
    structured_credit: {
      pool: "Deal structure",
      market_conditions: "Deal structure",
      default_spec: "Deal structure",
      prepayment_spec: "Deal structure",
      recovery_spec: "Deal structure",
      deal_metadata: "Deal structure",
      behavior_overrides: "Deal structure",
      hedge_swaps: "Deal structure",
      fees: "Deal structure",
      coverage_rules: "Deal structure",
      coverage_triggers: "Deal structure",
      credit_factors: "Deal structure",
      correlation_structure: "Deal structure",
      delinquency: "Deal structure",
      loss_allocation: "Deal structure",
      loss_recognition: "Deal structure",
      liquidation_price_pct: "Deal structure",
      card: "Deal structure",
      call_assumption: "Deal structure",
      cleanup_call_pct: "Deal structure",
      stochastic_default_spec: "Deal structure",
      stochastic_prepay_spec: "Deal structure",
      stochastic_recovery_spec: "Deal structure",
      tranche_draws: "Deal structure",
      tranche_readvance: "Deal structure",
      waterfall: "Deal structure",
      waterfall_rules: "Deal structure",
      principal_covers_senior_interest: "Deal structure",
      quote_settlement_date: "Deal structure",
    },
    revolving_credit: {
      fees: "Facility terms",
      margin_steps: "Facility terms",
      leq: "Facility terms",
      draw_repay_spec: "Facility terms",
      commitment_schedule: "Facility terms",
      scheduled_fees: "Facility terms",
      lc: "Facility terms",
      oid_eir: "Facility terms",
    },
    inflation_linked_bond: {
      lag: "Schedule conventions",
    },
    autocallable: {
      past_fixings: "Less common terms",
    },
    composite: {
      state: "Less common terms",
    },
  };

function marketLink(key: string, siblingKeys: readonly string[]): boolean {
  if (
    key.endsWith("_curve_id") ||
    key === "vol_surface_id" ||
    key === "div_yield_id"
  )
    return true;
  return (
    key === "spot_id" &&
    (siblingKeys.includes("underlying_ticker") ||
      siblingKeys.includes("ticker"))
  );
}

const PINNED = new Set(["exercise_style"]);

/** Deal term that stays on the first screen even when it matches a schema default. */
export function pinnedTerm(key: string): boolean {
  return PINNED.has(key);
}

/** Group for a filled convention, or null when the term stays on the first screen. */
export function termDisclosure(
  key: string,
  options: {
    atSpec: boolean;
    instrumentType?: string;
    siblingKeys: readonly string[];
  },
): string | null {
  if (options.atSpec && options.instrumentType) {
    const grouped = SPEC_GROUPS[options.instrumentType]?.[key];
    if (grouped) return grouped;
  }
  if (SCHEDULE.has(key)) return "Schedule conventions";
  if (marketLink(key, options.siblingKeys)) return "Market links";
  if (PRICING.has(key)) return "Pricing overrides";
  if (key === "attributes") return "Attributes";
  if (LESS_COMMON.has(key)) return "Less common terms";
  return null;
}
