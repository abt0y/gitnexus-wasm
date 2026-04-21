import React, { useState, useEffect } from 'react';
import { X, GitBranch, ArrowLeft, ArrowRight, Activity, Layers } from 'lucide-react';
import { useGitNexusStore } from '../hooks/useStore';

interface ContextPanelProps {
  node: any;
}

export function ContextPanel({ node }: ContextPanelProps) {
  const [context, setContext] = useState<any>(null);
  const [isLoading, setIsLoading] = useState(true);
  const { getContext, setSelectedNode } = useGitNexusStore();

  useEffect(() => {
    setIsLoading(true);
    getContext(node.name || node.id).then((data) => {
      setContext(data);
      setIsLoading(false);
    });
  }, [node, getContext]);

  if (isLoading) {
    return (
      <div className="h-full flex items-center justify-center">
        <div className="animate-pulse text-text-muted">Loading context...</div>
      </div>
    );
  }

  return (
    <div className="h-full flex flex-col">
      <div className="p-4 border-b border-border flex items-center justify-between">
        <h2 className="font-semibold">Context</h2>
        <button
          onClick={() => setSelectedNode(null)}
          className="p-1 rounded-md hover:bg-bg-tertiary"
        >
          <X className="w-4 h-4" />
        </button>
      </div>

      <div className="flex-1 overflow-y-auto p-4 space-y-6">
        {/* Symbol header */}
        <div>
          <div className="flex items-center gap-2 mb-1">
            <span className="px-2 py-0.5 rounded text-xs bg-accent/10 text-accent">
              {node.label || node.type}
            </span>
          </div>
          <h3 className="text-lg font-semibold">{node.name}</h3>
          {node.filePath && (
            <p className="text-sm text-text-muted mt-1">{node.filePath}</p>
          )}
        </div>

        {/* Code preview */}
        {node.content && (
          <div>
            <h4 className="text-sm font-medium text-text-secondary mb-2">Code</h4>
            <pre className="bg-bg-tertiary rounded-lg p-3 text-xs overflow-x-auto max-h-64">
              <code>{node.content.substring(0, 2000)}</code>
            </pre>
          </div>
        )}

        {/* Relationships */}
        {context?.incoming && Object.keys(context.incoming).length > 0 && (
          <div>
            <h4 className="text-sm font-medium text-text-secondary mb-2 flex items-center gap-2">
              <ArrowLeft className="w-3.5 h-3.5" />
              Incoming References
            </h4>
            <div className="space-y-1">
              {Object.entries(context.incoming).slice(0, 10).map(([key, val]: [string, any]) => (
                <div key={key} className="text-sm px-2 py-1.5 rounded bg-bg-tertiary">
                  <span className="text-text-secondary">{val.name || key}</span>
                  <span className="text-text-muted text-xs ml-2">{val.type}</span>
                </div>
              ))}
            </div>
          </div>
        )}

        {context?.outgoing && Object.keys(context.outgoing).length > 0 && (
          <div>
            <h4 className="text-sm font-medium text-text-secondary mb-2 flex items-center gap-2">
              <ArrowRight className="w-3.5 h-3.5" />
              Outgoing References
            </h4>
            <div className="space-y-1">
              {Object.entries(context.outgoing).slice(0, 10).map(([key, val]: [string, any]) => (
                <div key={key} className="text-sm px-2 py-1.5 rounded bg-bg-tertiary">
                  <span className="text-text-secondary">{val.name || key}</span>
                  <span className="text-text-muted text-xs ml-2">{val.type}</span>
                </div>
              ))}
            </div>
          </div>
        )}

        {/* Processes */}
        {context?.processes && context.processes.length > 0 && (
          <div>
            <h4 className="text-sm font-medium text-text-secondary mb-2 flex items-center gap-2">
              <Activity className="w-3.5 h-3.5" />
              Execution Flows
            </h4>
            <div className="space-y-2">
              {context.processes.map((proc: any) => (
                <div key={proc.id} className="p-2 rounded bg-bg-tertiary text-sm">
                  <p className="font-medium">{proc.label}</p>
                  <p className="text-text-muted text-xs">Step {proc.step} of {proc.stepCount}</p>
                </div>
              ))}
            </div>
          </div>
        )}

        {/* Impact analysis button */}
        <button
          onClick={() => {
            const { getImpact } = useGitNexusStore.getState();
            getImpact(node.id, 'upstream');
          }}
          className="w-full py-2 rounded-lg bg-accent/10 text-accent text-sm font-medium hover:bg-accent/20 transition-colors"
        >
          Analyze Impact
        </button>
      </div>
    </div>
  );
}
