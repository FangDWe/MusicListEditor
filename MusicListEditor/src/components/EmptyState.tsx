interface EmptyStateProps {
  recents: string[];
  onOpenWorkspace: () => void;
  onOpenFile: () => void;
  onOpenRecent: (path: string) => void;
}

export function EmptyState({ recents, onOpenWorkspace, onOpenFile, onOpenRecent }: EmptyStateProps) {
  return (
    <div className="empty">
      <h2>开始整理你的播放列表</h2>
      <p>
        选择一个文件夹作为工作区，应用会递归导入其中（含子目录）的全部 <code>.m3u</code> 播放列表与音频文件，
        并把没有被任何列表引用的曲目归入「未归类」。
      </p>
      <div className="empty-actions">
        <button className="btn primary" onClick={onOpenWorkspace}>
          选择工作区文件夹
        </button>
        <button className="btn" onClick={onOpenFile}>
          打开单个 m3u 文件
        </button>
      </div>
      {recents.length > 0 && (
        <div style={{ marginTop: 10, fontSize: 12, color: 'var(--color-text-3)' }}>
          最近打开：
          {recents.slice(0, 3).map((path) => (
            <button
              key={path}
              className="btn ghost"
              style={{ marginLeft: 6, height: 24, fontSize: 12 }}
              onClick={() => onOpenRecent(path)}
              title={path}
            >
              {path.split(/[\\/]/).pop() || path}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
