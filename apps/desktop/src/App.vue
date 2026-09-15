<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import {
  ChevronDown,
  FileArchive,
  FileDown,
  FileUp,
  Printer,
  QrCode,
  RefreshCw,
  Save,
  Trash2,
  X,
} from '@lucide/vue';
import QRCode from 'qrcode';
import { open as openDialog, save as saveDialog } from '@tauri-apps/plugin-dialog';
import { relaunch } from '@tauri-apps/plugin-process';
import {
  exportConfigFile,
  exportDiagnostics,
  fetchPapers,
  fetchPrinters,
  clearTaskHistory,
  getConfig,
  getLanAddress,
  getTaskHistory,
  getTaskHistoryEvents,
  importConfigFile,
  isDebugBuild,
  printTestPage,
  previewConfigImport,
  saveConfig,
} from '@/api';
import type {
  AgentConfig,
  EffectivePaper,
  ExportConfigOptions,
  ImportPreview,
  PaperInfo,
  PrinterAvailability,
  PrinterInfo,
  TaskHistoryEvent,
  TaskHistoryJob,
  TaskHistorySource,
  TaskHistoryStatus,
} from '@/types';
import { Alert, AlertDescription } from '@/components/ui/alert';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { Switch } from '@/components/ui/switch';
import StatusDoctorSheet from '@/components/StatusDoctorSheet.vue';
import { DEFAULT_UI_LANGUAGE, isUiLanguage, setI18nLocale, type UiLanguage } from '@/i18n';
import { useOnboarding } from '@/onboarding';

const DEFAULT_PAPER: EffectivePaper = {
  width_mm: 60,
  height_mm: 40,
};
const MIN_SERVICE_PORT = 10000;
const MAX_SERVICE_PORT = 65535;
const REQUIRED_LOOPBACK_IP = '127.0.0.1';
const THEME_STORAGE_KEY = 'yinshu.theme';
const RECENT_TASK_LIMIT = 2;
type ThemeMode = 'system' | 'light' | 'dark';

const DEFAULT_APP_CONFIG: AgentConfig['app'] = {
  autostart: false,
  language: DEFAULT_UI_LANGUAGE,
};

const config = ref<AgentConfig | null>(null);
const doctorOpen = ref(false);
const doctorNeedsAttention = ref(false);
const printers = ref<PrinterInfo[]>([]);
const papers = ref<PaperInfo[]>([]);
const taskHistory = ref<TaskHistoryJob[]>([]);
const selectedTaskJobId = ref<string | null>(null);
const selectedTaskEvents = ref<TaskHistoryEvent[]>([]);
const themeMode = ref<ThemeMode>(readThemeMode());
const originDraft = ref('');
const originErrorMessage = ref('');
const ipDraft = ref('');
const ipErrorMessage = ref('');
const lanAddress = ref<string | null>(null);
const lanCopied = ref(false);
const errorMessage = ref('');
const successMessage = ref('');
const loadingConfig = ref(true);
const loadingPrinters = ref(false);
const loadingTaskHistory = ref(false);
const loadingTaskEvents = ref(false);
const clearingTaskHistory = ref(false);
const confirmingClearTaskHistory = ref(false);
const saving = ref(false);
const savingOrigins = ref(false);
const exportingConfig = ref(false);
const exportingDiagnostics = ref(false);
const importingConfig = ref(false);
const previewingConfigImport = ref(false);
const showExportDialog = ref(false);
const showImportDialog = ref(false);
const exportPassword = ref('');
const importPassword = ref('');
const importPath = ref('');
const importPreview = ref<ImportPreview | null>(null);
const importErrorMessage = ref('');
const exportOptions = ref<ExportConfigOptions>(defaultExportOptions());
const testingPrint = ref(false);
const activePort = ref<number | null>(null);
const moreOpen = ref(false);
const qrOpen = ref(false);
const qrDataUrl = ref<string | null>(null);
const accessTab = ref<'websites' | 'devices'>('websites');
const showingAllTasks = ref(false);
const savedSettingsKey = ref('');
let colorSchemeQuery: MediaQueryList | null = null;
let successTimer: number | null = null;
let lanCopiedTimer: number | null = null;
let taskHistoryRequestId = 0;
let taskEventsRequestId = 0;

const { t } = useI18n();
const onboardingReady = computed(() => config.value !== null && !loadingConfig.value);
useOnboarding({
  ready: onboardingReady,
  t: (key) => t(key),
});

/** UI 请求当前使用的本地服务端口。 */
const servicePort = computed(() => activePort.value ?? config.value?.service.port ?? 0);
/** 保存的配置端口是否还未在当前服务中生效。 */
const hasPendingPortChange = computed(
  () =>
    activePort.value !== null &&
    config.value !== null &&
    config.value.service.port !== activePort.value,
);
/** 顶部状态栏显示的可读状态。 */
const statusLabel = computed(() => {
  if (loadingConfig.value) return t('loading');
  if (errorMessage.value || doctorNeedsAttention.value) return t('needsAttention');
  return t('ready');
});
/** 根据当前错误状态计算状态徽标样式。 */
const statusVariant = computed(() => {
  if (loadingConfig.value) return 'secondary';
  if (errorMessage.value || doctorNeedsAttention.value) return 'destructive';
  return 'success';
});
/** 打印、端口和偏好是否有尚未保存的改动。 */
const hasUnsavedSettings = computed(() => {
  if (!config.value || !savedSettingsKey.value) return false;
  return settingsKey(config.value) !== savedSettingsKey.value;
});
/** 当前配置是否足够提交测试打印。 */
const canTestPrint = computed(
  () =>
    Boolean(config.value?.printing.default_printer) &&
    Boolean(config.value?.printing.default_paper),
);
/** 默认打印机选择项的双向计算值。 */
const selectedPrinter = computed({
  get: () => config.value?.printing.default_printer ?? '',
  set: (value: string) => {
    if (!config.value) return;
    config.value.printing.default_printer = value || null;
  },
});
const selectedPrinterInfo = computed(
  () => printers.value.find((item) => item.name === selectedPrinter.value) ?? null,
);

/** 纸张预设或自定义纸张选择项的双向计算值。 */
const selectedPaper = computed({
  get: () => {
    const currentPaper = config.value?.printing.default_paper;
    if (!currentPaper) return 'custom';

    return matchingPaper(currentPaper)?.id ?? 'custom';
  },
  set: (paperId: string) => {
    const paper = papers.value.find((item) => item.id === paperId);
    if (!paper || !config.value) return;

    config.value.printing.default_paper = {
      width_mm: paper.width_mm,
      height_mm: paper.height_mm,
    };
  },
});
/** 当前选中的任务摘要。 */
const selectedTask = computed(
  () => taskHistory.value.find((item) => item.job_id === selectedTaskJobId.value) ?? null,
);
/** 首页只露出最近几条，其余进浮层。 */
const visibleTasks = computed(() => taskHistory.value.slice(0, RECENT_TASK_LIMIT));
const hasMoreTasks = computed(() => taskHistory.value.length > RECENT_TASK_LIMIT);
const visibleDevices = computed(
  () => config.value?.security.allowed_ips.filter((entry) => entry !== REQUIRED_LOOPBACK_IP) ?? [],
);
const lanCidr = computed(() => ipv4LanCidr(lanAddress.value));
const connectionUrl = computed(() => {
  if (!lanAddress.value || !servicePort.value) return null;
  return `ws://${lanAddress.value}:${servicePort.value}/ws`;
});

/** 当前设置的 UI 语言。 */
const currentLanguage = computed(() => config.value?.app.language ?? DEFAULT_APP_CONFIG.language);

watch(currentLanguage, (language) => setI18nLocale(language), { immediate: true });

/** 只比较需要显式保存的设置，网站名单改动会单独立即写入。 */
function settingsKey(value: AgentConfig): string {
  return JSON.stringify({
    service: value.service,
    printing: value.printing,
    app: value.app,
  });
}

function printerAvailabilityClass(availability: PrinterAvailability | undefined): string {
  if (availability === 'available') return 'bg-emerald-500';
  if (availability === 'unavailable') return 'bg-red-500';
  return 'bg-muted-foreground/30';
}

function printerAvailabilityLabel(availability: PrinterAvailability | undefined): string {
  if (availability === 'available') return t('printerReady');
  if (availability === 'unavailable') return t('printerOffline');
  return t('printerUnknown');
}

function rememberSavedSettings(value: AgentConfig): void {
  savedSettingsKey.value = settingsKey(value);
}

/** 确保加载后的配置始终有可用的默认纸张对象。 */
function normalizeConfig(value: AgentConfig): AgentConfig {
  return {
    ...value,
    app: {
      ...DEFAULT_APP_CONFIG,
      ...value.app,
      language: isUiLanguage(value.app?.language)
        ? value.app.language
        : DEFAULT_APP_CONFIG.language,
    },
    printing: {
      ...value.printing,
      default_paper: value.printing.default_paper ?? { ...DEFAULT_PAPER },
    },
  };
}

function defaultExportOptions(): ExportConfigOptions {
  return {
    service_port: true,
    allowed_origins: true,
    allowed_ips: true,
  };
}

/** 判断已存储的字符串是否是支持的主题模式。 */
function isThemeMode(value: string | null): value is ThemeMode {
  return value === 'system' || value === 'light' || value === 'dark';
}

/** 从本地存储读取已保存的主题模式。 */
function readThemeMode(): ThemeMode {
  const storedMode = window.localStorage.getItem(THEME_STORAGE_KEY);
  return isThemeMode(storedMode) ? storedMode : 'system';
}

/** 把选择的主题或系统推导出的主题应用到页面。 */
function applyTheme(mode: ThemeMode): void {
  const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
  const shouldUseDark = mode === 'dark' || (mode === 'system' && prefersDark);

  document.documentElement.classList.toggle('dark', shouldUseDark);
  document.documentElement.style.colorScheme = shouldUseDark ? 'dark' : 'light';
}

/** 保存选择的主题模式，并立即应用。 */
function setThemeMode(value: string): void {
  const nextMode = isThemeMode(value) ? value : 'system';

  themeMode.value = nextMode;
  window.localStorage.setItem(THEME_STORAGE_KEY, nextMode);
  applyTheme(nextMode);
}

/** 更新 UI 语言，随保存配置持久化。 */
function setLanguage(value: string): void {
  if (!config.value) return;
  config.value.app.language = isUiLanguage(value) ? value : DEFAULT_APP_CONFIG.language;
}

function preferenceOptionClass(isActive: boolean): string {
  return isActive ? 'text-foreground underline underline-offset-4' : 'text-muted-foreground';
}

/** 系统配色变化时重新应用系统主题。 */
function handleSystemThemeChange(): void {
  if (themeMode.value === 'system') {
    applyTheme('system');
  }
}

/** 注册系统主题模式需要的系统配色监听器。 */
function setupThemeSync(): void {
  colorSchemeQuery = window.matchMedia('(prefers-color-scheme: dark)');
  colorSchemeQuery.addEventListener('change', handleSystemThemeChange);
}

/** 返回可编辑的纸张配置，必要时创建默认纸张。 */
function currentPaper(): EffectivePaper {
  if (!config.value) return DEFAULT_PAPER;
  if (!config.value.printing.default_paper) {
    config.value.printing.default_paper = { ...DEFAULT_PAPER };
  }

  return config.value.printing.default_paper;
}

/** 查找与给定纸张尺寸相同的打印机纸张预设。 */
function matchingPaper(paper: EffectivePaper): PaperInfo | undefined {
  return papers.value.find(
    (item) =>
      Math.abs(item.width_mm - paper.width_mm) < 0.01 &&
      Math.abs(item.height_mm - paper.height_mm) < 0.01,
  );
}

/** 输入值有效时更新配置中的服务端口。 */
function setPort(value: string | number): void {
  if (!config.value) return;
  const port = Number(value);
  if (Number.isInteger(port)) {
    config.value.service.port = Math.min(MAX_SERVICE_PORT, Math.max(MIN_SERVICE_PORT, port));
  }
}

/** 输入值有效时更新一个自定义纸张尺寸。 */
function setPaperDimension(key: keyof EffectivePaper, value: string | number): void {
  const dimension = Number(value);
  if (Number.isFinite(dimension) && dimension > 0) {
    currentPaper()[key] = dimension;
  }
}

/** 关闭顶部提示消息。 */
function dismissMessage(kind: 'error' | 'success'): void {
  if (kind === 'error') {
    errorMessage.value = '';
  } else {
    if (successTimer) {
      window.clearTimeout(successTimer);
      successTimer = null;
    }
    successMessage.value = '';
  }
}

function showSuccess(message: string): void {
  errorMessage.value = '';
  if (successTimer) window.clearTimeout(successTimer);
  successMessage.value = message;
  successTimer = window.setTimeout(() => {
    successMessage.value = '';
    successTimer = null;
  }, 3200);
}

function openExportDialog(): void {
  exportOptions.value = defaultExportOptions();
  exportPassword.value = '';
  showExportDialog.value = true;
}

async function handleExportDiagnostics(): Promise<void> {
  exportingDiagnostics.value = true;
  errorMessage.value = '';
  successMessage.value = '';

  try {
    const path = await saveDialog({
      defaultPath: 'yinshu-diagnose.zip',
      filters: [{ name: 'ZIP', extensions: ['zip'] }],
    });
    if (!path) return;

    await exportDiagnostics(path);
    showSuccess(t('diagnosticsExported'));
  } catch (error) {
    errorMessage.value = error instanceof Error ? error.message : t('exportDiagnosticsFailed');
  } finally {
    exportingDiagnostics.value = false;
  }
}

async function handleExportConfig(): Promise<void> {
  exportingConfig.value = true;
  errorMessage.value = '';
  successMessage.value = '';

  try {
    const path = await saveDialog({
      defaultPath: 'yinshu-config.json',
      filters: [{ name: 'JSON', extensions: ['json'] }],
    });
    if (!path) return;

    await exportConfigFile(path, exportPassword.value, exportOptions.value);
    showExportDialog.value = false;
    showSuccess(t('configExported'));
  } catch (error) {
    errorMessage.value = error instanceof Error ? error.message : t('exportConfigFailed');
  } finally {
    exportingConfig.value = false;
  }
}

function openImportDialog(): void {
  importPassword.value = '';
  importPath.value = '';
  importPreview.value = null;
  importErrorMessage.value = '';
  showImportDialog.value = true;
}

function resetImportPreview(): void {
  importPreview.value = null;
  importErrorMessage.value = '';
}

async function chooseImportFile(): Promise<void> {
  const path = await openDialog({
    multiple: false,
    filters: [{ name: 'JSON', extensions: ['json'] }],
  });
  if (typeof path === 'string') {
    importPath.value = path;
    resetImportPreview();
  }
}

async function handlePreviewConfigImport(): Promise<void> {
  if (!importPath.value) return;
  previewingConfigImport.value = true;
  importErrorMessage.value = '';
  errorMessage.value = '';
  successMessage.value = '';

  try {
    importPreview.value = await previewConfigImport(importPath.value, importPassword.value);
  } catch (error) {
    importPreview.value = null;
    importErrorMessage.value = error instanceof Error ? error.message : t('importPreviewFailed');
  } finally {
    previewingConfigImport.value = false;
  }
}

async function handleImportConfig(): Promise<void> {
  if (!importPath.value || !importPreview.value) return;
  importingConfig.value = true;
  importErrorMessage.value = '';
  errorMessage.value = '';
  successMessage.value = '';

  try {
    const imported = normalizeConfig(
      await importConfigFile(importPath.value, importPassword.value, importPreview.value.file_hash),
    );
    config.value = normalizeConfig(await saveConfig(imported));
    rememberSavedSettings(config.value);
    showImportDialog.value = false;
    showSuccess(t('configImported'));
  } catch (error) {
    importErrorMessage.value = error instanceof Error ? error.message : t('importConfigFailed');
  } finally {
    importingConfig.value = false;
  }
}

/** 加载配置、打印机、纸张和任务历史，初始化页面状态。 */
async function loadConfig(): Promise<void> {
  loadingConfig.value = true;
  errorMessage.value = '';

  try {
    config.value = normalizeConfig(await getConfig());
    activePort.value = config.value.service.port;
    await Promise.all([refreshPrinters(), refreshTaskHistory(), refreshLanAddress()]);
    if (config.value) rememberSavedSettings(config.value);
  } catch (error) {
    errorMessage.value = error instanceof Error ? error.message : t('loadConfigFailed');
  } finally {
    loadingConfig.value = false;
  }
}

/** 刷新打印机列表，并在未配置时选择合适的默认打印机。 */
async function refreshPrinters(): Promise<void> {
  if (!config.value) return;
  loadingPrinters.value = true;
  errorMessage.value = '';

  try {
    printers.value = await fetchPrinters();
    if (!config.value.printing.default_printer) {
      config.value.printing.default_printer =
        printers.value.find((printer) => printer.is_default)?.name ??
        printers.value[0]?.name ??
        null;
    }
    await refreshPapers();
  } catch (error) {
    printers.value = [];
    papers.value = [];
    errorMessage.value = error instanceof Error ? error.message : t('refreshPrintersFailed');
  } finally {
    loadingPrinters.value = false;
  }
}

/** 刷新当前选中打印机的纸张选项。 */
async function refreshPapers(): Promise<void> {
  if (!config.value?.printing.default_printer) {
    papers.value = [];
    return;
  }

  errorMessage.value = '';

  try {
    papers.value = await fetchPapers(config.value.printing.default_printer);
  } catch (error) {
    papers.value = [];
    errorMessage.value = error instanceof Error ? error.message : t('refreshPapersFailed');
  }
}

/** 应用打印机选择，并重新加载对应纸张列表。 */
async function handlePrinterChange(value: string): Promise<void> {
  selectedPrinter.value = value;
  await refreshPapers();
}

/** 通过 Tauri 保存当前设置；端口变化时重启应用让新监听端口生效。 */
async function persistConfig(): Promise<void> {
  if (!config.value) return;
  saving.value = true;
  errorMessage.value = '';
  successMessage.value = '';

  try {
    const savedPort = config.value.service.port;
    const portChanged = activePort.value !== null && savedPort !== activePort.value;
    config.value = normalizeConfig(await saveConfig(config.value));
    rememberSavedSettings(config.value);
    if (portChanged) {
      if (await isDebugBuild()) {
        showSuccess(t('settingsSavedDev'));
        return;
      }
      showSuccess(t('settingsSavedRestarting'));
      await relaunch();
      return;
    }
    showSuccess(t('settingsSaved'));
  } catch (error) {
    errorMessage.value = error instanceof Error ? error.message : t('saveOrRestartFailed');
  } finally {
    saving.value = false;
  }
}

function invokeErrorMessage(error: unknown, depth = 0): string {
  if (depth > 3) return '';
  if (typeof error === 'string') {
    const trimmed = error.trim();
    if (trimmed.startsWith('{') || trimmed.startsWith('[')) {
      try {
        return invokeErrorMessage(JSON.parse(trimmed), depth + 1) || error;
      } catch {
        return error;
      }
    }
    return error;
  }
  if (error instanceof Error) {
    return invokeErrorMessage(error.message, depth + 1);
  }
  if (!error || typeof error !== 'object') return '';

  const record = error as Record<string, unknown>;
  for (const key of ['message', 'error']) {
    const nested = record[key];
    if (typeof nested === 'string' || (nested && typeof nested === 'object')) {
      const extracted = invokeErrorMessage(nested, depth + 1);
      if (extracted) return extracted;
    }
  }
  return '';
}

function testPrintErrorMessage(error: unknown): string {
  const message = invokeErrorMessage(error);
  if (message === 'printer offline' || message === 'print failed: printer offline') {
    return t('testPrintOffline');
  }
  return message || t('testPrintFailed');
}

/** 使用当前 Agent 默认打印设置提交一张配置测试页。 */
async function handleTestPrint(): Promise<void> {
  if (!config.value || !canTestPrint.value) return;
  testingPrint.value = true;
  errorMessage.value = '';
  successMessage.value = '';

  try {
    await printTestPage(config.value);
    await refreshTaskHistory();
    showSuccess(t('testPrintSubmitted'));
  } catch (error) {
    await refreshTaskHistory();
    errorMessage.value = testPrintErrorMessage(error);
  } finally {
    testingPrint.value = false;
  }
}

/** 从本地 Agent 刷新任务历史。 */
async function refreshTaskHistory(): Promise<void> {
  confirmingClearTaskHistory.value = false;
  const requestId = ++taskHistoryRequestId;
  loadingTaskHistory.value = true;

  try {
    const nextTaskHistory = await getTaskHistory();
    if (requestId !== taskHistoryRequestId) return;

    taskHistory.value = nextTaskHistory;
    errorMessage.value = '';
    if (selectedTaskJobId.value) {
      const stillExists = taskHistory.value.some((item) => item.job_id === selectedTaskJobId.value);
      if (stillExists) {
        await selectTask(selectedTaskJobId.value);
      } else {
        selectedTaskJobId.value = null;
        selectedTaskEvents.value = [];
      }
    }
  } catch (error) {
    if (requestId !== taskHistoryRequestId) return;
    errorMessage.value = error instanceof Error ? error.message : t('taskHistoryLoadFailed');
  } finally {
    if (requestId === taskHistoryRequestId) {
      loadingTaskHistory.value = false;
    }
  }
}

/** 读取单个任务的状态事件。 */
async function selectTask(jobId: string): Promise<void> {
  const requestId = ++taskEventsRequestId;
  selectedTaskJobId.value = jobId;
  selectedTaskEvents.value = [];
  loadingTaskEvents.value = true;

  try {
    const nextEvents = await getTaskHistoryEvents(jobId);
    if (requestId !== taskEventsRequestId || selectedTaskJobId.value !== jobId) return;

    selectedTaskEvents.value = nextEvents;
    errorMessage.value = '';
  } catch (error) {
    if (requestId !== taskEventsRequestId || selectedTaskJobId.value !== jobId) return;
    errorMessage.value = error instanceof Error ? error.message : t('taskStatusLoadFailed');
  } finally {
    if (requestId === taskEventsRequestId && selectedTaskJobId.value === jobId) {
      loadingTaskEvents.value = false;
    }
  }
}

function openAllTasks(): void {
  showingAllTasks.value = true;
  confirmingClearTaskHistory.value = false;
}

function closeTaskDetails(): void {
  selectedTaskJobId.value = null;
  selectedTaskEvents.value = [];
}

/** 清空本地任务历史。 */
async function handleClearTaskHistory(): Promise<void> {
  if (!confirmingClearTaskHistory.value) {
    confirmingClearTaskHistory.value = true;
    errorMessage.value = '';
    successMessage.value = '';
    return;
  }

  clearingTaskHistory.value = true;
  errorMessage.value = '';
  successMessage.value = '';

  try {
    await clearTaskHistory();
    taskHistoryRequestId++;
    taskEventsRequestId++;
    taskHistory.value = [];
    selectedTaskJobId.value = null;
    selectedTaskEvents.value = [];
    showingAllTasks.value = false;
    confirmingClearTaskHistory.value = false;
    showSuccess(t('taskHistoryCleared'));
  } catch (error) {
    confirmingClearTaskHistory.value = false;
    errorMessage.value = error instanceof Error ? error.message : t('taskHistoryClearFailed');
  } finally {
    clearingTaskHistory.value = false;
  }
}

type SecuritySnapshot = {
  allowed_origins: string[];
  allowed_ips: string[];
};

function snapshotSecurity(): SecuritySnapshot {
  return {
    allowed_origins: [...(config.value?.security.allowed_origins ?? [])],
    allowed_ips: [...(config.value?.security.allowed_ips ?? [])],
  };
}

/** 只保存网站和设备名单，不碰未保存的打印、端口和偏好。 */
async function persistSecurityChanges(previous: SecuritySnapshot): Promise<boolean> {
  if (!config.value) return false;
  savingOrigins.value = true;
  errorMessage.value = '';

  try {
    const persistedConfig = await getConfig();
    persistedConfig.security = {
      allowed_origins: [...config.value.security.allowed_origins],
      allowed_ips: normalizeAllowedIps(config.value.security.allowed_ips),
    };
    const savedConfig = normalizeConfig(await saveConfig(persistedConfig));
    config.value.security = savedConfig.security;
    return true;
  } catch (error) {
    config.value.security.allowed_origins = previous.allowed_origins;
    config.value.security.allowed_ips = previous.allowed_ips;
    errorMessage.value = error instanceof Error ? error.message : t('saveOrRestartFailed');
    return false;
  } finally {
    savingOrigins.value = false;
  }
}

async function refreshLanAddress(): Promise<void> {
  try {
    lanAddress.value = await getLanAddress();
  } catch {
    lanAddress.value = null;
  }
}

/** 把校验通过的浏览器 Origin 加入允许列表并立即保存。 */
async function addOrigin(): Promise<void> {
  if (!config.value) return;
  originErrorMessage.value = '';

  const origin = parseOrigin(originDraft.value);
  if (!origin) {
    if (originDraft.value.trim()) originErrorMessage.value = t('invalidOrigin');
    return;
  }
  if (config.value.security.allowed_origins.includes(origin)) {
    originErrorMessage.value = t('originAlreadyAdded');
    return;
  }

  const previous = snapshotSecurity();
  config.value.security.allowed_origins.push(origin);
  originDraft.value = '';
  if (!(await persistSecurityChanges(previous))) originDraft.value = origin;
}

/** 从完整网址取出浏览器 Origin：协议 + 主机 + 端口。 */
function parseOrigin(value: string): string | null {
  try {
    const url = new URL(value.trim());
    if (url.protocol !== 'http:' && url.protocol !== 'https:') return null;
    return `${url.protocol}//${url.host}`;
  } catch {
    return null;
  }
}

function closeTopOverlay(): void {
  if (doctorOpen.value) {
    doctorOpen.value = false;
    return;
  }
  if (showImportDialog.value) {
    showImportDialog.value = false;
    return;
  }
  if (showExportDialog.value) {
    showExportDialog.value = false;
    return;
  }
  if (selectedTaskJobId.value) {
    closeTaskDetails();
    return;
  }
  if (showingAllTasks.value) {
    showingAllTasks.value = false;
    return;
  }
  if (qrOpen.value) {
    qrOpen.value = false;
    return;
  }
  if (moreOpen.value) moreOpen.value = false;
}

function handleWindowKeydown(event: KeyboardEvent): void {
  if (event.key === 'Escape') closeTopOverlay();
}

/** 从允许列表移除一个浏览器 Origin 并立即保存。 */
async function removeOrigin(origin: string): Promise<void> {
  if (!config.value) return;
  const previous = snapshotSecurity();
  config.value.security.allowed_origins = config.value.security.allowed_origins.filter(
    (item) => item !== origin,
  );
  await persistSecurityChanges(previous);
}

function normalizeAllowedIps(entries: string[]): string[] {
  const normalized = [REQUIRED_LOOPBACK_IP];
  for (const rawEntry of entries) {
    const entry = rawEntry.trim();
    if (!entry || entry === REQUIRED_LOOPBACK_IP) continue;
    if (!normalized.includes(entry)) normalized.push(entry);
  }
  return normalized;
}

function isAnyAddressEntry(entry: string): boolean {
  return entry === '0.0.0.0' || entry === '::' || entry === '0.0.0.0/0' || entry === '::/0';
}

function isValidIpAddress(value: string): boolean {
  if (value.includes(':')) {
    try {
      const hostname = new URL(`http://[${value}]`).hostname;
      return hostname.replace(/^\[|\]$/g, '').length > 0;
    } catch {
      return false;
    }
  }

  const parts = value.split('.');
  return (
    parts.length === 4 &&
    parts.every((part) => {
      if (!/^\d+$/.test(part)) return false;
      const number = Number(part);
      return number >= 0 && number <= 255 && part === String(number);
    })
  );
}

function isValidAllowedIpEntry(value: string): boolean {
  const entry = value.trim();
  if (!entry || isAnyAddressEntry(entry)) return false;

  if (entry.includes('/')) {
    const [address, prefixText, extra] = entry.split('/');
    if (!address || !prefixText || extra !== undefined) return false;
    const prefix = Number(prefixText);
    if (!Number.isInteger(prefix)) return false;
    const maxPrefix = address.includes(':') ? 128 : 32;
    if (prefix <= 0 || prefix > maxPrefix) return false;
    return isValidIpAddress(address);
  }

  return isValidIpAddress(entry);
}

function ipv4LanCidr(address: string | null): string | null {
  if (!address || address.includes(':') || !isValidIpAddress(address)) return null;
  const parts = address.split('.');
  return `${parts[0]}.${parts[1]}.${parts[2]}.0/24`;
}

async function addAllowedIp(): Promise<void> {
  if (!config.value) return;
  ipErrorMessage.value = '';

  const entry = ipDraft.value.trim();
  if (!entry) return;
  if (!isValidAllowedIpEntry(entry)) {
    ipErrorMessage.value = isAnyAddressEntry(entry) ? t('invalidAnyIp') : t('invalidIp');
    return;
  }
  if (config.value.security.allowed_ips.includes(entry)) {
    ipErrorMessage.value = t('ipAlreadyAdded');
    return;
  }

  const previous = snapshotSecurity();
  config.value.security.allowed_ips = normalizeAllowedIps([
    ...config.value.security.allowed_ips,
    entry,
  ]);
  ipDraft.value = '';
  if (!(await persistSecurityChanges(previous))) ipDraft.value = entry;
}

async function removeAllowedIp(entry: string): Promise<void> {
  if (!config.value || entry === REQUIRED_LOOPBACK_IP) return;
  const previous = snapshotSecurity();
  config.value.security.allowed_ips = normalizeAllowedIps(
    config.value.security.allowed_ips.filter((item) => item !== entry),
  );
  await persistSecurityChanges(previous);
}

function allowThisNetwork(): void {
  const cidr = lanCidr.value;
  if (!cidr) return;
  ipDraft.value = cidr;
  void addAllowedIp();
}

async function copyLanAddress(): Promise<void> {
  if (!connectionUrl.value) return;
  try {
    await navigator.clipboard.writeText(connectionUrl.value);
    if (lanCopiedTimer) window.clearTimeout(lanCopiedTimer);
    lanCopied.value = true;
    lanCopiedTimer = window.setTimeout(() => {
      lanCopied.value = false;
      lanCopiedTimer = null;
    }, 1600);
  } catch {
    errorMessage.value = t('copyFailed');
  }
}

async function updateQrImage(): Promise<void> {
  if (!connectionUrl.value) {
    qrDataUrl.value = null;
    return;
  }
  try {
    qrDataUrl.value = await QRCode.toDataURL(connectionUrl.value, {
      margin: 1,
      width: 192,
      errorCorrectionLevel: 'M',
    });
  } catch {
    qrDataUrl.value = null;
  }
}

async function openConnectionOverlay(): Promise<void> {
  qrOpen.value = true;
  lanCopied.value = false;
  await refreshLanAddress();
  await updateQrImage();
}

/** 格式化 RFC3339 时间用于展示。 */
function formatDateTime(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;

  return date.toLocaleString();
}

/** 最近列表用更短的时间。 */
function formatTaskTime(value: string): string {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;

  const sameDay = date.toDateString() === new Date().toDateString();
  return date.toLocaleString(currentLanguage.value, {
    month: sameDay ? undefined : 'numeric',
    day: sameDay ? undefined : 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  });
}

/** 返回任务状态标签。 */
function taskStatusLabel(status: TaskHistoryStatus): string {
  const labels: Record<UiLanguage, Record<TaskHistoryStatus, string>> = {
    'zh-CN': {
      queued: '已排队',
      downloading: '下载中',
      printing: '提交中',
      submitted: '已提交',
      completed: '已完成',
      failed: '失败',
      unknown: '未知',
      cancelled: '已取消',
    },
    en: {
      queued: 'Queued',
      downloading: 'Downloading',
      printing: 'Submitting',
      submitted: 'Submitted',
      completed: 'Completed',
      failed: 'Failed',
      unknown: 'Unknown',
      cancelled: 'Cancelled',
    },
  };

  return labels[currentLanguage.value][status];
}

function taskStatusVariant(status: TaskHistoryStatus): 'success' | 'destructive' | 'outline' {
  if (status === 'failed' || status === 'cancelled') return 'destructive';
  if (status === 'completed' || status === 'submitted') return 'success';
  return 'outline';
}

/** 返回任务来源标签。 */
function taskSourceLabel(source: TaskHistorySource): string {
  const labels: Record<UiLanguage, Record<TaskHistorySource, string>> = {
    'zh-CN': {
      web_socket: '网页',
      test: '测试',
    },
    en: {
      web_socket: 'Web',
      test: 'Test',
    },
  };

  return labels[currentLanguage.value][source];
}

/** 格式化毫米尺寸，设置页用整数毫米展示。 */
function formatMillimeters(value: number): string {
  return Math.round(value).toString();
}

/** 生成纸张下拉项文案，避免尺寸型名称重复显示。 */
function formatPaperLabel(paper: PaperInfo): string {
  const sizeLabel = `${formatMillimeters(paper.width_mm)} x ${formatMillimeters(paper.height_mm)} mm`;
  if (/^\d+(?:\.\d+)? x \d+(?:\.\d+)? mm$/.test(paper.name)) {
    return sizeLabel;
  }

  return `${paper.name} · ${sizeLabel}`;
}

applyTheme(themeMode.value);

onMounted(() => {
  setupThemeSync();
  window.addEventListener('keydown', handleWindowKeydown);
  void loadConfig();
});

onBeforeUnmount(() => {
  window.removeEventListener('keydown', handleWindowKeydown);
  if (successTimer) window.clearTimeout(successTimer);
  if (lanCopiedTimer) window.clearTimeout(lanCopiedTimer);
  colorSchemeQuery?.removeEventListener('change', handleSystemThemeChange);
});
</script>

<template>
  <main
    class="relative flex h-screen flex-col overflow-hidden bg-background px-4 py-3 text-foreground"
  >
    <div
      data-testid="settings-column"
      class="mx-auto flex h-full w-full flex-col gap-3"
    >
      <header
        class="flex shrink-0 flex-col gap-2 border-b pb-2"
        data-tour="app-status"
      >
        <div class="flex items-center justify-between gap-3">
          <button
            type="button"
            data-testid="status-doctor-trigger"
            class="rounded-sm focus-visible:ring-2 focus-visible:ring-ring focus-visible:outline-none"
            :aria-label="t('doctor.open')"
            @click="doctorOpen = true"
          >
            <Badge :variant="statusVariant">
              {{ statusLabel }}
            </Badge>
          </button>
          <button
            type="button"
            data-testid="status-port"
            class="text-sm text-muted-foreground"
            :aria-label="t('localPort')"
            @click="moreOpen = true"
          >
            {{ servicePort || '-' }}
          </button>
        </div>
        <div
          v-if="errorMessage || successMessage"
          data-testid="app-toast"
        >
          <Alert
            :variant="errorMessage ? 'error' : 'success'"
            class="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-2 px-3 py-1.5"
          >
            <AlertDescription
              class="min-w-0 truncate text-xs leading-5"
              :title="errorMessage || successMessage"
            >
              {{ errorMessage || successMessage }}
            </AlertDescription>
            <Button
              variant="ghost"
              size="icon-sm"
              class="size-6 shrink-0"
              :class="
                errorMessage
                  ? 'text-destructive hover:bg-destructive/15 hover:text-destructive'
                  : 'text-emerald-800 hover:bg-emerald-500/15 hover:text-emerald-900 dark:text-emerald-200 dark:hover:text-emerald-100'
              "
              :aria-label="errorMessage ? t('closeError') : t('closeSuccess')"
              @click="dismissMessage(errorMessage ? 'error' : 'success')"
            >
              <X class="size-3.5" />
            </Button>
          </Alert>
        </div>
      </header>

      <p v-if="loadingConfig" class="py-12 text-center text-sm text-muted-foreground">
        {{ t('loadingSettings') }}
      </p>

      <div v-else-if="config" class="flex min-h-0 flex-1 flex-col gap-3">
        <section data-testid="print-section" data-tour="print-settings" class="grid shrink-0 gap-2">
          <div class="flex items-center justify-between gap-2">
            <h2 class="text-sm font-medium">{{ t('printSection') }}</h2>
            <div class="flex gap-1">
              <Button
                variant="ghost"
                size="sm"
                :disabled="loadingPrinters"
                @click="refreshPrinters"
              >
                <RefreshCw class="size-4" :class="{ 'animate-spin': loadingPrinters }" />
                {{ t('refresh') }}
              </Button>
              <Button
                variant="ghost"
                size="sm"
                :disabled="!canTestPrint || testingPrint"
                @click="handleTestPrint"
              >
                <Printer class="size-4" />
                {{ testingPrint ? t('submitting') : t('testPrint') }}
              </Button>
            </div>
          </div>
          <Select
            :model-value="selectedPrinter"
            @update:model-value="handlePrinterChange(String($event))"
          >
            <SelectTrigger id="default-printer" class="w-full" :aria-label="t('defaultPrinter')">
              <span class="flex min-w-0 items-center gap-2">
                <span
                  data-testid="printer-availability"
                  :data-printer="selectedPrinterInfo?.name ?? ''"
                  :data-availability="selectedPrinterInfo?.availability ?? 'unknown'"
                  class="size-2.5 shrink-0 rounded-full ring-1 ring-black/10"
                  :class="printerAvailabilityClass(selectedPrinterInfo?.availability)"
                  :aria-label="printerAvailabilityLabel(selectedPrinterInfo?.availability)"
                />
                <SelectValue :placeholder="t('selectPrinter')" />
              </span>
            </SelectTrigger>
            <SelectContent>
              <SelectItem v-for="printer in printers" :key="printer.name" :value="printer.name">
                <template #leading>
                  <span
                    class="size-2.5 shrink-0 rounded-full ring-1 ring-black/10"
                    :class="printerAvailabilityClass(printer.availability)"
                  />
                </template>
                {{ printer.name }}{{ printer.is_default ? ` (${t('systemDefault')})` : '' }}
              </SelectItem>
            </SelectContent>
          </Select>
          <div class="grid grid-cols-[minmax(0,1fr)_4.5rem_4.5rem] gap-2">
            <Select
              :model-value="selectedPaper"
              @update:model-value="selectedPaper = String($event)"
            >
              <SelectTrigger id="default-paper" class="w-full" :aria-label="t('defaultPaper')">
                <SelectValue :placeholder="t('selectPaper')" />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="custom"> {{ t('customSize') }} </SelectItem>
                <SelectItem v-for="paper in papers" :key="paper.id" :value="paper.id">
                  {{ formatPaperLabel(paper) }}
                </SelectItem>
              </SelectContent>
            </Select>
            <Input
              id="paper-width"
              type="number"
              min="1"
              step="0.1"
              :aria-label="t('widthMm')"
              :model-value="config.printing.default_paper?.width_mm ?? DEFAULT_PAPER.width_mm"
              @update:model-value="setPaperDimension('width_mm', $event)"
            />
            <Input
              id="paper-height"
              type="number"
              min="1"
              step="0.1"
              :aria-label="t('heightMm')"
              :model-value="config.printing.default_paper?.height_mm ?? DEFAULT_PAPER.height_mm"
              @update:model-value="setPaperDimension('height_mm', $event)"
            />
          </div>
        </section>

        <div class="border-t" />

        <section
          data-testid="access-section"
          data-tour="access-settings"
          class="flex min-h-0 flex-1 flex-col gap-2"
        >
          <div class="flex shrink-0 items-center justify-between gap-2">
            <h2 class="text-sm font-medium">{{ t('whoCanConnect') }}</h2>
            <Button
              variant="ghost"
              size="sm"
              data-testid="access-qr-button"
              :aria-label="t('qrCode')"
              @click="openConnectionOverlay"
            >
              <QrCode class="size-4" />
              {{ t('qrCode') }}
            </Button>
          </div>
          <div class="flex shrink-0 gap-4 text-sm" role="tablist" :aria-label="t('whoCanConnect')">
            <button
              type="button"
              role="tab"
              data-testid="access-websites-tab"
              :aria-selected="accessTab === 'websites'"
              :class="[preferenceOptionClass(accessTab === 'websites'), 'py-1']"
              @click="accessTab = 'websites'"
            >
              {{ t('websites') }}
            </button>
            <button
              type="button"
              role="tab"
              data-testid="access-devices-tab"
              :aria-selected="accessTab === 'devices'"
              :class="[preferenceOptionClass(accessTab === 'devices'), 'py-1']"
              @click="accessTab = 'devices'"
            >
              {{ t('devices') }}
            </button>
          </div>
          <form
            v-if="accessTab === 'websites'"
            data-testid="add-origin"
            class="flex shrink-0 items-start gap-2"
            @submit.prevent="addOrigin"
          >
            <div class="grid min-w-0 flex-1 gap-1">
              <Input
                v-model="originDraft"
                placeholder="https://example.com"
                autocomplete="off"
                :aria-invalid="originErrorMessage ? 'true' : 'false'"
              />
              <p class="min-h-4 text-xs text-destructive">
                {{ originErrorMessage }}
              </p>
            </div>
            <Button type="submit" size="sm" :disabled="savingOrigins || !originDraft.trim()">
              {{ t('add') }}
            </Button>
          </form>
          <div v-else data-testid="device-section" class="flex min-h-0 flex-1 flex-col gap-2">
            <form
              data-testid="add-device"
              class="grid shrink-0 gap-2"
              @submit.prevent="addAllowedIp"
            >
              <div class="flex items-start gap-2">
                <div class="grid min-w-0 flex-1 gap-1">
                  <Input
                    data-testid="device-input"
                    v-model="ipDraft"
                    placeholder="192.168.1.0/24"
                    autocomplete="off"
                    :aria-invalid="ipErrorMessage ? 'true' : 'false'"
                  />
                  <p class="min-h-4 text-xs text-destructive">
                    {{ ipErrorMessage }}
                  </p>
                </div>
                <Button type="submit" size="sm" :disabled="savingOrigins || !ipDraft.trim()">
                  {{ t('add') }}
                </Button>
              </div>
              <button
                v-if="lanCidr"
                type="button"
                data-testid="allow-this-network"
                class="w-fit text-sm text-muted-foreground underline-offset-4 hover:underline"
                :disabled="savingOrigins"
                @click="allowThisNetwork"
              >
                {{ t('allowThisNetwork') }}
              </button>
            </form>
            <div data-testid="device-list" class="min-h-0 flex-1 overflow-y-auto">
              <div
                v-for="entry in visibleDevices"
                :key="entry"
                class="flex items-center justify-between gap-2 py-1 text-sm"
              >
                <span class="truncate">{{ entry }}</span>
                <Button
                  variant="ghost"
                  size="icon-sm"
                  :aria-label="t('deleteDevice')"
                  :disabled="savingOrigins"
                  @click="removeAllowedIp(entry)"
                >
                  <Trash2 class="size-4" />
                </Button>
              </div>
              <p v-if="visibleDevices.length === 0" class="py-1 text-sm text-muted-foreground">
                {{ t('noAllowedDevices') }}
              </p>
            </div>
          </div>
          <div
            v-if="accessTab === 'websites'"
            data-testid="origin-list"
            class="min-h-0 flex-1 overflow-y-auto"
          >
            <div
              v-for="origin in config.security.allowed_origins"
              :key="origin"
              class="flex items-center justify-between gap-2 py-1 text-sm"
            >
              <span class="truncate">{{ origin }}</span>
              <Button
                variant="ghost"
                size="icon-sm"
                :aria-label="t('deleteOrigin')"
                :disabled="savingOrigins"
                @click="removeOrigin(origin)"
              >
                <Trash2 class="size-4" />
              </Button>
            </div>
            <p
              v-if="config.security.allowed_origins.length === 0"
              class="py-1 text-sm text-muted-foreground"
            >
              {{ t('noAllowedOrigins') }}
            </p>
          </div>
        </section>

        <div class="border-t" />

        <section
          data-testid="recent-section"
          data-tour="task-history"
          class="grid shrink-0 gap-1.5"
        >
          <div class="flex items-center justify-between gap-2">
            <h2 class="text-sm font-medium">{{ t('recent') }}</h2>
            <Button
              variant="ghost"
              size="sm"
              :disabled="loadingTaskHistory || clearingTaskHistory"
              @click="refreshTaskHistory"
            >
              <RefreshCw class="size-4" :class="{ 'animate-spin': loadingTaskHistory }" />
              {{ t('refresh') }}
            </Button>
          </div>

          <p v-if="taskHistory.length === 0" class="text-sm text-muted-foreground">
            {{ loadingTaskHistory ? t('loadingTasks') : t('noTasks') }}
          </p>
          <div v-else class="grid">
            <button
              v-for="entry in visibleTasks"
              :key="entry.job_id"
              type="button"
              :data-testid="`recent-task-row-${entry.job_id}`"
              class="flex items-center justify-between gap-2 py-1 text-left text-sm"
              @click="selectTask(entry.job_id)"
            >
              <span class="truncate">
                {{ formatTaskTime(entry.updated_at) }}
                <span class="text-muted-foreground">{{ taskSourceLabel(entry.source) }}</span>
              </span>
              <Badge :variant="taskStatusVariant(entry.current_status)">
                {{ taskStatusLabel(entry.current_status) }}
              </Badge>
            </button>
          </div>
          <button
            v-if="hasMoreTasks"
            type="button"
            data-testid="show-all-tasks"
            class="text-left text-sm text-muted-foreground underline-offset-4 hover:underline"
            @click="openAllTasks"
          >
            {{ t('allTasks') }}
          </button>
        </section>

        <div data-testid="panel-footer" class="grid shrink-0 gap-2 border-t pt-2">
          <button
            type="button"
            data-testid="more-toggle"
            data-tour="more-settings"
            class="flex w-full items-center justify-between text-sm font-medium"
            :aria-expanded="moreOpen"
            @click="moreOpen = true"
          >
            <span>{{ t('more') }}</span>
            <ChevronDown class="size-4 -rotate-90" />
          </button>
          <Button
            v-if="hasUnsavedSettings"
            data-testid="save-settings"
            class="w-full"
            :disabled="saving"
            @click="persistConfig"
          >
            <Save class="size-4" />
            {{ saving ? t('saving') : t('save') }}
          </Button>
        </div>
      </div>

      <div
        v-if="qrOpen"
        data-testid="connection-overlay"
        class="fixed inset-0 z-50 grid place-items-end bg-background/80 p-4 backdrop-blur-sm"
        @click.self="qrOpen = false"
      >
        <Card class="flex w-full max-w-md flex-col">
          <CardHeader class="flex flex-row items-center justify-between pb-3">
            <CardTitle class="text-base">{{ t('connectThisComputer') }}</CardTitle>
            <Button
              variant="ghost"
              size="icon-sm"
              :aria-label="t('cancel')"
              @click="qrOpen = false"
            >
              <X class="size-4" />
            </Button>
          </CardHeader>
          <CardContent class="grid gap-4">
            <div class="grid place-items-center">
              <img v-if="qrDataUrl" :src="qrDataUrl" :alt="t('qrCode')" class="size-48" />
              <p v-else class="py-12 text-sm text-muted-foreground">
                {{ t('lanUnavailable') }}
              </p>
            </div>
            <div class="flex items-center gap-2">
              <span data-testid="connection-url" class="min-w-0 flex-1 truncate font-mono text-sm">
                {{ connectionUrl ?? t('lanUnavailable') }}
              </span>
              <Button
                data-testid="copy-connection"
                variant="outline"
                size="sm"
                :disabled="!connectionUrl || lanCopied"
                @click="copyLanAddress"
              >
                {{ lanCopied ? t('lanCopied') : t('copyLan') }}
              </Button>
            </div>
            <p class="text-xs text-muted-foreground">{{ t('thisComputerAlwaysAllowed') }}</p>
          </CardContent>
        </Card>
      </div>

      <div
        v-if="moreOpen && config"
        data-testid="more-overlay"
        class="fixed inset-0 z-40 grid place-items-end bg-background/80 p-4 backdrop-blur-sm"
        @click.self="moreOpen = false"
      >
        <Card class="flex max-h-[calc(100vh-2rem)] w-full max-w-md flex-col">
          <CardHeader class="flex flex-row items-center justify-between pb-3">
            <CardTitle class="text-base">{{ t('more') }}</CardTitle>
            <Button
              variant="ghost"
              size="icon-sm"
              :aria-label="t('cancel')"
              @click="moreOpen = false"
            >
              <X class="size-4" />
            </Button>
          </CardHeader>
          <CardContent class="grid gap-4 overflow-y-auto">
            <div class="flex items-center justify-between gap-3">
              <div class="grid gap-1">
                <Label for="autostart">{{ t('autostart') }}</Label>
                <p class="text-xs text-muted-foreground">{{ t('autostartHint') }}</p>
              </div>
              <Switch id="autostart" v-model="config.app.autostart" />
            </div>
            <div class="grid gap-2">
              <Label>{{ t('language') }}</Label>
              <div class="flex gap-4 text-sm" role="group" :aria-label="t('language')">
                <button
                  type="button"
                  :class="preferenceOptionClass(currentLanguage === 'zh-CN')"
                  :aria-pressed="currentLanguage === 'zh-CN'"
                  @click="setLanguage('zh-CN')"
                >
                  {{ t('chinese') }}
                </button>
                <button
                  type="button"
                  :class="preferenceOptionClass(currentLanguage === 'en')"
                  :aria-pressed="currentLanguage === 'en'"
                  @click="setLanguage('en')"
                >
                  {{ t('english') }}
                </button>
              </div>
            </div>
            <div class="grid gap-2">
              <Label>{{ t('appearance') }}</Label>
              <div class="flex gap-4 text-sm" role="group" :aria-label="t('appearance')">
                <button
                  type="button"
                  :class="preferenceOptionClass(themeMode === 'light')"
                  :aria-pressed="themeMode === 'light'"
                  @click="setThemeMode('light')"
                >
                  {{ t('light') }}
                </button>
                <button
                  type="button"
                  :class="preferenceOptionClass(themeMode === 'dark')"
                  :aria-pressed="themeMode === 'dark'"
                  @click="setThemeMode('dark')"
                >
                  {{ t('dark') }}
                </button>
                <button
                  type="button"
                  :class="preferenceOptionClass(themeMode === 'system')"
                  :aria-pressed="themeMode === 'system'"
                  @click="setThemeMode('system')"
                >
                  {{ t('system') }}
                </button>
              </div>
            </div>
            <div class="grid gap-2">
              <Label for="service-port">{{ t('localPort') }}</Label>
              <Input
                id="service-port"
                type="number"
                :min="MIN_SERVICE_PORT"
                :max="MAX_SERVICE_PORT"
                :model-value="config.service.port"
                @update:model-value="setPort"
              />
              <p v-if="hasPendingPortChange" class="text-xs text-muted-foreground">
                {{ t('portPendingPrefix') }} {{ activePort }}{{ t('portPendingSuffix') }}
              </p>
            </div>
            <div class="flex flex-wrap gap-2">
              <Button
                data-testid="export-config"
                variant="outline"
                size="sm"
                :disabled="exportingConfig"
                @click="openExportDialog"
              >
                <FileDown class="size-4" />
                {{ t('exportConfig') }}
              </Button>
              <Button
                data-testid="import-config"
                variant="outline"
                size="sm"
                :disabled="importingConfig"
                @click="openImportDialog"
              >
                <FileUp class="size-4" />
                {{ t('importConfig') }}
              </Button>
              <Button
                data-testid="export-diagnostics"
                variant="outline"
                size="sm"
                :disabled="exportingDiagnostics"
                @click="handleExportDiagnostics"
              >
                <FileArchive class="size-4" />
                {{ t('exportDiagnostics') }}
              </Button>
            </div>
          </CardContent>
        </Card>
      </div>

      <div
        v-if="showingAllTasks"
        data-testid="all-tasks-overlay"
        class="fixed inset-0 z-40 grid place-items-end bg-background/80 p-4 backdrop-blur-sm"
        @click.self="showingAllTasks = false"
      >
        <Card class="flex max-h-[calc(100vh-2rem)] w-full max-w-md flex-col">
          <CardHeader class="flex flex-row items-center justify-between pb-3">
            <CardTitle class="text-base">{{ t('allTasks') }}</CardTitle>
            <div class="flex items-center gap-1">
              <Button
                :variant="confirmingClearTaskHistory ? 'destructive' : 'ghost'"
                size="sm"
                :disabled="loadingTaskHistory || clearingTaskHistory || taskHistory.length === 0"
                @click="handleClearTaskHistory"
              >
                <Trash2 class="size-4" />
                {{ confirmingClearTaskHistory ? t('confirmClear') : t('clear') }}
              </Button>
              <Button
                variant="ghost"
                size="icon-sm"
                :aria-label="t('cancel')"
                @click="showingAllTasks = false"
              >
                <X class="size-4" />
              </Button>
            </div>
          </CardHeader>
          <CardContent class="grid overflow-y-auto">
            <p v-if="taskHistory.length === 0" class="text-sm text-muted-foreground">
              {{ loadingTaskHistory ? t('loadingTasks') : t('noTasks') }}
            </p>
            <button
              v-for="entry in taskHistory"
              v-else
              :key="entry.job_id"
              type="button"
              :data-testid="`all-task-row-${entry.job_id}`"
              class="flex items-center justify-between gap-2 border-b py-2 text-left text-sm last:border-b-0"
              @click="selectTask(entry.job_id)"
            >
              <span class="truncate">
                {{ formatTaskTime(entry.updated_at) }}
                <span class="text-muted-foreground">{{ taskSourceLabel(entry.source) }}</span>
              </span>
              <Badge :variant="taskStatusVariant(entry.current_status)">
                {{ taskStatusLabel(entry.current_status) }}
              </Badge>
            </button>
          </CardContent>
        </Card>
      </div>

      <div
        v-if="selectedTaskJobId"
        data-testid="task-details"
        class="fixed inset-0 z-50 grid place-items-end bg-background/80 p-4 backdrop-blur-sm"
        @click.self="closeTaskDetails"
      >
        <Card class="flex max-h-[calc(100vh-2rem)] w-full max-w-md flex-col">
          <CardHeader class="flex flex-row items-start justify-between gap-3 pb-3">
            <div class="min-w-0">
              <CardTitle class="truncate text-base">
                {{ selectedTask?.job_id ?? t('taskDetails') }}
              </CardTitle>
              <p v-if="selectedTask" class="mt-1 text-xs text-muted-foreground">
                {{ selectedTask.paper_name ?? '-' }} · {{ selectedTask.copies ?? '-' }}
                {{ t('copiesUnit') }}
              </p>
            </div>
            <Button
              variant="ghost"
              size="icon-sm"
              :aria-label="t('cancel')"
              @click="closeTaskDetails"
            >
              <X class="size-4" />
            </Button>
          </CardHeader>
          <CardContent class="grid gap-2 overflow-y-auto">
            <p v-if="loadingTaskEvents" class="text-sm text-muted-foreground">
              {{ t('loadingStatus') }}
            </p>
            <p v-else-if="selectedTaskEvents.length === 0" class="text-sm text-muted-foreground">
              {{ t('noStatusRecords') }}
            </p>
            <div v-else class="grid gap-2">
              <div
                v-for="event in selectedTaskEvents"
                :key="event.id"
                class="grid gap-1 border-l-2 border-muted-foreground/30 pl-3"
              >
                <div class="flex flex-wrap items-center gap-2">
                  <Badge :variant="taskStatusVariant(event.status)">
                    {{ taskStatusLabel(event.status) }}
                  </Badge>
                  <span class="text-xs text-muted-foreground">
                    {{ formatDateTime(event.occurred_at) }}
                  </span>
                </div>
                <div class="break-words text-sm">
                  {{ event.message ?? '-' }}
                </div>
              </div>
            </div>
          </CardContent>
        </Card>
      </div>

      <div
        v-if="showExportDialog"
        data-testid="export-overlay"
        class="fixed inset-0 z-50 grid place-items-center bg-background/80 p-4 backdrop-blur-sm"
        @click.self="showExportDialog = false"
      >
        <Card class="w-full max-w-sm">
          <CardHeader class="pb-3">
            <CardTitle class="text-base">{{ t('exportConfig') }}</CardTitle>
          </CardHeader>
          <CardContent class="grid gap-4">
            <div class="grid gap-3">
              <label class="flex items-center gap-2 text-sm">
                <input v-model="exportOptions.service_port" type="checkbox" />
                {{ t('localPort') }}
              </label>
              <label class="flex items-center gap-2 text-sm">
                <input v-model="exportOptions.allowed_origins" type="checkbox" />
                {{ t('websites') }}
              </label>
              <label data-testid="export-allowed-ips" class="flex items-center gap-2 text-sm">
                <input v-model="exportOptions.allowed_ips" type="checkbox" />
                {{ t('devices') }}
              </label>
            </div>

            <div class="grid gap-2">
              <Label for="export-password">{{ t('password') }}</Label>
              <Input id="export-password" v-model="exportPassword" type="password" />
            </div>

            <div class="flex flex-wrap justify-end gap-2">
              <Button
                variant="outline"
                :disabled="exportingConfig"
                @click="showExportDialog = false"
              >
                {{ t('cancel') }}
              </Button>
              <Button :disabled="exportingConfig" @click="handleExportConfig">
                {{ exportingConfig ? t('exporting') : t('export') }}
              </Button>
            </div>
          </CardContent>
        </Card>
      </div>

      <div
        v-if="showImportDialog"
        data-testid="import-overlay"
        class="fixed inset-0 z-50 grid place-items-center bg-background/80 p-4 backdrop-blur-sm"
        @click.self="showImportDialog = false"
      >
        <Card class="flex max-h-[calc(100vh-2rem)] w-full max-w-sm flex-col">
          <CardHeader class="shrink-0 pb-3">
            <CardTitle class="text-base">{{ t('importConfig') }}</CardTitle>
          </CardHeader>
          <CardContent class="flex min-h-0 flex-1 flex-col gap-4">
            <div class="grid gap-2">
              <Label for="import-path">{{ t('configFile') }}</Label>
              <div class="flex gap-2">
                <Input id="import-path" class="min-w-0 flex-1" :model-value="importPath" readonly />
                <Button
                  variant="outline"
                  :disabled="previewingConfigImport"
                  @click="chooseImportFile"
                >
                  {{ t('choose') }}
                </Button>
              </div>
            </div>

            <div class="grid gap-2">
              <Label for="import-password">{{ t('password') }}</Label>
              <Input
                id="import-password"
                v-model="importPassword"
                type="password"
                @update:model-value="resetImportPreview"
              />
            </div>

            <Alert v-if="importErrorMessage" variant="error">
              <AlertDescription class="min-w-0 break-words">
                {{ importErrorMessage }}
              </AlertDescription>
            </Alert>

            <div v-if="importPreview" class="flex min-h-0 flex-col gap-2 border p-3">
              <div class="shrink-0 text-sm font-medium">{{ t('importChanges') }}</div>
              <div v-if="importPreview.items.length === 0" class="text-sm text-muted-foreground">
                {{ t('noImportChanges') }}
              </div>
              <div v-else class="grid max-h-[35vh] min-h-0 gap-2 overflow-y-auto pr-1">
                <div v-for="item in importPreview.items" :key="item.key" class="grid gap-1 text-sm">
                  <div class="font-medium">{{ item.label }}</div>
                  <div class="break-words text-muted-foreground">
                    {{ item.current }} -> {{ item.next }}
                  </div>
                </div>
              </div>
            </div>

            <div class="flex shrink-0 flex-wrap justify-end gap-2">
              <Button
                variant="outline"
                :disabled="previewingConfigImport || importingConfig"
                @click="showImportDialog = false"
              >
                {{ t('cancel') }}
              </Button>
              <Button
                variant="outline"
                :disabled="!importPath || previewingConfigImport || importingConfig"
                @click="handlePreviewConfigImport"
              >
                {{ previewingConfigImport ? t('previewing') : t('preview') }}
              </Button>
              <Button
                :disabled="
                  !importPreview ||
                  importPreview.items.length === 0 ||
                  previewingConfigImport ||
                  importingConfig
                "
                @click="handleImportConfig"
              >
                {{ importingConfig ? t('importing') : t('confirmImport') }}
              </Button>
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
    <StatusDoctorSheet v-model:open="doctorOpen" @status-change="doctorNeedsAttention = $event" />
  </main>
</template>
