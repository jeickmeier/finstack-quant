"use client";
import type { ExampleProps } from "./props";
import { useState } from "react";
import { RateInput } from "@/components/finstack/primitives/rate-input/rate-input";

export function Example(_props: ExampleProps) {
  const [rate, setRate] = useState<string | number>("0.0525");
  return <RateInput label="Rate" value={rate} onValueChange={setRate} />;
}
