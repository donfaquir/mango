# MS1 新增依赖一览

本文档汇总 MS1 五个 spec 中提议引入的所有新依赖，便于一次性评审与批准。**版本号在实施 PR 时通过 `WebSearch` 核对当时最新 stable 后再 pin**（项目规则 `general.md` §"Dependency & Version Constraints"）。

## Rust 依赖（workspace 根 `Cargo.toml`）

| crate | 用途 | 引入 spec | 备注 |
|---|---|---|---|
| `keyring` | macOS Keychain / Windows Credential Manager / Linux Secret Service 访问 | spec-13 | 已在 `docs/技术选型.md` §2.1 列入；走 `KeyringStore` trait 抽象便于单测 |
| `rpassword` | CLI 安全读取 stdin 不回显（`mango account add` 无 `--key` 时） | spec-13 | 仅 crates/cli 用 |
| `image` | 缩略图生成（jpeg/png/webp 编解码） | spec-12 | `default-features = false, features = ["jpeg", "png", "webp"]` |
| `slug` | 项目名生成默认目录建议（仅 GUI suggest_project_root 展示） | spec-10 | 仅 src-tauri 用；最终存储路径以 uuid 为底 |

`sha2`、`uuid`、`tempfile`、`tracing`、`thiserror`、`anyhow`、`tokio` 等 MS0 已引入，无需重复。

## Tauri 插件（`src-tauri/Cargo.toml`）

| 插件 | 用途 | 引入 spec |
|---|---|---|
| `tauri-plugin-dialog` | 原生目录选择器（创建项目时选择 root_path） | spec-10 |

注册：`src-tauri/src/lib.rs` `.plugin(tauri_plugin_dialog::init())`，并在 `src-tauri/capabilities/default.json` 加入对应 permission。

## npm 依赖（`package.json`）

| 包 | 用途 | 引入 spec |
|---|---|---|
| `react-hook-form` | 主体详情页 / 账号对话框表单状态管理 | spec-14 |
| `@hookform/resolvers` | 把 zod schema 接入 react-hook-form | spec-14 |
| `zod` | 表单输入校验 schema | spec-14 |
| `sonner` | toast 通知（导入失败、校验成功等反馈） | spec-12 / spec-14 |

## shadcn/ui 组件（`pnpm dlx shadcn@latest add ...`）

| 组件 | 用途 | 引入 spec |
|---|---|---|
| `tabs` | 主体库 4 类切换 | spec-14 |
| `select` | 服装表单选 character；账号对话框选 provider | spec-14 |
| `alert` | 错误态展示 | spec-14 |
| `skeleton` | 加载态占位 | spec-14 |
| `form` | react-hook-form + shadcn 字段壳 | spec-14 |
| `sonner` | toast 渲染容器（与 npm `sonner` 配合） | spec-14 |

## 文件系统/平台权限影响

- macOS：keyring 写入触发首次"允许访问钥匙串"弹窗
- Linux：需要 secret-service 守护进程（gnome-keyring / kwallet）；缺失时 `create_api_account` 直接报错（MS1 不降级）
- Windows：使用 Credential Manager，无额外配置

## 版本验证清单（实施 PR 时执行）

实施前，每项依赖按以下流程定版：
1. `WebSearch "<crate-name> latest version crates.io"` 拿到当前 stable
2. 与 `docs/技术选型.md` 既定版本比对（若已列入）
3. 与 workspace 中已存在的间接依赖兼容性比对
4. 在 spec 实施 PR 描述中记录所选版本与依据

不依赖训练数据；不接受"我记得是 X 版本"的笼统说法。
