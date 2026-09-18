# 印枢

本机打印代理。可信页面或手机把任务发过来，系统打印队列出纸。不弹系统打印框。

[English](./README.md) · **中文**

[Demo](https://github.com/wu9007/yinshu-demo) · [安装包](https://github.com/wu9007/yinshu/releases) · [协议](docs/technical.md)

<p align="center">
  <img src="screenshots/settings.png" width="360" alt="设置" />
</p>

## 上手

从 [Releases](https://github.com/wu9007/yinshu/releases) 安装。托盘 → 设置：打印机、纸张、网站名单。先打一张测试。

```bash
pnpm --dir apps/desktop tauri dev
```

## 连接

`ws://127.0.0.1:17890/ws` · 握手 Origin 必须已在网站名单里。

```json
{
  "type": "print",
  "request_id": "req-1",
  "job_id": "job-1",
  "format": "pdf",
  "file_url": "https://example.com/label.pdf"
}
```

`queued` = 这里收下了。`submitted` = 交给系统了。不是“纸已经出来”。

## 适配的操作系统

安装包只从 [Releases](https://github.com/wu9007/yinshu/releases) 下载。当前只发布 Windows x64 的 NSIS（`.exe`）。

| 系统 | 版本 | 架构 | 安装包 |
| --- | --- | --- | --- |
| Windows | 10、11 | x64 | NSIS（`.exe`） |
| macOS | 10.15 Catalina 及以上 | Apple Silicon、Intel | `.dmg`。目前未签名、未公证，需右键打开或去掉隔离属性。本版本未发布 |
| Linux 桌面 | webkit2gtk 4.1，例如 Ubuntu 22.04+、Debian 12+ | x64、ARM64 | deb、rpm、AppImage |
| Linux 无界面 | 同上 | x64、ARM64 | deb、rpm |
| 手机 / 平板 | iOS、Android 系统浏览器 | — | 无安装包。扫工位二维码，由已放行的网页打印。HTTPS 页面连不上 `ws://` |

## 待办

还没有安装包、CI 或真机验证，暂不支持：

- Windows 7、Windows 8.1
- 银河麒麟、优麒麟、OpenKylin 及其他国产桌面（含仍是 webkit2gtk 4.0 的版本）
- 龙芯 LoongArch

## 安全

只加你信任的网站和设备。不要写 `0.0.0.0/0`。端口不要暴露到不可信网络。

## 许可

[Apache-2.0](./LICENSE) · SumatraPDF：[THIRD_PARTY_NOTICES.md](./THIRD_PARTY_NOTICES.md)
