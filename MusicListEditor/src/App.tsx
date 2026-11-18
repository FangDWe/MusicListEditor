import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type MouseEvent,
  type PointerEvent,
} from 'react';
import { AnimatePresence } from 'motion/react';
import { confirm } from '@tauri-apps/plugin-dialog';
import type {
  AppConfig,
  AppState,
  Inclusion,
  Row,
  ScanProgress,
  ViewKind,
} from '../shared/types';
import { ApiError, api, logEvent, onFsChanged, onScanProgress, type ScanStats } from './lib/api';
import { Toolbar } from './components/Toolbar';
import { Sidebar } from './components/Sidebar';
import { TrackTable, type SortKey, type SortSpec } from './components/TrackTable';
import { ContextMenu, type MenuEntry } from './components/ContextMenu';
import { AddToPlaylistPanel } from './components/AddToPlaylistPanel';
import { Toast, type ToastState } from './components/Toast';
import { Dialog, type DialogState } from './components/Dialog';
import { EmptyState } from './components/EmptyState';
import { ScanBar } from './components/ScanBar';
import { ResizeHandles } from './components/ResizeHandles';

export function App() {
  const [app, setApp] = useState<AppState | null>(null);
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [scan, setScan] = useState<ScanProgress | null>(null);
  const [busy, setBusy] = useState(false);

  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [anchorKey, setAnchorKey] = useState<string | null>(null);
  const [search, setSearch] = useState('');
  const [sort, setSort] = useState<SortSpec | null>(null);

  const [menu, setMenu] = useState<{ x: number; y: number; row: Row } | null>(null);
  const [addPanel, setAddPanel] = useState<{ trackIds: string[] } | null>(null);
  const [inclusion, setInclusion] = useState<Inclusion[]>([]);
  const [inclusionLoading, setInclusionLoading] = useState(false);
  const [toast, setToast] = useState<ToastState | null>(null);
  const [dialog, setDialog] = useState<DialogState | null>(null);

  const toastTimer = useRef<number | null>(null);
  const restoredRef = useRef(false);
  const appRef = useRef<AppState | null>(null);

  // 侧栏宽度可拖拽：播放列表区与歌曲区宽度比例可调
  const [sidebarWidth, setSidebarWidth] = useState<number>(() => {
    const saved = Number(window.localStorage.getItem('mle.sidebarWidth'));
    return Number.isFinite(saved) && saved >= 220 && saved <= 560 ? saved : 300;
  });
  const [resizing, setResizing] = useState(false);
  const resizeRef = useRef<{ startX: number; startWidth: number } | null>(null);

  const startResize = useCallback(
    (event: PointerEvent<HTMLDivElement>) => {
      event.preventDefault();
      resizeRef.current = { startX: event.clientX, startWidth: sidebarWidth };
      event.currentTarget.setPointerCapture(event.pointerId);
      setResizing(true);
      document.body.classList.add('resizing');
    },
    [sidebarWidth],
  );

  const moveResize = useCallback((event: PointerEvent<HTMLDivElement>) => {
    const drag = resizeRef.current;
    if (!drag) return;
    const next = Math.min(560, Math.max(220, drag.startWidth + (event.clientX - drag.startX)));
    setSidebarWidth(next);
  }, []);

  const endResize = useCallback(() => {
    if (!resizeRef.current) return;
    resizeRef.current = null;
    setResizing(false);
    document.body.classList.remove('resizing');
    setSidebarWidth((value) => {
      window.localStorage.setItem('mle.sidebarWidth', String(value));
      return value;
    });
  }, []);

  const showToast = useCallback(
    (
      text: string,
      options?: { kind?: 'info' | 'error'; duration?: number; action?: ToastState['action'] },
    ): void => {
      if (toastTimer.current) window.clearTimeout(toastTimer.current);
      const id = Date.now() + Math.random();
      setToast({ id, text, kind: options?.kind ?? 'info', action: options?.action });
      const duration = options?.duration ?? (options?.action ? 10000 : 4000);
      toastTimer.current = window.setTimeout(() => setToast(null), duration);
    },
    [],
  );

  const handleError = useCallback(
    (error: unknown): void => {
      const code = error instanceof ApiError ? error.code : 'E5000';
      if (code === 'E1002') return; // 用户取消
      const message = error instanceof Error ? error.message : String(error);
      showToast(message, { kind: 'error', duration: 5000 });
    },
    [showToast],
  );

  const applyState = useCallback((next: AppState): void => {
    setApp(next);
    setSelected((prev) => new Set([...prev].filter((key) => next.view.rows.some((row) => row.key === key))));
  }, []);

  const reportScan = useCallback(
    (stats: ScanStats | null | undefined): void => {
      if (!stats) return;
      const parts: string[] = [];
      if (stats.skippedDirs > 0) parts.push(`已跳过 ${stats.skippedDirs} 个无权限目录`);
      if (stats.blockedDirs > 0) parts.push(`已屏蔽 ${stats.blockedDirs} 个目录`);
      if (stats.failedPlaylists.length > 0) parts.push(`${stats.failedPlaylists.length} 个播放列表解析失败`);
      if (parts.length > 0) showToast(parts.join('，'), { duration: 6000 });
    },
    [showToast],
  );

  // --------------------------------------------------------------- 初始化
  useEffect(() => {
    const off = onScanProgress((progress) => setScan(progress));
    return off;
  }, []);

  useEffect(() => {
    // 注意：React StrictMode 在开发模式下会「执行 → 清理 → 再执行」。
    // 这里不能用 cancelled 标志在清理时中止首次加载，否则第二次执行被 restoredRef 挡掉，
    // 配置永远读不到 → 设置按钮点不开、上次工作区也不会恢复。
    if (restoredRef.current) return;
    restoredRef.current = true;
    void (async () => {
      let cfg: AppConfig;
      try {
        cfg = await api.configGet();
      } catch (error) {
        logEvent('error', `读取配置失败：${String(error)}`);
        return;
      }
      setConfig(cfg);
      if (!cfg.restoreLast || !cfg.lastWorkspace) return;
      setBusy(true);
      setScan({ phase: '恢复上次工作区', done: 0, total: 0, currentPath: cfg.lastWorkspace });
      try {
        const res = await api.openWorkspace(cfg.lastWorkspace);
        setApp(res.state);
        reportScan(res.scan);
      } catch (error) {
        // 上次工作区不可用时静默失败，保留空状态（F2.2）
        logEvent('warn', `恢复上次工作区失败：${String(error)}`);
      } finally {
        setBusy(false);
        setScan(null);
      }
    })();
  }, [reportScan]);

  // 屏蔽 WebView 默认右键菜单（增量需求 4）；输入框保留系统菜单以便粘贴
  useEffect(() => {
    const onContextMenu = (event: globalThis.MouseEvent): void => {
      const target = event.target as HTMLElement | null;
      if (target?.closest('input, textarea, [contenteditable="true"]')) return;
      event.preventDefault();
    };
    window.addEventListener('contextmenu', onContextMenu);
    return () => window.removeEventListener('contextmenu', onContextMenu);
  }, []);

  // 前端异常写入日志文件（§5.5）
  useEffect(() => {
    const onError = (event: ErrorEvent): void => {
      logEvent('error', `未捕获异常：${event.message} @ ${event.filename}:${event.lineno}`);
    };
    const onRejection = (event: PromiseRejectionEvent): void => {
      logEvent('error', `未处理的 Promise 拒绝：${String(event.reason)}`);
    };
    window.addEventListener('error', onError);
    window.addEventListener('unhandledrejection', onRejection);
    return () => {
      window.removeEventListener('error', onError);
      window.removeEventListener('unhandledrejection', onRejection);
    };
  }, []);

  // --------------------------------------------------------------- 派生数据
  const sortedRows = useMemo(() => {
    const base = app?.view.rows ?? [];
    const keyword = search.trim().toLowerCase();
    let list = keyword
      ? base.filter((row) =>
          `${row.title} ${row.artist ?? ''} ${row.album ?? ''} ${row.path ?? ''}`
            .toLowerCase()
            .includes(keyword),
        )
      : base;

    if (sort) {
      const direction = sort.dir === 'asc' ? 1 : -1;
      list = [...list].sort((a, b) => {
        switch (sort.key) {
          case 'title':
            return direction * a.title.localeCompare(b.title, 'zh-Hans-CN');
          case 'artist':
            return direction * (a.artist ?? '').localeCompare(b.artist ?? '', 'zh-Hans-CN');
          case 'album':
            return direction * (a.album ?? '').localeCompare(b.album ?? '', 'zh-Hans-CN');
          case 'duration':
            return direction * ((a.duration ?? 0) - (b.duration ?? 0));
          case 'ext':
            return direction * (a.ext ?? '').localeCompare(b.ext ?? '');
          case 'refs':
            return direction * (a.refCount - b.refCount);
          default:
            return direction * (a.index - b.index);
        }
      });
    }
    return list;
  }, [app, search, sort]);

  const selectedRows = useMemo(() => {
    const rows = app?.view.rows ?? [];
    return rows.filter((row) => selected.has(row.key));
  }, [app, selected]);

  const selectedTrackIds = useMemo(
    () => [...new Set(selectedRows.map((row) => row.trackId).filter((id): id is string => Boolean(id)))],
    [selectedRows],
  );

  const dirtyCount = app?.playlists.filter((playlist) => playlist.dirty).length ?? 0;
  const hasWorkspace = Boolean(app?.workspace);
  const showEmpty = !app || (!app.workspace && app.view.kind !== 'single');

  // --------------------------------------------------------------- 操作
  const openWorkspace = useCallback(
    async (path?: string): Promise<void> => {
      if (busy) return;
      setBusy(true);
      setScan({ phase: '准备扫描', done: 0, total: 0, currentPath: path ?? '' });
      try {
        const res = await api.openWorkspace(path);
        applyState(res.state);
        setSearch('');
        setSort(null);
        setSelected(new Set());
        reportScan(res.scan);
        if (res.state.counts.tracks === 0) {
          showToast('工作区中没有找到音频文件', { duration: 5000 });
        }
      } catch (error) {
        handleError(error);
      } finally {
        setBusy(false);
        setScan(null);
      }
    },
    [busy, applyState, reportScan, handleError, showToast],
  );

  useEffect(() => {
    appRef.current = app;
  }, [app]);

  // 工作区 .m3u 变更（F2.6）：无未保存改动时自动重扫，否则提示
  useEffect(() => {
    const off = onFsChanged(() => {
      const current = appRef.current;
      if (!current?.workspace) return;
      if (current.playlists.some((playlist) => playlist.dirty)) {
        showToast('工作区有外部变更；保存后可重新扫描', { duration: 6000 });
        return;
      }
      void openWorkspace(current.workspace.root);
    });
    return off;
  }, [openWorkspace, showToast]);

  const openFile = useCallback(
    async (path?: string): Promise<void> => {
      if (busy) return;
      setBusy(true);
      try {
        applyState(await api.openSingle(path));
        setSearch('');
        setSort(null);
        setSelected(new Set());
      } catch (error) {
        handleError(error);
      } finally {
        setBusy(false);
      }
    },
    [busy, applyState, handleError],
  );

  const promoteSingle = useCallback(async (): Promise<void> => {
    setBusy(true);
    setScan({ phase: '扫描文件', done: 0, total: 0, currentPath: '' });
    try {
      applyState(await api.promoteSingle());
      setSelected(new Set());
    } catch (error) {
      handleError(error);
    } finally {
      setBusy(false);
      setScan(null);
    }
  }, [applyState, handleError]);

  const closeWorkspace = useCallback(async (): Promise<void> => {
    if (dirtyCount > 0) {
      const confirmed = await confirm(`有 ${dirtyCount} 个播放列表未保存，确定关闭工作区？`, {
        title: '关闭工作区',
        kind: 'warning',
      });
      if (!confirmed) return;
    }
    try {
      applyState(await api.closeWorkspace());
      setSelected(new Set());
      setSearch('');
    } catch (error) {
      handleError(error);
    }
  }, [dirtyCount, applyState, handleError]);

  const selectView = useCallback(
    async (kind: ViewKind, playlistId?: number | null): Promise<void> => {
      if (busy) return;
      setBusy(true);
      try {
        applyState(await api.selectView(kind, playlistId ?? null));
        setSelected(new Set());
        setSearch('');
      } catch (error) {
        handleError(error);
      } finally {
        setBusy(false);
      }
    },
    [busy, applyState, handleError],
  );

  const undo = useCallback(async (): Promise<void> => {
    try {
      applyState(await api.undo());
      showToast('已撤销上一步操作');
    } catch (error) {
      handleError(error);
    }
  }, [applyState, showToast, handleError]);

  /** 屏蔽 / 恢复目录（含子文件夹），随后后端重新扫描（F2.8） */
  const blockDir = useCallback(
    async (dir: string, blocked = true): Promise<void> => {
      setBusy(true);
      setScan({ phase: blocked ? '屏蔽目录' : '恢复目录', done: 0, total: 0, currentPath: dir });
      try {
        const res = await api.blockDir(dir, blocked);
        applyState(res.state);
        reportScan(res.scan);
        showToast(blocked ? `已屏蔽「${dir}」及其子文件夹` : `已恢复「${dir}」`);
      } catch (error) {
        handleError(error);
      } finally {
        setBusy(false);
        setScan(null);
      }
    },
    [applyState, reportScan, showToast, handleError],
  );

  const submitAdd = useCallback(
    async (playlistIds: number[], allowDup: boolean): Promise<void> => {
      if (!addPanel) return;
      setBusy(true);
      try {
        const res = await api.addToPlaylists(addPanel.trackIds, playlistIds, allowDup);
        applyState(res.state);
        const names = res.report.targetNames.map((name) => `「${name}」`).join('');
        let text = `已加入 ${res.report.added} 首到${names}`;
        if (res.report.skipped > 0) text += `，跳过 ${res.report.skipped} 条（已包含）`;
        if (res.report.failed.length > 0) text += `，${res.report.failed.length} 项失败`;
        setAddPanel(null);
        showToast(text, { action: { label: '撤销', run: () => void undo() } });
      } catch (error) {
        handleError(error);
      } finally {
        setBusy(false);
      }
    },
    [addPanel, applyState, showToast, handleError, undo],
  );

  const createPlaylist = useCallback(
    async (name: string): Promise<void> => {
      try {
        applyState(await api.createPlaylist(name));
        setDialog(null);
        showToast(`已创建「${name}」`);
      } catch (error) {
        handleError(error);
      }
    },
    [applyState, showToast, handleError],
  );

  const createPlaylistFromPanel = useCallback(
    async (name: string): Promise<number | null> => {
      const before = new Set((app?.playlists ?? []).map((playlist) => playlist.id));
      try {
        const next = await api.createPlaylist(name);
        applyState(next);
        const created = next.playlists.find((playlist) => !before.has(playlist.id));
        return created?.id ?? null;
      } catch (error) {
        handleError(error);
        return null;
      }
    },
    [app, applyState, handleError],
  );

  const renamePlaylist = useCallback(
    async (id: number, name: string): Promise<void> => {
      try {
        applyState(await api.renamePlaylist(id, name));
        setDialog(null);
        showToast('已重命名');
      } catch (error) {
        handleError(error);
      }
    },
    [applyState, showToast, handleError],
  );

  const deletePlaylist = useCallback(
    async (id: number): Promise<void> => {
      try {
        applyState(await api.deletePlaylist(id));
        setDialog(null);
        showToast('播放列表已移入回收站');
      } catch (error) {
        handleError(error);
      }
    },
    [applyState, showToast, handleError],
  );

  const saveCurrent = useCallback(
    async (force = false): Promise<void> => {
      const playlistId = app?.view.playlistId;
      if (!playlistId) return;
      setBusy(true);
      try {
        const res = await api.savePlaylist(playlistId, force);
        applyState(res.state);
        const result = res.results[0];
        if (result?.ok) {
          const extra = [result.converted ? '编码已转为 UTF-8' : '', result.backedUp ? '已备份 .bak' : '']
            .filter(Boolean)
            .join('，');
          setDialog(null);
          showToast(`已保存「${result.name}」${extra ? ` · ${extra}` : ''}`);
        }
      } catch (error) {
        if (error instanceof ApiError && error.code === 'E3001') {
          setDialog({ kind: 'conflict', playlistId, name: app?.view.title ?? '' });
        } else {
          handleError(error);
        }
      } finally {
        setBusy(false);
      }
    },
    [app, applyState, showToast, handleError],
  );

  const saveAll = useCallback(async (): Promise<void> => {
    setBusy(true);
    try {
      const res = await api.saveAll(false);
      applyState(res.state);
      const ok = res.results.filter((result) => result.ok).length;
      const failed = res.results.filter((result) => !result.ok);
      if (failed.length > 0) {
        showToast(`已保存 ${ok} 个，${failed.length} 个失败（可能已在外部修改）`, { kind: 'error' });
      } else {
        showToast(`已保存 ${ok} 个播放列表`);
      }
    } catch (error) {
      handleError(error);
    } finally {
      setBusy(false);
    }
  }, [applyState, showToast, handleError]);

  const removeSelected = useCallback(async (): Promise<void> => {
    const playlistId = app?.view.playlistId;
    if (!playlistId) return;
    const indexes = selectedRows
      .map((row) => row.entryIndex)
      .filter((value): value is number => value !== null);
    if (indexes.length === 0) return;
    setBusy(true);
    try {
      applyState(await api.removeEntries(playlistId, indexes));
      setSelected(new Set());
      showToast(`已从列表移除 ${indexes.length} 条`);
    } catch (error) {
      handleError(error);
    } finally {
      setBusy(false);
    }
  }, [app, selectedRows, applyState, showToast, handleError]);

  const openAddPanel = useCallback(
    async (row?: Row): Promise<void> => {
      const ids =
        selectedRows.length > 0
          ? selectedTrackIds
          : row?.trackId
            ? [row.trackId]
            : [];
      if (ids.length === 0) {
        showToast('所选条目没有可加入的曲目文件', { kind: 'error' });
        return;
      }
      setAddPanel({ trackIds: ids });
      setInclusion([]);
      setInclusionLoading(true);
      try {
        setInclusion(await api.inclusion(ids));
      } catch (error) {
        handleError(error);
      } finally {
        setInclusionLoading(false);
      }
    },
    [selectedRows.length, selectedTrackIds, showToast, handleError],
  );

  const showProperties = useCallback(
    async (row: Row): Promise<void> => {
      if (!row.trackId) return;
      try {
        setDialog({ kind: 'properties', props: await api.trackProperties(row.trackId) });
      } catch (error) {
        handleError(error);
      }
    },
    [handleError],
  );

  const changeConfig = useCallback(
    async (patch: Partial<AppConfig>): Promise<void> => {
      try {
        const next = await api.configSet(patch);
        setConfig(next);
        setDialog({ kind: 'settings', config: next });
      } catch (error) {
        handleError(error);
      }
    },
    [handleError],
  );

  /** 打开设置：配置尚未加载时先补读，避免按钮「点了没反应」 */
  const openSettings = useCallback(async (): Promise<void> => {
    try {
      const cfg = config ?? (await api.configGet());
      if (!config) setConfig(cfg);
      setDialog({ kind: 'settings', config: cfg });
    } catch (error) {
      handleError(error);
    }
  }, [config, handleError]);

  // --------------------------------------------------------------- 交互
  const handleRowClick = useCallback(
    (row: Row, index: number, event: MouseEvent): void => {
      if (event.shiftKey && anchorKey) {
        const from = sortedRows.findIndex((item) => item.key === anchorKey);
        if (from >= 0) {
          const [start, end] = from < index ? [from, index] : [index, from];
          const next = new Set(selected);
          for (let i = start; i <= end; i += 1) next.add(sortedRows[i].key);
          setSelected(next);
          return;
        }
      }
      if (event.ctrlKey || event.metaKey) {
        const next = new Set(selected);
        if (next.has(row.key)) next.delete(row.key);
        else next.add(row.key);
        setSelected(next);
        setAnchorKey(row.key);
        return;
      }
      setSelected(new Set([row.key]));
      setAnchorKey(row.key);
    },
    [anchorKey, sortedRows, selected],
  );

  const handleRowContextMenu = useCallback(
    (row: Row, event: MouseEvent): void => {
      event.preventDefault();
      if (!selected.has(row.key)) {
        setSelected(new Set([row.key]));
        setAnchorKey(row.key);
      }
      setMenu({ x: event.clientX, y: event.clientY, row });
    },
    [selected],
  );

  const handleRowDoubleClick = useCallback(
    async (row: Row): Promise<void> => {
      if (!row.trackId) return;
      try {
        await api.openTrack(row.trackId);
      } catch (error) {
        handleError(error);
      }
    },
    [handleError],
  );

  const toggleSort = useCallback((key: SortKey): void => {
    setSort((prev) => {
      if (!prev || prev.key !== key) return { key, dir: 'asc' };
      if (prev.dir === 'asc') return { key, dir: 'desc' };
      return null;
    });
  }, []);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent): void => {
      const target = event.target as HTMLElement | null;
      const typing = target?.tagName === 'INPUT' || target?.tagName === 'TEXTAREA';

      if (event.key === 'Escape') {
        setMenu(null);
        setAddPanel(null);
        setDialog(null);
        if (!typing) setSelected(new Set());
        return;
      }
      if (typing) return;

      const modifier = event.ctrlKey || event.metaKey;
      if (modifier && event.key.toLowerCase() === 'a') {
        event.preventDefault();
        setSelected(new Set(sortedRows.map((row) => row.key)));
        return;
      }
      if (modifier && event.key.toLowerCase() === 's') {
        event.preventDefault();
        void saveCurrent();
        return;
      }
      if (modifier && event.key.toLowerCase() === 'z') {
        event.preventDefault();
        void undo();
        return;
      }
      if (event.key === 'Delete') {
        void removeSelected();
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [sortedRows, saveCurrent, undo, removeSelected]);

  // --------------------------------------------------------------- 渲染
  const menuItems = useMemo<MenuEntry[]>(() => {
    if (!menu) return [];
    const row = menu.row;
    const hasTrack = Boolean(row.trackId);
    const inPlaylist = app?.view.kind === 'playlist';
    const count = selectedRows.length > 0 ? selectedRows.length : 1;

    return [
      {
        key: 'open',
        label: '用默认播放器打开',
        disabled: !hasTrack,
        onClick: () => {
          if (row.trackId) void api.openTrack(row.trackId).catch(handleError);
        },
      },
      {
        key: 'add',
        label: '加入播放列表…',
        hint: count > 1 ? `${count} 首` : undefined,
        disabled: !hasTrack || !hasWorkspace || (app?.playlists.length ?? 0) === 0,
        onClick: () => void openAddPanel(row),
      },
      {
        key: 'remove',
        label: '从当前列表移除',
        separatorBefore: true,
        disabled: !inPlaylist,
        onClick: () => void removeSelected(),
      },
      {
        key: 'reveal',
        label: '在资源管理器中显示',
        disabled: !hasTrack,
        onClick: () => {
          if (row.trackId) void api.revealTrack(row.trackId).catch(handleError);
        },
      },
      {
        key: 'copy',
        label: '复制文件路径',
        disabled: !row.path,
        onClick: () => {
          if (row.path) void api.copyText(row.path).catch(handleError);
        },
      },
      {
        key: 'props',
        label: '属性',
        separatorBefore: true,
        disabled: !hasTrack,
        onClick: () => void showProperties(row),
      },
    ];
  }, [menu, app, hasWorkspace, selectedRows.length, handleError, openAddPanel, removeSelected, showProperties]);

  return (
    <div className="app">
      <ResizeHandles />
      <Toolbar
        app={app}
        search={search}
        busy={busy}
        dirtyCount={dirtyCount}
        onSearch={setSearch}
        onOpenWorkspace={() => void openWorkspace()}
        onOpenFile={() => void openFile()}
        onRescan={() => void openWorkspace(app?.workspace?.root)}
        onSave={() => void saveCurrent()}
        onSaveAll={() => void saveAll()}
        onUndo={() => void undo()}
        onSettings={() => void openSettings()}
      />

      {scan && (
        <ScanBar progress={scan} onCancel={() => void api.cancelScan().catch(() => undefined)} />
      )}

      <div className="body">
        <Sidebar
          app={app}
          busy={busy}
          width={sidebarWidth}
          onSelect={(kind, playlistId) => void selectView(kind, playlistId)}
          onOpenWorkspace={() => void openWorkspace()}
          onCloseWorkspace={() => void closeWorkspace()}
          onNewPlaylist={() => setDialog({ kind: 'new-playlist' })}
          onRenamePlaylist={(id, current) => setDialog({ kind: 'rename', playlistId: id, current })}
          onDeletePlaylist={(id, name) => setDialog({ kind: 'confirm-delete', playlistId: id, name })}
          onPromoteSingle={() => void promoteSingle()}
          onBlockDir={(dir) => void blockDir(dir, true)}
        />

        <div
          className={`resizer${resizing ? ' active' : ''}`}
          onPointerDown={startResize}
          onPointerMove={moveResize}
          onPointerUp={endResize}
          onPointerCancel={endResize}
          title="拖动调整播放列表区域宽度"
        />

        <main className="content">
          {showEmpty ? (
            <EmptyState
              recents={config?.recentWorkspaces ?? []}
              onOpenWorkspace={() => void openWorkspace()}
              onOpenFile={() => void openFile()}
              onOpenRecent={(path) => void openWorkspace(path)}
            />
          ) : (
            <>
              <div className="view-header">
                <span className="view-title">{app?.view.title}</span>
                <span className="view-sub">{app?.view.subtitle}</span>
                {selected.size > 0 && <span className="view-sub">· 已选 {selected.size}</span>}
                {search.trim() && <span className="view-sub">· 筛选出 {sortedRows.length} 条</span>}
              </div>
              <TrackTable
                rows={sortedRows}
                selected={selected}
                sort={sort}
                viewKey={`${app?.view.kind}-${app?.view.playlistId ?? 'none'}`}
                emptyText={
                  app?.view.kind === 'unclassified'
                    ? '所有曲目都已归类 🎉'
                    : search.trim()
                      ? '没有匹配的曲目'
                      : '这里还没有内容'
                }
                onSort={toggleSort}
                onRowClick={handleRowClick}
                onRowDoubleClick={(row) => void handleRowDoubleClick(row)}
                onRowContextMenu={handleRowContextMenu}
              />
            </>
          )}
        </main>
      </div>

      <AnimatePresence>
        {menu && (
          <ContextMenu x={menu.x} y={menu.y} items={menuItems} onClose={() => setMenu(null)} />
        )}
      </AnimatePresence>

      <AnimatePresence>
        {addPanel && (
          <AddToPlaylistPanel
            trackCount={addPanel.trackIds.length}
            playlists={app?.playlists ?? []}
            inclusion={inclusion}
            loading={inclusionLoading}
            busy={busy}
            onClose={() => setAddPanel(null)}
            onSubmit={(ids, allowDup) => void submitAdd(ids, allowDup)}
            onCreate={createPlaylistFromPanel}
          />
        )}
      </AnimatePresence>

      <AnimatePresence>
        {toast && <Toast toast={toast} onClose={() => setToast(null)} />}
      </AnimatePresence>

      <AnimatePresence>
        {dialog && (
          <Dialog
            key={`${dialog.kind}-${'playlistId' in dialog ? dialog.playlistId : ''}`}
            state={dialog}
            busy={busy}
            blockedDirs={app?.blockedDirs ?? []}
            onClose={() => setDialog(null)}
            onCreatePlaylist={(name) => void createPlaylist(name)}
            onRenamePlaylist={(id, name) => void renamePlaylist(id, name)}
            onDeletePlaylist={(id) => void deletePlaylist(id)}
            onOverwrite={() => void saveCurrent(true)}
            onReload={(id) => {
              void (async () => {
                try {
                  applyState(await api.reloadPlaylist(id));
                  setDialog(null);
                  showToast('已从磁盘重新加载');
                } catch (error) {
                  handleError(error);
                }
              })();
            }}
            onConfigChange={(patch) => void changeConfig(patch)}
            onUnblockDir={(dir) => void blockDir(dir, false)}
          />
        )}
      </AnimatePresence>
    </div>
  );
}
