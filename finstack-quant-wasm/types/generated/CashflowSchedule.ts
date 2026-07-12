/** JSON shapes exposed by the cashflow schedule bridge. */
export interface MoneyJson {
  /** Exact decimal string; never a JS floating-point amount. */
  amount: string;
  currency: string;
}

export interface CashFlowJson {
  date: string;
  reset_date?: string | null;
  amount: MoneyJson;
  kind: string;
  accrual_factor: number;
  rate?: number | null;
  accrual?: CashFlowAccrualJson | null;
}

export interface CashFlowAccrualJson {
  start: string;
  end: string;
  day_count: string;
  projected_index_rate?: number | null;
}

export interface CashFlowMetaJson {
  representation: "contractual" | "projected" | "placeholder" | "no_residual";
  calendar_ids: string[];
  facility_limit?: MoneyJson | null;
  issue_date?: string | null;
  maturity_date?: string | null;
  accrual_periods?: Array<[string, string] | null>;
  accrual_day_counts?: Array<string | null>;
}

export interface NotionalJson {
  initial: MoneyJson;
  amort: unknown;
}

export interface CashFlowScheduleJson {
  flows: CashFlowJson[];
  notional: NotionalJson;
  day_count: string;
  meta: CashFlowMetaJson;
}

export interface DatedFlowJson {
  date: string;
  amount: MoneyJson;
}

export interface AccrualConfigJson {
  method: "linear" | "compounded";
  ex_coupon?: { days_before_coupon: number; calendar_id?: string | null } | null;
  include_pik: boolean;
  frequency?: { count: number; unit: string } | null;
}
