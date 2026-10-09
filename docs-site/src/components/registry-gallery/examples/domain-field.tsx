"use client";
import type { ExampleProps } from "./props";
import { useAppForm } from "@/lib/finstack/form";
import {
  DateField,
  DecimalField,
} from "@/components/finstack/core/primitives/domain-field/domain-field";

export function Example(_props: ExampleProps) {
  const form = useAppForm({
    defaultValues: { notional: "1000000.00", maturity: "2034-01-15" },
  });
  return (
    <form
      className="grid gap-3 sm:grid-cols-2"
      onSubmit={(event) => event.preventDefault()}
    >
      <form.AppForm>
        <form.AppField name="notional">
          {() => <DecimalField label="Notional" />}
        </form.AppField>
        <form.AppField name="maturity">
          {() => <DateField label="Maturity" />}
        </form.AppField>
      </form.AppForm>
    </form>
  );
}
