/**
 * Parse a date string written by SQLite's `datetime('now')`, which emits
 * `YYYY-MM-DD HH:MM:SS` in UTC with no timezone marker. Without the marker,
 * `new Date(...)` interprets the string as local time and the resulting
 * instant is wrong by the local UTC offset (e.g. 8h in UTC+8).
 *
 * Also accepts ISO 8601 strings that already carry a `T` separator and/or a
 * timezone suffix — those are passed through unchanged.
 */
export function parseDbDate(s: string): Date {
  const hasTimezone = /[zZ]|[+-]\d{2}:?\d{2}$/.test(s);
  const normalized = hasTimezone
    ? s
    : s.includes("T")
      ? `${s}Z`
      : `${s.replace(" ", "T")}Z`;
  return new Date(normalized);
}
