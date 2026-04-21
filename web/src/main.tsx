import React from 'react';
import ReactDOM from 'react-dom/client';
import App from './App';
import './index.css';

// Initialize WASM on load
async function init() {
  const loadingStatus = document.getElementById('loading-status');

  try {
    if (loadingStatus) loadingStatus.textContent = 'Loading GitNexus WASM...';

    // Dynamically import WASM module
    const { default: initWasm } = await import('../pkg/gitnexus_core');
    await initWasm();

    if (loadingStatus) loadingStatus.textContent = 'Starting application...';

    // Hide loading screen
    const loading = document.getElementById('loading');
    if (loading) loading.classList.add('hidden');

    // Mount React app
    ReactDOM.createRoot(document.getElementById('root')!).render(
      <React.StrictMode>
        <App />
      </React.StrictMode>
    );
  } catch (err) {
    console.error('Failed to initialize:', err);
    if (loadingStatus) {
      loadingStatus.textContent = 'Failed to load. Please refresh or use a modern browser.';
      loadingStatus.style.color = '#ef4444';
    }
  }
}

init();
