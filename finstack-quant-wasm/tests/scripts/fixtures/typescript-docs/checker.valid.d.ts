/** Calculator values with deterministic arithmetic. */
export interface Calculator {
  /**
   * Add a finite value to the stored total.
   * @param value - Finite amount to add in currency units.
   * @returns The updated total in currency units.
   * @throws Error - Thrown when `value` is not finite.
   */
  add(value: number): number;
  /**
   * Square a finite numeric value without a fallible conversion boundary.
   * @param value - Finite numeric input.
   * @returns `value` multiplied by itself.
   */
  square(value: number): number;
}

/**
 * Factory for calculator values.
 * @example
 * ```typescript
 * const calculator = new Calculator(2);
 * console.log(calculator.add(3));
 * ```
 */
export interface CalculatorConstructor {
  /**
   * Create a calculator with an initial total.
   * @param initial - Initial finite total in currency units.
   * @returns A calculator initialized to `initial`.
   * @throws Error - Thrown when `initial` is not finite.
   */
  new (initial: number): Calculator;
}

/**
 * Namespaced arithmetic functions.
 * @example
 * ```typescript
 * const result = math.calculate(2);
 * console.log(result);
 * ```
 */
export interface MathNamespace {
  /**
   * Double a finite input.
   * @param value - Finite numeric input.
   * @returns Twice `value`.
   * @throws Error - Thrown when `value` is not finite.
   */
  calculate(value: number): number;
}

/**
 * Double a finite input through the package-root facade.
 * @example
 * ```typescript
 * console.log(calculate(2));
 * ```
 * @param value - Finite numeric input.
 * @returns Twice `value`.
 * @throws Error - Thrown when `value` is not finite.
 */
export function calculate(value: number): number;

/** Namespaced arithmetic entry point. */
export const math: MathNamespace;

/**
 * Reusable WebAssembly report handle used by the documentation checker fixture.
 * @example
 * ```typescript
 * console.log(report.toJson());
 * ```
 */
export declare class ReportHandle {
  /**
   * Serialize this report to canonical JSON.
   * @returns Canonical JSON string for the report.
   * @throws Error - Thrown when serialization cannot complete.
   */
  toJson(): string;
}
