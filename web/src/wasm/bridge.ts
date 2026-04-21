/**
 * bridge.ts — WASM module loader & Pipeline Orchestrator
 *
 * This version uses the modularized GitNexus API to support:
 * 1. Parallel Parsing via Web Workers (Task 2)
 * 2. Incremental Updates via Hashing (Task 7)
 * 3. Community Detection & Process Extraction (Tasks 3 & 4)
 */

import { loadKuzu } from './kuzu-loader';
import { loadOnnx  } from './onnx-loader';
import { ParserWorkerPool } from './worker-pool';

let _instance: any = null;
let _workerPool: ParserWorkerPool | null = null;

export async function initGitNexus(): Promise<any> {
  if (_instance) return _instance;

  // 1. KuzuDB must be in window.kuzu
  await loadKuzu();

  // 2. Load Rust WASM module
  // @ts-ignore
  const wasm = await import('../../pkg/gitnexus_core');
  await wasm.default();

  // 3. Construct and initialise engine
  const engine = new wasm.GitNexus();
  const initResult = await engine.init();
  const data = JSON.parse(initResult.data ?? '{}');

  if (!initResult.success) {
    throw new Error(data.error ?? 'GitNexus init failed');
  }

  // 4. Initialize Parser Worker Pool
  _workerPool = new ParserWorkerPool();
  await _workerPool.init();

  console.log('[bridge] GitNexus & WorkerPool ready:', data);
  _instance = engine;
  return engine;
}

export interface AnalysisProgress {
  phase: string;
  percent: number;
  message: string;
  stats?: {
    filesProcessed: number;
    totalFiles: number;
  };
}

/**
 * Orchestrate the full analysis pipeline
 */
export async function runFullAnalysis(
  onProgress: (p: AnalysisProgress) => void
): Promise<void> {
  const engine = _instance;
  if (!engine) throw new Error('GitNexus not initialised');
  if (!_workerPool) throw new Error('Worker pool not initialised');

  try {
    // Phase 1: Parallel Parsing
    onProgress({ phase: 'parsing', percent: 0, message: 'Detecting files...' });
    const filesToParse = engine.get_files_for_parsing();
    
    onProgress({ phase: 'parsing', percent: 5, message: 'Parsing files in parallel...' });
    const parsedResults = await _workerPool.parseBatch(filesToParse, (done, total) => {
      onProgress({ 
        phase: 'parsing', 
        percent: 5 + Math.floor((done / total) * 45), 
        message: `Parsed ${done}/${total} files...` 
      });
    });

    // Phase 2: Graph Ingestion
    onProgress({ phase: 'graph', percent: 50, message: 'Ingesting symbols into graph...' });
    const ingestResult = await engine.ingest_parsed_results(parsedResults);
    if (!ingestResult.success) throw new Error('Graph ingestion failed');

    // Phase 3: Community Detection (Louvain)
    onProgress({ phase: 'communities', percent: 70, message: 'Detecting code communities...' });
    await engine.run_community_detection({ resolution: 1.0, minCommunitySize: 3 });

    // Phase 4: Process Extraction (BFS)
    onProgress({ phase: 'processes', percent: 90, message: 'Extracting execution flows...' });
    await engine.run_process_extraction({ maxDepth: 10, minConfidence: 0.5 });

    onProgress({ phase: 'complete', percent: 100, message: 'Analysis complete!' });
  } catch (err) {
    console.error('[bridge] Analysis failed:', err);
    throw err;
  }
}

export async function initEmbeddings(modelUrl?: string): Promise<void> {
  const engine = _instance;
  if (!engine) throw new Error('GitNexus not initialised');

  await loadOnnx();
  const result = await engine.init_embeddings(modelUrl ?? null);
  if (!result.success) {
    throw new Error(JSON.parse(result.data).error ?? 'Embedding init failed');
  }
}

export function getGitNexus(): any {
  return _instance;
}
