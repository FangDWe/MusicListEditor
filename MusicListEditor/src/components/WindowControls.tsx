import { getCurrentWindow } from '@tauri-apps/api/window';

/**
 * 自绘窗口按钮（增量需求 8）：无边框窗口下放在工具栏「设置」右侧，
 * 其余按钮因此整体左移。
 */
export function WindowControls() {
  const win = getCurrentWindow();

  return (
    <div className="win-controls">
      <button
        className="win-btn"
        title="最小化"
        aria-label="最小化"
        onClick={() => void win.minimize()}
      >
        <svg viewBox="0 0 10 10" aria-hidden="true">
          <path d="M2 5h6" />
        </svg>
      </button>
      <button
        className="win-btn"
        title="最大化 / 还原"
        aria-label="最大化或还原"
        onClick={() => void win.toggleMaximize()}
      >
        <svg viewBox="0 0 10 10" aria-hidden="true">
          <rect x="2.2" y="2.2" width="5.6" height="5.6" rx="0.8" />
        </svg>
      </button>
      <button
        className="win-btn close"
        title="关闭"
        aria-label="关闭窗口"
        onClick={() => void win.close()}
      >
        <svg viewBox="0 0 10 10" aria-hidden="true">
          <path d="M2.4 2.4l5.2 5.2M7.6 2.4L2.4 7.6" />
        </svg>
      </button>
    </div>
  );
}
