"use client";
import type { ExampleProps } from "./props";
import { useState } from "react";
import { DayCountSelect } from "@/components/finstack/primitives/day-count-select/day-count-select";

export function Example(_props: ExampleProps) {
  const [dayCount, setDayCount] = useState("act_365f");
  return (
    <DayCountSelect
      label="Day count"
      value={dayCount}
      onValueChange={setDayCount}
    />
  );
}
