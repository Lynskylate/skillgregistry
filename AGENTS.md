# Agent Skill Registry - Project Guide

本项目是一个用于索引、发现和分发 AI Agent 技能的注册中心系统，遵循 [Agent Skills 规范](https://agentskills.io/specification)。

## 项目使命
为 AI Agent 提供一个标准化的技能仓库，支持技能的自动发现（从 GitHub）、验证、版本管理和分发。

## 技术栈
- **后端 (Backend)**: Rust
  - `api`: Axum 编写的 REST API。
  - `worker`: 异步任务处理器，负责技能发现和同步。
  - `common`: 共享库，包含数据库实体 (SeaORM)、S3 存储和配置逻辑。
- **前端 (Frontend)**: React (Vite)
  - **样式 (Styling)**: [Tailwind CSS](https://tailwindcss.com/docs/installation/using-vite)
  - **组件库 (UI Components)**: [shadcn/ui](https://ui.shadcn.com/llms.txt)
- **数据库**: SQLite (本地开发) / PostgreSQL (生产环境)
- **对象存储**: S3 兼容存储 (用于存储技能压缩包)

## 核心逻辑
### 1. 技能发现 (Discovery)
`worker` 会定期搜索 GitHub 上标记为 `agent-skill` 的仓库，或根据配置的关键词搜索代码。

### 2. 技能同步与验证 (Sync & Verify)
- **下载**: 从 GitHub 下载技能仓库的 ZIP 包。
- **验证**: 调用 `verify_skill` 检查 `SKILL.md` 的 Frontmatter 是否符合规范。
- **打包**: 调用 `package_skill` 重新打包技能目录，计算 MD5 哈希。
- **分发**: 将打包后的技能上传至 S3，并在数据库中记录版本信息。

## 开发指南
### 运行测试
```bash
cd backend/worker
cargo test
```

### 添加新功能
- 数据库变更：修改 `backend/common/src/entities/` 中的模型。
- API 变更：在 `backend/api/src/handlers.rs` 中添加处理函数。
- Worker 逻辑：在 `backend/worker/src/tasks/` 中修改任务逻辑。

## Agent Skills 规范总结 (Specification Summary)
Agent Skills 是一种标准化的格式，用于定义 AI Agent 的技能。

### 目录结构
- `skill-name/`
    - `SKILL.md` (必需，包含 YAML Frontmatter 和 Markdown 正文)
    - `scripts/` (可选)
    - `references/` (可选)
    - `assets/` (可选)

### SKILL.md 约束
- `name`: 1-64 字符，小写字母、数字、连字符。
- `description`: 1-1024 字符。
- 允许的字段: `name`, `description`, `license`, `compatibility`, `allowed-tools`, `metadata`。

## 参考资料 (References)
- **Agent Skills 官方规范**: [https://agentskills.io/specification](https://agentskills.io/specification)
- **Anthropic Skills 仓库**: [https://github.com/anthropics/skills](https://github.com/anthropics/skills)
