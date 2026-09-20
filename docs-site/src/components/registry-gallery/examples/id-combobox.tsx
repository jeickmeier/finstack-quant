"use client";
import type { ExampleProps } from "./props";
import { useState } from "react";
import { IdCombobox } from "@/components/finstack/primitives/id-combobox/id-combobox";
export function Example(_props: ExampleProps) {
  const [choice, setChoice] = useState("alpha");
  return (
    <IdCombobox
      label="Identifier"
      value={choice}
      onValueChange={setChoice}
      options={["alpha", "beta", "literal/id.with.dot"]}
    />
  );
}
