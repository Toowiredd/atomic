import { useRef, useEffect } from 'react';
import { Search, Tag, FileText, Loader2 } from 'lucide-react';
import { PaletteMode } from './types';

interface CommandInputProps {
  query: string;
  onChange: (value: string) => void;
  onKeyDown: (e: React.KeyboardEvent) => void;
  mode: PaletteMode;
  isSearching: boolean;
}

const modeConfig: Record<PaletteMode, { placeholder: string; icon: React.ReactNode }> = {
  commands: {
    placeholder: 'Type a command or search...',
    icon: <Search className="w-5 h-5" strokeWidth={2} />,
  },
  search: {
    placeholder: 'Search atoms...',
    icon: <FileText className="w-5 h-5" strokeWidth={2} />,
  },
  tags: {
    placeholder: 'Filter by tag...',
    icon: <Tag className="w-5 h-5" strokeWidth={2} />,
  },
};

export function CommandInput({
  query,
  onChange,
  onKeyDown,
  mode,
  isSearching,
}: CommandInputProps) {
  const inputRef = useRef<HTMLInputElement>(null);
  const config = modeConfig[mode];

  // Auto-focus on mount
  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  return (
    <div className="flex items-center gap-3 px-4 py-3 border-b border-[var(--color-border)]">
      <div className="text-[var(--color-text-secondary)]">
        {isSearching ? (
          <Loader2 className="w-5 h-5 animate-spin" strokeWidth={2} />
        ) : (
          config.icon
        )}
      </div>

      <input
        ref={inputRef}
        type="text"
        value={query}
        onChange={(e) => onChange(e.target.value)}
        onKeyDown={onKeyDown}
        placeholder={config.placeholder}
        className="flex-1 bg-transparent text-[var(--color-text-primary)] placeholder-[var(--color-text-tertiary)] outline-none text-base"
        autoComplete="off"
        autoCorrect="off"
        autoCapitalize="off"
        spellCheck={false}
      />

      <div className="flex items-center gap-2 text-xs text-[var(--color-text-tertiary)]">
        <kbd className="px-1.5 py-0.5 bg-[var(--color-bg-hover)] rounded text-[10px] font-mono">
          ⌘P
        </kbd>
      </div>
    </div>
  );
}
