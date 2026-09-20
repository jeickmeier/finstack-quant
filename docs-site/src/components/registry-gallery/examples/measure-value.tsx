"use client";
import type { ExampleProps } from "./props";
import { MeasureValue } from "@/components/finstack/primitives/measure-value/measure-value";

export function Example(_props: ExampleProps) {
  return (
    <MeasureValue label="Original measure" value={-0.0000123456789} signed />
  );
}
