import ReactDOM from 'react-dom/client';
import App from './App';
import { api } from './api';
import './app.css';

window.addEventListener('error', (event) => {
  api.log('error', `前端错误: ${event.message}`).catch(() => {});
});
window.addEventListener('unhandledrejection', (event) => {
  api.log('error', `前端未处理异常: ${String(event.reason)}`).catch(() => {});
});

// 渲染前先按上次使用的主题设置根节点，避免深浅色切换时白闪
try {
  const saved = localStorage.getItem('loudmail-theme');
  document.documentElement.dataset.theme = saved === 'dark' ? 'dark' : 'light';
} catch {
  // ignore
}

ReactDOM.createRoot(document.getElementById('root')).render(<App />);
