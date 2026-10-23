import { useState } from 'react';
import { api } from '../api';
import ConfirmDialog from '../components/ConfirmDialog';
import Modal from '../components/Modal';
import Switch from '../components/Switch';
import { GripIcon, PlayIcon } from '../icons';

const emptyForm = {
  name: '',
  sender: '',
  keyword: '',
  soundId: '',
  playCount: 1,
  enabled: true,
};

export default function Strategies({ strategies, sounds, flash, onRefresh }) {
  const [modalOpen, setModalOpen] = useState(false);
  const [editing, setEditing] = useState(null);
  const [form, setForm] = useState(emptyForm);
  const [error, setError] = useState('');
  const [saving, setSaving] = useState(false);
  const [deleteTarget, setDeleteTarget] = useState(null);
  const [draggingId, setDraggingId] = useState(null);
  const [dropTarget, setDropTarget] = useState(null);

  const enabledCount = strategies.filter((s) => s.enabled).length;

  const openCreate = () => {
    setEditing(null);
    setForm({ ...emptyForm, soundId: sounds[0]?.id || '' });
    setError('');
    setModalOpen(true);
  };

  const openEdit = (strategy) => {
    setEditing(strategy);
    setForm({
      name: strategy.name,
      sender: strategy.sender,
      keyword: strategy.keyword,
      soundId: strategy.soundId || '',
      playCount: strategy.playCount || 1,
      enabled: strategy.enabled,
    });
    setError('');
    setModalOpen(true);
  };

  const save = async () => {
    if (!form.name.trim()) return setError('请填写策略名称');
    if (!form.keyword.trim()) return setError('请填写标题关键词');
    if (!form.soundId) return setError('请选择报警音频');
    setSaving(true);
    setError('');
    try {
      if (editing) {
        await api.updateStrategy(editing.id, form);
      } else {
        await api.createStrategy(form);
      }
      setModalOpen(false);
      await onRefresh();
      flash(editing ? '策略已保存' : '策略已新增');
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  };

  const toggle = async (strategy, checked) => {
    try {
      await api.setStrategyEnabled(strategy.id, checked);
      await onRefresh();
      flash(`「${strategy.name}」已${checked ? '启用' : '停用'}`);
    } catch (e) {
      flash(String(e));
    }
  };

  const confirmDelete = async () => {
    const target = deleteTarget;
    setDeleteTarget(null);
    if (!target) return;
    try {
      await api.deleteStrategy(target.id);
      await onRefresh();
      flash('策略已删除');
    } catch (e) {
      flash(String(e));
    }
  };

  const onDragStart = (event, strategy) => {
    setDraggingId(strategy.id);
    event.dataTransfer.effectAllowed = 'move';
    event.dataTransfer.setData('text/plain', strategy.id);
    const row = event.currentTarget.closest('.strategy-row');
    if (row) event.dataTransfer.setDragImage(row, 24, 24);
  };

  const onDragOver = (event, strategy) => {
    if (!draggingId) return;
    event.preventDefault();
    event.dataTransfer.dropEffect = 'move';
    if (strategy.id === draggingId) {
      setDropTarget(null);
      return;
    }
    const rect = event.currentTarget.getBoundingClientRect();
    const after = event.clientY > rect.top + rect.height / 2;
    setDropTarget({ id: strategy.id, after });
  };

  const onDrop = async (event, strategy) => {
    event.preventDefault();
    const after = dropTarget?.id === strategy.id ? dropTarget.after : false;
    const from = strategies.findIndex((s) => s.id === draggingId);
    setDraggingId(null);
    setDropTarget(null);
    if (from < 0 || strategy.id === draggingId) return;
    const ids = strategies.map((s) => s.id);
    const [moved] = ids.splice(from, 1);
    const target = ids.indexOf(strategy.id);
    if (target < 0) return;
    ids.splice(target + (after ? 1 : 0), 0, moved);
    try {
      await api.reorderStrategies(ids);
      await onRefresh();
      flash('策略顺序已更新');
    } catch (e) {
      flash(String(e));
    }
  };

  return (
    <section className="page">
      <div className="page-head">
        <h1>检查策略</h1>
        <div className="head-actions">
          <button className="btn btn-primary" onClick={openCreate}>
            ＋ 新增策略
          </button>
        </div>
      </div>

      <div className="strategy-meta">
        <span className="strategy-note">根据策略顺序从上至下匹配，同一封邮件仅命中播报一次</span>
        <span className="muted strategy-count">
          {strategies.length} 条策略 · {enabledCount} 条启用
        </span>
      </div>

      <div className="panel">
        <div className="table-head strategy-grid">
          <span />
          <span>策略名</span>
          <span>发件人</span>
          <span>标题关键词</span>
          <span>语音</span>
          <span>启用</span>
          <span>操作</span>
        </div>
        <div>
          {strategies.length === 0 ? (
            <div className="empty">
              <div className="empty-ico">◎</div>
              <b>还没有检查策略</b>
              <p>创建一条策略，指定发件人与标题关键词，命中后自动播放报警声。</p>
              <button className="btn btn-primary" onClick={openCreate}>
                ＋ 新增策略
              </button>
            </div>
          ) : (
            strategies.map((strategy) => {
              const sound = sounds.find((s) => s.id === strategy.soundId);
              return (
                <div
                  key={strategy.id}
                  className={`strategy-row strategy-grid ${
                    draggingId === strategy.id ? 'dragging' : ''
                  } ${
                    dropTarget?.id === strategy.id
                      ? dropTarget.after
                        ? 'drag-over-after'
                        : 'drag-over-before'
                      : ''
                  }`}
                  onDragOver={(e) => onDragOver(e, strategy)}
                  onDrop={(e) => onDrop(e, strategy)}
                >
                  <div className="drag-cell">
                    <span
                      className="drag-handle"
                      draggable
                      title="拖动排序"
                      onDragStart={(e) => onDragStart(e, strategy)}
                      onDragEnd={() => {
                        setDraggingId(null);
                        setDropTarget(null);
                      }}
                    >
                      <GripIcon />
                    </span>
                  </div>
                  <div className="strategy-name" title={strategy.name}>
                    {strategy.name}
                  </div>
                  <div>
                    {strategy.sender ? (
                      <div className="sender-email" title={strategy.sender}>
                        {strategy.sender}
                      </div>
                    ) : (
                      <span className="sender-any">任意发件人</span>
                    )}
                  </div>
                  <div>
                    <code className="keyword">{strategy.keyword}</code>
                  </div>
                  <div>
                    <span className={`sound-chip${sound ? '' : ' missing'}`}>
                      {sound ? sound.name : '缺少语音'}
                    </span>
                    <span className="mult">× {strategy.playCount} 次</span>
                  </div>
                  <div>
                    <Switch
                      checked={strategy.enabled}
                      onChange={(checked) => toggle(strategy, checked)}
                      title={strategy.enabled ? '停用' : '启用'}
                    />
                  </div>
                  <div className="row-actions">
                    <button onClick={() => openEdit(strategy)}>编辑</button>
                    <button className="danger" onClick={() => setDeleteTarget(strategy)}>
                      删除
                    </button>
                  </div>
                </div>
              );
            })
          )}
        </div>
      </div>

      <Modal
        open={modalOpen}
        title={editing ? '编辑策略' : '新增策略'}
        onClose={() => setModalOpen(false)}
        footer={
          <>
            <button className="btn" onClick={() => setModalOpen(false)}>
              取消
            </button>
            <button className="btn btn-primary" onClick={save} disabled={saving}>
              {saving ? '保存中…' : '保存'}
            </button>
          </>
        }
      >
        <div className="field">
          <label>
            策略名称 <span className="req">*</span>
          </label>
          <input
            type="text"
            maxLength={40}
            value={form.name}
            placeholder="例如：SOC 安全告警"
            onChange={(e) => setForm({ ...form, name: e.target.value })}
          />
        </div>
        <div className="field">
          <label>
            发件人邮箱 <span className="opt">可选</span>
          </label>
          <input
            type="text"
            value={form.sender}
            placeholder="aqyyptsoc@dc.icbc.com.cn"
            onChange={(e) => setForm({ ...form, sender: e.target.value })}
          />
          <p className="hint">
            精确匹配发件人地址，忽略显示名与大小写。留空表示匹配任意发件人，可能产生转发噪音。
          </p>
        </div>
        <div className="field">
          <label>
            标题关键词 <span className="req">*</span>
          </label>
          <input
            type="text"
            maxLength={100}
            value={form.keyword}
            placeholder="SOC_安全告警"
            onChange={(e) => setForm({ ...form, keyword: e.target.value })}
          />
          <p className="hint">作为服务端检索关键词，同时会在本地再校验标题是否包含该关键词。</p>
        </div>
        <div className="field">
          <label>
            报警音频 <span className="req">*</span>
          </label>
          <div className="field-row">
            <select
              value={form.soundId}
              onChange={(e) => setForm({ ...form, soundId: e.target.value })}
            >
              {sounds.length === 0 ? <option value="">请先在语音库添加语音</option> : null}
              {sounds.map((sound) => (
                <option key={sound.id} value={sound.id}>
                  {sound.name} · {formatDuration(sound.durationMs)}
                </option>
              ))}
            </select>
            <button
              className="btn"
              type="button"
              onClick={() =>
                form.soundId
                  ? api.previewSound(form.soundId).catch((e) => flash(String(e)))
                  : flash('请先选择语音')
              }
            >
              <PlayIcon />
              试听
            </button>
          </div>
        </div>
        <div className="field">
          <label>播报次数</label>
          <div className="segmented">
            {[1, 2, 3].map((count) => (
              <button
                key={count}
                type="button"
                className={form.playCount === count ? 'active' : ''}
                onClick={() => setForm({ ...form, playCount: count })}
              >
                {count} 次
              </button>
            ))}
          </div>
          <p className="hint">多次播放会依次进行，不会与其他策略的报警重叠。</p>
        </div>
        <div className="field field-inline">
          <div className="label">
            启用
            <small>停用后不参与轮询</small>
          </div>
          <Switch
            checked={form.enabled}
            onChange={(checked) => setForm({ ...form, enabled: checked })}
          />
        </div>
        {error ? <p className="form-error">{error}</p> : null}
      </Modal>

      <ConfirmDialog
        open={Boolean(deleteTarget)}
        title={`删除策略「${deleteTarget?.name || ''}」？`}
        description="历史播报记录会保留，删除后该策略不再参与轮询。"
        confirmText="删除"
        onCancel={() => setDeleteTarget(null)}
        onConfirm={confirmDelete}
      />
    </section>
  );
}

function formatDuration(ms) {
  if (!ms) return '—';
  const seconds = Math.max(1, Math.round(ms / 1000));
  return `${seconds} 秒`;
}
