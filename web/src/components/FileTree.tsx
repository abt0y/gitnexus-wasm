import { useState } from 'react';
import { Folder, FileCode, ChevronRight, ChevronDown } from 'lucide-react';
import { useGitNexusStore } from '../store/useStore';

export function FileTree() {
  const { currentRepo } = useGitNexusStore();

  if (!currentRepo) {
    return (
      <div className="p-4 text-center text-text-muted text-sm">
        No repository loaded
      </div>
    );
  }

  return (
    <div className="p-2">
      <h3 className="px-2 py-1 text-xs font-medium text-text-muted uppercase tracking-wider">
        Files
      </h3>
      <div className="mt-1 space-y-0.5">
        {currentRepo.files.map((file) => (
          <FileTreeItem key={file.path} file={file} depth={0} />
        ))}
      </div>
    </div>
  );
}

function FileTreeItem({ file, depth }: { file: any; depth: number }) {
  const [isOpen, setIsOpen] = useState(true);

  const icon = file.isDirectory ? (
    <Folder className="w-4 h-4 text-accent" />
  ) : (
    <FileCode className="w-4 h-4 text-text-muted" />
  );

  return (
    <div>
      <button
        onClick={() => file.isDirectory && setIsOpen(!isOpen)}
        className="w-full flex items-center gap-1.5 px-2 py-1 rounded-md hover:bg-bg-tertiary text-sm text-left transition-colors"
        style={{ paddingLeft: `${depth * 12 + 8}px` }}
      >
        {file.isDirectory && (
          isOpen ? <ChevronDown className="w-3 h-3 text-text-muted" /> 
                 : <ChevronRight className="w-3 h-3 text-text-muted" />
        )}
        {!file.isDirectory && <span className="w-3" />}
        {icon}
        <span className="truncate">{file.name}</span>
      </button>
    </div>
  );
}
