/**
 * KuzuDB WASM loader
 * Loads KuzuDB into the global scope so the Rust WASM bridge can find `window.kuzu`.
 * Referenced as a manual chunk in vite.config.ts for code-splitting.
 */

const KUZU_CDN = 'https://cdn.jsdelivr.net/npm/kuzu-wasm@0.6.0/browser/kuzu.js';

let loadPromise: Promise<void> | null = null;

export async function loadKuzu(): Promise<void> {
  if ((window as any).kuzu) return;
  if (loadPromise) return loadPromise;

  loadPromise = new Promise<void>((resolve, reject) => {
    const script = document.createElement('script');
    script.src = KUZU_CDN;
    script.crossOrigin = 'anonymous';
    script.onload = () => {
      console.log('[kuzu-loader] KuzuDB loaded');
      resolve();
    };
    script.onerror = () => reject(new Error(`Failed to load KuzuDB from ${KUZU_CDN}`));
    document.head.appendChild(script);
  });

  return loadPromise;
}

export function isKuzuLoaded(): boolean {
  return !!(window as any).kuzu;
}
