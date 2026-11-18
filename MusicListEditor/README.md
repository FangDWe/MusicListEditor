# 音乐列表编辑器（Tauri 版）

Windows 桌面端 `.m3u` 播放列表整理工具，按仓库根目录 [需求文档](../音乐列表编辑器.md) 实现（当前 **V0.1.0**），技术栈 **Tauri 2 + Rust 1.100 + React 19 + TypeScript 6**。

## 运行

```bash
npm install
npm run tauri dev     # 开发模式（Vite 1420 + Rust 增量编译）
npm run tauri build   # 产物：src-tauri/target/release/bundle（NSIS / MSI）
```

首次构建会编译约 470 个 crate，耗时较久；之后增量编译只需几秒。

## 结构

```
src-tauri/src/
├─ lib.rs        日志插件（tauri-plugin-log）、dialog 插件、状态托管、27 个命令注册
├─ main.rs       入口
├─ commands.rs   全部 IPC 命令 + AppContext（与文档 §4.5 命令表一一对应）
├─ store.rs      工作区状态机：索引、未归类、编辑、撤销、写回、冲突检测
├─ m3u.rs        解析 / 解码（UTF-8 → GB18030 → lossy）/ 标准格式序列化
├─ scanner.rs    递归扫描（含屏蔽目录）+ 并发解析播放列表
├─ watcher.rs    .m3u 变更监听（notify，400ms 去抖）
├─ config.rs     配置持久化（app_data_dir/config.json）
├─ model.rs      领域模型、路径规范化 / 展示路径 / 标准格式路径
├─ dto.rs        IPC DTO（camelCase，与 shared/types.ts 对应）
└─ error.rs      统一错误 { code, message }（文档 §6）

src/             React 渲染层（自 main 分支的 Electron 版移植）
├─ App.tsx       状态编排（React Hooks）
├─ components/   Toolbar（含自绘窗口按钮）/ Sidebar / TrackTable / ContextMenu /
│                 AddToPlaylistPanel / Dialog / Toast / ScanBar / EmptyState /
│                 WindowControls / ResizeHandles
├─ lib/          api.ts（IPC 封装）、grouping.ts（目录分组/折叠）、format.ts
└─ styles/       tokens.css（设计 token）+ app.css
shared/types.ts  前后端共享 DTO 定义
```

## 与文档的对应

| 需求 | 实现 |
| --- | --- |
| F1 读取与可视化 | `m3u.rs` 宽容解析；表格虚拟滚动、搜索、三态排序、多选 |
| F1.6 单文件打开 | `single_open` / `single_promote`，「以此为工作区」 |
| F2 工作区 | 递归扫描、并发解析、未归类差集、扫描进度与取消、跳过系统目录/符号链接、拒绝嵌套工作区 |
| F2.2 工作区记忆 | `config.json` 存 `lastWorkspace`，启动自动恢复；设置可关；「关闭工作区」会清除 |
| F2.6 变更监听 | 检测到 `.m3u` 变更 → 无未保存改动自动重扫，有改动仅提示 |
| F2.8 目录屏蔽 | 侧栏分组右键屏蔽（含子目录），配置持久化，设置中恢复，立即重扫 |
| F3 多选加入 | `entries_add_to_playlists`：多选曲目 × 多选列表、已包含/半选、原子提交、撤销；面板与侧栏同套目录分组 |
| F4 编辑与写回 | 移除、新建/重命名/删除（回收站）、**标准格式写回**（`#EXTM3U` + 绝对路径 `/` 分隔）、统一 UTF-8 with BOM、非 UTF-8 源强制备份、临时文件 + 原子替换、E3001 冲突检测 |
| 视觉/动效 | 蓝白 token、motion 过渡、`prefers-reduced-motion` 降级、主题蓝滚动条、无边框窗口 |
| 可观测性 | `tauri-plugin-log`：控制台 + `%LOCALAPPDATA%\com.fdw.music-list-editor\logs`，2MB × 5 滚动；前端异常经 `log_write` 汇入 |

## 本地约定

- 写回一律 **UTF-8（默认带 BOM）**，内容为标准格式，旧 `#EXTINF`/注释/指令不会保留（文档 F4.3 / Q10）。
- 界面路径统一去掉 `\\?\` 前缀（`model::display_path`）。
- 删除走系统回收站，不做不可恢复删除。

## 待完成（P1/P2）

- 拖拽排序（命令 `entries_reorder` 已实现，UI 未接）
- ID3/FLAC 标签读取（当前标题取路径文件名）
- 安装包签名、自动更新、深色主题
