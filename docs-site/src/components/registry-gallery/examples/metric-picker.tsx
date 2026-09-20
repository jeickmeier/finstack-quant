"use client";
import type { ExampleProps } from "./props";
import { useState } from "react";
import { MetricPicker } from "@/components/finstack/primitives/metric-picker/metric-picker";
const options = [
  { value: "alpha", label: "Alpha", group: "Supplied" },
  { value: "beta", label: "Beta", group: "Supplied" },
];
export function Example(_props: ExampleProps) {
  const [metrics, setMetrics] = useState<string[]>(["alpha"]);
  return (
    <MetricPicker
      label="Supplied metrics"
      value={metrics}
      onValueChange={setMetrics}
      options={options}
    />
  );
}
