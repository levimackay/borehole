import type { ReactNode } from "react";
import "./EmptyState.css";

export function EmptyState({
  title,
  description,
  action,
}: {
  title: string;
  description?: string;
  action?: ReactNode;
}) {
  return (
    <div className="bh-empty-state">
      <p className="bh-empty-state__title">{title}</p>
      {description && <p className="bh-empty-state__description">{description}</p>}
      {action && <div className="bh-empty-state__action">{action}</div>}
    </div>
  );
}
