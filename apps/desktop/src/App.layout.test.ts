// @vitest-environment jsdom
import { defineComponent, nextTick } from 'vue';
import { flushPromises, mount } from '@vue/test-utils';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const api = vi.hoisted(() => ({
  exportConfigFile: vi.fn(),
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

import { open } from '@tauri-apps/plugin-dialog';
import App from './App.vue';
import { i18n, setI18nLocale } from '@/i18n';
import type { AgentConfig, TaskHistoryJob } from '@/types';

const config: AgentConfig = {
  service: { host: '127.0.0.1', port: 17890 },
  security: { allowed_origins: ['https://erp.example.com'], allowed_ips: ['127.0.0.1'] },
  printing: {
    default_printer: 'Zebra_GX430t',
    default_paper: { width_mm: 60, height_mm: 40 },
    default_copies: 1,
  },
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

function job(overrides: Partial<TaskHistoryJob>): TaskHistoryJob {
  return {
    job_id: 'job-1',
    request_id: null,
    batch_id: null,
    source: 'web_socket',
    current_status: 'submitted',
    current_message: null,
    printer_name: 'Zebra_GX430t',
    paper_name: '60x40',
    copies: 1,
    created_at: '2026-09-15T06:00:00.000Z',
    updated_at: '2026-09-15T06:00:00.000Z',
    finished_at: null,
    ...overrides,
  };
}

function mountApp() {
  return mount(App, {
    global: {
      plugins: [i18n],
      stubs: {
        StatusDoctorSheet: StatusDoctorSheetStub,
        Select: true,
        SelectTrigger: true,
        SelectContent: true,
        SelectItem: true,
        SelectValue: true,
      },
    },
  });
}

describe('App 单页状态面板', () => {
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
    api.fetchPrinters.mockResolvedValue([
      {
        name: 'Zebra_GX430t',
        is_default: true,
        dpi: null,
        port: null,
        is_local: true,
        is_network: false,
        is_virtual: false,
        availability: 'unavailable',
      },
    ]);
    api.fetchPapers.mockResolvedValue([]);
    api.getLanAddress.mockResolvedValue('192.168.1.23');
    api.getTaskHistory.mockResolvedValue([
      job({ job_id: 'job-1', source: 'test', updated_at: '2026-09-15T06:02:00.000Z' }),
      job({ job_id: 'job-2', current_status: 'failed', updated_at: '2026-09-15T05:58:00.000Z' }),
      job({ job_id: 'job-3', updated_at: '2026-09-15T05:50:00.000Z' }),
      job({ job_id: 'job-4', updated_at: '2026-09-15T05:40:00.000Z' }),
    ]);
  });

  it('取消平级标签，按打印、谁可以连接、最近、更多排列', async () => {
    const wrapper = mountApp();
    await flushPromises();

    expect(wrapper.find('[data-testid="print-section"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="access-section"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="recent-section"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="more-toggle"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="computers-section"]').exists()).toBe(false);
    expect(wrapper.text()).not.toContain('YinShu');
    expect(wrapper.find('#service-port').exists()).toBe(false);
    expect(wrapper.find('[data-testid="export-config"]').exists()).toBe(false);
    expect(wrapper.get('[data-testid="access-websites-tab"]').text()).toContain('网站');
    expect(wrapper.get('[data-testid="access-devices-tab"]').text()).toContain('设备');
    expect(wrapper.get('[data-testid="access-qr-button"]').text()).toContain('二维码');
    expect(wrapper.find('[data-testid="add-device"]').exists()).toBe(false);
    expect(wrapper.find('[data-testid="connection-overlay"]').exists()).toBe(false);
  });

  it('主栏不再被 max-w-md 卡住', async () => {
    const wrapper = mountApp();
    await flushPromises();
    expect(wrapper.get('[data-testid="settings-column"]').classes()).not.toContain('max-w-md');
  });

  it('没有提示时不占空位', async () => {
    const wrapper = mountApp();
    await flushPromises();

    expect(wrapper.find('[data-testid="app-toast"]').exists()).toBe(false);
    expect(wrapper.find('[data-testid="print-section"]').exists()).toBe(true);
  });

  it('离线打印机状态点在下拉内部', async () => {
    const wrapper = mount(App, {
      global: {
        plugins: [i18n],
        stubs: {
          StatusDoctorSheet: StatusDoctorSheetStub,
          SelectContent: true,
        },
      },
    });
    await flushPromises();

    const trigger = wrapper.get('#default-printer');
    const status = trigger.get('[data-testid="printer-availability"]');
    expect(status.attributes('data-availability')).toBe('unavailable');
    expect(status.attributes('data-printer')).toBe('Zebra_GX430t');
    expect(wrapper.find('#default-paper').exists()).toBe(true);
  });

  it('提示长在顶栏里，不盖住打印区也不留空行', async () => {
    api.printTestPage.mockRejectedValue({
      kind: 'runtime',
      message: 'print failed: printer offline',
    });
    const wrapper = mountApp();
    await flushPromises();

    const testPrint = wrapper
      .findAll('button')
      .find((button) => button.text().includes('测试打印'));
    await testPrint!.trigger('click');
    await flushPromises();

    const toast = wrapper.get('[data-testid="app-toast"]');
    const alert = toast.get('[data-slot="alert"]');
    expect(toast.classes()).not.toContain('absolute');
    expect(wrapper.get('[data-testid="print-section"]').text()).toContain('打印');
    expect(alert.classes()).toContain('bg-red-50');
    expect(alert.classes()).not.toContain('bg-destructive/15');
  });

  it('Tauri 把离线错误收成字符串时也提示没有提交', async () => {
    api.printTestPage.mockRejectedValue(
      JSON.stringify({
        kind: 'runtime',
        message: 'print failed: printer offline',
      }),
    );
    const wrapper = mountApp();
    await flushPromises();

    const testPrint = wrapper
      .findAll('button')
      .find((button) => button.text().includes('测试打印'));
    await testPrint!.trigger('click');
    await flushPromises();

    expect(wrapper.get('[data-testid="app-toast"]').text()).toContain(
      '打印机离线，没有提交。请开机并插好线后再试。',
    );
  });

  it('打印机离线时提示没有提交，不显示已提交', async () => {
    api.printTestPage.mockRejectedValue({
      kind: 'runtime',
      message: 'print failed: printer offline',
    });
    const wrapper = mountApp();
    await flushPromises();

    api.getTaskHistory.mockClear();
    const testPrint = wrapper
      .findAll('button')
      .find((button) => button.text().includes('测试打印'));
    expect(testPrint).toBeTruthy();
    await testPrint!.trigger('click');
    await flushPromises();

    expect(api.getTaskHistory).toHaveBeenCalled();
    expect(wrapper.get('[data-testid="app-toast"]').text()).toContain(
      '打印机离线，没有提交。请开机并插好线后再试。',
    );
    expect(wrapper.get('[data-testid="app-toast"]').text()).not.toContain('已提交');
  });

  it('普通打印失败不改写成没有提交', async () => {
    api.printTestPage.mockRejectedValue(
      new Error('print failed: print command failed: lp: printer offline'),
    );
    const wrapper = mountApp();
    await flushPromises();

    const testPrint = wrapper
      .findAll('button')
      .find((button) => button.text().includes('测试打印'));
    await testPrint!.trigger('click');
    await flushPromises();

    expect(wrapper.get('[data-testid="app-toast"]').text()).toContain(
      'print failed: print command failed: lp: printer offline',
    );
    expect(wrapper.get('[data-testid="app-toast"]').text()).not.toContain('没有提交');
  });

  it('测试打印后刷新最近任务', async () => {
    api.printTestPage.mockResolvedValue(undefined);
    const wrapper = mountApp();
    await flushPromises();

    api.getTaskHistory.mockClear();
    api.getTaskHistory.mockResolvedValue([
      job({
        job_id: 'test-1',
        source: 'test',
        current_status: 'submitted',
        updated_at: '2026-09-15T02:30:00.000Z',
      }),
    ]);

    const testPrint = wrapper
      .findAll('button')
      .find((button) => button.text().includes('测试打印'));
    expect(testPrint).toBeTruthy();
    await testPrint!.trigger('click');
    await flushPromises();

    expect(api.getTaskHistory).toHaveBeenCalled();
    expect(wrapper.get('[data-testid="recent-section"]').text()).toContain('测试');
  });

  it('没有未保存改动时不显示保存，改纸张后才出现', async () => {
    const wrapper = mountApp();
    await flushPromises();

    expect(wrapper.find('[data-testid="save-settings"]').exists()).toBe(false);

    await wrapper.get('#paper-width').setValue('80');
    await nextTick();

    expect(wrapper.find('[data-testid="save-settings"]').exists()).toBe(true);
  });

  it('最近任务默认只显示 2 条，展开后能看明细', async () => {
    const wrapper = mountApp();
    await flushPromises();

    expect(wrapper.findAll('[data-testid^="recent-task-row-"]').length).toBe(2);

    await wrapper.get('[data-testid="show-all-tasks"]').trigger('click');
    await nextTick();
    expect(wrapper.findAll('[data-testid^="recent-task-row-"]').length).toBe(2);
    expect(wrapper.findAll('[data-testid^="all-task-row-"]').length).toBe(4);

    api.getTaskHistoryEvents.mockResolvedValue([
      {
        id: 1,
        job_id: 'job-2',
        status: 'failed',
        message: 'printer offline',
        occurred_at: '2026-09-15T05:58:00.000Z',
      },
    ]);
    await wrapper.get('[data-testid="all-task-row-job-2"]').trigger('click');
    await flushPromises();

    expect(wrapper.get('[data-testid="task-details"]').text()).toContain('printer offline');
    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
    await nextTick();
    expect(wrapper.find('[data-testid="task-details"]').exists()).toBe(false);
  });

  it('加网站只在名单内滚动，不挤动打印、最近和更多', async () => {
    api.getConfig.mockResolvedValue({
      ...config,
      security: {
        allowed_origins: [
          'https://a.example.com',
          'https://b.example.com',
          'https://c.example.com',
          'https://d.example.com',
          'https://e.example.com',
        ],
        allowed_ips: ['127.0.0.1'],
      },
    });

    const wrapper = mountApp();
    await flushPromises();

    const origins = wrapper.get('[data-testid="origin-list"]');
    expect(origins.text()).toContain('https://e.example.com');
    expect(origins.classes()).toContain('overflow-y-auto');
    expect(wrapper.get('[data-testid="print-section"]').text()).toContain('打印');
    expect(wrapper.get('[data-testid="recent-section"]').text()).toContain('最近');
    expect(
      wrapper.get('[data-testid="panel-footer"]').get('[data-testid="more-toggle"]').text(),
    ).toContain('更多');
  });

  it('更多里才出现端口和导入导出，没有本机地址和设备', async () => {
    const wrapper = mountApp();
    await flushPromises();

    await wrapper.get('[data-testid="more-toggle"]').trigger('click');
    await nextTick();

    expect(wrapper.find('#service-port').exists()).toBe(true);
    expect(wrapper.find('[data-testid="export-config"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="import-config"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="lan-address"]').exists()).toBe(false);
    expect(wrapper.find('[data-testid="add-device"]').exists()).toBe(false);
    expect(wrapper.find('[data-testid="device-section"]').exists()).toBe(false);

    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
    await nextTick();
    await wrapper.get('[data-testid="status-port"]').trigger('click');
    await nextTick();
    expect(wrapper.find('#service-port').exists()).toBe(true);
  });

  it('设备标签里能加网段，回环不出现，二维码弹出可复制的连接地址', async () => {
    const wrapper = mountApp();
    await flushPromises();

    await wrapper.get('[data-testid="access-devices-tab"]').trigger('click');
    await nextTick();

    expect(wrapper.get('[data-testid="device-list"]').text()).not.toContain('127.0.0.1');
    expect(wrapper.get('[data-testid="device-list"]').text()).toContain('还没有放行的设备');

    await wrapper.get('[data-testid="access-qr-button"]').trigger('click');
    await flushPromises();

    expect(wrapper.get('[data-testid="connection-url"]').text()).toContain(
      'ws://192.168.1.23:17890/ws',
    );
    expect(wrapper.get('[data-testid="copy-connection"]').attributes('disabled')).toBeUndefined();

    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
    await nextTick();
    expect(wrapper.find('[data-testid="connection-overlay"]').exists()).toBe(false);
  });

  it('没有局域网地址时二维码不能复制', async () => {
    api.getLanAddress.mockResolvedValue(null);

    const wrapper = mountApp();
    await flushPromises();
    await wrapper.get('[data-testid="access-qr-button"]').trigger('click');
    await flushPromises();

    expect(wrapper.get('[data-testid="connection-overlay"]').text()).toContain('未检测到本机地址');
    expect(wrapper.get('[data-testid="copy-connection"]').attributes('disabled')).toBeDefined();
  });

  it('导入后保留文件里的设备名单', async () => {
    vi.mocked(open).mockResolvedValue('/tmp/yinshu-config.json');
    api.previewConfigImport.mockResolvedValue({
      file_hash: 'hash-1',
      items: [
        {
          key: 'security.allowed_origins',
          label: '网站',
          current: 'https://erp.example.com',
          next: 'https://new.example.com',
        },
      ],
    });
    api.importConfigFile.mockResolvedValue({
      ...config,
      security: {
        allowed_origins: ['https://new.example.com'],
        allowed_ips: ['192.168.1.8', '10.0.0.0/8'],
      },
    });
    api.saveConfig.mockImplementation(async (next) => next);

    const wrapper = mountApp();
    await flushPromises();
    await wrapper.get('[data-testid="more-toggle"]').trigger('click');
    await wrapper.get('[data-testid="import-config"]').trigger('click');
    await nextTick();

    const choose = wrapper.findAll('button').find((button) => button.text() === '选择');
    expect(choose).toBeTruthy();
    await choose!.trigger('click');
    await flushPromises();

    const preview = wrapper.findAll('button').find((button) => button.text() === '预览');
    expect(preview).toBeTruthy();
    await preview!.trigger('click');
    await flushPromises();

    const confirm = wrapper.findAll('button').find((button) => button.text() === '确认导入');
    expect(confirm).toBeTruthy();
    await confirm!.trigger('click');
    await flushPromises();

    expect(api.importConfigFile).toHaveBeenCalledWith('/tmp/yinshu-config.json', '', 'hash-1');
    expect(api.saveConfig).toHaveBeenCalledWith(
      expect.objectContaining({
        security: {
          allowed_origins: ['https://new.example.com'],
          allowed_ips: ['192.168.1.8', '10.0.0.0/8'],
        },
      }),
    );
  });

  it('加网站和保存打印都不会清掉已放行的设备', async () => {
    api.getConfig.mockResolvedValue({
      ...config,
      security: {
        allowed_origins: ['https://erp.example.com'],
        allowed_ips: ['127.0.0.1', '192.168.1.0/24'],
      },
    });
    api.saveConfig.mockImplementation(async (next) => next);

    const wrapper = mountApp();
    await flushPromises();
    await wrapper
      .get('input[placeholder="https://example.com"]')
      .setValue('https://wms.example.com');
    await wrapper.get('[data-testid="add-origin"]').trigger('submit');
    await flushPromises();

    expect(api.saveConfig).toHaveBeenCalledWith(
      expect.objectContaining({
        security: expect.objectContaining({
          allowed_origins: ['https://erp.example.com', 'https://wms.example.com'],
          allowed_ips: expect.arrayContaining(['192.168.1.0/24']),
        }),
      }),
    );

    await wrapper.get('#paper-width').setValue('80');
    await nextTick();
    await wrapper.get('[data-testid="save-settings"]').trigger('click');
    await flushPromises();

    expect(api.saveConfig).toHaveBeenLastCalledWith(
      expect.objectContaining({
        security: expect.objectContaining({
          allowed_ips: expect.arrayContaining(['192.168.1.0/24']),
        }),
      }),
    );
  });

  it('设备标签里能加网段、拒绝重复、任意地址和非法 IPv6，放行本网写成 /24', async () => {
    api.saveConfig.mockImplementation(async (next) => next);

    const wrapper = mountApp();
    await flushPromises();
    await wrapper.get('[data-testid="access-devices-tab"]').trigger('click');
    await nextTick();

    await wrapper.get('[data-testid="device-input"]').setValue('10.0.0.5');
    await wrapper.get('[data-testid="add-device"]').trigger('submit');
    await flushPromises();
    expect(api.saveConfig).toHaveBeenCalledWith(
      expect.objectContaining({
        security: expect.objectContaining({
          allowed_ips: expect.arrayContaining(['127.0.0.1', '10.0.0.5']),
        }),
      }),
    );

    await wrapper.get('[data-testid="device-input"]').setValue('10.0.0.5');
    await wrapper.get('[data-testid="add-device"]').trigger('submit');
    await flushPromises();
    expect(wrapper.get('[data-testid="device-section"]').text()).toContain('这个地址已经在名单里');

    await wrapper.get('[data-testid="device-input"]').setValue('0.0.0.0/0');
    await wrapper.get('[data-testid="add-device"]').trigger('submit');
    await nextTick();
    expect(wrapper.get('[data-testid="device-section"]').text()).toContain('不能放行任意地址');
    expect(api.saveConfig).toHaveBeenCalledTimes(1);

    await wrapper.get('[data-testid="device-input"]').setValue(':::');
    await wrapper.get('[data-testid="add-device"]').trigger('submit');
    await nextTick();
    expect(wrapper.get('[data-testid="device-section"]').text()).toContain(
      '请输入 IP 或网段，例如 192.168.1.0/24',
    );
    expect(api.saveConfig).toHaveBeenCalledTimes(1);

    await wrapper.get('[data-testid="allow-this-network"]').trigger('click');
    await flushPromises();
    expect(api.saveConfig).toHaveBeenLastCalledWith(
      expect.objectContaining({
        security: expect.objectContaining({
          allowed_ips: expect.arrayContaining(['192.168.1.0/24']),
        }),
      }),
    );
  });

  it('导出默认勾选设备', async () => {
    const wrapper = mountApp();
    await flushPromises();
    await wrapper.get('[data-testid="more-toggle"]').trigger('click');
    await wrapper.get('[data-testid="export-config"]').trigger('click');
    await nextTick();

    expect(wrapper.get('[data-testid="export-allowed-ips"]').get('input').element).toHaveProperty(
      'checked',
      true,
    );
  });

  it('粘贴带路径的网址时只收下 Origin', async () => {
    api.saveConfig.mockImplementation(async (next) => next);

    const wrapper = mountApp();
    await flushPromises();
    await wrapper
      .get('input[placeholder="https://example.com"]')
      .setValue('https://shop.example.com/orders?id=1');
    await wrapper.get('[data-testid="add-origin"]').trigger('submit');
    await flushPromises();

    expect(api.saveConfig).toHaveBeenCalledWith(
      expect.objectContaining({
        security: expect.objectContaining({
          allowed_origins: ['https://erp.example.com', 'https://shop.example.com'],
        }),
      }),
    );
    expect(wrapper.find('[data-testid="app-toast"]').exists()).toBe(false);
  });

  it('重复添加同一网站时给出说明，不重复写入', async () => {
    api.saveConfig.mockImplementation(async (next) => next);

    const wrapper = mountApp();
    await flushPromises();
    await wrapper
      .get('input[placeholder="https://example.com"]')
      .setValue('https://erp.example.com');
    await wrapper.get('[data-testid="add-origin"]').trigger('submit');
    await flushPromises();

    expect(wrapper.get('[data-testid="access-section"]').text()).toContain('这个网站已经在名单里');
    expect(api.saveConfig).not.toHaveBeenCalled();
  });

  it('Escape 和点遮罩能关掉更多', async () => {
    const wrapper = mountApp();
    await flushPromises();
    await wrapper.get('[data-testid="more-toggle"]').trigger('click');
    await nextTick();
    expect(wrapper.find('#service-port').exists()).toBe(true);

    window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }));
    await nextTick();
    expect(wrapper.find('#service-port').exists()).toBe(false);

    await wrapper.get('[data-testid="more-toggle"]').trigger('click');
    await nextTick();
    await wrapper.get('[data-testid="more-overlay"]').trigger('click');
    await nextTick();
    expect(wrapper.find('#service-port').exists()).toBe(false);
  });
});
