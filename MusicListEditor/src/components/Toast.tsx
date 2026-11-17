import { motion } from 'motion/react';

export interface ToastState {
  id: number;
  text: string;
  kind: 'info' | 'error';
  action?: { label: string; run: () => void };
}

interface ToastProps {
  toast: ToastState;
  onClose: () => void;
}

export function Toast({ toast, onClose }: ToastProps) {
  return (
    <motion.div
      className={`toast${toast.kind === 'error' ? ' error' : ''}`}
      initial={{ opacity: 0, y: 10 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0, y: 6, transition: { duration: 0.16 } }}
      transition={{ duration: 0.16, ease: [0.2, 0.8, 0.2, 1] }}
    >
      <span>{toast.text}</span>
      {toast.action && (
        <button
          onClick={() => {
            toast.action?.run();
            onClose();
          }}
        >
          {toast.action.label}
        </button>
      )}
      <button onClick={onClose}>关闭</button>
    </motion.div>
  );
}
