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

ReactDOM.createRoot(document.getElementById('root')).render(<App />);
