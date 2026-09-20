"use client";
import { useState } from "react";
import { SchemaForm, type SchemaFormProps } from "../schema-form/schema-form";
import { calibrationModule } from "./calibration";
/** Edit the complete generated envelope. Remount to load a different initial document. */
export function CalibrationForm({
  defaultJson,
  validate,
  onSubmit,
  onValidated,
  onInputJson,
}: {
  /** Full canonical envelope; no market, quote, model or solver defaults are inferred. */
  defaultJson: string;
  validate: SchemaFormProps["validate"];
  /** Receives native canonical JSON on explicit submission. Connect it to useCalibrate. */
  onSubmit: SchemaFormProps["onSubmit"];
  onValidated?: SchemaFormProps["onValidated"];
  onInputJson?: SchemaFormProps["onInputJson"];
}) {
  const [initial] = useState(() => {
    try {
      const value = calibrationModule.codec.parse(defaultJson);
      if (value === null || typeof value !== "object" || Array.isArray(value))
        throw new TypeError("Calibration envelope must be a JSON object");
      return { value: value as Record<string, unknown> };
    } catch (error) {
      return { error: error instanceof Error ? error.message : String(error) };
    }
  });
  if ("error" in initial) return <p role="alert">{initial.error}</p>;
  return (
    <SchemaForm
      module={calibrationModule}
      defaultValues={initial.value}
      validate={validate}
      onSubmit={onSubmit}
      onValidated={onValidated}
      onInputJson={onInputJson}
      submitLabel="Calibrate"
      layout="full"
    />
  );
}
