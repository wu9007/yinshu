# YinShu (印枢)

A print agent that stays on the workstation. Trusted websites and LAN devices send jobs over WebSocket; the OS print queue does the actual printing.

It does not replace printer drivers and does not bypass the system queue. YinShu checks the caller, downloads or converts files, and queues work. Paper still comes from the OS, the driver, and the printer.

Built for warehouses, stores, and domestic / Xinchuang workstations that print labels, waybills, receipts, and pick lists without a system print dialog. No certification is claimed.

[中文](./README.md)

## What it does

- Lives in the tray. Printer, paper, website list, and device list are one settings screen.
- Websites are allowed by Origin. Devices are allowed by IP or CIDR. Loopback can always connect and does not show in the device list.
- LAN devices get `ws://host:port/ws` from a QR code. The whole internet is never allowlisted by default.
- Binds on all interfaces. The only protocol path is `/ws`.
- PDF, images, Office, HTML, and raw commands such as ESC/POS, TSPL, and ZPL.
- Each job can pick a printer and paper size; otherwise the settings defaults are used.
- One serial queue per printer. CUPS jobs are not submitted to a printer that is already offline.
- Recent jobs, encrypted config import/export, and a CLI. Windows, macOS, Linux; Linux also has a headless build.

## Run it

Development:

```bash
pnpm --dir apps/desktop tauri dev
```

After install, open settings from the tray: pick a printer and paper, add websites, and allow devices or this LAN if needed. Use test print to confirm paper comes out.

Release builds are on [Releases](https://github.com/wu9007/yinshu/releases).

Headless Linux uses the same CLI:

```bash
yinshu printer set-default "Printer Name"
yinshu paper set 60 40
yinshu origin add "https://example.com"
yinshu ip add "192.168.1.0/24"
```

## Connect from a website

A browser page connects to `ws://127.0.0.1:17890/ws` (LAN devices use this computer’s address). The handshake Origin must be on the website list.

```json
{
  "type": "print",
  "request_id": "req-1",
  "job_id": "job-1",
  "format": "pdf",
  "file_url": "https://example.com/label.pdf"
}
```

`queued` means the local queue accepted the job. `submitted` means it reached the system print queue, not that paper has come out. Status events use the same WebSocket.

Protocol details: [technical notes](docs/technical_en.md).

## Safety

- Allow only trusted website Origins.
- Allow only trusted device IPs or CIDRs. Do not add `0.0.0.0/0`.
- The service may listen on the LAN; do not expose the port to untrusted networks.
- Do not hand sensitive file URLs to untrusted pages. HTML jobs cannot fetch localhost or private-network resources.

## Repo

| Path | Role |
| --- | --- |
| `crates/core` | Config, protocol, validation |
| `crates/runtime` | Queue, printing, WebSocket |
| `crates/cli` | Shared commands |
| `apps/desktop` | Tray settings (Vue + Tauri) |
| `apps/server` | Linux headless |

Product name: 印枢. Code name: YinShu. Binary: `yinshu`. Bundle ID: `cn.yinshu.app`.

## License

[Apache License 2.0](./LICENSE). Windows ships SumatraPDF under its own terms; see [THIRD_PARTY_NOTICES.md](./THIRD_PARTY_NOTICES.md).
