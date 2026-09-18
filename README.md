# Yinshu

Local print agent. A trusted page or phone sends the job; the OS print queue puts it on paper. No system print dialog.

**English** · [中文](./README.zh-CN.md)

[Demo](https://github.com/wu9007/yinshu-demo) · [Releases](https://github.com/wu9007/yinshu/releases) · [Protocol](docs/technical_en.md)

<p align="center">
  <img src="screenshots/settings.png" width="360" alt="Settings" />
</p>

## Start

Install from [Releases](https://github.com/wu9007/yinshu/releases). Tray → settings: printer, paper, website list. Test print once.

```bash
pnpm --dir apps/desktop tauri dev
```

## Connect

`ws://127.0.0.1:17890/ws` · Origin must be on the website list.

```json
{
  "type": "print",
  "request_id": "req-1",
  "job_id": "job-1",
  "format": "pdf",
  "file_url": "https://example.com/label.pdf"
}
```

`queued` = accepted here. `submitted` = handed to the OS. Not “paper is out”.

## Supported operating systems

Installers are only on [Releases](https://github.com/wu9007/yinshu/releases). The pipeline currently publishes Windows x64 NSIS (`.exe`).

| OS | Versions | Arch | Packages |
| --- | --- | --- | --- |
| Windows | 10, 11 | x64 | NSIS (`.exe`) |
| macOS | 10.15 Catalina and later | Apple Silicon, Intel | `.dmg`. Currently unsigned and not notarized; right-click to open or remove quarantine. Not in this release |
| Linux desktop | webkit2gtk 4.1, e.g. Ubuntu 22.04+, Debian 12+ | x64, ARM64 | deb, rpm, AppImage |
| Linux headless | same | x64, ARM64 | deb, rpm |
| Phone / tablet | iOS and Android system browsers | — | No app. Scan the workstation QR code; an allowlisted page prints. HTTPS pages cannot use `ws://` |

## Backlog

No installers, CI, or machine verification yet. These are not supported:

- Windows 7, Windows 8.1
- Kylin, Ubuntu Kylin, OpenKylin, and other domestic desktops (including webkit2gtk 4.0)
- LoongArch

## Safety

Allow only sites and devices you trust. Never `0.0.0.0/0`. Don’t expose the port.

## License

[Apache-2.0](./LICENSE) · SumatraPDF: [THIRD_PARTY_NOTICES.md](./THIRD_PARTY_NOTICES.md)
