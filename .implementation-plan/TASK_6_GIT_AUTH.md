# Task 6: Git Authentication — Implementation Guide

**Priority**: P1 (Enhancement)  
**Estimated Effort**: 1 week  
**Skill Level**: Intermediate (HTTPS auth, browser security)  
**Dependencies**: None  
**Blocks**: None

---

## Problem Statement

isomorphic-git supports HTTPS clone but requires authentication for private repos. Current implementation lacks:
1. Personal Access Token (PAT) input
2. Secure token storage (session only)
3. CORS proxy configuration
4. Provider support (GitHub, GitLab, Bitbucket)

---

## Implementation

### Step 1: Git Auth Modal UI (Day 1-2)

```typescript
// web/src/components/GitAuthModal.tsx
import React, { useState } from 'react';
import { X, Github, Gitlab, Lock } from 'lucide-react';

interface GitAuthModalProps {
    isOpen: boolean;
    onClose: () => void;
    onAuth: (config: GitAuthConfig) => void;
}

interface GitAuthConfig {
    provider: 'github' | 'gitlab' | 'bitbucket' | 'custom';
    token: string;
    username?: string;
    corsProxy?: string;
}

export function GitAuthModal({ isOpen, onClose, onAuth }: GitAuthModalProps) {
    const [provider, setProvider] = useState<'github' | 'gitlab' | 'bitbucket' | 'custom'>('github');
    const [token, setToken] = useState('');
    const [username, setUsername] = useState('');
    const [showToken, setShowToken] = useState(false);
    const [error, setError] = useState<string | null>(null);

    if (!isOpen) return null;

    const handleSubmit = (e: React.FormEvent) => {
        e.preventDefault();

        if (!token.trim()) {
            setError('Token is required');
            return;
        }

        // Store in sessionStorage (NOT localStorage — security)
        sessionStorage.setItem('gitnexus_git_token', token);
        sessionStorage.setItem('gitnexus_git_provider', provider);

        onAuth({
            provider,
            token,
            username: username || undefined,
            corsProxy: provider === 'github' ? 'https://cors.isomorphic-git.org' : undefined,
        });

        onClose();
    };

    return (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
            <div className="bg-bg-secondary border border-border rounded-2xl p-6 w-full max-w-md mx-4">
                <div className="flex items-center justify-between mb-6">
                    <h2 className="text-lg font-semibold">Git Authentication</h2>
                    <button onClick={onClose} className="p-1 hover:bg-bg-tertiary rounded">
                        <X className="w-5 h-5" />
                    </button>
                </div>

                <form onSubmit={handleSubmit} className="space-y-4">
                    {/* Provider selection */}
                    <div>
                        <label className="text-sm text-text-secondary mb-2 block">Provider</label>
                        <div className="grid grid-cols-3 gap-2">
                            {(['github', 'gitlab', 'bitbucket'] as const).map((p) => (
                                <button
                                    key={p}
                                    type="button"
                                    onClick={() => setProvider(p)}
                                    className={`p-2 rounded-lg border text-sm capitalize transition-colors ${
                                        provider === p
                                            ? 'border-accent bg-accent/10 text-accent'
                                            : 'border-border hover:border-text-muted'
                                    }`}
                                >
                                    {p}
                                </button>
                            ))}
                        </div>
                    </div>

                    {/* Token input */}
                    <div>
                        <label className="text-sm text-text-secondary mb-2 block">
                            Personal Access Token
                            <span className="text-danger ml-1">*</span>
                        </label>
                        <div className="relative">
                            <input
                                type={showToken ? 'text' : 'password'}
                                value={token}
                                onChange={(e) => {
                                    setToken(e.target.value);
                                    setError(null);
                                }}
                                placeholder="ghp_xxxxxxxxxxxx"
                                className="w-full px-3 py-2 rounded-lg bg-bg-tertiary border border-border text-sm focus:outline-none focus:border-accent pr-10"
                            />
                            <button
                                type="button"
                                onClick={() => setShowToken(!showToken)}
                                className="absolute right-3 top-1/2 -translate-y-1/2 text-text-muted"
                            >
                                <Lock className="w-4 h-4" />
                            </button>
                        </div>
                        {error && <p className="text-danger text-xs mt-1">{error}</p>}
                    </div>

                    {/* Username (for GitLab) */}
                    {provider === 'gitlab' && (
                        <div>
                            <label className="text-sm text-text-secondary mb-2 block">Username (optional)</label>
                            <input
                                type="text"
                                value={username}
                                onChange={(e) => setUsername(e.target.value)}
                                placeholder="your-username"
                                className="w-full px-3 py-2 rounded-lg bg-bg-tertiary border border-border text-sm focus:outline-none focus:border-accent"
                            />
                        </div>
                    )}

                    {/* Security notice */}
                    <div className="bg-warning/10 border border-warning/20 rounded-lg p-3 text-xs text-warning">
                        <p className="font-medium mb-1">⚠️ Security Notice</p>
                        <p>Your token is stored in sessionStorage and cleared when you close the tab. Never share your token or commit it to code.</p>
                    </div>

                    <button
                        type="submit"
                        className="w-full py-2 rounded-lg bg-accent text-white font-medium hover:bg-accent-hover transition-colors"
                    >
                        Authenticate
                    </button>
                </form>
            </div>
        </div>
    );
}
```

### Step 2: Integrate with Git Operations (Day 3-4)

```rust
// crates/gitnexus-git/src/lib.rs (additions)

#[wasm_bindgen]
impl GitRepo {
    pub async fn clone_with_auth(
        &self,
        url: &str,
        token: &str,
        username: Option<String>,
        cors_proxy: Option<String>,
    ) -> Result<(), JsValue> {
        let clone_method: js_sys::Function = Reflect::get(&self.git_instance, &"clone".into())?.dyn_into()?;

        let options = Object::new();
        Reflect::set(&options, &"dir".into(), &JsValue::from_str(&self.dir))?;
        Reflect::set(&options, &"url".into(), &JsValue::from_str(url))?;
        Reflect::set(&options, &"fs".into(), &self.get_fs()?)?;
        Reflect::set(&options, &"depth".into(), &JsValue::from_f64(1.0))?;

        // Auth headers
        let headers = Object::new();
        Reflect::set(&headers, &"Authorization".into(), &JsValue::from_str(&format!("Bearer {}", token)))?;
        Reflect::set(&options, &"headers".into(), &headers)?;

        if let Some(proxy) = cors_proxy {
            Reflect::set(&options, &"corsProxy".into(), &JsValue::from_str(&proxy))?;
        }

        let promise: Promise = clone_method.call1(&JsValue::NULL, &options)?.dyn_into()?;
        wasm_bindgen_futures::JsFuture::from(promise).await?;

        info!("Cloned {} with auth", url);
        Ok(())
    }
}
```

### Step 3: Git Status in UI (Day 5-7)

```typescript
// web/src/hooks/useStore.ts (additions)
interface GitState {
    branch: string | null;
    modifiedFiles: string[];
    isGitRepo: boolean;
}

// In store:
gitState: GitState | null;

async detectGitState(): Promise<void> {
    const { engine, currentRepo } = get();
    if (!engine || !currentRepo) return;

    try {
        const git = await GitRepo.new(currentRepo.path);
        const branch = await git.current_branch();
        const status = await git.status();

        set({
            gitState: {
                branch,
                modifiedFiles: status.filter(s => s.status !== 'unmodified').map(s => s.path),
                isGitRepo: true,
            }
        });
    } catch (err) {
        console.log('Not a git repo or git not available');
    }
}
```

---

## Acceptance Criteria

- [ ] Can clone private GitHub repo with PAT
- [ ] Token stored in sessionStorage (cleared on tab close)
- [ ] UI shows current branch and modified files
- [ ] CORS proxy configurable
- [ ] GitLab and Bitbucket supported
- [ ] Error handling for invalid/expired tokens

---

## Deliverables

1. `web/src/components/GitAuthModal.tsx` — Auth UI
2. `crates/gitnexus-git/src/lib.rs` — Modified (HTTPS auth)
3. `web/src/hooks/useStore.ts` — Modified (git state)
