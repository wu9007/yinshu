/** Agent 配置中保存的实际纸张尺寸。 */
export interface EffectivePaper {
  width_mm: number;
  height_mm: number;
}

export type DoctorStatus = 'PASS' | 'WARN' | 'FAIL';

export interface DoctorCheck {
  code: string;
  status: DoctorStatus;
  message: string;
  suggestion?: string;
}

export interface DoctorSummary {
  pass: number;
  warn: number;
  fail: number;
}

export interface DoctorReport {
  checks: DoctorCheck[];
  summary: DoctorSummary;
}

/** Tauri UI 和本地 Agent 共享的完整配置。 */
export interface AgentConfig {
  service: {
    host: string;
    port: number;
  };
  security: {
    allowed_origins: string[];
    allowed_ips: string[];
  };
  printing: {
    default_printer: string | null;
    default_paper: EffectivePaper | null;
    default_copies: number;
  };
  limits: {
    max_file_size_mb: number;
    max_batch_jobs: number;
    max_copies: number;
    download_timeout_seconds: number;
  };
  app: {
    autostart: boolean;
    language: 'zh-CN' | 'en';
  };
}

export type PrinterAvailability = 'available' | 'unavailable' | 'unknown';

/** 本地服务返回的打印机摘要。 */
export interface PrinterInfo {
  name: string;
  is_default: boolean;
  dpi: number | null;
  port: string | null;
  is_local: boolean | null;
  is_network: boolean | null;
  is_virtual: boolean | null;
  availability: PrinterAvailability;
}

/** 本地服务返回的纸张尺寸摘要。 */
export interface PaperInfo {
  id: string;
  name: string;
  width_mm: number;
  height_mm: number;
}

export type TaskHistoryStatus =
  | 'queued'
  | 'downloading'
  | 'printing'
  | 'submitted'
  | 'completed'
  | 'failed'
  | 'unknown'
  | 'cancelled';

export type TaskHistorySource = 'web_socket' | 'test';

export interface TaskHistoryJob {
  job_id: string;
  request_id: string | null;
  batch_id: string | null;
  source: TaskHistorySource;
  current_status: TaskHistoryStatus;
  current_message: string | null;
  printer_name: string | null;
  paper_name: string | null;
  copies: number | null;
  created_at: string;
  updated_at: string;
  finished_at: string | null;
}

export interface TaskHistoryEvent {
  id: number;
  job_id: string;
  status: TaskHistoryStatus;
  message: string | null;
  occurred_at: string;
}

export interface ExportConfigOptions {
  service_port: boolean;
  allowed_origins: boolean;
  allowed_ips: boolean;
}

export interface ImportPreviewItem {
  key: string;
  label: string;
  current: string;
  next: string;
}

export interface ImportPreview {
  file_hash: string;
  items: ImportPreviewItem[];
}
