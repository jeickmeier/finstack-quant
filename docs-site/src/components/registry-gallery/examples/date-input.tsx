"use client";
import type { ExampleProps } from "./props";
import { useState } from "react";
import { DateInput } from "@/components/finstack/core/primitives/date-input/date-input";

export function Example(_props: ExampleProps) {
  const [date, setDate] = useState("2024-02-29");
  return <DateInput label="As of" value={date} onValueChange={setDate} />;
}
