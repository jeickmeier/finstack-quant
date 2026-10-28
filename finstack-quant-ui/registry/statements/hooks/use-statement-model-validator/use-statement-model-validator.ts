"use client";
import { useNativeValidator } from "@/hooks/shared/use-finstack/validation";
/** Native canonical FinancialModelSpec JSON through the shared worker; superseded revisions reject. */
export function useStatementModelValidator() {
  return useNativeValidator("validateStatementModel", "Statement model");
}
