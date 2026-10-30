"use client";
import { useNativeValidator } from "@/hooks/shared/use-finstack/validation";
/** Native canonical CsaSpec text through the shared worker; superseded revisions reject. */
export function useCsaValidator() {
  return useNativeValidator("validateCsa", "CSA");
}
