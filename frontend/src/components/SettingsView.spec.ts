// @vitest-environment happy-dom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { mount, flushPromises } from '@vue/test-utils';
import SettingsView from './SettingsView.vue';
import { api } from '../api/client';

vi.mock('../api/client', () => ({
  api: {
    getSettings: vi.fn(),
    updateSettings: vi.fn(),
    uploadFile: vi.fn(),
    testQwenConnection: vi.fn(),
  },
  imageUrl: vi.fn((path: string | null) => (path ? `/api/files/${path}` : null)),
}));

describe('SettingsView', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('renders unconfigured state when user has no settings', async () => {
    vi.mocked(api.getSettings).mockResolvedValue({
      tryon_provider: 'gemini',
      has_api_key: false,
      api_key_hint: null,
      local_qwen_url: 'http://127.0.0.1:8001',
      reference_image_path: null,
    });

    const wrapper = mount(SettingsView);
    await flushPromises();

    expect(wrapper.text()).toContain('Virtual Try-On Settings');
    expect(wrapper.text()).toContain('Not configured');
    expect(wrapper.text()).toContain('No photo uploaded');
  });

  it('renders configured status with hint when settings exist', async () => {
    vi.mocked(api.getSettings).mockResolvedValue({
      tryon_provider: 'gemini',
      has_api_key: true,
      api_key_hint: '...a1b2',
      local_qwen_url: 'http://127.0.0.1:8001',
      reference_image_path: 'users/123/uuid-ref.jpg',
    });

    const wrapper = mount(SettingsView);
    await flushPromises();

    expect(wrapper.text()).toContain('Configured (...a1b2)');
    expect(wrapper.text()).toContain('Photo uploaded');
    expect(wrapper.find('img.reference-img').exists()).toBe(true);
  });

  it('saves an API key when entered', async () => {
    vi.mocked(api.getSettings).mockResolvedValue({
      tryon_provider: 'gemini',
      has_api_key: false,
      api_key_hint: null,
      local_qwen_url: 'http://127.0.0.1:8001',
      reference_image_path: null,
    });
    vi.mocked(api.updateSettings).mockResolvedValue({
      tryon_provider: 'gemini',
      has_api_key: true,
      api_key_hint: '...1234',
      local_qwen_url: 'http://127.0.0.1:8001',
      reference_image_path: null,
    });

    const wrapper = mount(SettingsView);
    await flushPromises();

    const input = wrapper.find('input[type="password"]');
    await input.setValue('AIzaSyTestKey1234');

    const saveBtn = wrapper.findAllComponents({ name: 'MdButton' }).find(b => b.text().includes('Save Key'));
    expect(saveBtn).toBeDefined();
    await saveBtn!.trigger('click');
    await flushPromises();

    expect(api.updateSettings).toHaveBeenCalledWith({ api_key: 'AIzaSyTestKey1234' });
    expect(wrapper.text()).toContain('Configured (...1234)');
  });

  it('switches provider to qwen_local and shows local server configuration', async () => {
    vi.mocked(api.getSettings).mockResolvedValue({
      tryon_provider: 'gemini',
      has_api_key: true,
      api_key_hint: '...a1b2',
      local_qwen_url: 'http://127.0.0.1:8001',
      reference_image_path: null,
    });
    vi.mocked(api.updateSettings).mockResolvedValue({
      tryon_provider: 'qwen_local',
      has_api_key: true,
      api_key_hint: '...a1b2',
      local_qwen_url: 'http://127.0.0.1:8001',
      reference_image_path: null,
    });

    const wrapper = mount(SettingsView);
    await flushPromises();

    const qwenCard = wrapper.findAll('.engine-card').find(c => c.text().includes('Qwen-Image-2.1-Turbo'));
    expect(qwenCard).toBeDefined();
    await qwenCard!.trigger('click');
    await flushPromises();

    expect(api.updateSettings).toHaveBeenCalledWith({ tryon_provider: 'qwen_local' });
    expect(wrapper.text()).toContain('Local Server Configuration');
    expect(wrapper.text()).toContain('Test Connection');
  });

  it('tests local Qwen connection when button is clicked', async () => {
    vi.mocked(api.getSettings).mockResolvedValue({
      tryon_provider: 'qwen_local',
      has_api_key: false,
      api_key_hint: null,
      local_qwen_url: 'http://127.0.0.1:8001',
      reference_image_path: null,
    });
    vi.mocked(api.testQwenConnection).mockResolvedValue({
      status: 'ok',
      model: 'qwen-image-2.1-turbo-Q6_K_XL.gguf',
      ready: true,
    });

    const wrapper = mount(SettingsView);
    await flushPromises();

    const testBtn = wrapper.findAllComponents({ name: 'MdButton' }).find(b => b.text().includes('Test Connection'));
    expect(testBtn).toBeDefined();
    await testBtn!.trigger('click');
    await flushPromises();

    expect(api.testQwenConnection).toHaveBeenCalledWith('http://127.0.0.1:8001');
    expect(wrapper.text()).toContain('Connected to Qwen/Qwen-Image-2.1-Turbo');
  });
});

