export default function Switch({ checked, onChange, title, shape = 'pill', variant = 'default' }) {
  const className = [
    'switch',
    shape === 'square' ? 'square' : '',
    variant === 'danger' ? 'danger' : '',
  ]
    .filter(Boolean)
    .join(' ');

  return (
    <label className={className} title={title}>
      <input
        type="checkbox"
        checked={checked}
        onChange={(event) => onChange(event.target.checked)}
      />
      <span />
    </label>
  );
}
