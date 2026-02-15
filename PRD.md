# MDXport CLI — PRD

## 一句话

Rust CLI 工具，将 Markdown 文件转换为排版精良的 PDF，底层直接调用 Typst 作为 library。

## 背景

MDXport 已有 Web 版（Svelte + Typst WASM），在浏览器中完成 Markdown → Typst → PDF 的全流程。现在需要一个 CLI 版本，用于：
1. 批量/自动化场景（CI、脚本、AI agent）
2. 离线使用，无需浏览器
3. 后续封装为 OpenClaw skill

## 核心功能（MVP）

### 输入
- 单个或多个 `.md` 文件
- 支持 stdin 输入（管道场景）
- YAML frontmatter 解析（title, author, authors, lang）

### 转换管线
```
Markdown (string)
  → 解析: comrak/pulldown-cmark (GFM, math, footnotes, frontmatter)
  → 转换: mdast → Typst markup (main.typ)
  → 编译: typst crate → PDF bytes
  → 输出: 写入文件 / stdout
```

### Markdown 语法支持（与 Web 版对齐）
- **块级**: H1-H6, 段落, 有序/无序/嵌套列表, 引用块, fenced 代码块, 分隔线, GFM 表格, 数学块 ($$)
- **行内**: 加粗, 斜体, 删除线, 行内代码, 链接, 行内数学 ($), 脚注, 上标/下标
- **特殊**: `[toc]` → 自动目录, 任务列表 checkbox（Web 版未实现，CLI 版补上）
- **暂不支持**: Mermaid 图表（后续版本）, 内嵌 HTML（忽略）

### 输出
- PDF 文件（默认 `<input>.pdf`）
- 可指定输出路径 `-o / --output`

### CLI 接口设计

```
mdxport <input.md> [options]

Options:
  -o, --output <path>     输出文件路径（默认: <input>.pdf）
  -s, --style <name>      排版风格: modern-tech | classic-editorial（默认: modern-tech）
  -t, --title <title>     覆盖文档标题（优先于 frontmatter）
  -a, --author <author>   覆盖作者
  --lang <lang>           文档语言: zh | en（默认: auto-detect）
  --toc                   强制生成目录
  --no-toc                禁止生成目录
  -w, --watch             监听文件变更，自动重新编译
  -v, --verbose           详细输出
  -h, --help              帮助
  -V, --version           版本

Examples:
  mdxport report.md                        # → report.pdf
  mdxport report.md -o out.pdf -s classic-editorial
  mdxport *.md                             # 批量转换
  cat notes.md | mdxport -o notes.pdf      # stdin
  mdxport draft.md -w                      # watch mode
```

## 技术选型

| 模块 | 选择 | 理由 |
|------|------|------|
| Markdown 解析 | `comrak` | Rust 原生 GFM 实现，支持 frontmatter/footnotes/math |
| Typst 编译 | `typst` crate (library mode) | 直接调用，无需 CLI 子进程 |
| CLI 框架 | `clap` | Rust 标准，derive 模式简洁 |
| 文件监听 | `notify` | watch mode |
| LaTeX→Typst 数学 | 自实现或参考 `tex2typst` | 行内/块级数学公式转换 |
| 字体 | 内嵌或系统字体 | 优先系统字体，fallback 内嵌基础字体集 |

## 项目结构

```
mdxport-cli/
├── Cargo.toml
├── src/
│   ├── main.rs              # CLI 入口 + clap
│   ├── lib.rs               # 公共 API（供 library 使用）
│   ├── convert.rs           # Markdown → Typst 转换核心
│   ├── compile.rs           # Typst → PDF 编译
│   ├── frontmatter.rs       # YAML frontmatter 解析
│   ├── template/
│   │   ├── mod.rs
│   │   ├── modern_tech.typ  # 内嵌 Typst 模板
│   │   └── classic_editorial.typ
│   └── watch.rs             # watch mode
├── tests/
│   └── fixtures/            # 测试用 .md 文件
├── AGENTS.md
└── README.md
```

## 质量要求
- `cargo test` 覆盖核心转换逻辑
- fixture 测试：提供几个典型 .md，验证编译不报错、PDF 非空
- `cargo clippy` 无 warning

## 非目标（本期不做）
- Mermaid 图表渲染（需要外部依赖或 headless browser）
- 图片嵌入（本地图片路径解析，后续版本）
- 自定义 Typst 模板（用户传入 .typ 文件）
- GUI / TUI
- 发布到 crates.io（先跑通再说）

## 后续规划
1. v0.2: 图片支持、自定义模板
2. v0.3: Mermaid 支持（通过 mermaid-cli 或内嵌方案）
3. Skill: 封装为 OpenClaw skill，SKILL.md + 安装脚本
