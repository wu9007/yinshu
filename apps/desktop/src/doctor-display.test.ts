import { describe, expect, it } from 'vitest';
import { messages } from '@/i18n';
import { groupDoctorChecks, presentDoctorCheck } from '@/doctor-display';
import type { DoctorCheck } from '@/types';

const check = (code: string, status: DoctorCheck['status']): DoctorCheck => ({
  code,
  status,
  message: 'backend English must not be rendered',
  suggestion: 'backend English suggestion must not be rendered',
});

function hasPath(value: unknown, path: string): boolean {
  let current = value;
  for (const segment of path.split('.')) {
    if (!current || typeof current !== 'object' || !(segment in current)) return false;
    current = (current as Record<string, unknown>)[segment];
  }
  return true;
}

describe('doctor display mapping', () => {
  it('groups known checks in the designed order', () => {
    const groups = groupDoctorChecks([
      check('office.pptx', 'WARN'),
      check('printing.printers', 'PASS'),
      check('config.valid', 'PASS'),
    ]);

    expect(groups.map((group) => group.key)).toEqual(['core', 'printing']);
    expect(groups[0].checks.map((item) => item.check.code)).toEqual([
      'config.valid',
      'printing.printers',
    ]);
    expect(groups[1].checks.map((item) => item.check.code)).toEqual(['office.pptx']);
  });

  it('keeps unknown checks visible with a technical code', () => {
    expect(presentDoctorCheck(check('future.check', 'WARN'))).toMatchObject({
      group: 'other',
      titleKey: 'doctor.checks.unknown.title',
      resultKey: 'doctor.checks.unknown.warn',
      purposeKey: 'doctor.checks.unknown.purpose',
      suggestionKey: 'doctor.checks.unknown.warnSuggestion',
      technicalCode: 'future.check',
    });
  });

  it('defines every generated key in Chinese and English', () => {
    const known = [
      'config.valid',
      'data.directory',
      'agent.ipc',
      'service.port',
      'printing.printers',
      'browser.available',
      'office.docx',
      'office.xlsx',
      'office.pptx',
      'future.check',
    ];

    for (const code of known) {
      for (const status of ['PASS', 'WARN', 'FAIL'] as const) {
        const item = presentDoctorCheck(check(code, status));
        for (const locale of ['zh-CN', 'en'] as const) {
          expect(hasPath(messages[locale], item.titleKey)).toBe(true);
          expect(hasPath(messages[locale], item.resultKey)).toBe(true);
          expect(hasPath(messages[locale], item.purposeKey)).toBe(true);
          if (item.suggestionKey) {
            expect(hasPath(messages[locale], item.suggestionKey)).toBe(true);
          }
        }
      }
    }
  });
});
