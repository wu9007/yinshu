import { spawnSync } from 'node:child_process';
import { globSync, statSync } from 'node:fs';
import { join, basename } from 'node:path';
import { pathToFileURL } from 'node:url';

const ARCH = {
  x64: 'x64',
  amd64: 'x64',
  x86_64: 'x64',
  arm64: 'arm64',
  aarch64: 'arm64',
};

/** Map a Tauri/headless bundle file name to an OS-labeled release asset name. */
export function labelReleaseAssetName(filename) {
  const nsis = filename.match(/^印枢_(.+)_x64-setup\.exe$/);
  if (nsis) {
    return `yinshu-${nsis[1]}-Windows-x64-setup.exe`;
  }

  const msi = filename.match(/^(?:印枢|yinshu)_(.+)_(x64|arm64)(?:_[A-Za-z-]+)?\.msi$/);
  if (msi) {
    return `yinshu-${msi[1]}-Windows-${msi[2]}.msi`;
  }

  const dmg = filename.match(/^印枢_(.+)_(aarch64|x64)\.dmg$/);
  if (dmg) {
    const chip = dmg[2] === 'aarch64' ? 'AppleSilicon' : 'Intel';
    return `yinshu-${dmg[1]}-macOS-${chip}.dmg`;
  }

  const desktopDeb = filename.match(/^印枢_(.+)_(amd64|arm64)\.deb$/);
  if (desktopDeb) {
    return `yinshu-${desktopDeb[1]}-Linux-${ARCH[desktopDeb[2]]}.deb`;
  }

  const appImage = filename.match(/^印枢_(.+)_(amd64|aarch64)\.AppImage$/);
  if (appImage) {
    return `yinshu-${appImage[1]}-Linux-${ARCH[appImage[2]]}.AppImage`;
  }

  const desktopRpm = filename.match(/^印枢-(.+)-1\.(x86_64|aarch64)\.rpm$/);
  if (desktopRpm) {
    return `yinshu-${desktopRpm[1]}-Linux-${ARCH[desktopRpm[2]]}.rpm`;
  }

  const serverDeb = filename.match(/^yinshu-server_(.+)_(amd64|arm64)\.deb$/);
  if (serverDeb) {
    return `yinshu-server-${serverDeb[1]}-Linux-${ARCH[serverDeb[2]]}.deb`;
  }

  const serverRpm = filename.match(/^yinshu-server-(.+)-1\.(x86_64|aarch64)\.rpm$/);
  if (serverRpm) {
    return `yinshu-server-${serverRpm[1]}-Linux-${ARCH[serverRpm[2]]}.rpm`;
  }

  return null;
}

/** Keep the newest local file when rust-cache leaves duplicate bundle names. */
export function newestBundlePathByName(paths, mtimeMsByPath) {
  const byName = new Map();
  for (const path of paths) {
    const name = basename(path);
    const mtime = mtimeMsByPath.get(path) ?? 0;
    const current = byName.get(name);
    if (!current || mtime > current.mtime) {
      byName.set(name, { path, mtime });
    }
  }
  return new Map([...byName].map(([name, value]) => [name, value.path]));
}

/** GitHub sometimes stores `印枢_1.0.0_x64.dmg` as `_1.0.0_x64.dmg`. */
export function releaseNameCandidates(filename) {
  return filename.startsWith('印枢') ? [filename, filename.slice('印枢'.length)] : [filename];
}

/** `gh` treats `-1.0.0-1.x86_64.rpm` as a flag unless `--` precedes the name. */
export function ghReleaseDeleteAssetArgs(tag, name) {
  return ['release', 'delete-asset', tag, '--yes', '--', name];
}

/** Older drafts used `印枢-1.0.0-Linux-x64.rpm` before filenames switched to yinshu. */
export function previousLabeledAssetName(labeled) {
  if (labeled.startsWith('yinshu-') && !labeled.startsWith('yinshu-server-')) {
    return `印枢-${labeled.slice('yinshu-'.length)}`;
  }
  return null;
}

/** Delete only the candidate names that this release already has. */
export function releaseAssetNamesToDelete(releaseAssetNames, current) {
  const release = new Set(releaseAssetNames);
  const labeled = labelReleaseAssetName(current);
  const names = [...releaseNameCandidates(current)];
  const previous = labeled ? previousLabeledAssetName(labeled) : null;
  if (previous) {
    names.push(previous);
  }
  return names.filter((name) => name !== labeled && release.has(name));
}

/** Collect `name` and `label` from `gh release view --json assets`. */
export function githubAssetNames(assets) {
  return (assets ?? []).flatMap((asset) =>
    [asset.name, asset.label].filter((name) => typeof name === 'string' && name.length > 0),
  );
}

/** Relabel local bundles that this tag already has; skip other matrix leftovers. */
export function selectReleaseAssetsToRelabel(releaseAssetNames, localNames) {
  const release = new Set(releaseAssetNames);
  const selected = [];
  const seenLabeled = new Set();
  for (const current of localNames) {
    const labeled = labelReleaseAssetName(current);
    if (!labeled || labeled === current || seenLabeled.has(labeled)) {
      continue;
    }
    if (!releaseNameCandidates(current).some((name) => release.has(name))) {
      continue;
    }
    seenLabeled.add(labeled);
    selected.push({ current, labeled });
  }
  return selected;
}

function collectBundleFiles(root) {
  const discovered = [
    ...globSync('target/**/release/bundle/**/*', { cwd: root }),
    ...globSync('target/packages/*', { cwd: root }),
  ]
    .map((path) => join(root, path))
    .filter((path) => {
      try {
        return statSync(path).isFile() && labelReleaseAssetName(basename(path));
      } catch {
        return false;
      }
    });
  const mtimes = new Map(discovered.map((path) => [path, statSync(path).mtimeMs]));
  return newestBundlePathByName(discovered, mtimes);
}

function runGh(args) {
  const result = spawnSync('gh', args, { encoding: 'utf8' });
  if (result.status !== 0) {
    throw new Error(result.stderr || result.stdout || `gh ${args.join(' ')} failed`);
  }
  return result.stdout;
}

function listReleaseAssetNames(tag) {
  const release = JSON.parse(runGh(['release', 'view', tag, '--json', 'assets']));
  return githubAssetNames(release.assets);
}

function relabelLocalFiles(root, tag) {
  const localByName = collectBundleFiles(root);
  const releaseAssetNames = listReleaseAssetNames(tag);
  const selected = selectReleaseAssetsToRelabel(releaseAssetNames, [
    ...localByName.keys(),
  ]);
  for (const { current, labeled } of selected) {
    const path = localByName.get(current);
    runGh(['release', 'upload', tag, `${path}#${labeled}`, '--clobber']);
    for (const name of releaseAssetNamesToDelete(releaseAssetNames, current)) {
      try {
        runGh(ghReleaseDeleteAssetArgs(tag, name));
      } catch (error) {
        if (!String(error).includes('not found')) {
          throw error;
        }
      }
    }
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const [mode, ...rest] = process.argv.slice(2);
  if (mode === 'name' && rest[0]) {
    const labeled = labelReleaseAssetName(rest[0]);
    if (!labeled) {
      process.exit(2);
    }
    console.log(labeled);
    process.exit(0);
  }

  if (mode === 'upload') {
    const tagIndex = rest.indexOf('--tag');
    const rootIndex = rest.indexOf('--root');
    const tag = tagIndex >= 0 ? rest[tagIndex + 1] : process.env.RELEASE_TAG;
    const root = rootIndex >= 0 ? rest[rootIndex + 1] : process.cwd();
    if (!tag) {
      console.error('Usage: node scripts/label-release-assets.mjs upload --tag <tag> [--root <dir>]');
      process.exit(1);
    }
    relabelLocalFiles(root, tag);
    process.exit(0);
  }

  console.error('Usage: node scripts/label-release-assets.mjs <name|upload> ...');
  process.exit(1);
}
