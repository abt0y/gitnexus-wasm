import React, { useRef, useEffect, useState } from 'react';
import ForceGraph2D from 'react-force-graph-2d';
import { useGitNexusStore } from '../hooks/useStore';

interface GraphViewProps {
  data: any;
  onNodeClick: (node: any) => void;
  selectedNode: any;
}

export function GraphView({ data, onNodeClick, selectedNode }: GraphViewProps) {
  const fgRef = useRef<any>(null);
  const [hoverNode, setHoverNode] = useState<any>(null);

  // Transform graph data for force-graph
  const graphData = React.useMemo(() => {
    if (!data) return { nodes: [], links: [] };

    const nodes = data.nodes?.map((n: any) => ({
      id: n.id,
      label: n.label,
      name: n.properties?.name || n.id,
      ...n.properties,
      val: getNodeSize(n.label),
      color: getNodeColor(n.label),
    })) || [];

    const links = data.relationships?.map((r: any) => ({
      source: r.sourceId,
      target: r.targetId,
      type: r.type,
    })) || [];

    return { nodes, links };
  }, [data]);

  useEffect(() => {
    if (fgRef.current && graphData.nodes.length > 0) {
      fgRef.current.zoomToFit(400);
    }
  }, [graphData]);

  if (!data) return null;

  return (
    <div className="w-full h-full">
      <ForceGraph2D
        ref={fgRef}
        graphData={graphData}
        nodeLabel={(node: any) => `${node.name} (${node.label})`}
        nodeColor={(node: any) => node.color}
        nodeVal={(node: any) => node.val}
        linkColor={() => '#475569'}
        linkWidth={1}
        linkDirectionalArrowLength={6}
        linkDirectionalArrowRelPos={1}
        onNodeClick={(node: any) => onNodeClick(node)}
        onNodeHover={(node: any) => setHoverNode(node)}
        backgroundColor="#0f172a"
        warmupTicks={100}
        cooldownTicks={50}
        width={undefined}
        height={undefined}
      />

      {hoverNode && (
        <div className="absolute bottom-4 left-4 bg-bg-secondary border border-border rounded-lg p-3 text-sm">
          <p className="font-medium">{hoverNode.name}</p>
          <p className="text-text-muted">{hoverNode.label}</p>
          {hoverNode.filePath && (
            <p className="text-text-muted text-xs mt-1">{hoverNode.filePath}</p>
          )}
        </div>
      )}
    </div>
  );
}

function getNodeSize(label: string): number {
  const sizes: Record<string, number> = {
    'File': 3,
    'Folder': 4,
    'Function': 2,
    'Class': 5,
    'Interface': 4,
    'Method': 2,
    'Community': 8,
    'Process': 6,
  };
  return sizes[label] || 2;
}

function getNodeColor(label: string): string {
  const colors: Record<string, string> = {
    'File': '#60a5fa',
    'Folder': '#94a3b8',
    'Function': '#34d399',
    'Class': '#f472b6',
    'Interface': '#a78bfa',
    'Method': '#2dd4bf',
    'Struct': '#fbbf24',
    'Enum': '#fb923c',
    'Community': '#ef4444',
    'Process': '#8b5cf6',
  };
  return colors[label] || '#94a3b8';
}
