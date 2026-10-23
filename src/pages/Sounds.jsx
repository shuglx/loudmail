import { useEffect, useRef, useState } from 'react';
import { api } from '../api';
import ConfirmDialog from '../components/ConfirmDialog';
import Modal from '../components/Modal';
import { InfoIcon, MicIcon, PlayIcon, StopIcon, UploadIcon } from '../icons';

const sourceLabel = { builtin: '内置', upload: '上传', record: '录音' };

export default function Sounds({ sounds, strategies, flash, onRefresh }) {
  const [playingId, setPlayingId] = useState(null);
  const [uploadOpen, setUploadOpen] = useState(false);
  const [picked, setPicked] = useState(null);
  const [uploadName, setUploadName] = useState('');
  const [uploadError, setUploadError] = useState('');
  const [importing, setImporting] = useState(false);
  const [recordOpen, setRecordOpen] = useState(false);
  const [recordPhase, setRecordPhase] = useState('idle');
  const [recordSeconds, setRecordSeconds] = useState(0);
  const [recordResult, setRecordResult] = useState(null);
  const [recordName, setRecordName] = useState('');
  const [recordError, setRecordError] = useState('');
  const [renameTarget, setRenameTarget] = useState(null);
  const [renameValue, setRenameValue] = useState('');
  const [deleteTarget, setDeleteTarget] = useState(null);
  const playTimer = useRef(null);
  const recordTimer = useRef(null);

  useEffect(
    () => () => {
      if (playTimer.current) clearTimeout(playTimer.current);
      if (recordTimer.current) clearInterval(recordTimer.current);
    },
    []
  );

  const usedBy = (id) => strategies.filter((s) => s.soundId === id);

  const preview = async (sound) => {
    if (playingId === sound.id) {
      try {
        await api.stopPlayback();
      } catch {
        // ignore
      }
      setPlayingId(null);
      return;
    }
    try {
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

  const openUpload = () => {
    setPicked(null);
    setUploadName('');
    setUploadError('');
    setUploadOpen(true);
  };

  const pickFile = async () => {
    try {
      const file = await api.pickAudioFile();
      if (!file) return;
      setPicked(file);
      setUploadName(file.name.replace(/\.(wav|mp3|ogg)$/i, ''));
      setUploadError('');
    } catch (e) {
      setUploadError(String(e));
    }
  };

  const importSound = async () => {
    if (!picked) return setUploadError('请先选择音频文件');
    if (!uploadName.trim()) return setUploadError('请填写语音名称');
    setImporting(true);
    setUploadError('');
    try {
      await api.importSound(picked.path, uploadName.trim());
      setUploadOpen(false);
      await onRefresh();
      flash('语音已导入并转换为 WAV');
    } catch (e) {
      setUploadError(String(e));
    } finally {
      setImporting(false);
    }
  };

  const openRecord = () => {
    setRecordPhase('idle');
    setRecordSeconds(0);
    setRecordResult(null);
    setRecordName('');
    setRecordError('');
    setRecordOpen(true);
  };

  const closeRecord = async () => {
    if (recordTimer.current) clearInterval(recordTimer.current);
    recordTimer.current = null;
    if (recordPhase === 'recording') {
      try {
        await api.stopRecording();
      } catch {
        // ignore
      }
    }
    setRecordOpen(false);
  };

  const startRecording = async () => {
    setRecordError('');
    try {
      await api.startRecording();
      setRecordPhase('recording');
      setRecordSeconds(0);
      if (recordTimer.current) clearInterval(recordTimer.current);
      recordTimer.current = setInterval(() => {
        setRecordSeconds((value) => {
          const next = value + 0.1;
          if (next >= 60) {
            clearInterval(recordTimer.current);
            recordTimer.current = null;
            stopRecording();
          }
          return next;
        });
      }, 100);
    } catch (e) {
      setRecordError(String(e));
    }
  };

  const stopRecording = async () => {
    if (recordTimer.current) clearInterval(recordTimer.current);
    recordTimer.current = null;
    try {
      const result = await api.stopRecording();
      setRecordResult(result);
      setRecordPhase('done');
      setRecordName((value) => value || '新录音');
    } catch (e) {
      setRecordError(String(e));
      setRecordPhase('idle');
    }
  };

  const saveRecording = async () => {
    if (!recordResult) return;
    if (!recordName.trim()) return setRecordError('请填写语音名称');
    setRecordError('');
    try {
      await api.saveRecording(recordResult.path, recordName.trim(), recordResult.durationMs);
      setRecordOpen(false);
      await onRefresh();
      flash('录音已保存到语音库');
    } catch (e) {
      setRecordError(String(e));
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

  return (
    <section className="page">
      <div className="page-head">
        <h1>语音库</h1>
        <div className="head-actions">
          <button className="btn" onClick={openUpload}>
            <UploadIcon size={14} />
            上传音频
          </button>
          <button className="btn btn-primary" onClick={openRecord}>
            <MicIcon size={14} />
            录音
          </button>
        </div>
      </div>

      <div className="notice">
        <InfoIcon size={15} />
        <span>支持 WAV / MP3 / OGG，导入后统一转换为 WAV 保存在本地。录音需要系统麦克风权限，最长 60 秒。</span>
      </div>

      <div className="sound-grid">
        {sounds.map((sound) => {
          const playing = playingId === sound.id;
          return (
            <div className={`sound-card${playing ? ' playing' : ''}`} key={sound.id}>
              <div className="sound-top">
                <button
                  className="play-btn"
                  onClick={() => preview(sound)}
                  aria-label={playing ? '停止' : '试听'}
                >
                  {playing ? <StopIcon /> : <PlayIcon />}
                </button>
                <div className="sound-name" title={sound.name}>
                  {sound.name}
                </div>
                <span className="tag">{sourceLabel[sound.source] || sound.source}</span>
              </div>
              <div className="sound-meta">
                <span>
                  {formatDuration(sound.durationMs)} · 被 {usedBy(sound.id).length} 条策略使用
                </span>
                {playing ? (
                  <span className="eq">
                    <i />
                    <i />
                    <i />
                  </span>
                ) : null}
              </div>
              <div className="sound-actions">
                <button
                  onClick={() => {
                    setRenameTarget(sound);
                    setRenameValue(sound.name);
                  }}
                >
                  重命名
                </button>
                <button className="danger" onClick={() => setDeleteTarget(sound)}>
                  删除
                </button>
              </div>
            </div>
          );
        })}
        <div className="sound-card tts-card">
          <div className="tts-ico">TTS</div>
          <div style={{ flex: 1 }}>
            <b>文本转语音</b>
            <p>输入文字生成提示音，后续版本支持</p>
          </div>
          <span className="tag">后续</span>
        </div>
      </div>

      <Modal
        open={uploadOpen}
        title="上传音频"
        onClose={() => setUploadOpen(false)}
        footer={
          <>
            <button className="btn" onClick={() => setUploadOpen(false)}>
              取消
            </button>
            <button
              className="btn btn-primary"
              onClick={importSound}
              disabled={!picked || importing}
            >
              {importing ? '转换中…' : '导入'}
            </button>
          </>
        }
      >
        <div className="dropzone" onClick={pickFile}>
          <div className="dz-ico">
            <UploadIcon size={16} />
          </div>
          <b>{picked ? '重新选择音频文件' : '选择音频文件'}</b>
          <p>WAV / MP3 / OGG，单文件不超过 20MB、120 秒</p>
        </div>
        {picked ? (
          <div className="file-info">
            <div className="fi-ico">♪</div>
            <div style={{ minWidth: 0 }}>
              <b>{picked.name}</b>
              <small>{(picked.size / 1024 / 1024).toFixed(2)} MB · 导入时转换为 WAV</small>
            </div>
          </div>
        ) : null}
        <div className="field" style={{ marginTop: 14 }}>
          <label>语音名称</label>
          <input
            type="text"
            maxLength={40}
            value={uploadName}
            placeholder="例如：默认警报"
            onChange={(e) => setUploadName(e.target.value)}
          />
        </div>
        {uploadError ? <p className="form-error">{uploadError}</p> : null}
      </Modal>

      <Modal
        open={recordOpen}
        title="录制语音"
        onClose={closeRecord}
        footer={
          <>
            <button className="btn" onClick={closeRecord}>
              取消
            </button>
            <button
              className="btn btn-primary"
              onClick={saveRecording}
              disabled={recordPhase !== 'done'}
            >
              保存
            </button>
          </>
        }
      >
        <div className="record-box">
          <div className="record-timer">{formatClock(recordSeconds)}</div>
          <div className={`record-wave${recordPhase === 'recording' ? ' on' : ''}`}>
            {Array.from({ length: 24 }).map((_, i) => (
              <i key={i} style={{ animationDelay: `${(i % 7) * 0.08}s` }} />
            ))}
          </div>
          {recordPhase === 'idle' ? (
            <button className="btn btn-primary" onClick={startRecording}>
              开始录音
            </button>
          ) : null}
          {recordPhase === 'recording' ? (
            <button className="btn" onClick={stopRecording}>
              停止
            </button>
          ) : null}
          {recordPhase === 'done' ? (
            <button
              className="btn"
              onClick={() => {
                setRecordPhase('idle');
                setRecordSeconds(0);
                setRecordResult(null);
              }}
            >
              重录
            </button>
          ) : null}
          <p className="record-hint">
            {recordPhase === 'idle'
              ? '首次录音会请求系统麦克风权限。单声道，最长 60 秒。'
              : recordPhase === 'recording'
                ? '正在录音…'
                : `已录制 ${formatDuration(recordResult?.durationMs)}。确认名称后保存。`}
          </p>
        </div>
        {recordPhase === 'done' ? (
          <div className="record-save">
            <div className="field">
              <label>语音名称</label>
              <input
                type="text"
                maxLength={40}
                value={recordName}
                placeholder="例如：交接班提醒"
                onChange={(e) => setRecordName(e.target.value)}
              />
            </div>
          </div>
        ) : null}
        {recordError ? <p className="form-error">{recordError}</p> : null}
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
