export default function Switch({
  checked,
  onChange,
  title,
  shape = 'pill',
  variant = 'default',
  disabled = false,
}) {
  const className = [
    'switch',
    shape === 'square' ? 'square' : '',
    variant === 'danger' ? 'danger' : '',
    disabled ? 'disabled' : '',
  ]
    .filter(Boolean)
    .join(' ');

  return (
    <label className={className} title={title}>
      <input
        type="checkbox"
        checked={checked}
        disabled={disabled}
        onChange={(event) => onChange(event.target.checked)}
      />
      <span />
    </label>
  );
}
