import { create } from 'zustand';

interface GraphNode {
  id: string;
  label: string;
  properties: Record<string, any>;
}

interface GraphRelationship {
  id: string;
  sourceId: string;
  targetId: string;
  type: string;
}

interface GraphData {
  nodes: GraphNode[];
  relationships: GraphRelationship[];
}

interface FileEntry {
  path: string;
  name: string;
  isDirectory: boolean;
  size?: number;
}

interface RepoState {
  name: string;
  path: string;
  files: FileEntry[];
  isGitRepo: boolean;
}

interface ProgressState {
  phase: string;
  percent: number;
  message: string;
}

interface GitNexusState {
  // Engine state
  isInitialized: boolean;
  isAnalyzing: boolean;
  engine: any | null;

  // Repository
  currentRepo: RepoState | null;

  // Graph
  graphData: GraphData | null;
  selectedNode: GraphNode | null;

  // Progress
  progress: ProgressState | null;

  // Actions
  setInitialized: (value: boolean) => void;
  setEngine: (engine: any) => void;
  setRepo: (repo: RepoState | null) => void;
  setGraphData: (data: GraphData | null) => void;
  setSelectedNode: (node: GraphNode | null) => void;
  setAnalyzing: (value: boolean) => void;
  setProgress: (progress: ProgressState | null) => void;

  // Async actions
  importDirectory: () => Promise<void>;
  importFiles: (files: FileList) => Promise<void>;
  analyzeRepo: () => Promise<void>;
  search: (query: string) => Promise<any[]>;
  getContext: (name: string) => Promise<any>;
  getImpact: (target: string, direction: string) => Promise<any>;
}

export const useGitNexusStore = create<GitNexusState>((set, get) => ({
  isInitialized: false,
  isAnalyzing: false,
  engine: null,
  currentRepo: null,
  graphData: null,
  selectedNode: null,
  progress: null,

  setInitialized: (value) => set({ isInitialized: value }),
  setEngine: (engine) => set({ engine }),
  setRepo: (repo) => set({ currentRepo: repo }),
  setGraphData: (data) => set({ graphData: data }),
  setSelectedNode: (node) => set({ selectedNode: node }),
  setAnalyzing: (value) => set({ isAnalyzing: value }),
  setProgress: (progress) => set({ progress }),

  importDirectory: async () => {
    const { engine } = get();
    if (!engine) return;

    try {
      // Use File System Access API
      const dirHandle = await (window as any).showDirectoryPicker();
      const result = await engine.import_from_handle(dirHandle);

      if (result.success) {
        const data = JSON.parse(result.data);
        set({ 
          currentRepo: {
            name: data.name,
            path: '/',
            files: [], // Would populate from handle
            isGitRepo: data.isGitRepo,
          }
        });
      }
    } catch (err) {
      console.error('Import failed:', err);
    }
  },

  importFiles: async (files: FileList) => {
    const { engine } = get();
    if (!engine) return;

    try {
      const result = await engine.import_from_files(files);
      if (result.success) {
        const data = JSON.parse(result.data);
        set({ 
          currentRepo: {
            name: 'dropped-files',
            path: '/',
            files: Array.from(files).map(f => ({
              path: f.name,
              name: f.name,
              isDirectory: false,
              size: f.size,
            })),
            isGitRepo: false,
          }
        });
      }
    } catch (err) {
      console.error('File import failed:', err);
    }
  },

  analyzeRepo: async () => {
    const { engine } = get();
    if (!engine) return;

    set({ isAnalyzing: true, progress: { phase: 'init', percent: 0, message: 'Starting analysis...' } });

    try {
      const progressCallback = new (window as any).Function('progress', `
        const store = document.__gitnexus_store;
        if (store) store.setProgress(progress);
      `);

      // Attach store reference for callback
      (document as any).__gitnexus_store = { setProgress: (p: any) => set({ progress: p }) };

      const result = await engine.analyze(progressCallback);

      if (result.success) {
        // Fetch graph data
        const graphResult = await engine.export_graph();
        const graphData = JSON.parse(graphResult);
        set({ graphData });
      }
    } catch (err) {
      console.error('Analysis failed:', err);
    } finally {
      set({ isAnalyzing: false });
    }
  },

  search: async (query: string) => {
    const { engine } = get();
    if (!engine) return [];

    try {
      const result = await engine.search({ query, mode: 'hybrid', limit: 20 });
      if (result.success) {
        return JSON.parse(result.data);
      }
    } catch (err) {
      console.error('Search failed:', err);
    }
    return [];
  },

  getContext: async (name: string) => {
    const { engine } = get();
    if (!engine) return null;

    try {
      const result = await engine.context(name, null);
      if (result.success) {
        return JSON.parse(result.data);
      }
    } catch (err) {
      console.error('Context failed:', err);
    }
    return null;
  },

  getImpact: async (target: string, direction: string) => {
    const { engine } = get();
    if (!engine) return null;

    try {
      const result = await engine.impact(target, direction, 3);
      if (result.success) {
        return JSON.parse(result.data);
      }
    } catch (err) {
      console.error('Impact failed:', err);
    }
    return null;
  },
}));
