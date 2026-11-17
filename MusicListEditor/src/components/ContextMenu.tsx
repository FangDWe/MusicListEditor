import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { motion } from 'motion/react';

export interface MenuEntry {
  key: string;
  label: string;
  hint?: string;
  disabled?: boolean;
  danger?: boolean;
  separatorBefore?: boolean;
  onClick?: () => void;
}

interface ContextMenuProps {
  x: number;
  y: number;
  items: MenuEntry[];
  onClose: () => void;
}

export function ContextMenu({ x, y, items, onClose }: ContextMenuProps) {
  const ref = useRef<HTMLDivElement>(null);
  const [position, setPosition] = useState({ left: x, top: y });

  useLayoutEffect(() => {
    const element = ref.current;
    if (!element) return;
    const rect = element.getBoundingClientRect();
    setPosition({
      left: Math.max(8, Math.min(x, window.innerWidth - rect.width - 8)),
      top: Math.max(8, Math.min(y, window.innerHeight - rect.height - 8)),
    });
  }, [x, y]);

  useEffect(() => {
    const onPointerDown = (event: globalThis.MouseEvent): void => {
      if (!ref.current?.contains(event.target as Node)) onClose();
    };
    const onKeyDown = (event: KeyboardEvent): void => {
      if (event.key === 'Escape') onClose();
    };
    window.addEventListener('mousedown', onPointerDown);
    window.addEventListener('keydown', onKeyDown);
    window.addEventListener('resize', onClose);
    return () => {
      window.removeEventListener('mousedown', onPointerDown);
      window.removeEventListener('keydown', onKeyDown);
      window.removeEventListener('resize', onClose);
    };
  }, [onClose]);

  return (
    <motion.div
      ref={ref}
      className="menu"
      style={{ left: position.left, top: position.top }}
      initial={{ opacity: 0, scale: 0.96, y: -4 }}
      animate={{ opacity: 1, scale: 1, y: 0 }}
      exit={{ opacity: 0, scale: 0.97, transition: { duration: 0.09 } }}
      transition={{ duration: 0.12, ease: [0.2, 0.8, 0.2, 1] }}
    >
      {items.map((item) => (
        <div key={item.key}>
          {item.separatorBefore && <div className="menu-sep" />}
          <div
            className={`menu-item${item.disabled ? ' disabled' : ''}`}
            style={item.danger ? { color: 'var(--color-danger)' } : undefined}
            onClick={() => {
              if (item.disabled) return;
              item.onClick?.();
              onClose();
            }}
          >
            {item.label}
            {item.hint && <span className="hint">{item.hint}</span>}
          </div>
        </div>
      ))}
    </motion.div>
  );
}
