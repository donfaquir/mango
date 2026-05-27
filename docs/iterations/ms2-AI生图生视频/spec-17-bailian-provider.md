# SPEC-17: 百炼 Provider 实现（wan2.7-image-pro + happyhorse-1.0-r2v）

## 概述

实现 `mango_core::provider::bailian::BailianProvider`，对 [SPEC-15](spec-15-task-engine-skeleton.md) 定义的 `ModelProvider` trait 给出真实落地。一个 provider 实例覆盖两个模型，按 `model_id` 内部分支：

- `wan2.7-image-pro`（通义万相 2.7 Pro，文生图）—— **同步调用**：单次 REST 请求即响应，单 task ~30-60s
- `happyhorse-1.0-r2v`（快乐马，参考图生视频）—— **异步任务**：POST submit 拿 `task_id` → GET 轮询 → 完成拿结果 URL

trait 仍只暴露 `submit / poll / cancel / download`：同步模型在 provider 内部用一次性 future + 内存缓存把响应包装成异步外观（详见 §"同步模型轮询包装"）。

本 spec 还实现「任务终态后的结果下载 + Asset 入库」外层包装——`BailianTaskWrapper`（更准确名字 `BailianResultMaterializer`），把 `provider.download()` 衔接到 `<project_root>/assets/{asset_id}.{ext}` + 在 `asset` 表建行 + 关联到 `generation_task.result_asset_id`。这部分 spec-15 的 runner 留出了钩子。

> ⚠️ **依赖说明**：本 spec 假定 [SPEC-15](spec-15-task-engine-skeleton.md) + [SPEC-16](spec-16-oss-uploader-account.md) 已合并。本 spec 仅消费它们暴露的接口（trait / registry / AssetUploader / ProviderCredentials），不再回头改它们的内容。

## 设计目标 / 非目标

**目标**：

- `BailianProvider` 实现 `ModelProvider` trait，按 `model_id` 路由两个模型
- wan2.7 同步包装：内部 `tokio::sync::Mutex<HashMap<String, CachedResult>>`，submit 立即拉远端、缓存 task_id → 结果 URL；poll 命中即返
- happyhorse 异步轮询：调 `/services/aigc/video-generation/video-synthesis` + `X-DashScope-Async: enable`，poll 走 task 状态查询接口
- happyhorse 在 submit 前对每张参考图：调 `AssetUploader::upload(local_path)` 拿到公网 URL → 拼进 `media[]`；任务终态时（runner 通知）调 `cleanup(remote_id)`
- 错误分类与映射：429 限流 / 401 认证 / 余额不足 / 4xx 参数错 / 5xx 远端故障 / 网络超时
- `BailianResultMaterializer`：trait `ResultMaterializer`，BailianProvider 给出实现，把远端结果文件下载并建 Asset
- 在 spec-15 的 runner 集成点：success 分支调 `materializer.materialize(task, external_id)` 写入 `result_asset_id`，并发出 cleanup 信号
- 把 `BailianProvider` 注册到 `ProviderRegistry::builder()` 的 setup 阶段（替换 stub）

**非目标**：

- 取代 spec-15 的状态机或 runner 协程结构——只补 success 分支和 cleanup hook
- 即梦（jimeng）provider 实现——MS4 范围
- 重试策略升级——MS2 不引入指数退避，failed 任务由用户在 UI 重提
- 模型参数的 schema 强校验——provider 只校验"必填 + 类型对"，参数业务有效性（如 `size` 是否在白名单）由 DashScope 后端拒绝；返回错误透传给用户
- happyhorse 的「断点续轮询」——任务运行中应用退出 → spec-15 的 orphan reset 把它改回 pending → 用户重新点 retry → 调用 `submit` 重提（提交新 task，不复用 external_task_id）。MS2 保持简单
- 进度百分比的精确解析——`poll` 返回 `Running { progress: None }`，UI 仅显示「生成中...」

## 技术方案

### 目录/文件结构

```text
crates/core/src/
├── provider/
│   ├── mod.rs                           # +pub mod bailian;
│   └── bailian/
│       ├── mod.rs                       # 重导出 BailianProvider; pub mod 子文件
│       ├── client.rs                    # HTTP 客户端 + 通用请求/重试/错误映射
│       ├── wan27.rs                     # wan2.7-image-pro 同步路径 + cache
│       ├── happyhorse.rs                # happyhorse-1.0-r2v 异步路径 + 参考图上传
│       └── types.rs                     # DashScope 请求/响应反序列化结构
├── task_engine/
│   ├── runner.rs                        # 接入 ResultMaterializer + uploader cleanup hook
│   ├── materializer.rs                  # 新增：ResultMaterializer trait + 默认 NoopMaterializer
│   └── handle.rs                        # spawn 增加 materializer 参数
└── asset/
    └── from_remote.rs                   # 新增：从 URL/字节流建 Asset（不走 import.rs 的扩展名白名单）

src-tauri/src/
└── lib.rs                               # setup 中构造 BailianProvider 注入 registry + materializer

src/components/tasks/
└── (具体面板/表单 由 spec-18 实现，本 spec 不动 UI)
```

### 依赖项变更

**Rust（无新增）**——`reqwest` 0.12+ 已在 MS0；`serde_json`、`tokio` 已在；`async-trait` 已在；OSS uploader 由 spec-16 负责依赖。

**Tauri 插件（无新增）**

**npm（无新增）**

### 数据模型变更

无 migration。`generation_task.params_json` 已有，本 spec 在其中存 provider 私有参数：

**wan2.7 任务的 params_json 形态**：

```json
{
  "n": 1,
  "size": "2K",
  "enable_sequential": false,
  "negative_prompt": ""
}
```

**happyhorse 任务的 params_json 形态**：

```json
{
  "media": [
    {"asset_id": "<asset_uuid>", "type": "reference_image"}
  ],
  "resolution": "720P",
  "ratio": "16:9",
  "duration": 5
}
```

> **设计说明**：`media[].asset_id` 是 mango 内部的 Asset ID（指向本地参考图），不是 DashScope `media[].url`——后者由 happyhorse 子模块在 submit 时用 `AssetUploader::upload` 现转。这样：(a) DB 行不存"会过期的 URL"；(b) 用户重提任务时无需重新选参考图；(c) 任务历史可重现。

### 错误类型扩展

不扩 `CoreError`。spec-15 已有 `Provider(String)` 和 spec-16 已有 `Upload(String)`，本 spec 内部错误用以下私有枚举映射后转 `CoreError::Provider(formatted)`：

```rust
// crates/core/src/provider/bailian/client.rs
#[derive(thiserror::Error, Debug)]
enum BailianError {
    #[error("authentication failed: invalid or expired API key")]
    AuthFailed,
    #[error("rate limited (HTTP 429)")]
    RateLimited,
    #[error("insufficient quota or balance")]
    QuotaExhausted,
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("remote service error (HTTP {status}): {body}")]
    RemoteError { status: u16, body: String },
    #[error("network error: {0}")]
    Network(String),
    #[error("timeout after {secs}s")]
    Timeout { secs: u32 },
    #[error("malformed response: {0}")]
    MalformedResponse(String),
}

impl From<BailianError> for crate::error::CoreError {
    fn from(e: BailianError) -> Self {
        crate::error::CoreError::Provider(e.to_string())
    }
}
```

### BailianProvider 总入口

`crates/core/src/provider/bailian/mod.rs`：

```rust
mod client;
mod happyhorse;
mod types;
mod wan27;

use std::path::Path;
use std::sync::Arc;
use async_trait::async_trait;
use tokio::sync::Mutex;
use std::collections::HashMap;

use crate::error::Result;
use crate::provider::asset_uploader::traits::AssetUploader;
use crate::provider::traits::*;
use client::BailianClient;
use wan27::Wan27Cache;

/// 一个百炼 provider 实例覆盖两个模型；按 model_id 内部分支。
/// 凭证从 ProviderCredentials 中现取——不持久化在结构体里
/// （registry 注册时的 ProviderCredentials 是空壳，运行时每次 submit 注入实际凭证）。
pub struct BailianProvider {
    /// HTTP 客户端，超时 60s，自动 gzip。
    http: reqwest::Client,
    /// wan2.7 的同步结果缓存：task_id → result_url。
    /// 在 submit 时填，poll 时读。entry 在 task 终态后由 runner 主动清掉
    /// （通过 ProviderResources::clear_cache）。
    wan_cache: Arc<Mutex<Wan27Cache>>,
}

impl BailianProvider {
    pub fn new() -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .gzip(true)
            .build()
            .expect("reqwest builder default config never fails");
        Self {
            http,
            wan_cache: Arc::new(Mutex::new(Wan27Cache::default())),
        }
    }
}

#[async_trait]
impl ModelProvider for BailianProvider {
    async fn submit(&self, params: GenerationParams) -> Result<String> {
        let client = BailianClient::new(&self.http, &params.credentials)?;
        match params.model_id.as_str() {
            "wan2.7-image-pro" => wan27::submit(&client, &self.wan_cache, &params).await,
            "happyhorse-1.0-r2v" => happyhorse::submit(&client, &params, self.uploader_for(&params)?).await,
            other => Err(crate::error::CoreError::Provider(
                format!("bailian: unknown model_id '{other}'")
            )),
        }
    }

    async fn poll(&self, external_task_id: &str) -> Result<ProviderTaskStatus> {
        // 用 task_id 前缀路由：wan27 提交时返回 "wan27:<uuid>"; happyhorse 提交时返回 DashScope task id 原值
        if let Some(local) = external_task_id.strip_prefix("wan27:") {
            wan27::poll(&self.wan_cache, local).await
        } else {
            // happyhorse / 未知前缀全部走异步查询接口；上层 runner 调用一定是合法 happyhorse id
            happyhorse::poll(&self.http_with_creds(...)?, external_task_id).await
        }
    }

    async fn cancel(&self, external_task_id: &str) -> Result<()> {
        if external_task_id.starts_with("wan27:") {
            // 同步任务无外部状态可取消——清缓存即可
            self.wan_cache.lock().await.remove(external_task_id);
            Ok(())
        } else {
            // happyhorse: DashScope DELETE /api/v1/tasks/{id}
            happyhorse::cancel(/* ... */).await
        }
    }

    async fn download(&self, external_task_id: &str, dest: &Path) -> Result<std::path::PathBuf> {
        // 1) 解 result_url：wan27 从 cache，happyhorse 从最近一次 poll 的缓存或重新查
        // 2) GET result_url 流式写到 dest
        // 3) 返回 dest（按 Content-Type 决定是否补扩展名）
        unimplemented!()
    }
}
```

> **设计说明**：
> - **凭证不持久化在 provider 实例里**：每次 `submit` 从 `params.credentials` 现取构造 `BailianClient`。这样同一 provider 实例支持多个用户账号（虽然 MS2 范围内只有一个账号活跃，但保留可扩展性）。
> - **task_id 加前缀**：wan27 用 `wan27:<uuid>`，happyhorse 用 DashScope 原值（DashScope task id 是 UUID 格式，不会冲突）。runner 看到的就是这个加前缀的 ID 持久化在 `external_task_id` 列。
> - **AssetUploader 的获取**：从 `params.credentials.extra_json` 解 OSS 配置 → 构造 `OssUploader`。每次 happyhorse submit 都新建 uploader 实例（轻量；reqwest::Client 会内部连接池复用）。

### 同步模型轮询包装（wan27）

`crates/core/src/provider/bailian/wan27.rs`：

```rust
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

use super::client::BailianClient;
use super::types::*;
use crate::error::Result;
use crate::provider::traits::{GenerationParams, ProviderTaskStatus};

#[derive(Default)]
pub(super) struct Wan27Cache {
    entries: HashMap<String, ProviderTaskStatus>,
}

impl Wan27Cache {
    fn put(&mut self, task_id: String, status: ProviderTaskStatus) { self.entries.insert(task_id, status); }
    fn get(&self, task_id: &str) -> Option<ProviderTaskStatus> { self.entries.get(task_id).cloned() }
    pub(super) fn remove(&mut self, task_id: &str) { self.entries.remove(task_id); }
}

pub(super) async fn submit(
    client: &BailianClient,
    cache: &Mutex<Wan27Cache>,
    params: &GenerationParams,
) -> Result<String> {
    let body = build_wan27_body(params)?;
    // chat-messages 风格 endpoint
    let resp: Wan27Response = client
        .post_json("/services/aigc/multimodal-generation/generation", &body)
        .await?;

    let result_url = resp.output.choices.first()
        .and_then(|c| c.message.content.first())
        .and_then(|c| c.image.as_deref())
        .ok_or_else(|| BailianError::MalformedResponse(
            "wan27 response missing output.choices[0].message.content[0].image".into()
        ))?;

    let task_id = format!("wan27:{}", Uuid::new_v4());
    cache.lock().await.put(
        task_id.clone(),
        ProviderTaskStatus::Success { result_url: result_url.to_string() },
    );
    Ok(task_id)
}

pub(super) async fn poll(
    cache: &Mutex<Wan27Cache>,
    task_id: &str,
) -> Result<ProviderTaskStatus> {
    cache.lock().await.get(&format!("wan27:{task_id}"))
        .ok_or_else(|| crate::error::CoreError::TaskEngine(
            format!("wan27 cache miss for {task_id}; was the task already cleared?")
        ))
}

fn build_wan27_body(params: &GenerationParams) -> Result<serde_json::Value> {
    // 解 params.provider_params 取 size / n / enable_sequential / negative_prompt
    Ok(serde_json::json!({
        "model": "wan2.7-image-pro",
        "input": {
            "messages": [{
                "role": "user",
                "content": [{ "text": params.prompt }]
            }]
        },
        "parameters": {
            "n": params.provider_params.get("n").and_then(|v| v.as_u64()).unwrap_or(1),
            "size": params.provider_params.get("size").and_then(|v| v.as_str()).unwrap_or("2K"),
            "enable_sequential": params.provider_params.get("enable_sequential")
                .and_then(|v| v.as_bool()).unwrap_or(false),
            "negative_prompt": params.provider_params.get("negative_prompt").and_then(|v| v.as_str()).unwrap_or("")
        }
    }))
}
```

> **设计说明**：
> - **缓存 entry 不带过期**：DashScope 给的 result_url 通常 24h 有效，runner 在 success 后立即调 download，几秒到一分钟内必定走完。entry 在 success 后由 runner 通过 cancel hook 主动清。退化情况（runner 崩溃 → 缓存条目泄漏直到进程退出）可接受——HashMap 一条 entry 几百字节，不构成内存压力。
> - **submit 失败不写缓存**：让 runner 重试时能再次拿到错误（虽然 MS2 不自动重试）。
> - **同步模型为何要 task_id**：与 trait 一致是关键——`generation_task.external_task_id` 必须有值才能让 cancel/download 路由回到 wan27 分支。前缀 `wan27:` 仅用于路由不暴露给用户。

### 异步模型路径（happyhorse）

`crates/core/src/provider/bailian/happyhorse.rs`：

```rust
use std::sync::Arc;
use crate::error::Result;
use crate::provider::asset_uploader::traits::{AssetUploader, UploadedAsset};
use crate::provider::traits::*;
use super::client::BailianClient;
use super::types::*;

/// 提交：先把每张本地参考图上传到 OSS 拿公网 URL，再 POST submit。
pub(super) async fn submit(
    client: &BailianClient,
    params: &GenerationParams,
    uploader: Arc<dyn AssetUploader>,
) -> Result<String> {
    // 1) 解 params.provider_params.media[]，每个 entry { asset_id, type }
    let media_entries: Vec<MediaEntry> = serde_json::from_value(
        params.provider_params.get("media").cloned()
            .ok_or_else(|| crate::error::CoreError::Validation(
                "happyhorse params missing 'media[]'".into()))?,
    ).map_err(|e| crate::error::CoreError::Validation(format!("media[] schema: {e}")))?;

    if media_entries.is_empty() {
        return Err(crate::error::CoreError::Validation(
            "happyhorse requires at least one reference image".into()));
    }

    // 2) 并行上传所有参考图（Vec<UploadedAsset>）
    //    asset_id → 本地路径解析由 caller 完成（spec-15 runner 在调 submit 前
    //    把 asset_id 解为本地绝对路径塞 provider_params；详见集成点）
    let uploads: Vec<UploadedAsset> = futures::future::try_join_all(
        media_entries.iter().map(|m| {
            let path = std::path::PathBuf::from(&m.local_path);
            let up = uploader.clone();
            async move { up.upload(&path).await }
        })
    ).await?;

    // 3) 构造请求体
    let body = build_happyhorse_body(params, &uploads)?;
    let task_resp: AsyncSubmitResponse = client
        .post_json_async("/services/aigc/video-generation/video-synthesis", &body)
        .await?;

    // 4) 把 OSS remote_id 列表挂到 task 元数据：task_engine 在终态时回调 cleanup
    //    具体方式：写到 generation_task.params_json 的 _internal.uploaded_remote_ids 子树
    //    runner 终态 hook 读这个列表逐个 cleanup
    persist_uploaded_remote_ids(/* ... */)?;

    Ok(task_resp.output.task_id)
}

pub(super) async fn poll(
    client: &BailianClient,
    external_task_id: &str,
) -> Result<ProviderTaskStatus> {
    let resp: AsyncPollResponse = client
        .get_json(&format!("/tasks/{external_task_id}"))
        .await?;

    Ok(match resp.output.task_status.as_str() {
        "PENDING" | "QUEUED" => ProviderTaskStatus::Pending,
        "RUNNING" => ProviderTaskStatus::Running { progress: None },
        "SUCCEEDED" => ProviderTaskStatus::Success {
            result_url: resp.output.video_url.unwrap_or_default(),
        },
        "FAILED" | "UNKNOWN" => ProviderTaskStatus::Failed {
            message: resp.output.message.unwrap_or_else(|| "task failed".into()),
        },
        other => ProviderTaskStatus::Failed {
            message: format!("unknown DashScope task_status '{other}'"),
        },
    })
}

pub(super) async fn cancel(
    client: &BailianClient,
    external_task_id: &str,
) -> Result<()> {
    // DashScope DELETE /api/v1/tasks/{id} —— 实际接口名待 SDK 文档复核。
    let _ = client.delete(&format!("/tasks/{external_task_id}")).await;
    Ok(()) // best-effort
}

fn build_happyhorse_body(
    params: &GenerationParams,
    uploads: &[UploadedAsset],
) -> Result<serde_json::Value> {
    Ok(serde_json::json!({
        "model": "happyhorse-1.0-r2v",
        "input": {
            "prompt": params.prompt,
            "media": uploads.iter().map(|u| serde_json::json!({
                "type": "reference_image",
                "url": u.url,
            })).collect::<Vec<_>>()
        },
        "parameters": {
            "resolution": params.provider_params.get("resolution").and_then(|v| v.as_str()).unwrap_or("720P"),
            "ratio": params.provider_params.get("ratio").and_then(|v| v.as_str()).unwrap_or("16:9"),
            "duration": params.provider_params.get("duration").and_then(|v| v.as_u64()).unwrap_or(5),
        }
    }))
}
```

> **设计说明**：
> - **参考图上传放在 submit 内**：让 trait 保持 4 方法不变。`uploader` 由 `BailianProvider::submit` 从 `credentials.extra_json` 现造后注入。
> - **uploaded_remote_ids 持久化到 DB**：app 崩溃后重启，runner 看到 task 已 success 但参考图还在 OSS，下次 cleanup 时还能读到列表逐个删。entry 在 cleanup 完成后清。**这是一个关键的 fail-safe**——OSS 对象积压不能依赖内存状态。
> - **cancel 是 best-effort**：DashScope task 可能已进入生成阶段，cancel 不一定能阻止扣费。UI 文案要明示「已请求取消，但若任务已计费则无法退还」。

### HTTP 客户端

`crates/core/src/provider/bailian/client.rs`：

```rust
use serde::{de::DeserializeOwned, Serialize};
use crate::error::{CoreError, Result};
use crate::provider::traits::ProviderCredentials;
use super::BailianError;

const DEFAULT_BASE_URL: &str = "https://dashscope.aliyuncs.com/api/v1";

pub(super) struct BailianClient<'a> {
    http: &'a reqwest::Client,
    base_url: String,
    api_key: String,
}

impl<'a> BailianClient<'a> {
    pub fn new(http: &'a reqwest::Client, creds: &ProviderCredentials) -> Result<Self> {
        if creds.api_key.is_empty() {
            return Err(CoreError::Provider("bailian api_key empty".into()));
        }
        Ok(Self {
            http,
            base_url: DEFAULT_BASE_URL.into(),
            api_key: creds.api_key.clone(),
        })
    }

    pub async fn post_json<T: Serialize, R: DeserializeOwned>(
        &self, path: &str, body: &T,
    ) -> Result<R> {
        self.send(self.http.post(format!("{}{path}", self.base_url)).json(body), false).await
    }

    pub async fn post_json_async<T: Serialize, R: DeserializeOwned>(
        &self, path: &str, body: &T,
    ) -> Result<R> {
        self.send(self.http.post(format!("{}{path}", self.base_url))
            .json(body)
            .header("X-DashScope-Async", "enable"), false).await
    }

    pub async fn get_json<R: DeserializeOwned>(&self, path: &str) -> Result<R> {
        self.send(self.http.get(format!("{}{path}", self.base_url)), true).await
    }

    pub async fn delete(&self, path: &str) -> Result<()> {
        let _: serde_json::Value = self.send(
            self.http.delete(format!("{}{path}", self.base_url)),
            true,
        ).await?;
        Ok(())
    }

    async fn send<R: DeserializeOwned>(
        &self, req: reqwest::RequestBuilder, allow_empty: bool,
    ) -> Result<R> {
        let resp = req
            .bearer_auth(&self.api_key)
            .send()
            .await
            .map_err(|e| if e.is_timeout() {
                BailianError::Timeout { secs: 60 }
            } else {
                BailianError::Network(e.to_string())
            })?;

        let status = resp.status().as_u16();
        let body = resp.text().await.unwrap_or_default();

        match status {
            200..=299 => {
                if allow_empty && body.is_empty() {
                    serde_json::from_str("null").map_err(|e| {
                        BailianError::MalformedResponse(e.to_string()).into()
                    })
                } else {
                    serde_json::from_str(&body).map_err(|e| {
                        BailianError::MalformedResponse(format!("{e}; body: {body}")).into()
                    })
                }
            }
            401 => Err(BailianError::AuthFailed.into()),
            403 => {
                if body.contains("InsufficientBalance") || body.contains("Quota") {
                    Err(BailianError::QuotaExhausted.into())
                } else {
                    Err(BailianError::AuthFailed.into())
                }
            }
            429 => Err(BailianError::RateLimited.into()),
            400..=499 => Err(BailianError::InvalidRequest(body).into()),
            500..=599 => Err(BailianError::RemoteError { status, body }.into()),
            _ => Err(BailianError::RemoteError { status, body }.into()),
        }
    }
}
```

> **设计说明**：
> - **不内置重试**：429 / 5xx 直接报错让用户看到。spec-15 不自动重试，UI 提供「重新提交」按钮（spec-18）。MS3+ 视实际限流情况再加退避。
> - **base_url 固定不读 DB**：`provider.base_url` 列在 spec-16 已 seed 为 DashScope URL，但 BailianClient 内部硬编码默认值——这样测试时不需要先 seed 数据库。需要环境变量覆盖（如指 staging）时由调用方在 `extra_json.bailian_base_url` 注入；MS2 不实现。
> - **错误体里包 body**：让用户看到 DashScope 的具体错误描述（如 `Code: InvalidParameter, Message: ...`）。但要注意 body 可能含敏感 header echo——先 trim 到 1KB。

### 类型定义

`crates/core/src/provider/bailian/types.rs`：

```rust
use serde::{Deserialize, Serialize};

// wan27 chat-messages 响应
#[derive(Debug, Deserialize)]
pub(super) struct Wan27Response {
    pub output: Wan27Output,
    #[serde(default)]
    pub usage: Option<serde_json::Value>,
}
#[derive(Debug, Deserialize)]
pub(super) struct Wan27Output { pub choices: Vec<Wan27Choice> }
#[derive(Debug, Deserialize)]
pub(super) struct Wan27Choice { pub message: Wan27Message }
#[derive(Debug, Deserialize)]
pub(super) struct Wan27Message { pub content: Vec<Wan27Content> }
#[derive(Debug, Deserialize)]
pub(super) struct Wan27Content { pub image: Option<String> }

// happyhorse async submit
#[derive(Debug, Deserialize)]
pub(super) struct AsyncSubmitResponse { pub output: AsyncSubmitOutput }
#[derive(Debug, Deserialize)]
pub(super) struct AsyncSubmitOutput { pub task_id: String }

// happyhorse async poll
#[derive(Debug, Deserialize)]
pub(super) struct AsyncPollResponse { pub output: AsyncPollOutput }
#[derive(Debug, Deserialize)]
pub(super) struct AsyncPollOutput {
    pub task_status: String,
    pub video_url: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct MediaEntry {
    pub asset_id: String,
    /// 由 spec-15 runner 在 submit 前写入：本地绝对路径。不持久化进 DB。
    #[serde(default, skip_serializing)]
    pub local_path: String,
    #[serde(rename = "type")]
    pub media_type: String,
}
```

### ResultMaterializer 抽象

spec-15 的 runner 在 success 分支只置 status；具体「下载结果 + 入库 + cleanup uploader」由 `ResultMaterializer` 完成。本 spec 给出 trait 与默认实现。

`crates/core/src/task_engine/materializer.rs`：

```rust
use async_trait::async_trait;
use crate::error::Result;
use crate::models::generation_task::GenerationTask;

#[async_trait]
pub trait ResultMaterializer: Send + Sync {
    /// 任务进入 success 时被 runner 调用。返回 result_asset_id（写到 generation_task）。
    async fn materialize(&self, task: &GenerationTask, result_url: &str) -> Result<String>;

    /// 任务终态后调用，让 provider/uploader 清理远端临时资源。
    /// best-effort——失败仅 warn。
    async fn cleanup(&self, task: &GenerationTask) -> Result<()> { Ok(()) }
}
```

`crates/core/src/provider/bailian/materializer.rs`（新文件，与 provider 同 crate）：

```rust
use std::sync::Arc;
use async_trait::async_trait;
use uuid::Uuid;

use crate::asset::from_remote::download_to_asset;
use crate::error::{CoreError, Result};
use crate::models::generation_task::{GenerationTask, TaskKind};
use crate::provider::asset_uploader::traits::AssetUploader;
use crate::task_engine::materializer::ResultMaterializer;

pub struct BailianResultMaterializer {
    db: tokio_rusqlite::Connection,
    keyring: Arc<dyn crate::account::keyring::KeyringStore>,
}

#[async_trait]
impl ResultMaterializer for BailianResultMaterializer {
    async fn materialize(&self, task: &GenerationTask, result_url: &str) -> Result<String> {
        // 1) 拿 project_id（task.shot_id → episode → project，或者从 task 直接关联……
        //    实际：generation_task 表当前没有 project_id 列，shot_id 也可能 null
        //    本 spec 决定：MS2 范围内任务必须关联 shot；spec-18 提交表单强制选 shot。
        //    若 task.shot_id 为 None → Validation 错误
        let project_id = resolve_project_id(&self.db, task).await?;
        let asset_type = match task.task_type {
            TaskKind::Image => "image",
            TaskKind::Video => "video",
            _ => return Err(CoreError::Provider("bailian: only image/video supported".into())),
        };
        let asset = download_to_asset(&self.db, &project_id, task.shot_id.as_deref(),
            result_url, asset_type).await?;
        Ok(asset.id)
    }

    async fn cleanup(&self, task: &GenerationTask) -> Result<()> {
        // 1) 解 task.params_json._internal.uploaded_remote_ids
        // 2) 从 task.account_id 解 OSS credentials → 构 OssUploader
        // 3) 逐个调 cleanup（best-effort）
        // 4) 清空 _internal.uploaded_remote_ids 子树
        Ok(())
    }
}

async fn resolve_project_id(db: &tokio_rusqlite::Connection, task: &GenerationTask) -> Result<String> {
    let shot_id = task.shot_id.clone()
        .ok_or_else(|| CoreError::Validation(
            "task without shot_id cannot persist its result".into()))?;
    let pid: String = db.call(move |conn| {
        Ok::<_, rusqlite::Error>(conn.query_row(
            "SELECT episode.project_id FROM shot \
             JOIN episode ON shot.episode_id = episode.id WHERE shot.id = ?1",
            rusqlite::params![shot_id],
            |r| r.get(0),
        )?)
    }).await.map_err(|e| CoreError::Sqlite(rusqlite::Error::ExecuteReturnedResults))?
        .map_err(CoreError::Sqlite)?;
    // ... 简化：实际用 utils 包好
    Ok(pid)
}
```

### asset::from_remote

`crates/core/src/asset/from_remote.rs`：

```rust
use std::path::PathBuf;
use uuid::Uuid;

use crate::asset::thumbnail;
use crate::db::queries::asset as queries;
use crate::error::{CoreError, Result};
use crate::models::asset::Asset;
use crate::paths;

/// 从远端 URL 下载并落 Asset 行。区别于 asset::import：
/// - 不做扩展名白名单（视频结果 mp4 不在 import 的允许列表）
/// - 不做 50 MiB 上限校验
/// - 文件名扩展名从 Content-Type 推断
pub async fn download_to_asset(
    db: &tokio_rusqlite::Connection,
    project_id: &str,
    shot_id: Option<&str>,
    url: &str,
    asset_type: &str,    // 'image' | 'video' | 'audio'
) -> Result<Asset> {
    let project_id_owned = project_id.to_string();
    let project_root: PathBuf = db.call(move |conn| {
        Ok::<_, rusqlite::Error>(conn.query_row(
            "SELECT root_path FROM project WHERE id = ?1",
            rusqlite::params![project_id_owned],
            |r| r.get::<_, String>(0).map(PathBuf::from),
        )?)
    }).await.expect("db call").map_err(CoreError::Sqlite)?;

    paths::ensure_project_layout(&project_root)?;

    // 1) HTTP GET + 流式写盘
    let resp = reqwest::get(url).await
        .map_err(|e| CoreError::Provider(format!("download failed: {e}")))?;
    if !resp.status().is_success() {
        return Err(CoreError::Provider(format!(
            "download HTTP {}: result url may have expired", resp.status())));
    }
    let ext = ext_from_content_type(resp.headers(), asset_type);
    let asset_id = Uuid::new_v4().to_string();
    let file_name = format!("{asset_id}.{ext}");
    let dest = paths::assets_dir(&project_root).join(&file_name);
    let bytes = resp.bytes().await
        .map_err(|e| CoreError::Provider(format!("download body: {e}")))?;
    std::fs::write(&dest, &bytes)?;

    // 2) 缩略图（仅 image）
    let thumb_relative = if asset_type == "image" {
        let thumb_name = format!("{asset_id}_thumb.webp");
        let thumb_dest = paths::thumbnails_dir(&project_root).join(&thumb_name);
        match thumbnail::generate_with_metadata(&dest, &thumb_dest) {
            Ok(_) => Some(format!("{}/{}", paths::THUMBNAILS_SUBDIR, thumb_name)),
            Err(e) => { tracing::warn!("thumbnail for ai result failed: {e}"); None }
        }
    } else {
        None
    };

    // 3) INSERT asset 行
    // ... (调用 queries::insert_remote_origin 或扩 import.rs 的 persist_artifacts)
    unimplemented!()
}

fn ext_from_content_type(headers: &reqwest::header::HeaderMap, asset_type: &str) -> &'static str {
    let ct = headers.get("content-type").and_then(|v| v.to_str().ok()).unwrap_or("");
    match (asset_type, ct) {
        ("image", t) if t.contains("png") => "png",
        ("image", t) if t.contains("webp") => "webp",
        ("image", _) => "jpg",
        ("video", t) if t.contains("webm") => "webm",
        ("video", _) => "mp4",
        ("audio", t) if t.contains("wav") => "wav",
        ("audio", _) => "mp3",
        _ => "bin",
    }
}
```

### 集成到 spec-15 runner

`crates/core/src/task_engine/handle.rs` `spawn` 增加 materializer 参数：

```rust
pub fn spawn(
    db: Connection,
    providers: ProviderRegistry,
    materializer: Arc<dyn ResultMaterializer>,
    max_concurrency: usize,
) -> (Self, mpsc::UnboundedReceiver<TaskEvent>) { /* ... */ }
```

`runner.rs` success 分支：

```rust
Ok(ProviderTaskStatus::Success { result_url }) => {
    // 1) 把 result_url 持久化到 generation_task（用于 cancel 后还能复查）
    // 2) 调 materializer.materialize → 拿 result_asset_id
    // 3) UPDATE generation_task SET result_asset_id = ?, status='success', finished_at=...
    // 4) emit Success event
    // 5) 调 materializer.cleanup（best-effort）
    let asset_id = match engine.materializer.materialize(&task, &result_url).await {
        Ok(id) => id,
        Err(e) => return state::fail(&engine, &task_id, &e.to_string()).await,
    };
    state::succeed(&engine, &task_id, &result_url, Some(&asset_id)).await;

    // cleanup 是 best-effort 后台执行，不阻塞 runner 退出
    let m = engine.materializer.clone();
    let t = task.clone();
    tokio::spawn(async move {
        if let Err(e) = m.cleanup(&t).await {
            tracing::warn!("materializer cleanup for task {} failed: {}", t.id, e);
        }
    });
    return;
}
```

### Tauri setup 改造

`src-tauri/src/lib.rs` setup 中：

```rust
use mango_core::provider::bailian::BailianProvider;
use mango_core::provider::bailian::materializer::BailianResultMaterializer;

let providers = mango_core::provider::registry::ProviderRegistry::builder()
    .register("bailian", Arc::new(BailianProvider::new()))
    .build();
let materializer = Arc::new(BailianResultMaterializer::new(db.clone(), keyring.clone()));
let (engine, mut event_rx) = mango_core::task_engine::TaskEngineHandle::spawn(
    db.clone(), providers, materializer, 4
);
```

### 错误场景

| 场景 | 行为 |
|---|---|
| account 没配 OSS、提交 happyhorse 任务 | submit 报 `Validation("happyhorse requires oss credentials")`；任务直接 failed |
| 参考图本地文件不存在 | uploader.upload 报 `Io`；任务 failed |
| OSS 上传成功、DashScope submit 失败 | 立即调 uploader.cleanup 已上传部分；返回 `Provider` 错误 |
| 下载 result_url 时已过期 | `download_to_asset` 报 `Provider("HTTP 403: ...")`；runner 把任务转 failed；cleanup 仍执行 |
| 下载成功但 metadata_json 解码失败（视频元数据缺失） | 不挂阻，metadata 写 None，任务正常 success |
| materialize 写 asset 行违反 FK（shot_id 已删） | `Sqlite`；runner 转 failed |
| 任务在 cancel 中 / 已 cancelled | provider.poll 仍可能返回 SUCCESS，但 runner 跳过 materialize |
| 同一参考图反复提交 | OSS object key 含 sha256；同内容覆盖即 OK，无负担 |
| wan27 cache miss（重启后） | spec-15 orphan reset 已把 task 转 pending；用户在 UI 手动重提 → 走新流程 |

## 测试策略

### Rust 单元测试

`crates/core/src/provider/bailian/client.rs`（mockito）：
- 200 + 合法 JSON → 反序列化成功
- 200 + 空 body + allow_empty=false → MalformedResponse
- 401 → AuthFailed
- 403 + body 含 "InsufficientBalance" → QuotaExhausted
- 403 + body 不含 → AuthFailed
- 429 → RateLimited
- 500 → RemoteError
- 网络断 → Network

`crates/core/src/provider/bailian/wan27.rs`（mockito）：
- submit 成功 → cache 有 entry → poll 返回 Success
- submit 远端返回 4xx → Provider 错；cache 无 entry
- 缓存 miss poll → TaskEngine 错

`crates/core/src/provider/bailian/happyhorse.rs`（mockito + 自定义 fake AssetUploader）：
- submit 成功 → 触发上传 N 张参考图 → DashScope 收到 media[].url 数组
- submit 远端 4xx → 调用 uploader.cleanup 回滚已上传
- poll PENDING/RUNNING/SUCCEEDED/FAILED 各走对应分支
- cancel 远端 5xx → 仍返回 Ok（best-effort）

`crates/core/src/asset/from_remote.rs`（mockito）：
- 下载 png → 文件落盘 + 缩略图生成 + asset 行入库
- 下载 mp4 → 文件落盘 + 缩略图为 None
- 下载 403 → Provider 错；无文件残留

### Rust 集成测试

`crates/core/tests/bailian_e2e.rs`（mockito + 内存 DB + InMemoryKeyring + InMemoryUploader）：
- 端到端：seed providers → 创 project + episode + shot + account + asset → submit 一个 wan27 任务 → 等 success → 验证 result_asset_id 写入
- 端到端：同上但 happyhorse → 验证参考图被上传 / 下载 / cleanup 都被调

### 手动验证清单

- [ ] 启动 dev 模式，DB 已 seed bailian + 2 model
- [ ] 配置一个真实百炼账号 + OSS（用真实测试账号；测试 bucket 可在阿里云控制台建 mango-test-refs）
- [ ] 在前端调出任务提交 IPC（spec-18 实现 UI 前可用 DevTools console），提交一个 wan27 任务，prompt: "教室阳光照在书桌上的少年"
- [ ] 任务 30s 内变 success；assets/ 目录有新图；任务面板显示缩略图
- [ ] 提交一个 happyhorse 任务（绑定带参考图的 character/scene）
- [ ] 在 OSS 控制台看到上传的对象；任务 success 后对象消失
- [ ] 应用退出 → DashScope 控制台看到 task 还在 / 已完成；重启应用 → orphan reset 把任务转 pending；点重试 → 新建一个 task

## 验收标准

- [ ] `BailianProvider` 实现 `ModelProvider` trait 全部 4 方法
- [ ] wan2.7-image-pro 同步包装通过 mockito 测试
- [ ] happyhorse-1.0-r2v 异步轮询通过 mockito 测试
- [ ] 参考图自动上传 + 任务终态自动清理 OSS
- [ ] `ResultMaterializer` trait 在 `task_engine::materializer` 定义；spec-15 runner 接入
- [ ] `BailianResultMaterializer` 实现 + 注册到 setup
- [ ] `asset::from_remote::download_to_asset` 处理 image / video 两种 asset_type；扩展名按 Content-Type 推断
- [ ] `cargo test --workspace` 全绿；新增至少 15 单测
- [ ] `cargo clippy` 无 warning
- [ ] `pnpm typecheck` 全绿
- [ ] `cargo test -p mango-tauri export_bindings` 重新生成 commands.ts diff 干净
- [ ] 文档：在 `docs/技术选型.md` provider 子节追加「BailianProvider 路由策略 + result url 24h 过期约束」一句

## 依赖关系

- **前置**：[SPEC-15](spec-15-task-engine-skeleton.md)（trait + runner + materializer 接口）、[SPEC-16](spec-16-oss-uploader-account.md)（OssUploader 实现 + 账号 OSS 配置 + bailian seed）
- **后续**：
  - [SPEC-18](spec-18-task-ui.md) 任务 UI 消费本 spec 暴露的 commands；提交表单按 model_id 决定渲染哪些字段（wan27 显示 size/n、happyhorse 显示 resolution/ratio/duration + 主体选择器）
  - [SPEC-19](spec-19-cli-task.md) CLI 直接调 task_engine API，不感知 provider 内部分支

## 与原任务文档的差异

`开发任务.md` 任务 2（百炼 Provider 实现）+ 任务 4（结果下载 + 自动创建 Asset）+ 任务 10（基本错误处理）中关于 provider 层错误的部分整合为本 spec。原任务 4 在 ms1-asset import 既有的 `import.rs` 上扩展不再可行（视频 mp4 不在白名单 + 50MiB 上限；从 URL 下载与从本地拖入的字段集不同），本 spec 新建 `asset::from_remote` 模块解耦。原任务清单的「开放问题 #1」（wan27 同步调用是否引入异常路径）由本 spec 通过「内存 cache + task_id 前缀路由」给出确定方案——cache 在终态后由 cancel 主动清，进程崩溃后的泄漏由 spec-15 orphan reset 兜底。
