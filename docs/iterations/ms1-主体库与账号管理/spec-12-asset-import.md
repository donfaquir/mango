# SPEC-12: 素材导入管线

## 概述

实现把外部图片文件导入到项目的完整流程：拖入或选择文件 → 校验 → 计算 SHA-256 → 复制到 `assets/` → 生成 200×200 webp 缩略图到 `thumbnails/` → 写入 `asset` 表 → 返回 Asset 记录。所有路径以**项目根的相对路径**入库。

对应 `开发任务.md` 任务 7。MS1 只支持图片类型（jpg / png / webp）；视频/音频导入推迟到 MS2/MS5。

## 设计目标 / 非目标

**目标**：
- 单一通用 IPC 命令 `import_asset(input)` 完成端到端导入
- 校验：扩展名、文件存在性、文件大小（默认上限 50 MiB）
- 内容指纹：SHA-256，防止文件名变化导致的引用断裂（spec-10 §"素材链接断裂防护"）
- **内容去重**：同一文件多次导入复用同一 `asset` 行（节省磁盘，建立稳定 id 引用）
- 缩略图：200×200 webp，居中裁剪填充（cover 模式）；失败不阻塞导入
- 系统文件拖入：通过 Tauri `onDragDropEvent` 拦截（前端 spec-14 实现 UI 部分）
- 前端反馈：返回 `Asset` 记录给调用方（参考图设置等用例）
- DB 连接占用最小化：文件 IO 阶段**不**持有 rusqlite Connection（spec 设计的核心约束）
- 元数据：导入时把图片宽高写入 `metadata_json`，便于 V2 展示

**非目标**：
- 不实现 video/audio 导入（asset_type 列保留，MS2/MS5 扩展）
- 不在本 spec 把 Asset 自动绑定到某个实体；调用方负责后续 update（如 `update_character.reference_image_path`）
- 不实现链接断裂检测/修复（V2）
- 不实现已导入素材的"重新生成缩略图"IPC（V2；MS1 用户若需修复可手动删 row 再导一遍）
- 不限制每项目 asset 总数；理论上限 ~50 MiB × N 张，由用户磁盘容量自然约束

## 技术方案

### 目录/文件结构

```
crates/core/src/
├── asset/                              # 新增模块
│   ├── mod.rs                          # 模块导出
│   ├── import.rs                       # 导入流水线（校验 + 复制 + 哈希）
│   ├── thumbnail.rs                    # 缩略图生成 + 元数据提取
│   └── error.rs                        # 模块特定错误（扩展 CoreError）
├── models/
│   └── asset.rs                        # Asset / ImportAssetInput / 类型
├── db/queries/
│   └── asset.rs                        # asset 表 CRUD + 按 content_hash 查询
└── lib.rs                              # 添加 pub mod asset;

src-tauri/src/commands/
├── asset.rs                            # import_asset / list_assets / delete_asset
└── project.rs                          # 在 get_project / list_projects 之后 allow scope（见 §FsScope）

src/hooks/
└── useAssets.ts

src/lib/
└── assetUrl.ts                         # convertFileSrc 包装
```

### 依赖项变更

**新增 Rust 依赖**（需批准）：
```toml
# workspace 根 Cargo.toml
image = { version = "0.25", default-features = false, features = ["jpeg", "png", "webp"] }
```

> `image` 是 Rust 生态最成熟的纯 Rust 图像库，0.25+ 内置 webp 编码（无需 C 依赖）。**实施前**用 `WebSearch` 核对当时的最新 stable 版本（项目规则要求）。`sha2` 与 `uuid` 已在 workspace 中（MS0 spec-04 引入）。

`crates/core/Cargo.toml` 引用：
```toml
[dependencies]
image = { workspace = true }
```

### 数据模型

`crates/core/src/models/asset.rs`：
```rust
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum AssetType {
    Image,
    Video,
    Audio,
    Script,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Asset {
    pub id: String,
    pub project_id: String,
    pub shot_id: Option<String>,
    pub asset_type: AssetType,
    pub original_name: String,
    pub file_path: String,           // 相对项目根，如 "assets/{id}.png"
    pub thumbnail_path: Option<String>,
    #[specta(type = specta_typescript::Number)]
    pub file_size: i64,
    pub content_hash: Option<String>,  // SHA-256 hex
    pub metadata_json: Option<String>, // {"width":1920,"height":1080}
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize, Type)]
pub struct ImportAssetInput {
    pub project_id: String,
    /// 外部源文件的绝对路径（来自拖放事件或文件选择器）
    pub source_path: String,
    /// 可选：绑定到 shot；MS1 主体导入不传。空串视为 None。
    #[serde(default)]
    pub shot_id: Option<String>,
}
```

### 常量

```rust
// crates/core/src/asset/import.rs
pub const MAX_FILE_SIZE_BYTES: u64 = 50 * 1024 * 1024;
pub const ALLOWED_IMAGE_EXTS: &[&str] = &["jpg", "jpeg", "png", "webp"];
pub const THUMB_SIZE: u32 = 200;
```

> 抽到模块顶部为命名常量，便于错误消息引用 + 测试断言；前端文案"最大 50 MiB"可以由 `pnpm tauri-specta` 暴露或 hardcode 中文显示。

### 导入流水线（三段式，DB 连接最小占用）

核心约束：`tokio-rusqlite` 全部 IO 在单一后台线程串行执行。如果 import 整体跑在 `conn.call(...)` 闭包里，复制+哈希 50 MiB 大图 + image crate 解码会**阻塞所有其它 DB 查询**。

解决：拆成 3 个阶段。

```rust
// crates/core/src/asset/import.rs
use std::fs;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::asset::thumbnail;
use crate::db::queries::asset as queries;
use crate::error::{CoreError, Result};
use crate::models::asset::{Asset, AssetType, ImportAssetInput};
use crate::paths;

/// 阶段 1：从 DB 读取项目根路径（持锁极短）。
pub fn resolve_project_root(conn: &Connection, project_id: &str) -> Result<PathBuf> {
    let path: String = conn.query_row(
        "SELECT root_path FROM project WHERE id = ?1",
        params![project_id],
        |r| r.get(0),
    ).map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => CoreError::NotFound {
            entity: "project",
            id: project_id.to_string(),
        },
        other => CoreError::Sqlite(other),
    })?;
    Ok(PathBuf::from(path))
}

/// 阶段 2：纯文件系统操作（不持有 Connection）。返回准备好的 ImportArtifacts。
pub struct ImportArtifacts {
    pub asset_id: String,
    pub original_name: String,
    pub file_relative: String,
    pub thumb_relative: Option<String>,
    pub file_size: i64,
    pub content_hash: String,
    pub metadata_json: Option<String>,
}

pub fn prepare_artifacts(project_root: &Path, source_path: &str) -> Result<ImportArtifacts> {
    let source = PathBuf::from(source_path);
    if !source.is_file() {
        return Err(CoreError::Validation(
            format!("source file not found: {source_path}"),
        ));
    }

    let ext = source
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase())
        .unwrap_or_default();

    if !ALLOWED_IMAGE_EXTS.contains(&ext.as_str()) {
        return Err(CoreError::Validation(
            format!("unsupported file extension: .{ext}; allowed: jpg, jpeg, png, webp"),
        ));
    }

    let metadata = fs::metadata(&source)?;
    if metadata.len() > MAX_FILE_SIZE_BYTES {
        return Err(CoreError::Validation(format!(
            "file too large: {} bytes > {} bytes (max 50 MiB)",
            metadata.len(), MAX_FILE_SIZE_BYTES
        )));
    }

    paths::ensure_project_layout(project_root)?;

    let asset_id = Uuid::new_v4().to_string();
    let file_name = format!("{asset_id}.{ext}");
    let dest = paths::assets_dir(project_root).join(&file_name);

    // 复制 + 哈希（一遍流读取，避免大文件双倍 IO）
    let hash = copy_with_hash(&source, &dest)?;

    // 缩略图 + 元数据
    let thumb_name = format!("{asset_id}_thumb.webp");
    let thumb_dest = paths::thumbnails_dir(project_root).join(&thumb_name);
    let (thumb_relative, metadata_json) = match thumbnail::generate_with_metadata(&dest, &thumb_dest) {
        Ok((dims, _path)) => (
            Some(format!("{}/{}", paths::THUMBNAILS_SUBDIR, thumb_name)),
            Some(serde_json::json!({ "width": dims.0, "height": dims.1 }).to_string()),
        ),
        Err(e) => {
            tracing::warn!("thumbnail generation failed for {asset_id}: {e}");
            (None, None)
        }
    };

    let original_name = source
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();
    let file_relative = format!("{}/{}", paths::ASSETS_SUBDIR, file_name);

    Ok(ImportArtifacts {
        asset_id,
        original_name,
        file_relative,
        thumb_relative,
        file_size: metadata.len() as i64,
        content_hash: hash,
        metadata_json,
    })
}

/// 阶段 3：INSERT DB（持锁极短）。
/// 如果同项目下已有相同 content_hash 的 asset，复用旧行并删除阶段 2 新写入的文件（去重）。
pub fn persist_artifacts(
    conn: &Connection,
    project_id: &str,
    shot_id: Option<&str>,
    project_root: &Path,
    artifacts: ImportArtifacts,
) -> Result<Asset> {
    // 去重：同项目内同 hash 已存在 → 复用
    if let Some(existing) = queries::find_by_content_hash(conn, project_id, &artifacts.content_hash)? {
        // 清理本次新写入的副本
        let _ = fs::remove_file(project_root.join(&artifacts.file_relative));
        if let Some(t) = &artifacts.thumb_relative {
            let _ = fs::remove_file(project_root.join(t));
        }
        return Ok(existing);
    }

    conn.execute(
        "INSERT INTO asset
            (id, project_id, shot_id, asset_type, original_name,
             file_path, thumbnail_path, file_size, content_hash, metadata_json)
         VALUES (?1, ?2, ?3, 'image', ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            artifacts.asset_id, project_id, shot_id, artifacts.original_name,
            artifacts.file_relative, artifacts.thumb_relative,
            artifacts.file_size, artifacts.content_hash, artifacts.metadata_json,
        ],
    )?;

    queries::get_by_id(conn, &artifacts.asset_id)
}

/// 高阶组合（CLI / 同步路径使用）。Tauri command 层应自行编排三段，
/// 让阶段 2 不持有 tokio-rusqlite Connection 锁。
pub fn import(conn: &Connection, input: ImportAssetInput) -> Result<Asset> {
    let project_root = resolve_project_root(conn, &input.project_id)?;
    let artifacts = prepare_artifacts(&project_root, &input.source_path)?;
    let shot_id = input.shot_id.as_deref().and_then(|s| {
        let t = s.trim();
        if t.is_empty() { None } else { Some(t) }
    });
    persist_artifacts(conn, &input.project_id, shot_id, &project_root, artifacts)
}

fn copy_with_hash(src: &Path, dst: &Path) -> Result<String> {
    let mut reader = BufReader::new(fs::File::open(src)?);
    let mut writer = fs::File::create(dst)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 { break; }
        hasher.update(&buf[..n]);
        std::io::Write::write_all(&mut writer, &buf[..n])?;
    }
    Ok(format!("{:x}", hasher.finalize()))
}
```

> **设计说明**：
> - **三段拆分是核心**：阶段 1/3 在 `conn.call` 闭包内（持锁），阶段 2 在 tokio 普通任务中（不持锁）。Tauri command 层负责编排（见下文）。
> - **去重**：在 INSERT 之前查 `content_hash`，命中则清理本次写入的孤儿文件并复用旧 Asset。下游引用 `asset.id` 自动稳定。
> - **shot_id trim**：前端把空 `<select>` 序列化成 `""` 是常见误用，core 层防御性 trim→None。
> - 缩略图失败不阻塞主流程：图像格式损坏但元数据可读时，导入仍可成功；前端用 `thumbnail_path === null` 判断是否走占位符。
> - 路径入库统一用 `/` 分隔（即使 Windows）；读取端 `Path::new` 接受双向斜杠，保持 SQLite 跨平台一致。
> - **不在事务里**做文件 IO：先建文件再 INSERT。INSERT 失败时遗留的孤儿文件由 V2 的"垃圾回收"扫描清理。

### Tauri command 三段编排

```rust
// src-tauri/src/commands/asset.rs
#[tauri::command]
#[specta::specta]
pub async fn import_asset(
    state: State<'_, AppState>,
    app: tauri::AppHandle,
    input: ImportAssetInput,
) -> Result<Asset, IpcError> {
    // 阶段 1：DB → project_root（持锁极短）
    let project_id_for_q = input.project_id.clone();
    let project_root = state.db.call(move |conn| {
        mango_core::asset::import::resolve_project_root(conn, &project_id_for_q)
            .map_err(|e| tokio_rusqlite::Error::Other(Box::new(e)))
    }).await.map_err(IpcError::from)?;

    // 阶段 1.5：动态把项目根加入 FsScope（保证前端 convertFileSrc 能访问）
    register_project_scope(&app, &project_root)?;

    // 阶段 2：纯文件 IO，不持有 DB 连接
    let source_path = input.source_path.clone();
    let project_root_for_io = project_root.clone();
    let artifacts = tokio::task::spawn_blocking(move || {
        mango_core::asset::import::prepare_artifacts(&project_root_for_io, &source_path)
    })
    .await
    .map_err(|e| IpcError::internal(e.to_string()))?
    .map_err(IpcError::from)?;

    // 阶段 3：INSERT DB（持锁极短）
    let project_id = input.project_id.clone();
    let shot_id = input.shot_id.clone();
    let project_root_for_p = project_root.clone();
    state.db.call(move |conn| {
        let shot = shot_id.as_deref().and_then(|s| {
            let t = s.trim();
            if t.is_empty() { None } else { Some(t) }
        });
        mango_core::asset::import::persist_artifacts(
            conn, &project_id, shot, &project_root_for_p, artifacts,
        ).map_err(|e| tokio_rusqlite::Error::Other(Box::new(e)))
    })
    .await
    .map_err(IpcError::from)
}

#[tauri::command]
#[specta::specta]
pub async fn list_assets(/* ... */) -> Result<Vec<Asset>, IpcError> { /* 单段 conn.call */ }

#[tauri::command]
#[specta::specta]
pub async fn delete_asset(/* ... */) -> Result<(), IpcError> { /* 单段 conn.call；仅删 DB 行 */ }
```

### asset 表 CRUD（`crates/core/src/db/queries/asset.rs`）

只暴露 MS1 用得上的：
- `pub fn get_by_id(conn, id) -> Result<Asset>`
- `pub fn list(conn, ListAssetsOptions { project_id, asset_type? }) -> Result<Vec<Asset>>`
- `pub fn delete(conn, id) -> Result<()>`（仅删 DB 行；文件留在磁盘，由 V2 回收）
- `pub fn find_by_content_hash(conn, project_id, hash) -> Result<Option<Asset>>`（新增；去重用）

`create` 不公开——所有创建走 `asset::import::persist_artifacts`。

### 缩略图与元数据（`crates/core/src/asset/thumbnail.rs`）

```rust
use std::path::Path;
use image::imageops::FilterType;

const THUMB_SIZE: u32 = 200;

/// 生成 200x200 webp 缩略图，并返回原图尺寸 (width, height)。
pub fn generate_with_metadata(
    source: &Path,
    dest: &Path,
) -> image::ImageResult<((u32, u32), ())> {
    let img = image::open(source)?;
    let dims = (img.width(), img.height());
    let resized = img.resize_to_fill(THUMB_SIZE, THUMB_SIZE, FilterType::Lanczos3);
    resized.save_with_format(dest, image::ImageFormat::WebP)?;
    Ok((dims, ()))
}
```

> 用 `resize_to_fill` 而非 `thumbnail`/`resize`，保证所有缩略图同尺寸便于网格对齐；`Lanczos3` 在 200px 缩放下质量与速度平衡良好。元数据 `metadata_json` 仅含 `width`/`height`；色彩空间、EXIF 等 V2 再扩展。

### FsScope：让前端能用 `convertFileSrc` 访问项目目录

Tauri 2 的 asset 协议（`convertFileSrc`）默认拒绝任意路径访问。需要动态把项目根目录加入 FsScope。

**静态配置**（`src-tauri/tauri.conf.json`）：
```jsonc
{
  "app": {
    "security": {
      "assetProtocol": {
        "enable": true,
        "scope": []     // 留空；运行时由 register_project_scope 动态加
      }
    }
  }
}
```

**运行时动态加 scope**（`src-tauri/src/commands/asset.rs` 内部辅助函数）：
```rust
fn register_project_scope(app: &tauri::AppHandle, project_root: &Path) -> Result<(), IpcError> {
    use tauri::Manager;
    // tauri 2.x 的 asset_protocol_scope API；具体名实施时按当时文档核对
    let scope = app.asset_protocol_scope();
    scope.allow_directory(project_root, /* recursive */ true)
        .map_err(|e| IpcError::internal(e.to_string()))
}
```

> **设计说明**：
> - 静态 scope 留空避免一次性放开整个文件系统。每次 `import_asset` 调用前注册一次（幂等：`allow_directory` 重复调用不报错）。
> - 在 GUI 应用启动后第一次访问任何项目时也要走这一步。建议在 `useProject(id).data` 拿到后调用专门的 `register_project_scope(rootPath)` IPC，避免每次拖入都重新注册。
> - 实施前用 `WebSearch` 核对 `asset_protocol_scope` API 是否变名（tauri 2.x 在 release 之间偶有调整）。

### 文件拖入接入（前端，spec-14 详述）

主体详情页（spec-14）的参考图区域监听拖放：
```ts
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { useEffect } from "react";

useEffect(() => {
  let active = true;
  const unlistenPromise = getCurrentWebview().onDragDropEvent(async (event) => {
    if (!active || event.payload.type !== "drop") return;
    const paths = event.payload.paths;
    if (!paths.length || !projectId) return;

    const asset = await unwrap(commands.importAsset({
      projectId,
      sourcePath: paths[0],
    }));
    onChange(asset.filePath);   // 由父表单 react-hook-form 接管
  });
  return () => {
    active = false;
    // unlisten 返回 Promise<UnlistenFn>；要等 promise 才能 unlisten
    unlistenPromise.then((un) => un());
  };
}, [projectId, onChange]);
```

> **设计说明**：
> - `active` 标志防御性处理 race condition：组件已卸载但事件回调仍在执行。
> - `unlistenPromise.then(fn => fn())` 必须等 promise 解析；直接 `unlistenPromise()` 不工作。
> - 多个 Dropzone 同时挂载的冲突由 spec-14 §"useGlobalDropTarget" 解决。

### 错误场景

| 场景 | 处理 |
|---|---|
| 文件不存在（拖放路径已被移动） | Validation 错误返回前端，toast 提示 |
| 扩展名不支持 | Validation；前端提示"仅支持 JPG/PNG/WebP" |
| 文件 > 50 MiB | Validation；提示"文件过大，最大 50 MiB" |
| 缩略图生成失败（图像损坏） | 主流程成功，记 warn；asset.thumbnail_path = NULL，metadata_json = NULL |
| 复制中途磁盘满 | IO 错误返回；遗留半个目标文件——V2 的 GC 扫描；MS1 不主动清 |
| 同一文件多次拖入 | 命中 content_hash 去重，复用旧 asset 行，删除新副本 |
| project_id 无效 | NotFound |
| FsScope 注册失败（路径含非法字符） | IpcError；asset 实际导入是否完成视错误时机而定，前端 toast |

### 前端 hooks

`src/hooks/useAssets.ts`：
```ts
export function useImportAsset() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (input: ImportAssetInput) =>
      unwrap(commands.importAsset(input)),
    onSuccess: (_, input) => {
      qc.invalidateQueries({ queryKey: ["assets", input.projectId] });
    },
  });
}

export function useAssetList(projectId: string | undefined, type?: AssetType) {
  return useQuery<Asset[]>({
    queryKey: ["assets", projectId, type ?? "all"],
    queryFn: () => unwrap(commands.listAssets({
      projectId: projectId!,
      assetType: type,
    })),
    enabled: !!projectId,
  });
}
```

### 路径解析（前端预览）

前端拿到 `asset.file_path` = `"assets/{id}.png"`，需要拼接绝对路径才能 `<img src>`。Tauri 通过 `convertFileSrc(path)` 将本地路径转为 `tauri://` 协议 URL。

新建工具：`src/lib/assetUrl.ts`
```ts
import { convertFileSrc } from "@tauri-apps/api/core";
import { join } from "@tauri-apps/api/path";

export async function resolveAssetUrl(
  projectRoot: string,
  relativePath: string,
): Promise<string> {
  const abs = await join(projectRoot, relativePath);
  return convertFileSrc(abs);
}
```

`Project.root_path` 已由 spec-10 提供，前端 `useProject(id)` 拿到后传入。spec-14 的 `useResolvedAssetUrl` hook 把异步 join + convertFileSrc 包成同步可用值（见 spec-14）。

## 测试策略

### Rust 单元测试

`crates/core/src/asset/import.rs`：
- 用 `tempfile::tempdir()` 建临时项目根 + 测试图片
- `prepare_artifacts_jpg_succeeds` 验证 file/thumb 都生成、artifacts 字段正确（含 metadata_json）
- `prepare_artifacts_unsupported_ext_fails`
- `prepare_artifacts_oversize_fails`（用 `vec![0u8; 51 * MiB]` 建临时文件）
- `content_hash_matches`（手算 sha256 对比）
- `corrupt_image_succeeds_without_thumbnail`（写入仅头几字节是 PNG 魔数的伪文件 → artifacts.thumb_relative is None）
- `persist_dedupes_same_content_hash`：同一文件导入两次 → 第二次返回同一 asset.id，磁盘只剩一份文件
- `shot_id_empty_string_becomes_none`：通过 `import` 入口传 `Some("")` → DB row 的 shot_id is NULL

`crates/core/src/asset/thumbnail.rs`：
- `thumbnail_size_is_200x200`（生成后 image::open 读尺寸验证）
- `thumbnail_format_is_webp`（验证文件头 magic）
- `metadata_returns_original_dims`

### 前端测试

`src/hooks/useAssets.test.ts`：
- mock `commands.importAsset` 返回固定 Asset
- 验证 mutation 成功后 invalidate 正确 query key

## 验收标准

- [ ] 拖入 jpg/png/webp 图片：文件出现在 `<root>/assets/{id}.{ext}`，缩略图在 `<root>/thumbnails/{id}_thumb.webp`
- [ ] `asset` 表行 `file_path` 是相对路径（含 `/` 分隔），`content_hash` 为 64 字符 hex，`metadata_json` 含 width/height
- [ ] 拖入 .pdf / .gif / .bmp 等：返回 Validation 错误，前端 toast 显示
- [ ] 拖入超过 50 MiB 的文件：被拒绝
- [ ] 同一文件导入第二次：返回首次的 asset.id，磁盘无重复副本（content_hash 去重）
- [ ] 缩略图生成失败时，主流程仍写入 asset 行（`thumbnail_path = NULL`），列表用占位符
- [ ] `import_asset` IPC 阶段 2（文件 IO）**不**持有 tokio-rusqlite 连接锁；可由"导入 50 MiB 文件同时另一 IPC 调用 `list_projects` 验证 < 100ms 响应"手测
- [ ] FsScope 在导入前注册项目根目录；前端 `convertFileSrc` 可成功加载图片
- [ ] `cargo test -p mango-core` 至少 8 用例覆盖 import / thumbnail（含去重 + shot_id trim）
- [ ] `pnpm typecheck` 全绿；`assetUrl` 工具产出可在 `<img>` 中渲染

## 依赖关系

- **前置**：spec-10（需要项目根路径与 `paths::*` 模块 + `CoreError::Io`）；spec-11（Character 已有 `update_character.reference_image_path` 字段）
- **后续**：
  - spec-14（主体详情页）：使用 `useImportAsset` + `resolveAssetUrl` 实现拖放区域与预览
  - MS2 任务结果下载：复用 `asset::import::*` 三段 API 写入生成的图片/视频
  - MS5 视频导入：扩展 `ALLOWED_*_EXTS` 与 `AssetType::Video` 分支

## 与原任务文档的差异

`开发任务.md` 任务 7 验收里只笼统说"自动复制""自动生成缩略图"，未规定：
- 错误恢复策略（孤儿文件谁清理）→ 本 spec 决定：MS1 不清，V2 GC
- 缩略图失败是否阻塞（本 spec 决定不阻塞）
- 大小上限（本 spec 定 50 MiB）
- 哈希算法与编码（本 spec 定 SHA-256 hex）
- 路径分隔符（本 spec 定 `/`）
- 重复文件处理（本 spec 决定按 content_hash 去重）
- DB 连接占用（本 spec 强制三段拆分）

实施时按本 spec 执行；与任务文档不一致处以本 spec 为准。
