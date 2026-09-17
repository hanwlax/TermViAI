# SSH Keepalive Interval（2026-09-17）

## 使用方式

在左侧 **Settings** 中设置 **SSH keepalive interval**，单位为秒：

- 默认值为 `30` 秒。
- 设置为 `0` 会关闭 keepalive。
- 可设置范围为 `0`–`86400` 秒。
- 点击 **Save** 后持久化；新值从下一次建立 SSH 连接起生效，已经连接的会话保持原间隔直至重连。

该设置与全局终端字号一起保存在 Hosts 数据目录内的 `settings.json`。Windows 默认通常是 `%APPDATA%\wezterm\termviai\settings.json`；设置 `TERMVIAI_DATA_DIR` 后使用该目录。字段名为 `ssh_keepalive_interval_seconds`。

## 实现

TermViAI 创建 SSH Domain 时把间隔写入现有 SSH 配置项 `serveraliveinterval`。底层将非零值转换为定时器，并通过当前默认的 libssh 后端发送 SSH `IGNORE` 包；`0` 不创建定时器。实现复用了已有 SSH 会话循环，没有额外创建 UI 定时线程。

间隔也纳入 TermViAI SSH Domain 的缓存标识。因此修改设置后再次连接会注册带新参数的 Domain，不会复用旧间隔的缓存对象。Host、端口、用户和密钥逻辑保持不变。

当前机制用于在空闲连接上周期性产生 SSH 流量，不实现 `ServerAliveCountMax` 一类的应答计数或主动断线策略。

## 验证

- Windows mux Release 测试：20 项通过。
- Windows TermViAI GUI Release 测试：60 项通过，覆盖默认 30 秒、`0` 关闭、持久化、Settings 双字段和 SSH Domain 参数/缓存标识。
- Windows x64 GUI Release 构建成功。
- `git diff --check` 通过；TermViAI 相关源文件与 Windows 构建快照一致。

## 新版

```text
C:\Users\h00893113\Documents\Codex\termviai-win\releases\ssh-keepalive-20260917\wezterm-gui.exe
```

SHA256：`c58eddf92c70921dccf56877998b0b7e0727c41c21a5e223263d218c32bb89ef`。

真实 SSH 空闲链路仍建议人工验收：分别使用 `30`、自定义短间隔和 `0` 建立新连接，确认中间网络设备的空闲超时行为符合预期。
