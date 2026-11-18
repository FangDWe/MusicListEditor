import type { AppState } from '../../shared/types';
import { WindowControls } from './WindowControls';

interface ToolbarProps {
  app: AppState | null;
  search: string;
  busy: boolean;
  dirtyCount: number;
  onSearch: (value: string) => void;
  onOpenWorkspace: () => void;
  onOpenFile: () => void;
  onRescan: () => void;
  onSave: () => void;
  onSaveAll: () => void;
  onUndo: () => void;
  onSettings: () => void;
}

/**
 * 顶部工具栏：不再显示应用名（原有系统标题栏已去掉），
 * 窗口按钮移到「设置」右侧，其余按钮整体左移（增量需求 8）。
 */
export function Toolbar(props: ToolbarProps) {
  const { app, search, busy, dirtyCount } = props;

  return (
    <header className="toolbar" data-tauri-drag-region>
      <button className="btn" onClick={props.onOpenWorkspace} disabled={busy}>
        打开工作区
      </button>
      <button className="btn" onClick={props.onOpenFile} disabled={busy}>
        打开 m3u
      </button>
      {app?.workspace && (
        <button className="btn ghost" onClick={props.onRescan} disabled={busy} title="重新扫描工作区">
          重新扫描
        </button>
      )}

      <div className="spacer" data-tauri-drag-region />

      <div className="search">
        <input
          value={search}
          onChange={(event) => props.onSearch(event.target.value)}
          placeholder="搜索标题 / 艺术家 / 专辑 / 路径"
          spellCheck={false}
        />
      </div>

      <div className="spacer" data-tauri-drag-region />

      <button className="btn" onClick={props.onUndo} disabled={!app?.canUndo || busy} title="撤销上次编辑 (Ctrl+Z)">
        撤销
      </button>
      {dirtyCount > 1 && (
        <button className="btn" onClick={props.onSaveAll} disabled={busy} title="保存全部未保存的列表">
          保存全部 ({dirtyCount})
        </button>
      )}
      <button
        className="btn primary"
        onClick={props.onSave}
        disabled={!app?.view.canSave || busy}
        title="保存当前播放列表 (Ctrl+S)"
      >
        保存
      </button>
      <button className="btn ghost" onClick={props.onSettings} title="设置">
        设置
      </button>
      <WindowControls />
    </header>
  );
}
