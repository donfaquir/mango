import { useLocation } from "react-router-dom";

function getTitle(pathname: string): string {
  if (pathname === "/") return "项目列表";
  if (pathname === "/settings") return "设置";
  if (pathname.startsWith("/project/")) return "项目工作区";
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
