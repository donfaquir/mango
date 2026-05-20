# SPEC-08: 基础 App Shell UI

## 概述

实现应用的整体布局框架：左侧边栏 + 顶部导航 + 主内容区，配置 React Router 路由，建立 Zustand UI 状态管理和 TanStack Query 数据获取模式。这是所有页面的容器骨架。

## 技术方案

### 目录/文件结构

```
src/
├── App.tsx                     # 根组件 — Router 配置
├── main.tsx                    # 入口 — Provider 包裹
├── app.css                     # 全局样式
├── routes.tsx                  # 路由表定义
├── components/
│   ├── ui/                     # shadcn/ui 组件
│   │   ├── button.tsx
│   │   ├── tooltip.tsx
│   │   └── separator.tsx
│   └── layout/
│       ├── AppLayout.tsx       # 整体布局容器
│       ├── Sidebar.tsx         # 左侧边栏
│       ├── Header.tsx          # 顶部导航
│       └── Breadcrumb.tsx      # 面包屑
├── stores/
│   └── ui-store.ts             # UI 状态（Zustand）
├── hooks/
│   └── useProjects.ts          # 项目数据 hooks（TanStack Query）
└── lib/
    ├── utils.ts                # cn() 等工具
    └── bindings/               # 自动生成的类型
```

### 依赖项

shadcn/ui 组件（通过 CLI 安装）：
```bash
pnpm dlx shadcn@latest add button tooltip separator scroll-area
```

已安装（SPEC-02）：
- react-router-dom ^7
- @tanstack/react-query ^5
- zustand ^5

新增图标库：
```bash
pnpm add lucide-react
```

### 核心实现

#### 路由表（routes.tsx）

```typescript
import { createBrowserRouter } from "react-router-dom";
import { AppLayout } from "@/components/layout/AppLayout";

// 懒加载页面
const ProjectListPage = lazy(() => import("@/pages/ProjectListPage"));
const ProjectWorkspacePage = lazy(() => import("@/pages/ProjectWorkspacePage"));
const SettingsPage = lazy(() => import("@/pages/SettingsPage"));

export const router = createBrowserRouter([
  {
    path: "/",
    element: <AppLayout />,
    children: [
      {
        index: true,
        element: <ProjectListPage />,
      },
      {
        path: "project/:projectId",
        element: <ProjectWorkspacePage />,
      },
      {
        path: "settings",
        element: <SettingsPage />,
      },
    ],
  },
]);
```

#### App.tsx

```typescript
import { RouterProvider } from "react-router-dom";
import { router } from "./routes";

function App() {
  return <RouterProvider router={router} />;
}

export default App;
```

#### 整体布局（components/layout/AppLayout.tsx）

```typescript
import { Outlet } from "react-router-dom";
import { Sidebar } from "./Sidebar";
import { Header } from "./Header";
import { useUIStore } from "@/stores/ui-store";

export function AppLayout() {
  const sidebarCollapsed = useUIStore((s) => s.sidebarCollapsed);

  return (
    <div className="flex h-screen overflow-hidden">
      {/* 侧边栏 */}
      <Sidebar collapsed={sidebarCollapsed} />

      {/* 主内容区 */}
      <div className="flex flex-1 flex-col overflow-hidden">
        <Header />
        <main className="flex-1 overflow-auto p-6">
          <Suspense fallback={<LoadingSpinner />}>
            <Outlet />
          </Suspense>
        </main>
      </div>
    </div>
  );
}
```

#### 侧边栏（components/layout/Sidebar.tsx）

```typescript
import { Link, useLocation } from "react-router-dom";
import { useUIStore } from "@/stores/ui-store";
import { FolderOpen, Settings, PanelLeftClose, PanelLeftOpen } from "lucide-react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

interface SidebarProps {
  collapsed: boolean;
}

export function Sidebar({ collapsed }: SidebarProps) {
  const location = useLocation();
  const toggleSidebar = useUIStore((s) => s.toggleSidebar);

  const navItems = [
    { path: "/", label: "项目", icon: FolderOpen },
    { path: "/settings", label: "设置", icon: Settings },
  ];

  return (
    <aside
      className={cn(
        "flex flex-col border-r bg-muted/30 transition-all duration-200",
        collapsed ? "w-16" : "w-60"
      )}
    >
      {/* Logo / 折叠按钮 */}
      <div className="flex h-14 items-center justify-between px-4">
        {!collapsed && <span className="text-lg font-bold">Mango</span>}
        <Button variant="ghost" size="icon" onClick={toggleSidebar}>
          {collapsed ? <PanelLeftOpen size={18} /> : <PanelLeftClose size={18} />}
        </Button>
      </div>

      {/* 导航项 */}
      <nav className="flex-1 space-y-1 px-2 py-4">
        {navItems.map((item) => (
          <Link
            key={item.path}
            to={item.path}
            className={cn(
              "flex items-center gap-3 rounded-md px-3 py-2 text-sm transition-colors",
              "hover:bg-muted",
              location.pathname === item.path && "bg-muted font-medium"
            )}
          >
            <item.icon size={18} />
            {!collapsed && <span>{item.label}</span>}
          </Link>
        ))}
      </nav>
    </aside>
  );
}
```

#### 顶部导航（components/layout/Header.tsx）

```typescript
import { useLocation } from "react-router-dom";

export function Header() {
  const location = useLocation();

  const getTitle = () => {
    if (location.pathname === "/") return "项目列表";
    if (location.pathname === "/settings") return "设置";
    if (location.pathname.startsWith("/project/")) return "项目工作区";
    return "Mango";
  };

  return (
    <header className="flex h-14 items-center border-b px-6">
      <h1 className="text-lg font-medium">{getTitle()}</h1>
    </header>
  );
}
```

#### UI 状态管理（stores/ui-store.ts）

```typescript
import { create } from "zustand";
import { persist } from "zustand/middleware";

interface UIState {
  sidebarCollapsed: boolean;
  toggleSidebar: () => void;
  setSidebarCollapsed: (collapsed: boolean) => void;
}

export const useUIStore = create<UIState>()(
  persist(
    (set) => ({
      sidebarCollapsed: false,
      toggleSidebar: () => set((s) => ({ sidebarCollapsed: !s.sidebarCollapsed })),
      setSidebarCollapsed: (collapsed) => set({ sidebarCollapsed: collapsed }),
    }),
    {
      name: "mango-ui-state",
    }
  )
);
```

#### 数据获取 Hook 模式（hooks/useProjects.ts）

```typescript
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { listProjects, createProject, deleteProject } from "@/lib/bindings/commands";
import type { CreateProjectInput } from "@/lib/bindings/commands";

export const projectKeys = {
  all: ["projects"] as const,
  detail: (id: string) => ["projects", id] as const,
};

export function useProjectList() {
  return useQuery({
    queryKey: projectKeys.all,
    queryFn: listProjects,
  });
}

export function useCreateProject() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (input: CreateProjectInput) => createProject(input),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: projectKeys.all });
    },
  });
}

export function useDeleteProject() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => deleteProject(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: projectKeys.all });
    },
  });
}
```

#### 占位页面

```typescript
// src/pages/ProjectListPage.tsx — 详见 SPEC-09
// src/pages/ProjectWorkspacePage.tsx
export default function ProjectWorkspacePage() {
  return <div className="text-muted-foreground">项目工作区（待实现）</div>;
}

// src/pages/SettingsPage.tsx
export default function SettingsPage() {
  return <div className="text-muted-foreground">设置页面（待实现）</div>;
}
```

## 接口定义

**Zustand Store API：**
```typescript
useUIStore.getState().sidebarCollapsed  // boolean
useUIStore.getState().toggleSidebar()    // void
useUIStore.getState().setSidebarCollapsed(true) // void
```

**数据 Hooks API：**
```typescript
useProjectList()   → { data: Project[], isLoading, error }
useCreateProject() → { mutate, mutateAsync, isPending }
useDeleteProject() → { mutate, mutateAsync, isPending }
```

**组件 Props：**
```typescript
<AppLayout />          // 无 props，通过 Outlet 渲染子路由
<Sidebar collapsed />  // collapsed: boolean
<Header />             // 无 props，读取 location
```

## 验收标准

```bash
# 1. 应用启动
pnpm tauri dev
# → 桌面窗口显示完整布局（侧边栏 + 导航 + 内容区）

# 2. 侧边栏折叠/展开
# → 点击折叠按钮，侧边栏收起为图标模式
# → 再次点击展开
# → 刷新后状态保持（localStorage 持久化）

# 3. 路由导航
# → 点击"项目"导航到 /
# → 点击"设置"导航到 /settings
# → 页面切换无白屏、无闪烁

# 4. 响应式
# → 窗口缩放至 960px 宽度，布局不溢出不错位

# 5. 数据加载状态
# → 项目列表加载时显示 loading 状态
# → 后端未启动时显示 error 状态

# 6. TypeScript 编译
npx tsc --noEmit
# → 无类型错误
```

## 依赖关系

- **前置**：
  - SPEC-02（前端工程 + 组件库 + 状态管理依赖）
  - SPEC-07（类型绑定可用）
- **后续**：
  - SPEC-09（项目管理页面）在此布局内实现
  - MS1+ 所有新页面都在 AppLayout 内添加路由
