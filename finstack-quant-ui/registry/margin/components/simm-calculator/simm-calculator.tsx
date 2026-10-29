"use client";
import { useState } from "react";
import type { ImResult, SimmSensitivitiesJson } from "finstack-quant-wasm";
import {
  SchemaForm,
  type SchemaFormProps,
} from "@/components/finstack/core/components/schema-form/schema-form";
import { DateInput } from "@/components/finstack/core/primitives/date-input/date-input";
import { SimmSensitivityTable } from "@/components/finstack/margin/components/simm-sensitivity-table/simm-sensitivity-table";
import { SimmReport } from "@/components/finstack/margin/components/simm-report/simm-report";
import { simmModule } from "./simm";

export interface SimmCalculatorProps {
  /** Initial SimmSensitivitiesJson text; omitted starts from the schema's Rust example. Remount to load another document. */
  defaultJson?: string;
  /** ISO calculation date stamped on the result; Rust validates it. */
  asOf: string;
  onAsOfChange(asOf: string): void;
  /** Native validation returning canonical SimmSensitivitiesJson text; connect useSimmSensitivitiesValidator. */
  validate: SchemaFormProps["validate"];
  /** Receives the canonical sensitivities on Calculate; connect it to the useSimmIm request. */
  onSubmit(json: string): void;
  /** The returned ImResult; null while none is available. */
  result: ImResult | null;
  loading?: boolean;
  /** The native calculation error, shown as returned. */
  error?: string;
  /** Display text of a supplied or returned number; omitted shows every digit. */
  formatValue?: (value: number) => string;
  density?: "compact" | "comfortable";
}
/**
 * Edits a SIMM sensitivity set with the generated schema form and a
 * calculation date, shows the natively validated set by risk class, and
 * presents the returned initial margin. Validation and the calculation are
 * the caller's native callbacks; nothing is computed here.
 */
export function SimmCalculator({
  defaultJson,
  asOf,
  onAsOfChange,
  validate,
  onSubmit,
  result,
  loading,
  error,
  formatValue,
  density = "compact",
}: SimmCalculatorProps) {
  const [initial] = useState(() => {
    if (defaultJson === undefined) return { value: undefined };
    try {
      const value = simmModule.codec.parse(defaultJson);
      if (value === null || typeof value !== "object" || Array.isArray(value))
        throw new TypeError("SIMM sensitivities must be a JSON object");
      return { value: value as Record<string, unknown> };
    } catch (failure) {
      return {
        error: failure instanceof Error ? failure.message : String(failure),
      };
    }
  });
  const [validated, setValidated] = useState<SimmSensitivitiesJson | null>(
    null,
  );
  if ("error" in initial) return <p role="alert">{initial.error}</p>;
  return (
    <section
      aria-label="SIMM calculator"
      data-density={density}
      className="space-y-6 font-sans text-sm text-foreground"
    >
      <div className="max-w-xs">
        <DateInput label="As of" value={asOf} onValueChange={onAsOfChange} />
      </div>
      <SchemaForm
        module={simmModule}
        defaultValues={initial.value}
        validate={validate}
        onValidated={(json) =>
          setValidated(
            json === null
              ? null
              : (simmModule.codec.parse(json) as SimmSensitivitiesJson),
          )
        }
        onSubmit={onSubmit}
        submitLabel="Calculate initial margin"
        layout="basic"
      />
      <SimmSensitivityTable
        sensitivities={validated}
        formatValue={formatValue}
        density={density}
      />
      <SimmReport
        result={result}
        loading={loading}
        error={error}
        formatValue={formatValue}
        density={density}
      />
    </section>
  );
}
