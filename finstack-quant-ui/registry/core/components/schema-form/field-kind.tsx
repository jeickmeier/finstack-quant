"use client";
import {
  DateField,
  DecimalField,
} from "@/components/finstack/core/primitives/domain-field/domain-field";
import type { FieldInfo } from "@/components/finstack/shared/primitives/field-frame/field-frame";
import type { Metadata, Schema } from "./schema";
import type { SchemaFormApi } from "./schema-form";

/** Scalar controls selected from the generated schema kind. Objects and arrays stay with the layout renderer. */
export function ScalarField({
  form,
  path,
  schema,
  metadata,
  info,
  label,
}: {
  form: SchemaFormApi;
  path: string;
  schema: Schema;
  metadata: Metadata | undefined;
  info: FieldInfo;
  label: string;
}) {
  return (
    <form.AppField name={path}>
      {(field) => {
        if (schema.type === "boolean") return <field.BooleanField {...info} />;
        if (schema.format === "date") return <DateField {...info} />;
        if (
          schema.type === "string" &&
          (metadata?.source.includes("/decimal") || schema.title === "Decimal")
        )
          return <DecimalField {...info} pattern={schema.pattern} />;
        if (["string", "integer", "number"].includes(String(schema.type)))
          return (
            <field.TextField
              {...info}
              inputMode={
                schema.type === "integer"
                  ? "numeric"
                  : schema.type === "number"
                    ? "decimal"
                    : "text"
              }
            />
          );
        return (
          <p className="text-sm text-muted-foreground">
            {label}: unsupported generated schema construct.
          </p>
        );
      }}
    </form.AppField>
  );
}
