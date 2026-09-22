"use client";
import type { ExampleProps } from "./props";
import { PricingWorkbench } from "@/components/finstack/valuations/blocks/pricing-workbench/pricing-workbench";
import data from "../data.json";
export function Example({
  density = "compact",
  variant = "default",
  instrument,
}: ExampleProps) {
  return (
    <PricingWorkbench
      defaultInstrumentType={instrument}
      defaultRequest={
        variant === "equity-option"
          ? {
              ...data.cashflows.cases.find(
                (entry) => entry.type === "equity_option",
              )!.request,
              metrics: ["delta", "gamma", "vega", "theta"],
            }
          : variant === "structured-credit"
            ? data.details.cases.find(
                (entry) => entry.type === "structured_credit",
              )!.request
            : data.bond.request
      }
      density={density}
      defaultCalibrationJson={JSON.stringify(data.calibration[0]!.input)}
    />
  );
}
