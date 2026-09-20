"use client";
import { EnumField, type EnumFieldProps } from "../enum-field/enum-field";
import contracts from "@/lib/finstack/generated/primitive-contracts.json";
const defaults = contracts.business_day_convention.schema.oneOf.map(
  (option) => ({
    value: option.const,
    label: option.const,
    description: option.description,
  }),
);
/** Business-day convention choices from schema, with no local adjustment calculation. */
export function BdcSelect(
  props: Omit<EnumFieldProps, "options"> & {
    options?: EnumFieldProps["options"];
  },
) {
  return (
    <EnumField {...props} options={props.options ?? defaults} layout="select" />
  );
}
