#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -n 1)
PACKAGE_VERSION=$(node "$ROOT/scripts/release-version.mjs" linux "$VERSION")
TARGET=${CARGO_BUILD_TARGET:-$(rustc -vV | sed -n 's/^host: //p')}
ARCH=${PACKAGE_ARCH:-amd64}
OUT="$ROOT/target/packages"
STAGE="$ROOT/target/package-stage"

cargo build --release -p yinshu-server --target "$TARGET"
rm -rf "$STAGE"
mkdir -p "$STAGE/deb/DEBIAN" "$STAGE/deb/usr/bin" "$STAGE/deb/lib/systemd/system" "$OUT"
install -m 0755 "$ROOT/target/$TARGET/release/yinshu" "$STAGE/deb/usr/bin/yinshu"
install -m 0644 "$ROOT/apps/server/packaging/systemd/yinshu.service" "$STAGE/deb/lib/systemd/system/yinshu.service"
sed -e "s/\${VERSION}/$PACKAGE_VERSION/" -e "s/\${ARCH}/$ARCH/" "$ROOT/apps/server/packaging/deb/control" > "$STAGE/deb/DEBIAN/control"
for script in postinst prerm postrm; do
  install -m 0755 "$ROOT/apps/server/packaging/deb/$script" "$STAGE/deb/DEBIAN/$script"
done
dpkg-deb --build "$STAGE/deb" "$OUT/yinshu-server_${PACKAGE_VERSION}_${ARCH}.deb"

if command -v rpmbuild >/dev/null 2>&1; then
  RPMROOT="$STAGE/rpmbuild"
  mkdir -p "$RPMROOT/BUILD" "$RPMROOT/BUILDROOT" "$RPMROOT/RPMS" "$RPMROOT/SOURCES" "$RPMROOT/SPECS" "$RPMROOT/SRPMS"
  install -m 0755 "$ROOT/target/$TARGET/release/yinshu" "$RPMROOT/SOURCES/yinshu"
  install -m 0644 "$ROOT/apps/server/packaging/systemd/yinshu.service" "$RPMROOT/SOURCES/yinshu.service"
  sed "s/__VERSION__/$PACKAGE_VERSION/" "$ROOT/apps/server/packaging/rpm/yinshu.spec" > "$RPMROOT/SPECS/yinshu.spec"
  rpmbuild --define "_topdir $RPMROOT" -bb "$RPMROOT/SPECS/yinshu.spec"
  find "$RPMROOT/RPMS" -name '*.rpm' -exec cp {} "$OUT/" \;
fi
