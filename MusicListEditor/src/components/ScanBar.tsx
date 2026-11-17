import type { ScanProgress } from '../../shared/types';

interface ScanBarProps {
  progress: ScanProgress;
  onCancel: () => void;
}

export function ScanBar({ progress, onCancel }: ScanBarProps) {
  const determinate = progress.total > 0;
  const percent = determinate ? Math.min(100, Math.round((progress.done / progress.total) * 100)) : 0;

  return (
    <>
      <div className="scan-bar">
        <div
          className={`fill${determinate ? '' : ' indeterminate'}`}
          style={determinate ? { width: `${percent}%` } : undefined}
        />
      </div>
      <div className="scan-hint">
        {progress.phase}
        {determinate ? ` ${progress.done}/${progress.total}` : ` ${progress.done}`}
        {progress.currentPath ? ` · ${progress.currentPath}` : ''}
        <button className="btn ghost" style={{ height: 20, padding: '0 6px', marginLeft: 8 }} onClick={onCancel}>
          取消
        </button>
      </div>
    </>
  );
}
