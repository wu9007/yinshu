// @vitest-environment jsdom
import { defineComponent, nextTick } from 'vue';
import { flushPromises, shallowMount } from '@vue/test-utils';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const api = vi.hoisted(() => ({
  exportConfigFile: vi.fn(),
  exportDiagnostics: vi.fn(),
  fetchPapers: vi.fn(),
  fetchPrinters: vi.fn(),
  clearTaskHistory: vi.fn(),
  getConfig: vi.fn(),
  getTaskHistory: vi.fn(),
  getTaskHistoryEvents: vi.fn(),
  importConfigFile: vi.fn(),
  isDebugBuild: vi.fn(),
  printTestPage: vi.fn(),
  previewConfigImport: vi.fn(),
  getLanAddress: vi.fn(),
  saveConfig: vi.fn(),
}));

vi.mock('@/api', () => api);
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn(), save: vi.fn() }));
vi.mock('@tauri-apps/plugin-process', () => ({ relaunch: vi.fn() }));
vi.mock('@/onboarding', () => ({ useOnboarding: () => ({}) }));

import App from './App.vue';
import { i18n, setI18nLocale } from '@/i18n';
import type { AgentConfig } from '@/types';

const config: AgentConfig = {
  service: { host: '127.0.0.1', port: 17890 },
  security: { allowed_origins: [], allowed_ips: ['127.0.0.1'] },
  printing: { default_printer: null, default_paper: null, default_copies: 1 },
  limits: {
    max_file_size_mb: 50,
    max_batch_jobs: 20,
    max_copies: 10,
    download_timeout_seconds: 30,
  },
  app: { autostart: false, language: 'zh-CN' },
};

const StatusDoctorSheetStub = defineComponent({
  name: 'StatusDoctorSheet',
  props: { open: Boolean },
  emits: ['update:open', 'status-change'],
  template: '<div data-testid="doctor-sheet-stub" />',
});

const BadgeStub = defineComponent({
  props: { variant: String },
  template: '<span><slot /></span>',
});

function mountApp() {
  return shallowMount(App, {
    global: {
      plugins: [i18n],
      stubs: { Badge: BadgeStub, StatusDoctorSheet: StatusDoctorSheetStub },
    },
  });
}

describe('App status Doctor integration', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    setI18nLocale('zh-CN');
    Object.defineProperty(window, 'matchMedia', {
      configurable: true,
      value: vi.fn(() => ({
        matches: false,
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
      })),
    });
    api.getConfig.mockResolvedValue(structuredClone(config));
    api.getLanAddress.mockResolvedValue('192.168.1.23');
    api.fetchPrinters.mockResolvedValue([]);
    api.fetchPapers.mockResolvedValue([]);
    api.getTaskHistory.mockResolvedValue([]);
  });

  it('opens the sheet and reflects its attention state', async () => {
    const wrapper = mountApp();
    await flushPromises();

    await wrapper.get('[data-testid="status-doctor-trigger"]').trigger('click');
    expect(wrapper.getComponent(StatusDoctorSheetStub).props('open')).toBe(true);

    wrapper.getComponent(StatusDoctorSheetStub).vm.$emit('status-change', false);
    await nextTick();
    expect(wrapper.get('[data-testid="status-doctor-trigger"]').text()).toContain('服务正常');
    expect(wrapper.getComponent(BadgeStub).props('variant')).toBe('success');

    wrapper.getComponent(StatusDoctorSheetStub).vm.$emit('status-change', true);
    await nextTick();
    expect(wrapper.get('[data-testid="status-doctor-trigger"]').text()).toContain('需处理');
    expect(wrapper.getComponent(BadgeStub).props('variant')).toBe('destructive');
  });

  it('keeps existing page errors higher priority than Doctor readiness', async () => {
    api.getConfig.mockRejectedValueOnce(new Error('config unavailable'));
    const wrapper = mountApp();
    await flushPromises();

    const sheet = wrapper.getComponent(StatusDoctorSheetStub);
    sheet.vm.$emit('status-change', false);
    await nextTick();

    expect(wrapper.get('[data-testid="status-doctor-trigger"]').text()).toContain('需处理');
  });
});
