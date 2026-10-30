"use client";
import type { ExampleProps } from "./props";
import { CovenantReport } from "@/components/finstack/covenants/components/covenant-report/covenant-report";
import data from "../data.json";

export function Example({ density = "compact" }: ExampleProps) {
  return (
    <CovenantReport
      reports={JSON.parse(data.details.covenantReportsJson)}
      density={density}
    />
  );
}
