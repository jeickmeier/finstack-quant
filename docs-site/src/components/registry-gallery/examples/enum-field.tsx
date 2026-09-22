"use client";
import type { ExampleProps } from "./props";
import { useState } from "react";
import { EnumField } from "@/components/finstack/shared/primitives/enum-field/enum-field";
const options = [
  { value: "alpha", label: "Alpha", group: "Supplied" },
  { value: "beta", label: "Beta", group: "Supplied" },
];
export function Example(_props: ExampleProps) {
  const [choice, setChoice] = useState("alpha");
  return (
    <EnumField
      label="Supplied choice"
      value={choice}
      onValueChange={setChoice}
      options={options}
    />
  );
}
