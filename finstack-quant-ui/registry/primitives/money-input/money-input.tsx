"use client";
import type { MoneyValue } from "finstack-quant-wasm";
import {
  DecimalInput,
  type DecimalInputProps,
} from "../decimal-input/decimal-input";
import { EnumField } from "../enum-field/enum-field";
/** Exact money amount and caller-supplied currency choices; performs no FX conversion. */
export function MoneyInput({
  value,
  onValueChange,
  currencies,
  lockCurrency = false,
  label,
  ...props
}: Omit<DecimalInputProps, "value" | "onValueChange" | "suffix"> & {
  value: MoneyValue;
  onValueChange: (value: MoneyValue) => void;
  currencies: readonly string[];
  lockCurrency?: boolean;
}) {
  return (
    <fieldset className="grid grid-cols-[minmax(0,1fr)_auto] items-end gap-2 font-sans text-base text-foreground">
      <legend className="text-sm text-muted-foreground">{label}</legend>
      <DecimalInput
        {...props}
        label={`${label} amount`}
        value={value.amount}
        onValueChange={(amount) => onValueChange({ ...value, amount })}
      />
      {lockCurrency ? (
        <span aria-label={`${label} currency`} className="font-mono">
          {value.currency}
        </span>
      ) : (
        <EnumField
          label={`${label} currency`}
          value={value.currency}
          onValueChange={(currency) => onValueChange({ ...value, currency })}
          options={currencies.map((currency) => ({
            value: currency,
            label: currency,
          }))}
          layout="select"
          disabled={props.disabled}
        />
      )}
    </fieldset>
  );
}
