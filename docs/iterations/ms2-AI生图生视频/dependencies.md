# MS2 新增依赖一览

本文档汇总 MS2 五个 spec（15/16/17/18/19）中提议引入的所有新依赖，便于一次性评审与批准。**版本号在实施 PR 时通过 `WebSearch` / `WebFetch crates.io` / `WebFetch npmjs.com` 核对当时最新 stable 后再 pin**（项目规则 `general.md` §"Dependency & Version Constraints"）。

> 当前会话尝试通过 `WebFetch crates.io` 核对版本时网络受限，因此本表所有 Rust 版本号均以 `?` 占位。实施 PR **必须** 完成 §5 的版本验证清单后再写入 Cargo.toml。

---

## 1. Rust 依赖（workspace 根 `Cargo.toml`）

| crate | 用途 | 引入 spec | 备注 |
|---|---|---|---|
| `reqwest` | DashScope HTTP 调用 + OSS REST 调用 + 远端结果文件下载 | spec-17, spec-16, spec-17 (`asset::from_remote`) | 用 `default-features = false, features = ["rustls-tls", "json", "stream"]`；不要带 native-tls 避免 macOS Keychain 二次依赖 |
| `async-trait` | `ModelProvider` / `AssetUploader` / `ResultMaterializer` 三个 trait 都要 `async fn in trait` | spec-15, spec-16, spec-17 | Rust 2024 edition 已支持原生 async fn in trait，但加 `Send` bound 仍最稳走 async-trait。最终选型在 spec-15 实施时确认 |
| `aliyun-oss-rust-sdk` 或 `oss-rust-sdk` 或自写 | 阿里 OSS V4 签名上传 + 签名 URL 生成 | spec-16 | **三选一，spec-16 §SDK 选型决策」：实施初期跑 PoC 后定。如果都缺关键能力则自写 reqwest + V4 签名。自写时另需 `hmac` + `sha2`(已有) + `percent-encoding` |
| `mime_guess` | spec-17 `asset::from_remote` 根据 Content-Type / URL 推扩展名 | spec-17 | 备选：用 `infer` 从下载的字节头猜，更可靠但多一次缓冲 |
| `tokio-rusqlite` | 已在 workspace；spec-15 task_engine 直接用 | spec-15 | 已有，不算新增 |
| `humantime` | CLI `task list --since 1d/12h/30m` 解析 | spec-19 | 仅 crates/cli 使用 |
| `comfy-table` | CLI `task list` 表格输出 | spec-19 | MS1 已加（account/character 列表用）；不算新增 |
| `assert_cmd` (dev) | CLI 子命令 e2e 测试 | spec-19 §7.1 | dev-dependencies；用于 `tests/cli_task_*.rs` |
| `predicates` (dev) | 配合 `assert_cmd` 做 stdout/stderr 断言 | spec-19 §7.1 | dev-dependencies |

### 1.1 spec-15 task_engine 内部仅用现有依赖

spec-15 的任务引擎完全基于已有依赖（tokio / tokio-rusqlite / serde / serde_json / thiserror / tracing / chrono / uuid），不引入新 crate。`async-trait` 是从 trait 设计角度才出现在表里。

### 1.2 OSS SDK 选型说明（spec-16）

候选：

| 候选 | 优点 | 风险 |
|---|---|---|
| `aliyun-oss-rust-sdk` | 名称符合官方风格，文档相对系统 | 维护活跃度需 PoC 验证；V4 签名支持需确认 |
| `oss-rust-sdk` | crates.io 上有较长历史 | 同上；可能不支持 V4 / STS |
| 自写 reqwest + V4 签名 | 完全可控，零未审依赖 | 工作量约 200-300 LOC；风险点是签名串编码（query 参数排序、percent-encoding） |

**实施 PR 决策流程**：先尝试候选 1 → 跑 PoC（上传一个 1MB 文件 + 生成 1h 签名 URL + 用浏览器访问通过）→ 通过则 pin 该版本；失败则尝试候选 2；都失败转候选 3（自写）。

PoC 评估要点：
- V4 签名（`x-oss-signature-version: V4` 或 `OSS4-HMAC-SHA256`）必须支持
- 临时签名 URL 生成（`presign_get` 或等效）必须支持
- async/await 原生支持（不能阻塞 tokio runtime）

---

## 2. Tauri 插件 / src-tauri 增量

无新 Tauri 插件。spec-18 仅复用现有 `tauri-plugin-dialog`（如果结果文件「在 Finder 显示」需要 `tauri-plugin-opener`，spec-18 未列入；MS3 再考虑）。

`src-tauri/Cargo.toml` 只需把 spec-15 / spec-17 引入的 core crate 新模块顺路 re-export，不直接添加新 Rust 依赖（task_engine 完全在 core 内运转）。

---

## 3. npm 依赖（`package.json`）

**MS2 spec-18 不新增 npm 包**。

MS1 已 pin 的可复用包：
- `react-hook-form` — spec-18 表单状态
- `@hookform/resolvers` — zod 适配器
- `zod` — 表单 schema 校验
- `sonner` — toast 通知
- `@tanstack/react-query` — `useTaskList` / `useSubmitTask` 等
- `@tauri-apps/api` — `events.listen` / `convertFileSrc`
- `lucide-react` — 任务状态图标 (`Loader2` / `Wand2` etc)

---

## 4. shadcn/ui 组件（`pnpm dlx shadcn@latest add ...`）

| 组件 | 用途 | 引入 spec |
|---|---|---|
| `progress` | `TaskProgress` 进度条（任务卡片 running 时） | spec-18 §4.2 |
| `badge` | `TaskStatusBadge` 可选用，不用则手写 `<span>` | spec-18 §4.3 (可选) |
| `tabs` | `TaskListPanel` 全部/进行中/失败过滤 | spec-18 §4.1 — MS1 已加，无需重复 |
| `dialog` | 主体选择器、结果预览 | spec-18 — MS1 已加，无需重复 |
| `select` | 模型选择、模型私有参数下拉 | spec-18 — MS1 已加，无需重复 |
| `form` | spec-18 各模型参数表单 | spec-18 — MS1 已加，无需重复 |

实施时 `pnpm dlx shadcn@latest add progress badge`（仅这两个新增）。

---

## 5. 文件系统/平台权限影响

- spec-16 OSS 上传：网络流量出方向（HTTPS）。macOS 应用首次访问网络可能触发系统提示（防火墙），无需 entitlements 调整
- spec-17 视频结果下载：可能下载几 MB - 几十 MB 文件到项目 `assets/` 目录。`asset::from_remote` 不写入项目目录之外的位置
- keyring：spec-16 在「主 API key」之外**多写一条** `api_account:{id}:oss_secret`。macOS 钥匙串 / Linux secret-service / Windows Credential Manager 三处都支持多 entry，无 schema 变更
- 网络故障：spec-17 错误映射要明确区分 401(认证) / 403(余额) / 429(限流) / 5xx(远端) / 本地超时；spec-19 CLI 不带美化只透传

---

## 6. 数据库 migration

| migration | 内容 | 引入 spec |
|---|---|---|
| `004_api_account_params_json.sql` | `ALTER TABLE api_account ADD COLUMN params_json TEXT NOT NULL DEFAULT '{}';` | spec-16 |
| Provider seed 升级 | DELETE kling 行（CASCADE 清掉旧账号）；INSERT bailian 行 | spec-16 |
| `005_task_engine_lock.sql`（可选） | task_engine 心跳锁表，给 CLI 检测 GUI 是否在跑用。spec-19 §3.4 已说明可舍弃此精度 | spec-19 (可选) |

> 005 是 spec-19 的优化项，默认 **不做**。如果实施时确认需要再补 migration。

---

## 7. 版本验证清单（实施 PR 时执行）

实施前，每项依赖按以下流程定版：

1. `WebSearch "<crate-name> latest version crates.io"` 或 `WebFetch https://crates.io/crates/<name>` 拿到当前 stable
2. 与 `docs/技术选型.md` 既定版本比对（若已列入）
3. 与 workspace 中已存在的间接依赖兼容性比对（特别是 reqwest 与 tokio 的 feature 组合）
4. OSS SDK：必须做 PoC（§1.2 评估要点）后再 pin
5. 在 spec 实施 PR 描述中记录所选版本与依据

不依赖训练数据；不接受「我记得是 X 版本」的笼统说法。

## 8. 风险与未决项

- **OSS SDK 选择**（§1.2）：spec-16 实施第一周内必须出 PoC 决策
- **`async-trait` vs 原生 async fn in trait**：Rust 2024 edition 已稳定 async fn in trait，但 `Send` bound 仍需手动声明。spec-15 实施时确认，二选一，统一全 trait
- **DashScope 同步模型连接复用**（开发任务.md §开放问题 #1 已删除但实测项保留）：wan2.7-image-pro 同步 HTTP 请求长达数十秒，需要在 spec-17 实施初期实测 reqwest pool 是否正确续连。如不行，回退到「单连接 + 自维护 timeout」
- **结果文件 URL 失效**：DashScope 结果 URL 约 24h 失效。spec-17 已规定「download 紧跟 poll 成功」。若任务引擎重启时 task 已成功但结果未下，可能错过窗口。spec-15 启动恢复策略需要在「running 重置为 pending」之外，额外把「success 但未 materialize」也重新调度一次 materialize（不重新调用 provider）
