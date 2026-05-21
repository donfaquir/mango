# SPEC-06: CLI 骨架

## 概述

实现 `mango` 命令行工具的基础框架，支持 `mango project create` 和 `mango project list` 命令。CLI 直接调用 core crate 函数，使用同步 DB 连接，与 GUI 共享同一数据库。

## 技术方案

### 目录/文件结构

```
crates/cli/
├── Cargo.toml
└── src/
    ├── main.rs             # 入口 + clap 顶层定义
    └── commands/
        ├── mod.rs          # 子命令路由
        └── project.rs      # project 子命令实现
```

### 依赖项

crates/cli/Cargo.toml：
```toml
[package]
name = "mango-cli"
version.workspace = true
edition.workspace = true

[[bin]]
name = "mango"
path = "src/main.rs"

[dependencies]
mango-core = { path = "../core" }
clap = { version = "4", features = ["derive"] }
comfy-table = "7"
anyhow = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
dirs = "6"
```

### 核心实现

#### 入口（main.rs）

```rust
use clap::Parser;
use tracing_subscriber::EnvFilter;

mod commands;

#[derive(Parser)]
#[command(name = "mango", version, about = "Mango - 漫剧创作工作站 CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// 数据库文件路径（默认 ~/.local/share/com.mango.app/mango.db）
    #[arg(long, global = true)]
    db: Option<String>,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// 项目管理
    Project(commands::project::ProjectArgs),
}

fn main() -> anyhow::Result<()> {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();

    // 解析数据库路径
    let db_path = match cli.db {
        Some(path) => std::path::PathBuf::from(path),
        None => default_db_path()?,
    };

    // 确保目录存在
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    // 打开数据库（同步）
    let conn = mango_core::db::open_sync(&db_path)?;

    // 路由子命令
    match cli.command {
        Commands::Project(args) => commands::project::execute(&conn, args)?,
    }

    Ok(())
}

/// 默认数据库路径（与 Tauri app_data_dir 一致）
fn default_db_path() -> anyhow::Result<std::path::PathBuf> {
    let data_dir = dirs::data_dir()
        .ok_or_else(|| anyhow::anyhow!("cannot determine data directory"))?;

    // macOS: ~/Library/Application Support/com.mango.app/
    // Linux: ~/.local/share/com.mango.app/
    // Windows: C:\Users\<user>\AppData\Roaming\com.mango.app\
    let app_dir = data_dir.join("com.mango.app");
    Ok(app_dir.join("mango.db"))
}
```

#### 子命令路由（commands/mod.rs）

```rust
pub mod project;
```

#### Project 子命令（commands/project.rs）

```rust
use clap::{Args, Subcommand};
use comfy_table::{Table, presets::UTF8_FULL_CONDENSED};
use rusqlite::Connection;
use mango_core::models::project::CreateProjectInput;
use mango_core::db::queries::project as project_queries;

#[derive(Args)]
pub struct ProjectArgs {
    #[command(subcommand)]
    action: ProjectAction,
}

#[derive(Subcommand)]
enum ProjectAction {
    /// 创建新项目
    Create {
        /// 项目名称
        #[arg(long)]
        name: String,

        /// 项目描述
        #[arg(long, default_value = "")]
        description: String,

        /// 风格提示词
        #[arg(long, default_value = "")]
        style_prompt: String,
    },
    /// 列出所有项目
    List,
    /// 查看项目详情
    Get {
        /// 项目 ID
        id: String,
    },
    /// 删除项目
    Delete {
        /// 项目 ID
        id: String,

        /// 跳过确认
        #[arg(long)]
        yes: bool,
    },
}

pub fn execute(conn: &Connection, args: ProjectArgs) -> anyhow::Result<()> {
    match args.action {
        ProjectAction::Create { name, description, style_prompt } => {
            let project = project_queries::create(conn, CreateProjectInput {
                name,
                description: Some(description),
                style_prompt: Some(style_prompt),
                global_seed: None,
            })?;

            println!("✓ Project created successfully");
            println!("  ID:   {}", project.id);
            println!("  Name: {}", project.name);
        }

        ProjectAction::List => {
            let projects = project_queries::list(conn)?;

            if projects.is_empty() {
                println!("No projects found. Create one with: mango project create --name \"My Project\"");
                return Ok(());
            }

            let mut table = Table::new();
            table.load_preset(UTF8_FULL_CONDENSED);
            table.set_header(vec!["ID", "Name", "Description", "Created"]);

            for p in &projects {
                table.add_row(vec![
                    &p.id[..8],  // 只显示前 8 位
                    &p.name,
                    &truncate(&p.description, 30),
                    &p.created_at,
                ]);
            }

            println!("{table}");
            println!("\n{} project(s) total", projects.len());
        }

        ProjectAction::Get { id } => {
            let project = project_queries::get_by_id(conn, &id)?;
            println!("ID:          {}", project.id);
            println!("Name:        {}", project.name);
            println!("Description: {}", project.description);
            println!("Style:       {}", project.style_prompt);
            println!("Created:     {}", project.created_at);
            println!("Updated:     {}", project.updated_at);
        }

        ProjectAction::Delete { id, yes } => {
            if !yes {
                eprint!("Delete project {}? [y/N] ", &id[..8.min(id.len())]);
                let mut input = String::new();
                std::io::stdin().read_line(&mut input)?;
                if !input.trim().eq_ignore_ascii_case("y") {
                    println!("Cancelled.");
                    return Ok(());
                }
            }
            project_queries::delete(conn, &id)?;
            println!("✓ Project deleted");
        }
    }

    Ok(())
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() > max {
        format!("{}...", &s[..max - 3])
    } else {
        s.to_string()
    }
}
```

## 接口定义

**CLI 命令结构：**
```
mango [--db <path>] <subcommand>

mango project create --name <NAME> [--description <DESC>] [--style-prompt <PROMPT>]
mango project list
mango project get <ID>
mango project delete <ID> [--yes]
mango --help
mango --version
```

**输出格式：**
- `create`：确认信息 + ID
- `list`：UTF-8 表格
- `get`：键值对
- `delete`：确认提示 + 结果

## 验收标准

```bash
# 1. 编译
cargo build -p mango-cli

# 2. 帮助信息
cargo run -p mango-cli -- --help
cargo run -p mango-cli -- project --help
cargo run -p mango-cli -- project create --help

# 3. 创建项目
cargo run -p mango-cli -- project create --name "测试项目" --description "这是一个测试"
# → 输出 ID 和确认信息

# 4. 列出项目
cargo run -p mango-cli -- project list
# → 表格展示，包含刚创建的项目

# 5. 数据共享验证（使用相同 DB 路径）
# CLI 创建 → pnpm tauri dev 中列表可见（反之亦然）

# 6. 错误处理
cargo run -p mango-cli -- project get nonexistent-id
# → 友好的错误信息，非 panic

# 7. 版本信息
cargo run -p mango-cli -- --version
# → mango 0.1.0
```

## 依赖关系

- **前置**：
  - SPEC-01（cli crate 结构）
  - SPEC-03（DB open_sync）
  - SPEC-04（Project CRUD 函数）
- **后续**：
  - MS1 中扩展 character / account 子命令
  - MS2 中扩展 task 子命令
