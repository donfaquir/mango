# SPEC-37: TTS Provider 接入（百炼 CosyVoice）

> 对应 MS6 原任务 1+2（AI 配音 Provider 接入 + 选型）。在现有 BailianProvider 中扩展 TTS 模型路由，接入 DashScope CosyVoice 语音合成服务。

依赖：MS2 spec-15（task engine）、spec-16（account/credential）

非目标：
- 新建独立 Provider 模块 — 复用现有百炼 Provider，加路由分支
- 语音克隆 / 自定义音色训练
- 流式音频输出（实时播放未完成的生成）
- 接入 MiniMax / 即梦等外部服务（统一走百炼）
- 情感/风格控制（speed 以外的参数为 P2）

---

## 1. 选型决策

### 1.1 约束

所有外部 API 统一使用百炼（DashScope）服务，不接入其他厂商。

### 1.2 百炼 TTS 能力

DashScope 提供的 TTS 模型：

| 模型 | 特点 | API 模式 | 说明 |
|------|------|---------|------|
| **CosyVoice 2.0** | 高质量、多音色、支持情感 | 同步/流式 | 首选 |
| Sambert 系列 | 老一代、音色固定 | 同步 | 备选 |

### 1.3 结论

**使用 CosyVoice 2.0（cosyvoice-v2）**：
- 已在百炼平台上线，与现有 wan2.7 / happyhorse 同一个 API key
- 同步调用模式（类似 wan2.7），简单可靠
- 中文语音质量好，30+ 预设音色覆盖漫剧场景
- 无需额外开通服务或配置新账号

---

## 2. 架构设计

### 2.1 扩展现有 BailianProvider

当前 `BailianProvider` 按 `model_id` 路由：
- `"wan2.7-image-pro"` → 同步文生图
- `"happyhorse-1.0-r2v"` → 异步参考图生视频

新增：
- `"cosyvoice-v2"` → 同步文本转语音

模式与 wan2.7 完全一致：`submit` 同步拿到音频 → 缓存 → `poll` 立即返回 Success。

### 2.2 参数映射

TTS 特有参数通过 `GenerationParams.provider_params: Value` 传递：

```json
{
  "voice_id": "longxiaochun",
  "text": "这是需要合成的文本...",
  "speed": 1.0,
  "volume": 50,
  "pitch": 0,
  "sample_rate": 24000,
  "format": "mp3"
}
```

TaskKind = `Audio`，AssetType = `Audio`（均已存在于 enum 中）。

### 2.3 复用现有基础设施

| 组件 | 复用情况 |
|------|---------|
| `BailianClient` | 直接复用（同 API key / auth header 格式） |
| `Wan27Cache` | TTS 也需要同步缓存，复用同一数据结构或创建类似的 `TtsCache` |
| `validate.rs` | 追加 cosyvoice 参数验证分支 |
| Provider/Model seed | 追加 cosyvoice-v2 model |
| Account | 复用现有百炼账号（同一个 api_key） |

---

## 3. 实现细节

### 3.1 新增文件

```
crates/core/src/provider/bailian/
├── mod.rs           # 追加 cosyvoice 路由分支
├── cosyvoice.rs     # NEW: CosyVoice TTS 逻辑
├── client.rs        # 无需修改（已有通用 HTTP 方法）
├── types.rs         # 追加 TTS 请求/响应类型
├── validate.rs      # 追加 cosyvoice 参数验证
├── wan27.rs         # 不动
└── happyhorse.rs    # 不动
```

### 3.2 cosyvoice.rs

```rust
// crates/core/src/provider/bailian/cosyvoice.rs

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

use super::client::BailianClient;
use super::types::{CosyVoiceRequest, CosyVoiceResponse};
use crate::provider::error::ProviderErrorDetail;
use crate::provider::traits::{GenerationParams, ProviderTaskStatus};

/// In-memory cache for synchronous TTS results (same pattern as Wan27Cache)
pub type TtsCache = HashMap<String, CachedAudio>;

pub struct CachedAudio {
    pub data: Vec<u8>,
    pub format: String,  // "mp3" | "wav"
}

pub struct TtsSubmitResult {
    pub external_task_id: String,
    pub request_id: Option<String>,
    pub http_status: u16,
}

/// Submit TTS request synchronously, cache the audio result
pub async fn submit(
    client: &BailianClient,
    cache: &Arc<Mutex<TtsCache>>,
    params: &GenerationParams,
) -> std::result::Result<TtsSubmitResult, ProviderErrorDetail>;

/// Poll: if cached → Success; if not → Failed
pub async fn poll(
    cache: &Arc<Mutex<TtsCache>>,
    external_task_id: &str,
) -> std::result::Result<ProviderTaskStatus, ProviderErrorDetail>;

/// Cancel: remove from cache
pub async fn cancel(cache: &Arc<Mutex<TtsCache>>, external_task_id: &str);

/// Download: write cached bytes to dest path
pub async fn download(
    cache: &Arc<Mutex<TtsCache>>,
    external_task_id: &str,
    dest: &std::path::Path,
) -> std::result::Result<std::path::PathBuf, ProviderErrorDetail>;
```

### 3.3 DashScope TTS API

DashScope CosyVoice 接口：

```
POST https://dashscope.aliyuncs.com/api/v1/services/aigc/text2audio/generation
Authorization: Bearer {api_key}
Content-Type: application/json

{
  "model": "cosyvoice-v2",
  "input": {
    "text": "要合成的文本"
  },
  "parameters": {
    "voice": "longxiaochun",
    "format": "mp3",
    "sample_rate": 24000,
    "rate": 1.0,
    "volume": 50,
    "pitch": 0
  }
}
```

响应：
- 成功时 `Content-Type: audio/mpeg` 直接返回音频二进制
- 失败时 `Content-Type: application/json` 返回错误 JSON

### 3.4 mod.rs 路由追加

```rust
// 在 BailianProvider::submit 的 match 中追加：
"cosyvoice-v2" => {
    let result = cosyvoice::submit(&client, &self.tts_cache, &params).await?;
    Ok(SubmitOutcome {
        external_task_id: result.external_task_id,
        request_id: result.request_id,
        http_status: Some(result.http_status as i64),
        upload: None,
    })
}
```

```rust
// 在 BailianProvider::poll 中追加前缀判断：
if external_task_id.starts_with("cosyvoice:") {
    let status = cosyvoice::poll(&self.tts_cache, external_task_id).await?;
    return Ok(PollOutcome::bare(status));
}
```

### 3.5 BailianProvider struct 变更

```rust
pub struct BailianProvider {
    http: reqwest::Client,
    wan_cache: Arc<Mutex<Wan27Cache>>,
    tts_cache: Arc<Mutex<cosyvoice::TtsCache>>,  // 新增
    creds_cache: Arc<Mutex<HashMap<String, String>>>,
}
```

---

## 4. 参数验证

```rust
// crates/core/src/provider/bailian/validate.rs — 追加

fn validate_cosyvoice_params(params: &Value) -> Result<(), ProviderErrorDetail> {
    // text: 必填，非空，max 2000 chars（CosyVoice 单次上限）
    // voice_id: 必填，非空字符串
    // speed/rate: 可选，0.5 - 2.0
    // volume: 可选，0 - 100
    // pitch: 可选，-500 - 500
    // format: 可选，"mp3" | "wav" | "pcm"
    // sample_rate: 可选，8000 | 16000 | 22050 | 24000 | 44100 | 48000
}
```

---

## 5. Seed 数据

```rust
// crates/core/src/seed/providers.rs — 追加 model

Model {
    id: "cosyvoice-v2",
    provider_id: "bailian",       // 复用现有 bailian provider
    name: "CosyVoice 2.0",
    model_type: "audio",
    capabilities_json: r#"{
        "task_types": ["audio"],
        "sync_submit": true,
        "max_text_length": 2000,
        "output_format": "mp3",
        "supported_voices": true
    }"#,
    default_params_json: r#"{
        "voice_id": "longxiaochun",
        "speed": 1.0,
        "volume": 50,
        "pitch": 0,
        "sample_rate": 24000,
        "format": "mp3"
    }"#,
}
```

无需新 provider seed，百炼 provider 已存在。无需新建账号，复用用户现有百炼 api_key。

---

## 6. 预设音色列表

存入 `capabilities_json.voices` 或独立 seed 常量：

| voice_id | 名称 | 性别 | 风格 |
|----------|------|------|------|
| `longxiaochun` | 龙小淳 | 女 | 温柔知性 |
| `longlaotie` | 龙老铁 | 男 | 东北话 |
| `longshu` | 龙叔 | 男 | 沉稳旁白 |
| `longxiaoxia` | 龙小夏 | 女 | 活泼少女 |
| `longyue` | 龙悦 | 女 | 温柔甜美 |
| `longfei` | 龙飞 | 男 | 激昂解说 |
| `longjielidou` | 龙杰力豆 | 男 | 少年音 |
| `longxiaobai` | 龙小白 | 女 | 客服播报 |

完整音色列表在实施时从百炼文档获取最新版本。

---

## 7. 错误映射

复用现有 `BailianClient` 的错误映射逻辑：

| HTTP 状态 | ProviderErrorKind | 可重试 |
|----------|-------------------|--------|
| 401 | Auth | 否 |
| 429 | RateLimited | 是 |
| 400 | InvalidRequest | 否 |
| 5xx | Unknown | 是 |
| timeout | Timeout | 是 |

CosyVoice 特有错误（响应 JSON 中判断）：
- `InvalidParameter` → InvalidRequest
- `Throttling` → RateLimited

---

## 8. IPC 一览

无新增 IPC command。现有命令即可驱动：

| 现有 command | 用途 |
|---|---|
| `submit_task` | 提交 TTS 任务（provider_id="bailian", model_id="cosyvoice-v2", task_type="audio"） |
| `list_models` | 前端获取 cosyvoice-v2 模型信息 |
| `list_api_accounts` | 获取百炼账号（复用现有） |

用户不需要创建新账号 — 现有百炼 api_key 同时支持文生图/生视频/TTS。

---

## 9. 目录增量

```
crates/core/src/provider/bailian/
├── mod.rs           # 修改：追加 cosyvoice 路由 + tts_cache 字段
├── cosyvoice.rs     # NEW
├── types.rs         # 修改：追加 CosyVoice 请求/响应类型
└── validate.rs      # 修改：追加 cosyvoice 验证分支
```

---

## 10. 测试

| 用例 | 说明 |
|------|------|
| `validate_cosyvoice_valid` | 正常参数通过验证 |
| `validate_cosyvoice_empty_text` | 空文本被拒 |
| `validate_cosyvoice_text_too_long` | 超 2000 字被拒 |
| `validate_cosyvoice_speed_range` | speed 超范围被拒 |
| `validate_cosyvoice_missing_voice` | 缺 voice_id 被拒 |
| `submit_200_caches_audio` | 模拟 200 音频响应，验证缓存写入 |
| `submit_401_returns_auth_error` | 模拟 401 |
| `submit_429_retryable` | 模拟 429，标记可重试 |
| `poll_after_submit_returns_success` | submit 后 poll 立即 Success |
| `download_writes_mp3` | 缓存中有数据时写入 .mp3 文件 |
| `cancel_clears_cache` | cancel 后 poll 返回 Failed |
| `seed_creates_cosyvoice_model` | seed 执行后 cosyvoice-v2 model 存在 |

---

## 11. 验收清单

- [ ] cosyvoice-v2 model 在 app 启动时 seed 完成（provider=bailian）
- [ ] `BailianProvider` 正确路由 cosyvoice-v2 到 TTS 逻辑
- [ ] submit 正常文本返回 SubmitOutcome（external_task_id 以 "cosyvoice:" 开头）
- [ ] poll 立即返回 Success（同步模型）
- [ ] download 写入 .mp3 文件到目标路径
- [ ] 参数验证在网络请求前执行（voice_id/text/speed 范围）
- [ ] 错误映射覆盖 401/429/400/5xx
- [ ] `cargo test -p mango-core` 新增 11+ 测试全通过
- [ ] 复用现有百炼账号，无需用户额外配置
- [ ] 音色列表可通过 model capabilities 获取
