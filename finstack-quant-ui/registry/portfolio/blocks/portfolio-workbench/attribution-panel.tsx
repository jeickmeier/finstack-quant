"use client";
import { useState } from "react";
import { PortfolioAttributionReport } from "@/components/finstack/portfolio/components/portfolio-attribution-report/portfolio-attribution-report";
import {
  EnumField,
  type EnumOption,
} from "@/components/finstack/shared/primitives/enum-field/enum-field";
import {
  useAttributePortfolioPnl,
  type PortfolioAttributionRequest,
} from "@/hooks/portfolio/use-attribute-portfolio-pnl/use-attribute-portfolio-pnl";
import { useDefaultWaterfallOrder } from "@/hooks/attribution/use-default-waterfall-order/use-default-waterfall-order";

/** The host's closing snapshot and reconciliation tolerance for the Attribution tab. */
export interface PortfolioAttributionInputs {
  /** MarketContextState JSON of the closing (T1) snapshot; the workbench's marketJson is the opening one. A seasoned floating leg needs its FIXING: series here once T1 is past a fixing date, or Rust returns that error. */
  readonly marketT1Json: string;
  /** ISO date of the closing snapshot; the workbench's asOf is the opening date. */
  readonly asOfT1: string;
  /** Largest absolute gap between total P&L and its buckets, in base-currency units, that Rust still reports as reconciled. */
  readonly reconciliationTolerance: number;
}

type Choice = "" | "parallel" | "waterfall" | "metrics_based" | "taylor";
type Method = PortfolioAttributionRequest["method"];
type WaterfallOrder = Extract<Method, { waterfall: unknown }>["waterfall"];

/** The AttributionMethod wire value of a choice: waterfall in Rust's default order, Taylor with its config left for Rust to fill; null until both exist. */
function attributionMethod(
  choice: Choice,
  order: readonly string[] | undefined,
): Method | null {
  if (choice === "") return null;
  if (choice === "waterfall")
    return order ? { waterfall: [...order] as WaterfallOrder } : null;
  return choice === "taylor" ? { taylor: {} } : choice;
}

interface AttributionPanelProps extends PortfolioAttributionInputs {
  /** Canonical PortfolioSpec JSON the workbench values. */
  specJson: string;
  /** MarketContextState JSON of the opening (T0) snapshot. */
  marketJson: string;
  /** ISO date of the opening snapshot. */
  asOf: string;
  density: "compact" | "comfortable";
}

/**
 * The book's P&L between the workbench market and the host's T1 market: one
 * native portfolio.attributePortfolioPnl per chosen method, mounted (and so
 * called) when the Attribution tab first opens and a method is chosen.
 */
export function AttributionPanel({
  specJson,
  marketJson,
  asOf,
  marketT1Json,
  asOfT1,
  reconciliationTolerance,
  density,
}: AttributionPanelProps) {
  const [choice, setChoice] = useState<Choice>("");
  const order = useDefaultWaterfallOrder();
  const method = attributionMethod(choice, order.data);
  const attribution = useAttributePortfolioPnl(
    method
      ? {
          specJson,
          marketT0Json: marketJson,
          marketT1Json,
          asOfT0: asOf,
          asOfT1,
          method,
          reconciliationTolerance,
        }
      : null,
  );
  // Every Rust method is offered; one that fails on this book shows its native error.
  const methods: EnumOption[] = [
    { value: "parallel", label: "parallel" },
    {
      value: "waterfall",
      label: "waterfall",
      description: order.data
        ? `Rust's default order: ${order.data.join(", ")}`
        : undefined,
      disabled: !order.data,
    },
    { value: "metrics_based", label: "metrics_based" },
    { value: "taylor", label: "taylor" },
  ];
  return (
    <div className="flex flex-col gap-4">
      <p className="text-xs text-muted-foreground">
        Rust attributes every position&apos;s P&amp;L from the workbench market
        on {asOf} to the supplied closing market on {asOfT1}, then converts to
        the book currency.
      </p>
      <div className="max-w-xs">
        <EnumField
          label="Attribution method"
          value={choice}
          onValueChange={(value) => setChoice(value as Choice)}
          options={methods}
          layout="radio"
          error={order.error?.message}
        />
      </div>
      {method ? (
        <PortfolioAttributionReport
          report={attribution.data ?? null}
          loading={attribution.isFetching}
          error={attribution.error?.message}
          density={density}
        />
      ) : (
        <p role="status" className="text-sm text-muted-foreground">
          Choose a method; Rust requires one.
        </p>
      )}
    </div>
  );
}
