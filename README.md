# 印枢

本机打印代理。网站和局域网设备把任务交给工位上的印枢，操作系统打印队列负责出纸。

面向仓库、门店、国产工位和信创环境。不宣称任何信创认证。

[English](./README_en.md)

<p align="center">
  <img src="screenshots/settings.png" width="280" alt="设置" />
  <img src="screenshots/devices.png" width="280" alt="设备名单" />
  <img src="screenshots/qr.png" width="280" alt="连接二维码" />
</p>

## 适配的操作系统

安装包只从 [Releases](https://github.com/wu9007/yinshu/releases) 下载。

| 系统 | 版本 | 架构 | 安装包 |
| --- | --- | --- | --- |
| Windows | **7**、8.1、10、11 | x64 | NSIS（`.exe`）、MSI。Windows 7 需 WebView2，建议用 NSIS |
| macOS | 10.15 Catalina 及以上 | Apple Silicon、Intel | `.dmg` |
| Linux 桌面 | webkit2gtk 4.1，例如 Ubuntu 22.04+、Debian 12+ | x64、ARM64 | deb、rpm、AppImage |
| Linux 无界面 | 同上 | x64、ARM64 | deb、rpm |

不提供 32 位 Windows，也不提供 Windows ARM 安装包。

## 做什么

- 托盘常驻。打印机、纸张、网站名单、设备名单在同一屏。
- 网站按 Origin 放行，设备按 IP / 网段放行。不默认放行全网。
- 手机扫二维码拿到 `ws://地址:端口/ws`。本机浏览器不用扫，始终能连。
- PDF、图片、Office、HTML，以及 ESC/POS / TSPL / ZPL。

装好后从托盘打开设置：选打印机和纸张，加网站，需要时再放行设备。点「测试打印」确认能出纸。

## 开发

```bash
pnpm --dir apps/desktop tauri dev
```

无界面 Linux：

```bash
yinshu printer set-default "Printer Name"
yinshu paper set 60 40
yinshu origin add "https://example.com"
yinshu ip add "192.168.1.0/24"
```

## 网站怎么连

浏览器连 `ws://127.0.0.1:17890/ws`。局域网设备换成这台电脑的地址。握手 Origin 必须已在网站名单里。

```json
{
  "type": "print",
  "request_id": "req-1",
  "job_id": "job-1",
  "format": "pdf",
  "file_url": "https://example.com/label.pdf"
}
```

`queued` 表示进了本机队列。`submitted` 表示已交给系统打印队列，不表示纸已经出来。协议见 [技术说明](docs/technical.md)。

## License

[Apache License 2.0](./LICENSE)。Windows 随包的 SumatraPDF 见 [THIRD_PARTY_NOTICES.md](./THIRD_PARTY_NOTICES.md)。
