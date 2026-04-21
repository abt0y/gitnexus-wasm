// WASM module loader and bridge
let wasmModule: any = null;
let gitnexusInstance: any = null;

export async function initGitNexus(): Promise<any> {
  if (gitnexusInstance) return gitnexusInstance;

  // Load required JS libraries first
  await loadScript('https://cdn.jsdelivr.net/npm/kuzu-wasm@0.6.0/browser/kuzu.js');

  // Import Rust WASM
  const wasm = await import('../../pkg/gitnexus_core');
  await wasm.default();

  // Create engine instance
  gitnexusInstance = new wasm.GitNexus();

  // Initialize all subsystems
  const result = await gitnexusInstance.init();

  if (result.success) {
    console.log('GitNexus initialized:', JSON.parse(result.data));
    return gitnexusInstance;
  } else {
    throw new Error(result.error || 'Initialization failed');
  }
}

async function loadScript(src: string): Promise<void> {
  return new Promise((resolve, reject) => {
    const script = document.createElement('script');
    script.src = src;
    script.onload = () => resolve();
    script.onerror = () => reject(new Error(`Failed to load ${src}`));
    document.head.appendChild(script);
  });
}

export function getGitNexus(): any {
  return gitnexusInstance;
}
