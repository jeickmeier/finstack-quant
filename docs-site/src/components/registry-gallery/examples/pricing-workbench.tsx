"use client";
import type { ExampleProps } from "./props";
import { PricingWorkbench } from "@/components/finstack/valuations/blocks/pricing-workbench/pricing-workbench";
import type { PriceRequest } from "@/hooks/valuations/use-price-instrument/use-price-instrument";
import data from "../data.json";

const cashflowCase = (type: string) =>
  data.cashflows.cases.find((entry) => entry.type === type)!.request;
const detailCase = (type: string) =>
  data.details.cases.find((entry) => entry.type === type)!.request;

/** Complete, native-backed requests for the instrument choices in this example. */
export const exampleRequests: Record<string, PriceRequest> = {
  bond: data.bond.request,
  fx_swap: cashflowCase("fx_swap"),
  xccy_swap: cashflowCase("xccy_swap"),
  equity_option: {
    ...cashflowCase("equity_option"),
    metrics: ["delta", "gamma", "vega", "theta"],
  },
  commodity_option: detailCase("commodity_option"),
  composite: detailCase("composite"),
  credit_default_swap: detailCase("credit_default_swap"),
  fx_option: detailCase("fx_option"),
  structured_credit: detailCase("structured_credit"),
};

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
      onInstrumentTypeChange={(type) => {
        const url = new URL(window.location.href);
        url.searchParams.set("instrument", type);
        window.history.replaceState(null, "", url);
      }}
      density={density}
      defaultCalibrationJson={JSON.stringify(data.calibration[0]!.input)}
    />
  );
}
