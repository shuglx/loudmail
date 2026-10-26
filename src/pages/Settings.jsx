import { useEffect, useState } from 'react';
import { api } from '../api';
import Modal from '../components/Modal';
import Select from '../components/Select';
import Switch from '../components/Switch';
import { ExternalIcon } from '../icons';

const GITHUB_URL = 'https://github.com/shuglx/loudmail';

export default function Settings({ config, status, flash, onConfigChange }) {
  const [host, setHost] = useState(config?.mcp.host || '127.0.0.1');
  const [port, setPort] = useState(String(config?.mcp.port || 11328));
  const [autostart, setAutostart] = useState(Boolean(config?.behavior.autostart));
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState(null);
  const [searchOpen, setSearchOpen] = useState(false);
  const [keyword, setKeyword] = useState('');
  const [limit, setLimit] = useState(3);
  const [searching, setSearching] = useState(false);
  const [searchResult, setSearchResult] = useState(null);
  const [appInfo, setAppInfo] = useState(null);
  const [ttsStatus, setTtsStatus] = useState(null);
  const [checkingTts, setCheckingTts] = useState(false);
  const [ttsUrl, setTtsUrl] = useState(config?.tts.intranetUrl || '');
  const [ttsKey, setTtsKey] = useState(config?.tts.intranetKey || '');
  const [ttsInstr, setTtsInstr] = useState(config?.tts.intranetInstructions || '');

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

  const runTestSearch = async () => {
    const value = keyword.trim();
    if (!value || searching) return;
    setSearching(true);
    setSearchResult(null);
    try {
      setSearchResult(await api.testSearch(value, limit));
    } catch (e) {
      setSearchResult({ ok: false, ms: 0, request: '', response: '', error: String(e) });
    } finally {
      setSearching(false);
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

  const tts = config.tts || {
    enabled: false,
    provider: 'edge',
    edgeVoice: 'zh-CN-XiaoxiaoNeural',
    speed: 1,
    intranetUrl: '',
    intranetKey: '',
    intranetVoice: 'vivian',
    intranetInstructions: '播音腔，声音正式，精神饱满，情绪稳定',
  };

  const saveTts = (patch) => saveConfig({ tts: { ...tts, ...patch } });

  const switchTts = (provider) => {
    setTtsStatus(null);
    saveTts({ provider });
  };

  // 内网配置项同步进本地状态（保存发生在 onBlur）
  useEffect(() => {
    if (!config?.tts) return;
    setTtsUrl(config.tts.intranetUrl || '');
    setTtsKey(config.tts.intranetKey || '');
    setTtsInstr(config.tts.intranetInstructions || '');
  }, [config?.tts?.intranetUrl, config?.tts?.intranetKey, config?.tts?.intranetInstructions]);

  const checkTts = async () => {
    setCheckingTts(true);
    try {
      setTtsStatus(await api.ttsStatus());
    } catch (e) {
      flash(String(e));
    } finally {
      setCheckingTts(false);
    }
  };

  return (
    <section className="page">
      <div className="page-head">
        <h1>设置</h1>
      </div>

      <div className="settings-stack">
        
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
            <div className="setting-row">
              <div className="label">
                <b>界面主题</b>
                <small>暗色模式适合夜间值守</small>
              </div>
              <Select
                value={config.theme === 'dark' ? 'dark' : 'light'}
                options={[
                  { value: 'light', label: '浅色' },
                  { value: 'dark', label: '深色' },
                ]}
                onChange={(value) => saveConfig({ theme: value })}
              />
            </div>
          </section>
        </div>

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
                <button
                  className="btn btn-sm"
                  onClick={() => setSearchOpen(true)}
                  disabled={!testResult?.overall}
                  title={
                    testResult?.overall ? '用 MCP 接口查一次邮件' : '请先测试连接，通过后才能测试接口'
                  }
                >
                  测试接口
                </button>
              </div>
            </div>
            {testResult ? <TestResult result={testResult} /> : null}
            <div className="setting-row">
              <div className="label">
                <b>扫描间隔</b>
                <small>每条启用策略的轮询周期</small>
              </div>
              <Select
                value={config.scan.intervalSec}
                options={[
                  { value: 60, label: '1 分钟' },
                  { value: 120, label: '2 分钟' },
                  { value: 180, label: '3 分钟' },
                ]}
                onChange={(value) => saveConfig({ scan: { ...config.scan, intervalSec: value } })}
              />
            </div>
            <div className="setting-row">
              <div className="label">
                <b>检索时间窗</b>
                <small>每次轮询检索最近多久的邮件</small>
              </div>
              <Select
                value={config.scan.lookbackMin}
                options={[
                  { value: 10, label: '10 分钟' },
                  { value: 30, label: '30 分钟' },
                  { value: 60, label: '60 分钟' },
                ]}
                onChange={(value) => saveConfig({ scan: { ...config.scan, lookbackMin: value } })}
              />
            </div>
            <div className="setting-row">
              <div className="label">
                <b>模拟邮箱 [仅限开发人员调试]</b>
                <small>不连接真实MCP而是模拟返回数据</small>
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
            <h2>语音合成（TTS）</h2>
          </div>
          <section className="settings-card">
            <div className="setting-row">
              <div className="label">
                <b>启用文本转语音</b>
                <small>用于生成报警语音，录音与文件导入不受影响</small>
              </div>
              <Switch
                checked={Boolean(tts.enabled)}
                onChange={(checked) => saveTts({ enabled: checked })}
              />
            </div>
            {tts.enabled ? (
              <>
                <div className="setting-row">
                  <div className="label">
                    <b>语音服务</b>
                    <small>联网服务使用微软语音，内网服务使用内部TTS接口</small>
                  </div>
                  <div className="seg-tabs tts-tabs">
                    <button
                      className={tts.provider === 'intranet' ? '' : 'active'}
                      onClick={() => switchTts('edge')}
                    >
                      联网服务
                    </button>
                    <button
                      className={tts.provider === 'intranet' ? 'active' : ''}
                      onClick={() => switchTts('intranet')}
                    >
                      内网服务
                    </button>
                  </div>
                </div>
                {tts.provider === 'intranet' ? (
                  <>
                    <div className="setting-row">
                      <div className="label">
                        <b>服务地址</b>
                        <small>完整 WebSocket 接口地址，以 ws:// 开头</small>
                      </div>
                      <input
                        className="tts-input"
                        type="text"
                        value={ttsUrl}
                        placeholder="ws://host:port/v1/audio/speech/stream"
                        onChange={(e) => setTtsUrl(e.target.value)}
                        onBlur={() => saveTts({ intranetUrl: ttsUrl.trim() })}
                      />
                    </div>
                    <div className="setting-row">
                      <div className="label">
                        <b>认证信息</b>
                        <small>可选，填写后以 Bearer Token 携带</small>
                      </div>
                      <input
                        className="tts-input"
                        type="text"
                        value={ttsKey}
                        placeholder="不填则不带鉴权"
                        onChange={(e) => setTtsKey(e.target.value)}
                        onBlur={() => saveTts({ intranetKey: ttsKey.trim() })}
                      />
                    </div>
                    <div className="setting-row">
                      <div className="label">
                        <b>音色选择</b>
                        <small>内网服务可用音色</small>
                      </div>
                      <Select
                        value={tts.intranetVoice || 'vivian'}
                        options={[
                          { value: 'vivian', label: '女声（vivian）' },
                          { value: 'ryan', label: '男声（ryan）' },
                        ]}
                        onChange={(value) => saveTts({ intranetVoice: value })}
                      />
                    </div>
                    <div className="setting-row">
                      <div className="label">
                        <b>语气约束词</b>
                        <small>留空则用服务端默认</small>
                      </div>
                      <input
                        className="tts-input"
                        type="text"
                        value={ttsInstr}
                        placeholder="播音腔，声音正式，精神饱满，情绪稳定"
                        onChange={(e) => setTtsInstr(e.target.value)}
                        onBlur={() => saveTts({ intranetInstructions: ttsInstr.trim() })}
                      />
                    </div>
                  </>
                ) : (
                  <div className="setting-row">
                    <div className="label">
                      <b>音色选择</b>
                      <small>选择配音性别</small>
                    </div>
                    <Select
                      value={tts.edgeVoice || 'zh-CN-XiaoxiaoNeural'}
                      options={[
                        { value: 'zh-CN-XiaoxiaoNeural', label: '女声（晓晓）' },
                        { value: 'zh-CN-YunyangNeural', label: '男声（云扬）' },
                      ]}
                      onChange={(value) => saveTts({ edgeVoice: value })}
                    />
                  </div>
                )}
                <div className="setting-row">
                  <div className="label">
                    <b>可用性</b>
                    <small>检查当前语音服务配置</small>
                  </div>
                  <div className="endpoint-actions">
                    {ttsStatus ? (
                      <span className={`conn-inline${ttsStatus.ready ? '' : ' off'}`}>
                        <i />
                        {ttsStatus.ready ? '就绪' : '不可用'}
                      </span>
                    ) : null}
                    <button className="btn btn-sm" onClick={checkTts} disabled={checkingTts}>
                      {checkingTts ? '检测中…' : '检测'}
                    </button>
                  </div>
                </div>
                {ttsStatus && !ttsStatus.ready ? (
                  <p className="form-error inline">{ttsStatus.message}</p>
                ) : null}
              </>
            ) : null}
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
                <div className="about-name-row">
                  <b>LoudMail</b>
                  <button
                    className="icon-btn about-github-btn"
                    title="打开 GitHub 仓库"
                    onClick={() => openPath(GITHUB_URL)}
                  >
                    <ExternalIcon size={12} />
                  </button>
                </div>
                <small>版本 {appInfo?.version || '—'}</small>
              </div>
              <button className="btn btn-danger" onClick={() => api.quitApp()}>
                退出应用
              </button>
            </div>
          </section>
        </div>
      </div>

      <Modal open={searchOpen} title="测试接口" wide onClose={() => setSearchOpen(false)}>
        <div className="field">
          <label>查找关键词</label>
          <div className="debug-query">
            <input
              type="text"
              value={keyword}
              placeholder="例如 SOC"
              onChange={(e) => setKeyword(e.target.value)}
            />
            <Select
              className="debug-limit"
              value={limit}
              options={[1, 3, 5, 10].map((n) => ({ value: n, label: `${n} 封` }))}
              onChange={setLimit}
            />
            <button
              className="btn"
              onClick={runTestSearch}
              disabled={searching || !keyword.trim()}
            >
              {searching ? '测试中…' : '发起测试'}
            </button>
          </div>
        </div>
        <div className="debug-block">
          <span className="debug-label">发送报文</span>
          <textarea
            className="debug-area"
            readOnly
            spellCheck={false}
            value={searchResult?.request || ''}
            placeholder="点击「发起测试」后显示"
          />
        </div>
        <div className="debug-block">
          <span className="debug-label">返回报文</span>
          <textarea
            className="debug-area"
            readOnly
            spellCheck={false}
            value={searchResult?.response || ''}
            placeholder="点击「发起测试」后显示"
          />
        </div>
        {searchResult && !searchResult.ok ? (
          <div className="test-banner fail">{`FAIL · ${searchResult.error || '查询失败'}`}</div>
        ) : null}
        {searchResult && searchResult.ok ? (
          <p className="hint">查询完成，用时 {searchResult.ms} ms</p>
        ) : null}
      </Modal>
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
              (server
                ? [server.name, server.version].filter(Boolean).join(' ')
                : '返回 serverInfo')}
          </small>
        </span>
        <span className="step-ms">
          {result.handshake.ok ? `${result.handshake.ms} ms` : '—'}
        </span>
      </div>
      <div className={`test-banner${result.overall ? '' : ' fail'}`}>
        {result.overall
          ? `PASS · 握手返回 ${[server?.name || 'CoremailAir', server?.version]
              .filter(Boolean)
              .join(' ')}`
          : `FAIL · ${result.handshake.error || result.tcp.error || '连接失败'}`}
      </div>
    </div>
  );
}
