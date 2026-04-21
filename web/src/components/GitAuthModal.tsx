import React, { useState } from 'react';
import { X, Lock, Eye, EyeOff, AlertTriangle } from 'lucide-react';

export interface GitAuthConfig {
  provider: 'github' | 'gitlab' | 'bitbucket' | 'custom';
  token: string;
  username?: string;
  corsProxy?: string;
  customHost?: string;
}

interface GitAuthModalProps {
  isOpen: boolean;
  onClose: () => void;
  /** Called with the resolved auth config once the user submits. */
  onAuth: (config: GitAuthConfig) => void;
}

const CORS_PROXIES: Record<string, string> = {
  github:    'https://cors.isomorphic-git.org',
  gitlab:    'https://cors.isomorphic-git.org',
  bitbucket: 'https://cors.isomorphic-git.org',
  custom:    '',
};

const PAT_HINTS: Record<string, string> = {
  github:    'ghp_xxxxxxxxxxxxxxxxxxxx',
  gitlab:    'glpat-xxxxxxxxxxxxxxxxxxxx',
  bitbucket: 'ATBB-xxxxxxxxxxxxxxxxxxxx',
  custom:    'Your personal access token',
};

const PAT_LINKS: Record<string, string> = {
  github:    'https://github.com/settings/tokens/new',
  gitlab:    'https://gitlab.com/-/profile/personal_access_tokens',
  bitbucket: 'https://bitbucket.org/account/settings/app-passwords/new',
  custom:    '',
};

export function GitAuthModal({ isOpen, onClose, onAuth }: GitAuthModalProps) {
  const [provider,    setProvider]    = useState<GitAuthConfig['provider']>('github');
  const [token,       setToken]       = useState('');
  const [username,    setUsername]    = useState('');
  const [customHost,  setCustomHost]  = useState('');
  const [showToken,   setShowToken]   = useState(false);
  const [error,       setError]       = useState<string | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);

  if (!isOpen) return null;

  function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!token.trim()) { setError('Token is required'); return; }
    if (provider === 'custom' && !customHost.trim()) {
      setError('Host URL is required for custom providers');
      return;
    }

    setIsSubmitting(true);
    setError(null);

    // Store in sessionStorage only — cleared automatically when the tab closes.
    sessionStorage.setItem('gitnexus_git_token',    token);
    sessionStorage.setItem('gitnexus_git_provider', provider);
    if (username) sessionStorage.setItem('gitnexus_git_user', username);

    const config: GitAuthConfig = {
      provider,
      token,
      username:    username || undefined,
      corsProxy:   CORS_PROXIES[provider] || undefined,
      customHost:  provider === 'custom' ? customHost : undefined,
    };

    onAuth(config);
    setIsSubmitting(false);
    onClose();
  }

  function handleClear() {
    sessionStorage.removeItem('gitnexus_git_token');
    sessionStorage.removeItem('gitnexus_git_provider');
    sessionStorage.removeItem('gitnexus_git_user');
    setToken('');
    setUsername('');
  }

  const storedToken    = sessionStorage.getItem('gitnexus_git_token');
  const storedProvider = sessionStorage.getItem('gitnexus_git_provider');

  return (
    <div
      className="fixed inset-0 bg-black/60 flex items-center justify-center z-50"
      onClick={(e) => { if (e.target === e.currentTarget) onClose(); }}
    >
      <div className="bg-bg-secondary border border-border rounded-2xl p-6 w-full max-w-md mx-4 shadow-2xl">

        {/* Header */}
        <div className="flex items-center justify-between mb-6">
          <h2 className="text-lg font-semibold">Git Authentication</h2>
          <button
            onClick={onClose}
            className="p-1.5 rounded-lg hover:bg-bg-tertiary transition-colors"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Existing session banner */}
        {storedToken && (
          <div className="mb-4 p-3 rounded-lg bg-success/10 border border-success/20 text-sm flex items-center justify-between">
            <span className="text-success">
              Authenticated as <strong>{storedProvider}</strong>
            </span>
            <button
              onClick={handleClear}
              className="text-xs text-text-muted hover:text-text-primary underline"
            >
              Clear
            </button>
          </div>
        )}

        <form onSubmit={handleSubmit} className="space-y-4">

          {/* Provider selector */}
          <div>
            <label className="block text-sm text-text-secondary mb-2">Provider</label>
            <div className="grid grid-cols-4 gap-2">
              {(['github', 'gitlab', 'bitbucket', 'custom'] as const).map((p) => (
                <button
                  key={p}
                  type="button"
                  onClick={() => { setProvider(p); setError(null); }}
                  className={`py-2 rounded-lg border text-xs font-medium capitalize transition-colors ${
                    provider === p
                      ? 'border-accent bg-accent/10 text-accent'
                      : 'border-border hover:border-text-muted text-text-secondary'
                  }`}
                >
                  {p}
                </button>
              ))}
            </div>
          </div>

          {/* Custom host */}
          {provider === 'custom' && (
            <div>
              <label className="block text-sm text-text-secondary mb-1.5">
                Host URL <span className="text-danger">*</span>
              </label>
              <input
                type="url"
                value={customHost}
                onChange={(e) => { setCustomHost(e.target.value); setError(null); }}
                placeholder="https://git.mycompany.com"
                className="w-full px-3 py-2 rounded-lg bg-bg-tertiary border border-border text-sm
                           focus:outline-none focus:border-accent"
              />
            </div>
          )}

          {/* Username (optional for most, required for Bitbucket) */}
          {(provider === 'bitbucket' || provider === 'gitlab' || provider === 'custom') && (
            <div>
              <label className="block text-sm text-text-secondary mb-1.5">
                Username
                {provider === 'bitbucket' ? <span className="text-danger"> *</span> : ' (optional)'}
              </label>
              <input
                type="text"
                value={username}
                onChange={(e) => setUsername(e.target.value)}
                placeholder="your-username"
                className="w-full px-3 py-2 rounded-lg bg-bg-tertiary border border-border text-sm
                           focus:outline-none focus:border-accent"
              />
            </div>
          )}

          {/* Token input */}
          <div>
            <div className="flex items-center justify-between mb-1.5">
              <label className="text-sm text-text-secondary">
                Personal Access Token <span className="text-danger">*</span>
              </label>
              {PAT_LINKS[provider] && (
                <a
                  href={PAT_LINKS[provider]}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="text-xs text-accent hover:underline"
                >
                  Generate token ↗
                </a>
              )}
            </div>

            <div className="relative">
              <Lock className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-text-muted" />
              <input
                type={showToken ? 'text' : 'password'}
                value={token}
                onChange={(e) => { setToken(e.target.value); setError(null); }}
                placeholder={PAT_HINTS[provider]}
                autoComplete="off"
                className="w-full pl-9 pr-10 py-2 rounded-lg bg-bg-tertiary border border-border text-sm
                           focus:outline-none focus:border-accent font-mono"
              />
              <button
                type="button"
                onClick={() => setShowToken(!showToken)}
                className="absolute right-3 top-1/2 -translate-y-1/2 text-text-muted hover:text-text-primary"
              >
                {showToken ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
              </button>
            </div>

            {error && <p className="mt-1 text-xs text-danger">{error}</p>}
          </div>

          {/* Security notice */}
          <div className="p-3 rounded-lg bg-warning/5 border border-warning/20 flex gap-2.5 text-xs text-warning">
            <AlertTriangle className="w-4 h-4 flex-shrink-0 mt-0.5" />
            <div>
              <p className="font-medium mb-0.5">Security Notice</p>
              <p className="text-text-muted">
                Your token is stored only in <code>sessionStorage</code> and is discarded when
                the tab closes. Never commit tokens to source control.
              </p>
            </div>
          </div>

          {/* Actions */}
          <div className="flex gap-3 pt-1">
            <button
              type="button"
              onClick={onClose}
              className="flex-1 py-2 rounded-lg border border-border text-sm hover:bg-bg-tertiary transition-colors"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={isSubmitting}
              className="flex-1 py-2 rounded-lg bg-accent text-white text-sm font-medium
                         hover:bg-accent-hover transition-colors disabled:opacity-50"
            >
              {isSubmitting ? 'Authenticating…' : 'Authenticate'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}

/** Read previously saved auth from sessionStorage (survives React re-renders). */
export function loadSavedAuth(): GitAuthConfig | null {
  const token    = sessionStorage.getItem('gitnexus_git_token');
  const provider = sessionStorage.getItem('gitnexus_git_provider') as GitAuthConfig['provider'] | null;
  if (!token || !provider) return null;
  return {
    provider,
    token,
    username:  sessionStorage.getItem('gitnexus_git_user') ?? undefined,
    corsProxy: CORS_PROXIES[provider] ?? undefined,
  };
}
