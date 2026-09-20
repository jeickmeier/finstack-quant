"use client";
import type { ExampleProps } from "./props";
import { useState } from "react";
import { MoneyInput } from "@/components/finstack/primitives/money-input/money-input";

export function Example(_props: ExampleProps) {
  const [money, setMoney] = useState({
    amount: "1000000.123456789",
    currency: "USD",
  });
  return (
    <MoneyInput
      label="Notional"
      value={money}
      onValueChange={setMoney}
      currencies={["USD", "EUR"]}
    />
  );
}
