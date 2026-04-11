import { useEffect, useCallback, useRef, useState, useMemo } from 'react';
import { Loader2, Plus, Minus, RotateCcw } from 'lucide-react';
import { useShallow } from 'zustand/react/shallow';
import { TransformWrapper, TransformComponent } from 'react-zoom-pan-pinch';
import { useUIStore } from '../../stores/ui';
import { CanvasBreadcrumb } from './CanvasBreadcrumb';
import { ClusterBubble } from './ClusterBubble';
import { AtomNode } from './AtomNode';
import { useHierarchicalForceSimulation } from './useHierarchicalForceSimulation';
import { useSemanticZoom } from './useSemanticZoom';
import type { CanvasNode, CanvasEdge } from '../../lib/api';

const EMPTY_NODES: CanvasNode[] = [];
const EMPTY_EDGES: CanvasEdge[] = [];

export function HierarchicalCanvas() {
  const { currentLevel, isLoading } = useUIStore(
    useShallow(s => ({
      currentLevel: s.canvasNav.currentLevel,
      isLoading: s.canvasNav.isLoading,
    }))
  );
  const navigateCanvas = useUIStore(s => s.navigateCanvas);
  const openReader = useUIStore(s => s.openReader);
  const selectedTagId = useUIStore(s => s.selectedTagId);

  const containerRef = useRef<HTMLDivElement>(null);
  const [dimensions, setDimensions] = useState({ width: 0, height: 0 });
  const [fadeIn, setFadeIn] = useState(false);

  // Measure container
  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;

    const observer = new ResizeObserver((entries) => {
      const { width, height } = entries[0].contentRect;
      setDimensions({ width, height });
    });
    observer.observe(el);
    return () => observer.disconnect();
  }, []);

  // Load root level on mount (only once)
  const hasInitializedRef = useRef(false);
  useEffect(() => {
    if (!currentLevel && !isLoading && !hasInitializedRef.current) {
      hasInitializedRef.current = true;
      navigateCanvas(null);
    }
  }, [currentLevel, isLoading, navigateCanvas]);

  // Sync with sidebar tag selection
  const lastSyncedTagRef = useRef<string | null>(null);
  useEffect(() => {
    if (selectedTagId && selectedTagId !== lastSyncedTagRef.current) {
      lastSyncedTagRef.current = selectedTagId;
      navigateCanvas(selectedTagId);
    } else if (!selectedTagId && lastSyncedTagRef.current) {
      lastSyncedTagRef.current = null;
      navigateCanvas(null);
    }
  }, [selectedTagId, navigateCanvas]);

  // Fade-in transition on level change
  useEffect(() => {
    if (currentLevel) {
      setFadeIn(false);
      const timer = requestAnimationFrame(() => setFadeIn(true));
      return () => cancelAnimationFrame(timer);
    }
  }, [currentLevel]);

  const nodes = currentLevel?.nodes ?? EMPTY_NODES;
  const edges = currentLevel?.edges ?? EMPTY_EDGES;

  const { simNodes } = useHierarchicalForceSimulation({
    nodes,
    edges,
    width: dimensions.width,
    height: dimensions.height,
  });

  // Compute initial scale
  const initialScale = nodes.length <= 8 ? 2 : nodes.length <= 20 ? 1.5 : 1.2;

  // Semantic zoom
  const {
    onTransformed,
    expandedClusterId,
    expandedSimNodes,
    expansionProgress,
    prefetchCluster,
    isExpandLoading,
  } = useSemanticZoom({
    simNodes,
    dimensions,
    initialScale,
    currentLevel,
  });

  const handleBreadcrumbNavigate = useCallback((parentId: string | null) => {
    navigateCanvas(parentId);
  }, [navigateCanvas]);

  const handleNodeClick = useCallback((node: CanvasNode) => {
    if (node.node_type === 'atom') {
      openReader(node.id);
    } else if (node.node_type === 'semantic_cluster') {
      navigateCanvas(node.id, node.children_ids);
    } else {
      navigateCanvas(node.id);
    }
  }, [navigateCanvas, openReader]);

  const handleAtomNodeClick = useCallback((atomId: string) => {
    openReader(atomId);
  }, [openReader]);

  // Build node position map for edge rendering
  const nodePositionMap = useMemo(() => {
    const map = new Map<string, { x: number; y: number }>();
    for (const sn of simNodes) {
      map.set(sn.id, { x: sn.x, y: sn.y });
    }
    return map;
  }, [simNodes]);

  // Compute per-atom connection counts from edges
  const atomConnectionCounts = useMemo(() => {
    const counts = new Map<string, number>();
    for (const edge of edges) {
      counts.set(edge.source_id, (counts.get(edge.source_id) || 0) + 1);
      counts.set(edge.target_id, (counts.get(edge.target_id) || 0) + 1);
    }
    return counts;
  }, [edges]);

  // Build atom summary objects for AtomNode (minimal shape)
  const buildAtomSummary = useCallback((node: CanvasNode) => ({
    id: node.id,
    title: node.label,
    snippet: '',
    tags: (node.dominant_tags || []).map(name => ({ id: name, name, parent_id: null, created_at: '' })),
    source_url: null,
    source: null,
    published_at: null,
    created_at: '',
    updated_at: '',
    embedding_status: 'complete' as const,
    tagging_status: 'complete' as const,
  }), []);

  const atomSummaryMap = useMemo(() => {
    const map = new Map<string, import('../../stores/atoms').AtomSummary>();
    for (const node of nodes) {
      if (node.node_type === 'atom') {
        map.set(node.id, buildAtomSummary(node));
      }
    }
    return map;
  }, [nodes, buildAtomSummary]);

  // Build summaries for expanded nodes too
  const expandedAtomSummaryMap = useMemo(() => {
    const map = new Map<string, import('../../stores/atoms').AtomSummary>();
    for (const sn of expandedSimNodes) {
      if (sn.canvasNode.node_type === 'atom') {
        map.set(sn.id, buildAtomSummary(sn.canvasNode));
      }
    }
    return map;
  }, [expandedSimNodes, buildAtomSummary]);

  return (
    <div className="flex flex-col h-full w-full">
      {/* Breadcrumb */}
      <CanvasBreadcrumb
        breadcrumb={currentLevel?.breadcrumb ?? []}
        onNavigate={handleBreadcrumbNavigate}
      />

      {/* Canvas area */}
      <div ref={containerRef} className="flex-1 relative overflow-hidden">
        {isLoading && (
          <div className="absolute inset-0 flex items-center justify-center z-10 bg-[var(--color-bg-main)]/50">
            <div className="flex items-center gap-2 text-[var(--color-text-secondary)]">
              <Loader2 className="animate-spin h-5 w-5" strokeWidth={2} />
              <span className="text-sm">Loading...</span>
            </div>
          </div>
        )}

        {!isLoading && nodes.length === 0 && (
          <div className="absolute inset-0 flex items-center justify-center">
            <div className="text-center text-[var(--color-text-secondary)]">
              <p className="text-lg mb-2">No content to display</p>
              <p className="text-sm">Create some atoms to see them on the canvas</p>
            </div>
          </div>
        )}

        {dimensions.width > 0 && simNodes.length > 0 && (() => {
          const offsetX = -(dimensions.width * initialScale - dimensions.width) / 2;
          const offsetY = -(dimensions.height * initialScale - dimensions.height) / 2;
          return (
          <div className={`w-full h-full transition-opacity duration-200 ${fadeIn ? 'opacity-100' : 'opacity-0'}`}>
          <TransformWrapper
            initialScale={initialScale}
            initialPositionX={offsetX}
            initialPositionY={offsetY}
            minScale={0.3}
            maxScale={8}
            limitToBounds={false}
            doubleClick={{ disabled: true }}
            panning={{ velocityDisabled: true }}
            onTransformed={onTransformed}
          >
            {({ zoomIn, zoomOut, resetTransform }) => (
              <>
                <TransformComponent
                  wrapperStyle={{ width: '100%', height: '100%' }}
                  contentStyle={{ width: dimensions.width, height: dimensions.height }}
                >
                  {/* Edge lines */}
                  <svg
                    className="absolute top-0 left-0 pointer-events-none"
                    width={dimensions.width}
                    height={dimensions.height}
                    style={{ zIndex: 0, overflow: 'visible' }}
                  >
                    {edges.map((edge) => {
                      const src = nodePositionMap.get(edge.source_id);
                      const tgt = nodePositionMap.get(edge.target_id);
                      if (!src || !tgt) return null;
                      const w = edge.weight;
                      const strong = w >= 0.6;
                      return (
                        <line
                          key={`${edge.source_id}-${edge.target_id}`}
                          x1={src.x}
                          y1={src.y}
                          x2={tgt.x}
                          y2={tgt.y}
                          stroke={strong ? 'hsl(265, 55%, 58%)' : 'hsl(0, 0%, 42%)'}
                          strokeWidth={0.5}
                          strokeOpacity={strong ? 0.5 : 0.2}
                          strokeDasharray={strong ? undefined : '3 3'}
                        />
                      );
                    })}
                  </svg>

                  {/* Parent level nodes */}
                  {simNodes.map((sn) => {
                    const node = sn.canvasNode;
                    const isExpanding = node.id === expandedClusterId;

                    if (node.node_type === 'atom') {
                      const atomSummary = atomSummaryMap.get(node.id);
                      if (!atomSummary) return null;
                      const connCount = atomConnectionCounts.get(node.id) || 0;
                      return (
                        <AtomNode
                          key={node.id}
                          atom={atomSummary}
                          atomId={node.id}
                          x={sn.x}
                          y={sn.y}
                          isFaded={false}
                          isHub={connCount >= 3}
                          connectionCount={connCount}
                          onClick={handleAtomNodeClick}
                        />
                      );
                    }

                    return (
                      <ClusterBubble
                        key={node.id}
                        node={node}
                        x={sn.x}
                        y={sn.y}
                        onClick={handleNodeClick}
                        onMouseEnter={
                          node.children_ids?.length
                            ? () => prefetchCluster(node.id, node.children_ids)
                            : undefined
                        }
                        style={isExpanding ? { opacity: 1 - expansionProgress } : undefined}
                      />
                    );
                  })}

                  {/* Loading spinner for expanding cluster */}
                  {isExpandLoading && expandedClusterId && (() => {
                    const pos = nodePositionMap.get(expandedClusterId);
                    if (!pos) return null;
                    return (
                      <div
                        className="absolute pointer-events-none"
                        style={{ left: pos.x, top: pos.y, transform: 'translate(-50%, -50%)' }}
                      >
                        <Loader2 className="animate-spin h-4 w-4 text-[var(--color-accent)]" strokeWidth={2} />
                      </div>
                    );
                  })()}

                  {/* Expanded cluster children */}
                  {expandedSimNodes.map((sn) => {
                    const node = sn.canvasNode;
                    if (node.node_type === 'atom') {
                      const atomSummary = expandedAtomSummaryMap.get(sn.id);
                      if (!atomSummary) return null;
                      return (
                        <AtomNode
                          key={`expanded-${sn.id}`}
                          atom={atomSummary}
                          atomId={sn.id}
                          x={sn.x}
                          y={sn.y}
                          isFaded={false}
                          onClick={handleAtomNodeClick}
                          style={{ opacity: expansionProgress }}
                        />
                      );
                    }
                    // Sub-clusters from large expansions
                    return (
                      <ClusterBubble
                        key={`expanded-${sn.id}`}
                        node={node}
                        x={sn.x}
                        y={sn.y}
                        onClick={handleNodeClick}
                        style={{ opacity: expansionProgress }}
                      />
                    );
                  })}
                </TransformComponent>

                {/* Zoom controls */}
                <div className="absolute bottom-4 left-4 flex flex-col gap-1 z-10">
                  <button
                    onClick={() => zoomIn()}
                    className="w-8 h-8 rounded-md bg-[var(--color-bg-card)] border border-[var(--color-border)] text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)] hover:bg-[var(--color-bg-hover)] flex items-center justify-center transition-colors"
                    title="Zoom in"
                  >
                    <Plus className="w-4 h-4" strokeWidth={2} />
                  </button>
                  <button
                    onClick={() => zoomOut()}
                    className="w-8 h-8 rounded-md bg-[var(--color-bg-card)] border border-[var(--color-border)] text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)] hover:bg-[var(--color-bg-hover)] flex items-center justify-center transition-colors"
                    title="Zoom out"
                  >
                    <Minus className="w-4 h-4" strokeWidth={2} />
                  </button>
                  <button
                    onClick={() => resetTransform()}
                    className="w-8 h-8 rounded-md bg-[var(--color-bg-card)] border border-[var(--color-border)] text-[var(--color-text-secondary)] hover:text-[var(--color-text-primary)] hover:bg-[var(--color-bg-hover)] flex items-center justify-center transition-colors"
                    title="Reset view"
                  >
                    <RotateCcw className="w-4 h-4" strokeWidth={2} />
                  </button>
                </div>
              </>
            )}
          </TransformWrapper>
          </div>
          );
        })()}
      </div>
    </div>
  );
}
