import { useEffect, useRef, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { appWindow } from '@tauri-apps/api/window';
import { api } from './api';
import Toast from './components/Toast';
import Modal from './components/Modal';
import History from './pages/History';
import Settings from './pages/Settings';
import Sounds from './pages/Sounds';
import Strategies from './pages/Strategies';
import {
  ClockIcon,
  QuestionIcon,
  ShieldIcon,
  SlidersIcon,
  SpeakerIcon,
} from './icons';

const PAGES = ['strategies', 'history', 'sounds', 'settings'];

const NAV = [
  { key: 'strategies', label: '匹配策略', icon: ShieldIcon },
  { key: 'history', label: '播报记录', icon: ClockIcon },
  { key: 'sounds', label: '语音库', icon: SpeakerIcon },
  { key: 'settings', label: '设置', icon: SlidersIcon },
];

const HELP_ROWS = [
  ['匹配策略', '按发件人邮箱 + 标题关键词过滤邮件，命中后播放指定语音；拖动列表可调整匹配优先级，同一封邮件只播报一次。'],
  ['播报记录', '记录每次命中的邮件与播放的语音，可按发件人 / 标题搜索，也可一键清空。'],
  ['语音库', '内置铃声、上传音频、麦克风录音、文本转语音四种来源；支持把文本与铃声合成为一段报警语音。'],
  ['设置', 'MCP 端点、扫描间隔与检索时间窗、TTS 语音来源、开机自启动等参数调整。'],
  ['后台托盘', '关闭窗口即隐藏到托盘并继续监听；托盘菜单可显示主窗口、暂停播报、退出应用。'],
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
  const [ready, setReady] = useState(false);
  const [appVersion, setAppVersion] = useState('');
  const [maximized, setMaximized] = useState(false);
  const [helpOpen, setHelpOpen] = useState(false);
  const toastTimer = useRef(null);

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

  useEffect(() => {
    let disposed = false;
    const syncMaximized = () => {
      api
        .isWindowMaximized()
        .then((value) => {
          if (!disposed) setMaximized(Boolean(value));
        })
        .catch(() => {});
    };
    syncMaximized();
    let unlisten = null;
    listen('tauri://resize', syncMaximized)
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

  const toggleMaximize = () => {
    api
      .toggleMaximizeWindow()
      .then((value) => setMaximized(Boolean(value)))
      .catch((e) => flash(String(e)));
  };

  // 标题栏自绘拖动：单击拖动窗口，双击最大化 / 还原
  const onTitlebarMouseDown = (event) => {
    if (event.button !== 0) return;
    event.preventDefault();
    if (event.detail === 2) {
      toggleMaximize();
      return;
    }
    if (event.detail === 1) appWindow.startDragging().catch(() => {});
  };

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
          : [record, ...prev].slice(0, 500)
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
        {/* 单击拖动、双击最大化 / 还原，状态由 resize 事件回填 */}
        <div className="titlebar-drag" onMouseDown={onTitlebarMouseDown}>
          <span className="titlebar-title">LoudMail</span>
          <span className="proto-badge">v{appVersion || '—'}</span>
        </div>
        <div className="titlebar-actions">
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

          <div className="sidebar-foot">
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
