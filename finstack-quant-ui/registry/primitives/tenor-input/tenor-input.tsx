"use client";
import { DecimalInput } from "../decimal-input/decimal-input";
import { EnumField, type EnumOption } from "../enum-field/enum-field";
import type { FieldInfo } from "../field-frame/field-frame";
import contracts from "@/lib/finstack/generated/primitive-contracts.json";
export interface TenorEdit {
  count: number | string;
  unit: string;
}
/** Edits the supplied count and unit; canonical integer constraints and options are copied from schema. */
export function TenorInput({
  value,
  onValueChange,
  units = contracts.tenor.schema.$defs.TenorUnit.oneOf.map((option) => ({
    value: option.const,
    label: option.const,
    description: option.description,
  })),
  label,
  error,
  disabled,
}: FieldInfo & {
  value: TenorEdit;
  onValueChange: (value: TenorEdit) => void;
  units?: readonly EnumOption[];
  disabled?: boolean;
}) {
  return (
    <fieldset className="grid grid-cols-2 gap-2 font-sans text-base text-foreground">
      <legend>{label}</legend>
      <DecimalInput
        label={`${label} count`}
        value={String(value.count)}
        onValueChange={(text) => {
          const count = Number(text);
          onValueChange({
            ...value,
            count:
              /^-?\d+$/.test(text) && Number.isSafeInteger(count)
                ? count
                : text,
          });
        }}
        pattern={/^-?\d+$/.source}
        min={contracts.tenor.schema.properties.count.minimum}
        error={error}
        disabled={disabled}
      />
      <EnumField
        label={`${label} unit`}
        value={value.unit}
        onValueChange={(unit) => onValueChange({ ...value, unit })}
        options={units}
        layout="select"
        disabled={disabled}
      />
    </fieldset>
  );
}
