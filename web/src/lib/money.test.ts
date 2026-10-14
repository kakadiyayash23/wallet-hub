import { describe, expect, it } from 'vitest'
import { formatMinor, formatSigned, normalizeAmountInput } from './money'

describe('money formatting', () => {
  it('formats minor units per locale', () => {
    expect(formatMinor(124750, 'EUR', 'en')).toBe('€1,247.50')
    expect(formatMinor(124750, 'EUR', 'de').replace(/ /g, ' ')).toBe('1.247,50 €')
  })
  it('keeps the sign visible in ledgers', () => {
    expect(formatSigned(600, 'EUR', 'en')).toBe('+€6.00')
    expect(formatSigned(-250, 'EUR', 'en')).toBe('−€2.50')
    expect(formatSigned(0, 'EUR', 'en')).toBe('€0.00')
  })
  it('accepts comma and dot input', () => {
    expect(normalizeAmountInput('12,5')).toBe('12.50')
    expect(normalizeAmountInput('50')).toBe('50.00')
    expect(normalizeAmountInput('1.005')).toBeNull()
    expect(normalizeAmountInput('abc')).toBeNull()
  })
})
