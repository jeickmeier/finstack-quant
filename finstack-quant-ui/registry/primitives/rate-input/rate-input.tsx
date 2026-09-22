"use client";
import { numericEdit } from "@/lib/finstack/schema.mjs";
import {
  DecimalInput,
  type DecimalInputProps,
} from "../decimal-input/decimal-input";
import {
  formatRate,
  rateToWire,
  type RatePresentation,
} from "@/lib/finstack/format/format";
export interface RateInputProps extends Omit<
  DecimalInputProps,
  "value" | "onValueChange" | "suffix"
> {
  value: string | number;
  onValueChange: (value: string | number) => void;
  presentation?: RatePresentation;
  representation?: "decimal" | "number";
}
/** Reversible source-backed display; invalid text remains editable and missing units stay raw. */
export function RateInput({
  value,
  onValueChange,
  presentation,
  representation = "decimal",
  ...props
}: RateInputProps) {
  let text = String(value);
  try {
    text = formatRate(value, presentation).text;
  } catch {
    /* Keep incomplete editing text. */
  }
  return (
    <DecimalInput
      {...props}
      value={text}
      suffix={presentation?.display ?? "Unit unavailable"}
      onValueChange={(edited) => {
        let wire = edited;
        try {
          wire = rateToWire(edited, presentation);
        } catch {
          /* Keep incomplete editing text. */
        }
        onValueChange(representation === "number" ? numericEdit(wire) : wire);
      }}
    />
  );
}
