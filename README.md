# 印枢

本机打印代理。受信任的网站和局域网设备通过 WebSocket 把任务交给工位上的印枢，再由操作系统打印队列出纸。

它不替代打印机驱动，也不绕过系统队列。印枢只做来源校验、下载或转换、排队；真正出纸仍是操作系统和打印机的事。

面向仓库、门店、国产工位和信创环境：标签、面单、小票、拣货单，少弹一次系统打印框。不宣称任何信创认证。

[English](./README_en.md)

## 做什么

- 托盘常驻，点开设置。默认打印机、纸张、网站名单、设备名单都在这一屏。
- 网站按 Origin 放行。设备按 IP / 网段放行。本机回环始终可以连，不出现在设备名单里。
- 局域网设备用二维码拿到 `ws://地址:端口/ws`。不默认放行全网。
- 听在本机所有网卡，协议只有 `/ws`。
- PDF、图片、Office、HTML，以及 ESC/POS / TSPL / ZPL 这类原始指令。
- 任务可指定打印机和纸张；没指定就用设置里的默认值。
- 同一台打印机串行排队。CUPS 上已离线的打印机不会再假提交。
- 最近任务、配置加密导入导出、CLI 运维。Windows / macOS / Linux，Linux 另有无界面版本。

## 怎么跑

开发：

```bash
pnpm --dir apps/desktop tauri dev
```

装好后从托盘打开设置：选打印机和纸张，加网站，需要的话再放行设备和本网。点「测试打印」确认能出纸。

发布包见 [Releases](https://github.com/wu9007/yinshu/releases)。

无界面 Linux 用同一个命令行配打印机、纸张和名单：

```bash
yinshu printer set-default "Printer Name"
yinshu paper set 60 40
yinshu origin add "https://example.com"
yinshu ip add "192.168.1.0/24"
```

## 网站怎么连

浏览器页面连 `ws://127.0.0.1:17890/ws`（局域网设备换成这台电脑的地址）。握手带页面 Origin，必须已在网站名单里。

```json
{
  "type": "print",
  "request_id": "req-1",
  "job_id": "job-1",
  "format": "pdf",
  "file_url": "https://example.com/label.pdf"
}
```

`queued` 只表示进了本机队列。`submitted` 表示已经交给系统打印队列，不表示纸已经出来。状态走同一条 WebSocket。

完整协议、格式和限制见 [技术说明](docs/technical.md)。

## 安全

- 只加可信网站的 Origin。
- 只加可信设备的 IP 或网段。不要写 `0.0.0.0/0`。
- 服务可以听局域网，但不要把端口暴露到不可信网络。
- 打印用的文件 URL 不要给不可信页面。页面里的 HTML 资源也不能打本机或私网地址。

## 仓库

| 目录 | 做什么 |
| --- | --- |
| `crates/core` | 配置、协议、校验 |
| `crates/runtime` | 队列、打印、WebSocket |
| `crates/cli` | 统一命令 |
| `apps/desktop` | 托盘设置（Vue + Tauri） |
| `apps/server` | Linux 无界面 |

代码名 YinShu，二进制 `yinshu`，包标识 `cn.yinshu.app`。

## License

[Apache License 2.0](./LICENSE)。Windows 随包的 SumatraPDF 见 [THIRD_PARTY_NOTICES.md](./THIRD_PARTY_NOTICES.md)。
