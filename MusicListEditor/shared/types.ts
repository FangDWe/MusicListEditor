/** 主进程与渲染进程共享的 DTO 定义（对应需求文档 §4.3 数据模型） */

export type ViewKind = 'playlist' | 'unclassified' | 'all' | 'single';

/** 行状态：正常 / 文件缺失 / 网络流 */
export type RowStatus = 'ok' | 'missing' | 'stream';

export interface Row {
  /** 稳定 key：播放列表内为 `e:<entryIndex>`，曲目列表为 `t:<trackId>` */
  key: string;
  /** 展示用序号（1 起） */
  index: number;
  /** 播放列表视图中对应的条目下标，移除/重排时需要 */
  entryIndex: number | null;
  trackId: string | null;
  title: string;
  artist: string | null;
  album: string | null;
  duration: number | null;
  ext: string | null;
  path: string | null;
  status: RowStatus;
  /** 被多少个播放列表引用 */
  refCount: number;
  /** 引用它的播放列表名，用于悬浮提示 */
  refNames: string[];
}

export interface PlaylistSummary {
  id: number;
  name: string;
  relPath: string;
  path: string;
  entryCount: number;
  missingCount: number;
  dirty: boolean;
}

export interface ViewState {
  kind: ViewKind;
  playlistId: number | null;
  title: string;
  subtitle: string;
  rows: Row[];
  canSave: boolean;
}

export interface AppState {
  workspace: { root: string; name: string } | null;
  playlists: PlaylistSummary[];
  counts: { playlists: number; tracks: number; unclassified: number; missing: number };
  view: ViewState;
  canUndo: boolean;
  /** 当前工作区被屏蔽的相对目录（F2.8） */
  blockedDirs: string[];
}

export interface ScanProgress {
  phase: string;
  done: number;
  total: number;
  currentPath: string;
}

export interface AddFailure {
  playlistId: number;
  name: string;
  reason: string;
}

export interface AddReport {
  added: number;
  skipped: number;
  failed: AddFailure[];
  targetNames: string[];
}

export interface SaveResult {
  id: number;
  name: string;
  ok: boolean;
  error?: string;
  backedUp?: boolean;
  converted?: boolean;
}

/** 选中曲目在各播放列表中的包含情况（F3.3 半选状态） */
export interface Inclusion {
  playlistId: number;
  included: number;
  total: number;
}

export interface TrackProps {
  path: string;
  size: number;
  ext: string;
  refCount: number;
  refNames: string[];
  exists: boolean;
}

/** 歌曲标签（F6） */
export interface TrackTags {
  title: string | null;
  artist: string | null;
  album: string | null;
  /** `data:<mime>;base64,...`，无封面为 null */
  coverDataUrl: string | null;
  ext: string;
  editable: boolean;
}

export interface TagPatch {
  title?: string;
  artist?: string;
  album?: string;
  /** 未截取时：用磁盘图片替换第一张封面 */
  coverPath?: string;
  /** 截取后：`data:<mime>;base64,...`，优先于 coverPath */
  coverDataUrl?: string;
}

/** 删除歌曲的结果（F4.6） */
export interface DeleteReport {
  deleted: number;
  failed: { trackId: string; name: string; reason: string }[];
  affectedPlaylists: number;
  recordPath: string;
}

export interface AppConfig {
  writeBom: boolean;
  backup: boolean;
  extensions: string[];
  recentWorkspaces: string[];
  lastWorkspace?: string;
  /** 启动时自动恢复上次工作区（F2.2） */
  restoreLast: boolean;
  /** 已屏蔽的目录（F2.8），root 为工作区根 */
  blockedDirs: { root: string; rel: string }[];
}

/** 一次扫描的统计信息（§4.5） */
export interface ScanStats {
  skippedDirs: number;
  blockedDirs: number;
  failedPlaylists: { path: string; reason: string }[];
}

export interface CreatePlaylistOptions {
  name: string;
  dir?: string;
}

/** IPC 统一返回：成功带 data，失败带错误码（§6） */
export type ApiResult<T> =
  | { ok: true; data: T }
  | { ok: false; code: string; message: string };

export const DEFAULT_AUDIO_EXTS = [
  'mp3',
  'flac',
  'wav',
  'm4a',
  'aac',
  'ogg',
  'opus',
  'wma',
  'ape',
  'aiff',
  'aif',
  'wv',
];

export const PLAYLIST_EXTS = ['m3u', 'm3u8'];
