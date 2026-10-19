<template>
  <div class="image-preview">
    <!-- Single image preview -->
    <div v-if="imageUrl" class="preview-container">
      <img 
        :src="imageUrl" 
        alt="Item image preview"
        @error="handleImageError"
      />
      
      <!-- Overlay with replace button -->
      <button 
        type="button"
        class="replace-btn"
        title="Replace image"
        @click="$emit('replace')"
      >
        ✕
      </button>
    </div>

    <!-- Empty state hint -->
    <div v-else-if="showHint" class="preview-hint">
      Image preview will appear here
    </div>

    <!-- Error state -->
    <div v-else-if="error" class="preview-error">
      Failed to load image. Please try again.
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue';

const props = defineProps<{
  imageUrl?: string;
  showHint?: boolean;
}>();

const emit = defineEmits<{
  replace: [];
}>();

function handleImageError() {
  // Could emit an error event if needed
}
</script>

<style scoped>
.image-preview {
  position: relative;
  display: inline-block;
  max-width: 100%;
}

.preview-container {
  position: relative;
  width: 80px;
  height: 80px;
  border-radius: var(--md-shape-medium);
  overflow: hidden;
  background: #f5f5f5;
  border: 1px solid #ddd;
}

.preview-container img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.replace-btn {
  position: absolute;
  top: 4px;
  right: 4px;
  width: 24px;
  height: 24px;
  border-radius: 50%;
  background: rgba(0, 0, 0, 0.6);
  color: white;
  border: none;
  font-size: 14px;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
}

.replace-btn:hover {
  background: rgba(0, 0, 0, 0.8);
}

.preview-hint {
  padding: 1rem;
  text-align: center;
  color: #999;
  font-size: var(--md-label-large-size);
  font-style: italic;
}

.preview-error {
  padding: 0.5rem;
  background: #ffebee;
  border-left: 3px solid var(--md-error);
  color: var(--md-on-error);
  font-size: var(--md-label-large-size);
}
</style>