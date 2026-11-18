import { useRef, useState } from 'react';
import { motion } from 'motion/react';
import { open } from '@tauri-apps/plugin-dialog';
import type { AppConfig, TagPatch, TrackProps, TrackTags } from '../../shared/types';
import { api } from '../lib/api';
import { formatExt, formatSize } from '../lib/format';

export type DialogState =
  | { kind: 'new-playlist' }
  | { kind: 'conflict'; playlistId: number; name: string }
  | { kind: 'confirm-delete'; playlistId: number; name: string }
  | { kind: 'confirm-delete-tracks'; trackIds: string[]; names: string[] }
  | { kind: 'rename'; playlistId: number; current: string }
  | { kind: 'properties'; props: TrackProps }
  | { kind: 'tags'; trackId: string; info: TrackTags }
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
  onDeleteTracks: (trackIds: string[]) => void;
  onSaveTags: (trackId: string, patch: TagPatch) => void;
  onOverwrite: (id: number) => void;
  onReload: (id: number) => void;
  onConfigChange: (patch: Partial<AppConfig>) => void;
  onUnblockDir: (dir: string) => void;
  onError: (error: unknown) => void;
}

const IMAGE_EXTS = ['jpg', 'jpeg', 'png', 'webp', 'bmp', 'gif'];

/** 截取预览框尺寸（与 CSS 中的 .crop-box 一致） */
const CROP_W = 208;
const CROP_H = 132;

function clamp(value: number, min: number, max: number): number {
  return Math.min(Math.max(value, min), Math.max(min, max));
}

function baseName(path: string): string {
  return path.split(/[\\/]/).pop() ?? path;
}

export function Dialog(props: DialogProps) {
  const { state, busy, onClose } = props;
  const [text, setText] = useState(() => (state.kind === 'rename' ? state.current : ''));
  // 标签编辑表单（F6）
  const [tagTitle, setTagTitle] = useState(() =>
    state.kind === 'tags' ? (state.info.title ?? '') : '',
  );
  const [tagArtist, setTagArtist] = useState(() =>
    state.kind === 'tags' ? (state.info.artist ?? '') : '',
  );
  const [tagAlbum, setTagAlbum] = useState(() =>
    state.kind === 'tags' ? (state.info.album ?? '') : '',
  );
  const [coverPath, setCoverPath] = useState<string | null>(null);
  const [coverPreview, setCoverPreview] = useState<string | null>(null);
  const [cropOn, setCropOn] = useState(false);
  const [cropPos, setCropPos] = useState({ x: 0, y: 0 });
  const [natural, setNatural] = useState({ w: 0, h: 0 });
  const [saving, setSaving] = useState(false);
  const [picking, setPicking] = useState(false);
  const imageRef = useRef<HTMLImageElement | null>(null);
  const dragRef = useRef<{ x: number; y: number; ox: number; oy: number } | null>(null);

  const submitText = (): void => {
    const value = text.trim();
    if (!value) return;
    if (state.kind === 'new-playlist') props.onCreatePlaylist(value);
    if (state.kind === 'rename') props.onRenamePlaylist(state.playlistId, value);
  };

  const pickCover = async (): Promise<void> => {
    const picked = await open({
      multiple: false,
      directory: false,
      filters: [{ name: '图片', extensions: IMAGE_EXTS }],
    });
    if (typeof picked !== 'string') return;
    setPicking(true);
    try {
      setCoverPreview(await api.imageDataUrl(picked));
      setCoverPath(picked);
      setCropPos({ x: 0, y: 0 });
    } catch (error) {
      props.onError(error);
    } finally {
      setPicking(false);
    }
  };

  /** 生成保存补丁：开启截取时用 canvas 裁成正方形 data URL */
  const submitTags = async (): Promise<void> => {
    if (state.kind !== 'tags') return;
    setSaving(true);
    try {
      const patch: TagPatch = { title: tagTitle, artist: tagArtist, album: tagAlbum };
      if (coverPath) {
        const img = imageRef.current;
        if (cropOn && img && natural.w > 0 && natural.h > 0) {
          const side = Math.min(natural.w, natural.h);
          const out = Math.min(1200, side);
          const canvas = document.createElement('canvas');
          canvas.width = out;
          canvas.height = out;
          const context = canvas.getContext('2d');
          if (context) {
            context.drawImage(img, cropPos.x, cropPos.y, side, side, 0, 0, out, out);
            const png = coverPath.toLowerCase().endsWith('.png');
            patch.coverDataUrl = canvas.toDataURL(png ? 'image/png' : 'image/jpeg', 0.92);
          }
        }
        if (!patch.coverDataUrl) patch.coverPath = coverPath;
      }
      props.onSaveTags(state.trackId, patch);
    } finally {
      setSaving(false);
    }
  };

  // 待替换封面在预览框里的缩放比（contain）
  const scale =
    natural.w > 0 && natural.h > 0
      ? Math.min(CROP_W / natural.w, CROP_H / natural.h)
      : 1;

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

        {state.kind === 'confirm-delete-tracks' && (
          <>
            <h3>删除歌曲文件</h3>
            <p>
              将把 {state.names.length} 个文件移入回收站，并从全部播放列表中移除对应条目（列表会变为未保存）。
              文件名会追加记录到应用数据目录的 <code>deleted-songs.txt</code>。
            </p>
            <div className="tag-files">
              {state.names.slice(0, 6).map((name, index) => (
                <div key={`${name}-${index}`} className="tag-file">
                  {name}
                </div>
              ))}
              {state.names.length > 6 && (
                <div className="tag-file muted">… 还有 {state.names.length - 6} 个</div>
              )}
            </div>
            <div className="dialog-actions">
              <button className="btn" onClick={onClose}>
                取消
              </button>
              <button
                className="btn danger"
                onClick={() => props.onDeleteTracks(state.trackIds)}
                disabled={busy}
              >
                删除文件
              </button>
            </div>
          </>
        )}

        {state.kind === 'tags' && (
          <>
            <h3>歌曲信息</h3>
            <p className="tag-hint">
              直接读写文件标签（{state.info.ext.toUpperCase()}），封面只替换第一张
              {state.info.editable ? '' : ' · 该格式暂不支持写入'}
            </p>

            <div className="tag-cover">
              <div className="tag-thumb">
                {state.info.coverDataUrl ? (
                  <img src={state.info.coverDataUrl} alt="当前封面" />
                ) : (
                  <span className="tag-thumb-empty">无封面</span>
                )}
              </div>

              <div className="crop-slot">
                {cropOn && coverPreview ? (
                  <div className="crop-box" style={{ width: CROP_W, height: CROP_H }}>
                    <img
                      className="crop-img"
                      src={coverPreview}
                      alt="待替换封面"
                      ref={imageRef}
                      onLoad={(event) => {
                        const img = event.currentTarget;
                        setNatural({ w: img.naturalWidth, h: img.naturalHeight });
                        setCropPos({ x: 0, y: 0 });
                      }}
                    />
                    {natural.w > 0 && natural.h > 0 && (
                      <div
                        className="crop-square"
                        style={{
                          left: (CROP_W - natural.w * scale) / 2 + cropPos.x * scale,
                          top: (CROP_H - natural.h * scale) / 2 + cropPos.y * scale,
                          width: Math.min(natural.w, natural.h) * scale,
                          height: Math.min(natural.w, natural.h) * scale,
                        }}
                        onPointerDown={(event) => {
                          event.preventDefault();
                          event.currentTarget.setPointerCapture(event.pointerId);
                          dragRef.current = {
                            x: event.clientX,
                            y: event.clientY,
                            ox: cropPos.x,
                            oy: cropPos.y,
                          };
                        }}
                        onPointerMove={(event) => {
                          const drag = dragRef.current;
                          if (!drag) return;
                          const side = Math.min(natural.w, natural.h);
                          setCropPos({
                            x: clamp(drag.ox + (event.clientX - drag.x) / scale, 0, natural.w - side),
                            y: clamp(drag.oy + (event.clientY - drag.y) / scale, 0, natural.h - side),
                          });
                        }}
                        onPointerUp={(event) => {
                          dragRef.current = null;
                          event.currentTarget.releasePointerCapture(event.pointerId);
                        }}
                        title="拖动调整截取区域"
                      />
                    )}
                  </div>
                ) : (
                  <span className="crop-hint">
                    {coverPreview ? '已选择新封面；开启「截取为正方形」可调整截取区域' : '尚未选择新封面'}
                  </span>
                )}
              </div>
            </div>

            <div className="tag-cover-actions">
              <button className="btn" onClick={() => void pickCover()} disabled={busy || picking}>
                替换封面
              </button>
              <label className="switch">
                <input
                  type="checkbox"
                  checked={cropOn}
                  onChange={(event) => setCropOn(event.target.checked)}
                />
                <span>截取为正方形</span>
              </label>
              {coverPath && (
                <span className="tag-cover-path" title={coverPath}>
                  {baseName(coverPath)}
                </span>
              )}
            </div>

            <div className="field">
              <label>标题</label>
              <input
                value={tagTitle}
                onChange={(event) => setTagTitle(event.target.value)}
                placeholder="留空则清除该字段"
                spellCheck={false}
              />
            </div>
            <div className="field">
              <label>艺术家</label>
              <input
                value={tagArtist}
                onChange={(event) => setTagArtist(event.target.value)}
                placeholder="留空则清除该字段"
                spellCheck={false}
              />
            </div>
            <div className="field">
              <label>专辑</label>
              <input
                value={tagAlbum}
                onChange={(event) => setTagAlbum(event.target.value)}
                placeholder="留空则清除该字段"
                spellCheck={false}
              />
            </div>

            <div className="dialog-actions">
              <button className="btn" onClick={onClose}>
                取消
              </button>
              <button
                className="btn primary"
                disabled={busy || saving || !state.info.editable}
                onClick={() => void submitTags()}
              >
                保存到文件
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
