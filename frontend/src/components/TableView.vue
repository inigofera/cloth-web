<template>
  <div class="table-view">
    <h2>{{ tableName }}</h2>
    
    <div v-if="loading" class="loading">Loading rows...</div>
    <div v-else-if="error" class="error">{{ error }}</div>
    <div v-else>
      <div class="columns-info">
        <h3>Columns ({{ columns.length }})</h3>
        <div class="columns-list">
          <span
            v-for="col in columns"
            :key="col.column_name"
            class="column-tag"
          >
            {{ col.column_name }} <span class="data-type">({{ col.data_type }})</span>
          </span>
        </div>
      </div>
      
      <div v-if="rows.length > 0" class="table-preview">
        <h3>First {{ Math.min(rows.length, 5) }} Row(s)</h3>
        <table>
          <thead>
            <tr>
              <th v-for="col in displayedColumns" :key="col">{{ col }}</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="(row, idx) in firstFiveRows" :key="idx">
              <td v-for="col in displayedColumns" :key="col">
                {{ formatValue(row[col]) }}
              </td>
            </tr>
          </tbody>
        </table>
      </div>
      <div v-else class="empty-message">No rows in this table</div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue';
import { api } from '../api/client';
import type { ColumnInfo } from '../api/types';

interface Props {
  tableName: string;
  columns: ColumnInfo[];
}

const props = defineProps<Props>();

const loading = ref(true);
const error = ref<string | null>(null);
const rows = ref<Record<string, unknown>[]>([]);

const displayedColumns = computed(() => {
  return props.columns.map(col => col.column_name);
});

const firstFiveRows = computed(() => {
  return rows.value.slice(0, 5);
});

function formatValue(value: unknown): string {
  if (value === null || value === undefined) {
    return 'NULL';
  }
  if (typeof value === 'object') {
    return JSON.stringify(value);
  }
  return String(value);
}

onMounted(async () => {
  try {
    loading.value = true;
    error.value = null;
    const data = await api.getTableRows(props.tableName);
    rows.value = Array.isArray(data) ? (data as Record<string, unknown>[]) : [];
  } catch (err) {
    error.value = String(err);
  } finally {
    loading.value = false;
  }
});
</script>

<style scoped>
.table-view {
  margin: 2rem 0;
  padding: 1.5rem;
  border: 1px solid #ddd;
  border-radius: 8px;
  background: #f9f9f9;
}

.table-view h2 {
  margin-top: 0;
  color: #333;
  border-bottom: 2px solid #007bff;
  padding-bottom: 0.5rem;
}

.columns-info {
  margin: 1rem 0;
}

.columns-info h3 {
  font-size: 1rem;
  margin-bottom: 0.5rem;
  color: #666;
}

.columns-list {
  display: flex;
  flex-wrap: wrap;
  gap: 0.5rem;
}

.column-tag {
  display: inline-block;
  padding: 0.25rem 0.75rem;
  background: #e3f2fd;
  border: 1px solid #90caf9;
  border-radius: 4px;
  font-size: 0.875rem;
}

.data-type {
  color: #666;
  font-size: 0.75rem;
}

.table-preview {
  margin-top: 1.5rem;
}

.table-preview h3 {
  font-size: 1rem;
  margin-bottom: 0.75rem;
  color: #666;
}

table {
  width: 100%;
  border-collapse: collapse;
  background: white;
  box-shadow: 0 2px 4px rgba(0, 0, 0, 0.1);
}

thead {
  background: #f5f5f5;
}

th {
  padding: 0.75rem;
  text-align: left;
  font-weight: 600;
  border-bottom: 2px solid #ddd;
  color: #333;
}

td {
  padding: 0.75rem;
  border-bottom: 1px solid #eee;
}

tbody tr:hover {
  background: #f9f9f9;
}

.empty-message {
  margin-top: 1rem;
  padding: 1rem;
  background: #fff3cd;
  border: 1px solid #ffc107;
  border-radius: 4px;
  color: #856404;
}

.loading {
  color: #666;
  font-style: italic;
}

.error {
  color: red;
  padding: 1rem;
  background: #ffe0e0;
  border: 1px solid #ff6b6b;
  border-radius: 4px;
}
</style>

