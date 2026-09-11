<p align="center">
  <img src="assets/iconcraft-hero.png" alt="IconCraft 图标工坊" width="100%" />
</p>

<h1 align="center">IconCraft 图标工坊</h1>

<p align="center">
  面向 Windows 与 macOS 的文件、目录图标自定义工具。
  <br />
  支持背景换色、图片叠加、递归应用、规则保护、批量还原和彩蛋模式。
</p>

## 功能

- 为目录生成简洁的彩色背景图标。
- 为文件生成文档、圆角文档、卡片和方形背景图标。
- 上传 PNG、JPG 或 WebP 图片并调整大小、水平位置和垂直位置。
- 独立开启或关闭背景层与自定义图片层。
- 自动处理真实透明通道，并可清除棋盘格等伪透明背景。
- 支持当前目录、限制递归层级和全部递归。
- 每次应用自动创建规则，可独立开启或关闭图标保护。
- 自动识别目标失效、图标资源丢失并关闭无效规则。
- 支持选择文件或目录一键还原，也支持从规则中还原。
- 支持托盘常驻、立即检查、图标保护开关和开机自启。
- 彩蛋模式可将指定范围内的图标统一替换，并按批次完整还原。

## 平台支持

| 能力 | macOS | Windows |
| --- | :---: | :---: |
| 单个目录自定义图标 | ✅ | ✅ |
| 单个文件自定义图标 | ✅ | — |
| 按扩展名设置文件图标 | — | ✅ |
| 目录递归设置 | ✅ | ✅ |
| 图标保护与自动修复 | ✅ | ✅ |
| 托盘与开机自启 | ✅ | ✅ |
| 彩蛋模式批量还原 | ✅ | ✅ |

> [!IMPORTANT]
> Windows 的文件图标由扩展名关联决定。彩蛋模式会全局修改扫描到的扩展名，例如所选目录中出现 `.txt`，电脑上的所有 `.txt` 文件都会受到影响。IconCraft 会在修改前保存关联信息，并提供整批还原。

## 安全与恢复

IconCraft 会在应用图标前保存必要的恢复信息：

- 普通操作可在“规则”页面还原系统默认图标。
- 删除规则时可选择仅删除规则，或还原图标后删除。
- 一键还原会自动判断对应的规则并停止后续保护。
- 彩蛋模式以批次保存所有目标的原始状态。
- 彩蛋模式应用中途失败时，会自动回滚已经完成的项目。

## 技术栈

- [Tauri 2](https://tauri.app/)：跨平台桌面应用框架
- Rust：原生图标操作、托盘、规则保护和持久化
- React 19 + TypeScript：桌面 UI
- Vite：前端开发与生产构建
- Canvas：图标合成和透明通道处理

## 本地开发

环境要求：

- Node.js 20 或更高版本
- npm 10 或更高版本
- Rust stable 工具链
- 对应平台的 Tauri 系统依赖

```bash
git clone https://github.com/dingzd1995/icon-craft.git
cd icon-craft
npm install
npm run tauri dev
```

仅启动前端预览：

```bash
npm run dev
```

## 构建

构建当前平台的安装包：

```bash
npm run tauri build
```

执行前端类型检查和生产构建：

```bash
npm run build
```

执行 Rust 测试：

```bash
cd src-tauri
cargo test --lib
```

构建产物默认位于 `src-tauri/target/release/bundle/`。

## 项目结构

```text
icon-craft/
├── assets/                 # README 图片与品牌资源
├── src/                    # React/TypeScript UI
├── src-tauri/
│   ├── capabilities/       # Tauri 权限声明
│   ├── icons/              # 各平台应用图标
│   └── src/                # Rust 原生能力
├── package.json
└── vite.config.ts
```

## 当前状态

项目仍处于早期版本。macOS 已完成本地构建及原生图标应用、还原测试；Windows 实现需要继续在 Windows 真机验证资源管理器刷新、扩展名关联和安装包行为。

