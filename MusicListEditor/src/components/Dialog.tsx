import { useState } from 'react';
import { motion } from 'motion/react';
import type { AppConfig, TrackProps } from '../../shared/types';
import { formatExt, formatSize } from '../lib/format';

export type DialogState =
  | { kind: 'new-playlist' }
  | { kind: 'conflict'; playlistId: number; name: string }
  | { kind: 'confirm-delete'; playlistId: number; name: string }
  | { kind: 'rename'; playlistId: number; current: string }
  | { kind: 'properties'; props: TrackProps }
  | { kind: 'settings'; config: AppConfig };

interface DialogProps {
  state: DialogState;
  busy: boolean;
  /** 当前工作区被屏蔽的目录（F2.8） */
  blockedDirs: string[];
  onClose: () => void;
  onCreatePlaylist: (name: string) => void;
  onRenamePlaylist: (id: number, name: string) => void;
  onDeletePlaylist: (id: number) => void;
  onOverwrite: (id: number) => void;
  onReload: (id: number) => void;
  onConfigChange: (patch: Partial<AppConfig>) => void;
  onUnblockDir: (dir: string) => void;
}

export function Dialog(props: DialogProps) {
  const { state, busy, onClose } = props;
  const [text, setText] = useState(() => (state.kind === 'rename' ? state.current : ''));

  const submitText = (): void => {
    const value = text.trim();
    if (!value) return;
    if (state.kind === 'new-playlist') props.onCreatePlaylist(value);
    if (state.kind === 'rename') props.onRenamePlaylist(state.playlistId, value);
  };

  return (
    <div className="panel-backdrop" onMouseDown={onClose}>
      <motion.div
        className="dialog"
        onMouseDown={(event) => event.stopPropagation()}
        initial={{ opacity: 0, scale: 0.97, y: 6 }}
        animate={{ opacity: 1, scale: 1, y: 0 }}
        exit={{ opacity: 0, scale: 0.98, transition: { duration: 0.1 } }}
        transition={{ duration: 0.16, ease: [0.2, 0.8, 0.2, 1] }}
      >
        {(state.kind === 'new-playlist' || state.kind === 'rename') && (
          <>
            <h3>{state.kind === 'new-playlist' ? '新建播放列表' : '重命名播放列表'}</h3>
            <p style={{ marginBottom: 4 }}>文件默认保存在工作区根目录，扩展名为 .m3u。</p>
            <div className="field">
              <input
                autoFocus
                value={text}
                onChange={(event) => setText(event.target.value)}
                onKeyDown={(event) => {
                  if (event.key === 'Enter') submitText();
                  if (event.key === 'Escape') onClose();
                }}
                placeholder="播放列表名称"
                spellCheck={false}
              />
            </div>
            <div className="dialog-actions">
              <button className="btn" onClick={onClose}>
                取消
              </button>
              <button className="btn primary" onClick={submitText} disabled={!text.trim() || busy}>
                确定
              </button>
            </div>
          </>
        )}

        {state.kind === 'confirm-delete' && (
          <>
            <h3>删除播放列表</h3>
            <p>
              将把「{state.name}」移入回收站，此操作不会删除音频文件。删除后其条目会从「未归类」计算中移除引用。
            </p>
            <div className="dialog-actions">
              <button className="btn" onClick={onClose}>
                取消
              </button>
              <button className="btn danger" onClick={() => props.onDeletePlaylist(state.playlistId)} disabled={busy}>
                移入回收站
              </button>
            </div>
          </>
        )}

        {state.kind === 'conflict' && (
          <>
            <h3>文件已在外部修改</h3>
            <p>
              「{state.name}」在打开后被其它程序修改过。覆盖会丢弃外部改动并写入当前内容（保存前会自动备份），
              重新加载会丢弃你本次的编辑。
            </p>
            <div className="dialog-actions">
              <button className="btn" onClick={onClose}>
                取消
              </button>
              <button className="btn" onClick={() => props.onReload(state.playlistId)} disabled={busy}>
                重新加载
              </button>
              <button className="btn primary" onClick={() => props.onOverwrite(state.playlistId)} disabled={busy}>
                覆盖保存
              </button>
            </div>
          </>
        )}

        {state.kind === 'properties' && (
          <>
            <h3>属性</h3>
            <div className="kv">
              <span className="k">路径</span>
              <span className="v">{state.props.path}</span>
              <span className="k">大小</span>
              <span className="v">{formatSize(state.props.size)}</span>
              <span className="k">格式</span>
              <span className="v">{formatExt(state.props.ext)}</span>
              <span className="k">引用次数</span>
              <span className="v">{state.props.refCount}</span>
              <span className="k">所在列表</span>
              <span className="v">{state.props.refNames.length ? state.props.refNames.join('、') : '未被引用'}</span>
              <span className="k">文件状态</span>
              <span className="v">{state.props.exists ? '存在' : '缺失'}</span>
            </div>
            <div className="dialog-actions">
              <button className="btn primary" onClick={onClose}>
                关闭
              </button>
            </div>
          </>
        )}

        {state.kind === 'settings' && (
          <>
            <h3>设置</h3>
            <div
              className="settings-row"
              style={{ cursor: 'pointer' }}
              onClick={() => props.onConfigChange({ restoreLast: !state.config.restoreLast })}
            >
              <span>启动时恢复上次工作区</span>
              <span style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                <span className={`checkbox${state.config.restoreLast ? ' checked' : ''}`} />
                {state.config.restoreLast ? '开' : '关'}
              </span>
            </div>
            <div
              className="settings-row"
              style={{ cursor: 'pointer' }}
              onClick={() => props.onConfigChange({ writeBom: !state.config.writeBom })}
            >
              <span>写回 UTF-8 时写入 BOM</span>
              <span style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                <span className={`checkbox${state.config.writeBom ? ' checked' : ''}`} />
                {state.config.writeBom ? '开' : '关'}
              </span>
            </div>
            <div
              className="settings-row"
              style={{ cursor: 'pointer' }}
              onClick={() => props.onConfigChange({ backup: !state.config.backup })}
            >
              <span>保存前自动备份为 .bak</span>
              <span style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                <span className={`checkbox${state.config.backup ? ' checked' : ''}`} />
                {state.config.backup ? '开' : '关'}
              </span>
            </div>

            <div className="settings-group-title">已屏蔽的文件夹（含子文件夹）</div>
            {props.blockedDirs.length === 0 ? (
              <p style={{ fontSize: 12, color: 'var(--color-text-3)' }}>
                暂无。在侧栏右键点击目录分组即可屏蔽。
              </p>
            ) : (
              <div className="settings-list">
                {props.blockedDirs.map((dir) => (
                  <div className="settings-row" key={dir}>
                    <span className="settings-path" title={dir}>
                      {dir}
                    </span>
                    <button className="btn ghost" onClick={() => props.onUnblockDir(dir)} disabled={busy}>
                      恢复
                    </button>
                  </div>
                ))}
              </div>
            )}

            <p style={{ marginTop: 10, fontSize: 12, color: 'var(--color-text-3)' }}>
              写回编码固定为 UTF-8（需求文档 Q3），格式统一为「#EXTM3U + 每行一条绝对路径」；源文件为其它编码时保存会自动转换并强制备份。
            </p>
            <div className="dialog-actions">
              <button className="btn primary" onClick={onClose}>
                关闭
              </button>
            </div>
          </>
        )}
      </motion.div>
    </div>
  );
}
