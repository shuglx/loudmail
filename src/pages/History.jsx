import { useMemo, useState } from 'react';
import { api } from '../api';
import ConfirmDialog from '../components/ConfirmDialog';
import Modal from '../components/Modal';
import Select from '../components/Select';
import { SearchIcon, TrashIcon } from '../icons';

function pageList(current, total) {
  if (total <= 7) return Array.from({ length: total }, (_, i) => i + 1);
  const set = new Set([1, total, current - 1, current, current + 1]);
  if (current <= 3) [2, 3, 4].forEach((n) => set.add(n));
  if (current >= total - 2) [total - 1, total - 2, total - 3].forEach((n) => set.add(n));
  const sorted = [...set].filter((n) => n >= 1 && n <= total).sort((a, b) => a - b);
  const out = [];
  let prev = 0;
  for (const n of sorted) {
    if (n - prev > 1) out.push('gap');
    out.push(n);
    prev = n;
  }
  return out;
}

function confirmLabel(record) {
  if (!record.requireConfirm) return '无需确认';
  return record.confirmed ? '已确认' : '待确认';
}

function confirmDetail(record) {
  const state = confirmLabel(record);
  return record.confirmed && record.confirmedAt ? `${state} · ${record.confirmedAt}` : state;
}

export default function History({ history, flash, onCleared }) {
  const [query, setQuery] = useState('');
  const [filter, setFilter] = useState('all');
  const [pageSize, setPageSize] = useState(10);
  const [page, setPage] = useState(1);
  // 只存 id，记录本身从 history 里取，确认/重复播报后详情自动跟着更新
  const [selectedId, setSelectedId] = useState(null);
  const [clearOpen, setClearOpen] = useState(false);

  const strategyNames = useMemo(
    () => Array.from(new Set(history.map((h) => h.strategyName))),
    [history]
  );

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    return history.filter((record) => {
      if (filter !== 'all' && record.strategyName !== filter) return false;
      if (!q) return true;
      return [record.subject, record.sender, record.strategyName, record.mailId]
        .join(' ')
        .toLowerCase()
        .includes(q);
    });
  }, [history, query, filter]);

  const total = filtered.length;
  const totalPages = Math.max(1, Math.ceil(total / pageSize));
  const currentPage = Math.min(page, totalPages);
  const start = (currentPage - 1) * pageSize;
  const items = filtered.slice(start, start + pageSize);
  const from = total === 0 ? 0 : start + 1;
  const to = Math.min(start + pageSize, total);
  const selected = history.find((record) => record.id === selectedId) || null;

  const setQueryAndReset = (value) => {
    setQuery(value);
    setPage(1);
  };
  const setFilterAndReset = (value) => {
    setFilter(value);
    setPage(1);
  };
  const setPageSizeAndReset = (value) => {
    setPageSize(Number(value));
    setPage(1);
  };

  const clear = async () => {
    setClearOpen(false);
    try {
      const kept = await api.clearHistory();
      setPage(1);
      await onCleared();
      flash(kept ? `已清空，保留 ${kept} 条待确认记录` : '播报记录已清空');
    } catch (e) {
      flash(String(e));
    }
  };

  const confirm = async (event, record) => {
    // 不要冒泡到整行，否则会顺带打开详情弹窗
    event.stopPropagation();
    if (record.confirmed) return;
    try {
      await api.confirmHistory(record.id);
      flash('已确认收到');
    } catch (e) {
      flash(String(e));
    }
  };

  return (
    <section className="page">
      <div className="page-head">
        <h1>播报记录</h1>
        <div className="head-actions">
          <button className="btn" onClick={() => setClearOpen(true)}>
            <TrashIcon />
            清空记录
          </button>
        </div>
      </div>

      <div className="toolbar">
        <label className="search">
          <SearchIcon size={14} />
          <input
            type="text"
            value={query}
            placeholder="搜索标题、发件人或策略"
            onChange={(e) => setQueryAndReset(e.target.value)}
          />
        </label>
        <Select
          value={filter}
          options={[
            { value: 'all', label: '全部策略' },
            ...strategyNames.map((name) => ({ value: name, label: name })),
          ]}
          onChange={setFilterAndReset}
        />
        <Select
          value={pageSize}
          options={[
            { value: 10, label: '每页 10 条' },
            { value: 25, label: '每页 25 条' },
            { value: 50, label: '每页 50 条' },
          ]}
          onChange={setPageSizeAndReset}
        />
        <span className="toolbar-note">滚动保留最新 1000 条，待确认记录不受限</span>
      </div>

      {total > 0 ? (
        <div className="pager">
          <span className="pager-info">
            第 {from}–{to} 条，共 {total} 条
          </span>
          <div className="pager-actions">
            <button disabled={currentPage === 1} onClick={() => setPage(currentPage - 1)}>
              上一页
            </button>
            {pageList(currentPage, totalPages).map((n, index) =>
              n === 'gap' ? (
                <span className="ellipsis" key={`gap-${index}`}>
                  …
                </span>
              ) : (
                <button
                  key={n}
                  className={n === currentPage ? 'active' : ''}
                  onClick={() => setPage(n)}
                >
                  {n}
                </button>
              )
            )}
            <button
              disabled={currentPage === totalPages}
              onClick={() => setPage(currentPage + 1)}
            >
              下一页
            </button>
          </div>
        </div>
      ) : null}

      <div className="panel">
        <div className="table-head history-head">
          <span>命中时间</span>
          <span>命中策略</span>
          <span>邮件信息</span>
          <span>人工确认</span>
          <span>播报语音</span>
        </div>
        {total === 0 ? (
          <div className="empty">
            <div className="empty-ico">◷</div>
            <b>{history.length ? '没有匹配的记录' : '还没有播报记录'}</b>
            <p>
              {history.length ? '试试更换搜索词或策略筛选。' : '策略命中新邮件后，播报记录会出现在这里。'}
            </p>
          </div>
        ) : (
          items.map((record) => {
            // 已错过第一时间确认：重复播报过且仍未确认
            const overdue = record.rebroadcastCount > 0 && !record.confirmed;
            return (
            <div
              className={`history-row${overdue ? ' overdue' : ''}`}
              key={record.id}
              role="button"
              tabIndex={0}
              onClick={() => setSelectedId(record.id)}
              onKeyDown={(event) => {
                // 焦点在「确认」按钮上时交给按钮自己处理
                if (event.target !== event.currentTarget) return;
                if (event.key === 'Enter' || event.key === ' ') {
                  event.preventDefault();
                  setSelectedId(record.id);
                }
              }}
            >
              <span className="history-grid">
                <span className="history-time">
                  <span className="history-date">{record.time.slice(5, 16)}</span>
                  {overdue ? (
                    <span className="history-time-tag">
                      <span className="tag timeout" title="未在第一时间确认，已重复播报">
                        重复播报
                      </span>
                    </span>
                  ) : null}
                </span>
                <span className="history-strategy" title={record.strategyName}>
                  {record.strategyName}
                </span>
                <span className="history-main">
                  <b
                    title={
                      record.rebroadcastCount > 0
                        ? `${record.subject}（已重复播报 ${record.rebroadcastCount} 次，未在第一时间确认）`
                        : record.subject
                    }
                  >
                    {record.subject}
                  </b>
                  <span>{record.sender}</span>
                </span>
                <span className="history-confirm">
                  {record.requireConfirm && !record.confirmed ? (
                    <button
                      className="confirm-btn"
                      type="button"
                      title="确认已收到，不再重复播报"
                      onClick={(event) => confirm(event, record)}
                    >
                      确认
                    </button>
                  ) : record.requireConfirm ? (
                    <button className="confirm-btn confirmed" type="button" disabled>
                      已确认
                    </button>
                  ) : (
                    <button className="confirm-btn done" type="button" disabled>
                      无需确认
                    </button>
                  )}
                </span>
                <span className="history-side">
                  <span
                    className="tag"
                    title={`${record.soundName} × ${record.playCount}`}
                  >
                    {record.soundName} × {record.playCount}
                  </span>
                </span>
              </span>
            </div>
            );
          })
        )}
      </div>

      <Modal
        open={Boolean(selected)}
        title="播报详情"
        onClose={() => setSelectedId(null)}
        footer={
          <button className="btn" onClick={() => setSelectedId(null)}>
            关闭
          </button>
        }
      >
        {selected ? (
          <>
            <div className="detail-list">
              <Detail k="播报时间" v={selected.time} />
              <Detail k="命中策略" v={selected.strategyName} />
              <Detail k="发件人" v={selected.sender} />
              <Detail k="邮件标题" v={selected.subject} />
              <Detail k="播放语音" v={selected.soundName} />
              <Detail k="播报次数" v={`${selected.playCount} 次`} />
              <Detail k="确认状态" v={confirmDetail(selected)} />
              {selected.rebroadcastCount > 0 ? (
                <Detail
                  k="重复播报"
                  v={`${selected.rebroadcastCount} 次${
                    selected.lastBroadcastAt ? ` · 最近 ${selected.lastBroadcastAt}` : ''
                  }`}
                />
              ) : null}
              <Detail k="邮件 ID" v={selected.mailId} mono />
              <div className="detail-item detail-summary">
                <span className="k">邮件摘要</span>
                <textarea
                  readOnly
                  value={selected.summary || ''}
                  placeholder="无摘要"
                  rows={4}
                />
              </div>
            </div>
          </>
        ) : null}
      </Modal>

      <ConfirmDialog
        open={clearOpen}
        title="清空播报记录？"
        description="只清理「无需确认」和「已确认」的记录；待确认的记录会保留，必须人工确认后才会被清理或滚动淘汰。清空后不影响策略游标，已播报的邮件不会重复播报。"
        confirmText="清空"
        onCancel={() => setClearOpen(false)}
        onConfirm={clear}
      />
    </section>
  );
}

function Detail({ k, v, mono = false }) {
  return (
    <div className="detail-item">
      <span className="k">{k}</span>
      <span className={`v${mono ? ' mono' : ''}`}>{v}</span>
    </div>
  );
}
