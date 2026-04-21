/**
 * useStore.ts — Zustand global store
 *
 * This version delegates the analysis orchestration to bridge.ts
 */

import { create } from 'zustand';
import { runFullAnalysis, initGitNexus, getGitNexus } from '../wasm/bridge';
import {
  saveRepoMeta, loadRepoMeta, saveGraphSnapshot, loadGraphSnapshot,
  RepoMeta, GraphSnapshot,
} from '../wasm/persistence';

// ── Types ────────────────────────────────────────────────────────────────────

export interface FileEntry {
  path:        string;
  name:        string;
  isDirectory: boolean;
  content?:    string;
  size?:       number;
}

export interface RepoState {
  name:      string;
  path:      string;
  files:     FileEntry[];
  isGitRepo: boolean;
  branch?:   string;
}

export interface ProgressState {
  phase:   string;
  percent: number;
  message: string;
  stats?: {
    filesProcessed: number;
    totalFiles: number;
  };
}

// ── Store interface ──────────────────────────────────────────────────────────

interface GitNexusState {
  isInitialized: boolean;
  isAnalyzing:   boolean;
  currentRepo:  RepoState | null;
  graphData:    any | null;
  selectedNode: any | null;
  progress: ProgressState | null;

  setInitialized: (v: boolean)          => void;
  setRepo:        (r: RepoState | null) => void;
  setGraphData:   (d: any | null)       => void;
  setSelectedNode:(n: any | null)       => void;
  setAnalyzing:   (v: boolean)          => void;
  setProgress:    (p: ProgressState | null) => void;

  importDirectory:  ()                                => Promise<void>;
  analyzeRepo:      ()                                => Promise<void>;
  search:           (query: string)                   => Promise<any[]>;
  getContext:       (name: string)                    => Promise<any>;
  saveState:        ()                                => Promise<void>;
  loadState:        (repoName: string)                => Promise<boolean>;
}

export const useGitNexusStore = create<GitNexusState>((set, get) => ({
  isInitialized: false,
  isAnalyzing:   false,
  currentRepo:   null,
  graphData:     null,
  selectedNode:  null,
  progress:      null,

  setInitialized:  (v) => set({ isInitialized: v }),
  setRepo:         (r) => set({ currentRepo: r }),
  setGraphData:    (d) => set({ graphData: d }),
  setSelectedNode: (n) => set({ selectedNode: n }),
  setAnalyzing:    (v) => set({ isAnalyzing: v }),
  setProgress:     (p) => set({ progress: p }),

  importDirectory: async () => {
    const engine = await initGitNexus();
    try {
      const dirHandle = await (window as any).showDirectoryPicker();
      const result    = await engine.import_from_handle(dirHandle);
      const data = JSON.parse(result.data);
      
      set({
        currentRepo: {
          name:      data.name,
          path:      '/',
          files:     [],
          isGitRepo: data.isGitRepo,
        },
      });
    } catch (err) {
      console.error('Import failed:', err);
    }
  },

  analyzeRepo: async () => {
    const { isAnalyzing } = get();
    if (isAnalyzing) return;

    set({ isAnalyzing: true });
    try {
      await runFullAnalysis((progress) => {
        set({ progress });
      });

      const engine = getGitNexus();
      const graphData = JSON.parse(await engine.export_graph());
      set({ graphData });

      await get().saveState();
    } catch (err) {
      console.error('Analysis failed:', err);
      set({ progress: { phase: 'error', percent: 0, message: `Error: ${err}` } });
    } finally {
      set({ isAnalyzing: false });
    }
  },

  search: async (query: string) => {
    const engine = getGitNexus();
    if (!engine) return [];
    try {
      const result = await engine.search({ query, semantic: false });
      return JSON.parse(result.data);
    } catch (err) { return []; }
  },

  getContext: async (name: string) => {
    const engine = getGitNexus();
    if (!engine) return null;
    try {
      const result = await engine.context(name, null);
      if (result.success) return JSON.parse(result.data);
    } catch (err) {}
    return null;
  },

  saveState: async () => {
    const { currentRepo, graphData } = get();
    if (!currentRepo || !graphData) return;
    try {
      const meta: RepoMeta = {
        name:         currentRepo.name,
        importedAt:   new Date().toISOString(),
        lastAnalyzed: new Date().toISOString(),
        fileHashes:   {},
        fileCount:    0,
        isGitRepo:    currentRepo.isGitRepo,
      };
      await saveRepoMeta(meta);
      await saveGraphSnapshot({
        repoName: currentRepo.name,
        savedAt: new Date().toISOString(),
        graphJson: JSON.stringify(graphData),
        embeddingCount: 0,
      });
    } catch (err) {}
  },

  loadState: async (repoName: string) => {
    try {
      const snapshot = await loadGraphSnapshot(repoName);
      if (!snapshot) return false;
      const meta = await loadRepoMeta(repoName);
      set({
        graphData: JSON.parse(snapshot.graphJson),
        currentRepo: meta ? { name: meta.name, path: '/', files: [], isGitRepo: meta.isGitRepo } : null,
      });
      return true;
    } catch (err) { return false; }
  },
}));
