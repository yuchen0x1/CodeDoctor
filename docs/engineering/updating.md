# 安装与更新

从 [GitHub Releases](https://github.com/yuchen0x1/CodeDoctor/releases/latest) 选择当前系统和架构的附件。下载后与同一版本 `SHA256SUMS.txt` 核对。发布流水线为资产生成 GitHub 来源证明，也可以使用 `gh attestation verify <文件> --repo yuchen0x1/CodeDoctor` 验证。

## Windows

安装版支持 NSIS EXE 和 MSI。应用需要 Microsoft Edge WebView2 Runtime。`v0.1.x` 用户通过同类型安装器覆盖升级至 `v0.2.x`。免安装版则退出旧程序并替换 `codedoctor.exe`。

首次使用内置更新时，从“设置 → 软件更新”检查，或按日、周开启启动检查。检查只访问正式 GitHub Release 及其下载域名。关闭全屏弹窗后，同一版本的自动提示会延后，仍可以主动检查。应用显示版本、发布日期、更新日志和包大小。安装版按原安装器类型下载。免安装版显示发布页入口。

安装前拒绝非官方路径、超出声明大小、SHA-256 不符、签名不匹配或签名版本不一致的包。正式签名保留了签名版本，检查只选择高于当前版本的正式发行。完成或取消扫描后，应用使用 SQLite 备份 API 保存历史数据，随后启动系统安装器。签名验证不使用仓库写入令牌。

Windows Authenticode 需要独立的受信任代码签名证书。Tauri 更新包签名、GitHub 来源证明和 Authenticode 分别验证不同的信任链。Windows 系统安装提示以本机系统策略为准。

### 恢复

网络、校验或备份失败时，重试前可继续使用当前版本。安装器启动后，由 Windows 安装机制处理失败。若新版本无法启动，退出 CodeDoctor，保留 `%APPDATA%\com.codedoctor.desktop` 的副本，使用先前版本的同类官方安装器手动恢复。更新前数据库备份在该目录下的 `update-backups`。仅在确认要恢复哪一份快照后，将备份复制到独立目录检查，避免把不兼容的数据库覆盖到现有数据。

## Linux CLI

将 `CodeDoctor_<版本>_linux_x64-cli.tar.gz` 解压至用户目录，例如 `~/.local/bin` 所在的暂存目录。核对校验清单与来源证明，执行 `codedoctor-cli --version`，再替换先前的可执行文件。CLI 每次扫描使用内存数据库，输出重定向到调用方选择的位置。保留上一个已验证的可执行文件即可回退。

## 签名维护

`release-signing` 环境只接受版本标签。只读构建 job 产生 NSIS 和 MSI。签名 job 仅在受保护的正式标签和构建成功后取得 `TAURI_SIGNING_PRIVATE_KEY`、`TAURI_SIGNING_PRIVATE_KEY_PASSWORD`。签名后独立校验公钥、内容和签名版本，再生成 `latest.json`。发布 job 校验所有远端摘要并生成来源证明。

加密私钥备份与当前 Windows 用户的 DPAPI 密码备份存储在本机签名目录。将加密密钥和密码分别通过受控的备份渠道保存，权限仅限发行维护者。密钥轮换需要已发布客户端可以信任的过渡版本。
