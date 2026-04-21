import React, { useState, useCallback } from 'react';
import { useGitNexusStore } from './hooks/useStore';
import { Header } from './components/Header';
import { FileTree } from './components/FileTree';
import { GraphView } from './components/GraphView';
import { SearchPanel } from './components/SearchPanel';
import { ContextPanel } from './components/ContextPanel';
import { ProgressModal } from './components/ProgressModal';
import { WelcomeScreen } from './components/WelcomeScreen';

function App() {
  const { 
    isInitialized, 
    isAnalyzing, 
    currentRepo, 
    graphData,
    selectedNode,
    setSelectedNode,
  } = useGitNexusStore();

  const [activePanel, setActivePanel] = useState<'search' | 'context' | null>(null);

  const handleNodeClick = useCallback((node: any) => {
    setSelectedNode(node);
    setActivePanel('context');
  }, [setSelectedNode]);

  if (!isInitialized) {
    return <WelcomeScreen />;
  }

  return (
    <div className="h-screen flex flex-col bg-bg-primary text-text-primary overflow-hidden">
      <Header 
        onSearch={() => setActivePanel(activePanel === 'search' ? null : 'search')}
        repoName={currentRepo?.name}
      />

      {isAnalyzing && <ProgressModal />}

      <div className="flex-1 flex overflow-hidden">
        {/* Left sidebar - File tree */}
        <div className="w-64 border-r border-border bg-bg-secondary flex-shrink-0 overflow-y-auto">
          <FileTree />
        </div>

        {/* Main content - Graph */}
        <div className="flex-1 relative">
          {graphData ? (
            <GraphView 
              data={graphData} 
              onNodeClick={handleNodeClick}
              selectedNode={selectedNode}
            />
          ) : (
            <div className="h-full flex items-center justify-center text-text-secondary">
              <div className="text-center">
                <p className="text-lg mb-2">No graph data yet</p>
                <p className="text-sm text-text-muted">Import a repository to get started</p>
              </div>
            </div>
          )}
        </div>

        {/* Right panel - Search / Context */}
        {activePanel && (
          <div className="w-96 border-l border-border bg-bg-secondary flex-shrink-0 overflow-y-auto fade-in">
            {activePanel === 'search' && <SearchPanel />}
            {activePanel === 'context' && selectedNode && <ContextPanel node={selectedNode} />}
          </div>
        )}
      </div>
    </div>
  );
}

export default App;
