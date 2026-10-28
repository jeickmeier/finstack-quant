"use client";
import { useState } from "react";
import { SchemaForm } from "@/components/finstack/core/components/schema-form/schema-form";
import { useStatementModelValidator } from "@/hooks/statements/use-statement-model-validator/use-statement-model-validator";
import { financialModelModule } from "./model";
import { FormulaPreview } from "./formula-preview";

/** Edit explicit model periods, nodes and structure from the generated Rust schema. */
export function FinancialModelEditor({
  defaultJson,
  onSubmit,
  onValidated,
  onInputJson,
}: {
  defaultJson: string;
  /** Receives only native canonical JSON. Remount for a different document. */
  onSubmit(json: string): void | Promise<void>;
  onValidated?: (json: string | null) => void;
  onInputJson?: (json: string | null) => void;
}) {
  const validate = useStatementModelValidator();
  const [initial] = useState(() => {
    try {
      const value = financialModelModule.codec.parse(defaultJson);
      if (value === null || typeof value !== "object" || Array.isArray(value))
        throw new TypeError("Financial model must be a JSON object");
      return { value: value as Record<string, unknown> };
    } catch (error) {
      return { error: error instanceof Error ? error.message : String(error) };
    }
  });
  if ("error" in initial) return <p role="alert">{initial.error}</p>;
  return (
    <SchemaForm
      module={financialModelModule}
      defaultValues={initial.value}
      validate={validate}
      onSubmit={onSubmit}
      onValidated={onValidated}
      onInputJson={onInputJson}
      renderField={({ path, value, renderDefault }) =>
        path.endsWith(".formula_text") ? (
          <>
            {renderDefault()}
            <FormulaPreview
              formula={typeof value === "string" ? value : null}
            />
          </>
        ) : undefined
      }
      submitLabel="Evaluate model"
      layout="full"
    />
  );
}
