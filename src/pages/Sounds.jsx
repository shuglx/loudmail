import { useEffect, useRef, useState } from 'react';
import { api } from '../api';
import ConfirmDialog from '../components/ConfirmDialog';
import Modal from '../components/Modal';
import {
  DownloadIcon,
  PencilIcon,
  PlayIcon,
  SearchIcon,
  StopIcon,
  TrashIcon,
  UploadIcon,
} from '../icons';

const sourceLabel = { builtin: '内置', upload: '上传', record: '录音', tts: 'TTS' };

const isBuiltin = (sound) => sound.source === 'builtin';

const CHANNELS = [
  { key: 'upload', label: '上传文件' },
  { key: 'record', label: '录音' },
  { key: 'tts', label: '文本转语音' },
];

const emptyDraft = {
  channel: 'upload',
  name: '',
  picked: null,
  ringtoneId: '',
  loops: 1,
  recordPhase: 'idle',
  recordSeconds: 0,
  recordResult: null,
  ttsText: '',
  ttsBusy: false,
  ttsVoice: null,
  composed: null,
  composing: false,
};

export default function Sounds({ sounds, strategies, flash, onRefresh }) {
  const [playingId, setPlayingId] = useState(null);
  const [draft, setDraft] = useState(emptyDraft);
  const [newOpen, setNewOpen] = useState(false);
  const [importing, setImporting] = useState(false);
  const [error, setError] = useState('');
  const [previewing, setPreviewing] = useState('');
  const [ttsInfo, setTtsInfo] = useState(null);
  const [renameTarget, setRenameTarget] = useState(null);
  const [exportingId, setExportingId] = useState(null);
  const [renameValue, setRenameValue] = useState('');
  const [deleteTarget, setDeleteTarget] = useState(null);
  const [query, setQuery] = useState('');
  const playTimer = useRef(null);
  const recordTimer = useRef(null);

  useEffect(
    () => () => {
      if (playTimer.current) clearTimeout(playTimer.current);
      if (recordTimer.current) clearInterval(recordTimer.current);
    },
    []
  );

  useEffect(() => {
    if (!newOpen || draft.channel !== 'tts') return;
    api.ttsStatus().then(setTtsInfo).catch(() => setTtsInfo(null));
  }, [newOpen, draft.channel]);

  const usedBy = (id) => strategies.filter((s) => s.soundId === id);
  const builtins = sounds.filter(isBuiltin);
  const keyword = query.trim().toLowerCase();
  const visible = keyword
    ? sounds.filter((sound) => sound.name.toLowerCase().includes(keyword))
    : sounds;

  const voice =
    draft.channel === 'record' ? draft.recordResult : draft.ttsVoice;

  const patch = (next) => setDraft((prev) => ({ ...prev, ...next }));

  const resetComposed = () => patch({ composed: null });

  const stopPreview = async () => {
    try {
      await api.stopPlayback();
    } catch {
      // ignore
    }
    if (playTimer.current) clearTimeout(playTimer.current);
    setPreviewing('');
  };

  const playTemp = async (kind, path, durationMs) => {
    if (previewing === kind) {
      await stopPreview();
      return;
    }
    try {
      await api.stopPlayback();
      await api.previewTemp(path);
      setPlayingId(null);
      setPreviewing(kind);
      if (playTimer.current) clearTimeout(playTimer.current);
      playTimer.current = setTimeout(
        () => setPreviewing(''),
        Math.max(1000, durationMs || 2000)
      );
    } catch (e) {
      setError(String(e));
    }
  };

  const preview = async (sound) => {
    if (playingId === sound.id) {
      await stopPreview();
      setPlayingId(null);
      return;
    }
    try {
      await stopPreview();
      await api.previewSound(sound.id);
      setPlayingId(sound.id);
      if (playTimer.current) clearTimeout(playTimer.current);
      playTimer.current = setTimeout(
        () => setPlayingId(null),
        Math.max(1000, sound.durationMs || 2000)
      );
    } catch (e) {
      flash(String(e));
    }
  };

  const openNew = () => {
    setDraft(emptyDraft);
    setError('');
    setPreviewing('');
    setTtsInfo(null);
    setNewOpen(true);
  };

  const closeNew = async () => {
    if (recordTimer.current) clearInterval(recordTimer.current);
    recordTimer.current = null;
    if (draft.recordPhase === 'recording') {
      try {
        await api.stopRecording();
      } catch {
        // ignore
      }
    }
    try {
      await api.stopPlayback();
    } catch {
      // ignore
    }
    setNewOpen(false);
    setPreviewing('');
  };

  const switchChannel = async (channel) => {
    if (channel === draft.channel) return;
    if (draft.recordPhase === 'recording') {
      try {
        await api.stopRecording();
      } catch {
        // ignore
      }
    }
    if (recordTimer.current) clearInterval(recordTimer.current);
    recordTimer.current = null;
    setError('');
    patch({
      channel,
      recordPhase: 'idle',
      recordSeconds: 0,
      recordResult: null,
      composed: null,
    });
  };

  const pickFile = async () => {
    try {
      const file = await api.pickAudioFile();
      if (!file) return;
      patch({
        picked: file,
        name: draft.name || file.name.replace(/\.(wav|mp3|ogg)$/i, ''),
      });
      setError('');
    } catch (e) {
      setError(String(e));
    }
  };

  const startRecording = async () => {
    setError('');
    try {
      await api.startRecording();
      patch({ recordPhase: 'recording', recordSeconds: 0, recordResult: null, composed: null });
      if (recordTimer.current) clearInterval(recordTimer.current);
      recordTimer.current = setInterval(() => {
        setDraft((prev) => {
          if (prev.recordPhase !== 'recording') return prev;
          const next = prev.recordSeconds + 0.1;
          if (next >= 15) {
            clearInterval(recordTimer.current);
            recordTimer.current = null;
            stopRecording();
          }
          return { ...prev, recordSeconds: next };
        });
      }, 100);
    } catch (e) {
      setError(String(e));
    }
  };

  const stopRecording = async () => {
    if (recordTimer.current) clearInterval(recordTimer.current);
    recordTimer.current = null;
    try {
      const result = await api.stopRecording();
      patch({
        recordResult: result,
        recordPhase: 'done',
        name: draft.name || '新录音',
        composed: null,
      });
    } catch (e) {
      setError(String(e));
      patch({ recordPhase: 'idle' });
    }
  };

  const generateTts = async () => {
    if (!draft.ttsText.trim()) return setError('请输入要合成的文本');
    if (draft.ttsText.trim().length > 50) return setError('文本不超过 50 字');
    setError('');
    patch({ ttsBusy: true, ttsVoice: null, composed: null });
    try {
      const result = await api.ttsGenerate(draft.ttsText.trim());
      patch({
        ttsVoice: result,
        ttsBusy: false,
      });
    } catch (e) {
      setError(String(e));
      patch({ ttsBusy: false });
    }
  };

  const compose = async () => {
    if (!voice) return;
    setError('');
    patch({ composing: true, composed: null });
    try {
      const result = await api.composeAudio(
        voice.path,
        draft.ringtoneId || null,
        draft.loops
      );
      patch({ composed: result, composing: false });
    } catch (e) {
      setError(String(e));
      patch({ composing: false });
    }
  };

  const canSave =
    Boolean(draft.name.trim()) &&
    (draft.channel === 'upload'
      ? Boolean(draft.picked)
      : Boolean(draft.composed));

  const saveNew = async () => {
    if (!canSave) return;
    setError('');
    if (draft.channel === 'upload') {
      setImporting(true);
      try {
        await api.importSound(draft.picked.path, draft.name.trim());
        setNewOpen(false);
        await onRefresh();
        flash('语音已导入并转换为 WAV');
      } catch (e) {
        setError(String(e));
      } finally {
        setImporting(false);
      }
      return;
    }
    try {
      await api.saveComposedSound(
        draft.composed.path,
        draft.name.trim(),
        draft.channel === 'record' ? 'record' : 'tts'
      );
      setNewOpen(false);
      await onRefresh();
      flash('语音已保存到语音库');
    } catch (e) {
      setError(String(e));
    }
  };

  const saveRename = async () => {
    if (!renameTarget) return;
    if (!renameValue.trim()) return flash('请填写名称');
    try {
      await api.renameSound(renameTarget.id, renameValue.trim());
      setRenameTarget(null);
      await onRefresh();
      flash('语音已重命名');
    } catch (e) {
      flash(String(e));
    }
  };

  const downloadSound = async (sound) => {
    if (exportingId) return;
    setExportingId(sound.id);
    try {
      const path = await api.exportSound(sound.id);
      if (path) flash(`已保存到 ${path}`);
    } catch (e) {
      flash(String(e));
    } finally {
      setExportingId(null);
    }
  };

  const removeSound = async () => {
    const target = deleteTarget;
    setDeleteTarget(null);
    if (!target) return;
    try {
      if (playingId === target.id) {
        await api.stopPlayback();
        setPlayingId(null);
      }
      await api.deleteSound(target.id);
      await onRefresh();
      flash('语音已删除');
    } catch (e) {
      flash(String(e));
    }
  };

  const ringtone = builtins.find((s) => s.id === draft.ringtoneId) || null;

  return (
    <section className="page">
      <div className="page-head">
        <h1>语音库</h1>
        <div className="head-actions">
          <button className="btn btn-primary" onClick={openNew}>
            ＋ 新增语音
          </button>
        </div>
      </div>

      <div className="toolbar">
        <label className="search">
          <SearchIcon size={14} />
          <input
            type="text"
            value={query}
            placeholder="搜索语音名称"
            onChange={(e) => setQuery(e.target.value)}
          />
        </label>
        <span className="toolbar-note">共 {visible.length} 个语音</span>
      </div>

      <div className="sound-grid">
        {visible.length === 0 ? (
          <div className="empty" style={{ gridColumn: '1 / -1' }}>
            <b>{sounds.length ? '没有匹配的语音' : '还没有语音'}</b>
            <p>
              {sounds.length
                ? '试试其他名称关键词。'
                : '点击右上角「＋ 新增语音」添加第一段语音。'}
            </p>
          </div>
        ) : null}
        {visible.map((sound) => {
          const playing = playingId === sound.id;
          return (
            <div
              className={`sound-card${playing ? ' playing' : ''}${
                isBuiltin(sound) ? ' builtin' : ''
              }`}
              key={sound.id}
            >
              <div className="sound-top">
                <button
                  className="play-btn"
                  onClick={() => preview(sound)}
                  aria-label={playing ? '停止' : '试听'}
                >
                  {playing ? <StopIcon /> : <PlayIcon />}
                </button>
                <div className="sound-title">
                  <span className="sound-name" title={sound.name}>
                    {sound.name}
                  </span>
                  <span className="sound-duration">{formatDuration(sound.durationMs)}</span>
                </div>
                <span className="tag">{sourceLabel[sound.source] || sound.source}</span>
              </div>
              <div className="sound-meta">
                <span>被 {usedBy(sound.id).length} 条策略使用</span>
                {playing ? (
                  <span className="eq">
                    <i />
                    <i />
                    <i />
                  </span>
                ) : null}
                <div className="sound-actions">
                  {isBuiltin(sound) ? null : (
                    <button
                      className="icon"
                      title="重命名"
                      aria-label="重命名"
                      onClick={() => {
                        setRenameTarget(sound);
                        setRenameValue(sound.name);
                      }}
                    >
                      <PencilIcon size={12} />
                    </button>
                  )}
                  {isBuiltin(sound) ? null : (
                    <button
                      className="icon danger"
                      title="删除"
                      aria-label="删除"
                      onClick={() => setDeleteTarget(sound)}
                    >
                      <TrashIcon size={12} />
                    </button>
                  )}
                  <button
                    className="icon accent"
                    title="下载"
                    aria-label="下载"
                    disabled={exportingId === sound.id}
                    onClick={() => downloadSound(sound)}
                  >
                    <DownloadIcon size={12} />
                  </button>
                </div>
              </div>
            </div>
          );
        })}
      </div>

      <Modal
        open={newOpen}
        title="新增语音"
        wide
        onClose={closeNew}
        footer={
          <>
            <button className="btn" onClick={closeNew}>
              取消
            </button>
            <button className="btn btn-primary" onClick={saveNew} disabled={!canSave || importing}>
              {importing ? '转换中…' : '保存'}
            </button>
          </>
        }
      >
        <div className="seg-tabs">
          {CHANNELS.map((item) => (
            <button
              key={item.key}
              className={draft.channel === item.key ? 'active' : ''}
              onClick={() => switchChannel(item.key)}
            >
              {item.label}
            </button>
          ))}
        </div>

        <div className="new-panels">
          <div className={`new-panel upload${draft.channel === 'upload' ? ' on' : ''}`}>
            <div className="dropzone" onClick={pickFile}>
              <div className="dz-ico">
                <UploadIcon size={16} />
              </div>
              <b>{draft.picked ? '重新选择音频文件' : '选择音频文件'}</b>
              <p>WAV / MP3 / OGG，单文件不超过 20MB、120 秒</p>
            </div>
            {draft.picked ? (
              <div className="file-info">
                <div className="fi-ico">♪</div>
                <div style={{ minWidth: 0 }}>
                  <b>{draft.picked.name}</b>
                  <small>
                    {(draft.picked.size / 1024 / 1024).toFixed(2)} MB · 导入时转换为 WAV
                  </small>
                </div>
              </div>
            ) : null}
          </div>
          {['record', 'tts'].map((mode) => (
            <div
              key={mode}
              className={`new-panel${draft.channel === mode ? ' on' : ''}`}
              aria-hidden={draft.channel !== mode}
            >
              <div className="compose-area">
                <div className="compose-cols">
                  <div className="compose-col ring-col">
                    <div className="compose-head">
                      <span>① 报警铃声</span>
                      <label className="loops">
                        循环
                        <select
                          value={draft.loops}
                          onChange={(e) => {
                            patch({ loops: Number(e.target.value) });
                            resetComposed();
                          }}
                        >
                          {[1, 2, 3, 4, 5].map((n) => (
                            <option key={n} value={n}>
                              {n} 次
                            </option>
                          ))}
                        </select>
                      </label>
                    </div>
                    <div className="ringtone-list">
                      <button
                        className={`ringtone-item${draft.ringtoneId === '' ? ' active' : ''}`}
                        onClick={() => {
                          patch({ ringtoneId: '' });
                          resetComposed();
                        }}
                      >
                        无
                      </button>
                      {builtins.map((item) => (
                        <div
                          className={`ringtone-item${draft.ringtoneId === item.id ? ' active' : ''}`}
                          key={item.id}
                        >
                          <button
                            className="ringtone-name"
                            onClick={() => {
                              patch({ ringtoneId: item.id });
                              resetComposed();
                            }}
                          >
                            {item.name}
                          </button>
                          <button
                            className="ringtone-play"
                            aria-label="试听铃声"
                            onClick={() => preview(item)}
                          >
                            {playingId === item.id ? (
                              <StopIcon size={10} />
                            ) : (
                              <PlayIcon size={10} />
                            )}
                          </button>
                        </div>
                      ))}
                    </div>
                  </div>

                  <div className="compose-col voice-col">
                    <div className="compose-head">
                      <span>② {mode === 'record' ? '录制人声' : '文本人声'}</span>
                    </div>
                    {mode === 'record' ? (
                      <div className="record-box">
                        <div className="record-timer">{formatClock(draft.recordSeconds)}</div>
                        <div
                          className={`record-wave${draft.recordPhase === 'recording' ? ' on' : ''}`}
                        >
                          {Array.from({ length: 24 }).map((_, i) => (
                            <i key={i} style={{ animationDelay: `${(i % 7) * 0.08}s` }} />
                          ))}
                        </div>
                        <div className="voice-actions">
                          {draft.recordPhase === 'idle' ? (
                            <button className="btn btn-primary" onClick={startRecording}>
                              开始录音
                            </button>
                          ) : null}
                          {draft.recordPhase === 'recording' ? (
                            <button className="btn" onClick={stopRecording}>
                              停止
                            </button>
                          ) : null}
                          {draft.recordPhase === 'done' ? (
                            <>
                              <button
                                className="btn"
                                onClick={() =>
                                  playTemp(
                                    'voice',
                                    draft.recordResult.path,
                                    draft.recordResult.durationMs
                                  )
                                }
                              >
                                {previewing === 'voice' ? '停止' : '试听'}
                              </button>
                              <button
                                className="btn"
                                onClick={() => {
                                  patch({
                                    recordPhase: 'idle',
                                    recordSeconds: 0,
                                    recordResult: null,
                                    composed: null,
                                  });
                                }}
                              >
                                重录
                              </button>
                            </>
                          ) : null}
                        </div>
                        <p className="record-hint">
                          {draft.recordPhase === 'idle'
                            ? '首次录音会请求麦克风权限，最长 15 秒。'
                            : draft.recordPhase === 'recording'
                              ? '正在录音…'
                              : `已录制 ${formatDuration(draft.recordResult?.durationMs)}。`}
                        </p>
                      </div>
                    ) : (
                      <div className="tts-box">
                        <div className="tts-hint-row">
                          <span className="tts-hint">
                            {ttsInfo && !ttsInfo.ready
                              ? `TTS 不可用：${ttsInfo.message}`
                              : '输入播报内容后转为语音'}
                          </span>
                          <span className="counter">{draft.ttsText.length} / 50</span>
                        </div>
                        <textarea
                          rows={4}
                          maxLength={50}
                          value={draft.ttsText}
                          placeholder="例如：收到一条 SOC 系统的报警，请及时处理"
                          onChange={(e) =>
                            patch({ ttsText: e.target.value, ttsVoice: null, composed: null })
                          }
                        />
                        <div className="tts-actions">
                          <div className={`compose-submit${draft.ttsVoice ? ' split' : ''}`}>
                            <button
                              className="btn compose-btn"
                              onClick={generateTts}
                              disabled={draft.ttsBusy}
                            >
                              {draft.ttsBusy ? '生成中…' : '生成语音'}
                            </button>
                            {draft.ttsVoice ? (
                              <button
                                className="compose-play"
                                aria-label="试听生成的人声"
                                onClick={() =>
                                  playTemp(
                                    'voice',
                                    draft.ttsVoice.path,
                                    draft.ttsVoice.durationMs
                                  )
                                }
                              >
                                {previewing === 'voice' ? (
                                  <StopIcon size={11} />
                                ) : (
                                  <PlayIcon size={11} />
                                )}
                              </button>
                            ) : null}
                          </div>
                        </div>
                      </div>
                    )}
                  </div>
                </div>

                <div className="compose-track">
                  <div className="lane">
                    {ringtone ? (
                      <div className="lane-block ring">
                        <b>{ringtone.name}</b>
                        <span>× {draft.loops}</span>
                      </div>
                    ) : (
                      <div className="lane-block none">无铃声</div>
                    )}
                    <span className="lane-gap" aria-hidden="true" />
                    <div className={`lane-block voice${voice ? '' : ' empty'}`}>
                      {voice ? (
                        <>
                          <b>人声</b>
                          <span>{formatDuration(voice.durationMs)}</span>
                        </>
                      ) : mode === 'record' ? (
                        '待录制'
                      ) : (
                        '待生成'
                      )}
                    </div>
                  </div>
                  <div className={`merge-brace${draft.composed ? ' ready' : ''}`}>
                    <svg
                      viewBox="0 0 100 14"
                      preserveAspectRatio="none"
                      aria-hidden="true"
                      className="brace-svg"
                    >
                      <path
                        d="M1 1v6a3 3 0 0 0 3 3h92a3 3 0 0 0 3-3V1"
                        fill="none"
                        stroke="currentColor"
                        strokeWidth="1.2"
                      />
                    </svg>
                    <span className="brace-label">
                      {draft.composed
                        ? `已合成为一段语音 · ${formatDuration(draft.composed.durationMs)}`
                        : '合成为一段语音'}
                    </span>
                  </div>
                </div>
              </div>

              <div className="compose-actions">
                <div className={`compose-submit${draft.composed ? ' split' : ''}`}>
                  <button
                    className="btn compose-btn"
                    onClick={compose}
                    disabled={!voice || draft.composing}
                  >
                    {draft.composing ? (
                      <span className="compose-dots" aria-label="合成中">
                        <i />
                        <i />
                        <i />
                      </span>
                    ) : (
                      '开始合成'
                    )}
                  </button>
                  {draft.composed ? (
                    <button
                      className="compose-play"
                      aria-label="试听合成结果"
                      onClick={() =>
                        playTemp('final', draft.composed.path, draft.composed.durationMs)
                      }
                    >
                      {previewing === 'final' ? <StopIcon size={11} /> : <PlayIcon size={11} />}
                    </button>
                  ) : null}
                </div>
              </div>
            </div>
          ))}
        </div>

        <div className="field" style={{ marginTop: 14 }}>
          <label>语音名称</label>
          <input
            type="text"
            maxLength={40}
            value={draft.name}
            placeholder="例如：默认警报"
            onChange={(e) => patch({ name: e.target.value })}
          />
        </div>
        {error ? <p className="form-error">{error}</p> : null}
      </Modal>

      <Modal
        open={Boolean(renameTarget)}
        title="重命名语音"
        onClose={() => setRenameTarget(null)}
        footer={
          <>
            <button className="btn" onClick={() => setRenameTarget(null)}>
              取消
            </button>
            <button className="btn btn-primary" onClick={saveRename}>
              保存
            </button>
          </>
        }
      >
        <div className="field">
          <label>名称</label>
          <input
            type="text"
            maxLength={40}
            value={renameValue}
            onChange={(e) => setRenameValue(e.target.value)}
          />
        </div>
      </Modal>

      <ConfirmDialog
        open={Boolean(deleteTarget)}
        title={`删除语音「${deleteTarget?.name || ''}」？`}
        description={
          usedBy(deleteTarget?.id).length
            ? `该语音正被「${usedBy(deleteTarget?.id)
                .map((s) => s.name)
                .join('」「')}」使用。删除后这些策略会被自动停用并标记缺少语音。`
            : '删除后不可恢复，已播报记录中的语音名会保留。'
        }
        confirmText="删除"
        onCancel={() => setDeleteTarget(null)}
        onConfirm={removeSound}
      />
    </section>
  );
}

function formatDuration(ms) {
  if (!ms) return '—';
  return `${Math.max(1, Math.round(ms / 1000))} 秒`;
}

function formatClock(seconds) {
  const total = Math.floor(seconds);
  return `${String(Math.floor(total / 60)).padStart(2, '0')}:${String(total % 60).padStart(
    2,
    '0'
  )}`;
}
