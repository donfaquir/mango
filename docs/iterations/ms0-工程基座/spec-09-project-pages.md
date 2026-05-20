# SPEC-09: 项目管理页面

## 概述

实现项目列表展示、创建项目弹窗、项目卡片点击进入工作区的完整 UI 流程。这是用户与应用交互的第一个业务页面，验证前后端 IPC 全链路打通。

## 技术方案

### 目录/文件结构

```
src/
├── pages/
│   └── ProjectListPage.tsx         # 项目列表页
├── components/
│   ├── project/
│   │   ├── ProjectCard.tsx         # 项目卡片组件
│   │   ├── CreateProjectDialog.tsx # 创建项目弹窗
│   │   └── EmptyState.tsx          # 空状态引导
│   └── ui/
│       ├── dialog.tsx              # shadcn dialog（需安装）
│       ├── input.tsx               # shadcn input
│       ├── label.tsx               # shadcn label
│       └── textarea.tsx            # shadcn textarea
└── hooks/
    └── useProjects.ts              # 已在 SPEC-08 定义
```

### 依赖项

新增 shadcn/ui 组件：
```bash
pnpm dlx shadcn@latest add dialog input label textarea card
```

### 核心实现

#### 项目列表页（pages/ProjectListPage.tsx）

```typescript
import { useState } from "react";
import { useProjectList } from "@/hooks/useProjects";
import { ProjectCard } from "@/components/project/ProjectCard";
import { CreateProjectDialog } from "@/components/project/CreateProjectDialog";
import { EmptyState } from "@/components/project/EmptyState";
import { Button } from "@/components/ui/button";
import { Plus } from "lucide-react";

export default function ProjectListPage() {
  const [createDialogOpen, setCreateDialogOpen] = useState(false);
  const { data: projects, isLoading, error } = useProjectList();

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-64">
        <p className="text-muted-foreground">加载中...</p>
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex items-center justify-center h-64">
        <p className="text-destructive">加载失败: {error.message}</p>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {/* 页面头部 */}
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-2xl font-bold">我的项目</h2>
          <p className="text-muted-foreground">
            {projects?.length ?? 0} 个项目
          </p>
        </div>
        <Button onClick={() => setCreateDialogOpen(true)}>
          <Plus size={16} className="mr-2" />
          新建项目
        </Button>
      </div>

      {/* 项目列表 */}
      {projects && projects.length > 0 ? (
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3">
          {projects.map((project) => (
            <ProjectCard key={project.id} project={project} />
          ))}
        </div>
      ) : (
        <EmptyState onCreateClick={() => setCreateDialogOpen(true)} />
      )}

      {/* 创建弹窗 */}
      <CreateProjectDialog
        open={createDialogOpen}
        onOpenChange={setCreateDialogOpen}
      />
    </div>
  );
}
```

#### 项目卡片（components/project/ProjectCard.tsx）

```typescript
import { useNavigate } from "react-router-dom";
import { Card, CardHeader, CardTitle, CardDescription, CardContent } from "@/components/ui/card";
import type { Project } from "@/lib/bindings/commands";
import { FolderOpen } from "lucide-react";

interface ProjectCardProps {
  project: Project;
}

export function ProjectCard({ project }: ProjectCardProps) {
  const navigate = useNavigate();

  const handleClick = () => {
    navigate(`/project/${project.id}`);
  };

  // 格式化时间
  const formatDate = (dateStr: string) => {
    const date = new Date(dateStr);
    return date.toLocaleDateString("zh-CN", {
      year: "numeric",
      month: "short",
      day: "numeric",
    });
  };

  return (
    <Card
      className="cursor-pointer transition-shadow hover:shadow-md"
      onClick={handleClick}
    >
      <CardHeader className="pb-3">
        <div className="flex items-center gap-2">
          <FolderOpen size={18} className="text-primary" />
          <CardTitle className="text-base">{project.name}</CardTitle>
        </div>
        {project.description && (
          <CardDescription className="line-clamp-2">
            {project.description}
          </CardDescription>
        )}
      </CardHeader>
      <CardContent>
        <p className="text-xs text-muted-foreground">
          创建于 {formatDate(project.created_at)}
        </p>
      </CardContent>
    </Card>
  );
}
```

#### 创建项目弹窗（components/project/CreateProjectDialog.tsx）

```typescript
import { useState } from "react";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Textarea } from "@/components/ui/textarea";
import { useCreateProject } from "@/hooks/useProjects";

interface CreateProjectDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function CreateProjectDialog({ open, onOpenChange }: CreateProjectDialogProps) {
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [stylePrompt, setStylePrompt] = useState("");

  const createProject = useCreateProject();

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim()) return;

    await createProject.mutateAsync({
      name: name.trim(),
      description: description || null,
      style_prompt: stylePrompt || null,
      global_seed: null,
    });

    // 重置表单并关闭
    setName("");
    setDescription("");
    setStylePrompt("");
    onOpenChange(false);
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-[480px]">
        <DialogHeader>
          <DialogTitle>新建项目</DialogTitle>
        </DialogHeader>

        <form onSubmit={handleSubmit} className="space-y-4">
          <div className="space-y-2">
            <Label htmlFor="name">项目名称 *</Label>
            <Input
              id="name"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder="如：我的第一部漫剧"
              autoFocus
            />
          </div>

          <div className="space-y-2">
            <Label htmlFor="description">项目描述</Label>
            <Textarea
              id="description"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              placeholder="简要描述项目内容..."
              rows={3}
            />
          </div>

          <div className="space-y-2">
            <Label htmlFor="style">风格提示词</Label>
            <Textarea
              id="style"
              value={stylePrompt}
              onChange={(e) => setStylePrompt(e.target.value)}
              placeholder="如：赛博朋克风格，霓虹灯光，暗色调..."
              rows={2}
            />
          </div>

          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              onClick={() => onOpenChange(false)}
            >
              取消
            </Button>
            <Button
              type="submit"
              disabled={!name.trim() || createProject.isPending}
            >
              {createProject.isPending ? "创建中..." : "创建"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
```

#### 空状态（components/project/EmptyState.tsx）

```typescript
import { Button } from "@/components/ui/button";
import { FolderPlus } from "lucide-react";

interface EmptyStateProps {
  onCreateClick: () => void;
}

export function EmptyState({ onCreateClick }: EmptyStateProps) {
  return (
    <div className="flex flex-col items-center justify-center rounded-lg border-2 border-dashed p-12">
      <FolderPlus size={48} className="text-muted-foreground/50" />
      <h3 className="mt-4 text-lg font-medium">还没有项目</h3>
      <p className="mt-2 text-sm text-muted-foreground">
        创建你的第一个漫剧项目，开始创作之旅
      </p>
      <Button className="mt-6" onClick={onCreateClick}>
        新建项目
      </Button>
    </div>
  );
}
```

## 接口定义

**组件 Props：**
```typescript
// ProjectCard
interface ProjectCardProps {
  project: Project;
}

// CreateProjectDialog
interface CreateProjectDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

// EmptyState
interface EmptyStateProps {
  onCreateClick: () => void;
}
```

**页面路由：**
- `/` → ProjectListPage（项目列表）
- `/project/:projectId` → ProjectWorkspacePage（项目工作区，本迭代为空壳）

**数据流：**
```
ProjectListPage
  → useProjectList() [TanStack Query]
    → listProjects() [specta binding]
      → invoke("list_projects") [Tauri IPC]
        → project_queries::list() [core crate]
          → SQLite SELECT
```

## 验收标准

```bash
# 1. 应用启动后显示项目列表页
pnpm tauri dev
# → 首次打开显示空状态引导

# 2. 创建项目
# → 点击"新建项目"按钮
# → 弹窗中填写名称"我的第一部漫剧"
# → 点击"创建"
# → 弹窗关闭，列表中立即显示新项目卡片

# 3. 列表展示
# → 创建多个项目后，按时间倒序展示
# → 卡片显示名称、描述、创建时间

# 4. 点击进入工作区
# → 点击项目卡片，路由跳转到 /project/:id
# → 显示"项目工作区（待实现）"占位文本

# 5. 表单校验
# → 名称为空时"创建"按钮禁用
# → 只填名称也能成功创建（其余字段可选）

# 6. 加载/错误状态
# → 网络慢时显示"加载中..."
# → 后端异常时显示错误信息

# 7. CLI 数据共享
cargo run -p mango-cli -- project list
# → 显示在 GUI 中创建的项目（验证共享同一 DB）

# 8. TypeScript 编译
npx tsc --noEmit
# → 无类型错误
```

## 依赖关系

- **前置**：
  - SPEC-02（前端工程 + 组件库）
  - SPEC-05（Tauri IPC 可用）
  - SPEC-07（类型绑定已生成）
  - SPEC-08（App Shell 布局 + 路由 + hooks）
- **后续**：
  - MS1 中的主体管理页面参考此组件模式
  - ProjectWorkspacePage 在 MS2/MS3 中填充实际内容
