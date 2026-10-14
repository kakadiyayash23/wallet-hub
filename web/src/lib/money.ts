/** Money arrives from the api as integer minor units. Formatting is the only place it becomes a decimal. */
export function formatMinor(minor: number, currency: string, locale: string): string {
  return new Intl.NumberFormat(locale, { style: 'currency', currency, minimumFractionDigits: 2 }).format(minor / 100)
}

/** Signed form for ledger tables: "+6,00 €" / "−2,50 €". */
export function formatSigned(minor: number, currency: string, locale: string): string {
  const abs = formatMinor(Math.abs(minor), currency, locale)
  if (minor > 0) return `+${abs}`
  if (minor < 0) return `−${abs}`
  return abs
}

export function formatDate(iso: string, locale: string): string {
  return new Intl.DateTimeFormat(locale, { dateStyle: 'medium', timeStyle: 'short' }).format(new Date(iso))
}

/** User input ("12,50" or "12.50") to the decimal string the api accepts ("12.50"). */
export function normalizeAmountInput(input: string): string | null {
  const s = input.trim().replace(/\s/g, '').replace(',', '.')
  if (!/^\d+(\.\d{1,2})?$/.test(s)) return null
  return Number(s).toFixed(2)
}
