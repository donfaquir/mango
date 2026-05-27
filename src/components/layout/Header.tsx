import { useLocation } from "react-router-dom";
import {
  isValidSubjectKind,
  SUBJECT_KIND_LABELS,
} from "@/lib/subjectKind";

function getTitle(pathname: string): string {
  if (pathname === "/") return "项目列表";

  if (pathname.startsWith("/settings")) {
    const seg = pathname.split("/")[2];
    if (seg === "accounts") return "设置 / 账号";
    if (seg === "general" || !seg) return "设置 / 通用";
    return "设置";
  }

  if (pathname.startsWith("/project/")) {
    const parts = pathname.split("/");
    // /project/:id/generation
    if (parts[3] === "generation") {
      return "AI 生成";
    }
    // /project/:id/subjects/:kind[/:subjectId]
    if (parts[3] === "subjects") {
      const kind = parts[4];
      if (isValidSubjectKind(kind)) {
        return `主体库 / ${SUBJECT_KIND_LABELS[kind]}`;
      }
      return "主体库";
    }
    return "项目工作区";
  }

  return "Mango";
}

export function Header() {
  const location = useLocation();
  return (
    <header className="flex h-14 items-center border-b px-6">
      <h1 className="text-lg font-medium">{getTitle(location.pathname)}</h1>
    </header>
  );
}
