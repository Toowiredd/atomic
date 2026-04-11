import { useState } from 'react';
import { Network, Plus, Minus, Maximize2 } from 'lucide-react';
import { useControls } from 'react-zoom-pan-pinch';
import { ConnectionOptions } from './CanvasView';

interface CanvasControlsProps {
  connectionOptions: ConnectionOptions;
  onConnectionOptionsChange: (options: ConnectionOptions) => void;
}

export function CanvasControls({ connectionOptions, onConnectionOptionsChange }: CanvasControlsProps) {
  const { zoomIn, zoomOut, resetTransform } = useControls();
  const [showPanel, setShowPanel] = useState(false);

  const handleToggleTag = () => {
    onConnectionOptionsChange({
      ...connectionOptions,
      showTagConnections: !connectionOptions.showTagConnections,
    });
  };

  const handleToggleSemantic = () => {
    onConnectionOptionsChange({
      ...connectionOptions,
      showSemanticConnections: !connectionOptions.showSemanticConnections,
    });
  };

  const handleSimilarityChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    onConnectionOptionsChange({
      ...connectionOptions,
      minSimilarity: parseFloat(e.target.value),
    });
  };

  return (
    <>
      {/* Connection options panel */}
      {showPanel && (
        <div className="absolute bottom-16 right-4 z-10 bg-[var(--color-bg-card)] border border-[var(--color-border)] rounded-md p-3 w-56 shadow-lg">
          <div className="text-xs text-[var(--color-text-secondary)] mb-3 font-medium">Connections</div>

          {/* Tag connections toggle */}
          <label className="flex items-center gap-2 mb-2 cursor-pointer">
            <input
              type="checkbox"
              checked={connectionOptions.showTagConnections}
              onChange={handleToggleTag}
              className="w-4 h-4 rounded bg-[var(--color-bg-main)] border-[var(--color-border)] text-[var(--color-accent)] focus:ring-[var(--color-accent)] focus:ring-offset-0"
            />
            <span className="text-sm text-[var(--color-text-primary)] flex items-center gap-2">
              <span className="w-4 h-0.5 bg-[var(--color-text-tertiary)] inline-block" />
              Tag connections
            </span>
          </label>

          {/* Semantic connections toggle */}
          <label className="flex items-center gap-2 mb-3 cursor-pointer">
            <input
              type="checkbox"
              checked={connectionOptions.showSemanticConnections}
              onChange={handleToggleSemantic}
              className="w-4 h-4 rounded bg-[var(--color-bg-main)] border-[var(--color-border)] text-[var(--color-accent)] focus:ring-[var(--color-accent)] focus:ring-offset-0"
            />
            <span className="text-sm text-[var(--color-text-primary)] flex items-center gap-2">
              <span className="w-4 h-0.5 bg-[var(--color-accent)] inline-block" style={{ backgroundImage: 'repeating-linear-gradient(90deg, var(--color-accent) 0, var(--color-accent) 4px, transparent 4px, transparent 6px)' }} />
              Semantic connections
            </span>
          </label>

          {/* Similarity threshold slider */}
          {connectionOptions.showSemanticConnections && (
            <div className="pt-2 border-t border-[var(--color-border)]">
              <div className="flex items-center justify-between mb-1">
                <span className="text-xs text-[var(--color-text-secondary)]">Min similarity</span>
                <span className="text-xs text-[var(--color-text-primary)] font-mono">{connectionOptions.minSimilarity.toFixed(2)}</span>
              </div>
              <input
                type="range"
                min="0.3"
                max="0.9"
                step="0.05"
                value={connectionOptions.minSimilarity}
                onChange={handleSimilarityChange}
                className="w-full h-1.5 bg-[var(--color-bg-hover)] rounded-lg appearance-none cursor-pointer accent-[var(--color-accent)]"
              />
              <div className="flex justify-between text-xs text-[var(--color-text-tertiary)] mt-1">
                <span>More</span>
                <span>Fewer</span>
              </div>
            </div>
          )}

          {/* Legend */}
          <div className="mt-3 pt-2 border-t border-[var(--color-border)]">
            <div className="text-xs text-[var(--color-text-secondary)] mb-2">Legend</div>
            <div className="space-y-1.5 text-xs">
              <div className="flex items-center gap-2">
                <span className="w-5 h-0.5 bg-[var(--color-text-tertiary)] inline-block" />
                <span className="text-[var(--color-text-secondary)]">Shared tags</span>
              </div>
              <div className="flex items-center gap-2">
                <span className="w-5 h-0.5 inline-block" style={{ backgroundImage: 'repeating-linear-gradient(90deg, var(--color-accent) 0, var(--color-accent) 4px, transparent 4px, transparent 6px)' }} />
                <span className="text-[var(--color-text-secondary)]">Semantic similarity</span>
              </div>
              <div className="flex items-center gap-2">
                <span className="w-5 h-0.5 bg-[var(--color-accent-light)] inline-block" />
                <span className="text-[var(--color-text-secondary)]">Both</span>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Control buttons */}
      <div className="absolute bottom-4 right-4 flex flex-col gap-1 z-10">
        {/* Connection options toggle */}
        <button
          onClick={() => setShowPanel(!showPanel)}
          className={`w-8 h-8 border rounded transition-colors flex items-center justify-center ${
            showPanel
              ? 'bg-[var(--color-accent)] border-[var(--color-accent)] text-white'
              : 'bg-[var(--color-bg-card)] border-[var(--color-border)] text-[var(--color-text-primary)] hover:bg-[var(--color-bg-hover)]'
          }`}
          title="Connection options"
        >
          <Network className="w-4 h-4" strokeWidth={2} />
        </button>

        <div className="h-px bg-[var(--color-bg-hover)] my-1" />

        <button
          onClick={() => zoomIn()}
          className="w-8 h-8 bg-[var(--color-bg-card)] border border-[var(--color-border)] rounded text-[var(--color-text-primary)] hover:bg-[var(--color-bg-hover)] transition-colors flex items-center justify-center"
          title="Zoom in"
        >
          <Plus className="w-4 h-4" strokeWidth={2} />
        </button>
        <button
          onClick={() => zoomOut()}
          className="w-8 h-8 bg-[var(--color-bg-card)] border border-[var(--color-border)] rounded text-[var(--color-text-primary)] hover:bg-[var(--color-bg-hover)] transition-colors flex items-center justify-center"
          title="Zoom out"
        >
          <Minus className="w-4 h-4" strokeWidth={2} />
        </button>
        <button
          onClick={() => resetTransform()}
          className="w-8 h-8 bg-[var(--color-bg-card)] border border-[var(--color-border)] rounded text-[var(--color-text-primary)] hover:bg-[var(--color-bg-hover)] transition-colors flex items-center justify-center"
          title="Reset view"
        >
          <Maximize2 className="w-4 h-4" strokeWidth={2} />
        </button>
      </div>
    </>
  );
}
