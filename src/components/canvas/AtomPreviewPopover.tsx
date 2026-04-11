import { useEffect, useLayoutEffect, useMemo, useRef, useState, memo } from 'react';
import { ArrowRight, Loader2 } from 'lucide-react';
import { createPortal } from 'react-dom';
import ReactMarkdown from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { useKeyboard } from '../../hooks/useKeyboard';
import { getTransport } from '../../lib/transport';
import { chunkMarkdown } from '../../lib/markdown';
import type { AtomWithTags } from '../../stores/atoms';
import { TagChip } from '../tags/TagChip';
import { formatDate } from '../../lib/date';

interface AtomPreviewPopoverProps {
  atomId: string;
  anchorRect: { top: number; left: number; bottom: number; width: number };
  onClose: () => void;
  onViewAtom: (atomId: string) => void;
}

const POPOVER_WIDTH = 480;
const POPOVER_MAX_HEIGHT = 360;
const CHUNK_SIZE = 4000;
const INITIAL_CHUNKS = 1;
const CHUNKS_PER_BATCH = 2;

const remarkPluginsStable = [remarkGfm];

const MemoizedChunk = memo(function MemoizedChunk({ content }: { content: string }) {
  return (
    <ReactMarkdown remarkPlugins={remarkPluginsStable}>
      {content}
    </ReactMarkdown>
  );
});

function calculatePosition(
  anchorRect: { top: number; left: number; bottom: number; width: number },
  popoverHeight: number,
  popoverWidth: number
): { top: number; left: number } {
  const spaceBelow = window.innerHeight - anchorRect.bottom;
  const spaceAbove = anchorRect.top;

  let top: number;
  if (spaceBelow >= popoverHeight + 12 || spaceBelow >= spaceAbove) {
    top = anchorRect.bottom + 12;
  } else {
    top = anchorRect.top - popoverHeight - 12;
  }

  let left = anchorRect.left + anchorRect.width / 2 - popoverWidth / 2;
  left = Math.max(8, Math.min(left, window.innerWidth - popoverWidth - 8));

  top = Math.max(8, Math.min(top, window.innerHeight - popoverHeight - 8));

  return { top, left };
}

export function AtomPreviewPopover({ atomId, anchorRect, onClose, onViewAtom }: AtomPreviewPopoverProps) {
  const popoverRef = useRef<HTMLDivElement>(null);
  const [atom, setAtom] = useState<AtomWithTags | null>(null);
  const [isLoading, setIsLoading] = useState(true);

  const initialPosition = calculatePosition(anchorRect, POPOVER_MAX_HEIGHT, POPOVER_WIDTH);
  const [position, setPosition] = useState(initialPosition);

  useKeyboard('Escape', onClose, true);

  // Close on click outside
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (popoverRef.current && !popoverRef.current.contains(event.target as Node)) {
        onClose();
      }
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, [onClose]);

  // Fetch atom data
  useEffect(() => {
    setIsLoading(true);
    getTransport().invoke<AtomWithTags | null>('get_atom_by_id', { id: atomId })
      .then((result) => {
        setAtom(result);
        setIsLoading(false);
      })
      .catch((err) => {
        console.error('Failed to fetch atom for preview:', err);
        setIsLoading(false);
      });
  }, [atomId]);

  // Refine position after render
  useLayoutEffect(() => {
    if (!popoverRef.current) return;
    const rect = popoverRef.current.getBoundingClientRect();
    setPosition(calculatePosition(anchorRect, rect.height, rect.width));
  }, [anchorRect, atom, isLoading]);

  const handleViewAtom = () => {
    onViewAtom(atomId);
    onClose();
  };

  // Progressive rendering: chunk content, render first chunk immediately, load rest incrementally
  const chunks = useMemo(
    () => (atom ? chunkMarkdown(atom.content, CHUNK_SIZE) : []),
    [atom]
  );
  const [renderedChunkCount, setRenderedChunkCount] = useState(INITIAL_CHUNKS);
  const isFullyRendered = renderedChunkCount >= chunks.length;

  // Reset when atom changes
  useEffect(() => { setRenderedChunkCount(INITIAL_CHUNKS); }, [atom?.id]);

  // Progressively load remaining chunks
  useEffect(() => {
    if (isFullyRendered) return;
    if ('requestIdleCallback' in window) {
      const id = requestIdleCallback(() => {
        setRenderedChunkCount((prev) => Math.min(prev + CHUNKS_PER_BATCH, chunks.length));
      }, { timeout: 100 });
      return () => cancelIdleCallback(id);
    } else {
      const id = setTimeout(() => {
        setRenderedChunkCount((prev) => Math.min(prev + CHUNKS_PER_BATCH, chunks.length));
      }, 32);
      return () => clearTimeout(id);
    }
  }, [renderedChunkCount, chunks.length, isFullyRendered]);

  return createPortal(
    <div
      ref={popoverRef}
      data-modal="true"
      className="fixed z-[100] bg-[var(--color-bg-card)] border border-[var(--color-border)] rounded-lg shadow-xl"
      style={{ top: position.top, left: position.left, width: POPOVER_WIDTH, maxWidth: 'calc(100vw - 16px)' }}
    >
      {isLoading ? (
        <div className="flex items-center justify-center py-8">
          <div className="flex items-center gap-2 text-[var(--color-text-secondary)]">
            <Loader2 className="animate-spin h-4 w-4" strokeWidth={2} />
            <span className="text-sm">Loading...</span>
          </div>
        </div>
      ) : !atom ? (
        <div className="py-6 text-center text-sm text-[var(--color-text-secondary)]">
          Atom not found
        </div>
      ) : (
        <>
          {/* Header: title + metadata */}
          <div className="px-4 py-3 border-b border-[var(--color-border)]">
            <h3 className="text-sm font-medium text-[var(--color-text-primary)] line-clamp-2">
              {atom.title || 'Untitled'}
            </h3>
            <div className="flex items-center gap-2 mt-1">
              {atom.source_url && (
                <span className="text-[10px] text-[var(--color-text-tertiary)] bg-[var(--color-bg-main)] px-1.5 py-0.5 rounded-full truncate max-w-[200px]">
                  {atom.source || (() => { try { return new URL(atom.source_url!).hostname.replace(/^www\./, ''); } catch { return atom.source_url; } })()}
                </span>
              )}
              <span className="text-[10px] text-[var(--color-text-tertiary)]">
                {formatDate(atom.updated_at)}
              </span>
            </div>
          </div>

          {/* Content preview */}
          <div className="px-4 py-3 prose prose-invert prose-sm max-w-none [&_h1]:text-sm [&_h2]:text-sm [&_h3]:text-xs [&_h4]:text-xs [&_h1]:m-0 [&_h2]:m-0 [&_h3]:m-0 [&_h4]:m-0 [&_p]:text-xs [&_li]:text-xs [&_pre]:text-[10px] [&_code]:text-[10px] [&_blockquote]:text-xs max-h-[200px] overflow-y-auto">
            {chunks.slice(0, renderedChunkCount).map((chunk, i) => (
              <MemoizedChunk key={i} content={chunk} />
            ))}
          </div>

          {/* Tags */}
          {atom.tags.length > 0 && (
            <div className="px-4 pt-1 pb-2 flex flex-wrap gap-1">
              {atom.tags.slice(0, 5).map((tag) => (
                <TagChip key={tag.id} name={tag.name} size="xs" />
              ))}
              {atom.tags.length > 5 && (
                <span className="text-[10px] text-[var(--color-text-tertiary)] self-center">
                  +{atom.tags.length - 5} more
                </span>
              )}
            </div>
          )}

          {/* Footer */}
          <div className="px-4 py-2 border-t border-[var(--color-border)]">
            <button
              onClick={handleViewAtom}
              className="flex items-center gap-1 text-sm text-[var(--color-accent)] hover:text-[var(--color-accent-light)] transition-colors"
            >
              Open in reader
              <ArrowRight className="w-4 h-4" strokeWidth={2} />
            </button>
          </div>
        </>
      )}
    </div>,
    document.body
  );
}
