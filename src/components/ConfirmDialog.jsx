import Modal from './Modal';

export default function ConfirmDialog({
  open,
  title,
  description,
  confirmText = '确定',
  danger = true,
  onCancel,
  onConfirm,
}) {
  return (
    <Modal
      open={open}
      title={title}
      onClose={onCancel}
      footer={
        <>
          <button className="btn" onClick={onCancel}>
            取消
          </button>
          <button className={`btn ${danger ? 'btn-danger' : 'btn-primary'}`} onClick={onConfirm}>
            {confirmText}
          </button>
        </>
      }
    >
      <p className="confirm-desc">{description}</p>
    </Modal>
  );
}
