import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';

import {
  labelReleaseAssetName,
  newestBundlePathByName,
  selectReleaseAssetsToRelabel,
} from './label-release-assets.mjs';

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
  assert.match(workflow, /Clear cached installer bundles/);
  assert.match(workflow, /rm -rf target\/\*\/release\/bundle target\/release\/bundle/);
});

test('upload only relabels assets already on the GitHub release', () => {
  const selected = selectReleaseAssetsToRelabel(
    ['印枢_1.0.1_x64-setup.exe', '印枢-1.0.1-Windows-x64-setup.exe'],
    ['印枢_1.0.1_x64-setup.exe', '印枢_1.0.0_x64-setup.exe'],
  );

  assert.deepEqual(selected, [
    {
      current: '印枢_1.0.1_x64-setup.exe',
      labeled: '印枢-1.0.1-Windows-x64-setup.exe',
    },
  ]);
});

test('upload refuses to label a release asset that is not on disk', () => {
  assert.throws(
    () => selectReleaseAssetsToRelabel(['印枢_1.0.1_x64-setup.exe'], []),
    /印枢_1\.0\.1_x64-setup\.exe is not in the local bundle directory/,
  );
});

test('duplicate cached bundle names keep the newest file', () => {
  const newest = newestBundlePathByName(
    [
      'target/release/bundle/nsis/印枢_1.0.0_x64-setup.exe',
      'target/x86_64-pc-windows-msvc/release/bundle/nsis/印枢_1.0.0_x64-setup.exe',
    ],
    new Map([
      ['target/release/bundle/nsis/印枢_1.0.0_x64-setup.exe', 1],
      [
        'target/x86_64-pc-windows-msvc/release/bundle/nsis/印枢_1.0.0_x64-setup.exe',
        2,
      ],
    ]),
  );

  assert.equal(
    newest.get('印枢_1.0.0_x64-setup.exe'),
    'target/x86_64-pc-windows-msvc/release/bundle/nsis/印枢_1.0.0_x64-setup.exe',
  );
});
