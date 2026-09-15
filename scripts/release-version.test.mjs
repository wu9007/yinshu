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

test('release workflow builds from v* tags on main', () => {
  const workflow = readFileSync('.github/workflows/release.yml', 'utf8');

  assert.match(workflow, /tags:\n\s+- 'v\*'/);
  assert.doesNotMatch(workflow, /workflow_dispatch/);
  assert.doesNotMatch(workflow, /branches:\n\s+- release/);
  assert.doesNotMatch(workflow, /yinshu-v/);
  assert.match(workflow, /release-version\.mjs tag/);
  assert.match(workflow, /Tag must point at a commit on main/);
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

test('release workflow installs AppImage tools and separates NSIS from MSI', () => {
  const workflow = readFileSync('.github/workflows/release.yml', 'utf8');

  assert.match(workflow, /xdg-utils/);
  assert.match(workflow, /--bundles nsis\n/);
  assert.match(workflow, /--bundles msi\n/);
  assert.doesNotMatch(workflow, /--bundles nsis,msi/);
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
