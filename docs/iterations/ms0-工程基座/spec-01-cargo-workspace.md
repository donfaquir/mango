# SPEC-01: 初始化 Cargo Workspace

## 概述

创建 Rust workspace monorepo 结构，作为整个项目的后端骨架。Workspace 包含三个 crate：`core`（共享业务库）、`cli`（命令行工具）、`src-tauri`（桌面应用）。此任务是所有后端任务的基础。

## 技术方案

### 目录/文件结构

```
mango/
├── Cargo.toml                  # workspace 根配置
├── rust-toolchain.toml         # Rust 工具链版本锁定
├── crates/
│   ├── core/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       └── lib.rs
│   └── cli/
│       ├── Cargo.toml
│       └── src/
│           └── main.rs
└── src-tauri/
    ├── Cargo.toml
    ├── tauri.conf.json
    ├── capabilities/
    │   └── default.json
    ├── icons/                  # Tauri 默认图标
    └── src/
        ├── main.rs
        └── lib.rs
```

### 依赖项

**workspace.dependencies（统一版本管理）：**

| crate | 版本 | 用途 |
|-------|------|------|
| tokio | 1 (features: full) | 异步运行时 |
| serde | 1 (features: derive) | 序列化 |
| serde_json | 1 | JSON 处理 |
| rusqlite | 0.39 (features: bundled) | SQLite |
| tokio-rusqlite | 0.6 | 异步 SQLite 包装 |
| uuid | 1 (features: v4, serde) | UUID 生成 |
| thiserror | 2 | 错误类型定义 |
| anyhow | 1 | 应用级错误处理 |
| tracing | 0.1 | 结构化日志 |
| tracing-subscriber | 0.3 (features: env-filter) | 日志输出 |
| chrono | 0.4 (features: serde) | 时间处理 |
| sha2 | 0.10 | 哈希计算 |

**crates/core 额外依赖：**
- `ts-rs = "12"` (features: chrono-impl, uuid-impl)

**crates/cli 额外依赖：**
- `clap = "4"` (features: derive)
- `comfy-table = "7"` — 终端表格输出
- `mango-core = { path = "../core" }`

**src-tauri 额外依赖：**
- `tauri = "2"` (features: devtools)
- `tauri-plugin-shell = "2"` — 外部进程管理
- `tauri-specta = "2.0.0-rc"` (features: typescript, derive)
- `specta = "2.0.0-rc"` (features: typescript)
- `mango-core = { path = "../crates/core" }`

### 核心实现

**根 Cargo.toml：**
```toml
[workspace]
members = ["crates/core", "crates/cli", "src-tauri"]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "Apache-2.0"
authors = ["donfaquir"]

[workspace.dependencies]
# 在此统一声明所有依赖版本
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
rusqlite = { version = "0.39", features = ["bundled"] }
tokio-rusqlite = "0.6"
uuid = { version = "1", features = ["v4", "serde"] }
thiserror = "2"
anyhow = "1"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
chrono = { version = "0.4", features = ["serde"] }
sha2 = "0.10"
```

**rust-toolchain.toml：**
```toml
[toolchain]
channel = "stable"
```

**crates/core/src/lib.rs：**
```rust
pub mod db;

// 后续迭代中逐步添加:
// pub mod provider;
// pub mod task_engine;
// pub mod asset;
// pub mod account;
// pub mod checkpoint;
// pub mod ffmpeg;
```

**crates/cli/src/main.rs：**
```rust
fn main() {
    println!("mango CLI - coming soon");
}
```

**src-tauri/src/main.rs：**
```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    mango_tauri_lib::run();
}
```

**src-tauri/src/lib.rs：**
```rust
pub fn run() {
    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

### 配置

**src-tauri/tauri.conf.json（最小化配置）：**
```json
{
  "$schema": "https://raw.githubusercontent.com/nicerapp/tauri/tauri-v2/packages/cli/schema.json",
  "productName": "Mango",
  "version": "0.1.0",
  "identifier": "com.mango.app",
  "build": {
    "frontendDist": "../dist",
    "devUrl": "http://localhost:5173",
    "beforeDevCommand": "pnpm dev",
    "beforeBuildCommand": "pnpm build"
  },
  "app": {
    "title": "Mango",
    "windows": [
      {
        "label": "main",
        "title": "Mango",
        "width": 1280,
        "height": 800,
        "minWidth": 960,
        "minHeight": 600
      }
    ],
    "security": {
      "csp": null
    }
  },
  "plugins": {}
}
```

**src-tauri/capabilities/default.json：**
```json
{
  "identifier": "default",
  "description": "Default capabilities",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "shell:allow-open"
  ]
}
```

## 接口定义

此任务不暴露业务接口，仅建立项目结构。对外的 crate 公开模块：

- `mango-core`：`pub mod db`（后续添加更多模块）
- `mango-cli`：binary，无 lib 接口
- `mango-tauri` (src-tauri)：`pub fn run()`

## 验收标准

```bash
# 1. workspace 编译成功
cargo build

# 2. 所有 crate 可单独编译
cargo build -p mango-core
cargo build -p mango-cli
cargo build -p mango-tauri

# 3. 测试运行（无测试用例时 pass）
cargo test

# 4. CLI 可执行
cargo run -p mango-cli

# 5. 检查依赖树无冲突
cargo tree --duplicates
```

## 依赖关系

- **前置**：无
- **后续**：
  - SPEC-02（前端工程）依赖 src-tauri 目录存在
  - SPEC-03（SQLite schema）依赖 core crate 结构
  - SPEC-06（CLI 骨架）依赖 cli crate 结构
