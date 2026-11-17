import { useEffect, useRef, type MouseEvent } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import type { Row } from '../../shared/types';
import { formatDuration, formatExt } from '../lib/format';

export type SortKey = 'index' | 'title' | 'artist' | 'album' | 'duration' | 'ext' | 'refs';

export interface SortSpec {
  key: SortKey;
  dir: 'asc' | 'desc';
}

interface TrackTableProps {
  rows: Row[];
  selected: Set<string>;
  sort: SortSpec | null;
  viewKey: string;
  emptyText: string;
  onSort: (key: SortKey) => void;
  onRowClick: (row: Row, index: number, event: MouseEvent) => void;
  onRowDoubleClick: (row: Row) => void;
  onRowContextMenu: (row: Row, event: MouseEvent) => void;
}

const COLUMNS: { key: SortKey | null; label: string }[] = [
  { key: 'index', label: '#' },
  { key: 'title', label: '标题' },
  { key: 'artist', label: '艺术家' },
  { key: 'album', label: '专辑' },
  { key: 'duration', label: '时长' },
  { key: 'ext', label: '格式' },
  { key: null, label: '状态' },
  { key: 'refs', label: '引用' },
];

const STATUS_TEXT: Record<Row['status'], string> = {
  ok: '正常',
  missing: '缺失',
  stream: '网络流',
};

export function TrackTable(props: TrackTableProps) {
  const { rows, selected, sort, viewKey } = props;
  const parentRef = useRef<HTMLDivElement>(null);

  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 36,
    overscan: 14,
  });

  useEffect(() => {
    parentRef.current?.scrollTo({ top: 0 });
  }, [viewKey]);

  return (
    <div className="table">
      <div className="thead">
        {COLUMNS.map((column) => {
          const active = sort && column.key === sort.key;
          return (
            <div
              key={column.label}
              className={`th${column.key ? '' : ' static'}`}
              onClick={() => column.key && props.onSort(column.key)}
              title={column.key ? '点击切换排序' : undefined}
            >
              {column.label}
              {active && <span className="arrow">{sort?.dir === 'asc' ? '▲' : '▼'}</span>}
            </div>
          );
        })}
      </div>

      <div className="tbody" ref={parentRef}>
        {rows.length === 0 ? (
          <div className="list-empty">{props.emptyText}</div>
        ) : (
          <div style={{ height: virtualizer.getTotalSize(), position: 'relative' }}>
            {virtualizer.getVirtualItems().map((virtualRow) => {
              const row = rows[virtualRow.index];
              const isSelected = selected.has(row.key);
              return (
                <div
                  key={row.key}
                  className={`tr${isSelected ? ' selected' : ''}${virtualRow.index < 30 ? ' enter' : ''}`}
                  style={{
                    transform: `translateY(${virtualRow.start}px)`,
                    animationDelay: `${Math.min(virtualRow.index, 12) * 12}ms`,
                  }}
                  onClick={(event) => props.onRowClick(row, virtualRow.index, event)}
                  onDoubleClick={() => props.onRowDoubleClick(row)}
                  onContextMenu={(event) => props.onRowContextMenu(row, event)}
                >
                  <div className="td num">{row.index}</div>
                  <div className="td title" title={row.path ?? row.title}>
                    {row.title}
                  </div>
                  <div className={`td${row.artist ? '' : ' muted'}`}>{row.artist ?? '—'}</div>
                  <div className={`td${row.album ? '' : ' muted'}`}>{row.album ?? '—'}</div>
                  <div className="td num">{formatDuration(row.duration)}</div>
                  <div className="td muted">{formatExt(row.ext)}</div>
                  <div className="td">
                    <span className={`status ${row.status}`}>
                      <span className="status-dot" />
                      {STATUS_TEXT[row.status]}
                    </span>
                  </div>
                  <div
                    className={`td${row.refCount > 0 ? '' : ' muted'}`}
                    title={row.refNames.length > 0 ? row.refNames.join('、') : '未被引用'}
                  >
                    {row.refCount > 0 ? `${row.refCount} 个列表` : '—'}
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}
