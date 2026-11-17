# 音乐列表编辑器（Tauri 版）

Windows 桌面端 `.m3u` 播放列表整理工具，按仓库根目录 [需求文档](../音乐列表编辑器.md) 的 P0 范围（F1–F4）用 **Tauri 2 + Rust + React 19** 实现。

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
├─ lib.rs        插件注册、状态托管、命令注册
├─ main.rs       入口
├─ commands.rs   26 个 IPC 命令（与文档 §4.5 命令表一一对应）
├─ store.rs      工作区状态机：索引、未归类、编辑、撤销、写回、冲突检测
├─ m3u.rs        解析 / 序列化 / 编码探测（UTF-8 → GB18030 → lossy）
├─ scanner.rs    递归扫描 + 并发解析播放列表
├─ watcher.rs    .m3u 变更监听（notify，400ms 去抖）
├─ config.rs     配置持久化（app_data_dir/config.json）
├─ model.rs      领域模型、路径规范化
├─ dto.rs        IPC DTO（camelCase，与 shared/types.ts 对应）
└─ error.rs      统一错误 { code, message }（文档 §6）

src/             React 渲染层（自 main 分支的 Electron 版移植）
├─ App.tsx       状态编排
├─ components/   Toolbar / Sidebar / TrackTable / ContextMenu / 面板 / 对话框
└─ styles/       tokens.css（设计 token）+ app.css
shared/types.ts  前后端共享 DTO 定义
```

## 与文档的对应

| 需求 | 实现 |
| --- | --- |
| F1 读取与可视化 | `m3u.rs` 解析，保留原始行；表格虚拟滚动、搜索、三态排序、多选 |
| F1.6 单文件打开 | `single_open` / `single_promote`，「以此为工作区」 |
| F2 工作区 | 递归扫描、并发解析、未归类差集、扫描进度事件与取消、跳过系统目录/符号链接、拒绝嵌套工作区 |
| F2.6 变更监听 | `watcher.rs`：检测到 `.m3u` 变更 → 无未保存改动自动重扫，有改动仅提示 |
| F3 多选加入 | `entries_add_to_playlists`：多选曲目 × 多选列表，已包含/半选、原子提交、撤销 |
| F4 编辑与写回 | 移除、新建/重命名/删除（回收站）、**统一 UTF-8 with BOM** 写回、非 UTF-8 源强制备份、临时文件 + `rename` 原子替换、E3001 冲突检测 |
| 视觉/动效 | tokens.css 蓝白主题、motion 过渡、`prefers-reduced-motion` 降级 |

## 待完成（P1/P2）

- 拖拽排序（命令 `entries_reorder` 已实现，UI 未接）
- ID3/FLAC 标签读取（当前标题取 `#EXTINF` 或文件名）
- 安装包签名、自动更新
