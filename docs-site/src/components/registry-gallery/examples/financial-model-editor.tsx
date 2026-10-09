"use client";
import type { ExampleProps } from "./props";
import { FinancialModelEditor } from "@/components/finstack/statements/components/financial-model-editor/financial-model-editor";
import data from "../data.json";

const modelJson = JSON.stringify(data.statementModel);

export function Example(_props: ExampleProps) {
  return <FinancialModelEditor defaultJson={modelJson} onSubmit={() => {}} />;
}
