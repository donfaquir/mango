# Worklog 工作日志

记录 Claude Code session 的执行轨迹，方便第二天接续工作。

## 文件组织

- 每天一个文件: `YYYY-MM-DD.md`
- 每个 Claude session 是文件内的一个段落，由 SessionStart hook 自动生成开头
- 段落内容由 Claude 在关键节点主动追加：完成任务、做关键决策、遇到阻塞时

## 段落格式

```markdown
## 2026-05-22 17:30:00 · session:abc12345 · branch:feature/xxx-1
<由 hook 自动生成>

### 17:32 完成 X
- **做了什么**: ...
- **为什么**: ...
- **下一步**: ...

### 17:45 阻塞: Y
- **做了什么**: 尝试 A、B
- **为什么**: 都不行，原因 Z
- **下一步**: 需要用户决策
```

## 用法

- **开始新一天的工作**: 用户打开 `docs/worklog/<今天>.md` 或 `<昨天>.md` 看进度
- **Claude 启动新 session**: 自动读最近一天的 worklog 接续上下文（约束在 `.claude/CLAUDE.md`）

## Git

每日 worklog 文件不进 git (`.gitignore` 已配置)，仅本机可见。本 README 和 `.gitignore` 跟踪。
