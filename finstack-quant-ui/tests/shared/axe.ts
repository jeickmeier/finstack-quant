import { configureAxe } from "vitest-axe";

// jsdom has no layout or Canvas 2D, so colour contrast cannot be computed here;
// the Playwright gallery suite (tests/e2e) checks it in a real browser.
export const axe = configureAxe({
  rules: { "color-contrast": { enabled: false } },
});
