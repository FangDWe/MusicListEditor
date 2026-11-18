import type { PlaylistSummary } from '../../shared/types';

/** 取相对路径中的目录部分，用于按目录分组（F2.3） */
export function dirOf(relPath: string): string {
  const index = relPath.lastIndexOf('\\');
  return index > 0 ? relPath.slice(0, index) : '';
}

export interface DirGroup<T> {
  dir: string;
  items: T[];
}

/** 按所在目录分组；根目录的组 `dir === ''`，按相对路径排序 */
export function groupByDir<T extends { relPath: string }>(playlists: T[]): DirGroup<T>[] {
  const map = new Map<string, T[]>();
  for (const playlist of playlists) {
    const dir = dirOf(playlist.relPath);
    const list = map.get(dir);
    if (list) list.push(playlist);
    else map.set(dir, [playlist]);
  }
  return [...map.entries()]
    .sort((left, right) => left[0].localeCompare(right[0], 'zh-Hans-CN'))
    .map(([dir, items]) => ({ dir, items }));
}

const COLLAPSE_KEY = 'mle.collapsedDirs';

/** 折叠状态持久化：侧栏与「加入播放列表」面板共用一个键，行为保持一致 */
export function loadCollapsedDirs(): Set<string> {
  try {
    const raw = window.localStorage.getItem(COLLAPSE_KEY);
    const parsed: unknown = raw ? JSON.parse(raw) : [];
    return new Set(
      Array.isArray(parsed) ? parsed.filter((item): item is string => typeof item === 'string') : [],
    );
  } catch {
    return new Set();
  }
}

export function saveCollapsedDirs(dirs: Set<string>): void {
  window.localStorage.setItem(COLLAPSE_KEY, JSON.stringify([...dirs]));
}

/** 过滤：匹配名称或相对路径 */
export function filterPlaylists(
  playlists: PlaylistSummary[],
  keyword: string,
): PlaylistSummary[] {
  const text = keyword.trim().toLowerCase();
  if (!text) return playlists;
  return playlists.filter(
    (item) =>
      item.name.toLowerCase().includes(text) || item.relPath.toLowerCase().includes(text),
  );
}
