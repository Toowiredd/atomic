import { useEffect, useRef, useState } from 'react';
import { useUIStore } from '../../stores/ui';
import { useDatabasesStore } from '../../stores/databases';
import { getGlobalCanvas, type GlobalCanvasData } from '../../lib/api';
import Graph from 'graphology';
import Sigma from 'sigma';
import EdgeCurveProgram from '@sigma/edge-curve';
import {
  CANVAS_THEMES,
  DEFAULT_THEME,
  nodeColor,
  edgeColor,
  type CanvasTheme,
} from './sigma/themes';

function truncLabel(str: string, max: number): string {
  return str.length > max ? str.substring(0, max - 1) + '\u2026' : str;
}

export function SigmaCanvas() {
  const openDrawer = useUIStore(s => s.openDrawer);
  const selectedTagId = useUIStore(s => s.selectedTagId);
  const activeDbId = useDatabasesStore(s => s.activeId);
  const containerRef = useRef<HTMLDivElement>(null);
  const sigmaRef = useRef<Sigma | null>(null);
  const graphRef = useRef<Graph | null>(null);
  const [data, setData] = useState<GlobalCanvasData | null>(null);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [theme, setTheme] = useState<CanvasTheme>(DEFAULT_THEME);
  const [themePickerOpen, setThemePickerOpen] = useState(false);
  const [edgeThreshold, setEdgeThreshold] = useState(0);
  const edgeThresholdRef = useRef(0);
  const edgeAnimProgress = useRef(1); // 0 = invisible, 1 = fully visible
  const themeRef = useRef(theme);
  themeRef.current = theme;

  // Build a set of atom IDs that match the selected tag
  const selectedTagRef = useRef(selectedTagId);
  selectedTagRef.current = selectedTagId;

  // Fetch global canvas data
  useEffect(() => {
    let cancelled = false;
    setIsLoading(true);
    setError(null);

    getGlobalCanvas()
      .then((result) => {
        if (!cancelled) {
          setData(result);
          setIsLoading(false);
        }
      })
      .catch((err) => {
        if (!cancelled) {
          setError(err.message || 'Failed to load canvas');
          setIsLoading(false);
        }
      });

    return () => { cancelled = true; };
  }, [activeDbId]);

  // Precomputed data for the graph
  const graphDataRef = useRef<{
    edgeCounts: Map<string, number>;
    maxEdges: number;
  } | null>(null);

  // Create Sigma graph when data is loaded
  useEffect(() => {
    const container = containerRef.current;
    if (!container || !data || data.atoms.length === 0) return;

    if (sigmaRef.current) {
      sigmaRef.current.kill();
      sigmaRef.current = null;
    }

    const graph = new Graph();
    graphRef.current = graph;
    const scale = 500;

    // Compute per-atom edge count
    const edgeCounts = new Map<string, number>();
    for (const edge of data.edges) {
      edgeCounts.set(edge.source, (edgeCounts.get(edge.source) || 0) + 1);
      edgeCounts.set(edge.target, (edgeCounts.get(edge.target) || 0) + 1);
    }
    const maxEdges = Math.max(1, ...edgeCounts.values());
    graphDataRef.current = { edgeCounts, maxEdges };

    // Build atom → cluster index map
    const atomCluster = new Map<string, number>();
    for (let i = 0; i < data.clusters.length; i++) {
      for (const atomId of data.clusters[i].atom_ids) {
        atomCluster.set(atomId, i);
      }
    }

    // Add atom nodes at center — will animate to PCA positions
    const targetPositions: Record<string, { x: number; y: number }> = {};
    for (const atom of data.atoms) {
      const connectivity = (edgeCounts.get(atom.atom_id) || 0) / maxEdges;
      const clusterIdx = atomCluster.get(atom.atom_id);
      targetPositions[atom.atom_id] = { x: atom.x * scale, y: atom.y * scale };
      graph.addNode(atom.atom_id, {
        x: 0,
        y: 0,
        size: 2.5 + connectivity * 5,
        color: nodeColor(theme, connectivity, clusterIdx),
        label: truncLabel(atom.title || atom.atom_id.substring(0, 8), 30),
        fullLabel: atom.title || atom.atom_id.substring(0, 8),
        connectivity,
        clusterIndex: clusterIdx,
        tagIds: atom.tag_ids,
      });
    }

    // Add edges
    let minW = 1, maxW = 0;
    for (const edge of data.edges) {
      if (edge.weight < minW) minW = edge.weight;
      if (edge.weight > maxW) maxW = edge.weight;
    }
    const wRange = Math.max(maxW - minW, 0.001);

    for (const edge of data.edges) {
      if (!graph.hasNode(edge.source) || !graph.hasNode(edge.target)) continue;
      if (graph.hasEdge(edge.source, edge.target) || graph.hasEdge(edge.target, edge.source)) continue;
      const w = (edge.weight - minW) / wRange;
      graph.addEdge(edge.source, edge.target, {
        weight: w,
        type: 'curved',
      });
    }

    const sigma = new Sigma(graph, container, {
      renderLabels: true,
      labelRenderedSizeThreshold: 7,
      labelSize: 12,
      labelColor: { color: theme.nodeLabelColor },
      labelFont: 'system-ui, -apple-system, sans-serif',
      defaultEdgeColor: '#333',
      defaultNodeColor: '#555',
      defaultEdgeType: 'curved',
      edgeProgramClasses: {
        curved: EdgeCurveProgram,
      },
      minCameraRatio: 0.01,
      maxCameraRatio: 10,
      stagePadding: 40,
      defaultDrawNodeHover: (context, data, settings) => {
        const size = data.size || 4;
        const label = (data as any).fullLabel || data.label || '';
        const font = `${settings.labelFont || 'sans-serif'}`;
        const fontSize = 13;
        context.font = `${fontSize}px ${font}`;
        const textWidth = context.measureText(label).width;
        const padding = 6;
        const boxW = textWidth + padding * 2;
        const boxH = fontSize + padding * 2;
        const x = data.x + size + 4;
        const y = data.y - boxH / 2;

        // Dark pill background
        context.fillStyle = 'rgba(20, 20, 20, 0.92)';
        context.beginPath();
        context.roundRect(x, y, boxW, boxH, 4);
        context.fill();
        context.strokeStyle = 'rgba(255, 255, 255, 0.1)';
        context.lineWidth = 0.5;
        context.stroke();

        // Text
        context.fillStyle = '#d0d0d0';
        context.textAlign = 'left';
        context.textBaseline = 'middle';
        context.fillText(label, x + padding, data.y);

        // Highlight ring on the node
        context.beginPath();
        context.arc(data.x, data.y, size + 2, 0, Math.PI * 2);
        context.strokeStyle = 'rgba(255, 255, 255, 0.3)';
        context.lineWidth = 1.5;
        context.stroke();
      },
      nodeReducer: (_node, attrs) => {
        const tagId = selectedTagRef.current;
        if (!tagId) return attrs;
        const tagIds = (attrs as any).tagIds as string[] | undefined;
        const matches = tagIds?.includes(tagId);
        if (matches) return attrs;
        return {
          ...attrs,
          color: 'rgba(50, 50, 50, 0.3)',
          size: (attrs.size || 4) * 0.6,
          label: '',
        };
      },
      edgeReducer: (_edge, attrs) => {
        const w = (attrs as any).weight ?? 0.5;
        if (w < edgeThresholdRef.current) {
          return { ...attrs, hidden: true };
        }
        const t = themeRef.current;
        const anim = edgeAnimProgress.current;
        return {
          ...attrs,
          color: edgeColor(t, w * anim),
          size: (0.2 + w * 0.7) * anim,
        };
      },
    });

    sigmaRef.current = sigma;

    // Cluster labels canvas
    const labelCanvas = document.createElement('canvas');
    labelCanvas.style.position = 'absolute';
    labelCanvas.style.inset = '0';
    labelCanvas.style.pointerEvents = 'none';
    labelCanvas.style.zIndex = '10';
    container.appendChild(labelCanvas);

    function drawClusterLabels() {
      const width = container!.clientWidth;
      const height = container!.clientHeight;
      const ratio = window.devicePixelRatio || 1;
      labelCanvas.width = width * ratio;
      labelCanvas.height = height * ratio;
      labelCanvas.style.width = `${width}px`;
      labelCanvas.style.height = `${height}px`;

      const ctx = labelCanvas.getContext('2d');
      if (!ctx) return;
      ctx.setTransform(ratio, 0, 0, ratio, 0, 0);
      ctx.clearRect(0, 0, width, height);

      const t = themeRef.current;
      const fontSize = 13;
      ctx.font = `600 ${fontSize}px system-ui, -apple-system, sans-serif`;
      ctx.textAlign = 'center';
      ctx.textBaseline = 'middle';

      const sorted = [...data!.clusters].sort((a, b) => b.atom_count - a.atom_count);
      const placed: { x: number; y: number; w: number; h: number }[] = [];

      // Limit total labels: at most 1 label per 40,000px² of viewport
      const maxLabels = Math.max(4, Math.floor((width * height) / 40000));
      // Minimum gap between labels so they don't pack edge-to-edge
      const pad = 24;

      for (const cluster of sorted) {
        if (placed.length >= maxLabels) break;

        // Compute centroid from actual current node positions
        let cx = 0, cy = 0, count = 0;
        for (const atomId of cluster.atom_ids) {
          if (!graph!.hasNode(atomId)) continue;
          cx += graph!.getNodeAttribute(atomId, 'x') as number;
          cy += graph!.getNodeAttribute(atomId, 'y') as number;
          count++;
        }
        if (count === 0) continue;
        cx /= count;
        cy /= count;
        const pos = sigma!.graphToViewport({ x: cx, y: cy });

        const labelY = pos.y - 20;
        const metrics = ctx.measureText(cluster.label);
        const pillW = metrics.width + 16;
        const pillH = fontSize + 8;
        const rect = {
          x: pos.x - pillW / 2,
          y: labelY - pillH / 2,
          w: pillW,
          h: pillH,
        };

        // Check overlap using padded bounding boxes for spacing
        const overlaps = placed.some(p =>
          rect.x - pad < p.x + p.w &&
          rect.x + rect.w + pad > p.x &&
          rect.y - pad < p.y + p.h &&
          rect.y + rect.h + pad > p.y
        );
        if (overlaps) continue;
        placed.push(rect);

        ctx.fillStyle = t.labelBg;
        ctx.beginPath();
        ctx.roundRect(rect.x, rect.y, pillW, pillH, pillH / 2);
        ctx.fill();
        ctx.strokeStyle = t.labelBorder;
        ctx.lineWidth = 1;
        ctx.stroke();

        ctx.fillStyle = t.labelColor;
        ctx.fillText(cluster.label, pos.x, labelY);
      }
    }

    sigma.on('afterRender', drawClusterLabels);
    requestAnimationFrame(drawClusterLabels);

    // Lock the bounding box to the final layout so Sigma doesn't
    // recompute normalization as nodes move from center outward
    let xMin = Infinity, xMax = -Infinity, yMin = Infinity, yMax = -Infinity;
    for (const pos of Object.values(targetPositions)) {
      if (pos.x < xMin) xMin = pos.x;
      if (pos.x > xMax) xMax = pos.x;
      if (pos.y < yMin) yMin = pos.y;
      if (pos.y > yMax) yMax = pos.y;
    }
    sigma.setCustomBBox({ x: [xMin, xMax], y: [yMin, yMax] });

    // Animate nodes outward from center + fade edges in
    edgeAnimProgress.current = 0;
    const animStart = performance.now();
    let cancelledAnim = false;
    function animateTick(now: number) {
      if (cancelledAnim) return;
      const elapsed = now - animStart;

      // Node positions: 0 → target over 2.5s, cubic ease-out
      const nt = Math.min(1, elapsed / 2000);
      const ne = 1 - (1 - nt) ** 3;
      for (const [id, target] of Object.entries(targetPositions)) {
        if (!graph.hasNode(id)) continue;
        graph.setNodeAttribute(id, 'x', target.x * ne);
        graph.setNodeAttribute(id, 'y', target.y * ne);
      }

      // Edge fade: 0 → 1 over 3s, ease-in
      const et = Math.min(1, elapsed / 2500);
      edgeAnimProgress.current = et * et;

      if (nt < 1 || et < 1) {
        requestAnimationFrame(animateTick);
      }
    }
    requestAnimationFrame(animateTick);
    const cancelAnim = () => { cancelledAnim = true; };

    sigma.on('clickNode', ({ node }) => {
      openDrawer('viewer', node);
    });

    return () => {
      cancelAnim();
      sigma.kill();
      labelCanvas.remove();
      sigmaRef.current = null;
      graphRef.current = null;
    };
  }, [data, openDrawer]); // intentionally exclude theme — handled below

  // Update colors when theme changes (without recreating graph)
  useEffect(() => {
    const graph = graphRef.current;
    const sigma = sigmaRef.current;
    if (!graph || !sigma || !graphDataRef.current) return;

    const { edgeCounts, maxEdges } = graphDataRef.current;

    // Update node colors
    graph.forEachNode((node, attrs) => {
      const connectivity = (edgeCounts.get(node) || 0) / maxEdges;
      graph.setNodeAttribute(node, 'color', nodeColor(theme, connectivity, (attrs as any).clusterIndex));
    });

    // Update sigma label color setting
    sigma.setSetting('labelColor', { color: theme.nodeLabelColor });

    // Edges update via edgeReducer (reads themeRef.current)
    sigma.refresh();
  }, [theme]);

  // Refresh when selected tag changes (nodeReducer reads selectedTagRef)
  useEffect(() => {
    sigmaRef.current?.refresh();
  }, [selectedTagId]);

  // Animate edge threshold changes
  const thresholdAnimRef = useRef<number | null>(null);
  useEffect(() => {
    const sigma = sigmaRef.current;
    if (!sigma) {
      edgeThresholdRef.current = edgeThreshold;
      return;
    }
    const from = edgeThresholdRef.current;
    const to = edgeThreshold;
    if (Math.abs(from - to) < 0.001) return;

    if (thresholdAnimRef.current) cancelAnimationFrame(thresholdAnimRef.current);
    const start = performance.now();
    const duration = 400;
    function tick(now: number) {
      const t = Math.min(1, (now - start) / duration);
      const eased = 1 - (1 - t) ** 2; // ease out quad
      edgeThresholdRef.current = from + (to - from) * eased;
      sigma!.refresh();
      if (t < 1) {
        thresholdAnimRef.current = requestAnimationFrame(tick);
      } else {
        thresholdAnimRef.current = null;
      }
    }
    thresholdAnimRef.current = requestAnimationFrame(tick);
  }, [edgeThreshold]);

  return (
    <div className="flex flex-col h-full w-full">
      <div
        className="flex-1 relative overflow-hidden"
        style={{ backgroundColor: theme.background }}
      >
        {isLoading && (
          <div className="absolute inset-0 flex items-center justify-center z-10">
            <div className="flex items-center gap-2 text-[var(--color-text-secondary)]">
              <svg className="animate-spin h-5 w-5" viewBox="0 0 24 24">
                <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" fill="none" />
                <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
              </svg>
              <span className="text-sm">Computing layout...</span>
            </div>
          </div>
        )}

        {error && (
          <div className="absolute inset-0 flex items-center justify-center">
            <div className="text-center text-[var(--color-text-secondary)]">
              <p className="text-lg mb-2">Error loading canvas</p>
              <p className="text-sm">{error}</p>
            </div>
          </div>
        )}

        {!isLoading && data && data.atoms.length === 0 && (
          <div className="absolute inset-0 flex items-center justify-center">
            <div className="text-center text-[var(--color-text-secondary)]">
              <p className="text-lg mb-2">No atoms with embeddings</p>
              <p className="text-sm">Create some atoms and wait for embeddings to generate</p>
            </div>
          </div>
        )}

        <div
          ref={containerRef}
          className="w-full h-full"
          style={{ minHeight: 200 }}
        />

        {/* Theme picker + edge slider */}
        {!isLoading && data && data.atoms.length > 0 && (
          <div className="absolute bottom-4 left-4 z-20 flex flex-col gap-2">
            <div className="flex items-center gap-1.5">
              <button
                onClick={() => setThemePickerOpen(!themePickerOpen)}
                title="Change theme"
                className="w-6 h-6 rounded-full border border-white/20 hover:border-white/40 transition-all flex-shrink-0"
                style={{
                  background: `linear-gradient(135deg, rgb(${theme.nodeMin.join(',')}), rgb(${theme.nodeMax.join(',')}))`,
                }}
              />
              <div
                className={`flex gap-1.5 overflow-hidden transition-all duration-200 ${
                  themePickerOpen ? 'max-w-[200px] opacity-100' : 'max-w-0 opacity-0'
                }`}
              >
                {CANVAS_THEMES.filter(t => t.id !== theme.id).map((t) => (
                  <button
                    key={t.id}
                    onClick={() => { setTheme(t); setThemePickerOpen(false); }}
                    title={t.name}
                    className="w-5 h-5 rounded-full border border-white/15 hover:border-white/40 transition-all flex-shrink-0"
                    style={{
                      background: `linear-gradient(135deg, rgb(${t.nodeMin.join(',')}), rgb(${t.nodeMax.join(',')}))`,
                    }}
                  />
                ))}
              </div>
            </div>
            <div className="flex items-center gap-1.5">
              <input
                type="range"
                min={0}
                max={100}
                value={(1 - edgeThreshold) * 100}
                onChange={(e) => setEdgeThreshold(1 - Number(e.target.value) / 100)}
                className="w-20 h-1 appearance-none bg-white/10 rounded-full cursor-pointer
                  [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:w-2.5 [&::-webkit-slider-thumb]:h-2.5
                  [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-white/60"
                title={`Edges: ${Math.round((1 - edgeThreshold) * 100)}%`}
              />
              <span className="text-[9px] text-white/30">
                {Math.round((1 - edgeThreshold) * 100)}%
              </span>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
