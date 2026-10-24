const props = {
  viewBox: '0 0 24 24',
  fill: 'none',
  stroke: 'currentColor',
  strokeWidth: 1.6,
  strokeLinecap: 'round',
  strokeLinejoin: 'round',
};

function Svg({ size = 16, children, ...rest }) {
  return (
    <svg width={size} height={size} {...props} {...rest} aria-hidden="true">
      {children}
    </svg>
  );
}

export const ShieldIcon = ({ size }) => (
  <Svg size={size}>
    <path d="M12 3l7 3v5c0 4.6-3 7.6-7 9-4-1.4-7-4.4-7-9V6l7-3z" />
    <path d="M9 12l2 2 4-4" />
  </Svg>
);

export const ClockIcon = ({ size }) => (
  <Svg size={size}>
    <circle cx="12" cy="12" r="8.5" />
    <path d="M12 7.5V12l3 2" />
  </Svg>
);

export const SpeakerIcon = ({ size }) => (
  <Svg size={size}>
    <path d="M4 9.5h3.5L12 5.5v13l-4.5-4H4z" />
    <path d="M16 9a4 4 0 010 6" />
    <path d="M18.5 6.5a7.5 7.5 0 010 11" />
  </Svg>
);

export const SlidersIcon = ({ size }) => (
  <Svg size={size}>
    <path d="M4 7h9M17 7h3M4 17h3M11 17h9" />
    <circle cx="15" cy="7" r="2" />
    <circle cx="9" cy="17" r="2" />
  </Svg>
);

export const SearchIcon = ({ size }) => (
  <Svg size={size}>
    <circle cx="11" cy="11" r="6.5" />
    <path d="M16 16l4 4" />
  </Svg>
);

export const InfoIcon = ({ size }) => (
  <Svg size={size}>
    <circle cx="12" cy="12" r="8.5" />
    <path d="M12 11v5" />
    <path d="M12 8h.01" />
  </Svg>
);

export const QuestionIcon = ({ size = 15 }) => (
  <Svg size={size}>
    <circle cx="12" cy="12" r="8.5" />
    <path d="M9.6 9.5a2.5 2.5 0 114.1 2.3c-.8.6-1.7 1.1-1.7 2.3" />
    <path d="M12 17h.01" />
  </Svg>
);

export const GripIcon = ({ size = 14 }) => (
  <svg width={size} height={size} viewBox="0 0 16 16" fill="currentColor" aria-hidden="true">
    <circle cx="5" cy="4" r="1.2" />
    <circle cx="11" cy="4" r="1.2" />
    <circle cx="5" cy="8" r="1.2" />
    <circle cx="11" cy="8" r="1.2" />
    <circle cx="5" cy="12" r="1.2" />
    <circle cx="11" cy="12" r="1.2" />
  </svg>
);

export const PlayIcon = ({ size = 12 }) => (
  <svg width={size} height={size} viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
    <path d="M8 5l11 7-11 7V5z" />
  </svg>
);

export const StopIcon = ({ size = 11 }) => (
  <svg width={size} height={size} viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
    <rect x="6" y="5" width="4" height="14" rx="1" />
    <rect x="14" y="5" width="4" height="14" rx="1" />
  </svg>
);

export const UploadIcon = ({ size }) => (
  <Svg size={size}>
    <path d="M12 16V5" />
    <path d="M7.5 9.5L12 5l4.5 4.5" />
    <path d="M5 16v2.5h14V16" />
  </Svg>
);

/// 导出：文档 + 箭头向右上送出
export const ExportIcon = ({ size }) => (
  <Svg size={size}>
    <path d="M13.5 4H6.5A2.5 2.5 0 004 6.5v11A2.5 2.5 0 006.5 20h11a2.5 2.5 0 002.5-2.5V11" />
    <path d="M21 3l-8.5 8.5" />
    <path d="M15.4 3H21v5.6" />
  </Svg>
);

/// 导入：文档 + 箭头从右上落入文档内
export const ImportIcon = ({ size }) => (
  <Svg size={size}>
    <path d="M13.5 4H6.5A2.5 2.5 0 004 6.5v11A2.5 2.5 0 006.5 20h11a2.5 2.5 0 002.5-2.5V11" />
    <path d="M21 3l-8.5 8.5" />
    <path d="M17.9 11.5h-5.4V6.1" />
  </Svg>
);

export const MicIcon = ({ size }) => (
  <Svg size={size}>
    <rect x="9" y="3" width="6" height="11" rx="3" />
    <path d="M5.5 11a6.5 6.5 0 0013 0" />
    <path d="M12 17.5V21" />
    <path d="M9 21h6" />
  </Svg>
);

export const ExternalIcon = ({ size }) => (
  <Svg size={size}>
    <path d="M14 4h6v6" />
    <path d="M20 4l-8 8" />
    <path d="M18 14v5H5V6h5" />
  </Svg>
);

export const XIcon = ({ size = 14 }) => (
  <Svg size={size}>
    <path d="M6 6l12 12M18 6L6 18" />
  </Svg>
);

export const CheckIcon = ({ size = 12 }) => (
  <Svg size={size}>
    <path d="M5 12l4 4L19 6" />
  </Svg>
);

export const SunIcon = ({ size = 15 }) => (
  <Svg size={size}>
    <circle cx="12" cy="12" r="4.2" />
    <path d="M12 3v2M12 19v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M3 12h2M19 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4" />
  </Svg>
);

export const MoonIcon = ({ size = 15 }) => (
  <Svg size={size}>
    <path d="M20 14.3A8.2 8.2 0 019.7 4a8.4 8.4 0 100 16.8 8.4 8.4 0 0010.3-6.5z" />
  </Svg>
);

export const TrashIcon = ({ size = 13 }) => (
  <Svg size={size}>
    <path d="M4 7h16" />
    <path d="M9.5 7V5.2A1.2 1.2 0 0110.7 4h2.6a1.2 1.2 0 011.2 1.2V7" />
    <path d="M6 7l.9 11.2A1.8 1.8 0 008.7 20h6.6a1.8 1.8 0 001.8-1.8L18 7" />
    <path d="M10.5 11v5M13.5 11v5" />
  </Svg>
);

export const ChevronRightIcon = ({ size = 15 }) => (
  <Svg size={size}>
    <path d="M9 5l7 7-7 7" />
  </Svg>
);
