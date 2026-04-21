# GitNexus WASM — Implementation Plan Index

> **For**: Google Antigravity (or any AI coding agent)  
> **Project**: GitNexus WASM Rewrite  
> **Status**: Ready for execution

---

## Quick Navigation

| # | Task | Priority | Effort | Status | File |
|---|------|----------|--------|--------|------|
| 0 | **Master Plan** | P0 | — | Ready | [MASTER_PLAN.md](MASTER_PLAN.md) |
| 1 | **Real Tokenizer** | P0 | 2 weeks | Ready | [TASK_1_TOKENIZER.md](TASK_1_TOKENIZER.md) |
| 2 | **Web Workers** | P0 | 1.5 weeks | Ready | [TASK_2_WEB_WORKERS.md](TASK_2_WEB_WORKERS.md) |
| 3 | **Community Detection** | P0 | 1.5 weeks | Ready | [TASK_3_COMMUNITY_DETECTION.md](TASK_3_COMMUNITY_DETECTION.md) |
| 4 | **Process Extraction** | P0 | 1 week | Ready | [TASK_4_PROCESS_EXTRACTION.md](TASK_4_PROCESS_EXTRACTION.md) |
| 5 | **Semantic Search** | P0 | 2 weeks | Ready | [TASK_5_SEMANTIC_SEARCH.md](TASK_5_SEMANTIC_SEARCH.md) |
| 6 | **Git Authentication** | P1 | 1 week | Ready | [TASK_6_GIT_AUTH.md](TASK_6_GIT_AUTH.md) |
| 7 | **Incremental Updates** | P1 | 1.5 weeks | Ready | [TASK_7_INCREMENTAL_UPDATES.md](TASK_7_INCREMENTAL_UPDATES.md) |

---

## Execution Order

```
Phase 1: Foundation (Weeks 1-2)
├── Task 1: Real Tokenizer ──────────────┐
│   └── Unblocks Task 5                  │
└── Task 2: Web Workers ─────────────────┤
    └── Unblocks Task 3, 7               │
                                         │
Phase 2: Intelligence (Weeks 3-5)        │
├── Task 3: Community Detection ─────────┤
│   └── Unblocks Task 4                  │
├── Task 4: Process Extraction ──────────┤
│   └── Leaf task                        │
└── Task 5: Semantic Search ─────────────┘
    └── Depends on Task 1

Phase 3: Polish (Weeks 6-7.5)          
├── Task 6: Git Authentication            
│   └── Independent                       
└── Task 7: Incremental Updates          
    └── Depends on Task 2, 6            
```

---

## Agent Instructions

### Before Starting Each Task
1. Read the task file completely
2. Check `MASTER_PLAN.md` for dependency status
3. Verify previous tasks are complete (or handle gracefully)
4. Review acceptance criteria

### During Implementation
1. Follow the step-by-step guide in each task file
2. Write unit tests as you go (TDD preferred)
3. Update progress in this index file
4. Document decisions in `docs/ADR/`

### After Completing Each Task
1. Run all tests (Rust + browser)
2. Update status below
3. Commit with message: `feat(task-N): brief description`
4. Move to next task in dependency order

---

## Progress Tracker

- [ ] Task 1: Real Tokenizer — **NOT STARTED**
- [ ] Task 2: Web Workers — **NOT STARTED**
- [ ] Task 3: Community Detection — **NOT STARTED**
- [ ] Task 4: Process Extraction — **NOT STARTED**
- [ ] Task 5: Semantic Search — **NOT STARTED**
- [ ] Task 6: Git Authentication — **NOT STARTED**
- [ ] Task 7: Incremental Updates — **NOT STARTED**

---

## Key Files in Main Project

| File | Description |
|------|-------------|
| `crates/gitnexus-core/src/lib.rs` | Main WASM entry point |
| `crates/gitnexus-parse/src/lib.rs` | Tree-sitter parser |
| `crates/gitnexus-graph/src/lib.rs` | KuzuDB graph DB |
| `crates/gitnexus-embed/src/lib.rs` | ONNX embeddings |
| `crates/gitnexus-git/src/lib.rs` | Git operations |
| `web/src/App.tsx` | React root component |
| `web/src/hooks/useStore.ts` | Zustand state store |
| `.github/workflows/deploy.yml` | CI/CD pipeline |

---

## Emergency Contacts (Metaphorical)

| Issue | Resource |
|-------|----------|
| `tokenizers` crate won't compile to WASM | See Task 1 Option B (custom BPE) |
| KuzuDB WASM missing vector index | See Task 5 brute-force fallback |
| Web Workers crash on parser load | See Task 2 error recovery section |
| Louvain too slow | Reduce `max_levels` to 5, use `resolution: 0.5` |
| ONNX Runtime Web fails | Use CDN fallback, smaller model |
| 4GB memory exceeded | Implement streaming (chunked processing) |

---

*Generated for AI agent execution. Good luck, Antigravity.* 🚀
