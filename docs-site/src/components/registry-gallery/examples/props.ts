export interface ExampleProps {
  density?: "compact" | "comfortable";
  width?: number;
  publication?: boolean;
  variant?: string;
  instrument?: string;
}
/** Host display choice: six decimal places of the returned values. */
export const sixPlaces = (value: number) => value.toFixed(6);
