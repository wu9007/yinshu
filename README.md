# Yinshu

Local print agent. A trusted page or phone sends the job; the OS print queue puts it on paper. No system print dialog.

**English** · [中文](./README.zh-CN.md)

[Demo](https://github.com/wu9007/yinshu-demo) · [Releases](https://github.com/wu9007/yinshu/releases) · [Protocol](docs/technical_en.md)

<p align="center">
  <img src="screenshots/settings.png" width="360" alt="Settings" />
</p>

## Use

1. Install from [Releases](https://github.com/wu9007/yinshu/releases). Tray → printer, paper, **Test print**.
2. Add this page’s Origin. Match **scheme + host + port** exactly.

   `http://localhost:5173` · `http://127.0.0.1:5173` · `https://erp.example.com`
3. Connect `ws://127.0.0.1:17890/ws`. Phones: scan the QR, use that `ws://host:17890/ws`.

An HTTPS page cannot open `ws://`. Use `http://localhost` while developing.

### Print

```json
{
  "type": "print",
  "request_id": "req-1",
  "job_id": "job-1",
  "format": "pdf",
  "file_url": "https://example.com/label.pdf"
}
```

Omit `printer_name` / `paper` to use the tray defaults.

| format | Send | Notes |
| --- | --- | --- |
| `pdf` `image` `docx` `xlsx` `pptx` | `file_url` | Office needs a local Office / WPS / LibreOffice |
| `html` | `file_url` | Public http(s) only. Needs Chrome / Edge |
| `raw-html` | `html` | Same renderer. No `file_url` |
| `raw` | `data_base64` | Bytes as-is (ZPL / TSPL / ESC/POS). No `file_url` / `paper` / `copies` |

List printers: `{ "type": "get_printers_list", "request_id": "req-1" }`.

Blood-label ZPL: [yinshu-demo](https://github.com/wu9007/yinshu-demo) (`packages/print-zpl`).

### Status

`queued` → accepted here. `submitted` → handed to the OS. Not “paper is out”. `failed` arrives as `job_status`.

### Stuck

| You see | Check |
| --- | --- |
| Cannot connect | Yinshu running? Origin exact? HTTPS page + `ws://` will fail |
| Handshake closed | Website list missed this Origin (port included) |
| Raw prints as hex / text | Printer language is ZPL (or TSPL). Job must be `format: "raw"` |
| Office / HTML fails | Converter or Chrome/Edge installed? File URL public? |

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

## Develop

```bash
pnpm --dir apps/desktop tauri dev
```

## License

[Apache-2.0](./LICENSE) · SumatraPDF: [THIRD_PARTY_NOTICES.md](./THIRD_PARTY_NOTICES.md)
