import { createContext, useContext } from "react";
import type { components } from "./generated/api";

export type QuantityFormat = components["schemas"]["QuantityFormat"];

/**
 * An exact quantity of atoms as people read it: `atoms ÷ 10^scale`, every
 * nonzero digit written, trailing zeros trimmed down to `digits`, then the
 * unit. It never rounds, and it agrees digit for digit with the view's Rust
 * `QuantityFormat::write`.
 */
export function formatQuantity(atoms: string, format?: QuantityFormat): string {
  if (!format) return atoms;
  const scale = format.scale;
  const padded = atoms.length <= scale ? atoms.padStart(scale + 1, "0") : atoms;
  const whole = padded.slice(0, padded.length - scale);
  const fraction = padded.slice(padded.length - scale).replace(/0+$/, "");
  const digits = Math.max(format.digits ?? 0, fraction.length);
  const number = digits === 0 ? whole : `${whole}.${fraction.padEnd(digits, "0")}`;
  return format.unit ? `${number} ${format.unit}` : number;
}

/** The open document's formats, by asset key. */
export const QuantityFormats = createContext<Record<string, QuantityFormat>>({});

/** Writes quantities of an asset as the open document formats them. */
export function useQuantity(): (asset: string, atoms: string) => string {
  const formats = useContext(QuantityFormats);
  return (asset, atoms) => formatQuantity(atoms, formats[asset]);
}

/** A quantity of `asset`, formatted; the exact atoms stay one hover away. */
export function Amount({ asset, atoms }: { asset: string; atoms: string }) {
  const format = useContext(QuantityFormats)[asset];
  const text = formatQuantity(atoms, format);
  return format ? <span className="amount" title={`${atoms} atoms`}>{text}</span> : <>{text}</>;
}
