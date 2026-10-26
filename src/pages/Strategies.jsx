import { useEffect, useRef, useState } from 'react';
import { api } from '../api';
import ConfirmDialog from '../components/ConfirmDialog';
import Modal from '../components/Modal';
import Select from '../components/Select';
import Switch from '../components/Switch';
import {
  ExportIcon,
  GripIcon,
  ImportIcon,
  PencilIcon,
  PlayIcon,
  TrashIcon,
} from '../icons';

const MATCH_MODES = [
  { key: 'contains', label: '模糊匹配', symbol: '≈' },
  { key: 'prefix', label: '以此开头', symbol: '^' },
  { key: 'suffix', label: '以此结尾', symbol: '$' },
];

const modeMeta = (mode) =>
  MATCH_MODES.find((item) => item.key === mode) || MATCH_MODES[0];

const emptyForm = {
  name: '',
  sender: '',
  keyword: '',
  matchMode: 'contains',
  filterReplies: true,
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
  // 拖动排序：drag 保存被拖动的策略 id 与实时预览的顺序
  const [drag, setDrag] = useState(null);
  const dragRef = useRef(null);
  const [importTarget, setImportTarget] = useState(null);

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
      matchMode: strategy.matchMode || 'contains',
      filterReplies: strategy.filterReplies !== false,
      soundId: strategy.soundId || '',
      playCount: strategy.playCount || 1,
      enabled: strategy.enabled,
    });
    setError('');
    setModalOpen(true);
  };

  const save = async () => {
    if (!form.name.trim()) return setError('请填写策略名称');
    if (!form.sender.trim()) return setError('请填写发件人邮箱');
    if (!form.sender.includes('@')) return setError('发件人邮箱格式不正确');
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

  // 用 pointer 事件自行实现拖拽：没有原生拖影与「禁止」光标
  const startDrag = (event, strategy) => {
    if (event.button !== 0) return;
    event.preventDefault();
    const baseIds = strategies.map((s) => s.id);
    setDrag({
      id: strategy.id,
      name: strategy.name,
      baseIds,
      ids: baseIds,
      x: event.clientX,
      y: event.clientY,
    });
  };

  useEffect(() => {
    dragRef.current = drag;
  }, [drag]);

  const dragging = Boolean(drag);

  useEffect(() => {
    if (!dragging) return undefined;
    document.body.classList.add('is-row-dragging');

    const onMove = (event) => {
      setDrag((prev) => {
        if (!prev) return prev;
        const rest = prev.baseIds.filter((id) => id !== prev.id);
        const rows = Array.from(document.querySelectorAll('.strategy-row')).filter(
          (node) => node.dataset.id !== prev.id
        );
        let insertAt = rest.length;
        for (let i = 0; i < rows.length; i += 1) {
          const rect = rows[i].getBoundingClientRect();
          if (event.clientY < rect.top + rect.height / 2) {
            insertAt = Math.min(i, rest.length);
            break;
          }
        }
        const next = [...rest.slice(0, insertAt), prev.id, ...rest.slice(insertAt)];
        const updated = { ...prev, ids: next, x: event.clientX, y: event.clientY };
        // 同步给 ref，避免 pointerup 时读到上一帧的顺序
        dragRef.current = updated;
        return updated;
      });
    };

    const finish = async () => {
      const current = dragRef.current;
      setDrag(null);
      if (!current) return;
      if (current.ids.join('|') === current.baseIds.join('|')) return;
      try {
        await api.reorderStrategies(current.ids);
        await onRefresh();
        flash('策略顺序已更新');
      } catch (e) {
        flash(String(e));
      }
    };

    // 用鼠标事件而不是 Pointer Events：麒麟自带 WebKitGTK 2.28 不支持 PointerEvent，
    // 之前 onPointerDown 根本不触发，拖动完全没反应
    window.addEventListener('mousemove', onMove);
    window.addEventListener('mouseup', finish);
    return () => {
      document.body.classList.remove('is-row-dragging');
      window.removeEventListener('mousemove', onMove);
      window.removeEventListener('mouseup', finish);
    };
  }, [dragging]);

  const ordered = drag
    ? drag.ids.map((id) => strategies.find((s) => s.id === id)).filter(Boolean)
    : strategies;

  const exportStrategies = async () => {
    try {
      const path = await api.exportStrategies();
      if (path) flash('策略已导出');
    } catch (e) {
      flash(String(e));
    }
  };

  const pickImportFile = async () => {
    try {
      const preview = await api.pickStrategiesFile();
      if (preview) setImportTarget(preview);
    } catch (e) {
      flash(String(e));
    }
  };

  const confirmImport = async () => {
    const target = importTarget;
    setImportTarget(null);
    if (!target) return;
    try {
      const result = await api.importStrategies(target.path);
      await onRefresh();
      const parts = [`新增 ${result.added} 条`];
      if (result.updated) parts.push(`覆盖 ${result.updated} 条`);
      if (result.skipped) parts.push(`跳过 ${result.skipped} 条`);
      flash(`导入完成：${parts.join('，')}`);
    } catch (e) {
      flash(String(e));
    }
  };

  return (
    <section className="page">
      <div className="page-head">
        <h1>匹配策略</h1>
        <div className="head-actions">
          <div className="head-split">
            <button title="导出策略到 JSON 文件" onClick={exportStrategies}>
              <ExportIcon size={14} />
              导出
            </button>
            <button title="从 JSON 文件导入策略" onClick={pickImportFile}>
              <ImportIcon size={14} />
              导入
            </button>
          </div>
          <button className="btn btn-primary" onClick={openCreate}>
            ＋ 新增策略
          </button>
        </div>
      </div>

      <div className="strategy-meta">
        <span className="strategy-note">根据策略顺序从上至下匹配，同一封邮件仅会命中和播报一次</span>
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
          <span />
        </div>
        <div>
          {strategies.length === 0 ? (
            <div className="empty">
              <div className="empty-ico">◎</div>
              <b>还没有匹配策略</b>
              <p>创建一条策略，指定发件人与标题关键词，命中后自动播放报警声。</p>
              <button className="btn btn-primary" onClick={openCreate}>
                ＋ 新增策略
              </button>
            </div>
          ) : (
            ordered.map((strategy) => {
              const sound = sounds.find((s) => s.id === strategy.soundId);
              const mode = modeMeta(strategy.matchMode);
              return (
                <div
                  key={strategy.id}
                  data-id={strategy.id}
                  className={`strategy-row strategy-grid${
                    drag?.id === strategy.id ? ' dragging' : ''
                  }`}
                >
                  <div className="drag-cell">
                    <span
                      className="drag-handle"
                      title="拖动排序"
                      onMouseDown={(e) => startDrag(e, strategy)}
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
                    <span
                      className="keyword-cell"
                      title={`${mode.label}：${strategy.keyword}`}
                    >
                      <span className="kw-sym">{mode.symbol}</span>
                      <code className="keyword">{strategy.keyword}</code>
                    </span>
                  </div>
                  <div>
                    <span className={`sound-chip${sound ? '' : ' missing'}`}>
                      {sound ? sound.name : '缺少语音'}
                    </span>
                  </div>
                  <div>
                    <Switch
                      checked={strategy.enabled}
                      onChange={(checked) => toggle(strategy, checked)}
                      title={strategy.enabled ? '停用' : '启用'}
                    />
                  </div>
                  <div className="row-actions">
                    <button
                      className="icon"
                      title="编辑"
                      aria-label="编辑"
                      onClick={() => openEdit(strategy)}
                    >
                      <PencilIcon size={12} />
                    </button>
                    <button
                      className="icon danger"
                      title="删除"
                      aria-label="删除"
                      onClick={() => setDeleteTarget(strategy)}
                    >
                      <TrashIcon size={12} />
                    </button>
                  </div>
                </div>
              );
            })
          )}
        </div>
      </div>

      {drag ? (
        <div className="drag-ghost" style={{ top: drag.y + 14, left: drag.x + 16 }}>
          <GripIcon />
          <span>{drag.name}</span>
        </div>
      ) : null}

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
            placeholder="XXX告警"
            onChange={(e) => setForm({ ...form, name: e.target.value })}
          />
        </div>
        <div className="field">
          <label>
            发件人邮箱 <span className="req">*</span>
          </label>
          <input
            type="text"
            value={form.sender}
            placeholder="abc@xxx.com.cn"
            onChange={(e) => setForm({ ...form, sender: e.target.value })}
          />
          <p className="hint">
            模糊匹配发件人地址（忽略显示名与大小写）
          </p>
        </div>
        <div className="field">
          <label>
            标题关键词 <span className="req">*</span>
          </label>
          <div className="field-row">
            <input
              type="text"
              maxLength={100}
              value={form.keyword}
              placeholder="XXX邮件"
              onChange={(e) => setForm({ ...form, keyword: e.target.value })}
            />
            <div className="segmented compact">
              {MATCH_MODES.map((item) => (
                <button
                  key={item.key}
                  type="button"
                  className={form.matchMode === item.key ? 'active' : ''}
                  onClick={() => setForm({ ...form, matchMode: item.key })}
                >
                  {item.label}
                </button>
              ))}
            </div>
          </div>
          <p className="hint">
            服务端按关键词检索，本地再按所选方式校验标题（忽略大小写和首尾空格）
          </p>
        </div>
        <div className="field field-inline">
          <div className="label">
            过滤回复和转发的邮件
            <small>跳过标题以“转发/回复 + 冒号”开头的邮件</small>
          </div>
          <Switch
            checked={form.filterReplies}
            onChange={(checked) => setForm({ ...form, filterReplies: checked })}
          />
        </div>
        <div className="field">
          <label>
            报警音频 <span className="req">*</span>
          </label>
          <div className="field-row">
            <Select
              value={form.soundId}
              options={
                sounds.length === 0
                  ? [{ value: '', label: '请先在语音库添加语音' }]
                  : sounds.map((sound) => ({
                      value: sound.id,
                      label: `${sound.name} · ${formatDuration(sound.durationMs)}`,
                    }))
              }
              onChange={(value) => setForm({ ...form, soundId: value })}
            />
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
          <p className="hint">多次播放会依次进行，不会与其他策略的报警重叠</p>
        </div>
        <div className="field field-inline">
          <div className="label">
            启用
            <small>停用后不参与轮询匹配</small>
          </div>
          <Switch
            checked={form.enabled}
            onChange={(checked) => setForm({ ...form, enabled: checked })}
          />
        </div>
        {error ? <p className="form-error">{error}</p> : null}
      </Modal>

      <ConfirmDialog
        open={Boolean(importTarget)}
        title={`导入 ${importTarget?.count ?? 0} 条策略？`}
        description="同名策略会被覆盖（保留其列表位置），文件中不存在于本机的策略按文件顺序追加，本机其它策略保持不动；本机没有的语音会被置空。"
        confirmText="导入"
        onCancel={() => setImportTarget(null)}
        onConfirm={confirmImport}
      />

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
