import { createContext } from "react";

/** Generic collapsed group for defaulted or empty values. */
export const LESS_COMMON_TERMS = "Less common terms";

export interface DisclosureField {
  /** Canonical path of the object that owns the property ("" at the root). */
  path: string;
  key: string;
  siblingKeys: readonly string[];
  /** Value equals its schema default, is empty, or is an absent optional/nullable value. */
  secondary: boolean;
}

/** Where layout="basic" collapses object properties; a composing editor supplies its own. */
export interface Disclosure {
  /** Group names in sheet order; unlisted groups follow in first-use order. */
  order: readonly string[];
  /** Group name, or null to keep the property on the first screen. */
  group(field: DisclosureField): string | null;
}

export const secondaryDisclosure: Disclosure = {
  order: [LESS_COMMON_TERMS],
  group: ({ secondary }) => (secondary ? LESS_COMMON_TERMS : null),
};

/** Object rendered as the term sheet: its ancestors render without fieldsets; it renders as the finstack-schema-root fieldset labelled `label`. */
export interface SheetRoot {
  path: string;
  label: string;
}

export const revealKey = (path: string, group: string) =>
  JSON.stringify([path, group]);

export const SheetContext = createContext<{
  disclosure: Disclosure;
  root?: SheetRoot;
  revealers: Map<string, () => void>;
}>({ disclosure: secondaryDisclosure, revealers: new Map() });
