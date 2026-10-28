/**
 * Wall-clock allowance for a test, hook or wait, in milliseconds. Hosted CI
 * runners run the dev WASM build two to three times slower than a developer
 * machine, so allowances scale by four there; locally the stated value applies.
 * These are ceilings that catch hangs, not performance assertions.
 * @param ms - Allowance on a developer machine.
 * @returns The allowance for the current environment.
 */
export const budget = (ms: number) => (process.env.CI ? ms * 4 : ms);
