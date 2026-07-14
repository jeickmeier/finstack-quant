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
}

export interface NotionalJson {
  initial: MoneyJson;
  amort: unknown;
}

export type CouponTypeJson =
  | "Cash"
  | "PIK"
  | { Split: { cash_pct: string; pik_pct: string } };

export interface RateStepJson {
  date: string;
  rate: string;
}

export type CouponLegJson =
  | { kind: "fixed"; spec: unknown }
  | { kind: "floating"; spec: unknown }
  | { kind: "step_up"; spec: unknown }
  | { kind: "fixed_window"; start: string; end: string; spec: unknown }
  | { kind: "floating_window"; start: string; end: string; spec: unknown }
  | {
      kind: "fixed_to_float";
      switch: string;
      fixed: unknown;
      floating: unknown;
      fixed_split: CouponTypeJson;
    }
  | { kind: "floating_margin_program"; steps: RateStepJson[]; base: unknown };

export type PaymentProgramJson =
  | {
      kind: "window";
      start: string;
      end: string;
      split: CouponTypeJson;
    }
  | {
      kind: "program";
      steps: Array<{ date: string; split: CouponTypeJson }>;
    };

export interface CashflowScheduleBuildSpecJson {
  notional: NotionalJson;
  issue: string;
  maturity: string;
  coupon_program?: CouponLegJson[];
  payment_program?: PaymentProgramJson[];
  fees?: unknown[];
  principal_events?: unknown[];
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
  method: "Linear" | "Compounded";
  ex_coupon?: { days_before_coupon: number; calendar_id?: string | null } | null;
  include_pik: boolean;
  frequency?: { count: number; unit: string } | null;
}
