# 第九阶段：网络边界与安全检查记录

## 网络边界

已完成的静态证据：

- Rust `Cargo.toml` 不包含通用 HTTP、WebSocket 或代理客户端依赖。
- `src-tauri/src/security/network_boundary.rs` 递归检查 Rust 业务源码，拒绝 `TcpStream`、`WebSocket`、HTTP/HTTPS URL 等直接网络调用。
- Tauri CSP 的 `connect-src` 仅允许本地 IPC；远程脚本、字体、图片和 frame 均被禁止。
- 更新器边界状态固定为 `Disabled`，不接受业务元数据。
- 模型安装从用户选定的本地目录读取，`networkRequired` 固定为 `false`。

这些证据证明了代码边界，但不替代 9.3 要求的 Windows 网络审计。该审计需要在桌面运行时配置本地代理或 Windows 防火墙日志，记录导入、分析、搜索、提醒和备份期间无携带通知数据的出站连接。

## 安全检查

代码和既有测试覆盖：DPAPI 用户密钥隔离、SQLCipher 数据库、AES-256-GCM 附件、备份认证加密、篡改拒绝、恢复前隔离验证、事务回滚和安全日志脱敏。Rust 测试尚未能在当前主机执行，因为 `openssl-sys` 构建需要 Perl；因此 9.5 保持“环境阻塞”，不会把静态检查当作高风险用例通过。

## 复验命令

```text
pnpm format:check
pnpm lint
pnpm test
pnpm build
pnpm format:rust
pnpm lint:rust
pnpm test:rust
```

恢复 Rust 构建环境后，应重新运行后三项，并将命令输出、系统版本、测试数量和失败样本追加到本文件。
