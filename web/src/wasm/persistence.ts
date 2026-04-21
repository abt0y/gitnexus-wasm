/**
 * persistence.ts — IndexedDB state management (Task 7)
 *
 * Persists repo metadata and graph snapshots so that reopening the tab
 * restores the previous analysis without a full re-parse.
 *
 * Keys:
 *   repos   → RepoMeta  (lightweight – paths, hashes, dates)
 *   graphs  → { repoName, json }  (full graph JSON export)
 */

const DB_NAME    = 'gitnexus-v1';
const DB_VERSION = 1;

// ── Types ────────────────────────────────────────────────────────────────────

export interface RepoMeta {
  /** Unique key – use the repo directory name */
  name:         string;
  importedAt:   string;          // ISO-8601
  lastAnalyzed: string;          // ISO-8601
  /** path → content hash (FNV-1a hex from Rust) */
  fileHashes:   Record<string, string>;
  fileCount:    number;
  isGitRepo:    boolean;
}

export interface GraphSnapshot {
  repoName:    string;
  savedAt:     string;
  /** Full JSON string produced by GraphDatabase.export() */
  graphJson:   string;
  /** Embedding count at time of save */
  embeddingCount: number;
}

// ── DB handle ────────────────────────────────────────────────────────────────

let _db: IDBDatabase | null = null;

export async function openDB(): Promise<IDBDatabase> {
  if (_db) return _db;

  return new Promise<IDBDatabase>((resolve, reject) => {
    const req = indexedDB.open(DB_NAME, DB_VERSION);

    req.onerror = () => reject(req.error);

    req.onsuccess = () => {
      _db = req.result;
      resolve(_db);
    };

    req.onupgradeneeded = (event) => {
      const db = (event.target as IDBOpenDBRequest).result;

      if (!db.objectStoreNames.contains('repos')) {
        db.createObjectStore('repos', { keyPath: 'name' });
      }
      if (!db.objectStoreNames.contains('graphs')) {
        db.createObjectStore('graphs', { keyPath: 'repoName' });
      }
    };
  });
}

// ── Repo metadata ─────────────────────────────────────────────────────────────

export async function saveRepoMeta(meta: RepoMeta): Promise<void> {
  const db = await openDB();
  return idbPut(db, 'repos', meta);
}

export async function loadRepoMeta(name: string): Promise<RepoMeta | null> {
  const db = await openDB();
  return idbGet<RepoMeta>(db, 'repos', name);
}

export async function listRepos(): Promise<RepoMeta[]> {
  const db = await openDB();
  return idbGetAll<RepoMeta>(db, 'repos');
}

export async function deleteRepoMeta(name: string): Promise<void> {
  const db = await openDB();
  return idbDelete(db, 'repos', name);
}

// ── Graph snapshots ───────────────────────────────────────────────────────────

export async function saveGraphSnapshot(snapshot: GraphSnapshot): Promise<void> {
  const db = await openDB();
  return idbPut(db, 'graphs', snapshot);
}

export async function loadGraphSnapshot(repoName: string): Promise<GraphSnapshot | null> {
  const db = await openDB();
  return idbGet<GraphSnapshot>(db, 'graphs', repoName);
}

export async function deleteGraphSnapshot(repoName: string): Promise<void> {
  const db = await openDB();
  return idbDelete(db, 'graphs', repoName);
}

// ── High-level helpers ─────────────────────────────────────────────────────────

/**
 * Compute a deterministic hash for a file (mirrors Rust `hash_file`).
 * Uses a simple FNV-1a 64-bit hash — cheap, no SubtleCrypto needed.
 */
export function hashFile(path: string, content: string): string {
  const input = `${path}\x00${content}`;
  let hi = 0xcbf29ce4;
  let lo = 0x84222325;
  for (let i = 0; i < input.length; i++) {
    const b = input.charCodeAt(i) & 0xff;
    lo ^= b;
    // 64-bit FNV multiply split into 32-bit chunks
    const hi2 = (hi * 0x100 + lo * 0x00000001) >>> 0;
    const lo2 = (lo * 0x100 + (hi & 0xff) * 0x00000001b3 + lo * 0x1b3) >>> 0;
    hi = hi2;
    lo = lo2;
  }
  return hi.toString(16).padStart(8, '0') + lo.toString(16).padStart(8, '0');
}

/**
 * Given the stored RepoMeta for a repo and a fresh set of file entries,
 * returns which files need re-parsing.
 */
export function findChangedFiles(
  stored:   RepoMeta,
  incoming: Array<{ path: string; content: string }>,
): { changed: typeof incoming; unchanged: string[]; deleted: string[] } {
  const changed:   typeof incoming = [];
  const unchanged: string[]        = [];
  const incomingPaths = new Set(incoming.map(f => f.path));

  for (const file of incoming) {
    const newHash = hashFile(file.path, file.content);
    if (stored.fileHashes[file.path] === newHash) {
      unchanged.push(file.path);
    } else {
      changed.push(file);
    }
  }

  const deleted = Object.keys(stored.fileHashes)
    .filter(p => !incomingPaths.has(p));

  return { changed, unchanged, deleted };
}

// ── Low-level IndexedDB helpers ───────────────────────────────────────────────

function idbPut(db: IDBDatabase, store: string, value: unknown): Promise<void> {
  return new Promise((resolve, reject) => {
    const tx  = db.transaction(store, 'readwrite');
    const req = tx.objectStore(store).put(value);
    req.onsuccess = () => resolve();
    req.onerror   = () => reject(req.error);
  });
}

function idbGet<T>(db: IDBDatabase, store: string, key: string): Promise<T | null> {
  return new Promise((resolve, reject) => {
    const tx  = db.transaction(store, 'readonly');
    const req = tx.objectStore(store).get(key);
    req.onsuccess = () => resolve((req.result as T) ?? null);
    req.onerror   = () => reject(req.error);
  });
}

function idbGetAll<T>(db: IDBDatabase, store: string): Promise<T[]> {
  return new Promise((resolve, reject) => {
    const tx  = db.transaction(store, 'readonly');
    const req = tx.objectStore(store).getAll();
    req.onsuccess = () => resolve(req.result as T[]);
    req.onerror   = () => reject(req.error);
  });
}

function idbDelete(db: IDBDatabase, store: string, key: string): Promise<void> {
  return new Promise((resolve, reject) => {
    const tx  = db.transaction(store, 'readwrite');
    const req = tx.objectStore(store).delete(key);
    req.onsuccess = () => resolve();
    req.onerror   = () => reject(req.error);
  });
}
