// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import { i18n, setI18nLocale } from '@/i18n';
import type { DoctorReport } from '@/types';
import StatusDoctorSheet from './StatusDoctorSheet.vue';

const mocks = vi.hoisted(() => ({ runDoctor: vi.fn() }));
vi.mock('@/api', () => ({ runDoctor: mocks.runDoctor }));

const report = (status: 'PASS' | 'WARN' | 'FAIL', code = 'printing.printers'): DoctorReport => ({
  checks: [{ code, status, message: 'backend English', suggestion: 'backend suggestion' }],
  summary: {
    pass: status === 'PASS' ? 1 : 0,
    warn: status === 'WARN' ? 1 : 0,
    fail: status === 'FAIL' ? 1 : 0,
  },
});

function lastStatusChange(wrapper: ReturnType<typeof mount>): unknown[] | undefined {
  const events = wrapper.emitted('status-change') ?? [];
  return events[events.length - 1];
}

describe('StatusDoctorSheet', () => {
  beforeEach(() => {
    mocks.runDoctor.mockReset();
    setI18nLocale('zh-CN');
  });

  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('runs on open, renders localized warnings, and keeps the app ready', async () => {
    mocks.runDoctor.mockResolvedValue(report('WARN'));
    const wrapper = mount(StatusDoctorSheet, {
      attachTo: document.body,
      global: { plugins: [i18n] },
      props: { open: true },
    });

    await flushPromises();

    expect(mocks.runDoctor).toHaveBeenCalledTimes(1);
    expect(document.body.textContent).toContain('系统里还没有打印机');
    expect(document.body.textContent).toContain('只管名单在不在。不管在不在线、有没有纸');
    expect(document.body.textContent).toContain('先在系统里把打印机装好，再回来重新检测');
    expect(document.body.textContent).not.toContain('backend English');
    expect(lastStatusChange(wrapper)).toEqual([false]);
  });

  it('reports FAIL and call errors as needing attention', async () => {
    mocks.runDoctor.mockResolvedValueOnce(report('FAIL'));
    const wrapper = mount(StatusDoctorSheet, {
      attachTo: document.body,
      global: { plugins: [i18n] },
      props: { open: true },
    });
    await flushPromises();
    expect(lastStatusChange(wrapper)).toEqual([true]);

    mocks.runDoctor.mockRejectedValueOnce(new Error('raw backend error'));
    document.querySelector<HTMLElement>('[data-testid="doctor-retry"]')!.click();
    await flushPromises();
    expect(document.body.textContent).toContain('状态检测失败，请重试。');
    expect(document.body.textContent).not.toContain('raw backend error');
    expect(lastStatusChange(wrapper)).toEqual([true]);
  });

  it('keeps unknown checks visible and rerenders language without rerunning', async () => {
    mocks.runDoctor.mockResolvedValue(report('WARN', 'future.check'));
    mount(StatusDoctorSheet, {
      attachTo: document.body,
      global: { plugins: [i18n] },
      props: { open: true },
    });
    await flushPromises();
    expect(document.body.textContent).toContain('future.check');

    setI18nLocale('en');
    await flushPromises();
    expect(document.body.textContent).toContain('Needs a look');
    expect(mocks.runDoctor).toHaveBeenCalledTimes(1);
  });

  it('reuses an in-flight run but starts a fresh run after completion', async () => {
    let resolveRun!: (value: DoctorReport) => void;
    const pending = new Promise<DoctorReport>((resolve) => {
      resolveRun = resolve;
    });
    mocks.runDoctor.mockReturnValue(pending);
    const wrapper = mount(StatusDoctorSheet, {
      attachTo: document.body,
      global: { plugins: [i18n] },
      props: { open: true },
    });

    await wrapper.setProps({ open: false });
    await wrapper.setProps({ open: true });
    expect(mocks.runDoctor).toHaveBeenCalledTimes(1);

    resolveRun(report('PASS'));
    await flushPromises();
    expect(document.body.textContent).toContain('系统里看得到打印机');
    expect(document.body.textContent).toContain('只管名单在不在。不管在不在线、有没有纸');
    await wrapper.setProps({ open: false });
    await wrapper.setProps({ open: true });
    await flushPromises();
    expect(mocks.runDoctor).toHaveBeenCalledTimes(2);
  });
});
