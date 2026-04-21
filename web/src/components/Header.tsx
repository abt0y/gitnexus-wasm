import React from 'react';
import { Search, Settings, GitBranch, Database } from 'lucide-react';
import { useGitNexusStore } from '../hooks/useStore';

interface HeaderProps {
  onSearch: () => void;
  repoName?: string;
}

export function Header({ onSearch, repoName }: HeaderProps) {
  const { currentRepo, isAnalyzing, analyzeRepo, graphData } = useGitNexusStore();

  return (
    <header className="h-14 border-b border-border bg-bg-secondary flex items-center px-4 gap-4">
      <div className="flex items-center gap-2">
        <div className="w-8 h-8 rounded-lg bg-accent/10 flex items-center justify-center">
          <Database className="w-4 h-4 text-accent" />
        </div>
        <span className="font-semibold">GitNexus</span>
      </div>

      {repoName && (
        <div className="flex items-center gap-2 px-3 py-1 rounded-md bg-bg-tertiary text-sm">
          <GitBranch className="w-3.5 h-3.5 text-text-muted" />
          <span className="text-text-secondary">{repoName}</span>
        </div>
      )}

      <div className="flex-1" />

      <div className="flex items-center gap-2">
        {currentRepo && !isAnalyzing && !graphData && (
          <button
            onClick={analyzeRepo}
            className="px-3 py-1.5 rounded-md bg-accent text-white text-sm font-medium hover:bg-accent-hover transition-colors"
          >
            Analyze
          </button>
        )}

        <button
          onClick={onSearch}
          className="p-2 rounded-md hover:bg-bg-tertiary transition-colors"
          title="Search"
        >
          <Search className="w-4 h-4 text-text-secondary" />
        </button>

        <button className="p-2 rounded-md hover:bg-bg-tertiary transition-colors" title="Settings">
          <Settings className="w-4 h-4 text-text-secondary" />
        </button>
      </div>
    </header>
  );
}
