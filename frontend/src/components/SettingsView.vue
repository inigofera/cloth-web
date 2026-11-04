<template>
  <div class="settings-view">
    <div class="settings-header">
      <h1 class="settings-title">Virtual Try-On Settings</h1>
      <p class="settings-subtitle">
        Configure your Gemini API key and full-body reference photo to enable AI-powered virtual try-on previews.
      </p>
    </div>

    <div v-if="loading" class="loading">Loading settings…</div>
    <div v-else-if="fetchError" class="error-wrapper">
      <ErrorDisplay :message="fetchError" type="network" can-retry @retry="loadSettings" />
    </div>

    <div v-else class="settings-sections">
      <!-- Action Error -->
      <ErrorDisplay
        v-if="actionError"
        :message="actionError"
        dismissible
        class="action-error"
        @dismiss="actionError = null"
      />

      <!-- Section 1: Try-On Engine & Inference Provider -->
      <section class="settings-card">
        <div class="card-header">
          <div class="card-title-group">
            <h2 class="card-title">Try-On Engine</h2>
            <div
              :class="['status-badge', currentProvider === 'gemini' ? (settings?.has_api_key ? 'status-badge--configured' : 'status-badge--missing') : (qwenTestSuccess ? 'status-badge--configured' : 'status-badge--missing')]"
            >
              {{ currentProvider === 'gemini'
                  ? (settings?.has_api_key ? `Gemini Configured (${settings.api_key_hint || 'active'})` : 'Gemini Key Missing')
                  : (qwenTestSuccess ? `Connected: ${qwenModelName || 'Qwen-Image-2.1-Turbo'}` : 'Local Qwen') }}
            </div>
          </div>
          <p class="card-description">
            Choose whether to generate virtual try-on previews using Google Gemini API in the cloud or a locally hosted Qwen-Image-2.1-Turbo model.
          </p>
        </div>

        <!-- Engine Selector: Radio Cards -->
        <div class="engine-selector">
          <button
            type="button"
            class="engine-card"
            :class="{ 'engine-card--active': currentProvider === 'gemini' }"
            @click="switchProvider('gemini')"
          >
            <div class="engine-card-header">
              <span class="engine-icon">✨</span>
              <span class="engine-title">Nano Banana</span>
              <span class="engine-tag">Google Gemini API</span>
            </div>
            <p class="engine-desc">
              Cloud generation with <code>gemini-2.5-flash-image</code>. Fast, requires an API key.
            </p>
          </button>

          <button
            type="button"
            class="engine-card"
            :class="{ 'engine-card--active': currentProvider === 'qwen_local' }"
            @click="switchProvider('qwen_local')"
          >
            <div class="engine-card-header">
              <span class="engine-icon">🖥️</span>
              <span class="engine-title">Qwen-Image-2.1-Turbo</span>
              <span class="engine-tag">Self-Hosted GGUF</span>
            </div>
            <p class="engine-desc">
              8-step accelerated local inference with <code>sd-cli</code>. Fully private, no API key needed.
            </p>
          </button>
        </div>

        <!-- Option A: Gemini Settings -->
        <div v-if="currentProvider === 'gemini'" class="provider-panel">
          <div class="subpanel-header">
            <h3 class="subpanel-title">Gemini API Key (BYOK)</h3>
            <div
              :class="['status-badge', settings?.has_api_key ? 'status-badge--configured' : 'status-badge--missing']"
            >
              {{ settings?.has_api_key ? `Configured (${settings.api_key_hint || 'active'})` : 'Not configured' }}
            </div>
          </div>
          <p class="card-description">
            Your API key is used exclusively to generate try-on images with <code>gemini-2.5-flash-image</code>.
            It is encrypted at rest using AES-256-GCM on the server and is never shared.
          </p>

          <form class="key-form" @submit.prevent="saveApiKey">
            <div class="field-row">
              <input
                v-model="apiKeyInput"
                type="password"
                class="text-input"
                aria-label="Gemini API key"
                placeholder="Paste your Gemini API key (AIzaSy...)"
                :disabled="savingKey"
              />
              <div class="button-group">
                <MdButton
                  variant="filled"
                  :disabled="savingKey || !apiKeyInput.trim()"
                  @click="saveApiKey"
                >
                  {{ savingKey ? 'Saving…' : 'Save Key' }}
                </MdButton>
                <MdButton
                  v-if="settings?.has_api_key"
                  variant="outlined"
                  :disabled="savingKey"
                  @click="clearApiKey"
                >
                  Clear Key
                </MdButton>
              </div>
            </div>
          </form>

          <div class="instructions-box">
            <button
              type="button"
              class="instructions-toggle"
              @click="showInstructions = !showInstructions"
            >
              <span>{{ showInstructions ? '▼ Hide instructions' : '▶ How to get a Gemini API key' }}</span>
            </button>
            <div v-if="showInstructions" class="instructions-content">
              <ol class="instructions-list">
                <li>
                  Visit
                  <a
                    href="https://aistudio.google.com/apikey"
                    target="_blank"
                    rel="noopener noreferrer"
                    class="link"
                  >Google AI Studio</a>.
                </li>
                <li>
                  Create an API key in a project with billing enabled for multimodal image generation (<code>gemini-2.5-flash-image</code>).
                </li>
                <li>
                  Copy and paste the key into the input above and click <strong>Save Key</strong>.
                </li>
              </ol>
            </div>
          </div>
        </div>

        <!-- Option B: Local Qwen Settings -->
        <div v-else class="provider-panel">
          <div class="subpanel-header">
            <h3 class="subpanel-title">Local Server Configuration</h3>
            <div
              v-if="qwenTestSuccess"
              class="status-badge status-badge--configured"
            >
              Connected to {{ qwenModelName || 'Qwen/Qwen-Image-2.1-Turbo' }}
            </div>
            <div
              v-else
              class="status-badge status-badge--missing"
            >
              {{ testingConnection ? 'Testing…' : 'Not verified' }}
            </div>
          </div>
          <p class="card-description">
            Cloth-web sends inference requests to your local Python microservice endpoint.
          </p>

          <form class="key-form" @submit.prevent="saveQwenUrl">
            <div class="field-row">
              <input
                v-model="qwenUrlInput"
                type="url"
                class="text-input"
                aria-label="Local Qwen server URL"
                placeholder="http://127.0.0.1:8001"
                :disabled="savingQwenUrl || testingConnection"
              />
              <div class="button-group">
                <MdButton
                  variant="filled"
                  :disabled="savingQwenUrl || !qwenUrlInput.trim() || qwenUrlInput.trim() === settings?.local_qwen_url"
                  @click="saveQwenUrl"
                >
                  {{ savingQwenUrl ? 'Saving…' : 'Save URL' }}
                </MdButton>
                <MdButton
                  variant="tonal"
                  :disabled="testingConnection || !qwenUrlInput.trim()"
                  @click="runTestQwenConnection"
                >
                  {{ testingConnection ? 'Testing…' : 'Test Connection' }}
                </MdButton>
              </div>
            </div>
          </form>

          <!-- Qwen Test Feedback -->
          <div v-if="qwenTestError" class="test-error-box">
            <ErrorDisplay
              :message="qwenTestError"
              type="network"
              dismissible
              @dismiss="qwenTestError = null"
            />
          </div>

          <div v-if="qwenTestSuccess" class="connection-success-alert">
            <span class="success-icon">✓</span>
            <div class="success-details">
              <strong>Connected to Qwen/Qwen-Image-2.1-Turbo</strong>
              <span class="success-sub">Worker is online and ready for virtual try-on inference.</span>
            </div>
          </div>

          <!-- Setup Guide -->
          <div class="instructions-box">
            <button
              type="button"
              class="instructions-toggle"
              @click="showQwenInstructions = !showQwenInstructions"
            >
              <span>{{ showQwenInstructions ? '▼ Hide setup guide' : '▶ Local Qwen-Image-2.1-Turbo Setup Guide' }}</span>
            </button>
            <div v-if="showQwenInstructions" class="instructions-content">
              <p class="guide-step-title">1. Start the local inference microservice:</p>
              <pre class="code-block"><code>cd local_inference
pip install -r requirements.txt
uvicorn server:app --host 127.0.0.1 --port 8001</code></pre>
              <p class="guide-hardware-note">
                Requires an AMD Radeon (e.g. RX 9070 XT with Vulkan / ROCm) or NVIDIA GPU with ≥16GB VRAM (uses CPU offload and VAE tiling). Uses the 8-step Turbo schedule.
              </p>
            </div>
          </div>
        </div>
      </section>

      <!-- Section 2: Full-Body Reference Photo -->
      <section class="settings-card">
        <div class="card-header">
          <div class="card-title-group">
            <h2 class="card-title">Full-Body Reference Photo</h2>
            <div
              :class="['status-badge', settings?.reference_image_path ? 'status-badge--configured' : 'status-badge--missing']"
            >
              {{ settings?.reference_image_path ? 'Photo uploaded' : 'No photo uploaded' }}
            </div>
          </div>
          <p class="card-description">
            Upload a clear full-body photo of yourself. The virtual try-on engine replaces the clothes worn in this photo with your selected wardrobe items.
          </p>
        </div>

        <div class="reference-photo-layout">
          <!-- Preview container -->
          <div class="photo-preview-column">
            <!-- Hidden file input referenced by ref -->
            <input
              ref="fileInputRef"
              type="file"
              accept="image/jpeg,image/png,image/webp,image/gif"
              class="file-input-hidden"
              :disabled="uploadingPhoto"
              @change="handleFileInputChange"
            />

            <div v-if="settings?.reference_image_path" class="reference-preview">
              <img
                :src="previewUrl"
                alt="Full-body reference preview"
                class="reference-img"
              />
              <div class="preview-actions">
                <MdButton
                  variant="filled"
                  :disabled="uploadingPhoto"
                  @click="openFilePicker"
                >
                  {{ uploadingPhoto ? 'Uploading…' : 'Replace Photo' }}
                </MdButton>
                <MdButton
                  variant="outlined"
                  :disabled="uploadingPhoto"
                  @click="clearReferencePhoto"
                >
                  Remove
                </MdButton>
              </div>
            </div>

            <div v-else class="upload-placeholder">
              <div
                class="drop-zone"
                :class="{ 'drop-zone--dragging': isDragging, 'drop-zone--uploading': uploadingPhoto }"
                role="button"
                tabindex="0"
                @click="openFilePicker"
                @keydown.enter.space.prevent="openFilePicker"
                @dragover.prevent="onDragOver"
                @dragleave.prevent="onDragLeave"
                @drop.prevent="onDrop"
              >
                <span class="upload-icon" aria-hidden="true">{{ uploadingPhoto ? '⏳' : '📷' }}</span>
                <span class="upload-primary-text">
                  {{ uploadingPhoto ? 'Uploading and optimizing…' : 'Click or drop reference photo' }}
                </span>
                <span class="upload-secondary-text">JPEG, PNG, WebP up to 10MB</span>
              </div>
            </div>

            <ErrorDisplay
              v-if="photoError"
              :message="photoError"
              dismissible
              class="photo-error"
              @dismiss="photoError = null"
            />
          </div>

          <!-- Guidelines Checklist -->
          <div class="guidelines-card">
            <h3 class="guidelines-title">Photo Guidelines</h3>
            <ul class="guidelines-list">
              <li>
                <span class="check-icon">✓</span>
                <span><strong>Form-fitting base clothing:</strong> Wear a plain t-shirt and shorts or leggings. Avoid bulky jackets, coats, or loose dresses.</span>
              </li>
              <li>
                <span class="check-icon">✓</span>
                <span><strong>Natural upright pose:</strong> Stand facing the camera with your arms relaxed at your sides (not crossed or hidden in pockets).</span>
              </li>
              <li>
                <span class="check-icon">✓</span>
                <span><strong>Full body visibility:</strong> Ensure your complete body is visible from head to shoes with adequate lighting against a plain background.</span>
              </li>
            </ul>
          </div>
        </div>
      </section>
    </div>

    <Toast v-model:show="showToast" :message="toastMessage" />
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue';
import { api, imageUrl } from '../api/client';
import { sanitizeText, validateImageFile } from '../lib/sanitize';
import { classifyError } from '../lib/errors';
import { MdButton, ErrorDisplay, Toast } from '../ui';
import type { TryOnProvider, UserSettings } from '../api/types';

const loading = ref(true);
const fetchError = ref<string | null>(null);
const actionError = ref<string | null>(null);
const photoError = ref<string | null>(null);
const settings = ref<UserSettings | null>(null);

const currentProvider = ref<TryOnProvider>('gemini');
const qwenUrlInput = ref('http://127.0.0.1:8001');
const savingQwenUrl = ref(false);
const testingConnection = ref(false);
const qwenTestSuccess = ref(false);
const qwenTestError = ref<string | null>(null);
const qwenModelName = ref<string | null>(null);
const showQwenInstructions = ref(true);

const fileInputRef = ref<HTMLInputElement | null>(null);
const isDragging = ref(false);
const apiKeyInput = ref('');
const savingKey = ref(false);
const uploadingPhoto = ref(false);
const showInstructions = ref(false);

const showToast = ref(false);
const toastMessage = ref('');

const previewUrl = computed(() => {
  if (!settings.value?.reference_image_path) return '';
  return imageUrl(settings.value.reference_image_path, { width: 720 }) || '';
});

function notify(message: string) {
  toastMessage.value = message;
  showToast.value = true;
}

function openFilePicker() {
  if (uploadingPhoto.value) return;
  fileInputRef.value?.click();
}

function onDragOver() {
  if (!uploadingPhoto.value) {
    isDragging.value = true;
  }
}

function onDragLeave() {
  isDragging.value = false;
}

async function onDrop(event: DragEvent) {
  isDragging.value = false;
  if (uploadingPhoto.value) return;
  const file = event.dataTransfer?.files?.[0];
  if (file) {
    await processFile(file);
  }
}

async function loadSettings() {
  loading.value = true;
  fetchError.value = null;
  try {
    settings.value = await api.getSettings();
    currentProvider.value = settings.value.tryon_provider || 'gemini';
    qwenUrlInput.value = settings.value.local_qwen_url || 'http://127.0.0.1:8001';
  } catch (err) {
    fetchError.value = classifyError(err).message;
  } finally {
    loading.value = false;
  }
}

async function switchProvider(provider: TryOnProvider) {
  if (provider === currentProvider.value && provider === settings.value?.tryon_provider) return;
  currentProvider.value = provider;
  actionError.value = null;
  try {
    settings.value = await api.updateSettings({ tryon_provider: provider });
    notify(`Try-on engine switched to ${provider === 'gemini' ? 'Nano Banana (Gemini API)' : 'Local Qwen-Image'}.`);
  } catch (err) {
    actionError.value = classifyError(err).message;
    if (settings.value) {
      currentProvider.value = settings.value.tryon_provider;
    }
  }
}

async function saveQwenUrl() {
  const url = qwenUrlInput.value.trim();
  if (!url) {
    actionError.value = 'Please enter a valid URL.';
    return;
  }
  savingQwenUrl.value = true;
  actionError.value = null;
  qwenTestError.value = null;
  try {
    settings.value = await api.updateSettings({ local_qwen_url: url });
    notify('Local server URL saved.');
  } catch (err) {
    actionError.value = classifyError(err).message;
  } finally {
    savingQwenUrl.value = false;
  }
}

async function runTestQwenConnection() {
  testingConnection.value = true;
  qwenTestError.value = null;
  qwenTestSuccess.value = false;
  actionError.value = null;
  const urlToTest = qwenUrlInput.value.trim();
  try {
    const res = await api.testQwenConnection(urlToTest || undefined);
    if (res.ready) {
      qwenTestSuccess.value = true;
      qwenModelName.value = res.model || 'qwen-image-2.1-turbo-Q6_K_XL.gguf';
      notify('Connected to Qwen/Qwen-Image-2.1-Turbo.');
    } else {
      const details: string[] = [];
      if (!res.sd_cli_found) {
        details.push("stable-diffusion.cpp executable ('sd-cli.exe' or 'sd.exe') was not found in PATH or local_inference folder.");
      }
      if (Array.isArray(res.missing_files) && res.missing_files.length > 0) {
        details.push(`Missing model files: ${res.missing_files.join(', ')}`);
      }
      qwenTestError.value = `Local server is running but degraded: ${details.join(' ')}`;
    }
  } catch (err) {
    qwenTestError.value = classifyError(err).message;
  } finally {
    testingConnection.value = false;
  }
}

async function saveApiKey() {
  const sanitized = sanitizeText(apiKeyInput.value, { max: 256 });
  if (!sanitized) {
    actionError.value = 'Please enter a valid API key.';
    return;
  }
  if (sanitized.length < 10) {
    actionError.value = 'API key must be at least 10 characters long.';
    return;
  }

  savingKey.value = true;
  actionError.value = null;
  try {
    settings.value = await api.updateSettings({ api_key: sanitized });
    apiKeyInput.value = '';
    notify('Gemini API key saved securely.');
  } catch (err) {
    actionError.value = classifyError(err).message;
  } finally {
    savingKey.value = false;
  }
}

async function clearApiKey() {
  if (!confirm('Are you sure you want to remove your configured Gemini API key?')) {
    return;
  }
  savingKey.value = true;
  actionError.value = null;
  try {
    settings.value = await api.updateSettings({ api_key: '' });
    apiKeyInput.value = '';
    notify('API key removed.');
  } catch (err) {
    actionError.value = classifyError(err).message;
  } finally {
    savingKey.value = false;
  }
}

async function handleFileInputChange(event: Event) {
  const target = event.target as HTMLInputElement;
  const file = target.files?.[0];
  if (file) {
    await processFile(file);
  }
  target.value = '';
}

async function processFile(file: File) {
  photoError.value = null;
  actionError.value = null;
  uploadingPhoto.value = true;

  try {
    const validation = await validateImageFile(file);
    if (!validation.ok || !validation.file) {
      photoError.value = validation.error || 'Invalid image file.';
      return;
    }

    const uploadedPath = await api.uploadFile(validation.file);
    settings.value = await api.updateSettings({ reference_image_path: uploadedPath });
    notify('Reference photo uploaded successfully.');
  } catch (err) {
    photoError.value = classifyError(err).message;
  } finally {
    uploadingPhoto.value = false;
  }
}

async function clearReferencePhoto() {
  if (!confirm('Are you sure you want to remove your reference photo?')) {
    return;
  }
  uploadingPhoto.value = true;
  photoError.value = null;
  actionError.value = null;
  try {
    settings.value = await api.updateSettings({ reference_image_path: '' });
    notify('Reference photo removed.');
  } catch (err) {
    photoError.value = classifyError(err).message;
  } finally {
    uploadingPhoto.value = false;
  }
}

onMounted(loadSettings);
</script>

<style scoped>
.settings-view {
  max-width: 900px;
  margin: 0 auto;
  padding: 1rem 0 3rem;
}

.settings-header {
  margin-bottom: 2rem;
}

.settings-title {
  font-size: var(--md-headline-medium-size, 1.75rem);
  font-weight: 700;
  color: var(--md-on-surface);
  margin: 0 0 0.5rem;
}

.settings-subtitle {
  color: var(--md-on-surface-variant);
  font-size: var(--md-body-large-size, 1rem);
  line-height: 1.5;
  margin: 0;
}

.settings-sections {
  display: flex;
  flex-direction: column;
  gap: 2rem;
}

.action-error {
  margin-bottom: 1rem;
}

.settings-card {
  background: var(--md-surface, #fff);
  border: 1px solid var(--md-outline-variant, #e0e0e0);
  border-radius: var(--md-shape-large, 16px);
  padding: 1.5rem;
  box-shadow: var(--md-elevation-1, 0 1px 3px rgba(0, 0, 0, 0.05));
}

.card-header {
  margin-bottom: 1.25rem;
}

.card-title-group {
  display: flex;
  align-items: center;
  gap: 1rem;
  margin-bottom: 0.5rem;
  flex-wrap: wrap;
}

.card-title {
  font-size: var(--md-title-large-size, 1.25rem);
  font-weight: 600;
  color: var(--md-on-surface);
  margin: 0;
}

.card-description {
  color: var(--md-on-surface-variant);
  font-size: var(--md-body-medium-size, 0.875rem);
  line-height: 1.45;
  margin: 0;
}

.status-badge {
  font-size: 0.75rem;
  font-weight: 600;
  padding: 0.25rem 0.65rem;
  border-radius: var(--md-shape-full, 999px);
  display: inline-flex;
  align-items: center;
}

.status-badge--configured {
  background: #e8f5e9;
  color: #2e7d32;
  border: 1px solid #c8e6c9;
}

.status-badge--missing {
  background: #fff3e0;
  color: #e65100;
  border: 1px solid #ffe0b2;
}

.engine-selector {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(280px, 1fr));
  gap: 1rem;
  margin: 1.25rem 0;
}

.engine-card {
  text-align: left;
  background: var(--md-surface, #fff);
  border: 2px solid var(--md-outline-variant, #e0e0e0);
  border-radius: var(--md-shape-medium, 12px);
  padding: 1.25rem;
  cursor: pointer;
  transition: all 0.2s ease;
  outline: none;
}

.engine-card:hover {
  border-color: var(--md-primary, #6750a4);
  background: var(--md-surface-container-high, #f8f9fa);
}

.engine-card--active {
  border-color: var(--md-primary, #6750a4);
  background: var(--md-primary-container, #f3edf7);
  box-shadow: 0 0 0 1px var(--md-primary, #6750a4);
}

.engine-card-header {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  margin-bottom: 0.5rem;
  flex-wrap: wrap;
}

.engine-icon {
  font-size: 1.25rem;
}

.engine-title {
  font-size: 1rem;
  font-weight: 700;
  color: var(--md-on-surface);
}

.engine-tag {
  font-size: 0.75rem;
  background: rgba(0, 0, 0, 0.06);
  padding: 0.15rem 0.5rem;
  border-radius: 4px;
  color: var(--md-on-surface-variant);
}

.engine-desc {
  font-size: 0.85rem;
  color: var(--md-on-surface-variant);
  margin: 0;
  line-height: 1.4;
}

.provider-panel {
  padding-top: 1.25rem;
  border-top: 1px solid var(--md-outline-variant, #f0f0f0);
}

.subpanel-header {
  display: flex;
  align-items: center;
  gap: 1rem;
  margin-bottom: 0.5rem;
  flex-wrap: wrap;
}

.subpanel-title {
  font-size: 1.1rem;
  font-weight: 600;
  color: var(--md-on-surface);
  margin: 0;
}

.test-error-box {
  margin-top: 0.75rem;
}

.connection-success-alert {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  margin-top: 0.75rem;
  padding: 0.85rem 1rem;
  background: #e8f5e9;
  border: 1px solid #c8e6c9;
  border-radius: var(--md-shape-small, 8px);
  color: #2e7d32;
}

.success-icon {
  font-size: 1.25rem;
  font-weight: bold;
}

.success-details {
  display: flex;
  flex-direction: column;
  gap: 0.15rem;
  font-size: 0.875rem;
}

.success-sub {
  font-size: 0.8rem;
  color: #388e3c;
}

.guide-step-title {
  font-size: 0.875rem;
  font-weight: 600;
  color: var(--md-on-surface);
  margin: 0 0 0.5rem;
}

.code-block {
  background: #1e1e1e;
  color: #d4d4d4;
  padding: 0.85rem 1rem;
  border-radius: 6px;
  font-family: 'Consolas', 'Monaco', monospace;
  font-size: 0.85rem;
  line-height: 1.5;
  overflow-x: auto;
  margin: 0 0 0.75rem;
}

.code-block code {
  color: inherit;
  background: none;
  padding: 0;
}

.guide-hardware-note {
  font-size: 0.825rem;
  font-style: italic;
  color: var(--md-on-surface-variant);
  margin: 0;
  line-height: 1.45;
}

.field-row {
  display: flex;
  gap: 0.75rem;
  flex-wrap: wrap;
  align-items: center;
}

.text-input {
  flex: 1;
  min-width: 260px;
  height: 44px;
  padding: 0 1rem;
  border: 1px solid var(--md-outline, #79747e);
  border-radius: var(--md-shape-small, 8px);
  font-size: var(--md-body-medium-size, 0.875rem);
  background: var(--md-surface, #fff);
  color: var(--md-on-surface);
  outline: none;
  transition: border-color 0.2s;
}

.text-input:focus {
  border-color: var(--md-primary, #6750a4);
  box-shadow: 0 0 0 1px var(--md-primary, #6750a4);
}

.button-group {
  display: flex;
  gap: 0.5rem;
}

.instructions-box {
  margin-top: 1rem;
  padding-top: 1rem;
  border-top: 1px solid var(--md-outline-variant, #f0f0f0);
}

.instructions-toggle {
  background: none;
  border: none;
  padding: 0;
  color: var(--md-primary, #6750a4);
  font-size: 0.85rem;
  font-weight: 600;
  cursor: pointer;
  text-align: left;
}

.instructions-content {
  margin-top: 0.75rem;
  background: var(--md-surface-container, #f8f9fa);
  padding: 1rem 1.25rem;
  border-radius: var(--md-shape-medium, 12px);
}

.instructions-list {
  margin: 0;
  padding-left: 1.25rem;
  color: var(--md-on-surface-variant);
  font-size: 0.875rem;
  line-height: 1.6;
}

.link {
  color: var(--md-primary, #6750a4);
  text-decoration: underline;
}

.reference-photo-layout {
  display: grid;
  grid-template-columns: 240px 1fr;
  gap: 1.5rem;
  align-items: start;
}

@media (max-width: 700px) {
  .reference-photo-layout {
    grid-template-columns: 1fr;
  }
}

.photo-preview-column {
  display: flex;
  flex-direction: column;
}

.reference-preview {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
  align-items: center;
}

.reference-img {
  width: 220px;
  height: 320px;
  object-fit: cover;
  border-radius: var(--md-shape-medium, 12px);
  border: 1px solid var(--md-outline-variant, #ccc);
  box-shadow: 0 2px 6px rgba(0, 0, 0, 0.08);
}

.preview-actions {
  display: flex;
  gap: 0.5rem;
  flex-wrap: wrap;
  justify-content: center;
}

.file-input-hidden {
  position: absolute;
  width: 1px;
  height: 1px;
  padding: 0;
  margin: -1px;
  overflow: hidden;
  clip: rect(0, 0, 0, 0);
  white-space: nowrap;
  border: 0;
  opacity: 0;
  pointer-events: none;
}

.upload-placeholder {
  width: 220px;
  height: 320px;
}

.drop-zone {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  width: 100%;
  height: 100%;
  border: 2px dashed var(--md-outline-variant, #ccc);
  border-radius: var(--md-shape-medium, 12px);
  padding: 1.5rem 1rem;
  text-align: center;
  cursor: pointer;
  background: var(--md-surface-container, #f8f9fa);
  transition: border-color 0.2s, background-color 0.2s, transform 0.15s ease;
}

.drop-zone:hover {
  border-color: var(--md-primary, #6750a4);
  background: var(--md-surface-container-high, #f1f3f5);
}

.drop-zone--dragging {
  border-color: var(--md-primary, #6750a4);
  background: var(--md-primary-container, #eaddff);
  transform: scale(1.02);
}

.drop-zone--uploading {
  cursor: wait;
  opacity: 0.8;
}

.photo-error {
  margin-top: 0.75rem;
  max-width: 240px;
}

.upload-icon {
  font-size: 2.25rem;
  margin-bottom: 0.5rem;
}

.upload-primary-text {
  font-size: 0.875rem;
  font-weight: 600;
  color: var(--md-on-surface);
  margin-bottom: 0.25rem;
}

.upload-secondary-text {
  font-size: 0.75rem;
  color: var(--md-on-surface-variant);
}

.guidelines-card {
  background: var(--md-surface-container, #f8f9fa);
  border-radius: var(--md-shape-medium, 12px);
  padding: 1.25rem;
}

.guidelines-title {
  font-size: 0.95rem;
  font-weight: 600;
  color: var(--md-on-surface);
  margin: 0 0 0.75rem;
}

.guidelines-list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
}

.guidelines-list li {
  display: flex;
  align-items: flex-start;
  gap: 0.5rem;
  font-size: 0.85rem;
  line-height: 1.45;
  color: var(--md-on-surface-variant);
}

.check-icon {
  color: #2e7d32;
  font-weight: bold;
  flex-shrink: 0;
}

.loading {
  color: var(--md-on-surface-variant);
  font-style: italic;
  padding: 2rem 0;
}

.error-wrapper {
  margin-top: 1rem;
}
</style>

