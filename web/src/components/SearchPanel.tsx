import React, { useState } from 'react';
import { Search, Loader2, FileCode, ArrowRight } from 'lucide-react';
import { useGitNexusStore } from '../hooks/useStore';

export function SearchPanel() {
  const [query, setQuery] = useState('');
  const [results, setResults] = useState<any[]>([]);
  const [isSearching, setIsSearching] = useState(false);
  const { search, setSelectedNode } = useGitNexusStore();

  const handleSearch = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!query.trim()) return;

    setIsSearching(true);
    const searchResults = await search(query);
    setResults(searchResults);
    setIsSearching(false);
  };

  return (
    <div className="h-full flex flex-col">
      <div className="p-4 border-b border-border">
        <h2 className="font-semibold mb-3">Search</h2>
        <form onSubmit={handleSearch} className="relative">
          <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-text-muted" />
          <input
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search symbols, files, concepts..."
            className="w-full pl-9 pr-4 py-2 rounded-lg bg-bg-tertiary border border-border text-sm focus:outline-none focus:border-accent"
          />
        </form>
      </div>

      <div className="flex-1 overflow-y-auto p-2">
        {isSearching ? (
          <div className="flex items-center justify-center py-8">
            <Loader2 className="w-5 h-5 text-accent animate-spin" />
          </div>
        ) : results.length === 0 ? (
          <div className="text-center py-8 text-text-muted text-sm">
            {query ? 'No results found' : 'Enter a query to search'}
          </div>
        ) : (
          <div className="space-y-1">
            {results.map((result, idx) => (
              <SearchResultItem
                key={idx}
                result={result}
                onClick={() => setSelectedNode(result)}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}

function SearchResultItem({ result, onClick }: { result: any; onClick: () => void }) {
  return (
    <button
      onClick={onClick}
      className="w-full text-left p-3 rounded-lg hover:bg-bg-tertiary transition-colors group"
    >
      <div className="flex items-start gap-2">
        <FileCode className="w-4 h-4 text-text-muted mt-0.5 flex-shrink-0" />
        <div className="flex-1 min-w-0">
          <p className="font-medium text-sm truncate">{result.name}</p>
          <p className="text-xs text-text-muted mt-0.5">{result.type} · {result.filePath}</p>
          {result.startLine && (
            <p className="text-xs text-text-muted">Line {result.startLine}</p>
          )}
        </div>
        <ArrowRight className="w-4 h-4 text-text-muted opacity-0 group-hover:opacity-100 transition-opacity" />
      </div>
    </button>
  );
}
