import { useEffect, useRef, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { api } from './api';
import Toast from './components/Toast';
import History from './pages/History';
import Settings from './pages/Settings';
import Sounds from './pages/Sounds';
import Strategies from './pages/Strategies';
import { ClockIcon, ShieldIcon, SlidersIcon, SpeakerIcon } from './icons';

const PAGES = ['strategies', 'history', 'sounds', 'settings'];

const NAV = [
  { key: 'strategies', label: '检查策略', icon: ShieldIcon },
  { key: 'history', label: '播报记录', icon: ClockIcon },
  { key: 'sounds', label: '语音库', icon: SpeakerIcon },
  { key: 'settings', label: '设置', icon: SlidersIcon },
];

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
    <div className="window">
      <header className="titlebar">
        <div className="traffic">
          <button className="dot close" title="隐藏到托盘" onClick={() => api.hideWindow()} />
          <button className="dot min" title="最小化" onClick={() => api.minimizeWindow()} />
          <span className="dot max" />
        </div>
        <div className="titlebar-drag" data-tauri-drag-region>
          <span className="titlebar-title">LoudMail</span>
          <span className="proto-badge">v0.1</span>
        </div>
        <div className="titlebar-right">邮件到达播报</div>
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
                    : status.lastError || '等待客户端启动'}
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

      <Toast message={toast} />
    </div>
  );
}
