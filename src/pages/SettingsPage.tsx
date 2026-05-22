import { NavLink, Outlet } from "react-router-dom";
import { cn } from "@/lib/utils";

const SECTIONS = [
  { path: "general", label: "通用" },
  { path: "accounts", label: "账号" },
] as const;

export default function SettingsPage() {
  return (
    <div className="flex flex-col gap-6">
      <header>
        <h2 className="text-xl font-semibold">设置</h2>
      </header>
      <nav className="flex gap-1 border-b">
        {SECTIONS.map((section) => (
          <NavLink
            key={section.path}
            to={section.path}
            className={({ isActive }) =>
              cn(
                "border-b-2 px-4 py-2 text-sm transition-colors",
                isActive
                  ? "border-primary text-foreground"
                  : "border-transparent text-muted-foreground hover:text-foreground",
              )
            }
          >
            {section.label}
          </NavLink>
        ))}
      </nav>
      <div>
        <Outlet />
      </div>
    </div>
  );
}
