一、 整体技术方案架构
系统被设计为一个单二进制（Single Binary）的 Rust 应用程序（命名为 mend），同时充当 人类交互 CLI 与 Agent 透明执行 Hook。

                                  [ 调用入口 ]
                   ┌───────────────────┴───────────────────┐
                   ▼                                       ▼
           【人类终端模式】                          【Agent Hook 模式】
        eval $(mend --hook)                     mend exec -- "cargo build"
                   │                                       │
                   └───────────────────┬───────────────────┘
                                       ▼
                       ┌───────────────────────────────┐
                       │   CLI Context Ingestion       │
                       │ (OSC 133 / PTY / Exit Code)   │
                       └───────────────┬───────────────┘
                                       │
                                       ▼
                       ┌───────────────────────────────┐
                       │  Tier 1: 本地 Fast-Path 引擎   │ ──(命中且安全)──> [直接执行/产出]
                       │  - Levenshtein 模糊 PATH 查找  │
                       │  - 硬编码权限检测 (Exit 126)   │
                       └───────────────┬───────────────┘
                                       │ (未命中)
                                       ▼
                       ┌───────────────────────────────┐
                       │  State & Prompt Packager      │
                       │  - ANSI 清洗、Tail 截断       │
                       │  - 实体提取 (Regex/Trie)      │
                       │  - 动态路由 Criteria 方案     │
                       └───────────────┬───────────────┘
                                       │
                                       ▼ (h2 / Keep-Alive TCP, <100ms)
                       ┌───────────────────────────────┐
                       │      TypeSafe Jev Engine      │
                       │  - Choice (错误根因)          │
                       │  - Choice (修复策略)          │
                       │  - Noul   (破坏性风险门禁)    │
                       └───────────────┬───────────────┘
                                       │
                                       ▼
                       ┌───────────────────────────────┐
                       │  Remediation Reification      │
                       │  - 严格阈值判决 (Confidence)   │
                       │  - 模板实体渲染 (Zero-LLM)     │
                       └───────────────┬───────────────┘
                                       │
                       ┌───────────────┴───────────────┐
                       ▼                               ▼
                 [人类交互确认]                  [Agent 自动单次重试]
             (ratatui / inline TUI)          (Exit 0 回传 / 失败熔断)

二、 核心技术细节详解

1. 终端输出与错误流捕获（解决原版重放副作用痛点）
   原版 thefuck 依赖后台重跑命令抓取 stdout，遇到 rm、带有状态变更的构建脚本极度危险。

人类终端场景：
采用 OSC 133 Shell Integration 规范（现代化终端如 iTerm2、WezTerm、Ghostty、VSCode 默认支持）。在 Shell 的 preexec 和 precmd 钩子中，终端会在每次命令输出的前后插入特定的转义序列（\x1b]133;C\x07 代表输出开始，\x1b]133;D;<exit-code>\x07 代表命令结束）。Rust 客户端通过读取当前 Shell 会话的滚动缓冲区或 Hook 临时写出的 FIFO 管道，直接抓取真实执行的 stderr/stdout，杜绝二次重跑。

Agent Hook 场景：
直接通过 Rust 的 portable-pty 创建伪终端执行命令，接管真实子进程的全部流。这样不仅能够准确拿到带格式的输出和退出状态码，还能欺骗部分只在 TUI 下输出全量错误的工具（如带有交互式颜色的 git 或 cargo）。

2. 上下文状态清洗与实体抽取（Context Hygiene）
   Jev 依赖精准的离散状态，多余的字符会干扰向量注意力。

ANSI 转义码过滤：使用基于状态机（SIMD 加速）的 strip-ansi-escapes crate，剥离所有色彩和光标控制字符。

尾部滑动窗口（Tail Window）：对 stderr 按行倒序扫描，丢弃空行和进度条输出（如带有 \r 刷新的文本），仅保留最后的 8~12 行关键报错。

轻量实体提取器（Entity Extractor）：通过静态编译的高性能正则匹配提取器，把诸如分支名（feat-xxx）、文件路径（/path/to/file）、包名提取到 entities 哈希表中备用。

3. Jev 决策编排协议与低延迟通讯
   连接复用：如果以 CLI 形式启动，每次建立 TLS 握手会耗费 60~100ms。技术方案采用 守护进程/套接字模式（Daemon over Unix Domain Socket） 或 长连接 HTTP/2。
   在用户登录终端时后台拉起轻量 Daemon，常驻内存维持与 TypeSafe API 的长连接池。CLI 前端只负责将收集到的 State 写入 UDS（耗时 < 1ms），Daemon 完成 Jev 请求后将结果返回。

Batch Evaluation（单次并发批处理）：
在单次 Payload 中下发 failure_reason (Choice)、remediation_action (Choice) 和 destructive_risk (Noul)。

4. 确定性重构引擎（Reification & Verification Engine）
   Jev 只输出枚举 ID，不生成任何 Shell 代码：

如果 Jev 裁决 remediation_action == "GIT_SET_UPSTREAM" 且 confidence >= 0.90：
引擎直接调用内部预编译的 Rust 渲染器：

Rust

format!("git push --set-upstream {} {}", entities["remote"], entities["branch"])
安全防御门禁：

人类模式：若 destructive_risk > 0.3，在终端显示醒目的红框并必须手动输入 yes 确认。

Agent 模式：若 destructive_risk > 0.1 或 confidence < 0.92，立即熔断放弃自动重试，向 Agent 返回原始报错。

三、 项目模块划分与 Rust Crate 依赖设计
Plaintext

mend/
├── Cargo.toml
├── crates/
│ ├── mend-core/ # 核心领域模型：State, Decision, Rule, Engine
│ ├── mend-capture/ # Shell 终端捕获、PTY 包装器、OSC133 解析
│ ├── mend-jev/ # Jev 客户端 SDK、Schema 序列化、长连接传输
│ ├── mend-reify/ # 模板映射、实体替换、安全检查门禁
│ └── mend-cli/ # 命令行前端、TUI 交互 (ratatui)、Shell 钩子生成
关键依赖选型 (Cargo.toml)
网络与并发：tokio (全异步运行时), reqwest (启用 http2, rustls-tls 减小体积并加速握手), serde, serde_json。

终端与系统：portable-pty (无副作用子进程包装), strip-ansi-escapes (流级 ANSI 过滤), crossterm + ratatui (交互式行内修正选择器)。

匹配与算法：strsim (本地 Fast-path 的 Levenshtein 极速距离计算), regex (实体抽取)。

四、 完整落地实现路线（Step-by-Step）
步骤 1：搭建 Workspace 与数据契约定义 (mend-core)
在 mend-core 中定义领域实体：

ExecutionState: 包含命令拆解、执行路径、环境变量、过滤后的 stderr。

DecisionPlan: 对应 Jev 的 Choice / Noul 问题描述定义。

JevResponse: 强类型的决策结果包装（包含 Option 枚举与浮点数置信度）。

定义修复策略枚举（Strategy Enum），例如：

Rust

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]pub enum ActionStrategy {
PrependSudo,
GitSetUpstream,
GitPullRebase,
AptInstallPackage,
PathCorrection,
Abort,
}
步骤 2：实现零副作用上下文捕获 (mend-capture)
实现 PTY 执行封装：
编写 PtyRunner，在执行外部命令时接管 stdio，在内存中维护固定大小的环形缓冲区（Ring Buffer，如 64KB）。

实现 ANSI 清洗与截断器：
编写 sanitize_stderr(raw: &[u8]) -> Vec<String>，剥离转义码，按行过滤掉空白行，仅保留末尾 8 行。

实现提取器：
用静态预编译正则从报错文本中抓取提取词（如 git push --set-upstream origin <branch> 中的分支名和 remote 标识）。

步骤 3：实现本地快速拦截层（Local Fast-Path）
在不触发任何网络请求前，完成极端简单的错误修复：

检查 Exit Code：若返回 126（Permission denied）或 stderr 明确包含 EACCES，直接组装 ActionStrategy::PrependSudo。

检查命令存在性：若返回 127（Command not found），扫描 $PATH 缓存，使用 strsim::levenshtein 检索相似度最高的二进制文件，若距离 $\le 2$，直接在本地生成替换命令，跳过 Jev 调用。

步骤 4：实现 Jev Client 通信层 (mend-jev)
封装符合 TypeSafe Jev API 规范的 HTTP 客户端：

初始化包含 Keep-Alive 的连接池。

实现状态载荷与动态问题集的拼接。

根据命令类型动态选择 Criteria：

编写路由分流器：git 命令只下发 Git 相关的原因与策略枚举；通用命令下发系统级策略枚举。

实现对单次请求中多题目（Choice + Noul）的并行解码反序列化，提取置信度与评估分值。

步骤 5：实现确定性模板组装与安全门禁 (mend-reify)
建立 ActionStrategy 到命令生成模板的映射表。

注入从 ExecutionState 中抽取的实体字典，完成变量替换。

实现 Security Gate：

Rust

pub fn verify_safety(strategy: &ActionStrategy, risk_score: f64, confidence: f64, is_agent: bool) -> Result<(), SafetyError> {
let min_conf = if is_agent { 0.92 } else { 0.70 };
let max_risk = if is_agent { 0.10 } else { 0.40 };

    if confidence < min_conf {
        return Err(SafetyError::LowConfidence(confidence));
    }
    if risk_score > max_risk {
        return Err(SafetyError::DestructiveAction(risk_score));
    }
    Ok(())

}
步骤 6：构建 CLI 交互层与 Agent Hook 模式 (mend-cli)
构建 Agent 执行子命令 (mend exec -- <CMD>)：

使用 PtyRunner 启动目标子进程。

若子进程 Exit Code 为 0，透明透传输出，原样退出。

若子进程 Exit Code 非 0，拦截输出，同步进入 Jev 自愈流水线。

校验通过后，自动在当前上下文中执行自愈后的命令一次；若自愈后成功，向 stdout 打印单行标记 [auto-mended] 并返回 0；若仍失败，彻底熔断并返回最终状态码。

构建人类交互命令 (mend fix)：

使用 ratatui 渲染微型行内选择框，高亮置信度最高的建议命令，支持回车执行、上下键翻阅、ESC 取消。

输出 Shell 注入脚本 (mend init zsh/bash)：

打印对应的 Shell 函数定义，挂载别名（如 fuck 或 fix）。

步骤 7：性能压测与端到端调优
冷启动控制：编译时开启 LTO（lto = "fat"），去除符号表（strip = true），将二进制体积控制在 5MB 以内。

网络瓶颈优化：测量 mend-jev 的端到端往返耗时（RTT）。如果单次冷启动 TLS 耗时过高，在 CLI 层实现可选的 mend daemon 模式，通过 Linux Abstract Domain Socket 进一步将端到端延迟死死压制在 100ms 左右。
