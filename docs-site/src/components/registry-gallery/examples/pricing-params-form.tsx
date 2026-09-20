"use client";
import type { ExampleProps } from "./props";
import { useState } from "react";
import {
  PricingParamsForm,
  type PricingParams,
} from "@/components/finstack/components/pricing-params-form/pricing-params-form";
import data from "../data.json";

export function Example(_props: ExampleProps) {
  const [params, setParams] = useState<PricingParams>(data.bond.request);
  return (
    <PricingParamsForm
      value={params}
      onValueChange={setParams}
      instrumentType="bond"
      models={{ bond: ["discounting", "rates_credit"] }}
      metrics={data.bond.groups}
    />
  );
}
