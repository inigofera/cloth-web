<template>
  <div id="app">
    <h1>Cloth Web</h1>

    <div v-if="!ready" class="loading">Loading…</div>

    <LoginForm v-else-if="!session" />

    <template v-else>
      <div class="topbar">
        <span class="who">{{ user?.email }}</span>
        <button class="signout" @click="signOut">Sign out</button>
      </div>

      <nav class="tabs">
        <button
          v-for="tab in tabs"
          :key="tab.id"
          class="tab"
          :class="{ active: activeTab === tab.id }"
          @click="activeTab = tab.id"
        >
          {{ tab.label }}
        </button>
      </nav>

      <OutfitsView v-if="activeTab === 'outfits'" />
      <TablesView v-else-if="activeTab === 'tables'" />
    </template>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import { useAuth } from './lib/auth';
import OutfitsView from './components/OutfitsView.vue';
import TablesView from './components/TablesView.vue';
import LoginForm from './components/LoginForm.vue';

const { session, user, ready, signOut } = useAuth();

const tabs = [
  { id: 'outfits', label: 'Outfits' },
  { id: 'tables', label: 'Tables' }
] as const;

type TabId = (typeof tabs)[number]['id'];

const activeTab = ref<TabId>('outfits');
</script>

<style scoped>
#app {
  max-width: 1200px;
  margin: 0 auto;
  padding: 2rem;
}

h1 {
  margin-bottom: 0.5rem;
}

.topbar {
  display: flex;
  justify-content: flex-end;
  align-items: center;
  gap: 1rem;
  margin-top: 1rem;
  font-size: 0.9rem;
}

.topbar .who {
  color: #555;
}

.topbar .signout {
  padding: 0.35rem 0.75rem;
  border: 1px solid #ccc;
  border-radius: 4px;
  background: #fff;
  cursor: pointer;
}

.tabs {
  display: flex;
  gap: 0.25rem;
  margin-top: 1.5rem;
  border-bottom: 2px solid #ddd;
}

.tab {
  padding: 0.5rem 1.25rem;
  border: none;
  border-bottom: 2px solid transparent;
  margin-bottom: -2px;
  background: none;
  font-size: 0.95rem;
  color: #555;
  cursor: pointer;
}

.tab:hover {
  color: #007bff;
}

.tab.active {
  color: #007bff;
  font-weight: 600;
  border-bottom-color: #007bff;
}

.loading {
  color: #666;
  padding: 1rem;
  font-style: italic;
}
</style>
