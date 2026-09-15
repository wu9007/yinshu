Name: yinshu-server
Version: __VERSION__
Release: 1%{?dist}
Summary: 印枢无界面服务
License: Apache-2.0
Requires: systemd, cups-client, libreoffice
Provides: yinshu
Conflicts: yinshu-desktop

%description
Runs the YinShu agent as the dedicated yinshu system user.

%install
install -D -m 0755 %{_sourcedir}/yinshu %{buildroot}%{_bindir}/yinshu
install -D -m 0644 %{_sourcedir}/yinshu.service %{buildroot}/usr/lib/systemd/system/yinshu.service

%pre
getent group yinshu >/dev/null || groupadd -r yinshu
getent passwd yinshu >/dev/null || useradd -r -g yinshu -d /var/lib/yinshu -s /sbin/nologin yinshu

%post
systemctl daemon-reload >/dev/null 2>&1 || :
systemctl enable --now yinshu.service >/dev/null 2>&1 || :

%preun
if [ "$1" -eq 0 ]; then
  systemctl disable --now yinshu.service >/dev/null 2>&1 || :
fi

%postun
systemctl daemon-reload >/dev/null 2>&1 || :
if [ "$1" -ge 1 ]; then
  systemctl try-restart yinshu.service >/dev/null 2>&1 || :
fi

%files
%{_bindir}/yinshu
/usr/lib/systemd/system/yinshu.service

%changelog
