export default function Switch({ checked, onChange, title }) {
  return (
    <label className="switch" title={title}>
      <input
        type="checkbox"
        checked={checked}
        onChange={(event) => onChange(event.target.checked)}
      />
      <span />
    </label>
  );
}
