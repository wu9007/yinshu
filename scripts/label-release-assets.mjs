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
    return `印枢-${nsis[1]}-Windows-x64-setup.exe`;
  }

  const msi = filename.match(/^印枢_(.+)_(x64|arm64)(?:_[A-Za-z-]+)?\.msi$/);
  if (msi) {
    return `印枢-${msi[1]}-Windows-${msi[2]}.msi`;
  }

  const dmg = filename.match(/^印枢_(.+)_(aarch64|x64)\.dmg$/);
  if (dmg) {
    const chip = dmg[2] === 'aarch64' ? 'AppleSilicon' : 'Intel';
    return `印枢-${dmg[1]}-macOS-${chip}.dmg`;
  }

  const desktopDeb = filename.match(/^印枢_(.+)_(amd64|arm64)\.deb$/);
  if (desktopDeb) {
    return `印枢-${desktopDeb[1]}-Linux-${ARCH[desktopDeb[2]]}.deb`;
  }

  const appImage = filename.match(/^印枢_(.+)_(amd64|aarch64)\.AppImage$/);
  if (appImage) {
    return `印枢-${appImage[1]}-Linux-${ARCH[appImage[2]]}.AppImage`;
  }

  const desktopRpm = filename.match(/^印枢-(.+)-1\.(x86_64|aarch64)\.rpm$/);
  if (desktopRpm) {
    return `印枢-${desktopRpm[1]}-Linux-${ARCH[desktopRpm[2]]}.rpm`;
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

/** Only rename assets that tauri-action already uploaded to this tag. */
export function selectReleaseAssetsToRelabel(releaseAssetNames, localNames) {
  const local = new Set(localNames);
  const selected = [];
  const seenLabeled = new Set();
  for (const current of releaseAssetNames) {
    const labeled = labelReleaseAssetName(current);
    if (!labeled || labeled === current || seenLabeled.has(labeled)) {
      continue;
    }
    if (!local.has(current)) {
      throw new Error(`release asset ${current} is not in the local bundle directory`);
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
  return (release.assets ?? []).map((asset) => asset.name);
}

function relabelLocalFiles(root, tag) {
  const localByName = collectBundleFiles(root);
  const selected = selectReleaseAssetsToRelabel(listReleaseAssetNames(tag), [
    ...localByName.keys(),
  ]);
  for (const { current, labeled } of selected) {
    const path = localByName.get(current);
    runGh(['release', 'upload', tag, `${path}#${labeled}`, '--clobber']);
    try {
      runGh(['release', 'delete-asset', tag, current, '--yes']);
    } catch (error) {
      if (!String(error).includes('not found')) {
        throw error;
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
