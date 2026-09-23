"use client";
import { DateInput } from "../date-input/date-input";
import { DecimalInput } from "../decimal-input/decimal-input";
import { errorText, useFieldContext } from "@/lib/finstack/form";
import type { FieldInfo } from "@/components/finstack/shared/primitives/field-frame/field-frame";

/** Decimal edit bound to the shared form context. Canonical validation stays outside this control. */
export function DecimalField(props: FieldInfo & { pattern?: string }) {
  const field = useFieldContext<string>();
  return (
    <DecimalInput
      {...props}
      path={field.name}
      dirty={field.state.meta.isDirty}
      value={field.state.value ?? ""}
      onValueChange={field.handleChange}
      error={errorText(field.state.meta.errors)}
    />
  );
}

/** Date edit bound to the shared form context. The calendar only presents the supplied value. */
export function DateField(props: FieldInfo) {
  const field = useFieldContext<string>();
  return (
    <DateInput
      {...props}
      path={field.name}
      dirty={field.state.meta.isDirty}
      value={field.state.value ?? ""}
      onValueChange={field.handleChange}
      error={errorText(field.state.meta.errors)}
    />
  );
}
