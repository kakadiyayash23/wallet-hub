import { createI18n } from 'vue-i18n'
import en from './en.json'
import de from './de.json'

export type Locale = 'en' | 'de'

export const i18n = createI18n({
  legacy: false,
  locale: 'en',
  fallbackLocale: 'en',
  messages: { en, de },
})

export function setLocale(locale: Locale) {
  i18n.global.locale.value = locale
  document.documentElement.lang = locale
}
