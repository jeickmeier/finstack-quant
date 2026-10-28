"use client";
import { useNativeValidator } from "@/hooks/shared/use-finstack/validation";
/**
 * Native parse-and-compile check of one formula; resolves to the native parse preview text and
 * rejects invalid formulas. A call rejects as superseded only when the caller passes an
 * AbortSignal that has been aborted; callers own debouncing and staleness.
 */
export function useStatementFormulaValidator() {
  return useNativeValidator("validateStatementFormula", "Formula");
}
