"use client";
import type { ExampleProps } from "./props";
import { useState } from "react";
import { useAppForm } from "@/lib/finstack/form";
import { DecimalField } from "@/components/finstack/core/primitives/domain-field/domain-field";
function FormDemo() {
  const [submitted, setSubmitted] = useState("");
  const form = useAppForm({
    defaultValues: { amount: "1234.567890123456789" },
    onSubmit: ({ value }) => {
      setSubmitted(value.amount);
    },
  });
  return (
    <form
      onSubmit={(event) => {
        event.preventDefault();
        void form.handleSubmit();
      }}
    >
      <form.AppForm>
        <form.AppField name="amount">
          {() => <DecimalField label="Exact amount" />}
        </form.AppField>
        <form.SubmitButton label="Submit amount" />
        <form.ResetButton />
      </form.AppForm>
      <output aria-label="Submitted amount" className="sr-only">
        {submitted}
      </output>
    </form>
  );
}
export function Example(_props: ExampleProps) {
  return <FormDemo />;
}
