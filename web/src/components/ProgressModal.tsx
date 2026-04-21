import { Loader2, CheckCircle2 } from 'lucide-react';
import { useGitNexusStore } from '../store/useStore';

export function ProgressModal() {
  const { progress } = useGitNexusStore();

  if (!progress) return null;

  const phases = [
    { id: 'parsing', label: 'Parsing', icon: '📝' },
    { id: 'building_graph', label: 'Building Graph', icon: '🕸️' },
    { id: 'communities', label: 'Communities', icon: '🏘️' },
    { id: 'processes', label: 'Processes', icon: '⚡' },
    { id: 'embeddings', label: 'Embeddings', icon: '🧠' },
    { id: 'complete', label: 'Complete', icon: '✅' },
  ];

  const currentPhaseIndex = phases.findIndex(p => 
    progress.phase.includes(p.id) || (p.id === 'building_graph' && progress.phase === 'building_graph')
  );

  return (
    <div className="absolute inset-0 bg-bg-primary/80 backdrop-blur-sm z-50 flex items-center justify-center">
      <div className="bg-bg-secondary border border-border rounded-2xl p-8 max-w-md w-full mx-4">
        <div className="flex items-center gap-3 mb-6">
          <Loader2 className="w-5 h-5 text-accent animate-spin" />
          <h3 className="font-semibold">Analyzing Repository</h3>
        </div>

        {/* Progress bar */}
        <div className="mb-6">
          <div className="h-2 bg-bg-tertiary rounded-full overflow-hidden">
            <div
              className="h-full bg-accent rounded-full transition-all duration-500"
              style={{ width: `${progress.percent}%` }}
            />
          </div>
          <p className="text-sm text-text-muted mt-2">{progress.message}</p>
        </div>

        {/* Phase indicators */}
        <div className="space-y-2">
          {phases.map((phase, idx) => {
            const isComplete = idx < currentPhaseIndex;
            const isActive = idx === currentPhaseIndex;

            return (
              <div
                key={phase.id}
                className={`flex items-center gap-3 px-3 py-2 rounded-lg text-sm transition-colors ${
                  isActive ? 'bg-accent/10 text-accent' :
                  isComplete ? 'text-success' :
                  'text-text-muted'
                }`}
              >
                {isComplete ? (
                  <CheckCircle2 className="w-4 h-4" />
                ) : (
                  <span>{phase.icon}</span>
                )}
                <span>{phase.label}</span>
                {isActive && progress.stats && (
                  <span className="ml-auto text-xs">
                    {progress.stats.filesProcessed}/{progress.stats.totalFiles} files
                  </span>
                )}
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}
