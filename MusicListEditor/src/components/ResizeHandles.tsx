import type { CSSProperties } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';

/** 无边框窗口（decorations: false）没有系统边框，用 8 块透明热区补回拖拽缩放 */
type Direction =
  | 'East'
  | 'North'
  | 'NorthEast'
  | 'NorthWest'
  | 'South'
  | 'SouthEast'
  | 'SouthWest'
  | 'West';

const EDGE = 5;

const HANDLES: { dir: Direction; style: CSSProperties }[] = [
  { dir: 'North', style: { top: 0, left: EDGE, right: EDGE, height: EDGE, cursor: 'ns-resize' } },
  { dir: 'South', style: { bottom: 0, left: EDGE, right: EDGE, height: EDGE, cursor: 'ns-resize' } },
  { dir: 'West', style: { left: 0, top: EDGE, bottom: EDGE, width: EDGE, cursor: 'ew-resize' } },
  { dir: 'East', style: { right: 0, top: EDGE, bottom: EDGE, width: EDGE, cursor: 'ew-resize' } },
  {
    dir: 'NorthWest',
    style: { top: 0, left: 0, width: EDGE, height: EDGE, cursor: 'nwse-resize' },
  },
  {
    dir: 'NorthEast',
    style: { top: 0, right: 0, width: EDGE, height: EDGE, cursor: 'nesw-resize' },
  },
  {
    dir: 'SouthWest',
    style: { bottom: 0, left: 0, width: EDGE, height: EDGE, cursor: 'nesw-resize' },
  },
  {
    dir: 'SouthEast',
    style: { bottom: 0, right: 0, width: EDGE, height: EDGE, cursor: 'nwse-resize' },
  },
];

export function ResizeHandles() {
  const win = getCurrentWindow();
  return (
    <>
      {HANDLES.map((handle) => (
        <div
          key={handle.dir}
          className="win-resize"
          style={handle.style}
          onPointerDown={(event) => {
            event.preventDefault();
            void win.startResizeDragging(handle.dir);
          }}
        />
      ))}
    </>
  );
}
