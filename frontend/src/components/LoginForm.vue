<template>
  <div class="login-card">
    <h2>{{ mode === 'signin' ? 'Sign in' : 'Create account' }}</h2>

    <form @submit.prevent="submit">
      <label>
        Email
        <input v-model.trim="email" type="email" required autocomplete="email" />
      </label>
      <label>
        Password
        <input v-model="password" type="password" required :autocomplete="mode === 'signin' ? 'current-password' : 'new-password'" />
      </label>

      <p v-if="error" class="error">{{ error }}</p>
      <p v-if="info" class="info">{{ info }}</p>

      <button type="submit" :disabled="busy">
        {{ busy ? 'Please wait…' : mode === 'signin' ? 'Sign in' : 'Sign up' }}
      </button>
    </form>

    <button class="oauth" :disabled="busy" @click="oauth">Continue with Google</button>

    <p class="switch">
      {{ mode === 'signin' ? "Don't have an account?" : 'Already have an account?' }}
      <a href="#" @click.prevent="toggleMode">{{ mode === 'signin' ? 'Sign up' : 'Sign in' }}</a>
    </p>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import { useAuth } from '../lib/auth';
import { isValidEmail, PASSWORD_MAX } from '../lib/sanitize';

const { signIn, signUp, signInWithOAuth } = useAuth();

const mode = ref<'signin' | 'signup'>('signin');
const email = ref('');
const password = ref('');
const error = ref<string | null>(null);
const info = ref<string | null>(null);
const busy = ref(false);

function toggleMode() {
  mode.value = mode.value === 'signin' ? 'signup' : 'signin';
  error.value = null;
  info.value = null;
}

async function submit() {
  error.value = null;
  info.value = null;
  if (!isValidEmail(email.value)) {
    error.value = 'Please enter a valid email address.';
    return;
  }
  if (!password.value || password.value.length > PASSWORD_MAX) {
    error.value = 'Please enter a valid password.';
    return;
  }
  busy.value = true;
  try {
    if (mode.value === 'signin') {
      const { error: authError } = await signIn(email.value, password.value);
      if (authError) throw authError;
    } else {
      const { data, error: authError } = await signUp(email.value, password.value);
      if (authError) throw authError;
      if (data.user && !data.session) {
        info.value = 'Account created. Check your email to confirm it, then sign in.';
      }
    }
  } catch (err) {
    error.value = err instanceof Error ? err.message : String(err);
  } finally {
    busy.value = false;
  }
}

async function oauth() {
  busy.value = true;
  error.value = null;
  const { error: authError } = await signInWithOAuth('google');
  if (authError) {
    error.value = authError.message;
    busy.value = false;
  }
}
</script>

<style scoped>
.login-card {
  max-width: 360px;
  margin: 3rem auto;
  padding: 2rem;
  border: 1px solid #ddd;
  border-radius: 8px;
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

h2 {
  margin: 0;
}

form {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
}

label {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
  font-size: 0.9rem;
}

input {
  padding: 0.5rem;
  border: 1px solid #ccc;
  border-radius: 4px;
}

button {
  padding: 0.5rem;
  border: none;
  border-radius: 4px;
  background: #333;
  color: white;
  cursor: pointer;
}

button:disabled {
  opacity: 0.6;
  cursor: default;
}

button.oauth {
  background: #fff;
  color: #333;
  border: 1px solid #ccc;
}

.error {
  color: #c0392b;
  font-size: 0.85rem;
  margin: 0;
}

.info {
  color: #27ae60;
  font-size: 0.85rem;
  margin: 0;
}

.switch {
  font-size: 0.85rem;
  margin: 0;
  text-align: center;
}
</style>
