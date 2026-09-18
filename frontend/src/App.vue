<template>
  <div id="app">
    <div v-if="!ready" class="loading">Loading…</div>

    <LoginForm v-else-if="!session" />

    <template v-else>
      <MdAppBar
        title="Cloth Web"
        elevation="1"
        @nav-click="drawerOpen = !drawerOpen"
      >
        <template #actions>
          <span class="who">{{ user?.email }}</span>
          <MdButton variant="text" @click="signOut">Sign out</MdButton>
        </template>
      </MdAppBar>

      <div class="shell">
        <MdNavDrawer v-model="activeTab" :open="drawerOpen" :items="navItems">
          <template #header>
            <div class="drawer-brand">
              <span class="drawer-brand__name">Cloth Web</span>
            </div>
          </template>
        </MdNavDrawer>

        <main class="content">
          <ClothesView v-if="activeTab === 'clothes'" />
          <OutfitsView v-else-if="activeTab === 'outfits'" />
          <InsightsView v-else-if="activeTab === 'insights'" />
          <TablesView v-else-if="activeTab === 'tables'" />
        </main>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { defineAsyncComponent, ref } from 'vue';
import { useAuth } from './lib/auth';
import ClothesView from './components/ClothesView.vue';
import OutfitsView from './components/OutfitsView.vue';
import TablesView from './components/TablesView.vue';
import LoginForm from './components/LoginForm.vue';
import { MdAppBar, MdNavDrawer, MdButton, type DrawerItem } from './ui';

// Chart.js is only needed by the Insights tab — keep it out of the main bundle.
const InsightsView = defineAsyncComponent(() => import('./components/InsightsView.vue'));

const { session, user, ready, signOut } = useAuth();

const navItems: DrawerItem[] = [
  { id: 'clothes', label: 'Clothes', icon: 'grid' },
  { id: 'outfits', label: 'Outfits', icon: 'shirt' },
  { id: 'insights', label: 'Insights', icon: 'insights' },
  { id: 'tables', label: 'Tables', icon: 'table' },
];

type TabId = (typeof navItems)[number]['id'];

const activeTab = ref<TabId>('outfits');
const drawerOpen = ref(true);
</script>

<style scoped>
#app {
  display: flex;
  flex-direction: column;
  height: 100vh;
}

.loading {
  color: var(--md-on-surface-variant);
  padding: 2rem;
  font-style: italic;
}

.shell {
  display: flex;
  flex: 1;
  min-height: 0;
}

.content {
  flex: 1;
  min-width: 0;
  overflow-y: auto;
  padding: 2rem;
}

.who {
  color: var(--md-on-surface-variant);
  font-size: var(--md-body-medium-size);
  line-height: var(--md-body-medium-line);
  max-width: 240px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.drawer-brand {
  padding: 8px;
}

.drawer-brand__name {
  font-size: var(--md-title-medium-size);
  line-height: var(--md-title-medium-line);
  font-weight: var(--md-title-medium-weight);
  color: var(--md-on-surface);
}
</style>
