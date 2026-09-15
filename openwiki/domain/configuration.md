# 配置

YinShu 将所有设置存储在单个 JSON 配置文件中，由 GUI、CLI、headless `serve` 和配置导入/导出共享。同一个文件是唯一的真相来源——CLI 命令直接修改它，GUI 通过 Tauri 命令读写它。

## 配置结构

根 `AgentConfig`（`config.rs`）包含五个配置段：

### `service`

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `host` | String | `"127.0.0.1"` | 仅兼容字段——不控制绑定。始终归一化为 `127.0.0.1`。 |
| `port` | u16 | `17890` | 实际监听端口。范围：10000–65535。 |

### `security`

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `allowed_origins` | Vec\<String\> | `[]` | WebSocket 连接的网站 Origin 白名单 |
| `allowed_ips` | Vec\<String\> | `["127.0.0.1"]` | IP/CIDR 白名单。`127.0.0.1` 始终存在且不可移除。 |

### `printing`

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `default_printer` | Option\<String\> | `null` | 默认打印机名称 |
| `default_paper` | Option\<EffectivePaper\> | `null` | 默认纸张（width_mm + height_mm） |
| `default_copies` | u16 | `1` | 默认打印份数 |

### `limits`

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `max_file_size_mb` | u32 | `20` | 每次下载的最大文件大小 |
| `max_batch_jobs` | u32 | `20` | 每批次最大作业数 |
| `max_copies` | u16 | `100` | 每个作业最大份数 |
| `download_timeout_seconds` | u64 | `30` | 下载超时时间 |

### `app`

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `autostart` | bool | `false` | 系统启动时自动运行 |
| `language` | UiLanguage | `zh-CN` | 界面语言（`zh-CN` 或 `en`） |

## 配置示例

```json
{
  "service": { "host": "127.0.0.1", "port": 17890 },
  "security": {
    "allowed_origins": ["https://example.com"],
    "allowed_ips": ["127.0.0.1", "192.168.1.0/24"]
  },
  "printing": {
    "default_printer": "TSC TE244",
    "default_paper": { "width_mm": 60, "height_mm": 40 },
    "default_copies": 1
  },
  "limits": {
    "max_file_size_mb": 20,
    "max_batch_jobs": 20,
    "max_copies": 100,
    "download_timeout_seconds": 30
  },
  "app": { "autostart": false, "language": "zh-CN" }
}
```

## 持久化与加载

- `AgentConfig::load(path)`——读取 JSON；文件不存在时返回 `Default`；始终调用 `normalized()`（强制 `host=127.0.0.1`，归一化 IP 白名单）
- `AgentConfig::save(path)`——美化输出 JSON，创建父目录
- GUI 和 CLI 使用同一个 `config.json` 文件

## 数据目录

| 平台 | 默认路径 |
|------|---------|
| Windows | `%APPDATA%\cn.yinshu.app` |
| macOS | `~/Library/Application Support/cn.yinshu.app` |
| Linux | `${XDG_CONFIG_HOME:-~/.config}/cn.yinshu.app` |

数据目录中的文件：

| 文件 | 用途 |
|------|------|
| `config.json` | Agent 配置 |
| `task_history.sqlite3` | 任务历史 + 事件日志 |

## 环境变量

| 变量 | 效果 |
|------|------|
| `YINSHU_DATA_DIR` | 覆盖整个数据目录（配置 + SQLite 数据库） |
| `YINSHU_CONFIG_PATH` | 仅覆盖配置文件路径；SQLite 数据库仍存放在数据目录 |
| `YINSHU_RUNTIME_DIR` | 无界面服务覆盖运行目录（默认 `/run/yinshu`） |

如果未设置 `YINSHU_CONFIG_PATH`，配置默认为 `{data_dir}/config.json`。

## 校验规则

配置系统强制执行以下约束：

- 端口范围：10,000–65,535
- IP 条目：有效 IPv4/IPv6 或 CIDR；拒绝 `0.0.0.0`、`::`、`/0` 前缀
- Origin：必须是带 http/https scheme 的有效 URL
- `127.0.0.1` 始终注入到 `allowed_ips`

## 配置导入/导出

加密配置传输格式和跨语言生成示例见 [安全模型](security.md)。设置界面支持：
- **导出：** 选择要包含的配置段，设置密码（可选），生成加密的 `yinshu-config.json`
- **导入：** 选择文件 + 密码，预览 diff，确认合并

## 源码参考

| 领域 | 文件 |
|------|------|
| 配置结构体 + 默认值 + 加载/保存 | `crates/core/src/config.rs` |
| 配置加密/解密 | `crates/cli/src/config_transfer.rs` |
| 数据目录解析 + 环境变量 | `crates/core/src/config.rs` |
| 系统路径（headless） | `apps/server/src/paths.rs` |
| 前端配置类型 | `apps/desktop/src/types.ts` |
| 前端 API 调用 | `apps/desktop/src/api.ts` |
