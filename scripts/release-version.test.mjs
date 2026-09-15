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

test('release tags drop a trailing .0 patch', () => {
  assert.equal(toReleaseTag('1.0.0'), 'v1.0');
  assert.equal(toReleaseTag('1.0.1'), 'v1.0.1');
  assert.equal(toReleaseTag('1.1.0'), 'v1.1');
  assert.equal(toReleaseTag('1.0.0-rc.1'), 'v1.0-rc.1');
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
  assert.doesNotMatch(supported, /Windows 7|8\.1/);
  assert.match(supported, /未签名、未公证/);
  assert.match(supported, /HTTPS 页面连不上 `ws:\/\//);
  assert.match(supportedEn, /\| Windows \| 10, 11 \|/);
  assert.doesNotMatch(supportedEn, /Windows 7|8\.1/);
  assert.match(supportedEn, /unsigned and not notarized/);
  assert.match(supportedEn, /HTTPS pages cannot use `ws:\/\//);

  assert.match(backlog, /Windows 7/);
  assert.match(backlog, /麒麟/);
  assert.match(backlog, /没有安装包/);
  assert.match(backlogEn, /Windows 7/);
  assert.match(backlogEn, /Kylin/);
  assert.match(backlogEn, /not supported/);
});

test('MSI uses a Chinese WiX language for the 印枢 product name', () => {
  const config = JSON.parse(readFileSync('apps/desktop/src-tauri/tauri.conf.json', 'utf8'));

  assert.equal(config.productName, '印枢');
  assert.equal(config.bundle.windows.wix.language, 'zh-CN');
});

test('release workflow ad-hoc signs macOS when Apple certificate is missing', () => {
  const workflow = readFileSync('.github/workflows/release.yml', 'utf8');
  const config = JSON.parse(readFileSync('apps/desktop/src-tauri/tauri.conf.json', 'utf8'));

  assert.equal(config.bundle.macOS.signingIdentity, '-');
  assert.match(workflow, /APPLE_SIGNING_IDENTITY=-/);
  assert.doesNotMatch(
    workflow,
    /APPLE_SIGNING_IDENTITY: \$\{\{\s*startsWith\(matrix\.platform, 'macos'\) && secrets\.APPLE_SIGNING_IDENTITY/,
  );
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

test('desktop and headless publishing are independent after release preparation', () => {
  const workflow = readFileSync('.github/workflows/release.yml', 'utf8');

  assert.match(workflow, /prepare-release:\n\s+needs: quality/);
  assert.match(workflow, /publish-tauri:\n\s+needs: prepare-release/);
  assert.match(workflow, /publish-headless:\n\s+needs: prepare-release/);
  assert.doesNotMatch(workflow, /publish-headless:\n\s+needs: publish-tauri/);
});

test('release workflow installs AppImage tools and builds NSIS then MSI on one Windows runner', () => {
  const workflow = readFileSync('.github/workflows/release.yml', 'utf8');

  assert.match(workflow, /xdg-utils/);
  assert.match(workflow, /--bundles nsis\n/);
  assert.match(workflow, /--bundles msi\n/);
  assert.doesNotMatch(workflow, /--bundles nsis,msi/);
  assert.equal((workflow.match(/platform: windows-latest/g) || []).length, 1);
  assert.match(workflow, /Publish Windows MSI/);
  assert.match(workflow, /Prepare Windows CLI sidecar/);
  assert.match(workflow, /prepare-windows-cli\.mjs x86_64-pc-windows-msvc/);
});

test('release workflow publishes after desktop and headless artifacts upload', () => {
  const workflow = readFileSync('.github/workflows/release.yml', 'utf8');

  assert.match(workflow, /gh release create/);
  assert.match(workflow, /ARGS=\([\s\S]*--draft/);
  assert.match(workflow, /releaseDraft: true/);
  assert.match(
    workflow,
    /publish-release:\n\s+needs: \[prepare-release, publish-tauri, publish-headless\]/,
  );
  assert.match(workflow, /gh release edit "\$\{\{ needs\.prepare-release\.outputs\.tag \}\}"[\s\S]*--draft=false/);
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
