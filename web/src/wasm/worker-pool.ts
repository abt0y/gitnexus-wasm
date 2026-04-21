/**
 * worker-pool.ts — Parallel parser worker pool (Task 2)
 *
 * Spawns up to `hardwareConcurrency` Web Workers, each running
 * parser-worker.js. Files are dispatched round-robin; crashes are
 * auto-recovered and the failed job is re-queued.
 */

export interface ParsedFile {
  file_path: string;
  language:  string;
  symbols:   Symbol[];
  imports:   Import[];
  calls:     CallSite[];
}

export interface Symbol {
  id:         string;
  name:       string;
  kind:       string;
  file_path:  string;
  start_line: number;
  end_line:   number;
  content?:   string;
}

export interface Import   { source: string; line: number; }
export interface CallSite { target: string; line: number; }

interface PendingJob {
  id:       number;
  filePath: string;
  content:  string;
  language: string;
  resolve:  (r: ParsedFile) => void;
  reject:   (e: Error) => void;
  workerIdx: number;
  timeoutHandle: ReturnType<typeof setTimeout>;
}

const FILE_TIMEOUT_MS = 30_000;
const WORKER_URL      = '/workers/parser-worker.js';

function detectLanguage(path: string): string | null {
  const ext = path.split('.').pop()?.toLowerCase() ?? '';
  const map: Record<string, string> = {
    ts: 'typescript', tsx: 'typescript', mts: 'typescript', cts: 'typescript',
    js: 'javascript', jsx: 'javascript', mjs: 'javascript', cjs: 'javascript',
    py: 'python',  pyi: 'python',
    go: 'go',
    rs: 'rust',
    java: 'java',
    c: 'c',    h: 'c',
    cpp: 'cpp', cc: 'cpp', cxx: 'cpp', hpp: 'cpp',
    cs: 'csharp',
    php: 'php',
    swift: 'swift',
    rb: 'ruby',
  };
  return map[ext] ?? null;
}

export class ParserWorkerPool {
  private workers:   Worker[]      = [];
  private pending:   Map<number, PendingJob> = new Map();
  private jobIdCtr = 0;
  private maxWorkers: number;

  constructor(maxWorkers?: number) {
    this.maxWorkers = maxWorkers ?? Math.max(2, navigator.hardwareConcurrency ?? 4);
  }

  async init(): Promise<void> {
    for (let i = 0; i < this.maxWorkers; i++) {
      this.spawnWorker(i);
    }
  }

  /** Parse an array of {path, content} file descriptors in parallel. */
  async parseBatch(
    files: Array<{ path: string; content: string }>,
    onProgress?: (done: number, total: number) => void,
  ): Promise<ParsedFile[]> {
    const eligible = files.filter(f => detectLanguage(f.path));
    const total    = eligible.length;
    let   done     = 0;

    const promises = eligible.map((file, i) => {
      const lang = detectLanguage(file.path)!;
      return this.dispatch(file.path, file.content, lang, i % this.maxWorkers)
        .then(r => { onProgress?.(++done, total); return r; })
        .catch(err => {
          console.warn(`[worker-pool] skipping ${file.path}: ${err.message}`);
          onProgress?.(++done, total);
          return null;
        });
    });

    const results = await Promise.all(promises);
    return results.filter((r): r is ParsedFile => r !== null);
  }

  terminate(): void {
    for (const job of this.pending.values()) {
      clearTimeout(job.timeoutHandle);
      job.reject(new Error('Worker pool terminated'));
    }
    this.pending.clear();
    this.workers.forEach(w => w.terminate());
    this.workers = [];
  }

  // ---- private ---------------------------------------------------------------

  private dispatch(
    filePath: string,
    content:  string,
    language: string,
    workerIdx: number,
  ): Promise<ParsedFile> {
    return new Promise<ParsedFile>((resolve, reject) => {
      const id = ++this.jobIdCtr;

      const timeoutHandle = setTimeout(() => {
        if (this.pending.has(id)) {
          this.pending.delete(id);
          reject(new Error(`Timeout parsing ${filePath}`));
        }
      }, FILE_TIMEOUT_MS);

      const job: PendingJob = {
        id, filePath, content, language,
        resolve, reject, workerIdx, timeoutHandle,
      };
      this.pending.set(id, job);

      const worker = this.workers[workerIdx];
      if (worker) {
        worker.postMessage({ id, filePath, content, language });
      } else {
        clearTimeout(timeoutHandle);
        this.pending.delete(id);
        reject(new Error(`Worker ${workerIdx} not available`));
      }
    });
  }

  private spawnWorker(idx: number): void {
    const worker = new Worker(WORKER_URL);

    worker.onmessage = (e: MessageEvent) => {
      const { id, success, result, error } = e.data;
      const job = this.pending.get(id);
      if (!job) return;

      clearTimeout(job.timeoutHandle);
      this.pending.delete(id);

      if (success) {
        job.resolve(result as ParsedFile);
      } else {
        job.reject(new Error(error ?? 'Unknown worker error'));
      }
    };

    worker.onerror = (err) => {
      console.error(`[worker-pool] Worker ${idx} crashed:`, err);
      // Re-queue jobs that were assigned to this worker
      for (const [id, job] of this.pending) {
        if (job.workerIdx === idx) {
          clearTimeout(job.timeoutHandle);
          this.pending.delete(id);
          // Retry on a different worker
          const nextIdx = (idx + 1) % this.maxWorkers;
          this.dispatch(job.filePath, job.content, job.language, nextIdx)
            .then(job.resolve)
            .catch(job.reject);
        }
      }
      // Respawn
      this.spawnWorker(idx);
    };

    this.workers[idx] = worker;
  }
}
