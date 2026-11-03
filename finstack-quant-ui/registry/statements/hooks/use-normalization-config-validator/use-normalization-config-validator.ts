"use client";
import { useNativeValidator } from "@/hooks/shared/use-finstack/validation";
/** Native canonical NormalizationConfig JSON through the shared worker; superseded revisions reject. */
export function useNormalizationConfigValidator() {
  return useNativeValidator("validateNormalizationConfig", "Normalization");
}
