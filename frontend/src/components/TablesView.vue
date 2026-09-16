<template>
  <div class="tables-view">
    <div v-if="loading" class="loading">Loading tables...</div>

    <div v-if="error" class="error">
      <p>API error: {{ error }}</p>
    </div>

    <div v-else-if="tableGroups.length > 0" class="tables-container">
      <TableView
        v-for="group in tableGroups"
        :key="group.tableName"
        :table-name="group.tableName"
        :columns="group.columns"
      />
    </div>

    <div v-else-if="!loading && tableGroups.length === 0" class="empty">
      No tables found
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue';
import { api } from '../api/client';
import type { ColumnInfo } from '../api/types';
import TableView from './TableView.vue';

interface TableGroup {
  tableName: string;
  columns: ColumnInfo[];
}

const loading = ref(true);
const error = ref<string | null>(null);
const allColumns = ref<ColumnInfo[]>([]);

const tableGroups = computed<TableGroup[]>(() => {
  const grouped: Record<string, ColumnInfo[]> = {};

  allColumns.value.forEach(col => {
    if (!grouped[col.table_name]) {
      grouped[col.table_name] = [];
    }
    grouped[col.table_name].push(col);
  });

  return Object.keys(grouped).map(tableName => ({
    tableName,
    columns: grouped[tableName]
  }));
});

onMounted(async () => {
  try {
    loading.value = true;
    const data = await api.listTables();
    allColumns.value = data;
  } catch (err) {
    error.value = String(err);
  } finally {
    loading.value = false;
  }
});
</script>

<style scoped>
.tables-container {
  margin-top: 2rem;
}

.error {
  color: red;
  padding: 1rem;
  background: #ffe0e0;
  border: 1px solid #ff6b6b;
  border-radius: 4px;
  margin: 1rem 0;
}

.loading {
  color: #666;
  padding: 1rem;
  font-style: italic;
}

.empty {
  padding: 2rem;
  text-align: center;
  color: #666;
}
</style>
