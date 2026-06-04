# SPEC-25: 百炼多模型能力收敛

> 对应 MS4 任务 1。在 **单 Provider（bailian）** 前提下，规范 `wan2.7-image-pro` 与 `happyhorse-1.0-r2v`（及 seed 中未来追加的 bailian 模型）的能力元数据、参数校验与前端筛选，为批量提交与抽卡对比提供稳定契约。

依赖：MS2 spec-16（seed）+ spec-17（百炼 Provider）+ spec-18（ModelPicker / 参数表单）

非目标：
- 第二 Provider（即梦）接入 — 后移独立迭代
- 批量提交 IPC — spec-26
- 任务引擎并发/重试 — spec-27

---

## 1. 设计目标 / 非目标

**目标**：

- 充实 `model.capabilities_json` / `default_params_json` seed（或启动时 `ON CONFLICT DO UPDATE` 覆盖系统列）
- Provider 层按 `model_id` 分发参数校验（JSON Schema 或 Rust struct per model）
- 统一 `ProviderErrorDetail` 映射，确保切换模型时错误文案一致
- 前端 `ModelPicker` / 批量模型多选：按 `task_type`、能力标签过滤 bailian 模型列表
- 文档化两模型差异表（同步 vs 异步、是否必须参考图）

**非目标**：

- 新增第三个百炼模型的完整 HTTP 实现（可只加 seed 占位 + 校验 stub，实施时按优先级）
- `usage_used` 回写 — P2，见 `docs/当前进展.md`

---

## 2. 模型能力元数据（seed）

`crates/core/src/seed/providers.rs` 中 bailian 下每个 model 的 `capabilities_json` 建议形态：

```json
{
  "task_types": ["image"],
  "requires_reference_media": false,
  "sync_submit": true,
  "supported_sizes": ["1K", "2K"],
  "max_n": 4
}
```

```json
{
  "task_types": ["video"],
  "requires_reference_media": true,
  "sync_submit": false,
  "supported_resolutions": ["720P", "1080P"],
  "supported_ratios": ["16:9", "9:16", "1:1"],
  "duration_sec": [5, 10]
}
```

`default_params_json` 与 spec-18 表单默认值对齐（减少批量提交时「空 params」）。

**验收**：启动后 `SELECT id, capabilities_json FROM model WHERE provider_id='bailian'` 两行均可解析且字段齐全。

---

## 3. Provider 校验入口

在 `BailianProvider::submit` 前增加 `validate_params(model_id, task_type, params_json) -> Result<ValidatedParams>`：

| model_id | 必填 | 拒绝场景 |
|---|---|---|
| `wan2.7-image-pro` | `prompt`, `size` | `task_type != image` |
| `happyhorse-1.0-r2v` | `prompt`, `media[]` 至少 1 项 | `task_type != video`；`media` 空 |

错误统一为 `ProviderErrorKind::InvalidRequest`，message 中文可操作（与 spec-17 一致）。

---

## 4. 前端：多模型选择 UX

**单任务路径**（既有 `ModelPicker`）：行为不变，仅确保列表来源 `list_models(provider_id=bailian)` 读 capabilities 显示 badge（图/视频、需参考图）。

**批量路径**（spec-26 UI 消费本 spec）：

- `MultiModelSelect`：Checkbox 组，仅展示与当前批量 `task_type` 一致的模型
- 若选中 `happyhorse` 且任一分镜缺参考图，提交前阻断并列出分镜 id/标题

---

## 5. 目录增量

```text
crates/core/src/
├── seed/providers.rs              # 更新 bailian models 的 capabilities/default_params
└── provider/bailian/
    └── validate.rs                # 新增：per-model 参数校验

src/components/generation/
├── ModelPicker.tsx                # 读 capabilities 展示标签
└── MultiModelSelect.tsx           # 新增（spec-26 复用）
```

无 migration。

---

## 6. 测试

| 层级 | 用例 |
|---|---|
| Rust unit | `validate_params` 对各模型合法/非法 JSON |
| Rust unit | seed 应用后 capabilities 非空 |
| 前端 | MultiModelSelect 过滤 task_type；happyhorse 无 media 时禁用提交 |

---

## 7. 验收清单

- [ ] bailian 下至少 2 个模型具备完整 `capabilities_json`
- [ ] 切换模型提交单任务，success 路径与 MS2 一致
- [ ] 非法 params 在 submit 前失败，任务行不创建
- [ ] 批量多选组件能同时勾选 wan27 + happyhorse（task_type 分 batch 时各跑一类，见 spec-26）
