import type { ReactNode } from "react";
import "./Panel.css";

export function Panel({
  title,
  actions,
  children,
  className,
}: {
  title?: string;
  actions?: ReactNode;
  children: ReactNode;
  className?: string;
}) {
  return (
    <section className={`bh-panel${className ? ` ${className}` : ""}`}>
      {title && (
        <header className="bh-panel__header">
          <h2 className="bh-panel__title">{title}</h2>
          {actions && <div className="bh-panel__actions">{actions}</div>}
        </header>
      )}
      <div className="bh-panel__body">{children}</div>
    </section>
  );
}
