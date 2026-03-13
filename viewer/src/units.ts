/**
 * Unit formatting and parsing utilities.
 *
 * Internal representation is always nanometers. These functions convert
 * between nanometers and human-readable dimension strings in mm, mil, or µm.
 */

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type DisplayUnit = 'mm' | 'mil' | 'µm';

// ---------------------------------------------------------------------------
// Conversion constants (nanometers per unit)
// ---------------------------------------------------------------------------

/** 1 mm = 1,000,000 nm */
export const NM_PER_MM = 1_000_000;

/** 1 mil (thousandth of an inch) = 25,400 nm */
export const NM_PER_MIL = 25_400;

/** 1 µm = 1,000 nm */
export const NM_PER_UM = 1_000;

// ---------------------------------------------------------------------------
// Formatting
// ---------------------------------------------------------------------------

/**
 * Format a dimension from nanometers to a human-readable string.
 *
 * Examples:
 *   formatDimension(2_540_000, 'mm')  → "2.54mm"
 *   formatDimension(2_540_000, 'mil') → "100.00mil"
 *   formatDimension(2_540_000, 'µm')  → "2540µm"
 */
export function formatDimension(nm: number, unit: DisplayUnit): string {
  switch (unit) {
    case 'mm': {
      const val = nm / NM_PER_MM;
      return `${stripTrailingZeros(val.toFixed(4))}mm`;
    }
    case 'mil': {
      const val = nm / NM_PER_MIL;
      return `${stripTrailingZeros(val.toFixed(4))}mil`;
    }
    case 'µm': {
      const val = nm / NM_PER_UM;
      return `${stripTrailingZeros(val.toFixed(1))}µm`;
    }
  }
}

/**
 * Remove unnecessary trailing zeros from a decimal string.
 *
 *   "2.5400" → "2.54"
 *   "100.00" → "100"
 *   "3.0"    → "3"
 *   "0.50"   → "0.5"
 */
function stripTrailingZeros(s: string): string {
  if (!s.includes('.')) return s;
  let end = s.length;
  while (end > 0 && s[end - 1] === '0') end--;
  if (s[end - 1] === '.') end--; // drop trailing dot entirely: "3." → "3"
  return s.slice(0, end);
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/**
 * Unit suffix patterns for parsing. Order matters — check longer suffixes first.
 * Handles: mm, mil, µm, um (alias for µm)
 */
const UNIT_PATTERNS: [RegExp, number][] = [
  [/mil$/i, NM_PER_MIL],
  [/mm$/i, NM_PER_MM],
  [/[µu]m$/i, NM_PER_UM],
];

/**
 * Parse a user-entered dimension string to nanometers.
 *
 * Accepts formats like: "2.54mm", "100mil", "2540µm", "2540um"
 * Whitespace between number and unit is tolerated.
 * Returns null for invalid input.
 */
export function parseUserDimension(input: string): number | null {
  const trimmed = input.trim();
  if (!trimmed) return null;

  for (const [pattern, nmPerUnit] of UNIT_PATTERNS) {
    // Find the unit suffix
    const match = trimmed.match(pattern);
    if (match) {
      const numStr = trimmed.slice(0, match.index!).trim();
      const num = Number(numStr);
      if (!isFinite(num) || numStr === '') return null;
      return Math.round(num * nmPerUnit);
    }
  }

  return null;
}
