"use client";
import type { ExampleProps } from "./props";
import { MoneyValue } from "@/components/finstack/core/primitives/money-value/money-value";

export function Example(_props: ExampleProps) {
  const money = {
    amount: "1000000.123456789",
    currency: "USD",
  };
  return <MoneyValue value={money} />;
}
