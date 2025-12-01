import { useEffect, useRef, useState } from 'react';
import * as d3 from 'd3-force';
import { AtomWithTags } from '../../stores/atoms';
import { Connection } from './ConnectionLines';

export interface SimulationNode extends d3.SimulationNodeDatum {
  id: string;
  atom: AtomWithTags;
  x: number;
  y: number;
}

interface SimulationLink extends d3.SimulationLinkDatum<SimulationNode> {
  source: string;
  target: string;
  strength: number;
}

interface UseForceSimulationProps {
  atoms: AtomWithTags[];
  embeddings: Map<string, number[]>;
  existingPositions: Map<string, { x: number; y: number }>;
  connections: Connection[];
  enabled: boolean;
  onSimulationEnd?: (nodes: SimulationNode[]) => void;
}

interface UseForceSimulationResult {
  nodes: SimulationNode[];
  isSimulating: boolean;
}

const CANVAS_CENTER = 2500;

export function useForceSimulation({
  atoms,
  embeddings,
  existingPositions,
  connections,
  enabled,
  onSimulationEnd,
}: UseForceSimulationProps): UseForceSimulationResult {
  const [nodes, setNodes] = useState<SimulationNode[]>([]);
  const [isSimulating, setIsSimulating] = useState(false);
  const simulationRef = useRef<d3.Simulation<SimulationNode, undefined> | null>(null);
  const onSimulationEndRef = useRef(onSimulationEnd);

  // Keep callback ref updated
  useEffect(() => {
    onSimulationEndRef.current = onSimulationEnd;
  }, [onSimulationEnd]);

  // Initialize nodes when atoms change
  useEffect(() => {
    if (!enabled || atoms.length === 0) {
      setNodes([]);
      return;
    }

    // Check if we need to run simulation
    const atomsWithoutPositions = atoms.filter(
      (atom) => !existingPositions.has(atom.id)
    );

    // If all atoms have positions, just use them
    if (atomsWithoutPositions.length === 0) {
      const initialNodes: SimulationNode[] = atoms.map((atom) => {
        const pos = existingPositions.get(atom.id)!;
        return {
          id: atom.id,
          atom,
          x: pos.x,
          y: pos.y,
        };
      });
      setNodes(initialNodes);
      return;
    }

    // Need to run simulation
    setIsSimulating(true);

    // Initialize nodes with existing positions or random positions
    const initialNodes: SimulationNode[] = atoms.map((atom) => {
      const existingPos = existingPositions.get(atom.id);
      if (existingPos) {
        return {
          id: atom.id,
          atom,
          x: existingPos.x,
          y: existingPos.y,
          fx: existingPos.x, // Fix position for existing nodes initially
          fy: existingPos.y,
        };
      }
      // Random position around center for new nodes
      return {
        id: atom.id,
        atom,
        x: CANVAS_CENTER + (Math.random() - 0.5) * 500,
        y: CANVAS_CENTER + (Math.random() - 0.5) * 500,
      };
    });

    // Build links from connections
    const links: SimulationLink[] = connections.map((conn) => ({
      source: conn.sourceId,
      target: conn.targetId,
      strength: conn.sharedTagCount * 0.1,
    }));

    // Create simulation
    const simulation = d3
      .forceSimulation<SimulationNode>(initialNodes)
      .force('charge', d3.forceManyBody<SimulationNode>().strength(-200))
      .force('collide', d3.forceCollide<SimulationNode>().radius(100))
      .force(
        'link',
        d3
          .forceLink<SimulationNode, SimulationLink>(links)
          .id((d) => d.id)
          .strength((d) => d.strength)
      )
      .force('center', d3.forceCenter(CANVAS_CENTER, CANVAS_CENTER))
      // Similarity force disabled - see commented code below for future re-enablement
      .alpha(1)
      .alphaDecay(0.05) // Faster convergence: ~150 ticks vs ~300 ticks
      .velocityDecay(0.4);

    // After a short time, unfix existing nodes to let them adjust
    setTimeout(() => {
      initialNodes.forEach((node) => {
        node.fx = undefined;
        node.fy = undefined;
      });
      simulation.alpha(0.5).restart();
    }, 500);

    // Throttled tick handler
    let tickCount = 0;
    simulation.on('tick', () => {
      tickCount++;
      // Update less frequently for smoother performance
      // 10 ticks provides good balance between smoothness and performance
      if (tickCount % 10 === 0) {
        setNodes([...initialNodes]);
      }
    });

    simulation.on('end', () => {
      setIsSimulating(false);
      setNodes([...initialNodes]);
      if (onSimulationEndRef.current) {
        onSimulationEndRef.current([...initialNodes]);
      }
    });

    simulationRef.current = simulation;

    return () => {
      simulation.stop();
      simulationRef.current = null;
    };
  }, [atoms, embeddings, existingPositions, connections, enabled]);

  return { nodes, isSimulating };
}

// Helper to build connections from atoms
export function buildConnections(atoms: AtomWithTags[]): Connection[] {
  const connections: Connection[] = [];

  for (let i = 0; i < atoms.length; i++) {
    const tagsA = new Set(atoms[i].tags.map((t) => t.id));

    for (let j = i + 1; j < atoms.length; j++) {
      const sharedCount = atoms[j].tags.filter((t) => tagsA.has(t.id)).length;

      if (sharedCount >= 2) {
        connections.push({
          sourceId: atoms[i].id,
          targetId: atoms[j].id,
          sharedTagCount: sharedCount,
        });
      }
    }
  }

  return connections;
}

