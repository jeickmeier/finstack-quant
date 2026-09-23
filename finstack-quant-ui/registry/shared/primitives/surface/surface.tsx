"use client";
import { createContext, useContext } from "react";
/** Theme and density of the owning surface; undefined inherits the document root. */
export interface Surface {
  theme?: "light" | "dark";
  density?: "compact" | "comfortable";
}
export const SurfaceContext = createContext<Surface>({});
/** Attributes for portaled popups, which render under body and otherwise miss a surface-local theme. */
export function useSurfaceAttributes() {
  const { theme, density } = useContext(SurfaceContext);
  return { "data-theme": theme, "data-density": density };
}
