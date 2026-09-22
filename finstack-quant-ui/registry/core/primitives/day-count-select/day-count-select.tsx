"use client";
import {
  EnumField,
  type EnumFieldProps,
} from "@/components/finstack/shared/primitives/enum-field/enum-field";
import contracts from "@/lib/finstack/generated/primitive-contracts.json";
const defaults = contracts.day_count.schema.oneOf.map((option) => ({
  value: option.const,
  label: option.const,
  description: option.description,
}));
/** Day-count choices projected from the canonical schema; never implements accrual rules. */
export function DayCountSelect(
  props: Omit<EnumFieldProps, "options"> & {
    options?: EnumFieldProps["options"];
  },
) {
  return (
    <EnumField {...props} options={props.options ?? defaults} layout="select" />
  );
}
