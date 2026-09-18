import type { DoctorCheck } from '@/types';

export type DoctorGroupKey = 'core' | 'printing' | 'other';

interface DoctorDescriptor {
  key: string;
  group: DoctorGroupKey;
}

export interface PresentedDoctorCheck {
  check: DoctorCheck;
  group: DoctorGroupKey;
  titleKey: string;
  resultKey: string;
  purposeKey: string;
  suggestionKey: string | null;
  technicalCode: string | null;
}

export interface DoctorCheckGroup {
  key: DoctorGroupKey;
  labelKey: string;
  checks: PresentedDoctorCheck[];
}

const DESCRIPTORS: Record<string, DoctorDescriptor> = {
  'config.valid': { key: 'configValid', group: 'core' },
  'data.directory': { key: 'dataDirectory', group: 'core' },
  'agent.ipc': { key: 'agentIpc', group: 'core' },
  'service.port': { key: 'servicePort', group: 'core' },
  'printing.printers': { key: 'printers', group: 'core' },
  'browser.available': { key: 'browser', group: 'printing' },
  'office.docx': { key: 'docx', group: 'printing' },
  'office.xlsx': { key: 'xlsx', group: 'printing' },
  'office.pptx': { key: 'pptx', group: 'printing' },
};

const GROUP_ORDER: DoctorGroupKey[] = ['core', 'printing', 'other'];
const CODE_ORDER = Object.keys(DESCRIPTORS);

function rank(code: string): number {
  const index = CODE_ORDER.indexOf(code);
  return index === -1 ? CODE_ORDER.length : index;
}

export function presentDoctorCheck(check: DoctorCheck): PresentedDoctorCheck {
  const descriptor = DESCRIPTORS[check.code];
  const key = descriptor?.key ?? 'unknown';
  const group = descriptor?.group ?? 'other';
  const status = check.status.toLowerCase();
  const base = `doctor.checks.${key}`;

  return {
    check,
    group,
    titleKey: `${base}.title`,
    resultKey: `${base}.${status}`,
    purposeKey: `${base}.purpose`,
    suggestionKey: check.status === 'PASS' ? null : `${base}.${status}Suggestion`,
    technicalCode: descriptor ? null : check.code,
  };
}

export function groupDoctorChecks(checks: DoctorCheck[]): DoctorCheckGroup[] {
  const presented = checks.map(presentDoctorCheck);
  return GROUP_ORDER.map((key) => ({
    key,
    labelKey: `doctor.groups.${key}`,
    checks: presented
      .filter((check) => check.group === key)
      .sort((a, b) => rank(a.check.code) - rank(b.check.code)),
  })).filter((group) => group.checks.length > 0);
}
