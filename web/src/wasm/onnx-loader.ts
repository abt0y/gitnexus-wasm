/**
 * ONNX Runtime Web loader
 * Loads ort into the global scope so the Rust WASM bridge can find `window.ort`.
 * Referenced as a manual chunk in vite.config.ts for code-splitting.
 */

const ORT_CDN = 'https://cdn.jsdelivr.net/npm/onnxruntime-web@1.17.0/dist/ort.min.js';

let loadPromise: Promise<void> | null = null;

export async function loadOnnx(): Promise<void> {
  if ((window as any).ort) return;
  if (loadPromise) return loadPromise;

  loadPromise = new Promise<void>((resolve, reject) => {
    const script = document.createElement('script');
    script.src = ORT_CDN;
    script.crossOrigin = 'anonymous';
    script.onload = () => {
      // Configure WASM backend: single-threaded + SIMD where available
      const ort = (window as any).ort;
      if (ort?.env?.wasm) {
        ort.env.wasm.numThreads = 1;
        ort.env.wasm.simd = true;
      }
      console.log('[onnx-loader] ONNX Runtime Web loaded');
      resolve();
    };
    script.onerror = () => reject(new Error(`Failed to load ONNX Runtime from ${ORT_CDN}`));
    document.head.appendChild(script);
  });

  return loadPromise;
}

export function isOnnxLoaded(): boolean {
  return !!(window as any).ort;
}
