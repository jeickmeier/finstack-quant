"use client";
import { useState, type ReactNode } from "react";
import type { portfolio } from "finstack-quant-wasm/types";
import type { TwrrLinkedRequest } from "@/workers/finstack-contract";
import { Button } from "@/components/ui/button";
import { labelFor } from "@/lib/finstack/format/label.mjs";
import { DecimalInput } from "@/components/finstack/core/primitives/decimal-input/decimal-input";
import { DateInput } from "@/components/finstack/core/primitives/date-input/date-input";
import {
  Outcome,
  typedNumber,
  yesNo,
} from "@/components/finstack/shared/primitives/native-display/native-display";

/** portfolio.twrrLinked as returned: cumulative, annualised, is_annualised and num_periods. */
export type LinkedReturn = portfolio.LinkedReturn;
/** One period's values and external flows, with the portfolio's signs: a contribution is positive. */
export type TwrrPeriod = portfolio.TwrrPeriod;
/** A dated flow from the investor's cash account: a contribution is negative, distributions and the terminal value positive. */
export type DatedCashflow = portfolio.DatedCashflow;
/** The host's starting data for the three forms; each form edits its own copy. Nothing is defaulted. */
export interface PortfolioReturnsInputs {
  /** Sub-period returns to link and their horizon. */
  readonly linked: TwrrLinkedRequest;
  /** One period's beginning and ending values and its flows, each weighted by the fraction of the period remaining after it. */
  readonly dietz: TwrrPeriod;
  /** Dated investor flows, in any order; Rust sorts them and nets equal dates. */
  readonly xirr: readonly DatedCashflow[];
}
/** One native result, its in-flight state and its native error. */
export interface ReturnView<T> {
  /** The native value; null while none is available. */
  value: T | null;
  loading?: boolean;
  /** The native error of the request, shown as returned. */
  error?: string;
}
export interface PortfolioReturnsProps {
  /** The host's returns, period and flows the forms start from. */
  defaultInputs: PortfolioReturnsInputs;
  /** Called with the entered returns and horizon, for portfolio.twrrLinked. */
  onSubmitLinked(input: TwrrLinkedRequest): void;
  /** Called with the entered period, for portfolio.twrrModifiedDietz. */
  onSubmitDietz(period: TwrrPeriod): void;
  /** Called with the entered flows in the order entered, for portfolio.mwrXirr. */
  onSubmitXirr(cashflows: DatedCashflow[]): void;
  /** The native twrrLinked result. */
  linked: ReturnView<LinkedReturn>;
  /** The native twrrModifiedDietz return. */
  dietz: ReturnView<number>;
  /** The native mwrXirr return. */
  xirr: ReturnView<number>;
  /** Display text of a returned decimal; omitted shows every digit. */
  formatValue?: (value: number) => string;
  density?: "compact" | "comfortable";
}
const text = (value: number) => (Number.isNaN(value) ? "" : String(value));
/** One-based row number shown beside an entry. */
// finstack-arith: display ordinal of a row, never sent to Rust
const ordinal = (index: number) => index + 1;
function Value({
  value,
  formatValue,
}: {
  value: number;
  formatValue: (value: number) => string;
}) {
  return (
    <span className="finstack-numeric" title={String(value)}>
      {formatValue(value)}
    </span>
  );
}
function Card({
  title,
  description,
  children,
}: {
  title: string;
  description: string;
  children: ReactNode;
}) {
  return (
    <section
      aria-label={title}
      className="min-w-0 space-y-4 rounded-lg border border-border bg-card p-4"
    >
      <header className="space-y-1">
        <h3 className="font-semibold">{title}</h3>
        <p className="text-xs text-muted-foreground">{description}</p>
      </header>
      {children}
    </section>
  );
}
function RemoveButton({
  label,
  onClick,
}: {
  label: string;
  onClick: () => void;
}) {
  return (
    <Button
      type="button"
      variant="outline"
      size="sm"
      aria-label={label}
      onClick={onClick}
    >
      Remove
    </Button>
  );
}
const without = <T,>(rows: readonly T[], index: number) =>
  rows.filter((_, at) => at !== index);
const replace = <T,>(rows: readonly T[], index: number, row: T) =>
  rows.map((current, at) => (at === index ? row : current));

/**
 * Three return calculations over host-supplied data, each one native call: the
 * geometric link of sub-period returns (twrrLinked), one period's Modified-Dietz
 * return (twrrModifiedDietz) and the money-weighted return of dated flows
 * (mwrXirr). Every value, unit and annualisation flag is Rust's; an input Rust
 * rejects shows its native error. The two flow forms keep their own signs.
 */
export function PortfolioReturns({
  defaultInputs,
  onSubmitLinked,
  onSubmitDietz,
  onSubmitXirr,
  linked,
  dietz,
  xirr,
  formatValue = String,
  density = "compact",
}: PortfolioReturnsProps) {
  const [returns, setReturns] = useState(() =>
    defaultInputs.linked.returns.map(text),
  );
  const [horizon, setHorizon] = useState(() =>
    text(defaultInputs.linked.horizonYears),
  );
  const [bmv, setBmv] = useState(() =>
    text(defaultInputs.dietz.beginning_market_value),
  );
  const [emv, setEmv] = useState(() =>
    text(defaultInputs.dietz.ending_market_value),
  );
  const [flows, setFlows] = useState(() =>
    (defaultInputs.dietz.cashflows ?? []).map((flow) => ({
      amount: text(flow.amount),
      fraction: text(flow.fraction_of_period_remaining),
    })),
  );
  const [dated, setDated] = useState(() =>
    defaultInputs.xirr.map((flow) => ({
      date: flow.date,
      amount: text(flow.amount),
    })),
  );
  return (
    <section
      aria-label="Portfolio returns"
      data-density={density}
      className="@container min-w-0 max-w-full space-y-4 font-sans text-sm text-foreground"
    >
      <Card
        title="Linked time-weighted return"
        description="Rust links the sub-period returns geometrically over the horizon (GIPS 2020). Returns are decimals; a horizon under one year is reported cumulatively, not annualised."
      >
        <form
          aria-label="Sub-period returns"
          className="space-y-3"
          onSubmit={(event) => {
            event.preventDefault();
            onSubmitLinked({
              returns: returns.map(typedNumber),
              horizonYears: typedNumber(horizon),
            });
          }}
        >
          <ol className="space-y-2">
            {returns.map((value, index) => (
              <li key={index} className="flex items-end gap-2">
                <div className="min-w-0 flex-1">
                  <DecimalInput
                    label={`Sub-period return ${ordinal(index)}`}
                    value={value}
                    onValueChange={(next) =>
                      setReturns(replace(returns, index, next))
                    }
                  />
                </div>
                <RemoveButton
                  label={`Remove sub-period return ${ordinal(index)}`}
                  onClick={() => setReturns(without(returns, index))}
                />
              </li>
            ))}
          </ol>
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={() => setReturns([...returns, ""])}
          >
            Add sub-period
          </Button>
          <div className="max-w-xs">
            <DecimalInput
              label="Horizon (years)"
              help="Elapsed time the returns cover, in 365-day years (whole days / 365)."
              value={horizon}
              onValueChange={setHorizon}
            />
          </div>
          <Button type="submit">Link returns</Button>
        </form>
        <Outcome
          state={{
            result: linked.value,
            loading: linked.loading,
            error: linked.error,
          }}
          working="Linking returns…"
          empty="No linked return"
        >
          {(value) => (
            <>
              <dl
                aria-label="Linked return"
                className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1"
              >
                <dt>{labelFor("cumulative")} (decimal)</dt>
                <dd>
                  <Value value={value.cumulative} formatValue={formatValue} />
                </dd>
                <dt>
                  {labelFor("annualised")} (
                  {value.is_annualised ? "decimal per year" : "decimal"})
                </dt>
                <dd>
                  <Value value={value.annualised} formatValue={formatValue} />
                </dd>
                <dt>{labelFor("is_annualised")}</dt>
                <dd>{yesNo(value.is_annualised)}</dd>
                <dt>{labelFor("num_periods")}</dt>
                <dd className="finstack-numeric">{value.num_periods}</dd>
              </dl>
              {!value.is_annualised && (
                <p role="note" className="text-xs text-muted-foreground">
                  Not annualised: Rust reports a horizon under one year
                  cumulatively, so the annualised figure equals the cumulative
                  one.
                </p>
              )}
            </>
          )}
        </Outcome>
      </Card>
      <Card
        title="Modified-Dietz period return"
        description="One period's return from its beginning and ending values and external flows, as a decimal for the period, not annualised. A contribution into the portfolio is positive, a withdrawal negative."
      >
        <form
          aria-label="Period values and flows"
          className="space-y-3"
          onSubmit={(event) => {
            event.preventDefault();
            onSubmitDietz({
              beginning_market_value: typedNumber(bmv),
              ending_market_value: typedNumber(emv),
              cashflows: flows.map((flow) => ({
                amount: typedNumber(flow.amount),
                fraction_of_period_remaining: typedNumber(flow.fraction),
              })),
            });
          }}
        >
          <div className="grid gap-3 @lg:grid-cols-2">
            <DecimalInput
              label={labelFor("beginning_market_value")}
              value={bmv}
              onValueChange={setBmv}
            />
            <DecimalInput
              label={labelFor("ending_market_value")}
              value={emv}
              onValueChange={setEmv}
            />
          </div>
          <ol className="space-y-2">
            {flows.map((flow, index) => (
              <li
                key={index}
                className="grid items-end gap-2 @lg:grid-cols-[1fr_1fr_auto]"
              >
                <DecimalInput
                  label={`Flow ${ordinal(index)} ${labelFor("amount").toLowerCase()}`}
                  help="Positive is a contribution into the portfolio."
                  value={flow.amount}
                  onValueChange={(amount) =>
                    setFlows(replace(flows, index, { ...flow, amount }))
                  }
                />
                <DecimalInput
                  label={`Flow ${ordinal(index)} ${labelFor("fraction_of_period_remaining").toLowerCase()}`}
                  help="Share of the period left after the flow, in [0, 1]: 1 at the start, 0 at the end."
                  value={flow.fraction}
                  onValueChange={(fraction) =>
                    setFlows(replace(flows, index, { ...flow, fraction }))
                  }
                />
                <RemoveButton
                  label={`Remove flow ${ordinal(index)}`}
                  onClick={() => setFlows(without(flows, index))}
                />
              </li>
            ))}
          </ol>
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={() => setFlows([...flows, { amount: "", fraction: "" }])}
          >
            Add flow
          </Button>
          <div>
            <Button type="submit">Compute period return</Button>
          </div>
        </form>
        <Outcome
          state={{
            result: dietz.value,
            loading: dietz.loading,
            error: dietz.error,
          }}
          working="Computing period return…"
          empty="No period return"
        >
          {(value) => (
            <p aria-label="Modified-Dietz return">
              Period return (decimal, not annualised):{" "}
              <Value value={value} formatValue={formatValue} />
            </p>
          )}
        </Outcome>
      </Card>
      <Card
        title="Money-weighted return (XIRR)"
        description="The annualised internal rate of return of dated flows from the investor's cash account, a decimal (Act/365F). A contribution is negative; distributions and the terminal value are positive. The net flows must change sign exactly once."
      >
        <form
          aria-label="Dated cash flows"
          className="space-y-3"
          onSubmit={(event) => {
            event.preventDefault();
            onSubmitXirr(
              dated.map((flow) => ({
                date: flow.date,
                amount: typedNumber(flow.amount),
              })),
            );
          }}
        >
          <ol className="space-y-2">
            {dated.map((flow, index) => (
              <li
                key={index}
                className="grid items-end gap-2 @lg:grid-cols-[1fr_1fr_auto]"
              >
                <DateInput
                  label={`Cash flow ${ordinal(index)} ${labelFor("date").toLowerCase()}`}
                  value={flow.date}
                  onValueChange={(date) =>
                    setDated(replace(dated, index, { ...flow, date }))
                  }
                />
                <DecimalInput
                  label={`Cash flow ${ordinal(index)} ${labelFor("amount").toLowerCase()}`}
                  help="Negative is a contribution by the investor."
                  value={flow.amount}
                  onValueChange={(amount) =>
                    setDated(replace(dated, index, { ...flow, amount }))
                  }
                />
                <RemoveButton
                  label={`Remove cash flow ${ordinal(index)}`}
                  onClick={() => setDated(without(dated, index))}
                />
              </li>
            ))}
          </ol>
          <Button
            type="button"
            variant="outline"
            size="sm"
            onClick={() => setDated([...dated, { date: "", amount: "" }])}
          >
            Add cash flow
          </Button>
          <div>
            <Button type="submit">Solve XIRR</Button>
          </div>
        </form>
        <Outcome
          state={{
            result: xirr.value,
            loading: xirr.loading,
            error: xirr.error,
          }}
          working="Solving XIRR…"
          empty="No money-weighted return"
        >
          {(value) => (
            <p aria-label="Money-weighted return">
              Money-weighted return (decimal per year, Act/365F):{" "}
              <Value value={value} formatValue={formatValue} />
            </p>
          )}
        </Outcome>
      </Card>
    </section>
  );
}
