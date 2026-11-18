import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  AddReport,
  AppConfig,
  AppState,
  Inclusion,
  SaveResult,
  ScanProgress,
  ScanStats,
  TrackProps,
  ViewKind,
} from '../../shared/types';

export class ApiError extends Error {
  code: string;

  constructor(code: string, message: string) {
    super(message);
    this.code = code;
    this.name = 'ApiError';
  }
}

type Payload = Record<string, unknown>;

async function call<T>(channel: string, payload: Payload = {}): Promise<T> {
  try {
    return await invoke<T>(channel, payload);
  } catch (raw) {
    if (raw && typeof raw === 'object' && 'code' in raw) {
      const value = raw as { code?: unknown; message?: unknown };
      throw new ApiError(String(value.code ?? 'E5000'), String(value.message ?? '未知错误'));
    }
    throw new ApiError('E5000', typeof raw === 'string' ? raw : '未知错误');
  }
}

export type { ScanStats };

export interface OpenResult {
  state: AppState;
  scan: ScanStats;
}

export interface SavePayload {
  results: SaveResult[];
  state: AppState;
}

export const api = {
  openWorkspace: (path?: string) => call<OpenResult>('workspace_open', { path: path ?? null }),
  rescan: () => call<OpenResult>('workspace_rescan'),
  closeWorkspace: () => call<AppState>('workspace_close'),
  openSingle: (path?: string) => call<AppState>('single_open', { path: path ?? null }),
  promoteSingle: () => call<AppState>('single_promote'),

  selectView: (kind: ViewKind, playlistId?: number | null) =>
    call<AppState>('view_select', { kind, playlistId: playlistId ?? null }),

  addToPlaylists: (trackIds: string[], playlistIds: number[], allowDup: boolean) =>
    call<{ report: AddReport; state: AppState }>('entries_add_to_playlists', {
      trackIds,
      playlistIds,
      allowDup,
    }),
  removeEntries: (playlistId: number, entryIndexes: number[]) =>
    call<AppState>('entries_remove', { playlistId, entryIndexes }),
  reorderEntry: (playlistId: number, from: number, to: number) =>
    call<AppState>('entries_reorder', { playlistId, from, to }),

  createPlaylist: (name: string, dir?: string) =>
    call<AppState>('playlist_create', { name, dir: dir ?? null }),
  renamePlaylist: (id: number, newName: string) =>
    call<AppState>('playlist_rename', { id, newName }),
  deletePlaylist: (id: number) => call<AppState>('playlist_delete', { id }),
  reloadPlaylist: (id: number) => call<AppState>('playlist_reload', { id }),
  savePlaylist: (id: number, force = false) =>
    call<SavePayload>('playlist_save', { id, force }),
  saveAll: (force = false) => call<SavePayload>('playlist_save_all', { force }),

  inclusion: (trackIds: string[]) => call<Inclusion[]>('playlists_inclusion', { trackIds }),

  openTrack: (trackId: string) => call<null>('track_open_external', { trackId }),
  revealTrack: (trackId: string) => call<null>('track_reveal', { trackId }),
  trackProperties: (trackId: string) => call<TrackProps>('track_properties', { trackId }),
  copyText: (text: string) => call<null>('clipboard_write', { text }),

  undo: () => call<AppState>('undo'),
  cancelScan: () => call<null>('cancel_scan'),
  scanStats: () => call<ScanStats>('scan_stats'),
  configGet: () => call<AppConfig>('config_get'),
  configSet: (patch: Partial<AppConfig>) => call<AppConfig>('config_set', { patch }),

  /** 屏蔽 / 恢复目录（含子目录），返回重新扫描后的状态（F2.8） */
  blockDir: (dir: string, blocked: boolean) => call<OpenResult>('folder_block', { dir, blocked }),
};

/** 把前端日志写入 Rust 日志文件（§5.5），失败时静默 */
export function logEvent(level: 'info' | 'warn' | 'error', message: string): void {
  void invoke('log_write', { level, message }).catch(() => undefined);
}

/** 订阅扫描进度（F2.6） */
export function onScanProgress(callback: (progress: ScanProgress) => void): () => void {
  let unlisten: UnlistenFn | undefined;
  void listen<ScanProgress>('scan:progress', (event) => callback(event.payload)).then((fn) => {
    unlisten = fn;
  });
  return () => unlisten?.();
}

/** 订阅工作区文件变更（F2.6） */
export function onFsChanged(callback: () => void): () => void {
  let unlisten: UnlistenFn | undefined;
  void listen('fs:changed', () => callback()).then((fn) => {
    unlisten = fn;
  });
  return () => unlisten?.();
}
