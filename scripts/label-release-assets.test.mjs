import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';

import { labelReleaseAssetName } from './label-release-assets.mjs';

test('desktop installers include the operating system in the file name', () => {
  assert.equal(
    labelReleaseAssetName('印枢_1.0.0_x64-setup.exe'),
    '印枢-1.0.0-Windows-x64-setup.exe',
  );
  assert.equal(
    labelReleaseAssetName('印枢_1.0.0_x64_zh-CN.msi'),
    '印枢-1.0.0-Windows-x64.msi',
  );
  assert.equal(
    labelReleaseAssetName('印枢_1.0.0_aarch64.dmg'),
    '印枢-1.0.0-macOS-AppleSilicon.dmg',
  );
  assert.equal(
    labelReleaseAssetName('印枢_1.0.0_x64.dmg'),
    '印枢-1.0.0-macOS-Intel.dmg',
  );
  assert.equal(
    labelReleaseAssetName('印枢_1.0.0_amd64.deb'),
    '印枢-1.0.0-Linux-x64.deb',
  );
  assert.equal(
    labelReleaseAssetName('印枢_1.0.0_arm64.deb'),
    '印枢-1.0.0-Linux-arm64.deb',
  );
  assert.equal(
    labelReleaseAssetName('印枢_1.0.0_amd64.AppImage'),
    '印枢-1.0.0-Linux-x64.AppImage',
  );
  assert.equal(
    labelReleaseAssetName('印枢_1.0.0_aarch64.AppImage'),
    '印枢-1.0.0-Linux-arm64.AppImage',
  );
  assert.equal(
    labelReleaseAssetName('印枢-1.0.0-1.x86_64.rpm'),
    '印枢-1.0.0-Linux-x64.rpm',
  );
  assert.equal(
    labelReleaseAssetName('印枢-1.0.0-1.aarch64.rpm'),
    '印枢-1.0.0-Linux-arm64.rpm',
  );
});

test('headless packages include Linux in the file name', () => {
  assert.equal(
    labelReleaseAssetName('yinshu-server_1.0.0_amd64.deb'),
    'yinshu-server-1.0.0-Linux-x64.deb',
  );
  assert.equal(
    labelReleaseAssetName('yinshu-server-1.0.0-1.aarch64.rpm'),
    'yinshu-server-1.0.0-Linux-arm64.rpm',
  );
});

test('unknown or already labeled names are left unchanged', () => {
  assert.equal(labelReleaseAssetName('latest.json'), null);
  assert.equal(labelReleaseAssetName('印枢-1.0.0-Windows-x64-setup.exe'), null);
  assert.equal(labelReleaseAssetName('印枢_1.0.0_x64-setup.exe.sig'), null);
});

test('release workflow relabels desktop and headless assets before publishing', () => {
  const workflow = readFileSync('.github/workflows/release.yml', 'utf8');

  assert.match(workflow, /label-release-assets\.mjs/);
  assert.match(workflow, /scripts\/label-release-assets\.test\.mjs/);
});
