import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import test from 'node:test';

import { isPrerelease, toLinuxPackageVersion, toReleaseTag } from './release-version.mjs';

test('stable versions remain stable Linux package versions', () => {
  assert.equal(toLinuxPackageVersion('0.2.0'), '0.2.0');
  assert.equal(isPrerelease('0.2.0'), false);
});

test('SemVer prerelease versions sort before the final Linux package version', () => {
  assert.equal(toLinuxPackageVersion('0.2.0-dev.1'), '0.2.0~dev.1');
  assert.equal(isPrerelease('0.2.0-dev.1'), true);
});

test('release tags keep the full SemVer', () => {
  assert.equal(toReleaseTag('1.0.0'), 'v1.0.0');
  assert.equal(toReleaseTag('1.0.1'), 'v1.0.1');
  assert.equal(toReleaseTag('1.1.0'), 'v1.1.0');
  assert.equal(toReleaseTag('1.0.0-rc.1'), 'v1.0.0-rc.1');
});

function markdownSection(markdown, heading) {
  const start = markdown.indexOf(heading);
  assert.ok(start >= 0, `missing ${heading}`);
  const rest = markdown.slice(start + heading.length);
  const next = rest.search(/\n## /);
  return next === -1 ? rest : rest.slice(0, next);
}

test('README OS table matches CI-backed platforms', () => {
  const readme = readFileSync('README.md', 'utf8');
  const readmeEn = readFileSync('README_en.md', 'utf8');
  const supported = markdownSection(readme, '## 适配的操作系统');
  const supportedEn = markdownSection(readmeEn, '## Supported operating systems');
  const backlog = markdownSection(readme, '## 待办');
  const backlogEn = markdownSection(readmeEn, '## Backlog');

  assert.match(supported, /\| Windows \| 10、11 \|/);
  assert.match(supported, /当前只发布 Windows x64 的 NSIS/);
  assert.doesNotMatch(supported, /当前只发布 Windows x64 的 NSIS.+Apple Silicon/);
  assert.doesNotMatch(supported, /Windows 7|8\.1/);
  assert.match(supported, /未签名、未公证/);
  assert.match(supported, /HTTPS 页面连不上 `ws:\/\//);
  assert.match(supportedEn, /\| Windows \| 10, 11 \|/);
  assert.match(supportedEn, /currently publishes Windows x64 NSIS/);
  assert.doesNotMatch(supportedEn, /currently publishes Windows x64 NSIS.+macOS/);
  assert.doesNotMatch(supportedEn, /Windows 7|8\.1/);
  assert.match(supportedEn, /unsigned and not notarized/);
  assert.match(supportedEn, /HTTPS pages cannot use `ws:\/\//);

  assert.match(backlog, /Windows 7/);
  assert.match(backlog, /麒麟/);
  assert.match(backlog, /龙芯/);
  assert.match(backlogEn, /Windows 7/);
  assert.match(backlogEn, /Kylin/);
  assert.match(backlogEn, /not supported/);
});

test('MSI uses a Chinese WiX language for the 印枢 product name', () => {
  const config = JSON.parse(readFileSync('apps/desktop/src-tauri/tauri.conf.json', 'utf8'));

  assert.equal(config.productName, '印枢');
  assert.equal(config.bundle.windows.wix.language, 'zh-CN');
  assert.equal(config.bundle.windows.webviewInstallMode.type, 'offlineInstaller');
  assert.equal(config.bundle.windows.nsis.installerHooks, 'windows/installer-hooks.nsh');
});

test('release workflow does not publish a Windows MSI', () => {
  const workflow = readFileSync('.github/workflows/release.yml', 'utf8');

  assert.doesNotMatch(workflow, /extra_msi/);
  assert.doesNotMatch(workflow, /--bundles msi/);
  assert.doesNotMatch(workflow, /Publish Windows MSI/);
  assert.doesNotMatch(workflow, /TAURI_WIX_SKIP_MSI_VALIDATION/);
});

test('NSIS installer writes install.log under the app log directory', () => {
  const hooks = readFileSync('apps/desktop/src-tauri/windows/installer-hooks.nsh', 'utf8');

  assert.match(hooks, /NSIS_HOOK_PREINSTALL/);
  assert.match(hooks, /NSIS_HOOK_POSTINSTALL/);
  assert.match(hooks, /\$LOCALAPPDATA\\cn\.yinshu\.app\\logs\\install\.log/);
});

test('release workflow only publishes Windows NSIS', () => {
  const workflow = readFileSync('.github/workflows/release.yml', 'utf8');

  assert.match(workflow, /runs-on: windows-2022/);
  assert.match(workflow, /--target x86_64-pc-windows-msvc --bundles nsis/);
  assert.doesNotMatch(workflow, /macos-latest|macos-15-intel/);
  assert.doesNotMatch(workflow, /aarch64-apple-darwin|x86_64-apple-darwin/);
  assert.doesNotMatch(workflow, /ubuntu-22\.04-arm/);
  assert.doesNotMatch(workflow, /publish-headless/);
  assert.doesNotMatch(workflow, /Import Apple Developer certificate/);
  assert.doesNotMatch(workflow, /xdg-utils/);
});

test('release workflow builds from v* tags on main', () => {
  const workflow = readFileSync('.github/workflows/release.yml', 'utf8');

  assert.match(workflow, /tags:\n\s+- 'v\*'/);
  assert.doesNotMatch(workflow, /workflow_dispatch/);
  assert.doesNotMatch(workflow, /branches:\n\s+- release/);
  assert.doesNotMatch(workflow, /yinshu-v/);
  assert.match(workflow, /release-version\.mjs tag/);
  assert.match(workflow, /Tag must point at a commit on main/);
  assert.match(workflow, /swatinem\/rust-cache@v2/);
  assert.match(workflow, /CARGO_BUILD_JOBS: "1"/);
  assert.match(workflow, /CARGO_PROFILE_TEST_DEBUG: "0"/);
  assert.doesNotMatch(workflow, /cargo check --workspace/);
  assert.doesNotMatch(workflow, /pnpm --dir apps\/desktop build/);
});

test('release workflow marks SemVer prereleases as GitHub prereleases', () => {
  const workflow = readFileSync('.github/workflows/release.yml', 'utf8');

  assert.match(workflow, /release-version\.mjs prerelease/);
  assert.match(workflow, /prerelease: \$\{\{ steps\.release_version\.outputs\.prerelease \}\}/);
});

test('desktop publishing starts after release preparation', () => {
  const workflow = readFileSync('.github/workflows/release.yml', 'utf8');

  assert.match(workflow, /prepare-release:\n\s+needs: quality/);
  assert.match(workflow, /publish-tauri:\n\s+needs: prepare-release/);
});

test('release workflow builds NSIS on one Windows runner', () => {
  const workflow = readFileSync('.github/workflows/release.yml', 'utf8');

  assert.match(workflow, /--bundles nsis/);
  assert.doesNotMatch(workflow, /--bundles nsis,msi/);
  assert.equal((workflow.match(/windows-2022/g) || []).length, 1);
  assert.doesNotMatch(workflow, /windows-latest/);
  assert.match(workflow, /Prepare Windows CLI sidecar/);
  assert.match(workflow, /prepare-windows-cli\.mjs x86_64-pc-windows-msvc/);
});

test('release workflow publishes after the Windows installer uploads', () => {
  const workflow = readFileSync('.github/workflows/release.yml', 'utf8');

  assert.match(workflow, /gh release create/);
  assert.match(workflow, /ARGS=\([\s\S]*--draft/);
  assert.match(workflow, /releaseDraft: true/);
  assert.doesNotMatch(workflow, /publish-release:/);
  assert.match(
    workflow,
    /Label release installers[\s\S]*gh release edit "\$\{\{ needs\.prepare-release\.outputs\.tag \}\}"[\s\S]*--draft=false/,
  );
});

test('headless packaging uses the normalized Linux package version', () => {
  const script = readFileSync('scripts/build-server-packages.sh', 'utf8');
  const controlTemplate = readFileSync('apps/server/packaging/deb/control', 'utf8');
  const renderedControl = controlTemplate
    .replace('${VERSION}', '0.2.0~dev.2')
    .replace('${ARCH}', 'amd64');

  assert.match(script, /release-version\.mjs" linux/);
  assert.ok(script.includes('s/\\${VERSION}/$PACKAGE_VERSION/'));
  assert.ok(script.includes('s/__VERSION__/$PACKAGE_VERSION/'));
  assert.match(renderedControl, /^Version: [0-9]/m);
  assert.match(renderedControl, /^Architecture: amd64$/m);
});
