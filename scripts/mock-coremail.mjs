// 本地 Coremail MCP 模拟服务，用于在没有 e企邮客户端的机器上联调。
// 用法：node scripts/mock-coremail.mjs
import http from 'node:http';

const port = Number(process.env.MOCK_MCP_PORT || 11328);
const sessions = new Map();
let seq = 22010;
let nextKind = 0;

const nowText = () => {
  const d = new Date();
  const p = (n) => String(n).padStart(2, '0');
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(
    d.getMinutes()
  )}:${p(d.getSeconds())}`;
};

function makeMail() {
  const kind = nextKind++ % 2;
  const id = seq++;
  if (kind === 0) {
    return {
      mail_id: `mock:${id}`,
      sender: '"aqyyptsoc" <aqyyptsoc@dc.icbc.com.cn>',
      subject: `【5级】SOC_安全告警_HIPS_10.9.${id % 200}.${id % 90}`,
      summary: `5级告警处置要求：15分钟内通知到应用维护部门。模拟时间 ${nowText()}`,
    };
  }
  return {
    mail_id: `mock:${id}`,
    sender: '"DBA Alert" <dba-alert@dc.icbc.com.cn>',
    subject: `DB_ALERT_连接数异常_节点_${id % 20}`,
    summary: `数据库连接数超过阈值。模拟时间 ${nowText()}`,
  };
}

const mails = [makeMail(), makeMail()];

setInterval(() => {
  mails.push(makeMail());
  if (mails.length > 300) mails.splice(0, mails.length - 300);
}, 5000);

const server = http.createServer((req, res) => {
  const url = new URL(req.url, `http://${req.headers.host || `127.0.0.1:${port}`}`);

  if (req.method === 'GET' && url.pathname === '/sse') {
    res.writeHead(200, {
      'Content-Type': 'text/event-stream',
      'Cache-Control': 'no-cache',
      Connection: 'keep-alive',
    });
    const sessionId = Math.random().toString(36).slice(2);
    sessions.set(sessionId, res);
    res.write(
      `event: endpoint\ndata: http://127.0.0.1:${port}/mcp?sessionId=${sessionId}\n\n`
    );
    req.on('close', () => sessions.delete(sessionId));
    return;
  }

  if (req.method === 'POST' && url.pathname === '/mcp') {
    let body = '';
    req.on('data', (chunk) => {
      body += chunk;
    });
    req.on('end', () => {
      let message;
      try {
        message = JSON.parse(body);
      } catch {
        res.writeHead(400).end('bad json');
        return;
      }
      const sse = sessions.get(url.searchParams.get('sessionId'));
      const send = (payload) => {
        if (sse && !sse.writableEnded) {
          sse.write(`data: ${JSON.stringify(payload)}\n\n`);
        }
      };
      res.writeHead(202, { 'Content-Length': 0 });
      res.end();

      if (message.method === 'initialize') {
        send({
          jsonrpc: '2.0',
          id: message.id,
          result: {
            protocolVersion: '2024-11-05',
            serverInfo: { name: 'CoremailAir', version: '4.2.1.1494' },
            capabilities: { tools: {} },
          },
        });
      } else if (message.method === 'notifications/initialized') {
        // no response
      } else if (message.method === 'tools/call') {
        const pattern = message.params?.arguments?.pattern || '';
        const matched = mails
          .filter((mail) => mail.subject.includes(pattern))
          .slice(-200);
        send({
          jsonrpc: '2.0',
          id: message.id,
          result: {
            content: matched.map((mail) => ({
              type: 'text',
              text: JSON.stringify(mail),
            })),
          },
        });
      }
    });
    return;
  }

  res.writeHead(404);
  res.end('not found');
});

server.listen(port, '127.0.0.1', () => {
  console.log(`[mock-coremail] SSE  http://127.0.0.1:${port}/sse`);
  console.log('[mock-coremail] 每 5 秒生成一封 SOC / DB 模拟邮件');
});
