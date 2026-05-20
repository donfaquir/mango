# SPEC-02: 初始化前端工程

## 概述

初始化前端技术栈：Vite 8 + React 19 + TypeScript 5.9 + Tailwind CSS 4 + shadcn/ui，并配置 TanStack Query 数据层和 Vitest 测试环境。前端代码位于项目根目录的 `src/` 下，通过 Tauri WebView 加载。

## 技术方案

### 目录/文件结构

```
mango/
├── package.json
├── pnpm-lock.yaml
├── vite.config.ts
├── tsconfig.json                # 统一 tsconfig（含 src + config 文件）
├── vitest.config.ts
├── components.json              # shadcn/ui 配置
├── index.html
├── public/
│   └── vite.svg
├── src/
│   ├── main.tsx                 # React 入口
│   ├── App.tsx                  # 根组件
│   ├── app.css                  # Tailwind CSS 入口（CSS-first 模式）
│   ├── vite-env.d.ts
│   ├── components/
│   │   └── ui/                  # shadcn/ui 组件输出目录
│   ├── lib/
│   │   ├── utils.ts             # cn() 等工具函数
│   │   └── bindings/            # tauri-specta / ts-rs 生成目录
│   └── test/
│       └── setup.ts             # Vitest 全局 setup
└── (不使用 tsconfig.node.json — 合并到 tsconfig.json 以避免 composite/noEmit 冲突)
```

### 依赖项

**dependencies：**
| 包名 | 版本 | 用途 |
|------|------|------|
| react | ^19.0.0 | UI 框架 |
| react-dom | ^19.0.0 | DOM 渲染 |
| react-router-dom | ^7.0.0 | 路由 |
| @tanstack/react-query | ^5.0.0 | 数据获取/缓存 |
| zustand | ^5.0.0 | UI 状态管理 |
| @tauri-apps/api | ^2.0.0 | Tauri 前端 API |
| @tauri-apps/plugin-shell | ^2.0.0 | Shell 插件 |
| clsx | ^2.0.0 | 条件 class 拼接 |
| tailwind-merge | ^3.0.0 | Tailwind class 合并 |
| class-variance-authority | ^0.7.0 | 组件变体管理（shadcn/ui 依赖） |
| lucide-react | ^0.500.0 | 图标库 |

**devDependencies：**
| 包名 | 版本 | 用途 |
|------|------|------|
| vite | ^8.0.0 | 构建工具（Rolldown 打包器） |
| @vitejs/plugin-react | ^6.0.0 | React 支持（Vite 8 兼容） |
| typescript | ^5.8.0 | 类型系统 |
| @types/react | ^19.0.0 | React 类型 |
| @types/react-dom | ^19.0.0 | ReactDOM 类型 |
| @types/node | ^25.0.0 | Node.js 类型（vite config 需要） |
| tailwindcss | ^4.0.0 | 样式框架 |
| @tailwindcss/vite | ^4.0.0 | Vite 插件 |
| vitest | ^4.0.0 | 测试框架（Vite 8 兼容） |
| @testing-library/react | ^16.0.0 | 组件测试 |
| @testing-library/jest-dom | ^6.0.0 | DOM 断言 |
| jsdom | ^25.0.0 | 测试环境 |
| @tauri-apps/cli | ^2.0.0 | Tauri CLI |

### 核心实现

**package.json：**
```json
{
  "name": "mango",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "build": "tsc -b && vite build",
    "preview": "vite preview",
    "test": "vitest",
    "test:run": "vitest run",
    "typecheck": "tsc --noEmit",
    "tauri": "tauri"
  }
}
```

**vite.config.ts：**
```typescript
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import path from "path";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
  },
  envPrefix: ["VITE_", "TAURI_"],
  build: {
    target: ["es2021", "chrome100", "safari13"],
    minify: !process.env.TAURI_DEBUG ? "esbuild" : false,
    sourcemap: !!process.env.TAURI_DEBUG,
  },
});
```

**tsconfig.json（统一配置，不使用 project references）：**
```json
{
  "compilerOptions": {
    "target": "ES2021",
    "useDefineForClassFields": true,
    "lib": ["ES2021", "DOM", "DOM.Iterable"],
    "module": "ESNext",
    "skipLibCheck": true,
    "moduleResolution": "bundler",
    "allowImportingTsExtensions": true,
    "isolatedModules": true,
    "moduleDetection": "force",
    "noEmit": true,
    "jsx": "react-jsx",
    "strict": true,
    "noUnusedLocals": true,
    "noUnusedParameters": true,
    "noFallthroughCasesInSwitch": true,
    "paths": {
      "@/*": ["./src/*"]
    }
  },
  "include": ["src", "vite.config.ts", "vitest.config.ts"]
}
```

> 注：不使用 `tsconfig.node.json` + `references` 模式，因为 TypeScript 的 `composite: true` 与 `noEmit: true` 存在冲突。统一 tsconfig 配合 `skipLibCheck: true` 即可满足需求。

**src/app.css（Tailwind CSS 4 CSS-first 模式 + shadcn/ui 完整主题变量）：**
```css
@import "tailwindcss";

@theme {
  --color-background: #ffffff;
  --color-foreground: #0a0a0a;
  --color-primary: #6366f1;
  --color-primary-foreground: #ffffff;
  --color-secondary: #f5f5f5;
  --color-secondary-foreground: #171717;
  --color-muted: #f5f5f5;
  --color-muted-foreground: #737373;
  --color-accent: #f5f5f5;
  --color-accent-foreground: #171717;
  --color-destructive: #ef4444;
  --color-destructive-foreground: #ffffff;
  --color-border: #e5e5e5;
  --color-input: #e5e5e5;
  --color-ring: #6366f1;
  --color-card: #ffffff;
  --color-card-foreground: #0a0a0a;
  --color-popover: #ffffff;
  --color-popover-foreground: #0a0a0a;
  --radius-sm: 0.25rem;
  --radius-md: 0.375rem;
  --radius-lg: 0.5rem;
}

body {
  font-family:
    -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue",
    Arial, sans-serif;
}
```

**src/main.tsx：**
```typescript
import React from "react";
import ReactDOM from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import App from "./App";
import "./app.css";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: 1,
      staleTime: 5 * 60 * 1000,
    },
  },
});

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <QueryClientProvider client={queryClient}>
      <App />
    </QueryClientProvider>
  </React.StrictMode>,
);
```

**src/App.tsx（初始骨架）：**
```typescript
function App() {
  return (
    <div className="flex min-h-screen items-center justify-center bg-background text-foreground">
      <div className="text-center">
        <h1 className="text-3xl font-bold text-primary">Mango</h1>
        <p className="mt-2 text-muted-foreground">
          漫剧创作工作站 - 工程基座初始化完成
        </p>
      </div>
    </div>
  );
}

export default App;
```

**src/lib/utils.ts：**
```typescript
import { type ClassValue, clsx } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}
```

**vitest.config.ts：**
```typescript
import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import path from "path";

export default defineConfig({
  plugins: [react()],
  test: {
    globals: true,
    environment: "jsdom",
    setupFiles: ["./src/test/setup.ts"],
  },
  resolve: {
    alias: {
      "@": path.resolve(__dirname, "./src"),
    },
  },
});
```

**src/test/setup.ts：**
```typescript
import "@testing-library/jest-dom/vitest";
```

**components.json（shadcn/ui 配置）：**
```json
{
  "$schema": "https://ui.shadcn.com/schema.json",
  "style": "new-york",
  "rsc": false,
  "tsx": true,
  "tailwind": {
    "config": "",
    "css": "src/app.css",
    "baseColor": "neutral",
    "cssVariables": true
  },
  "aliases": {
    "components": "@/components",
    "utils": "@/lib/utils",
    "ui": "@/components/ui",
    "lib": "@/lib",
    "hooks": "@/hooks"
  }
}
```

**index.html：**
```html
<!doctype html>
<html lang="zh-CN">
  <head>
    <meta charset="UTF-8" />
    <link rel="icon" type="image/svg+xml" href="/vite.svg" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Mango</title>
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="/src/main.tsx"></script>
  </body>
</html>
```

### 配置

初始化完成后安装 shadcn/ui Button 组件验证集成：

```bash
pnpm dlx shadcn@latest add button
```

## 接口定义

此任务不定义业务接口。对外提供：
- `src/lib/utils.ts` → `cn()` 函数
- TanStack Query `queryClient` 实例（通过 Provider 注入）

## 验收标准

```bash
# 1. 安装依赖
pnpm install

# 2. 前端开发服务器启动
pnpm dev
# → 浏览器访问 http://localhost:5173 看到 "Mango" 标题

# 3. Tauri 开发模式启动
pnpm tauri dev
# → 弹出桌面窗口，显示前端内容

# 4. Tailwind CSS 生效
# → 页面中 text-3xl / font-bold 等类正确应用样式

# 5. shadcn/ui 组件可用
pnpm dlx shadcn@latest add button
# → src/components/ui/button.tsx 生成，可在 App.tsx 中 import 使用

# 6. Vitest 运行
pnpm test:run
# → 无报错退出

# 7. TypeScript 编译检查
pnpm typecheck
# → 无类型错误
```

## 依赖关系

- **前置**：SPEC-01（需要 src-tauri 的 Cargo.toml 和 tauri.conf.json）
- **后续**：
  - SPEC-07（类型生成管线）依赖前端工程结构
  - SPEC-08（App Shell UI）依赖路由和组件库就绪
