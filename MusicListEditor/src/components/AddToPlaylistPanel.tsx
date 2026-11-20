import { useMemo, useState } from 'react';
import { motion } from 'motion/react';
import type { Inclusion, PlaylistSummary } from '../../shared/types';
import {
  filterPlaylists,
  groupByDir,
  loadCollapsedDirs,
  saveCollapsedDirs,
} from '../lib/grouping';

interface AddToPlaylistPanelProps {
  trackCount: number;
  playlists: PlaylistSummary[];
  inclusion: Inclusion[];
  loading: boolean;
  busy: boolean;
  onClose: () => void;
  onSubmit: (playlistIds: number[], removeIds: number[], allowDup: boolean) => void;
  onCreate: (name: string) => Promise<number | null>;
}

export function AddToPlaylistPanel(props: AddToPlaylistPanelProps) {
  const [checked, setChecked] = useState<Set<number>>(new Set());
  /** 已全部包含、但被用户取消勾选的播放列表：提交时把所选曲目从这些列表移除 */
  const [deselected, setDeselected] = useState<Set<number>>(new Set());
  const [allowDup, setAllowDup] = useState(false);
  const [filter, setFilter] = useState('');
  const [newName, setNewName] = useState('');
  const [creating, setCreating] = useState(false);
  const [collapsedDirs, setCollapsedDirs] = useState<Set<string>>(loadCollapsedDirs);

  const inclusionMap = useMemo(() => {
    const map = new Map<number, Inclusion>();
    for (const item of props.inclusion) map.set(item.playlistId, item);
    return map;
  }, [props.inclusion]);

  const visible = useMemo(() => filterPlaylists(props.playlists, filter), [props.playlists, filter]);

  // 与侧栏一致的目录分组（F3.3）
  const groups = useMemo(() => groupByDir(visible), [visible]);

  const addCount = checked.size;
  const removeCount = deselected.size;
  const submitLabel =
    addCount > 0 && removeCount > 0
      ? `应用（+${addCount} / −${removeCount}）`
      : addCount > 0
        ? `加入（${addCount}）`
        : removeCount > 0
          ? `移出（${removeCount}）`
          : '加入';

  const toggle = (id: number, fully: boolean): void => {
    // 已全部包含的列表默认处于勾选态，点一下表示「移出」；再点一下恢复
    if (fully && !checked.has(id)) {
      setDeselected((prev) => {
        const next = new Set(prev);
        if (next.has(id)) next.delete(id);
        else next.add(id);
        return next;
      });
      return;
    }
    setChecked((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const toggleDir = (dir: string): void => {
    setCollapsedDirs((prev) => {
      const next = new Set(prev);
      if (next.has(dir)) next.delete(dir);
      else next.add(dir);
      saveCollapsedDirs(next);
      return next;
    });
  };

  const createPlaylist = async (): Promise<void> => {
    const name = newName.trim();
    if (!name || creating) return;
    setCreating(true);
    try {
      const newId = await props.onCreate(name);
      if (newId !== null) setChecked((prev) => new Set(prev).add(newId));
      setNewName('');
    } finally {
      setCreating(false);
    }
  };

  return (
    <div className="panel-backdrop" onMouseDown={props.onClose}>
      <motion.div
        className="panel"
        onMouseDown={(event) => event.stopPropagation()}
        initial={{ opacity: 0, scale: 0.97, y: 6 }}
        animate={{ opacity: 1, scale: 1, y: 0 }}
        exit={{ opacity: 0, scale: 0.98, transition: { duration: 0.1 } }}
        transition={{ duration: 0.16, ease: [0.2, 0.8, 0.2, 1] }}
      >
        <div className="panel-head">
          <div className="panel-title">加入播放列表</div>
          <div className="panel-hint">
            已选 {props.trackCount} 首曲目，可同时加入或移出多个播放列表；取消勾选「已全部包含」的列表即从该列表移除
            {props.loading ? ' · 正在读取包含情况…' : ''}
          </div>
        </div>

        {props.playlists.length > 8 && (
          <div className="panel-search">
            <input
              value={filter}
              onChange={(event) => setFilter(event.target.value)}
              placeholder="筛选播放列表…"
              spellCheck={false}
              autoFocus
            />
          </div>
        )}

        <div className="panel-body">
          {props.playlists.length === 0 && (
            <div className="list-empty">当前工作区还没有播放列表，可在下方新建</div>
          )}
          {props.playlists.length > 0 && visible.length === 0 && (
            <div className="list-empty">没有匹配的播放列表</div>
          )}

          {groups.map(({ dir, items }) => {
            const collapsed = dir !== '' && collapsedDirs.has(dir);
            return (
              <div key={dir || '__root__'}>
                {dir !== '' && (
                  <div
                    className={`pl-group${collapsed ? ' collapsed' : ''}`}
                    onClick={() => toggleDir(dir)}
                    title={`${dir}（点击${collapsed ? '展开' : '折叠'}）`}
                  >
                    <span className="chevron" />
                    <span className="pl-group-label">{dir}</span>
                    <span className="badge">{items.length}</span>
                  </div>
                )}

                {!collapsed &&
                  items.map((pl) => {
                    const info = inclusionMap.get(pl.id);
                    const fully = Boolean(info && info.total > 0 && info.included === info.total);
                    const removed = fully && deselected.has(pl.id);
                    const selected = checked.has(pl.id) || (fully && !removed);
                    const partial = Boolean(info && info.included > 0 && !fully);
                    return (
                      <div
                        key={pl.id}
                        className={`pl-row${dir !== '' ? ' nested' : ''}`}
                        onClick={() => toggle(pl.id, fully)}
                        title={pl.relPath}
                      >
                        <span
                          className={`checkbox${selected ? ' checked' : partial ? ' partial' : ''}`}
                        />
                        <span className="name">{pl.name}</span>
                        <span className={`meta${removed ? ' removing' : ''}`}>
                          {fully
                            ? removed
                              ? '提交后移出所选曲目'
                              : '已全部包含'
                            : partial && info
                              ? `已含 ${info.included}/${info.total}`
                              : `${pl.entryCount} 条`}
                        </span>
                      </div>
                    );
                  })}
              </div>
            );
          })}
        </div>

        <div className="new-pl">
          <input
            value={newName}
            onChange={(event) => setNewName(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === 'Enter') void createPlaylist();
            }}
            placeholder="新建播放列表名称…（默认建在工作区根目录）"
            spellCheck={false}
          />
          <button className="btn" onClick={() => void createPlaylist()} disabled={!newName.trim() || creating}>
            新建
          </button>
        </div>

        <div className="panel-foot">
          <label
            style={{ display: 'flex', alignItems: 'center', gap: 6, fontSize: 12.5, cursor: 'pointer' }}
            onClick={() => setAllowDup((value) => !value)}
          >
            <span className={`checkbox${allowDup ? ' checked' : ''}`} />
            允许重复添加
          </label>
          <span className="spacer" />
          <button className="btn" onClick={props.onClose}>
            取消
          </button>
          <button
            className="btn primary"
            disabled={addCount + removeCount === 0 || props.busy}
            onClick={() => props.onSubmit([...checked], [...deselected], allowDup)}
          >
            {submitLabel}
          </button>
        </div>
      </motion.div>
    </div>
  );
}
