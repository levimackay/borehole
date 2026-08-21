import type { Symbol } from "../lib/ipc-types";
import { formatKind } from "../lib/format";
import "./SymbolResultsList.css";

export function SymbolResultsList({
  symbols,
  selectedId,
  onSelect,
}: {
  symbols: Symbol[];
  selectedId?: number;
  onSelect: (symbol: Symbol) => void;
}) {
  if (symbols.length === 0) {
    return <p className="bh-results__empty">No symbols matched.</p>;
  }

  return (
    <ul className="bh-results">
      {symbols.map((symbol) => (
        <li key={symbol.id}>
          <button
            type="button"
            className={`bh-results__row${symbol.id === selectedId ? " bh-results__row--selected" : ""}`}
            onClick={() => onSelect(symbol)}
            aria-current={symbol.id === selectedId}
          >
            <span className="bh-results__kind">{formatKind(symbol.kind)}</span>
            <span className="bh-results__name bh-mono">{symbol.name}</span>
            {symbol.is_exported && <span className="bh-results__exported">exported</span>}
            <span className="bh-results__location bh-mono">
              {symbol.file}:{symbol.span.start_line}
            </span>
          </button>
        </li>
      ))}
    </ul>
  );
}
