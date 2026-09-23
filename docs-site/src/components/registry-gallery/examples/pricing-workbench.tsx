"use client";
import type { ExampleProps } from "./props";
import { PricingWorkbench } from "@/components/finstack/valuations/blocks/pricing-workbench/pricing-workbench";
import type { PriceRequest } from "@/hooks/valuations/use-price-instrument/use-price-instrument";
import data from "../data.json";

const cashflowCase = (type: string) =>
  data.cashflows.cases.find((entry) => entry.type === type)!.request;
const detailCase = (type: string) =>
  data.details.cases.find((entry) => entry.type === type)!.request;
const pricingExamples = data.pricingExamples as Record<
  string,
  {
    instrument: unknown;
    market?: unknown;
    request: Omit<PriceRequest, "instrumentJson" | "marketJson">;
  }
>;
const nativeRequests: Record<string, PriceRequest> = Object.fromEntries(
  Object.entries(pricingExamples).map(([type, example]) => [
    type,
    {
      ...example.request,
      instrumentJson: JSON.stringify(example.instrument),
      marketJson: JSON.stringify(example.market ?? data.pricingMarket),
    },
  ]),
);

/** Complete, native-backed requests for the instrument choices in this example. */
export const exampleRequests: Record<string, PriceRequest> = {
  ...nativeRequests,
  bond: data.bond.request,
  fx_swap: {
    ...cashflowCase("fx_swap"),
    metrics: ["fx01", "dv01"],
  },
  xccy_swap: {
    ...cashflowCase("xccy_swap"),
    metrics: ["dv01", "theta"],
  },
  interest_rate_swap: {
    instrumentJson: JSON.stringify(data.irsInstrument),
    marketJson: JSON.stringify(data.pricingMarket),
    asOf: "2023-12-28",
    model: "discounting",
    metrics: ["par_rate", "dv01"],
    pricingOptions: null,
    marketHistory: null,
  },
  equity_option: {
    ...cashflowCase("equity_option"),
    metrics: ["delta", "gamma", "vega", "theta", "bucketed_vega"],
  },
  commodity_option: {
    ...detailCase("commodity_option"),
    metrics: ["theta"],
  },
  composite: detailCase("composite"),
  credit_default_swap: {
    ...detailCase("credit_default_swap"),
    metrics: ["par_spread", "risky_pv01", "jump_to_default"],
  },
  fx_option: {
    ...detailCase("fx_option"),
    metrics: ["delta", "gamma", "vega"],
  },
  structured_credit: {
    ...detailCase("structured_credit"),
    asOf: data.scenarios.asOf,
    metrics: ["expected_loss", "wal"],
  },
};

const exampleResultTabs = {
  ...Object.fromEntries(
    Object.keys(nativeRequests).map((type) => [type, "measures" as const]),
  ),
  bond: "buckets",
  fx_swap: "cashflows",
  xccy_swap: "cashflows",
  interest_rate_swap: "measures",
  equity_option: "buckets",
  commodity_option: "measures",
  composite: "measures",
  credit_default_swap: "measures",
  fx_option: "measures",
  structured_credit: "measures",
} as const;

export function Example({
  density = "compact",
  variant = "default",
  instrument,
}: ExampleProps) {
  const variantType =
    variant === "equity-option"
      ? "equity_option"
      : variant === "structured-credit"
        ? "structured_credit"
        : "bond";
  const selectedType =
    instrument && Object.hasOwn(exampleRequests, instrument)
      ? instrument
      : variantType;
  return (
    <PricingWorkbench
      key={selectedType}
      defaultInstrumentType={selectedType}
      defaultRequest={exampleRequests[selectedType]!}
      exampleRequests={exampleRequests}
      exampleResultTabs={exampleResultTabs}
      defaultResultTab={
        exampleResultTabs[selectedType as keyof typeof exampleResultTabs] ??
        "measures"
      }
      onInstrumentTypeChange={(type) => {
        const url = new URL(window.location.href);
        url.searchParams.set("instrument", type);
        window.history.replaceState(null, "", url);
      }}
      density={density}
      defaultCalibrationJson={JSON.stringify(data.calibration[0]!.input)}
      scenario={
        selectedType === "structured_credit"
          ? {
              trancheId: data.scenarios.trancheId,
              gridJson: JSON.stringify(data.scenarios.grid),
              priceDomain: [80, 140],
            }
          : undefined
      }
    />
  );
}
