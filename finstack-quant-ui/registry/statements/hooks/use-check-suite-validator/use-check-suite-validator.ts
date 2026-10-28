"use client";
import { useNativeValidator } from "@/hooks/shared/use-finstack/validation";
/** Native canonical CheckSuiteSpec JSON through the shared worker; superseded revisions reject. */
export function useCheckSuiteValidator() {
  return useNativeValidator("validateCheckSuiteSpec", "Check suite");
}
