"use client";
import { useState } from "react";
import { Input } from "@/components/ui/input";
import { FieldFrame, type FieldInfo } from "../field-frame/field-frame";
import { groupDecimal } from "@/lib/finstack/format/format";
import contracts from "@/lib/finstack/generated/primitive-contracts.json";
export interface DecimalInputProps extends FieldInfo {
  value: string;
  onValueChange: (value: string) => void;
  disabled?: boolean;
  readOnly?: boolean;
  required?: boolean;
  pattern?: string;
  min?: string | number;
  max?: string | number;
  scale?: number;
  allowNegative?: boolean;
  suffix?: string;
  placeholder?: string;
}
/** Exact controlled text; grouping is display-only on blur and incomplete edits are retained. */
export function DecimalInput({
  value,
  onValueChange,
  pattern = contracts.decimal.schema.pattern,
  allowNegative = true,
  suffix,
  scale,
  ...props
}: DecimalInputProps) {
  const [focused, setFocused] = useState(false);
  const { label, help, error, id, path, dirty, ...input } = props;
  const invalid =
    value !== "" &&
    (!new RegExp(pattern).test(value) ||
      (!allowNegative && value.startsWith("-")));
  let display = value;
  if (!focused)
    try {
      display = groupDecimal(value);
    } catch {
      /* Incomplete edits remain visible. */
    }
  return (
    <FieldFrame
      {...{ label, help, id, path, dirty }}
      error={
        error ??
        (invalid
          ? "Value does not match the supplied decimal constraints"
          : undefined)
      }
    >
      {(control) => (
        <div className="flex items-center gap-2">
          <Input
            {...input}
            {...control}
            type="text"
            inputMode="decimal"
            pattern={pattern}
            data-scale={scale}
            value={focused ? value : display}
            onChange={(event) =>
              onValueChange(event.target.value.replaceAll(",", ""))
            }
            onFocus={() => setFocused(true)}
            onBlur={() => setFocused(false)}
          />
          {suffix && (
            <span className="text-xs whitespace-nowrap text-muted-foreground">
              {suffix}
            </span>
          )}
        </div>
      )}
    </FieldFrame>
  );
}
