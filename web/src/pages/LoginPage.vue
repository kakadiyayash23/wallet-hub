<script setup lang="ts">
import { ref } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useSession } from '../stores/session'
import type { Locale } from '../i18n'

const { t } = useI18n()
const session = useSession()
const router = useRouter()
const name = ref('')
const locale = ref<Locale>('en')
const error = ref('')
const busy = ref(false)

async function submit() {
  error.value = ''
  busy.value = true
  try {
    await session.login(name.value, locale.value)
    router.push('/lobby')
  } catch (e) {
    error.value = (e as Error).message
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <div class="card" style="max-width:420px;margin:60px auto">
    <h2 style="margin-top:0">{{ t('login.title') }}</h2>
    <form @submit.prevent="submit" class="grid">
      <label>{{ t('login.name') }}<br /><input id="display_name" v-model="name" minlength="2" maxlength="40" required style="width:100%" /></label>
      <label>{{ t('login.locale') }}<br />
        <select id="login_locale" v-model="locale"><option value="en">English</option><option value="de">Deutsch</option></select>
      </label>
      <button class="primary" :disabled="busy">{{ t('login.enter') }}</button>
      <p class="muted" style="margin:0;font-size:13px">{{ t('login.hint') }}</p>
      <p v-if="error" class="error">{{ error }}</p>
    </form>
  </div>
</template>
