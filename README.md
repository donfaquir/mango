# 🥭 Mango

**本地优先的漫剧创作工作站** —— 从剧本到成片，素材自己管、模型自己选，零依赖第三方 SaaS。

<!-- ![Mango Screenshot](docs/assets/screenshot-placeholder.png) -->

## 核心特性

- **本地优先，数据不上云** —— 所有项目数据存储在本地 SQLite，仅在调用 AI 模型 API 时联网
- **AI 生图 / 生视频** —— 已对接百炼 DashScope（`wan2.7-image-pro` 文生图 + `happyhorse-1.0-r2v` 参考图生视频）
- **主体库管理** —— 角色 / 场景 / 道具 / 服装四类实体，支持参考图绑定，解决角色一致性问题
- **API 账号安全管理** —— 基于系统 Keychain（keyring）加密存储 API Key，不落盘明文
- **CLI + GUI 双模式** —— 桌面应用可视化操作，CLI 适配批量处理与自动化脚本
- **节点画布**（规划中） —— 漫剧业务语义的可视化编排，非通用节点编辑器

## 技术栈

| 层级 | 技术 |
|------|------|
| 后端核心 | Rust, tokio, rusqlite, reqwest, thiserror |
| 桌面框架 | Tauri 2.x（系统 WebView） |
| 前端 | React 19, TypeScript 6, Vite 8, Tailwind CSS 4, shadcn/ui |
| 状态管理 | Zustand 5 + TanStack Query 5 |
| CLI | clap 4, comfy-table |
| 密钥存储 | keyring 4（macOS Keychain / Windows Credential Manager / Linux Secret Service） |
| 类型对齐 | ts-rs 12 + specta 2 |

## 项目结构

```text
mango/
├── crates/
│   ├── core/               # 核心业务库（DB / Provider / 任务引擎 / 素材管理 / 账号）
│   └── cli/                # CLI 二进制（mango 命令行工具）
├── src-tauri/              # Tauri 桌面应用（IPC commands + 窗口管理）
├── src/                    # React 前端
│   ├── components/         # UI 组件（layout / subjects / generation / accounts）
│   ├── hooks/              # React Query hooks
│   ├── pages/              # 路由页面
│   ├── stores/             # Zustand stores
│   └── lib/                # 工具函数 + ts-rs 类型绑定
├── docs/                   # 项目文档（规划 / 迭代 spec / 工作日志）
├── Cargo.toml              # Rust workspace 根配置
├── package.json            # 前端依赖
└── vite.config.ts          # Vite 构建配置
```

## 快速开始

### 环境要求

| 工具 | 版本要求 |
|------|----------|
| Rust | 1.94+（见 `rust-toolchain.toml`） |
| Node.js | 见 `.node-version` |
| pnpm | 11.x |
| 系统依赖 | macOS: Xcode CLT / Linux: webkit2gtk 等 Tauri 依赖 |

### 安装与启动

```bash
# 克隆项目
git clone https://github.com/mango/mango.git
cd mango

# 安装前端依赖
pnpm install

# 开发模式启动（同时编译 Rust 后端 + 启动前端 dev server）
pnpm tauri dev
```

### CLI 构建与使用

```bash
# 构建 CLI
cargo build --release -p mango-cli

# 运行（二进制位于 target/release/mango-cli）
./target/release/mango-cli project list
```

## CLI 命令速查

```bash
# 项目管理
mango project create --name "我的漫剧"
mango project list

# 角色管理
mango character create --project <id> --name "小明" --appearance "黑发少年，校服"
mango character list --project <id>

# API 账号管理
mango account add --provider bailian --label "我的百炼账号"

# AI 生成任务
mango task submit --project <id> --model wan2.7-image-pro --prompt "少年站在樱花树下"
mango task status <task-id>
mango task list --project <id>
mango task cancel <task-id>
```

## 里程碑进度

| 里程碑 | 名称 | 状态 | 说明 |
|--------|------|------|------|
| MS0 | 工程基座 + 项目管理 | ✅ 已完成 | Tauri + React + SQLite + CLI 全链路 |
| MS1 | 主体库 + 账号管理 | ✅ 已完成 | 角色/场景/道具/服装 CRUD + keyring 账号 |
| MS2 | AI 生图 / 生视频 | ✅ 已完成 | 百炼 Provider + 任务引擎 + 结果下载 |
| MS3 | 节点画布 | 📋 规划中 | React Flow 业务语义画布 |
| MS4 | 单 Provider 下多模型 + 批量 + 版本管理 | 📋 规划中 | 百炼多模型 + 并发控制 + 抽卡对比 + Checkpoint |
| MS5 | 视频后期 | 📋 规划中 | FFmpeg 裁剪/拼接 + 字幕/气泡 |
| MS6 | 配音 + 成片闭环 | 📋 规划中 | TTS 配音 + 音效 + 最终合成导出 |

## 开发指南

### 代码检查

```bash
# Rust lint
cargo clippy --workspace --all-targets

# 前端类型检查
pnpm typecheck
```

### 测试

```bash
# Rust 单元测试
cargo test --workspace

# 前端测试
pnpm test:run
```

### 类型生成

Rust 结构体变更后，重新生成 TypeScript 类型绑定：

```bash
cargo test --workspace  # ts-rs 在测试阶段输出 .ts 文件到 src/lib/bindings/
```

### 开发模式

```bash
# 仅前端开发（不启动 Tauri，用于纯 UI 调试）
pnpm dev

# 完整桌面应用开发
pnpm tauri dev
```

## License

[Apache-2.0](LICENSE)
