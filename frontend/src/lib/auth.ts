import { computed, onMounted, ref } from 'vue';
import type { Session, User } from '@supabase/supabase-js';
import { supabase } from './supabase';

const session = ref<Session | null>(null);
const ready = ref(false);

let initialized = false;

async function init() {
  if (initialized) return;
  initialized = true;

  const { data } = await supabase.auth.getSession();
  session.value = data.session;

  supabase.auth.onAuthStateChange((_event, newSession) => {
    session.value = newSession;
  });

  ready.value = true;
}

export function useAuth() {
  onMounted(init);

  return {
    session,
    ready,
    user: computed<User | null>(() => session.value?.user ?? null),
    signIn: (email: string, password: string) =>
      supabase.auth.signInWithPassword({ email, password }),
    signUp: (email: string, password: string) =>
      supabase.auth.signUp({ email, password }),
    signInWithOAuth: (provider: 'google' | 'github') =>
      supabase.auth.signInWithOAuth({ provider }),
    signOut: () => supabase.auth.signOut(),
  };
}
