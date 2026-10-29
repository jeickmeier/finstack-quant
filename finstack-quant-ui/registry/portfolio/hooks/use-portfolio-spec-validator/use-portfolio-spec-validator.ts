"use client";
import { useNativeValidator } from "@/hooks/shared/use-finstack/validation";
/** Native canonical PortfolioSpec JSON through the shared worker; superseded revisions reject. */
export function usePortfolioSpecValidator() {
  return useNativeValidator("validatePortfolio", "Portfolio");
}
