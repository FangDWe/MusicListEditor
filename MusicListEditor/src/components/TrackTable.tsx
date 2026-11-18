import { useEffect, useRef, useState, type MouseEvent } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import type { Row } from '../../shared/types';
import { formatExt } from '../lib/format';

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

/** 列：艺术家 / 专辑 / 时长已下线，引用列吃掉腾出的空间（§3.1.6） */
const COLUMNS: { key: SortKey | null; label: string; className?: string }[] = [
  { key: 'index', label: '#' },
  { key: 'title', label: '标题' },
  { key: 'ext', label: '格式' },
  { key: null, label: '状态' },
  { key: 'refs', label: '引用', className: 'refs' },
];

const STATUS_TEXT: Record<Row['status'], string> = {
  ok: '正常',
  missing: '缺失',
  stream: '网络流',
};

const CHIP_FONT =
  "11.5px 'Segoe UI Variable Text', 'Microsoft YaHei UI', system-ui, sans-serif";
const CHIP_PADDING = 18;
const CHIP_GAP = 5;

let measureCtx: CanvasRenderingContext2D | null = null;
const textWidthCache = new Map<string, number>();

function textWidth(text: string): number {
  if (!measureCtx && typeof document !== 'undefined') {
    measureCtx = document.createElement('canvas').getContext('2d');
    if (measureCtx) measureCtx.font = CHIP_FONT;
  }
  const cached = textWidthCache.get(text);
  if (cached !== undefined) return cached;
  const width = measureCtx ? measureCtx.measureText(text).width : text.length * 7;
  textWidthCache.set(text, width);
  return width;
}

function moreLabel(hidden: number): string {
  return `（...${hidden} more）`;
}

/**
 * 引用列：每个播放列表一个圆角标签；放不下时只留前几个 + 「（...N more）」，
 * 悬浮单元格仍显示完整清单。
 */
function RefsCell({ names, width }: { names: string[]; width: number }) {
  const available = Math.max(40, width - 6);
  const chipWidth = (index: number): number => textWidth(names[index]) + CHIP_PADDING;
  const prefixWidth = (count: number): number => {
    let total = 0;
    for (let i = 0; i < count; i += 1) {
      total += chipWidth(i) + (i > 0 ? CHIP_GAP : 0);
    }
    return total;
  };

  let shown = names.length;
  if (prefixWidth(names.length) > available) {
    shown = 0;
    for (let candidate = names.length - 1; candidate >= 1; candidate -= 1) {
      const marker = textWidth(moreLabel(names.length - candidate)) + CHIP_PADDING;
      if (prefixWidth(candidate) + CHIP_GAP + marker <= available) {
        shown = candidate;
        break;
      }
    }
  }

  if (names.length === 0) {
    return <span className="refs-empty">—</span>;
  }

  return (
    <div className="chips">
      {names.slice(0, shown).map((name) => (
        <span className="chip" key={name}>
          {name}
        </span>
      ))}
      {shown < names.length && <span className="chip more">{moreLabel(names.length - shown)}</span>}
    </div>
  );
}

export function TrackTable(props: TrackTableProps) {
  const { rows, selected, sort, viewKey } = props;
  const parentRef = useRef<HTMLDivElement>(null);
  const headRef = useRef<HTMLDivElement>(null);
  const [refsWidth, setRefsWidth] = useState(240);

  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => 36,
    overscan: 14,
  });

  useEffect(() => {
    parentRef.current?.scrollTo({ top: 0 });
  }, [viewKey]);

  // 引用列宽随窗口变化，用于决定每行显示几个标签
  useEffect(() => {
    const cell = headRef.current?.querySelector<HTMLElement>('.th.refs');
    if (!cell) return;
    const update = (): void => setRefsWidth(cell.clientWidth);
    update();
    const observer = new ResizeObserver(update);
    observer.observe(cell);
    return () => observer.disconnect();
  }, []);

  return (
    <div className="table">
      <div className="thead" ref={headRef}>
        {COLUMNS.map((column) => {
          const active = sort && column.key === sort.key;
          return (
            <div
              key={column.label}
              className={`th${column.key ? '' : ' static'}${column.className ? ` ${column.className}` : ''}`}
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
                  <div className="td muted">{formatExt(row.ext)}</div>
                  <div className="td">
                    <span className={`status ${row.status}`}>
                      <span className="status-dot" />
                      {STATUS_TEXT[row.status]}
                    </span>
                  </div>
                  <div
                    className="td refs"
                    title={row.refNames.length > 0 ? row.refNames.join('、') : '未被引用'}
                  >
                    <RefsCell names={row.refNames} width={refsWidth} />
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
