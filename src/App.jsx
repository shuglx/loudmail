import { useEffect, useRef, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { api } from './api';
import watermarkUrl from '../icon-shadow.png';
import Switch from './components/Switch';
import Toast from './components/Toast';
import Modal from './components/Modal';
import History from './pages/History';
import Settings from './pages/Settings';
import Sounds from './pages/Sounds';
import Strategies from './pages/Strategies';
import {
  ClockIcon,
  MoonIcon,
  QuestionIcon,
  ShieldIcon,
  SlidersIcon,
  SpeakerIcon,
  SunIcon,
} from './icons';

const PAGES = ['strategies', 'history', 'sounds', 'settings'];

/// 播报记录滚动上限；待确认（开启人工确认且未确认）的记录不受该上限约束
const HISTORY_LIMIT = 1000;

/// 超出上限时只从尾部淘汰「无需确认 / 已确认」的记录，待确认的留到人工确认
function trimHistory(list) {
  if (list.length <= HISTORY_LIMIT) return list;
  const out = list.slice();
  let index = out.length;
  while (out.length > HISTORY_LIMIT && index > 0) {
    index -= 1;
    const item = out[index];
    if (!item.requireConfirm || item.confirmed) out.splice(index, 1);
  }
  return out;
}

const NAV = [
  { key: 'history', label: '播报记录', icon: ClockIcon },
  { key: 'strategies', label: '匹配策略', icon: ShieldIcon },
  { key: 'sounds', label: '语音库', icon: SpeakerIcon },
  { key: 'settings', label: '设置', icon: SlidersIcon },
];

const HELP_ROWS = [
  ['播报记录', '记录每次命中的邮件与播放的语音，可按发件人 / 标题搜索、确认收到未确认的告警。'],
  ['匹配策略', '按发件人邮箱 + 标题关键词过滤邮件，命中后播放指定语音；拖动列表可调整匹配优先级，同一封邮件只命中一次。'],
  ['语音库', '内置铃声、上传音频、麦克风录音、文本转语音四种来源；支持把文本与铃声合成为一段报警语音。'],
  ['设置', 'MCP 端点、扫描间隔与检索时间窗、TTS 语音来源、开机自启动等参数调整。'],
  ['后台托盘', '关闭窗口即隐藏到托盘并继续监听；托盘菜单可显示主窗口、退出应用。'],
];

const dotProps = {
  viewBox: '0 0 8 8',
  fill: 'none',
  stroke: 'currentColor',
  strokeWidth: 1.1,
  strokeLinecap: 'round',
  strokeLinejoin: 'round',
  'aria-hidden': 'true',
};

const CloseGlyph = () => (
  <svg width="7" height="7" {...dotProps}>
    <path d="M1.5 1.5l5 5M6.5 1.5l-5 5" />
  </svg>
);

const MinGlyph = () => (
  <svg width="7" height="7" {...dotProps}>
    <path d="M1.6 4h4.8" />
  </svg>
);

const ExpandGlyph = () => (
  <svg width="7" height="7" {...dotProps}>
    <path d="M1.6 3V1.6H3M5 1.6h1.4V3M6.4 5v1.4H5M3 6.4H1.6V5" />
  </svg>
);

const RestoreGlyph = () => (
  <svg width="7" height="7" {...dotProps}>
    <path d="M3 1.6V3H1.6M6.4 3H5V1.6M5 6.4V5h1.4M1.6 5H3v1.4" />
  </svg>
);

export default function App() {
  const [page, setPage] = useState(() => {
    const hash = window.location.hash.slice(1);
    return PAGES.includes(hash) ? hash : 'strategies';
  });
  const [config, setConfig] = useState(null);
  const [strategies, setStrategies] = useState([]);
  const [history, setHistory] = useState([]);
  const [sounds, setSounds] = useState([]);
  const [status, setStatus] = useState({
    connected: false,
    serverName: null,
    serverVersion: null,
    lastError: null,
    mock: false,
  });
  const [toast, setToast] = useState('');
  // 全局播报开关（替代原来的托盘「暂停播报」）
  const [paused, setPaused] = useState(false);
  const [ready, setReady] = useState(false);
  const [appVersion, setAppVersion] = useState('');
  const [maximized, setMaximized] = useState(false);
  const [helpOpen, setHelpOpen] = useState(false);
  const [playback, setPlayback] = useState({
    playing: false,
    pending: 0,
    previewing: false,
  });
  const toastTimer = useRef(null);

  useEffect(() => {
    api
      .getBroadcastPaused()
      .then((value) => setPaused(Boolean(value)))
      .catch(() => {});
  }, []);

  const togglePaused = (next) => {
    setPaused(next);
    api
      .setBroadcastPaused(next)
      .then((value) => {
        setPaused(Boolean(value));
        flash(value ? '已暂停全部播报' : '已恢复播报');
      })
      .catch((e) => {
        setPaused(!next);
        flash(String(e));
      });
  };

  const flash = (message) => {
    setToast(message);
    if (toastTimer.current) clearTimeout(toastTimer.current);
    toastTimer.current = setTimeout(() => setToast(''), 2400);
  };

  const loadStrategies = async () => setStrategies(await api.listStrategies());
  const loadHistory = async () => setHistory(await api.listHistory());
  const refreshAfterSound = async () => {
    const [nextSounds, nextStrategies] = await Promise.all([
      api.listSounds(),
      api.listStrategies(),
    ]);
    setSounds(nextSounds);
    setStrategies(nextStrategies);
  };

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const [nextConfig, nextStrategies, nextHistory, nextSounds, nextStatus] =
          await Promise.all([
            api.getConfig(),
            api.listStrategies(),
            api.listHistory(),
            api.listSounds(),
            api.getRuntimeStatus(),
          ]);
        if (cancelled) return;
        setConfig(nextConfig);
        setStrategies(nextStrategies);
        setHistory(nextHistory);
        setSounds(nextSounds);
        setStatus(nextStatus);
        setReady(true);
        setTimeout(() => {
          const heading = document.querySelector('.page-head h1')?.textContent || '未知页面';
          api.log('info', `前端已加载，当前页面: ${heading}`).catch(() => {});
        }, 300);
      } catch (e) {
        flash(String(e));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    // 版本号来自 tauri.conf.json 的 package.version
    api.getAppInfo().then((info) => setAppVersion(info.version || '')).catch(() => {});
  }, []);

  // 主题：dark / light，跟随配置切换并缓存到 localStorage 避免启动白闪
  useEffect(() => {
    const theme = config?.theme === 'dark' ? 'dark' : 'light';
    document.documentElement.dataset.theme = theme;
    try {
      localStorage.setItem('loudmail-theme', theme);
    } catch {
      // ignore
    }
  }, [config?.theme]);

  // 最大化状态只需要在启动时读一次：之后只有绿点按钮能改变它（双击最大化已禁用），
  // 按钮会直接返回新状态，不需要任何监听。
  useEffect(() => {
    api
      .isWindowMaximized()
      .then((value) => setMaximized(Boolean(value)))
      .catch(() => {});
  }, []);

  // 双击标题栏：macOS 禁用（内置脚本走 mouseup，容易和拖拽会话打架），
  // Windows / Linux 交给「我们自己的」最大化命令——内置脚本默认走 appWindow.maximize()，
  // 会绕过我们的日志、图标状态和 Linux 的铺满逻辑，导致双击和绿点行为不一致。
  useEffect(() => {
    const isMac = /Mac/i.test(navigator.userAgent);
    const block = (event) => {
      if (event.button !== 0 || event.detail !== 2) return;
      const target = event.target;
      if (!target || !target.closest || !target.closest('[data-tauri-drag-region]')) return;
      // 打断内置脚本（macOS 在 mouseup、Windows/Linux 在 mousedown 触发）
      event.stopPropagation();
      if (!isMac && event.type === 'mousedown') toggleMaximizeRef.current();
    };
    window.addEventListener('mousedown', block, true);
    window.addEventListener('mouseup', block, true);
    return () => {
      window.removeEventListener('mousedown', block, true);
      window.removeEventListener('mouseup', block, true);
    };
  }, []);

  useEffect(() => {
    let disposed = false;
    api
      .getPlaybackStatus()
      .then((value) => {
        if (!disposed) setPlayback(value);
      })
      .catch(() => {});
    let unlisten = null;
    listen('player://status', (event) => setPlayback(event.payload))
      .then((fn) => {
        if (disposed) fn();
        else unlisten = fn;
      })
      .catch(() => {});
    return () => {
      disposed = true;
      if (unlisten) unlisten();
    };
  }, []);

  const isDark = config?.theme === 'dark';

  const toggleTheme = () => {
    if (!config) return;
    const next = { ...config, theme: isDark ? 'light' : 'dark' };
    api
      .setConfig(next)
      .then(setConfig)
      .catch((e) => flash(String(e)));
  };

  const runTestPlayback = () => {
    api
      .testPlayback()
      .then((count) => flash(`已插入 ${count} 条测试播报`))
      .catch((e) => flash(String(e)));
  };

  const toggleMaximizeRef = useRef(() => {});
  const toggleMaximize = () => {
    api
      .toggleMaximizeWindow()
      .then((value) => {
        setMaximized(Boolean(value));
      })
      .catch((e) => flash(String(e)));
  };
  // 双击标题栏的处理函数早于 toggleMaximize 定义，用 ref 转发当前实现
  toggleMaximizeRef.current = toggleMaximize;

  useEffect(() => {
    let disposed = false;
    const unlisteners = [];
    listen('runtime://status', (event) => setStatus(event.payload)).then((unlisten) => {
      if (disposed) unlisten();
      else unlisteners.push(unlisten);
    });
    listen('history://new', (event) => {
      const record = event.payload;
      setHistory((prev) =>
        prev.some((item) => item.id === record.id)
          ? prev
          : trimHistory([record, ...prev])
      );
    }).then((unlisten) => {
      if (disposed) unlisten();
      else unlisteners.push(unlisten);
    });
    // 人工确认：原位替换
    listen('history://updated', (event) => {
      const record = event.payload;
      setHistory((prev) =>
        trimHistory(prev.map((item) => (item.id === record.id ? record : item)))
      );
    }).then((unlisten) => {
      if (disposed) unlisten();
      else unlisteners.push(unlisten);
    });
    // 未确认重播：这一批记录被移到列表最前（数组顺序即最终顺序）
    listen('history://rebroadcast', (event) => {
      const records = event.payload || [];
      if (!records.length) return;
      const moved = new Set(records.map((item) => item.id));
      setHistory((prev) =>
        trimHistory([...records, ...prev.filter((item) => !moved.has(item.id))])
      );
    }).then((unlisten) => {
      if (disposed) unlisten();
      else unlisteners.push(unlisten);
    });
    return () => {
      disposed = true;
      unlisteners.forEach((unlisten) => unlisten());
    };
  }, []);

  useEffect(() => {
    const onHash = () => {
      const hash = window.location.hash.slice(1);
      if (PAGES.includes(hash)) setPage(hash);
    };
    window.addEventListener('hashchange', onHash);
    return () => window.removeEventListener('hashchange', onHash);
  }, []);

  useEffect(
    () => () => {
      if (toastTimer.current) clearTimeout(toastTimer.current);
    },
    []
  );

  const navigate = (next) => {
    setPage(next);
    if (window.location.hash !== `#${next}`) window.location.hash = next;
  };

  return (
    <div className={`window${maximized ? ' maximized' : ''}`}>
      <header className="titlebar">
        <div className="traffic">
          <button className="dot close" title="隐藏到托盘" onClick={() => api.hideWindow()}>
            <CloseGlyph />
          </button>
          <button className="dot min" title="最小化" onClick={() => api.minimizeWindow()}>
            <MinGlyph />
          </button>
          <button
            className="dot max"
            title={maximized ? '还原' : '最大化'}
            onClick={toggleMaximize}
          >
            {maximized ? <RestoreGlyph /> : <ExpandGlyph />}
          </button>
        </div>
        {/* 拖动交给 tauri 内置拖拽区（与 kylinpaste 一致），双击最大化已禁用 */}
        <div className="titlebar-drag" data-tauri-drag-region>
          <span className="titlebar-title">LoudMail</span>
          <span className="proto-badge">v{appVersion || '—'}</span>
        </div>
        <div className="titlebar-actions">
          <button
            className="icon-btn"
            title={isDark ? '切换到浅色模式' : '切换到深色模式'}
            aria-label="切换主题"
            onClick={toggleTheme}
          >
            {isDark ? <SunIcon /> : <MoonIcon />}
          </button>
          <button
            className="icon-btn help-btn"
            title="使用说明"
            aria-label="使用说明"
            onClick={() => setHelpOpen(true)}
          >
            <QuestionIcon />
          </button>
        </div>
      </header>

      <div className="app">
        <aside className="sidebar">
          <div className="brand">
            <img src="/logo.png" alt="LoudMail" />
            <div>
              <b>LoudMail</b>
              <small>邮件到达播报</small>
            </div>
          </div>
          <nav className="nav">
            {NAV.map((item) => {
              const Icon = item.icon;
              return (
                <button
                  key={item.key}
                  className={page === item.key ? 'active' : ''}
                  onClick={() => navigate(item.key)}
                >
                  <span className="nav-ico">
                    <Icon />
                  </span>
                  <span>{item.label}</span>
                </button>
              );
            })}
          </nav>

          {/* 侧栏留白处的剪影水印，置于状态区之上 */}
          <div
            className="sidebar-mark"
            style={{ '--watermark': `url(${watermarkUrl})` }}
            aria-hidden="true"
          />

          <div className="sidebar-foot">
            <div
              className={`playback-status${playback.playing ? '' : ' idle'}`}
              role="status"
            >
              <span className="pb-ico">
                <SpeakerIcon size={13} />
              </span>
              <span className="pb-text">
                {playback.playing
                  ? `播报中 · 待播 ${playback.pending} 条`
                  : '目前无播报'}
              </span>
            </div>
            <div className={`broadcast-off${paused ? ' on' : ''}`}>
              <span>暂停全部播报</span>
              <Switch
                shape="square"
                variant="danger"
                checked={paused}
                onChange={togglePaused}
                title="停用后停止轮询匹配与播报"
              />
            </div>
            <div className="foot-row">
              <div className={`conn${status.connected ? '' : ' off'}`}>
                <span className="dot" />
                <div>
                  <b>{status.connected ? '已连接' : '未连接'}</b>
                  <small>
                    {status.connected
                      ? `${status.serverName || 'CoremailAir'} ${status.serverVersion || ''}`.trim()
                      : '邮箱 MCP 不可达'}
                  </small>
                </div>
              </div>
              {config?.behavior?.mock ? (
                <button
                  className="btn btn-sm test-play-btn"
                  title="向播报队列插入 3 条随机内置语音"
                  onClick={runTestPlayback}
                >
                  测试
                </button>
              ) : null}
            </div>
          </div>
        </aside>

        <main className="content">
          {!ready ? (
            <div className="loading">正在读取本地数据…</div>
          ) : page === 'strategies' ? (
            <Strategies
              strategies={strategies}
              sounds={sounds}
              flash={flash}
              onRefresh={loadStrategies}
            />
          ) : page === 'history' ? (
            <History history={history} flash={flash} onCleared={loadHistory} />
          ) : page === 'sounds' ? (
            <Sounds
              sounds={sounds}
              strategies={strategies}
              flash={flash}
              onRefresh={refreshAfterSound}
            />
          ) : (
            <Settings
              config={config}
              status={status}
              flash={flash}
              onConfigChange={setConfig}
            />
          )}
        </main>
      </div>

      <Modal
        open={helpOpen}
        title="使用说明"
        wide
        onClose={() => setHelpOpen(false)}
        footer={
          <button className="btn btn-primary" onClick={() => setHelpOpen(false)}>
            知道了
          </button>
        }
      >
        <p className="help-lead">
          轮询邮箱 MCP，邮件命中匹配策略时播放报警语音，确保重要邮件不漏听。
        </p>
        <table className="help-table">
          <thead>
            <tr>
              <th>模块</th>
              <th>功能说明</th>
            </tr>
          </thead>
          <tbody>
            {HELP_ROWS.map(([name, desc]) => (
              <tr key={name}>
                <td>{name}</td>
                <td>{desc}</td>
              </tr>
            ))}
          </tbody>
        </table>
        <p className="help-note">数据、语音与日志均保存在本地，不上传云端。</p>
      </Modal>

      <Toast message={toast} />
    </div>
  );
}
