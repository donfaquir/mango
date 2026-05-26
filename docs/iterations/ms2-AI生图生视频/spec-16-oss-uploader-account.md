# SPEC-16: OSS 上传层与账号管理扩展

## 概述

为后续视频模型（happyhorse-1.0-r2v 及 MS3+ 任何只接公网 URL 的模型）提供「本地素材 → 公网 HTTPS URL → 任务完成后清理」的统一适配层。本 spec 包含三条独立但耦合的工作线，必须打包在一个 PR 提交以保持账号 schema 一致：

1. **`asset_uploader` 模块**：定义 `AssetUploader` trait + `OssUploader`（阿里 OSS）实现
2. **账号管理扩展**：`api_account` 加 `params_json` 字段（migration 004）+ OSS 双密钥分存（endpoint/bucket/AK_ID 入 SQLite、AK_secret 入 keyring）+ `AddAccountDialog` 动态字段
3. **Provider seed 重构**：删除 `kling`（已决议放弃）、保留 `jimeng`（MS4 占位）、新增 `bailian`（wan2.7-image-pro + happyhorse-1.0-r2v）

> ⚠️ **破坏性变更**：
> - `CreateApiAccountInput` / `UpdateApiAccountInput` 新增 `oss: Option<OssConfigInput>` 字段
> - `ApiAccount` 新增 `oss_config: Option<OssConfigPublic>` 字段（不含 secret）
> - seed 主动 `DELETE FROM provider WHERE id = 'kling'`，CASCADE 清掉对应 api_account 与 model 行（开发期数据破坏可接受——项目未发布、目前无真实用户）

> ⚠️ **依赖说明**：本 spec 与 [SPEC-15](spec-15-task-engine-skeleton.md) 在接口面有耦合——`ProviderCredentials.extra_json` 的具体编码格式由本 spec 定义。spec-15 实现可先 stub `extra_json: None`，本 spec 完成后回填解析逻辑。

## 设计目标 / 非目标

**目标**：

- 定义 `AssetUploader` trait：`upload(local_path) -> UploadedAsset`、`cleanup(remote_id)`，与具体 provider 解耦——后续任何只接 URL 的模型（spec-17 的 happyhorse、未来 MS3+ 视频模型）都复用同一 trait
- `OssUploader` 实现：阿里 OSS V4 签名上传 + 生成临时签名 GET URL（默认 1h）+ DeleteObject
- `api_account` 加 `params_json TEXT NOT NULL DEFAULT '{}'` 字段（migration 004）
- 账号 keyring 支持「主 key」和「OSS secret」两条目共存：keyring entry name 区分 `api_account:<id>` 与 `api_account:<id>:oss_secret`
- `AddAccountDialog` 当 `providerId` 在「需要 OSS」清单（MS2 仅 `bailian`）时动态展开 4 字段：endpoint / bucket / AK ID / AK secret
- seed 重构：删除 `kling`（CASCADE）+ 保留 `jimeng`（MS4 占位）+ 新增 `bailian`（含 wan2.7-image-pro + happyhorse-1.0-r2v 两个 model）
- 提供 `account::service::resolve_credentials(account_id) -> ProviderCredentials`：spec-15 的 task_engine 通过它一次性拿全主 key + OSS 配置（如有），不再分开调

**非目标**：

- 把 OSS 上传集成到具体任务流——由 spec-17 在 BailianProvider 内部决定何时调 `AssetUploader`
- 替代级 STS（临时凭证）支持——MS2 只支持 AK/SK 长期凭证；MS3+ 视用户反馈再加
- 上传进度回调——MS2 上传同步执行（参考图通常 < 5MB，秒级完成），不显示进度
- 多 OSS 厂商支持——本 spec 仅适配阿里云 OSS；未来若接 AWS S3 / 七牛等，新增 trait 实现即可，trait 已为此中立设计
- 账号管理 UI 的"测试连通性"按钮——保留 spec-13 已有的 `verify_storage`（仅查 keyring 非空），不实测 OSS 上传往返；MS3 视情加

## 技术方案

### 目录/文件结构

```text
crates/core/src/
├── error.rs                            # +Upload(String) variant
├── account/
│   ├── service.rs                      # 扩 create/update 处理 oss 字段；新增 resolve_credentials
│   └── keyring.rs                      # 加 oss_secret 的 entry name 辅助
├── db/
│   └── migrations/
│       └── 004_add_api_account_params_json.sql   # 新增
├── models/
│   └── api_account.rs                  # 加 params_json 反序列化 + Public/Input/Update 改造
├── provider/                           # spec-15 已建
│   ├── mod.rs                          # +pub mod asset_uploader;
│   └── asset_uploader/                 # 新增子模块
│       ├── mod.rs                      # 重导出 AssetUploader, UploadedAsset, OssUploader
│       ├── traits.rs                   # AssetUploader trait + UploadedAsset 类型
│       └── oss.rs                      # OssUploader 实现
└── seed/
    └── providers.rs                    # kling DELETE; bailian INSERT; jimeng 保留

src/components/accounts/
├── AddAccountDialog.tsx                # 动态展开 OSS 字段（仅 bailian）
├── EditAccountDialog.tsx               # 新增：与 Add 共享 OssFields 子组件
├── OssFields.tsx                       # 新增：抽出 OSS 4 字段表单
└── providersWithOss.ts                 # 常量 ['bailian']
```

### 依赖项变更

**Rust（待批，实施时 web 验证版本）**：

```toml
# workspace 根 Cargo.toml
# OSS SDK 选型：候选 1（推荐）—— 自写 reqwest + V4 签名（无新依赖，签名实现 ~150 行）
# 候选 2 —— `oss-rust-sdk = "x"`（社区维护活跃度待 web 验证）
# 候选 3 —— `aliyun-oss-rust-sdk = "x"`（社区版本，活跃度待 web 验证）
hmac = "0.12"               # SDK 候选 1 用：HMAC-SHA256（V4 签名）
sha2 = "0.11"               # 已在；V4 签名 hash
percent-encoding = "2"      # SDK 候选 1 用：URL canonical
```

> ⚠️ **实施时强制**：在 PR 描述里记录最终 SDK 选型 + WebSearch 拿到的当时 stable 版本。三选一的判据：(a) 是否官方/可信维护方；(b) V4 签名是否已实现且与阿里官方一致（候选 2 部分实现仅 V1）；(c) 是否依赖过多重型 crate（候选 2/3 拉 ~80 个依赖；候选 1 仅多 hmac + percent-encoding）。

**Tauri 插件（无新增）**

**npm（无新增）**——shadcn `Input` / `Select` 已有。

### 数据模型变更

**`004_add_api_account_params_json.sql`**：

```sql
ALTER TABLE api_account ADD COLUMN params_json TEXT NOT NULL DEFAULT '{}';
```

> **设计说明**：`NOT NULL DEFAULT '{}'` 是 SQLite ALTER TABLE 唯一允许的"加非空列"形式（必须给静态默认）。`{}` 是合法 JSON 空对象，应用层反序列化时直接得空 struct，无需特殊兜底。已存在的 MS1 行（kling/jimeng 上的账号）会自动获得 `'{}'`，符合「老账号无 OSS 配置」语义。

**`models/api_account.rs`** 改造：

```rust
use serde::{Deserialize, Serialize};
use specta::Type;

/// 公开给前端的账号视图。AK secret 在 keyring 不在此处。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct ApiAccount {
    pub id: String,
    pub provider_id: String,
    pub label: String,
    pub key_last4: String,
    /// 仅对配置了 OSS 的账号填充；secret 永不出现。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oss_config: Option<OssConfigPublic>,
    #[specta(type = Option<specta_typescript::Number>)]
    pub usage_quota: Option<i64>,
    #[specta(type = specta_typescript::Number)]
    pub usage_used: i64,
    pub last_used_at: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct OssConfigPublic {
    pub endpoint: String,
    pub bucket: String,
    pub access_key_id: String,           // 公开端 ID 可见，secret 不可见
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    /// 签名 URL 有效期（秒），默认 3600。
    #[specta(type = specta_typescript::Number)]
    pub url_expires_seconds: u32,
}

#[derive(Debug, Deserialize, Type)]
pub struct CreateApiAccountInput {
    pub provider_id: String,
    pub label: String,
    pub api_key: String,
    /// 仅对需要素材上传的 provider 必填（MS2 仅 bailian）；否则忽略。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oss: Option<OssConfigInput>,
}

#[derive(Debug, Deserialize, Type)]
pub struct OssConfigInput {
    pub endpoint: String,
    pub bucket: String,
    pub access_key_id: String,
    pub access_key_secret: String,        // 提交后写 keyring 立即丢弃
    #[serde(default)]
    pub region: Option<String>,
    #[serde(default = "default_expires")]
    #[specta(type = specta_typescript::Number)]
    pub url_expires_seconds: u32,
}

fn default_expires() -> u32 { 3600 }

#[derive(Debug, Deserialize, Type)]
pub struct UpdateApiAccountInput {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    /// Some 替换整组 OSS 配置；None 不动。
    /// 内部子字段不支持单独改——前端表单永远把 4 个字段一起重提。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub oss: Option<OssConfigInput>,
}
```

DB 行 `params_json` 字段的存储格式：

```json
{
  "oss": {
    "endpoint": "oss-cn-hangzhou.aliyuncs.com",
    "bucket": "mango-ref-images",
    "access_key_id": "LTAI5tXXX",
    "region": "cn-hangzhou",
    "url_expires_seconds": 3600
  }
}
```

`access_key_secret` 不在此 JSON 内，走 keyring entry name `api_account:<id>:oss_secret`。

### 错误类型扩展

`crates/core/src/error.rs`：

```rust
#[derive(Error, Debug)]
pub enum CoreError {
    // ... existing variants ...

    /// OSS 上传/清理失败。封装 SDK 错误为 String，便于前端展示。
    /// 区别于 Provider 错误：upload 失败可识别为「OSS 配置错」而非「百炼接口错」。
    #[error("upload error: {0}")]
    Upload(String),
}
```

### keyring 扩展

`crates/core/src/account/keyring.rs` 加：

```rust
/// 主 API key 的 entry name（已有）。
pub fn user_id(account_id: &str) -> String {
    format!("api_account:{}", account_id)
}

/// OSS access_key_secret 的 entry name。
pub fn oss_secret_id(account_id: &str) -> String {
    format!("api_account:{}:oss_secret", account_id)
}
```

`account::service::create` / `update` 在写 OSS 时走 `oss_secret_id`：

```rust
if let Some(oss) = &input.oss {
    keyring.store(&oss_secret_id(&id), &oss.access_key_secret)?;
}
```

`delete` 删主 key 后多删一次 OSS secret（best-effort，错误吞掉）：

```rust
if let Err(e) = keyring.remove(&oss_secret_id(id)) {
    tracing::debug!("oss secret entry not present for {id}: {e}");
}
```

### account::service 改造

```rust
pub fn create(
    conn: &Connection,
    keyring: &dyn KeyringStore,
    input: CreateApiAccountInput,
) -> Result<ApiAccount> {
    // 既有：校验 / keyring 写主 key / INSERT 行
    // 新增：
    if let Some(oss) = &input.oss {
        validate_oss(oss)?;
        keyring.store(&oss_secret_id(&id), &oss.access_key_secret)?;

        let params = serde_json::json!({
            "oss": {
                "endpoint": oss.endpoint,
                "bucket": oss.bucket,
                "access_key_id": oss.access_key_id,
                "region": oss.region,
                "url_expires_seconds": oss.url_expires_seconds,
            }
        });
        conn.execute(
            "UPDATE api_account SET params_json = ?1 WHERE id = ?2",
            params![params.to_string(), id],
        )?;
    }
    queries::get_by_id(conn, &id)
}

fn validate_oss(oss: &OssConfigInput) -> Result<()> {
    if oss.endpoint.is_empty() { return Err(CoreError::Validation("oss.endpoint required".into())); }
    if !oss.endpoint.starts_with("oss-") || !oss.endpoint.ends_with(".aliyuncs.com") {
        return Err(CoreError::Validation(
            "oss.endpoint must look like 'oss-<region>.aliyuncs.com'".into()));
    }
    if oss.bucket.is_empty() { return Err(CoreError::Validation("oss.bucket required".into())); }
    if oss.access_key_id.is_empty() || oss.access_key_secret.is_empty() {
        return Err(CoreError::Validation("oss credentials required".into()));
    }
    if oss.url_expires_seconds < 60 || oss.url_expires_seconds > 86400 {
        return Err(CoreError::Validation("oss.url_expires_seconds out of [60, 86400]".into()));
    }
    Ok(())
}
```

新增 `resolve_credentials`：

```rust
use crate::provider::traits::ProviderCredentials;

/// 一次性拼装出 task_engine 所需的全套凭证。spec-15 的 runner 用它替代分散查询。
pub fn resolve_credentials(
    conn: &Connection,
    keyring: &dyn KeyringStore,
    account_id: &str,
) -> Result<ProviderCredentials> {
    let row = queries::get_with_params(conn, account_id)?; // get_by_id 的扩展版
    let api_key = keyring.fetch(account_id)?;

    // 解 oss 子树
    let extra_json = if !row.params_json.is_empty() && row.params_json != "{}" {
        let mut params: serde_json::Value = serde_json::from_str(&row.params_json)
            .map_err(|e| CoreError::Validation(format!("api_account.params_json: {e}")))?;
        if let Some(oss) = params.get_mut("oss") {
            // 把 secret 注入回去（仅在内存中，task_engine 把 ProviderCredentials 传给 provider 后立即销毁）
            let secret = keyring.fetch(&oss_secret_id(account_id))?;
            oss["access_key_secret"] = serde_json::Value::String(secret);
        }
        Some(params.to_string())
    } else { None };

    Ok(ProviderCredentials { api_key, extra_json })
}
```

> **设计说明**：`extra_json` 是 spec-15 在 `ProviderCredentials` 上预留的逃生口。spec-17 的 BailianProvider 在内部反序列化它的 `oss` 字段构造 `OssUploader`。这样 provider 不直接访问 DB / keyring，整个凭证生命周期被 `ProviderCredentials` 这个值对象包住，便于 mock。

### AssetUploader trait

`crates/core/src/provider/asset_uploader/traits.rs`：

```rust
use std::path::Path;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::error::Result;

/// 上传完成后的远端表示。`remote_id` 是 cleanup 用的句柄；OSS 实现里是 object key。
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct UploadedAsset {
    /// 公网 HTTPS URL，在 expires_at 前可被 GET（带签名参数）。
    pub url: String,
    /// 仅 cleanup 用——provider 不需要也不应回放给模型。
    pub remote_id: String,
    /// ISO8601 UTC 时间戳；超过后 url 失效。
    pub expires_at: String,
}

#[async_trait]
pub trait AssetUploader: Send + Sync {
    /// 上传本地文件并返回签名 URL。
    async fn upload(&self, local_path: &Path) -> Result<UploadedAsset>;

    /// 清理。best-effort——失败仅 warn 日志，不让任务流因此失败
    /// （任务已 success/failed/cancelled，此时已经无法回滚）。
    async fn cleanup(&self, remote_id: &str) -> Result<()>;
}
```

### OssUploader 实现要点

`crates/core/src/provider/asset_uploader/oss.rs`：

```rust
pub struct OssUploader {
    endpoint: String,            // 'oss-cn-hangzhou.aliyuncs.com'
    bucket: String,
    access_key_id: String,
    access_key_secret: String,
    region: String,              // 'cn-hangzhou'，从 endpoint 解析或显式给
    url_expires_seconds: u32,
    object_prefix: String,       // 默认 'mango/refs/' 让 bucket 内可识别本应用对象
    http: reqwest::Client,
}

impl OssUploader {
    pub fn from_credentials_json(json: &str) -> Result<Self> { /* 解 oss 子树 */ }
}

#[async_trait]
impl AssetUploader for OssUploader {
    async fn upload(&self, local_path: &Path) -> Result<UploadedAsset> {
        // 1) 读取文件 + 计算 sha256（用作 object key 一部分，避免重复上传同内容）
        // 2) object_key = format!("{}{}/{}.{ext}", prefix, today_yyyymmdd, sha[..16])
        // 3) PUT https://{bucket}.{endpoint}/{object_key} with body
        //    - V4 签名 header（X-OSS-Date / Authorization）
        //    - Content-Type 按文件扩展名
        // 4) 生成 presigned GET URL（path 风格 + V4 query 签名，expires=url_expires_seconds）
        // 5) 返回 UploadedAsset { url, remote_id: object_key, expires_at }
    }

    async fn cleanup(&self, remote_id: &str) -> Result<()> {
        // DELETE https://{bucket}.{endpoint}/{object_key} with V4 签名
        // 失败仅 tracing::warn —— 调用方已经在 best-effort 上下文里
    }
}
```

> **设计说明**：
> - **object key 含 sha256 前缀**让同一参考图重复提交任务时（如用户改 prompt 重提）不会浪费上传带宽——OSS 上 PUT 一个已存在的 object 是直接覆盖、合法。
> - **object key 含日期前缀**便于运维定期手动清理（例如 7 天前的所有 `mango/refs/2025-12-XX/*`），即使 cleanup 偶尔失败也不会无限堆积。
> - **不在 trait 里暴露 sha256 / object key 命名规则**——这是 OSS 实现的内部细节。换成 S3/七牛时各自决定 key 命名。

### Provider seed 重构

`crates/core/src/seed/providers.rs` 关键变更：

```rust
const PROVIDERS: &[ProviderSeed] = &[
    ProviderSeed {
        id: "bailian",
        name: "阿里云百炼",
        base_url: "https://dashscope.aliyuncs.com/api/v1",
        docs_url: "https://help.aliyun.com/zh/model-studio/",
        models: &[
            ModelSeed {
                id: "wan2.7-image-pro",
                name: "通义万相 2.7 Pro（文生图）",
                model_type: "image",
            },
            ModelSeed {
                id: "happyhorse-1.0-r2v",
                name: "快乐马 1.0（参考图生视频）",
                model_type: "video",
            },
        ],
    },
    ProviderSeed {
        id: "jimeng",
        name: "即梦",
        base_url: "",      // MS4 接入时填
        docs_url: "",
        models: &[ /* 保留 jimeng-image-v1 / jimeng-video-v1 占位 */ ],
    },
];

/// 应用 seed。除了原 INSERT/UPDATE，新增主动删除被废弃 provider。
pub fn apply(conn: &Connection) -> Result<()> {
    // 主动清理废弃 provider（CASCADE 会清掉依赖的 model 和 api_account）
    const DEPRECATED: &[&str] = &["kling"];
    for id in DEPRECATED {
        let n = conn.execute("DELETE FROM provider WHERE id = ?1", params![id])?;
        if n > 0 {
            tracing::warn!(
                "deleted deprecated provider '{id}' (cascade removed {n} dependent rows)"
            );
        }
    }

    // 既有 INSERT ... ON CONFLICT 流程（PROVIDERS / models）
    // ...
    Ok(())
}
```

> **设计说明**：删 kling 不优雅但必要。选项对比：
> - **A**（采用）：主动 DELETE，CASCADE 清除依赖。开发期数据丢失可接受（项目未发布）；语义最干净
> - **B**：保留 kling 行做"墓碑"。前端要过滤掉它，列出"可用 provider"时 hardcode 排除。耦合更深
> - **C**：把 deprecation 状态加到 provider 表（`deprecated_at TEXT`）。schema 变更更大，本 spec 不做
> 选 A，并把日志级别提到 `warn` 让用户在第一次升级时看到。

### AddAccountDialog 改造

`src/components/accounts/providersWithOss.ts`：

```ts
// MS2 仅 bailian 需要 OSS。MS3+ 视情扩展为 provider 表的 capabilities_json 字段。
export const PROVIDERS_WITH_OSS = ['bailian'] as const;

export function providerNeedsOss(providerId: string): boolean {
  return (PROVIDERS_WITH_OSS as readonly string[]).includes(providerId);
}
```

`src/components/accounts/OssFields.tsx`：

```tsx
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

export interface OssDraft {
  endpoint: string;
  bucket: string;
  accessKeyId: string;
  accessKeySecret: string;
  region: string;
  urlExpiresSeconds: number;
}

export const EMPTY_OSS_DRAFT: OssDraft = {
  endpoint: "",
  bucket: "",
  accessKeyId: "",
  accessKeySecret: "",
  region: "",
  urlExpiresSeconds: 3600,
};

interface Props {
  value: OssDraft;
  onChange: (next: OssDraft) => void;
  disabled?: boolean;
}

export function OssFields({ value, onChange, disabled }: Props) {
  // 4 字段表单 + region 可选 + expires 默认 3600
  // 提示：百炼 happyhorse 视频模型要求参考图通过公网 HTTPS URL 提交，
  //      mango 会用此 OSS 临时上传。详情：docs/iterations/ms2-...
}
```

`AddAccountDialog.tsx` 在 `providerId` 变化时切换：

```tsx
const showOss = providerNeedsOss(providerId);
const [oss, setOss] = useState<OssDraft>(EMPTY_OSS_DRAFT);

const ossValid = !showOss || (
  oss.endpoint.trim() &&
  oss.bucket.trim() &&
  oss.accessKeyId.trim() &&
  oss.accessKeySecret.trim()
);

const disabled = create.isPending || !providerId || !trimmedLabel || !trimmedKey || !ossValid;

// 提交时：
const payload: CreateApiAccountInput = {
  provider_id: providerId,
  label: trimmedLabel,
  api_key: trimmedKey,
  oss: showOss ? {
    endpoint: oss.endpoint.trim(),
    bucket: oss.bucket.trim(),
    access_key_id: oss.accessKeyId.trim(),
    access_key_secret: oss.accessKeySecret.trim(),
    region: oss.region.trim() || null,
    url_expires_seconds: oss.urlExpiresSeconds,
  } : null,
};
```

`EditAccountDialog`（新文件）功能对称：加载时把 `account.oss_config` 拷到 OssFields 状态（secret 字段强制为空，要求用户重输或留空表示不改——这部分 UI 文案要明确）。

> **设计说明**：编辑时 secret 字段留空表示**不改**，与主 api_key 字段的语义一致（spec-13 已有先例）。`UpdateApiAccountInput.oss` 只有在用户主动操作 OSS 区块时才发送，普通改 label 不触发。

### Tauri commands

无新增 command。`create_api_account` / `update_api_account` 的入参签名通过 `CreateApiAccountInput`/`UpdateApiAccountInput` 自动接收新字段。**`cargo test -p mango-tauri export_bindings` 会重新生成 commands.ts，需要在 PR 里 commit 这次 diff。**

### 错误场景

| 场景 | 行为 |
|---|---|
| `create` 时 `oss.endpoint` 不符合 `oss-*.aliyuncs.com` 格式 | `Validation` |
| `create` 时 `oss.url_expires_seconds` 越界 [60, 86400] | `Validation` |
| `create` 时 keyring 写主 key 成功、写 oss_secret 失败 | 立即调 `keyring.remove(主 key)` 回滚；返回 `Keyring` |
| `create` 时 keyring 都成功、UPDATE params_json 失败 | 回滚两条 keyring 条目；返回 `Sqlite` |
| `update` 时 `oss` 是 Some 但 endpoint 空 | `Validation` |
| `delete` 时 oss_secret keyring entry 不存在（老账号无 OSS） | 静默吞掉（`debug!` 日志） |
| `resolve_credentials` 时 params_json 不是合法 JSON | `Validation`——表示 DB 行被外部破坏；任务直接失败让用户重建账号 |
| `resolve_credentials` 时 params_json 含 oss 但 keyring 缺 secret | `Keyring("oss secret missing")`——可能用户在外部清掉了 keychain |
| `OssUploader::upload` 文件不存在 | `Io` |
| `OssUploader::upload` HTTP 401（签名错） | `Upload("oss authentication failed: check access key")` |
| `OssUploader::upload` HTTP 403（权限不足） | `Upload("oss permission denied: bucket missing PutObject grant")` |
| `OssUploader::upload` 网络超时 | `Upload("oss timeout after 30s; check network")` |
| `OssUploader::cleanup` 任意失败 | 仅 `tracing::warn`；返回 Ok（best-effort 语义） |

## 测试策略

### Rust 单元测试

`crates/core/src/account/service.rs`：
- `create` 带 oss → 主 key 与 oss_secret 都进 keyring；params_json 含正确 JSON、不含 secret
- `create` 带 oss、UPDATE 失败 → 两条 keyring 全回滚
- `update` 改 oss → keyring secret 被覆盖；params_json 行更新
- `update` `oss=None` → params_json 与 keyring 不动
- `delete` → 两条 keyring entry 都被尝试 remove；老账号无 oss 时不报错
- `resolve_credentials` → 拼出含 secret 的 extra_json；secret 在原 DB 行不存
- `resolve_credentials` → 没 oss 的账号 extra_json 为 None
- `validate_oss` 各错误分支

`crates/core/src/provider/asset_uploader/oss.rs`：
- 用 `mockito` 起本地 HTTP mock，覆盖：
  - upload 成功 → V4 签名 header 正确（断言 `Authorization` 前缀 `OSS4-HMAC-SHA256`）
  - upload HTTP 401 → 转 `Upload`
  - upload HTTP 403 → 转 `Upload`
  - cleanup 401 → 仅 warn 日志、返回 Ok
- presigned URL 包含必要 query 参数（`x-oss-signature-version=OSS4-HMAC-SHA256`、`x-oss-expires=...`）

`crates/core/src/seed/providers.rs`（既有测试增补）：
- `apply_deletes_kling`：先注入一行 `kling` + 一个 api_account → 调 apply → 验证 provider/api_account/model 全消失
- `apply_creates_bailian_with_two_models`
- `apply_keeps_jimeng_placeholder`

### Rust 集成测试

`crates/core/tests/account_oss_lifecycle.rs`：
- 创建带 OSS 的账号 → resolve_credentials → 校验 extra_json 结构 → 删账号 → 验证两条 keyring 都被清

### 前端测试

`src/components/accounts/AddAccountDialog.test.tsx`（扩既有）：
- 选 bailian → OSS 字段出现
- 选 jimeng → OSS 字段隐藏
- bailian 缺 OSS 字段 → 提交按钮禁用
- 提交时 payload 含 `oss` 子对象；选 jimeng 时 `oss` 为 null
- secret 字段是 password 类型（`type="password"`）

`src/components/accounts/OssFields.test.tsx`：
- 改 endpoint/bucket/AK/secret 字段触发 onChange
- region 留空 → onChange 时 region 为空字符串

### 手动验证清单

- [ ] 启动 `pnpm tauri dev`，看到日志「deleted deprecated provider 'kling'」（如有 kling 数据）
- [ ] 进入设置 / 账号管理，下拉只有 bailian + jimeng
- [ ] 选 bailian 后 OSS 字段展开；填齐后保存
- [ ] 钥匙串（macOS Keychain）出现两条 entry：`api_account:<id>` 和 `api_account:<id>:oss_secret`
- [ ] DB（sqlite3 直查）`SELECT params_json FROM api_account WHERE id=...` 含 oss JSON 但**不含** access_key_secret
- [ ] 删账号，钥匙串两条都消失
- [ ] 创建一个带 OSS 的账号、再编辑（label 改名）、保存——secret 不被清

## 验收标准

- [ ] `004_add_api_account_params_json.sql` 加入迁移；幂等
- [ ] `ApiAccount` / `CreateApiAccountInput` / `UpdateApiAccountInput` 改造完成；TS bindings 重新生成
- [ ] `OssConfigPublic` / `OssConfigInput` 在 TS bindings 出现
- [ ] keyring 可同时持有主 key + oss_secret 两条 entry，互不干扰
- [ ] `AssetUploader` trait + `UploadedAsset` 类型定义在 `provider/asset_uploader/`
- [ ] `OssUploader::from_credentials_json` 可从 `ProviderCredentials.extra_json` 直接构造
- [ ] V4 签名实现通过 mockito 单测
- [ ] `resolve_credentials` 拼装 + secret 注入正确
- [ ] seed 删除 kling、保留 jimeng、新增 bailian + 2 model
- [ ] `AddAccountDialog` 选 bailian 时 OSS 字段动态出现，校验完整
- [ ] `EditAccountDialog` 新增；secret 字段留空表示不改
- [ ] `OssFields` 抽出复用
- [ ] `cargo test --workspace` 全绿；新增 5+ 单测
- [ ] `pnpm test:run` 全绿
- [ ] `pnpm typecheck` 全绿
- [ ] `cargo clippy` 无 warning
- [ ] 文档：在 `docs/技术选型.md` 安全模型小节追加一句「OSS AK secret 与主 API key 同走 keyring，分两条 entry 区分；OSS 端点/bucket/AK ID 入 SQLite `api_account.params_json`」

## 依赖关系

- **前置**：MS1 全部 spec（spec-13 keyring 接口）
- **前置/平行**：[SPEC-15](spec-15-task-engine-skeleton.md) 的 `ProviderCredentials.extra_json` 字段约定本 spec 来填充。spec-15 实现可先 stub，本 spec 完成后回填解析
- **后续**：
  - [SPEC-17](spec-17-bailian-provider.md) 的 BailianProvider 在 happyhorse 任务路径上调 `OssUploader::upload` / `cleanup`
  - [SPEC-19](spec-19-cli-task.md) CLI 需要展示 oss_config 时调用 commands 即可（CLI 不做账号管理 UI）

## 与原任务文档的差异

`开发任务.md` 任务 2「百炼 Provider 实现」中嵌入的「关键阻塞：参考图必须是公网 HTTPS URL」段已在 2026-05-25 决议落地为 OSS 方案；本 spec 把"OSS 上传层 + 账号扩展 + seed 替换"三件事打包，让 spec-17 专注 provider 真正的 HTTP/响应处理逻辑。原任务清单的「开放问题 #2」（kling/jimeng seed 替换/扩充策略）由本 spec 给出最终决定：**主动删 kling、保留 jimeng 占位、新增 bailian**。
