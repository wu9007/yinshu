import { pathToFileURL } from 'node:url';

/** Return whether a SemVer version has a prerelease component. */
export function isPrerelease(version) {
  return version.includes('-');
}

/** Convert a SemVer prerelease separator to Linux package ordering syntax. */
export function toLinuxPackageVersion(version) {
  return version.replace('-', '~');
}

/** Map 1.0.0 → v1.0, 1.0.1 → v1.0.1, 1.0.0-rc.1 → v1.0-rc.1. */
export function toReleaseTag(version) {
  const [core, pre] = version.split('-', 2);
  const parts = core.split('.');
  const short =
    parts.length === 3 && parts[2] === '0' ? `${parts[0]}.${parts[1]}` : core;
  return pre ? `v${short}-${pre}` : `v${short}`;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const [mode, version] = process.argv.slice(2);

  if (!version || !['linux', 'prerelease', 'tag'].includes(mode)) {
    console.error('Usage: node scripts/release-version.mjs <linux|prerelease|tag> <version>');
    process.exit(1);
  }

  if (mode === 'linux') {
    console.log(toLinuxPackageVersion(version));
  } else if (mode === 'prerelease') {
    console.log(isPrerelease(version));
  } else {
    console.log(toReleaseTag(version));
  }
}
