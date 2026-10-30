"use client";
import { useNativeValidator } from "@/hooks/shared/use-finstack/validation";
/** Native canonical MertonModel JSON through the shared worker; superseded revisions reject. */
export function useMertonModelValidator() {
  return useNativeValidator("validateMertonModel", "Merton model");
}
