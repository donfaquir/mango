# SPEC-23: 双路拖放（内部面板 + 系统文件）

## 概述

实现两条素材进入画布的路径：

1. **内部素材面板 → 画布**：复用 MS1 已有的 `AssetGrid` / `AssetCard`，让用户从素材库面板拖动一个素材到画布空白处或目标节点上。HTML5 Drag & Drop API。
2. **系统文件管理器 → 画布**：用户从 Finder / Explorer 直接拖入图片/视频文件到画布，自动调用 MS1 spec-12 已有的 `import_asset` 管线（复制 → 缩略图 → 入库），然后在拖放位置创建 AssetNode。Tauri 2 `WebviewWindow::onDragDropEvent`。

两条路径的终点都收敛到 `addAssetNodeAt(position, assetId)` 单一动作，并且支持「拖到分镜节点上 → 绑定为 shot 的参考图/采用视频」副作用。

> ⚠️ **破坏性变更**：`GenerationWorkspacePage` 当前是双栏布局，本 spec **不改它**。本 spec 在画布页 `<CanvasInner>` 旁新增一个**可折叠的「素材抽屉」**（`<AssetDrawer>`），让用户在画布上能看到素材库。

## 设计目标 / 非目标

**目标**：

- `<AssetDrawer>` 组件：在 CanvasPage 右侧滑出/收起，列出当前项目的所有 asset（复用 `useAssetList`），支持按 source / asset_type 筛选
- 内部拖放：从 AssetDrawer 拖动一个 AssetCard → 画布 onDrop → 创建 AssetNode
- 内部拖放到目标节点上：拖到 StoryboardNode 上 → 弹「绑定为参考图 / 采用为视频结果」选择 → 执行业务操作
- 系统文件拖入：监听 Tauri `onDragDropEvent` → 校验文件类型 → 自动 `import_asset` → 在拖放位置创建 AssetNode
- 拖动过程中视觉反馈：拖动时画布显示淡蓝高亮；目标节点 hover 时边框变色
- 不支持的文件类型（如 .txt）：toast 友好提示，不创建节点

**非目标**（明确划归后续 spec）：

- 多文件批量拖入 → V2
- 拖入字幕文件 / 音频文件 → MS5/MS6
- 拖动现有 AssetNode 到另一 StoryboardNode 重新绑定 → 通过删除 + 新建实现，本 spec 不做"快捷重绑"
- 跨画布拖动（episode A 的节点拖到 episode B） → V2

## 技术方案

### 目录/文件结构

```text
src/components/canvas/
├── CanvasInner.tsx                     # spec-21 已建，本 spec 加 onDrop / onDragOver + 引入 AssetDrawer
├── drag/
│   ├── AssetDrawer.tsx                 # 新增：右侧可折叠抽屉
│   ├── useInternalAssetDrop.ts         # 新增：处理 HTML5 DnD
│   ├── useSystemFileDrop.ts            # 新增：处理 Tauri onDragDropEvent
│   ├── DropTargetOverlay.tsx           # 新增：拖动时画布的视觉高亮
│   └── BindAssetToShotDialog.tsx       # 新增：拖到分镜节点上的二次确认
└── nodeFactory.ts                      # spec-22 已建，本 spec 加 createAssetNodeFromDrop helper

src/components/assets/
└── AssetCard.tsx                       # 既有组件，本 spec 加 draggable + onDragStart 设 dataTransfer

crates/core/src/asset/
└── import.rs                           # spec-12 已建，本 spec **不改**
```

### 依赖项变更

无新增 npm / Rust / Tauri 插件依赖。系统文件拖入的核心 API：

```ts
// @tauri-apps/api/webviewWindow
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

const w = getCurrentWebviewWindow();
const unlisten = await w.onDragDropEvent((event) => {
  if (event.payload.type === "drop") {
    const paths = event.payload.paths;
    const position = event.payload.position; // 物理像素坐标
    // ...
  }
});
```

Tauri 2 已经把这个 API 暴露在 `@tauri-apps/api`，无需额外插件。

### 内部拖放（AssetCard → Canvas）

#### AssetCard 增强

```tsx
// src/components/assets/AssetCard.tsx
export function AssetCard({ asset, onClick }: AssetCardProps) {
  return (
    <div
      draggable
      onDragStart={(e) => {
        e.dataTransfer.setData("application/x-mango-asset-id", asset.id);
        e.dataTransfer.effectAllowed = "copy";
      }}
      onClick={onClick}
      className="..."
    >
      {/* 既有内容 */}
    </div>
  );
}
```

> **设计说明**：用 `application/x-mango-asset-id` 自定义 MIME，避免与系统文件拖放（默认 `Files` MIME）冲突。`onDrop` 时优先检查这个 MIME 决定走哪条分支。

#### CanvasInner onDrop

```tsx
// CanvasInner.tsx 增强
const { onDrop: onInternalDrop, isInternalDragOver } = useInternalAssetDrop(episodeId);
const { isSystemDragOver } = useSystemFileDrop(episodeId);

return (
  <div
    className={cn("relative h-full", (isInternalDragOver || isSystemDragOver) && "bg-blue-50/30")}
    onDragOver={(e) => e.preventDefault()}
    onDrop={onInternalDrop}
  >
    <ReactFlow ... />
    <DropTargetOverlay show={isInternalDragOver || isSystemDragOver} />
    <AssetDrawer projectId={projectId} />
  </div>
);
```

#### useInternalAssetDrop hook

```ts
// src/components/canvas/drag/useInternalAssetDrop.ts
export function useInternalAssetDrop(episodeId: string) {
  const [isInternalDragOver, setOver] = useState(false);
  const { screenToFlowPosition } = useReactFlow();
  const addNode = useCanvasStore((s) => s.addNode);

  // 监听全局 dragover/dragleave 判断我们的 MIME
  useEffect(() => {
    const onDragOver = (e: DragEvent) => {
      if (e.dataTransfer?.types.includes("application/x-mango-asset-id")) setOver(true);
    };
    const onDragLeave = () => setOver(false);
    document.addEventListener("dragover", onDragOver);
    document.addEventListener("dragleave", onDragLeave);
    document.addEventListener("drop", onDragLeave);
    return () => {
      document.removeEventListener("dragover", onDragOver);
      document.removeEventListener("dragleave", onDragLeave);
      document.removeEventListener("drop", onDragLeave);
    };
  }, []);

  const onDrop = useCallback(async (e: React.DragEvent) => {
    e.preventDefault();
    setOver(false);
    const assetId = e.dataTransfer.getData("application/x-mango-asset-id");
    if (!assetId) return;

    const position = screenToFlowPosition({ x: e.clientX, y: e.clientY });

    // 判断是否拖在了 StoryboardNode 上
    const targetShotNode = findNodeAtPosition(position, "storyboard");
    if (targetShotNode) {
      openBindAssetToShotDialog({ assetId, shotId: targetShotNode.data.shotId });
      return;
    }

    // 拖在空白处 → 创建 AssetNode
    addNode(createAssetNode(assetId, position));
  }, [screenToFlowPosition, addNode]);

  return { onDrop, isInternalDragOver };
}
```

> **设计说明**：`screenToFlowPosition` 是 React Flow 12 的 hook，自动处理缩放/平移变换。`findNodeAtPosition` 用 store 的 nodes 做 hit-test（节点 width/height 估算 + position 矩形）；React Flow 内置的 `getIntersectingNodes` 也可以替代，但需要 ref 当前画布元素。本 spec 用简单矩形 hit-test 即可，节点 size 取 NodeShell 的 default min-width/height。

#### BindAssetToShotDialog

```tsx
export function BindAssetToShotDialog({ assetId, shotId, onClose }: Props) {
  const asset = useAsset(assetId);
  const shot = useShot(shotId);
  const updateShot = useUpdateShot();
  const updateAsset = useUpdateAsset(); // MS1 spec-12 已有

  const bindAsReference = async () => {
    // shot.image_prompt 不动；但 asset 关联到 shot
    await updateAsset.mutateAsync({ id: assetId, shot_id: shotId });
    toast.success("已绑定为分镜参考素材");
    onClose();
  };

  const adoptAsResult = async () => {
    // 把 asset 作为这个 shot 的"采用结果" —— V2 才会有 shot.result_asset_id 字段
    // 本 spec：仅写 asset.shot_id 关联，UI 在 ShotCard 上显示该 asset 即可
    await updateAsset.mutateAsync({ id: assetId, shot_id: shotId });
    toast.success("已采用为分镜结果");
    onClose();
  };

  if (asset.data?.asset_type === "image") {
    return <Dialog>...只显示「绑定为参考图」按钮...</Dialog>;
  }
  if (asset.data?.asset_type === "video") {
    return <Dialog>...只显示「采用为视频结果」按钮...</Dialog>;
  }
  return null;
}
```

> **设计说明**：当前 schema 下「参考图」和「采用结果」**最小可行实现都是 `update asset.shot_id`**——asset 表已有 `shot_id` 列。区分的是 UI 上的语义提示与 toast 文案。MS4 引入 `shot.result_asset_id` 列后，"采用"才会真正写一个独立字段。把这个语义差异透明地暴露给用户（弹窗按钮文案不同）是为了让画布交互语义自洽，但后端短期内是同一动作。

### 系统文件拖入

#### useSystemFileDrop hook

```ts
// src/components/canvas/drag/useSystemFileDrop.ts
const ACCEPTED_EXTENSIONS = ["png", "jpg", "jpeg", "webp", "mp4", "mov", "webm"];

export function useSystemFileDrop(episodeId: string) {
  const [isSystemDragOver, setOver] = useState(false);
  const { screenToFlowPosition } = useReactFlow();
  const addNode = useCanvasStore((s) => s.addNode);
  const importAsset = useImportAsset(); // MS1 spec-12 已有 useImportAsset

  useEffect(() => {
    const w = getCurrentWebviewWindow();
    let unlisten: UnlistenFn | undefined;

    w.onDragDropEvent(async (event) => {
      const payload = event.payload;

      if (payload.type === "enter" || payload.type === "over") {
        setOver(true);
        return;
      }
      if (payload.type === "leave") {
        setOver(false);
        return;
      }
      if (payload.type === "drop") {
        setOver(false);
        const { paths, position } = payload;

        // 过滤受支持的文件
        const accepted = paths.filter((p) => {
          const ext = p.split(".").pop()?.toLowerCase();
          return ext && ACCEPTED_EXTENSIONS.includes(ext);
        });
        if (accepted.length === 0) {
          toast.error("不支持的文件类型。仅支持 PNG / JPG / WEBP / MP4 / MOV / WEBM");
          return;
        }
        if (paths.length > accepted.length) {
          toast.warning(`已忽略 ${paths.length - accepted.length} 个不支持的文件`);
        }

        // 物理像素 → DOM 像素 → ReactFlow 坐标
        const dpr = window.devicePixelRatio;
        const screenPos = { x: position.x / dpr, y: position.y / dpr };
        const flowPos = screenToFlowPosition(screenPos);

        for (const [idx, path] of accepted.entries()) {
          try {
            const asset = await importAsset.mutateAsync({ project_id: getProjectId(), source_path: path });
            addNode(createAssetNode(asset.id, { x: flowPos.x + idx * 20, y: flowPos.y + idx * 20 }));
          } catch (e) {
            toast.error(`导入 ${path.split("/").pop()} 失败：${e}`);
          }
        }
      }
    }).then((fn) => { unlisten = fn; });

    return () => { unlisten?.(); };
  }, [episodeId, screenToFlowPosition, addNode, importAsset]);

  return { isSystemDragOver };
}
```

> **设计说明**：
>
> 1. **坐标换算**：Tauri `onDragDropEvent` 的 position 是**物理像素**（受 devicePixelRatio 影响），需要除以 dpr 转 DOM 像素后再喂给 `screenToFlowPosition`。Retina 屏不做这一步会偏移 2x
> 2. **多文件递增偏移**：避免多文件全堆叠在一点，每个后续节点偏移 20px
> 3. **拖到分镜上的绑定**：系统拖入路径**不**做"拖到分镜节点绑定"的副作用——理由是系统拖入后 asset 是全新的，用户更可能想先看一眼再决定绑哪个 shot。如果需求强烈再加
> 4. **import_asset 自带去重**：spec-12 的 import_asset 用 content_hash 去重，重复拖入同一文件不会产生 2 个 asset 行——但会产生 2 个 AssetNode（指向同一 asset），用户应能从画布上手动删

### AssetDrawer

```tsx
// AssetDrawer.tsx
export function AssetDrawer({ projectId }: { projectId: string }) {
  const [open, setOpen] = useState(false);
  const assets = useAssetList(projectId);

  return (
    <>
      <Button
        size="icon" variant="outline"
        className="absolute right-4 top-4 z-10"
        onClick={() => setOpen(!open)}
      >
        <FolderOpen className="h-4 w-4" />
      </Button>
      <aside
        className={cn(
          "absolute right-0 top-0 h-full w-80 border-l bg-background shadow-lg transition-transform z-20",
          open ? "translate-x-0" : "translate-x-full",
        )}
      >
        <header className="flex items-center justify-between border-b p-3">
          <h3 className="font-medium">素材库</h3>
          <Button size="icon" variant="ghost" onClick={() => setOpen(false)}>×</Button>
        </header>
        <AssetFilterBar value={filter} onChange={setFilter} /> {/* 复用 MS1 已有组件 */}
        <div className="grid grid-cols-2 gap-2 overflow-auto p-3">
          {assets.data?.map((a) => <AssetCard key={a.id} asset={a} />)}
        </div>
      </aside>
    </>
  );
}
```

> **设计说明**：抽屉默认收起，避免遮挡画布。打开时占右侧 320px。AssetCard 的 draggable 已加，从抽屉拖出画布即触发上面的 `onInternalDrop`。

### DropTargetOverlay

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

### 错误场景

| 场景 | 行为 |
|---|---|
| 系统拖入文件名含 `..` 或路径穿越 | `import_asset` 内部 canonical path 校验拒绝；toast 错误 |
| 系统拖入 GIF | 当前 ACCEPTED_EXTENSIONS 不含 gif，toast 提示；后续可加 |
| 系统拖入超大文件（> 1GB） | `import_asset` 当前未实现 size 检查；本 spec 范围内**不加**，由 MS5 视频后期再统一加 size 限制 |
| 内部拖到自身（AssetCard 拖到它自己生成的 AssetNode） | onDrop 时检查目标 hit 节点是否已经是同 assetId 的 AssetNode → 是则忽略 + toast「该素材已在画布上」 |
| Tauri webview 还未 ready 时 mount hook | `getCurrentWebviewWindow()` 应安全返回；onDragDropEvent 的 Promise<UnlistenFn> 在 ready 后才决议；中间用户操作的拖入事件会丢失，但这是首屏 < 100ms 的窄窗口，可接受 |
| 多个 CanvasInner 实例同时 mount（理论不应发生） | 每个 hook 都 listen 同一个 webview event；都会触发 import + addNode → 重复节点。**约束：CanvasPage 单实例**，由路由保证 |

### 测试策略

#### 前端单元测试

`src/components/canvas/drag/useInternalAssetDrop.test.ts`：
- 触发 dragover 含我们的 MIME → isInternalDragOver=true
- onDrop 在空白处 → addNode 被调用 + 节点 data.assetId 正确
- onDrop 命中 storyboard 节点 → 打开 BindAssetToShotDialog

`src/components/canvas/drag/useSystemFileDrop.test.ts`：
- mock `getCurrentWebviewWindow().onDragDropEvent`，触发 "drop" 事件 + paths 含 .png
- 验证 importAsset.mutateAsync 被调用 + addNode 被调用
- paths 全部不支持时 toast 错误，不调 import
- 部分不支持时 toast warning + 仅处理支持的文件

`src/components/canvas/drag/BindAssetToShotDialog.test.tsx`：
- asset_type=image 时只显示「绑定为参考图」按钮
- asset_type=video 时只显示「采用为视频结果」按钮
- 点击按钮触发 updateAsset.mutateAsync

`src/components/canvas/drag/AssetDrawer.test.tsx`：
- 默认收起；点 toggle 按钮展开
- 渲染 AssetCard 列表（mock useAssetList）

#### 手动验证清单

- [ ] 打开画布 → 右上角 FolderOpen 图标点开 → 看到 AssetDrawer 滑出 + 素材列表
- [ ] 从 AssetDrawer 拖一个图片到画布空白 → 创建 AssetNode + drawer 自动关闭（可选）
- [ ] 拖一个 image asset 到 StoryboardNode 上 → 弹窗「绑定为参考图」→ 确认 → DB 查 `SELECT shot_id FROM asset WHERE id=...` 已更新
- [ ] 从 Finder 拖一个 .png 到画布 → 画布显示蓝色边框高亮 + "松开以添加到画布" → 松开 → import 完成 + AssetNode 出现
- [ ] 从 Finder 拖一个 .txt 到画布 → toast 错误，不创建节点
- [ ] 从 Finder 拖多个 .png（5 个）到画布 → 5 个 AssetNode 错位排列
- [ ] 拖入相同文件两次 → DB 只多 1 行 asset（去重）+ 画布多 2 个 AssetNode（同 assetId 不同 nodeId？）—— **本 spec 决议**：同 assetId 在画布上唯一（createAssetNode 用 `asset-${assetId}` 作 nodeId）；第二次拖入提示「该素材已在画布上」
- [ ] Retina 屏验证：拖入位置与鼠标松开位置一致（无 2x 偏移）

## 验收标准

- [ ] AssetCard 加 `draggable` + dataTransfer 设 `application/x-mango-asset-id`
- [ ] useInternalAssetDrop / useSystemFileDrop 两个 hook 完整实现
- [ ] DropTargetOverlay 在拖动时显示蓝色高亮
- [ ] AssetDrawer 右侧滑入滑出，复用 `AssetGrid` / `AssetFilterBar`
- [ ] BindAssetToShotDialog 按 asset_type 区分按钮文案
- [ ] 系统文件拖入物理像素 → 物理像素 / DPR → flow 坐标 换算正确
- [ ] 多文件批量拖入 + 不支持文件类型 + 重复拖入 三种边界正确处理
- [ ] `pnpm test:run` 至少新增 10 个前端测试
- [ ] `pnpm typecheck` 干净
- [ ] 手动验证清单全过

## 依赖关系

- **前置**：spec-22（需要 createAssetNode factory + AssetNode 组件 + StoryboardNode 用作拖放目标）
- **平行**：spec-24（拖放产生的 addNode 必须被 zundo 捕获，但 zundo middleware 由 spec-24 加，PR 顺序确保即可）
- **后续**：无

## 与原任务文档的差异

合并自原任务 8（素材面板 → 画布拖放）+ 任务 9（系统文件 → 画布拖放）。

- **原任务 8 提到"素材面板"** —— 本 spec 把"面板"具体化为 `AssetDrawer` 右侧滑出，因为画布页本身没有侧边栏空间放完整 panel
- **原任务 8 提到"拖放到分镜节点上：绑定为该分镜的参考图/采用视频"** —— 本 spec 引入 `BindAssetToShotDialog` 二次确认，避免误操作；当前 schema 下两种行为后端是同一动作（更新 `asset.shot_id`），UI 上用文案区分
- **原任务 9 没提坐标换算 / dpr 处理** —— Tauri `onDragDropEvent` 的 position 是物理像素，本 spec 明确写出 DPR 换算逻辑
- **原任务 9 没提"是否替换"** —— 本 spec 决策：拖入相同文件 import_asset 自动去重（已是 spec-12 行为），同 assetId 在画布上唯一（createAssetNode 用稳定 id）
