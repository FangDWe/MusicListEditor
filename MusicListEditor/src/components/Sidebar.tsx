import { Fragment, useEffect, useMemo, useState } from 'react';
import type { AppState, PlaylistSummary, ViewKind } from '../../shared/types';

interface SidebarProps {
  app: AppState | null;
  busy: boolean;
  width: number;
  onSelect: (kind: ViewKind, playlistId?: number | null) => void;
  onOpenWorkspace: () => void;
  onCloseWorkspace: () => void;
  onNewPlaylist: () => void;
  onRenamePlaylist: (id: number, current: string) => void;
  onDeletePlaylist: (id: number, name: string) => void;
  onPromoteSingle: () => void;
}

/** 取相对路径中的目录部分，用于按目录分组 */
function dirOf(relPath: string): string {
  const index = relPath.lastIndexOf('\\');
  return index > 0 ? relPath.slice(0, index) : '';
}

const COLLAPSE_KEY = 'mle.collapsedDirs';

function loadCollapsed(): Set<string> {
  try {
    const raw = window.localStorage.getItem(COLLAPSE_KEY);
    const parsed: unknown = raw ? JSON.parse(raw) : [];
    return new Set(Array.isArray(parsed) ? parsed.filter((item): item is string => typeof item === 'string') : []);
  } catch {
    return new Set();
  }
}

export function Sidebar(props: SidebarProps) {
  const { app } = props;
  const workspace = app?.workspace ?? null;
  const playlists = app?.playlists ?? [];
  const view = app?.view;
  const counts = app?.counts;

  const [collapsedDirs, setCollapsedDirs] = useState<Set<string>>(loadCollapsed);

  // 按相对目录分组；根目录的列表不显示分组头
  const groups = useMemo(() => {
    const map = new Map<string, PlaylistSummary[]>();
    for (const playlist of playlists) {
      const dir = dirOf(playlist.relPath);
      const list = map.get(dir);
      if (list) list.push(playlist);
      else map.set(dir, [playlist]);
    }
    return [...map.entries()].sort((left, right) => left[0].localeCompare(right[0], 'zh-Hans-CN'));
  }, [playlists]);

  // 选中某个列表时，自动展开它所在的分组
  useEffect(() => {
    if (view?.kind !== 'playlist') return;
    const active = playlists.find((item) => item.id === view.playlistId);
    if (!active) return;
    const dir = dirOf(active.relPath);
    if (!dir) return;
    setCollapsedDirs((prev) => {
      if (!prev.has(dir)) return prev;
      const next = new Set(prev);
      next.delete(dir);
      window.localStorage.setItem(COLLAPSE_KEY, JSON.stringify([...next]));
      return next;
    });
  }, [view, playlists]);

  const toggleDir = (dir: string): void => {
    setCollapsedDirs((prev) => {
      const next = new Set(prev);
      if (next.has(dir)) next.delete(dir);
      else next.add(dir);
      window.localStorage.setItem(COLLAPSE_KEY, JSON.stringify([...next]));
      return next;
    });
  };

  const active = (kind: ViewKind, playlistId?: number | null): boolean => {
    if (!view) return false;
    if (kind !== view.kind) return false;
    if (kind === 'playlist') return view.playlistId === (playlistId ?? null);
    return true;
  };

  return (
    <aside className="sidebar" style={{ width: props.width }}>
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
              style={{ float: 'right', height: 26, padding: '0 8px', fontSize: 15 }}
              onClick={props.onNewPlaylist}
              title="新建播放列表"
            >
              +
            </button>
          </div>

          {playlists.length === 0 && <div className="side-item muted">尚无播放列表</div>}

          {groups.map(([dir, items]) => {
            const collapsed = dir !== '' && collapsedDirs.has(dir);
            return (
              <Fragment key={dir || '__root__'}>
                {dir !== '' && (
                  <div
                    className={`side-group${collapsed ? ' collapsed' : ''}`}
                    onClick={() => toggleDir(dir)}
                    title={`${dir}（点击${collapsed ? '展开' : '折叠'}）`}
                  >
                    <span className="chevron" />
                    <span className="side-group-label">{dir}</span>
                    <span className="badge">{items.length}</span>
                  </div>
                )}

                {!collapsed &&
                  items.map((playlist) => (
                    <div
                      key={playlist.id}
                      className={`side-item${dir !== '' ? ' nested' : ''}${
                        active('playlist', playlist.id) ? ' active' : ''
                      }`}
                      onClick={() => props.onSelect('playlist', playlist.id)}
                      onDoubleClick={() => props.onRenamePlaylist(playlist.id, playlist.name)}
                      title={`${playlist.relPath}\n双击重命名，右键删除`}
                      onContextMenu={(event) => {
                        event.preventDefault();
                        props.onDeletePlaylist(playlist.id, playlist.name);
                      }}
                    >
                      <div className="side-labels">
                        <span className="name">{playlist.name}</span>
                      </div>
                      {playlist.missingCount > 0 && (
                        <span className="missing">缺失 {playlist.missingCount}</span>
                      )}
                      <span className="badge">{playlist.entryCount}</span>
                      {playlist.dirty && <span className="dot" title="有未保存的更改" />}
                    </div>
                  ))}
              </Fragment>
            );
          })}

          <div className="side-sep" />

          <div
            className={`side-item${active('unclassified') ? ' active' : ''}`}
            onClick={() => props.onSelect('unclassified')}
            title="未被任何播放列表引用的曲目"
          >
            <div className="side-labels">
              <span className="name">未归类</span>
            </div>
            <span className="badge">{counts?.unclassified ?? 0}</span>
          </div>

          <div
            className={`side-item${active('all') ? ' active' : ''}`}
            onClick={() => props.onSelect('all')}
            title="工作区内全部曲目"
          >
            <div className="side-labels">
              <span className="name">全部曲目</span>
            </div>
            <span className="badge">{counts?.tracks ?? 0}</span>
          </div>

          <div className="side-sep" />
        </>
      )}

      {!workspace && view?.kind === 'single' && (
        <>
          <div className="sidebar-section">临时列表</div>
          <div className="side-item active">
            <div className="side-labels">
              <span className="name">{view.title}</span>
            </div>
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
        <div style={{ padding: '6px 10px', fontSize: 14, color: 'var(--color-text-3)' }}>
          {counts?.tracks ?? 0} 首曲目 · {counts?.playlists ?? 0} 个列表
          {(counts?.missing ?? 0) > 0 ? ` · 缺失 ${counts?.missing}` : ''}
        </div>
      )}
    </aside>
  );
}
