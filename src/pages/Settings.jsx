import { useEffect, useState } from 'react';
import { api } from '../api';
import Switch from '../components/Switch';
import { ExternalIcon } from '../icons';

export default function Settings({ config, status, flash, onConfigChange }) {
  const [host, setHost] = useState(config?.mcp.host || '127.0.0.1');
  const [port, setPort] = useState(String(config?.mcp.port || 11328));
  const [autostart, setAutostart] = useState(Boolean(config?.behavior.autostart));
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState(null);
  const [appInfo, setAppInfo] = useState(null);

  useEffect(() => {
    if (!config) return;
    setHost(config.mcp.host);
    setPort(String(config.mcp.port));
    setAutostart(Boolean(config.behavior.autostart));
  }, [config]);

  useEffect(() => {
    api.getAppInfo().then(setAppInfo).catch(() => {});
    api
      .getAutostart()
      .then(setAutostart)
      .catch(() => {});
  }, []);

  if (!config) {
    return (
      <section className="page">
        <div className="page-head">
          <h1>设置</h1>
        </div>
        <p className="muted">正在读取设置…</p>
      </section>
    );
  }

  const saveConfig = async (patch) => {
    const next = { ...config, ...patch };
    try {
      const saved = await api.setConfig(next);
      onConfigChange(saved);
      flash('设置已保存');
    } catch (e) {
      flash(String(e));
    }
  };

  const saveEndpoint = () => {
    const value = Number(port);
    if (!Number.isInteger(value) || value < 1 || value > 65535) {
      setPort(String(config.mcp.port));
      flash('端口需为 1~65535 的整数');
      return;
    }
    if (host.trim() !== config.mcp.host || value !== config.mcp.port) {
      saveConfig({ mcp: { host: host.trim() || '127.0.0.1', port: value } });
    }
  };

  const testConnection = async () => {
    setTesting(true);
    setTestResult(null);
    try {
      const result = await api.testConnection(host.trim() || '127.0.0.1', Number(port) || 11328);
      setTestResult(result);
    } catch (e) {
      flash(String(e));
    } finally {
      setTesting(false);
    }
  };

  const toggleAutostart = async (enabled) => {
    try {
      const actual = await api.setAutostart(enabled);
      setAutostart(actual);
      onConfigChange({ ...config, behavior: { ...config.behavior, autostart: actual } });
      flash(actual ? '已开启开机自启动' : '已关闭开机自启动');
    } catch (e) {
      flash(String(e));
    }
  };

  const openPath = (path) => api.openPath(path).catch((e) => flash(String(e)));

  return (
    <section className="page">
      <div className="page-head">
        <h1>设置</h1>
      </div>

      <div className="settings-stack">
        <div className="settings-group">
          <div className="group-head">
            <h2>邮箱</h2>
          </div>
          <section className="settings-card">
            <div className="setting-row">
              <div className="label">
                <b>MCP 端点</b>
                <small>修改后下一轮扫描生效</small>
              </div>
              <div className="endpoint">
                <input
                  id="cfgHost"
                  type="text"
                  value={host}
                  spellCheck={false}
                  onChange={(e) => setHost(e.target.value)}
                  onBlur={saveEndpoint}
                />
                <span>:</span>
                <input
                  id="cfgPort"
                  type="text"
                  value={port}
                  spellCheck={false}
                  onChange={(e) => setPort(e.target.value)}
                  onBlur={saveEndpoint}
                />
              </div>
            </div>
            <div className="setting-row">
              <div className="label">
                <b>连接状态</b>
                <small>仅展示 MCP 是否可达</small>
              </div>
              <div className="endpoint-actions">
                <span className={`conn-inline${status.connected ? '' : ' off'}`}>
                  <i />
                  {status.connected ? '已连接' : '未连接'}
                </span>
                <button className="btn btn-sm" onClick={testConnection} disabled={testing}>
                  {testing ? '检测中…' : '测试连接'}
                </button>
              </div>
            </div>
            {testResult ? <TestResult result={testResult} /> : null}
            <div className="setting-row">
              <div className="label">
                <b>扫描间隔</b>
                <small>每条启用策略的轮询周期</small>
              </div>
              <select
                className="control"
                value={config.scan.intervalSec}
                onChange={(e) => saveConfig({ scan: { ...config.scan, intervalSec: Number(e.target.value) } })}
              >
                <option value={60}>1 分钟</option>
                <option value={120}>2 分钟</option>
                <option value={180}>3 分钟</option>
              </select>
            </div>
            <div className="setting-row">
              <div className="label">
                <b>检索时间窗</b>
                <small>每次轮询检索最近多久的邮件</small>
              </div>
              <select
                className="control"
                value={config.scan.lookbackMin}
                onChange={(e) => saveConfig({ scan: { ...config.scan, lookbackMin: Number(e.target.value) } })}
              >
                <option value={10}>10 分钟</option>
                <option value={30}>30 分钟</option>
                <option value={60}>60 分钟</option>
              </select>
            </div>
            <div className="setting-row">
              <div className="label">
                <b>模拟数据</b>
                <small>开发用：不访问真实端口，使用内置样例邮件</small>
              </div>
              <Switch
                checked={config.behavior.mock}
                onChange={(checked) =>
                  saveConfig({ behavior: { ...config.behavior, mock: checked } })
                }
              />
            </div>
          </section>
        </div>

        <div className="settings-group">
          <div className="group-head">
            <h2>系统</h2>
          </div>
          <section className="settings-card">
            <div className="setting-row">
              <div className="label">
                <b>开机自启动</b>
                <small>登录后自动运行</small>
              </div>
              <Switch checked={autostart} onChange={toggleAutostart} />
            </div>
          </section>
        </div>

        <div className="settings-group">
          <div className="group-head">
            <h2>存储</h2>
          </div>
          <section className="settings-card">
            {(appInfo?.storageFiles || []).map((file) => (
              <div className="storage-row" key={file.path}>
                <div className="path">
                  <b>{file.label}</b>
                  <code>{file.path}</code>
                </div>
                <button className="btn btn-sm" onClick={() => openPath(file.path)}>
                  <ExternalIcon size={12} />
                  打开
                </button>
              </div>
            ))}
          </section>
        </div>

        <div className="settings-group">
          <div className="group-head">
            <h2>关于</h2>
          </div>
          <section className="settings-card">
            <div className="about-row">
              <img src="/logo.png" alt="LoudMail" />
              <div>
                <b>LoudMail</b>
                <small>
                  版本 {appInfo?.version || '0.1.0'}
                </small>
              </div>
              <button className="btn btn-danger" onClick={() => api.quitApp()}>
                退出应用
              </button>
            </div>
          </section>
        </div>
      </div>
    </section>
  );
}

function TestResult({ result }) {
  const server = result.handshake?.serverInfo;
  return (
    <div className="test-result">
      <div className="test-step">
        <span className={`step-dot ${result.tcp.ok ? 'ok' : 'fail'}`}>
          {result.tcp.ok ? '✓' : '✕'}
        </span>
        <span className="step-main">
          <b>TCP 探测</b>
          <small>{result.tcp.error || '端口已监听'}</small>
        </span>
        <span className="step-ms">{result.tcp.ok ? `${result.tcp.ms} ms` : '—'}</span>
      </div>
      <div className="test-step">
        <span className={`step-dot ${result.handshake.ok ? 'ok' : 'fail'}`}>
          {result.handshake.ok ? '✓' : '✕'}
        </span>
        <span className="step-main">
          <b>SSE 握手 + initialize</b>
          <small>
            {result.handshake.error ||
              (server ? `${server.name} ${server.version}` : '返回 serverInfo')}
          </small>
        </span>
        <span className="step-ms">
          {result.handshake.ok ? `${result.handshake.ms} ms` : '—'}
        </span>
      </div>
      <div className={`test-banner${result.overall ? '' : ' fail'}`}>
        {result.overall
          ? `PASS · 握手返回 ${server?.name || 'CoremailAir'} ${server?.version || ''}`
          : `FAIL · ${result.handshake.error || result.tcp.error || '连接失败'}`}
      </div>
    </div>
  );
}
