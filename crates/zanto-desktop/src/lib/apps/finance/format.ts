/** Format a number as currency using the profile's ISO code; falls back to a
 * plain 2dp number if the code is unknown/empty. */
export function formatCurrency(amount: number | undefined, currency?: string): string {
  const n = amount ?? 0;
  const code = (currency ?? "").trim().toUpperCase();
  if (code) {
    try {
      return new Intl.NumberFormat(undefined, { style: "currency", currency: code }).format(n);
    } catch {
      /* invalid code → fall through */
    }
  }
  return n.toLocaleString(undefined, { maximumFractionDigits: 2 });
}
