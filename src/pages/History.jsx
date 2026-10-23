import { useMemo, useState } from 'react';
import { api } from '../api';
import ConfirmDialog from '../components/ConfirmDialog';
import Modal from '../components/Modal';
import { SearchIcon, ChevronRightIcon } from '../icons';

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

export default function History({ history, flash, onCleared }) {
  const [query, setQuery] = useState('');
  const [filter, setFilter] = useState('all');
  const [pageSize, setPageSize] = useState(10);
  const [page, setPage] = useState(1);
  const [selected, setSelected] = useState(null);
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
      await api.clearHistory();
      setPage(1);
      await onCleared();
      flash('播报记录已清空');
    } catch (e) {
      flash(String(e));
    }
  };

  const copyMailId = (mailId) => {
    const done = () => flash('已复制邮件 ID');
    if (navigator.clipboard?.writeText) {
      navigator.clipboard.writeText(mailId).then(done).catch(() => fallbackCopy(mailId, done));
    } else {
      fallbackCopy(mailId, done);
    }
  };

  return (
    <section className="page">
      <div className="page-head">
        <h1>播报记录</h1>
        <div className="head-actions">
          <button className="btn" onClick={() => setClearOpen(true)}>
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
        <select
          className="control"
          value={filter}
          onChange={(e) => setFilterAndReset(e.target.value)}
        >
          <option value="all">全部策略</option>
          {strategyNames.map((name) => (
            <option key={name} value={name}>
              {name}
            </option>
          ))}
        </select>
        <select
          className="control"
          value={pageSize}
          onChange={(e) => setPageSizeAndReset(e.target.value)}
        >
          <option value={10}>每页 10 条</option>
          <option value={25}>每页 25 条</option>
          <option value={50}>每页 50 条</option>
        </select>
        <span className="toolbar-note">滚动保留最新 500 条</span>
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
        {total === 0 ? (
          <div className="empty">
            <div className="empty-ico">◷</div>
            <b>{history.length ? '没有匹配的记录' : '还没有播报记录'}</b>
            <p>
              {history.length ? '试试更换搜索词或策略筛选。' : '策略命中新邮件后，播报记录会出现在这里。'}
            </p>
          </div>
        ) : (
          items.map((record) => (
            <button
              className="history-row"
              key={record.id}
              onClick={() => setSelected(record)}
            >
              <span className="history-time">{record.time.slice(5, 16)}</span>
              <span className="history-main">
                <b title={record.subject}>{record.subject}</b>
                <span>
                  {record.sender} · {record.strategyName}
                </span>
              </span>
              <span className="history-side">
                <span className="tag">
                  {record.soundName} ×{record.playCount}
                </span>
                <span className="chev">
                  <ChevronRightIcon />
                </span>
              </span>
            </button>
          ))
        )}
      </div>

      <Modal
        open={Boolean(selected)}
        title="播报详情"
        onClose={() => setSelected(null)}
        footer={
          <button className="btn" onClick={() => setSelected(null)}>
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
              <Detail k="邮件 ID" v={selected.mailId} mono />
            </div>
            <div className="mailid-box">
              <code>{selected.mailId}</code>
              <button className="btn btn-sm" onClick={() => copyMailId(selected.mailId)}>
                复制
              </button>
            </div>
          </>
        ) : null}
      </Modal>

      <ConfirmDialog
        open={clearOpen}
        title="清空全部播报记录？"
        description="清空后不影响策略游标，已播报的邮件不会被重复播报。"
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

function fallbackCopy(text, done) {
  const textarea = document.createElement('textarea');
  textarea.value = text;
  textarea.style.position = 'fixed';
  textarea.style.opacity = '0';
  document.body.appendChild(textarea);
  textarea.select();
  try {
    document.execCommand('copy');
    done();
  } catch {
    // ignore
  }
  document.body.removeChild(textarea);
}
