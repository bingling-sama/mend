# mend

> 零副作用的终端执行自愈引擎（Rust 单二进制）。  
> 专为人类开发者日常交互与自主 AI Coding Agent 设计，拦截真实执行流并实现确定性错误修复。

---

## 为什么需要 mend？

传统终端纠错工具（如 `thefuck`）存在一个致命的设计缺陷：**在后台默默重新执行失败的命令以抓取输出**。在涉及 `rm`、文件重命名、带有状态变更的数据库迁移或大型构建脚本时，这种机制极易引发二次破坏与脏状态。

`mend` 采用现代终端执行拦截机制：
1. **人类交互模式**：利用现代终端（Ghostty、WezTerm、iTerm2、VS Code）普及的 **OSC 133** 转义序列规范，在命令执行边界直接从会话抓取原始输出，绝不重跑。
2. **Agent 模式**：通过 `portable-pty` 伪终端封装执行命令，接管真实子进程流与退出状态码，维持 64KB 内存环形缓冲区。
3. **确定性模板渲染**：不依赖未受约束的 LLM 生成任意 Shell 脚本，而是分类错误根因并匹配严格审查过的 Rust 本地安全模板。
4. **双模态安全门禁**：
   - **Agent 模式**：要求置信度 $\ge 0.92$ 且破坏性风险 $\le 0.10$，超标立即熔断回传原始错误。
   - **人类模式**：破坏性风险 $> 0.30$ 强制在终端要求输入 `yes` 确认。

---

## 核心特性

- **Tier 1 极速本地 Fast-Path**：
  - Exit Code 126 / `EACCES`：毫秒级补全 `sudo`。
  - Exit Code 127（Command not found）：基于 Levenshtein 编辑距离模糊匹配 `$PATH` 可执行文件（距离 $\le 2$ 直接替换），无需网络往返。
  - 多工具链生态快速自愈：自动适配 Node (`pnpm`/`npm`/`yarn`/`bun`)、Git、Cargo、Python 等常见报错。
- **Tier 2 TypeSafe Jev 决策流水线**：
  - ANSI 转义序列清洗与尾部 8~12 行关键报错提取。
  - 基于编译期正则的参数实体抽取（git remote/branch、package 名、path 等）。
  - 支持长连接 / UDS Daemon 模式，降低评估往返耗时。
- **自动配置文件初始化与 Shell 引导**：
  - 自动检测并生成 `~/.mend.json` 配置文件。
  - 提供 `mend init` 一键自动挂载到当前使用 Shell（Zsh / Bash / Fish）。
- **轻量单二进制**：采用全静态链接与 LTO fat 优化，编译后单一二进制小于 4 MB。

---

## 安装与快速上手

### 1. 安装到 Cargo 二进制目录

```bash
cargo install --path crates/heal-cli
```

安装后将获得全局命令 `mend`。

### 2. 一键 Shell 挂载 (Init)

运行以下命令，会自动识别你的当前终端（Zsh、Bash 或 Fish），并安全地将 hook 写入对应的 `~/.zshrc`、`~/.bashrc` 或 `config.fish`：

```bash
mend init
```

然后根据提示刷新配置即可：
```bash
source ~/.zshrc    # 若使用 Zsh
```

> 注：若只需单独打印 Shell Hook 脚本文本，仍可指定 Shell 名称（如 `mend init zsh`）。

---

## Jev 决策引擎配置 (`~/.mend.json`)

当错误无法被 Tier 1 本地快速路径拦截时（例如复杂的 Git 冲突、非简单拼写的参数丢失），`mend` 会向 TypeSafe Jev 决策服务发起离散状态评估。

### 自动生成与读取

首次运行 `mend` 时，程序会在你的主目录下自动生成 `~/.mend.json`：

```json
{
  "jev_endpoint": "https://api.typesafe.ai/v1/jev/evaluate",
  "jev_api_key": ""
}
```

- 若要在遇到复杂错误时启用云端 Jev 自愈诊断，只需在 `~/.mend.json` 中填入你的 API Key 即可。
- 如果本地快速路径未命中，且未在配置文件中配置 `jev_api_key`，程序会在终端打印明晰的 Warning 提示。

### 覆盖优先级

配置生效优先级如下：
1. **CLI 参数**（最高）：`--jev-endpoint` / `--jev-api-key`
2. **环境变量**：`JEV_ENDPOINT` / `JEV_API_KEY`
3. **配置文件**：`~/.mend.json`（默认）

---

## 使用指南

### 1. 人类交互使用

当命令敲错或失败时，直接在终端敲击 `mend`（或别名 `fix` / `fuck`）：
```bash
$ gti status
zsh: command not found: gti

$ mend
git status [enter/↑/↓/ctrl+c]
```
- 按下 <kbd>Enter</kbd> 立即在当前环境执行建议命令。
- 按下 <kbd>↑</kbd> / <kbd>↓</kbd> 切换更多候选项（如有）。
- 按下 <kbd>Ctrl+C</kbd> 或 <kbd>Esc</kbd> 取消执行。

---

### 2. Agent 透明执行模式 (`exec`)

适用于 Claude Code、OpenCode、Codex 或 CI/CD 自动化流水线：

```bash
# 透明运行命令，遇到可修复错误自动单次重试
mend exec -- cargo build

# 自动处理权限缺失或命令拼写
mend exec -- touch /root/test.txt
```

- 若初始命令返回 `Exit 0`：透明输出并原样退出。
- 若失败且满足安全门禁：输出 `[auto-healed]` 标记，自动执行修复模板并回传自愈结果。
- 若自愈失败或触发安全门禁：立即熔断，原样返回退出码。

---

## 架构组成

工作空间采用 5-Crate 架构：

```
mend/
├── Cargo.toml
├── crates/
│   ├── heal-core/       # 领域核心实体：ExecutionState, DecisionPlan, ActionStrategy, Rule
│   ├── heal-capture/    # 环形缓冲区、ANSI清洗、OSC 133 解析、PTY 运行包装器
│   ├── heal-jev/        # Jev 客户端 SDK、动态 Criteria 路由、连接复用
│   ├── heal-reify/      # 模板渲染替换、双模态 SafetyGate 门禁、规则库集
│   └── heal-cli/        # 命令行入口、单行交互、Fast-Path 引擎、Shell Hook & Init
```

---

## 测试与代码验证

```bash
# 运行全工作空间测试套件
cargo test --workspace

# 类型与编译检查
cargo check --workspace
```

---

## 许可证

本项目采用 [MIT OR Apache-2.0](LICENSE) 双重开源许可证。
