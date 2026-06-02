# SPEC-23: 双路拖放（内部面板 + 系统文件）

## 概述

实现两条素材进入画布的路径：

1. **内部素材面板 → 画布**：复用 MS1 已有的 `AssetCard`，让用户从「素材抽屉」拖一个素材到画布空白处或目标节点上。HTML5 Drag & Drop API。
2. **系统文件管理器 → 画布**：用户从 Finder / Explorer 直接拖入图片/视频文件到画布，自动调用 MS1 spec-12 已有的 `import_asset` 管线（复制 → 缩略图 → 入库），然后在拖放位置创建 AssetNode。复用既有 `useGlobalDropTarget`（spec-12 引入，已在 `ReferenceImageDropzone` 上线）。

两条路径的终点都收敛到 `addNode(createAssetNode(assetId, flowPos))` 单一动作，并且支持「拖到分镜节点上 → 绑定为该 shot 的素材」副作用。

> ⚠️ **作用域**：`GenerationWorkspacePage` 当前是双栏布局，本 spec **不改它**。本 spec 在画布页 `<CanvasInner>` 旁新增一个**可折叠的「素材抽屉」**（`<AssetDrawer>`），让用户在画布上能看到素材库。
>
> ⚠️ **与 spec-22 的素材入口共存**：spec-22 的 `PaneContextMenu` 已经有「添加素材节点…」入口，弹出 `AssetPickerDialog`（modal，picks one）。本 spec 不替换它——pane 右键 picker 是「我要从库里挑一个」的语义，AssetDrawer 拖放是「我要批量把素材摆到画布上」的语义。两者并存，见下文 §"与已有素材入口的关系矩阵"。

## 设计目标 / 非目标

**目标**：

- `<AssetDrawer>` 组件：在 CanvasPage 右侧滑出/收起，列出当前项目所有 asset（复用 `useAssetList` + `AssetFilterBar`），每张 AssetCard 用一个 `<div draggable>` wrapper 包起来——**不修改 `AssetCard` 本体**，避免影响素材库页其它消费者
- 内部拖放：从 AssetDrawer 拖动 AssetCard → 画布 onDrop → 创建 AssetNode
- 内部拖放到 StoryboardNode 上：用 React Flow 12 `useReactFlow().getIntersectingNodes(rect)` 判定命中 → 弹 `BindAssetToShotDialog` 二次确认 → 写 `asset.shot_id`
- 系统文件拖入：复用 `useGlobalDropTarget`（需扩展 handler 签名暴露 position，见 §"useGlobalDropTarget 签名扩展"）→ 校验扩展名 → 自动 `import_asset` → 在拖放位置创建 AssetNode
- 拖动过程中视觉反馈：拖动时画布显示淡蓝高亮蒙层 + 文案「松开以添加到画布」；目标分镜节点 hover 时边框变色（用 React Flow 内置 selected/highlighted style）
- 不支持的文件类型（如 .txt）：toast 友好提示，不创建节点
- 已绑定到其它 shot 的 asset 重新绑定：dialog 显式二次确认（避免静默覆盖）

**非目标**（明确划归后续 spec）：

- 多文件批量拖入到同一 StoryboardNode → V2
- 拖入字幕文件 / 音频文件 → MS5/MS6
- 拖动现有 AssetNode 到另一 StoryboardNode 重新绑定 → 通过删除 + 新建实现，本 spec 不做"快捷重绑"
- 跨画布拖动（episode A 的节点拖到 episode B） → V2
- 用 AssetDrawer 的 drag-and-drop 取代 PaneContextMenu picker → 不做，两者语义不同（见 §"与已有素材入口的关系矩阵"）

## 与已有素材入口的关系矩阵

| 入口 | 触发 | UI | 适用场景 | 本 spec 是否新增 |
|---|---|---|---|---|
| Pane 右键 → 素材节点 | spec-22 | `AssetPickerDialog`（modal） | 「我要从库里挑一个素材，并知道它要放哪」 | 否，已存在 |
| AssetDrawer 拖 → 画布空白 | spec-23 | `useInternalAssetDrop` | 「我要把面板里某个素材摆到画布的具体位置」 | ✅ |
| AssetDrawer 拖 → StoryboardNode | spec-23 | `BindAssetToShotDialog` | 「我要把这个素材绑给那个分镜」 | ✅ |
| Finder 拖 → 画布 | spec-23 | `useSystemFileDrop` → import + addNode | 「我有项目外的文件，导入并摆放」 | ✅ |
| Finder 拖 → ReferenceImageDropzone | spec-12 | `useGlobalDropTarget` 既有消费 | 角色参考图编辑场景，作用域在 CharacterDialog 内 | 否，已存在 |

> **设计原则**：每个入口对应一种用户意图，不强行收敛。MS3 的目标是让画布能玩起来，不是重做素材管理。

## 技术方案

### 目录/文件结构

```text
src/components/canvas/
├── CanvasInner.tsx                     # spec-21 已建，本 spec 包一层 onDragOver/onDrop + 挂 AssetDrawer + 启 useSystemFileDrop
├── drag/
│   ├── AssetDrawer.tsx                 # 新增：右侧可折叠抽屉
│   ├── useInternalAssetDrop.ts         # 新增：HTML5 DnD onDrop 处理
│   ├── useSystemFileDrop.ts            # 新增：复用 useGlobalDropTarget；处理 dpr + ext 过滤 + import
│   ├── DropTargetOverlay.tsx           # 新增：拖动时画布的视觉高亮
│   ├── BindAssetToShotDialog.tsx       # 新增：拖到分镜节点上的二次确认（含覆盖警告）
│   └── constants.ts                    # 新增：MIME / accepted exts / drag-data 常量
└── nodeFactory.ts                      # spec-22 已建，本 spec 不改

src/components/assets/
└── AssetCard.tsx                       # 既有组件，**不改**。AssetDrawer 用 wrapper <div draggable> 套上 drag 行为

src/hooks/
├── useAssets.ts                        # 既有，本 spec 新增 useAssignAssetToShot mutation
└── useGlobalDropTarget.ts              # 既有，本 spec 扩展 DropHandler 签名加 position（向后兼容）

crates/core/src/db/queries/
└── asset.rs                            # 既有，本 spec 新增 assign_to_shot(conn, id, shot_id: Option<&str>) -> Result<Asset>

src-tauri/src/commands/
└── asset.rs                            # 既有，本 spec 新增 assign_asset_to_shot Tauri command
```

### 依赖项变更

**无新增 npm / Rust / Tauri 插件依赖**——本 spec 全部基于既有栈：

- HTML5 DnD：浏览器原生
- 系统文件拖入：`useGlobalDropTarget`（spec-12 已存在），底层 `@tauri-apps/api/webview` 的 `onDragDropEvent`
- 命中检测：React Flow 12 `useReactFlow().getIntersectingNodes(rect)`（`@xyflow/react` 已 pin）
- 弹窗：复用 spec-22 引入的 shadcn `dialog`、`dropdown-menu`

`dependencies.md` 增量（IPC 层）：

- 新增 Tauri command `assign_asset_to_shot(id: String, shot_id: Option<String>) -> Asset`
- 新增 frontend hook `useAssignAssetToShot()`（tanstack mutation）

### 后端：`assign_asset_to_shot`

**为什么单开命令而不复用 `update_asset_label` 的模式扩成通用 `update_asset`**：

- `asset.shot_id` 是 FK，单值覆盖语义清晰，单独一个 verb 命名 (`assign_to_shot`) 比 `update_asset({shot_id: ...})` 更能表达「绑定」这一业务动作
- 通用 `update_asset` 会让未来每加一个字段都要扩 input struct，容易 ts-rs 生成偏移；按动词拆命令是 MS1/MS2 一贯风格（`update_asset_label`、`unlink_shot_subject` 等）

**Rust 实现要点**（`crates/core/src/db/queries/asset.rs`）：

```rust
/// Bind (or unbind) an asset to a shot. Passing `Some(shot_id)` overwrites
/// any existing `asset.shot_id`; passing `None` clears it. The previous
/// value is NOT returned here — callers that need overwrite confirmation
/// must read `get_by_id` first.
pub fn assign_to_shot(conn: &Connection, id: &str, shot_id: Option<&str>) -> Result<Asset> {
    let n = conn.execute(
        "UPDATE asset SET shot_id = ?1, updated_at = datetime('now') WHERE id = ?2",
        params![shot_id, id],
    )?;
    if n == 0 {
        return Err(CoreError::NotFound { entity: "asset", id: id.to_string() });
    }
    get_by_id(conn, id)
}
```

测试：4 个 `#[cfg(test)]` 用例——bind / overwrite / unbind / not-found。

**Tauri command**（`src-tauri/src/commands/asset.rs`）：

```rust
#[tauri::command]
#[specta::specta]
pub async fn assign_asset_to_shot(
    state: State<'_, AppState>,
    id: String,
    shot_id: Option<String>,
) -> Result<Asset, IpcError> {
    with_db(&state, move |conn| {
        asset_queries::assign_to_shot(conn, &id, shot_id.as_deref())
    })
    .await
}
```

注册到 `src-tauri/src/lib.rs` 的 `invoke_handler!` + `collect_commands!`。

**Frontend hook**（`src/hooks/useAssets.ts` 追加）：

```ts
export function useAssignAssetToShot() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, shotId }: { id: string; shotId: string | null }) =>
      unwrap(commands.assignAssetToShot(id, shotId)),
    onSuccess: (asset) => {
      qc.invalidateQueries({ queryKey: ["assets", asset.project_id] });
      qc.invalidateQueries({ queryKey: assetKeys.detail(asset.id) });
    },
  });
}
```

### `useGlobalDropTarget` 签名扩展

当前签名（`src/hooks/useGlobalDropTarget.ts`）：

```ts
export type DropHandler = (paths: string[]) => void;
```

`useSystemFileDrop` 需要 `position`（物理像素）才能把文件放到正确画布坐标。**最小侵入扩展**：

```ts
import type { PhysicalPosition } from "@tauri-apps/api/dpi";
export type DropHandler = (paths: string[], position: PhysicalPosition) => void;
```

唯一既有消费者 `ReferenceImageDropzone` 的 handler 签名 `(paths: string[]) => void` 在结构类型下仍兼容（TypeScript 允许少形参函数赋给多形参函数类型）——**不需要改 ReferenceImageDropzone**。

`useGlobalDropTarget.ts` 内部把 `event.payload.position` 透传给栈顶 handler：

```ts
unlistenPromise = getCurrentWebview().onDragDropEvent((event) => {
  if (event.payload.type !== "drop") return;
  const top = handlerStack[handlerStack.length - 1];
  top?.(event.payload.paths, event.payload.position);
});
```

`useGlobalDropTarget.test.tsx` 现有用例对 handler 都用 `vi.fn()` 接所有参数，签名扩展不破坏断言。spec-23 在该测试文件再加 1 个用例：position 透传断言。

### 前端：内部拖放（AssetCard → Canvas）

#### AssetDrawer 内的 wrapper

不动 `AssetCard`，在抽屉里包一层：

```tsx
// src/components/canvas/drag/AssetDrawer.tsx 片段
import { DRAG_MIME_ASSET_ID } from "./constants";

function DraggableAssetCard({ asset, projectRoot }: { asset: Asset; projectRoot: string }) {
  return (
    <div
      draggable
      onDragStart={(e) => {
        e.dataTransfer.setData(DRAG_MIME_ASSET_ID, asset.id);
        e.dataTransfer.effectAllowed = "copy";
      }}
    >
      <AssetCard
        asset={asset}
        projectRoot={projectRoot}
        onPreview={() => {/* drawer 内不开 preview，留空或交给 AssetDrawer state */}}
        onDelete={() => {/* drawer 内不允许删除，传 noop */}}
      />
    </div>
  );
}
```

> **为什么不在 AssetCard 上加 `draggable`**：AssetCard 还被素材库主页等场景消费，在那里加 `draggable` 会让点击/选中行为变怪（拖动会抢先于点击）。Wrapper 模式让 drag 行为只在 AssetDrawer 内启用。

#### `useInternalAssetDrop`

```ts
// src/components/canvas/drag/useInternalAssetDrop.ts
import { useReactFlow } from "@xyflow/react";
import { DRAG_MIME_ASSET_ID } from "./constants";

export interface InternalDropResult {
  onDrop: (e: React.DragEvent) => void;
  onDragOver: (e: React.DragEvent) => void;
  isOver: boolean;
}

export function useInternalAssetDrop(opts: {
  onPlaceAsset: (assetId: string, flowPos: XYPosition) => void;
  onAttachToShot: (assetId: string, shotId: string) => void;
}): InternalDropResult {
  const [isOver, setOver] = useState(false);
  const { screenToFlowPosition, getIntersectingNodes } = useReactFlow();

  const onDragOver = useCallback((e: React.DragEvent) => {
    if (!e.dataTransfer.types.includes(DRAG_MIME_ASSET_ID)) return;
    e.preventDefault();
    e.dataTransfer.dropEffect = "copy";
    setOver(true);
  }, []);

  const onDrop = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setOver(false);
    const assetId = e.dataTransfer.getData(DRAG_MIME_ASSET_ID);
    if (!assetId) return;

    const flowPos = screenToFlowPosition({ x: e.clientX, y: e.clientY });

    // Build a 1×1 rect at drop point; ask React Flow which nodes intersect.
    const hit = getIntersectingNodes({
      x: flowPos.x,
      y: flowPos.y,
      width: 1,
      height: 1,
    });
    const shotNode = hit.find((n) => n.type === "storyboard");
    if (shotNode) {
      const shotId = (shotNode.data as StoryboardNodeData).shotId;
      opts.onAttachToShot(assetId, shotId);
      return;
    }

    opts.onPlaceAsset(assetId, flowPos);
  }, [screenToFlowPosition, getIntersectingNodes, opts]);

  return { onDrop, onDragOver, isOver };
}
```

> **设计说明**：用 React Flow 内置 `getIntersectingNodes` 代替自写 hit-test，避免节点尺寸常量在多处重复，并与节点选中/移动逻辑共用一套坐标系。drag-leave 不主动重置 `isOver`——React 的 `onDragLeave` 在子元素切换时会假触发；`onDrop` / 鼠标移出 React Flow 容器时由父组件的 `onDragLeave` 兜底（在 CanvasInner 里直接 `onDragLeave={() => setIsOver(false)}`）。

#### CanvasInner 集成

```tsx
// CanvasInner.tsx 增量
const handlePlaceAsset = useCallback(
  (assetId: string, flowPos: XYPosition) => addNode(createAssetNode(assetId, flowPos)),
  [addNode],
);
const [bindRequest, setBindRequest] = useState<{ assetId: string; shotId: string } | null>(null);
const handleAttachToShot = useCallback(
  (assetId: string, shotId: string) => setBindRequest({ assetId, shotId }),
  [],
);
const internal = useInternalAssetDrop({ onPlaceAsset: handlePlaceAsset, onAttachToShot: handleAttachToShot });
const { isSystemDragOver } = useSystemFileDrop({ projectId, onPlaceAsset: handlePlaceAsset });

return (
  <div
    className="relative h-full"
    onDragOver={internal.onDragOver}
    onDrop={internal.onDrop}
    onDragLeave={() => {/* internal hook owns its own isOver via mouse-up */}}
  >
    <ReactFlow ... />
    <DropTargetOverlay show={internal.isOver || isSystemDragOver} />
    {projectId && <AssetDrawer projectId={projectId} projectRoot={projectRoot} />}
    <BindAssetToShotDialog
      request={bindRequest}
      onClose={() => setBindRequest(null)}
    />
  </div>
);
```

### 前端：系统文件拖入

```ts
// src/components/canvas/drag/useSystemFileDrop.ts
import { useGlobalDropTarget } from "@/hooks/useGlobalDropTarget";
import { ACCEPTED_EXTENSIONS } from "./constants";

export function useSystemFileDrop(opts: {
  projectId: string;
  onPlaceAsset: (assetId: string, flowPos: XYPosition) => void;
}): { isSystemDragOver: boolean } {
  const { screenToFlowPosition } = useReactFlow();
  const importAsset = useImportAsset();

  // Stabilize the latest deps in a ref so we can register the global handler
  // with stable identity. React Query mutation objects are reference-equal
  // across renders only when nothing it watches changes — using a ref avoids
  // depending on that contract.
  const latestRef = useRef({ ...opts, importAsset, screenToFlowPosition });
  useEffect(() => {
    latestRef.current = { ...opts, importAsset, screenToFlowPosition };
  });

  const handler = useCallback<DropHandler>(async (paths, position) => {
    const { projectId, onPlaceAsset, importAsset, screenToFlowPosition } = latestRef.current;

    const accepted = paths.filter((p) => {
      const ext = p.split(".").pop()?.toLowerCase();
      return ext != null && ACCEPTED_EXTENSIONS.includes(ext);
    });
    if (accepted.length === 0) {
      toast.error("不支持的文件类型，仅支持 PNG / JPG / WEBP / MP4 / MOV / WEBM");
      return;
    }
    if (paths.length > accepted.length) {
      toast.warning(`已忽略 ${paths.length - accepted.length} 个不支持的文件`);
    }

    // Tauri payload.position is PHYSICAL pixels; divide by dpr to get CSS px,
    // then ask React Flow to map CSS px into flow-space coordinates.
    const dpr = window.devicePixelRatio || 1;
    const flowPos = screenToFlowPosition({
      x: position.x / dpr,
      y: position.y / dpr,
    });

    for (const [idx, path] of accepted.entries()) {
      try {
        const asset = await importAsset.mutateAsync({
          project_id: projectId,
          source_path: path,
          shot_id: null,
        });
        onPlaceAsset(asset.id, { x: flowPos.x + idx * 20, y: flowPos.y + idx * 20 });
      } catch (e) {
        toast.error(`导入失败：${path.split("/").pop()} — ${e}`);
        // No addNode on failure — partial import is acceptable; user sees a
        // toast per failed file.
      }
    }
  }, []); // stable identity by design

  useGlobalDropTarget(handler);

  // useGlobalDropTarget swallows enter/leave; for the overlay we'd need a
  // separate listener. Per spec-23 §"视觉反馈范围决议", we don't show overlay
  // during system drag until enter/leave is exposed (V2). Return false for now.
  return { isSystemDragOver: false };
}
```

> **设计决议**：
>
> 1. **坐标换算**：Tauri `onDragDropEvent` 的 `position` 是 `PhysicalPosition`（物理像素，受 devicePixelRatio 影响）。Retina 屏不除 dpr 会偏移 2x。**首次实施 PR 必须在 Retina 真机走一遍手动验证**——不仅文档写写。
> 2. **多文件递增偏移**：每个后续节点偏移 20px，避免堆叠
> 3. **拖到分镜上的绑定**：系统拖入路径**不**自动绑分镜——理由是系统拖入后 asset 是全新的，用户更可能想先看一眼再决定绑哪个 shot。如果需求强烈再加（V2）
> 4. **import_asset 自带去重**：spec-12 的 import_asset 用 content_hash 去重；重复拖入同一文件不会产生 2 个 asset 行，但 `mutateAsync` 会返回同 id 的 asset。`addNode` 在 spec-22 已加去重（`useCanvasStore.addNode` 检测同 nodeId 已存在则 toast「该素材已在画布上」），所以重复拖入不会爆增节点
> 5. **视觉反馈范围决议**：当前 `useGlobalDropTarget` 只透传 `drop`，没有透传 `enter`/`over`/`leave`。系统拖入过程中**画布不亮蓝色蒙层**——仅内部拖放（HTML5 DnD）显示 overlay。代价是 UX 一致性差一档，收益是不需要在 spec-23 里把 `useGlobalDropTarget` 改成支持完整事件流（那是独立的重构，影响 ReferenceImageDropzone）。V2 评估是否升级
> 6. **useEffect 依赖稳定化**：用 `useRef` 持有最新值 + handler `useCallback` 空依赖。这样无论 React Query mutation 引用是否变，全局 listener 只注册一次

### `BindAssetToShotDialog`（含覆盖警告）

**核心问题**：`asset.shot_id` 是单值 FK。如果用户把已绑给 shot A 的素材拖到 shot B 节点上，静默覆盖会丢失关联。必须显式确认。

```tsx
interface Request { assetId: string; shotId: string; }
interface Props { request: Request | null; onClose: () => void; }

export function BindAssetToShotDialog({ request, onClose }: Props) {
  const { data: asset } = useAsset(request?.assetId);
  const { data: targetShot } = useShot(request?.shotId);
  const { data: currentShot } = useShot(asset?.shot_id ?? null); // 当前绑给谁
  const assign = useAssignAssetToShot();

  if (!request || !asset || !targetShot) return null;

  const alreadyBoundElsewhere =
    asset.shot_id != null && asset.shot_id !== request.shotId;

  const confirm = async () => {
    try {
      await assign.mutateAsync({ id: request.assetId, shotId: request.shotId });
      toast.success(
        asset.asset_type === "video"
          ? "已采用为分镜的视频结果"
          : "已设为分镜的参考素材",
      );
      onClose();
    } catch (e) {
      toast.error(`绑定失败：${e}`);
    }
  };

  return (
    <Dialog open onOpenChange={(o) => !o && onClose()}>
      <DialogContent>
        <DialogTitle>绑定素材到分镜</DialogTitle>
        <p className="text-sm text-muted-foreground">
          将素材 <b>{asset.original_name}</b> 绑定到分镜
          <b> {targetShot.title || `Shot ${targetShot.order_index + 1}`}</b>。
        </p>
        {alreadyBoundElsewhere && (
          <div className="rounded-md border border-amber-300 bg-amber-50 p-3 text-sm">
            ⚠️ 该素材当前已绑定到
            <b> {currentShot?.title || asset.shot_id}</b>，
            继续操作会覆盖原绑定。
          </div>
        )}
        <DialogFooter>
          <Button variant="outline" onClick={onClose}>取消</Button>
          <Button onClick={confirm} disabled={assign.isPending}>
            {alreadyBoundElsewhere ? "覆盖并绑定" : "确认绑定"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
```

> **设计说明**：
>
> - **文案统一**：当前 schema 下「参考图」和「采用结果」**后端是同一动作**（写 `asset.shot_id`），所以对话框的确认按钮文案统一为「确认绑定」。只在 toast 里按 `asset_type` 区分语义提示。MS4 引入 `shot.result_asset_id` 列后，「采用为视频结果」才会真正写一个独立字段——届时新增按钮分支
> - **不在 dialog 里区分 image / video 行为**：避免用户误以为「图片只能做参考、视频只能做结果」——其实任何 asset 都允许绑定，UI 不该把后端的未来分裂提前到现在
> - **失败回滚**：assignment 失败 toast 错误并保留对话框（让用户能重试或取消）；不影响画布 store（addNode 已发生但 asset.shot_id 未变，画布上节点照常显示，无副作用）

### `AssetDrawer`

```tsx
export function AssetDrawer({ projectId, projectRoot }: { projectId: string; projectRoot: string }) {
  const [open, setOpen] = useState(false);
  const [source, setSource] = useState<AssetSource | undefined>(undefined);
  const [assetType, setAssetType] = useState<AssetType | undefined>(undefined);
  const [keyword, setKeyword] = useState("");

  const { data: assets } = useAssetList(projectId, assetType, { source, keyword });

  return (
    <>
      <Button
        size="icon" variant="outline"
        className="absolute right-4 top-4 z-10"
        onClick={() => setOpen((v) => !v)}
        title="素材库"
      >
        <FolderOpen className="size-4" />
      </Button>
      <aside
        className={cn(
          "absolute right-0 top-0 z-20 h-full w-80 border-l bg-background shadow-lg transition-transform",
          open ? "translate-x-0" : "translate-x-full pointer-events-none",
        )}
      >
        <header className="flex items-center justify-between border-b p-3">
          <h3 className="text-sm font-medium">素材库</h3>
          <Button size="icon" variant="ghost" onClick={() => setOpen(false)}>×</Button>
        </header>
        <AssetFilterBar
          source={source} onSourceChange={setSource}
          assetType={assetType} onAssetTypeChange={setAssetType}
          keyword={keyword} onKeywordChange={setKeyword}
        />
        <div className="grid grid-cols-2 gap-2 overflow-auto p-3">
          {assets?.map((a) => (
            <DraggableAssetCard key={a.id} asset={a} projectRoot={projectRoot} />
          ))}
        </div>
      </aside>
    </>
  );
}
```

> **设计说明**：
>
> - **默认收起**：避免遮挡画布；首次打开后状态**不持久化**（本 spec 不引入 UI 状态 store；V2 评估是否进 Zustand UI store）
> - **AssetFilterBar 复用前的现状检查**：实施 PR 第一步先打开 `src/components/assets/AssetFilterBar.tsx` 看 props 实际签名，按它的真实 props 调用——上面的代码是示意，实施时以 AssetFilterBar 当前签名为准
> - **关闭时 `pointer-events-none`**：避免隐藏的 aside 拦截画布右侧的点击/拖放事件

### `DropTargetOverlay`

```tsx
export function DropTargetOverlay({ show }: { show: boolean }) {
  if (!show) return null;
  return (
    <div className="pointer-events-none absolute inset-0 z-10 flex items-center justify-center border-4 border-dashed border-blue-400 bg-blue-50/20">
      <p className="rounded-md bg-background/90 px-4 py-2 text-sm font-medium shadow">
        松开以添加到画布
      </p>
    </div>
  );
}
```

### 常量集中

```ts
// src/components/canvas/drag/constants.ts
export const DRAG_MIME_ASSET_ID = "application/x-mango-asset-id";
export const ACCEPTED_EXTENSIONS = ["png", "jpg", "jpeg", "webp", "mp4", "mov", "webm"];
```

### 错误场景

| 场景 | 行为 |
|---|---|
| 系统拖入文件名含 `..` 或路径穿越 | `import_asset` 内部 canonical path 校验拒绝；toast 错误 |
| 系统拖入 GIF | `ACCEPTED_EXTENSIONS` 不含 gif，toast 提示；后续可加 |
| 系统拖入超大文件（> 1GB） | `import_asset` 当前未实现 size 检查；本 spec 范围内**不加**，由 MS5 视频后期再统一加 size 限制 |
| 内部拖到自身（同 assetId 已是画布上的 AssetNode） | `addNode` 在 spec-22 已有 dedup（同 nodeId 不允许重复），自动 toast「该素材已在画布上」，不重复创建 |
| 拖到 storyboard 节点但 shotId 已被删 | `useShot(shotId)` 返回 undefined → dialog 渲染 null，不卡 UI；用户重试或刷新 |
| `assign_asset_to_shot` 后端写失败 | dialog toast 错误，保留 dialog 让用户重试；不影响 canvas store |
| Tauri webview 还未 ready 时 hook mount | `getCurrentWebview()` 在 mount 时即可用（spec-12 / ReferenceImageDropzone 已验证）；onDragDropEvent 注册 promise resolve 前的事件会丢失，但首屏 < 100ms 窄窗口可接受 |
| 多个 CanvasInner 同时 mount（理论不应发生） | `useGlobalDropTarget` 栈语义：仅最后一个生效；约束：CanvasPage 单实例，由路由保证 |
| 同时拖动内部 asset + 系统文件（不可能并发） | 浏览器一次只能处理一种 drag source；不需要互斥逻辑 |

### 测试策略

#### 前端单元测试

**`src/components/canvas/drag/useInternalAssetDrop.test.ts`**：
- 触发 `onDragOver` 含 MIME → `isOver=true` + `dropEffect="copy"`
- 触发 `onDragOver` 不含 MIME → 不 setOver、不 preventDefault
- `onDrop` 在空白处（`getIntersectingNodes` 返回 []）→ `onPlaceAsset` 被调用，参数 = (assetId, flowPos)
- `onDrop` 命中 storyboard 节点 → `onAttachToShot` 被调用，参数 = (assetId, shotId)
- `onDrop` 命中非 storyboard 节点（如 character）→ 走 `onPlaceAsset` 分支（不绑定到非 shot）

**`src/components/canvas/drag/useSystemFileDrop.test.ts`**：
- mock `useGlobalDropTarget` 捕获 handler；mock `useImportAsset.mutateAsync`
- 触发 handler：paths=[a.png, b.txt], position={x:200,y:100}, dpr=2
  - 验证 toast.warning「已忽略 1 个不支持的文件」
  - 验证 importAsset.mutateAsync 仅被 1 次调用（a.png）
  - 验证 `onPlaceAsset` 被调用，flowPos 由 `screenToFlowPosition({x:100,y:50})` 推导
- paths=[a.txt] → toast.error，不调 import
- import 失败 → toast.error，不调 onPlaceAsset
- 多文件成功 → 后续节点偏移 20px

**`src/components/canvas/drag/BindAssetToShotDialog.test.tsx`**：
- `asset.shot_id == null`：渲染「确认绑定」按钮，无覆盖警告
- `asset.shot_id == request.shotId`：渲染「确认绑定」按钮，无覆盖警告（同 shot 不算覆盖）
- `asset.shot_id !== request.shotId && asset.shot_id != null`：渲染「覆盖并绑定」按钮 + 黄色 warning 块
- 点确认 → `useAssignAssetToShot.mutateAsync` 被调用，参数 = `{id, shotId}`
- mutation 失败 → toast.error，dialog 不关闭

**`src/components/canvas/drag/AssetDrawer.test.tsx`**：
- 默认 `translate-x-full`（收起）；点 toggle → `translate-x-0`
- 渲染 `DraggableAssetCard` 列表（mock useAssetList 返回 3 个 asset）
- `DraggableAssetCard` `onDragStart` 在 `dataTransfer` 上设 `application/x-mango-asset-id` = asset.id

**`src/hooks/useGlobalDropTarget.test.tsx`**（既有文件加 1 用例）：
- handler 收到的第二参数 = mock event payload 的 position

**`src/hooks/useAssets.test.ts`**（如不存在则新建；最少覆盖 `useAssignAssetToShot`）：
- mutate 成功 → invalidate 正确 queryKey

#### Rust 测试

**`crates/core/src/db/queries/asset.rs`** `#[cfg(test)]` 追加 4 个用例：

- `assign_to_shot_binds_when_empty`
- `assign_to_shot_overwrites_existing`
- `assign_to_shot_unbinds_with_none`
- `assign_to_shot_returns_not_found_for_missing_id`

#### 手动验证清单

- [ ] 打开画布 → 右上角 FolderOpen 图标点开 → AssetDrawer 滑出 + 显示项目素材
- [ ] 从 AssetDrawer 拖一个图片到画布空白 → 创建 AssetNode（drawer 不自动关闭）
- [ ] 拖一个 image asset 到 StoryboardNode 上 → 弹「确认绑定」对话框（无警告）→ 点确认 → `sqlite3 mango.db "SELECT shot_id FROM asset WHERE id='<id>'"` 已更新；toast「已设为分镜的参考素材」
- [ ] 同一 asset 拖到另一个 StoryboardNode → 弹「覆盖并绑定」对话框（含黄色警告显示当前绑定的 shot 名）→ 点覆盖 → DB shot_id 更新到新 shot
- [ ] 拖一个 video asset 到 StoryboardNode → 弹「确认绑定」→ 确认后 toast「已采用为分镜的视频结果」
- [ ] 从 Finder 拖一个 .png 到画布 → import 完成 + AssetNode 出现在鼠标松开位置（**Retina 屏须验证位置准确，无 2x 偏移**）
- [ ] 从 Finder 拖一个 .txt 到画布 → toast 错误，不创建节点
- [ ] 从 Finder 拖混合（2 png + 1 txt）到画布 → toast warning「已忽略 1 个」+ 2 个 AssetNode 错位排列
- [ ] 拖入相同文件两次 → DB 只多 1 行 asset（spec-12 去重）+ 画布上只有 1 个 AssetNode（spec-22 addNode 去重 + toast「该素材已在画布上」）
- [ ] AssetDrawer 收起后，画布右上区域可以正常右键弹 PaneContextMenu（验证 `pointer-events-none` 生效）
- [ ] ReferenceImageDropzone（角色编辑对话框）仍能接收系统拖入（`useGlobalDropTarget` 栈语义未破）

## 验收标准

- [ ] 后端 `assign_to_shot` query + `assign_asset_to_shot` Tauri command + ts-rs 绑定生成
- [ ] `useAssignAssetToShot` mutation hook，invalidate 正确 queryKey
- [ ] `useGlobalDropTarget` 签名扩展为 `(paths, position) => void`，既有 ReferenceImageDropzone 不需改
- [ ] `useInternalAssetDrop` / `useSystemFileDrop` 两个 hook 完整实现
- [ ] `DropTargetOverlay` 在内部拖动时显示蓝色高亮（系统拖入的 overlay 留待 V2）
- [ ] `AssetDrawer` 右侧滑入滑出，复用 `AssetCard` 与 `AssetFilterBar`；wrapper 模式不污染 AssetCard
- [ ] `BindAssetToShotDialog` 含覆盖警告分支；按 `asset_type` 区分 toast 文案，按钮文案统一
- [ ] 系统文件拖入 dpr 换算正确（Retina 真机过）
- [ ] 多文件批量拖入 + 不支持文件类型 + 重复拖入 三种边界正确处理
- [ ] Rust 端新增 ≥ 4 个测试；前端新增 ≥ 14 个测试
- [ ] `pnpm test:run` 全绿；`pnpm typecheck` 干净；`cargo clippy -p mango-core -p mango-tauri` 干净
- [ ] `dependencies.md` IPC 增量已更新
- [ ] 手动验证清单全过（Retina 屏验证必须完成）

## 依赖关系

- **前置**：spec-22（需要 `createAssetNode` factory + `AssetNode` 组件 + `StoryboardNode` 作为拖放目标；`useCanvasStore.addNode` 的 dedup 行为；shadcn dialog 已引入）
- **平行**：spec-24（拖放产生的 `addNode` / `assign_asset_to_shot` 必须被 zundo 捕获，但 zundo middleware 由 spec-24 加，PR 顺序确保即可；`assign_asset_to_shot` 是后端写入，**不进** zundo 撤销栈——撤销栈仅画布形态，后端业务变更靠 dialog 取消按钮规避）
- **基础设施**：本 spec 复用 spec-12 的 `useGlobalDropTarget`、`useImportAsset`，并对 `useGlobalDropTarget` 做向后兼容的签名扩展

## 与原任务文档的差异

合并自原任务 8（素材面板 → 画布拖放）+ 任务 9（系统文件 → 画布拖放）。本 spec 相对评审前版本（v1）的关键修订：

- **AssetPickerDialog 误判更正**：v1 一度认为 `AssetPickerDialog` 不存在；实际位于 `src/components/assets/AssetPickerDialog.tsx`，spec-22 的 PaneContextMenu 已在用。本 spec 明确「pane picker」与「drawer drag」语义分工，二者并存
- **复用既有 `useGlobalDropTarget`**：v1 自写 `w.onDragDropEvent` 监听，会与 `ReferenceImageDropzone` 的 webview-global listener 冲突。本 spec 改为扩展 `useGlobalDropTarget` 的 `DropHandler` 签名加 `position`，复用其栈语义
- **补 `assign_asset_to_shot` 后端**：v1 用「`useUpdateAsset` 写 `shot_id`」一笔带过，但代码库无 `useUpdateAsset` 也无 `update_asset` 命令。本 spec 显式补 core query + Tauri command + frontend hook
- **覆盖警告**：v1 静默写 `asset.shot_id`，会造成单值 FK 静默丢失关联。本 spec 在 `BindAssetToShotDialog` 加显式覆盖确认分支
- **image / video 文案统一**：v1 让按钮文案分裂为「绑定为参考图 / 采用为视频结果」，但后端是同一动作。本 spec 按钮统一「确认绑定」，仅 toast 按 `asset_type` 区分语义
- **AssetCard wrapper 模式**：v1 直接在 `AssetCard` 上加 `draggable`，会影响素材库主页等其它消费者。本 spec 改为 `DraggableAssetCard` wrapper，drag 行为只在 AssetDrawer 启用
- **hit-test 用 React Flow 内置 API**：v1 自写 `findNodeAtPosition`，重复维护节点尺寸常量。本 spec 用 `useReactFlow().getIntersectingNodes(rect)`
- **useEffect 依赖稳定化**：v1 把 React Query mutation 对象放进 deps，会导致重复注册全局 listener。本 spec 用 `useRef` 持有最新值 + `useCallback([])`
- **视觉反馈范围明确化**：v1 暗示系统拖入时也亮 overlay，但 `useGlobalDropTarget` 只透传 `drop` 事件。本 spec 显式决议系统拖入 overlay 待 V2，避免半实现
