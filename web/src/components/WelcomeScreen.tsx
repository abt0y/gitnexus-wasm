import React, { useState, useEffect } from 'react';
import { FolderOpen, FileUp, Github, Sparkles, Loader2 } from 'lucide-react';
import { useGitNexusStore } from '../hooks/useStore';
import { initGitNexus } from '../wasm/bridge';

export function WelcomeScreen() {
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const { setInitialized, setEngine } = useGitNexusStore();

  useEffect(() => {
    initGitNexus()
      .then((engine) => {
        setEngine(engine);
        setInitialized(true);
        setIsLoading(false);
      })
      .catch((err) => {
        setError(err.message);
        setIsLoading(false);
      });
  }, [setInitialized, setEngine]);

  const handleDrop = (e: React.DragEvent) => {
    e.preventDefault();
    const files = e.dataTransfer.files;
    if (files.length > 0) {
      const { importFiles } = useGitNexusStore.getState();
      importFiles(files);
    }
  };

  return (
    <div className="min-h-screen flex items-center justify-center bg-bg-primary">
      <div className="max-w-2xl w-full mx-4">
        <div className="text-center mb-12">
          <div className="inline-flex items-center justify-center w-16 h-16 rounded-2xl bg-accent/10 mb-6">
            <Sparkles className="w-8 h-8 text-accent" />
          </div>
          <h1 className="text-4xl font-bold mb-3">GitNexus</h1>
          <p className="text-text-secondary text-lg">
            AI-powered code intelligence — entirely in your browser
          </p>
          <div className="mt-4 inline-flex items-center gap-2 px-3 py-1 rounded-full bg-success/10 text-success text-sm">
            <span className="w-2 h-2 rounded-full bg-success animate-pulse" />
            Zero-server · Zero-install · 100% private
          </div>
        </div>

        {isLoading ? (
          <div className="flex flex-col items-center gap-3 py-12">
            <Loader2 className="w-8 h-8 text-accent animate-spin" />
            <p className="text-text-secondary">Loading WASM runtime...</p>
          </div>
        ) : error ? (
          <div className="bg-danger/10 border border-danger/20 rounded-xl p-6 text-center">
            <p className="text-danger font-medium mb-2">Initialization Failed</p>
            <p className="text-text-secondary text-sm">{error}</p>
          </div>
        ) : (
          <div
            onDrop={handleDrop}
            onDragOver={(e) => e.preventDefault()}
            className="space-y-4"
          >
            {/* Import options */}
            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
              <ImportCard
                icon={<FolderOpen className="w-6 h-6" />}
                title="Open Directory"
                description="Select a local folder with your code"
                onClick={() => {
                  const { importDirectory } = useGitNexusStore.getState();
                  importDirectory();
                }}
              />
              <ImportCard
                icon={<FileUp className="w-6 h-6" />}
                title="Drop Files"
                description="Drag & drop files or folders here"
                onClick={() => {}}
                highlight
              />
            </div>

            {/* Supported languages */}
            <div className="mt-8 pt-8 border-t border-border">
              <p className="text-text-muted text-sm text-center mb-4">
                Supported languages
              </p>
              <div className="flex flex-wrap justify-center gap-2">
                {['TypeScript', 'JavaScript', 'Python', 'Go', 'Rust', 'Java', 'C/C++', 'C#', 'PHP', 'Swift', 'Ruby'].map((lang) => (
                  <span key={lang} className="px-2 py-1 rounded-md bg-bg-secondary text-text-secondary text-xs">
                    {lang}
                  </span>
                ))}
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

function ImportCard({ icon, title, description, onClick, highlight }: {
  icon: React.ReactNode;
  title: string;
  description: string;
  onClick: () => void;
  highlight?: boolean;
}) {
  return (
    <button
      onClick={onClick}
      className={`w-full p-6 rounded-xl border text-left transition-all hover:scale-[1.02] ${
        highlight
          ? 'border-accent/30 bg-accent/5 hover:bg-accent/10'
          : 'border-border bg-bg-secondary hover:bg-bg-tertiary'
      }`}
    >
      <div className={`mb-3 ${highlight ? 'text-accent' : 'text-text-secondary'}`}>
        {icon}
      </div>
      <h3 className="font-semibold mb-1">{title}</h3>
      <p className="text-sm text-text-muted">{description}</p>
    </button>
  );
}
