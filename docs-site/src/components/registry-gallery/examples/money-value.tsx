"use client";
import type { ExampleProps } from "./props";
import type { MoneyValue as Money } from "finstack-quant-wasm";
import { MoneyValue } from "@/components/finstack/core/primitives/money-value/money-value";

export function Example(_props: ExampleProps) {
  const money: Money = {
    amount: "1000000.123456789",
    currency: "USD",
  };
  return <MoneyValue value={money} />;
}
