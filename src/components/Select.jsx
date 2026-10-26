import { useEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { ChevronDownIcon } from '../icons';

/** 菜单项高度，需与 app.css 里的 .select-item 保持一致 */
const ITEM_HEIGHT = 26;
const MENU_PADDING = 8;
const MENU_MAX_HEIGHT = 260;

/**
 * 自绘下拉。
 *
 * 不用原生 <select>：麒麟上的 WebKitGTK 由 GTK 绘制弹出列表，样式不受 CSS 控制，
 * 而且弹层会跑到应用窗口外面去（定位按屏幕而不是按控件算）。这里用一个按钮 +
 * position: fixed 的菜单自己实现，位置由 getBoundingClientRect 现算。
 */
export default function Select({
  value,
  options,
  onChange,
  className = '',
  searchable = false,
}) {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState('');
  const inputRef = useRef(null);
  const [pos, setPos] = useState(null);
  const btnRef = useRef(null);
  const menuRef = useRef(null);
  // 配置里的数值有时是字符串，按字符串比较避免选不中
  const same = (a, b) => String(a) === String(b);
  const current = options.find((option) => same(option.value, value));
  const keyword = query.trim().toLowerCase();
  const shown =
    searchable && keyword
      ? options.filter((option) => String(option.label).toLowerCase().includes(keyword))
      : options;

  useEffect(() => {
    if (!open) return undefined;
    const onDown = (event) => {
      if (btnRef.current?.contains(event.target)) return;
      if (menuRef.current?.contains(event.target)) return;
      setOpen(false);
    };
    // 光标在菜单里时：只滚菜单，且阻止冒泡到外层弹框（否则会把背后的编辑弹框滚走）。
    // 光标在菜单外时才关闭菜单，避免菜单和按钮脱开。
    // 只监听 wheel，不用 scroll / resize：窗口 resize 事件在 macOS 上可能高频连发。
    const onWheel = (event) => {
      const el = menuRef.current;
      if (el && el.contains(event.target)) {
        event.preventDefault();
        el.scrollTop += event.deltaY;
        return;
      }
      setOpen(false);
    };
    const onKey = (event) => {
      if (event.key === 'Escape') {
        // 拦住事件，避免菜单一关把外层弹窗也一起关掉
        event.stopImmediatePropagation();
        setOpen(false);
      }
    };
    window.addEventListener('mousedown', onDown);
    window.addEventListener('wheel', onWheel, { capture: true, passive: false });
    window.addEventListener('keydown', onKey, true);
    return () => {
      window.removeEventListener('mousedown', onDown);
      window.removeEventListener('wheel', onWheel, { capture: true });
      window.removeEventListener('keydown', onKey, true);
    };
  }, [open]);

  useEffect(() => {
    if (open && searchable) inputRef.current?.focus();
  }, [open, searchable]);

  const toggle = () => {
    if (open) {
      setQuery('');
      setOpen(false);
      return;
    }
    const rect = btnRef.current.getBoundingClientRect();
    const width = Math.max(rect.width, 150);
    const height = Math.min(options.length * ITEM_HEIGHT + MENU_PADDING, MENU_MAX_HEIGHT);
    // 下方放不下就往上弹：窗口边就是 webview 边，超出去会被裁掉
    let top = rect.bottom + 6;
    if (top + height > window.innerHeight - 8) {
      top = Math.max(8, rect.top - 6 - height);
    }
    let left = rect.left;
    if (left + width > window.innerWidth - 8) {
      left = Math.max(8, window.innerWidth - width - 8);
    }
    setPos({ top, left, width });
    setOpen(true);
  };

  return (
    <>
      <button
        type="button"
        ref={btnRef}
        className={`select${className ? ` ${className}` : ''}${open ? ' open' : ''}`}
        onClick={toggle}
      >
        <span className="select-value">{current ? current.label : ''}</span>
        <ChevronDownIcon size={12} />
      </button>
      {open && pos
        ? createPortal(
            <div
              ref={menuRef}
              className="select-menu"
              style={{ top: pos.top, left: pos.left, width: pos.width }}
            >
              {searchable ? (
                <input
                  ref={inputRef}
                  className="select-search"
                  type="text"
                  value={query}
                  placeholder="输入名称过滤"
                  spellCheck={false}
                  onChange={(event) => setQuery(event.target.value)}
                />
              ) : null}
              {shown.length === 0 ? (
                <div className="select-empty">无匹配项</div>
              ) : null}
              {shown.map((option) => (
                <button
                  key={String(option.value)}
                  type="button"
                  className={`select-item${same(option.value, value) ? ' on' : ''}`}
                  onClick={() => {
                    onChange(option.value);
                    setQuery('');
                    setOpen(false);
                  }}
                >
                  <span className="select-item-text">{option.label}</span>
                </button>
              ))}
            </div>,
            document.body
          )
        : null}
    </>
  );
}
