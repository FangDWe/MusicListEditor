import type { AppState, ViewKind } from '../../shared/types';

interface SidebarProps {
  app: AppState | null;
  busy: boolean;
  onSelect: (kind: ViewKind, playlistId?: number | null) => void;
  onOpenWorkspace: () => void;
  onCloseWorkspace: () => void;
  onNewPlaylist: () => void;
  onRenamePlaylist: (id: number, current: string) => void;
  onDeletePlaylist: (id: number, name: string) => void;
  onPromoteSingle: () => void;
}

export function Sidebar(props: SidebarProps) {
  const { app } = props;
  const workspace = app?.workspace ?? null;
  const playlists = app?.playlists ?? [];
  const view = app?.view;
  const counts = app?.counts;

  const active = (kind: ViewKind, playlistId?: number | null): boolean => {
    if (!view) return false;
    if (kind !== view.kind) return false;
    if (kind === 'playlist') return view.playlistId === (playlistId ?? null);
    return true;
  };

  return (
    <aside className="sidebar">
      {workspace ? (
        <div className="workspace-card" onClick={props.onOpenWorkspace} title={workspace.root}>
          <div className="ws-icon">{workspace.name.slice(0, 1).toUpperCase()}</div>
          <div className="ws-meta">
            <div className="ws-name">{workspace.name}</div>
            <div className="ws-path">{workspace.root}</div>
          </div>
        </div>
      ) : (
        <div className="workspace-card" onClick={props.onOpenWorkspace}>
          <div className="ws-icon">+</div>
          <div className="ws-meta">
            <div className="ws-name">选择工作区</div>
            <div className="ws-path">一个文件夹，递归导入</div>
          </div>
        </div>
      )}

      {workspace && (
        <>
          <div className="sidebar-section">
            播放列表
            <button
              className="btn ghost"
              style={{ float: 'right', height: 22, padding: '0 6px' }}
              onClick={props.onNewPlaylist}
              title="新建播放列表"
            >
              +
            </button>
          </div>

          {playlists.length === 0 && <div className="side-item muted">尚无播放列表</div>}

          {playlists.map((pl) => (
            <div
              key={pl.id}
              className={`side-item${active('playlist', pl.id) ? ' active' : ''}`}
              onClick={() => props.onSelect('playlist', pl.id)}
              onDoubleClick={() => props.onRenamePlaylist(pl.id, pl.name)}
              title={`${pl.relPath}\n双击重命名，右键删除`}
              onContextMenu={(event) => {
                event.preventDefault();
                props.onDeletePlaylist(pl.id, pl.name);
              }}
            >
              <span className="name">{pl.name}</span>
              {pl.missingCount > 0 && <span className="missing">缺失 {pl.missingCount}</span>}
              <span className="badge">{pl.entryCount}</span>
              {pl.dirty && <span className="dot" title="有未保存的更改" />}
            </div>
          ))}

          <div className="side-sep" />

          <div
            className={`side-item${active('unclassified') ? ' active' : ''}`}
            onClick={() => props.onSelect('unclassified')}
            title="未被任何播放列表引用的曲目"
          >
            <span className="name">未归类</span>
            <span className="badge">{counts?.unclassified ?? 0}</span>
          </div>

          <div
            className={`side-item${active('all') ? ' active' : ''}`}
            onClick={() => props.onSelect('all')}
            title="工作区内全部曲目"
          >
            <span className="name">全部曲目</span>
            <span className="badge">{counts?.tracks ?? 0}</span>
          </div>

          <div className="side-sep" />
        </>
      )}

      {!workspace && view?.kind === 'single' && (
        <>
          <div className="sidebar-section">临时列表</div>
          <div className="side-item active">
            <span className="name">{view.title}</span>
          </div>
          <button className="btn" style={{ margin: '10px 8px' }} onClick={props.onPromoteSingle}>
            以此为工作区
          </button>
        </>
      )}

      <div className="spacer" />

      {workspace && (
        <div style={{ display: 'flex', gap: 6, padding: '8px 4px 2px' }}>
          <button className="btn ghost" style={{ flex: 1 }} onClick={props.onOpenWorkspace} disabled={props.busy}>
            切换
          </button>
          <button className="btn ghost danger" style={{ flex: 1 }} onClick={props.onCloseWorkspace} disabled={props.busy}>
            关闭
          </button>
        </div>
      )}

      {app && (
        <div style={{ padding: '6px 10px', fontSize: 11, color: 'var(--color-text-3)' }}>
          {counts?.tracks ?? 0} 首曲目 · {counts?.playlists ?? 0} 个列表
          {(counts?.missing ?? 0) > 0 ? ` · 缺失 ${counts?.missing}` : ''}
        </div>
      )}
    </aside>
  );
}
