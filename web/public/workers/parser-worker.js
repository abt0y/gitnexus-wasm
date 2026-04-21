/**
 * parser-worker.js — Tree-sitter parser running in a Web Worker
 *
 * Receives: { id, filePath, content, language }
 * Responds: { id, success, result | error }
 *
 * Loaded by ParserWorkerPool in worker-pool.ts
 */

/* global importScripts, TreeSitter, self */

const TREE_SITTER_CDN =
  'https://cdn.jsdelivr.net/npm/web-tree-sitter@0.22.6/tree-sitter.js';

// Cache of already-loaded Language instances
const PARSERS = {};

// Mapping of language → WASM URL (relative to the origin)
const PARSER_URLS = {
  typescript: '/parsers/typescript.wasm',
  javascript: '/parsers/javascript.wasm',
  python:     '/parsers/python.wasm',
  go:         '/parsers/go.wasm',
  rust:       '/parsers/rust.wasm',
  java:       '/parsers/java.wasm',
  c:          '/parsers/c.wasm',
  cpp:        '/parsers/cpp.wasm',
  csharp:     '/parsers/csharp.wasm',
  php:        '/parsers/php.wasm',
  swift:      '/parsers/swift.wasm',
  ruby:       '/parsers/ruby.wasm',
};

// Language → node-type config mirrors gitnexus-parse/src/lib.rs
const LANG_CONFIG = {
  typescript: {
    function:  ['function_declaration', 'arrow_function', 'function'],
    class:     ['class_declaration', 'class'],
    interface: ['interface_declaration'],
    method:    ['method_definition'],
    enum:      ['enum_declaration'],
    import:    ['import_statement', 'import_declaration'],
    call:      ['call_expression'],
  },
  javascript: {
    function:  ['function_declaration', 'arrow_function', 'function'],
    class:     ['class_declaration'],
    interface: [],
    method:    ['method_definition'],
    enum:      [],
    import:    ['import_statement', 'import_declaration'],
    call:      ['call_expression'],
  },
  python: {
    function:  ['function_definition'],
    class:     ['class_definition'],
    interface: [],
    method:    ['function_definition'],
    enum:      [],
    import:    ['import_statement', 'import_from_statement'],
    call:      ['call'],
  },
  go: {
    function:  ['function_declaration'],
    class:     ['type_declaration'],
    interface: ['type_declaration'],
    method:    ['method_declaration'],
    enum:      ['const_declaration'],
    import:    ['import_declaration'],
    call:      ['call_expression'],
  },
  rust: {
    function:  ['function_item'],
    class:     ['struct_item', 'impl_item'],
    interface: ['trait_item'],
    method:    ['function_item'],
    enum:      ['enum_item'],
    import:    ['use_declaration'],
    call:      ['call_expression'],
  },
  java: {
    function:  ['method_declaration'],
    class:     ['class_declaration'],
    interface: ['interface_declaration'],
    method:    ['method_declaration'],
    enum:      ['enum_declaration'],
    import:    ['import_declaration'],
    call:      ['method_invocation'],
  },
};

// ---- initialisation --------------------------------------------------------

let treeSitterReady = false;

async function ensureTreeSitter() {
  if (treeSitterReady) return;
  importScripts(TREE_SITTER_CDN);
  await TreeSitter.init();
  treeSitterReady = true;
}

async function getParser(language) {
  if (PARSERS[language]) return PARSERS[language];

  await ensureTreeSitter();

  const url = PARSER_URLS[language];
  if (!url) throw new Error(`No parser URL for language: ${language}`);

  const lang = await TreeSitter.Language.load(url);
  const parser = new TreeSitter.Parser();
  parser.setLanguage(lang);

  PARSERS[language] = { parser, config: LANG_CONFIG[language] || LANG_CONFIG.javascript };
  return PARSERS[language];
}

// ---- symbol extraction -----------------------------------------------------

function extractSymbols(node, filePath, config, source) {
  const symbols = [];
  const imports = [];
  const calls   = [];

  function walk(n) {
    const type = n.type;

    if (config.function.includes(type) || config.method.includes(type)) {
      const nameNode = n.childForFieldName?.('name');
      const name = nameNode ? nameNode.text : '<anonymous>';
      const kind = config.method.includes(type) ? 'Method' : 'Function';
      symbols.push({
        id: `${kind}:${name}`,
        name,
        kind,
        file_path: filePath,
        start_line: n.startPosition.row + 1,
        end_line:   n.endPosition.row + 1,
        content:    source.slice(n.startIndex, n.endIndex).substring(0, 4000),
      });
    } else if (config.class.includes(type)) {
      const nameNode = n.childForFieldName?.('name');
      const name = nameNode ? nameNode.text : '<anonymous>';
      symbols.push({
        id: `Class:${name}`,
        name,
        kind: 'Class',
        file_path: filePath,
        start_line: n.startPosition.row + 1,
        end_line:   n.endPosition.row + 1,
        content:    source.slice(n.startIndex, n.endIndex).substring(0, 4000),
      });
    } else if (config.interface.includes(type)) {
      const nameNode = n.childForFieldName?.('name');
      const name = nameNode ? nameNode.text : '<anonymous>';
      symbols.push({
        id: `Interface:${name}`,
        name,
        kind: 'Interface',
        file_path: filePath,
        start_line: n.startPosition.row + 1,
        end_line:   n.endPosition.row + 1,
        content:    source.slice(n.startIndex, n.endIndex).substring(0, 2000),
      });
    } else if (config.import.includes(type)) {
      imports.push({
        source: n.text,
        line: n.startPosition.row + 1,
      });
    } else if (config.call.includes(type)) {
      const funcNode = n.childForFieldName?.('function');
      calls.push({
        target: funcNode ? funcNode.text : 'unknown',
        line: n.startPosition.row + 1,
      });
    }

    for (let i = 0; i < n.childCount; i++) {
      walk(n.child(i));
    }
  }

  walk(node);
  return { symbols, imports, calls };
}

// ---- message handler -------------------------------------------------------

self.onmessage = async function (e) {
  const { id, filePath, content, language } = e.data;

  try {
    const { parser, config } = await getParser(language);
    const tree = parser.parse(content);
    const { symbols, imports, calls } = extractSymbols(
      tree.rootNode, filePath, config, content
    );

    self.postMessage({
      id,
      success: true,
      result: { file_path: filePath, language, symbols, imports, calls },
    });
  } catch (err) {
    self.postMessage({ id, success: false, error: String(err) });
  }
};
