//! Tree-sitter based code parsing for GitNexus WASM
//!
//! This crate provides language-agnostic AST extraction using tree-sitter
//! grammars compiled to WASM. It runs entirely in the browser.
//!
//! Supported languages: TypeScript, JavaScript, Python, Go, Rust, Java,
//! C/C++, C#, PHP, Swift, Ruby, COBOL

use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use js_sys::{Promise, Reflect, Array};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use log::info;



// ============================================================================
// Language Registry
// ============================================================================

/// Language configuration for tree-sitter
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageConfig {
    pub name: String,
    pub parser_wasm_url: String,
    pub file_extensions: Vec<String>,
    pub node_types: LanguageNodeTypes,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageNodeTypes {
    pub function: Vec<String>,
    pub class: Vec<String>,
    pub interface: Vec<String>,
    pub method: Vec<String>,
    pub struct_type: Vec<String>,
    pub enum_type: Vec<String>,
    pub import: Vec<String>,
    pub call: Vec<String>,
    pub property: Vec<String>,
    pub namespace: Vec<String>,
}

/// Built-in language configurations
pub fn built_in_languages() -> Vec<LanguageConfig> {
    vec![
        LanguageConfig {
            name: "typescript".to_string(),
            parser_wasm_url: "./parsers/typescript.wasm".to_string(),
            file_extensions: vec!["ts".to_string(), "tsx".to_string(), "mts".to_string(), "cts".to_string()],
            node_types: LanguageNodeTypes {
                function: vec!["function_declaration", "arrow_function", "function"].iter().map(|s| s.to_string()).collect(),
                class: vec!["class_declaration", "class"].iter().map(|s| s.to_string()).collect(),
                interface: vec!["interface_declaration", "interface"].iter().map(|s| s.to_string()).collect(),
                method: vec!["method_definition", "method"].iter().map(|s| s.to_string()).collect(),
                struct_type: vec!["type_alias_declaration"].iter().map(|s| s.to_string()).collect(),
                enum_type: vec!["enum_declaration"].iter().map(|s| s.to_string()).collect(),
                import: vec!["import_statement", "import_declaration"].iter().map(|s| s.to_string()).collect(),
                call: vec!["call_expression"].iter().map(|s| s.to_string()).collect(),
                property: vec!["property_definition", "property_signature"].iter().map(|s| s.to_string()).collect(),
                namespace: vec!["module_declaration", "namespace"].iter().map(|s| s.to_string()).collect(),
            },
        },
        LanguageConfig {
            name: "javascript".to_string(),
            parser_wasm_url: "./parsers/javascript.wasm".to_string(),
            file_extensions: vec!["js".to_string(), "jsx".to_string(), "mjs".to_string(), "cjs".to_string()],
            node_types: LanguageNodeTypes {
                function: vec!["function_declaration", "arrow_function", "function"].iter().map(|s| s.to_string()).collect(),
                class: vec!["class_declaration", "class"].iter().map(|s| s.to_string()).collect(),
                interface: vec!["no_interface"].iter().filter(|s| !s.is_empty()).map(|s| s.to_string()).collect(),
                method: vec!["method_definition", "method"].iter().map(|s| s.to_string()).collect(),
                struct_type: vec![].iter().map(|s: &&str| s.to_string()).collect(),
                enum_type: vec![].iter().map(|s: &&str| s.to_string()).collect(),
                import: vec!["import_statement", "import_declaration"].iter().map(|s| s.to_string()).collect(),
                call: vec!["call_expression"].iter().map(|s| s.to_string()).collect(),
                property: vec!["property_definition"].iter().map(|s| s.to_string()).collect(),
                namespace: vec![].iter().map(|s: &&str| s.to_string()).collect(),
            },
        },
        LanguageConfig {
            name: "python".to_string(),
            parser_wasm_url: "./parsers/python.wasm".to_string(),
            file_extensions: vec!["py".to_string(), "pyi".to_string()],
            node_types: LanguageNodeTypes {
                function: vec!["function_definition"].iter().map(|s| s.to_string()).collect(),
                class: vec!["class_definition"].iter().map(|s| s.to_string()).collect(),
                interface: vec![].iter().map(|s: &&str| s.to_string()).collect(),
                method: vec!["function_definition"].iter().map(|s| s.to_string()).collect(),
                struct_type: vec!["dataclass"].iter().map(|s| s.to_string()).collect(),
                enum_type: vec!["class_definition"].iter().map(|s| s.to_string()).collect(),
                import: vec!["import_statement", "import_from_statement"].iter().map(|s| s.to_string()).collect(),
                call: vec!["call"].iter().map(|s| s.to_string()).collect(),
                property: vec!["attribute"].iter().map(|s| s.to_string()).collect(),
                namespace: vec![].iter().map(|s: &&str| s.to_string()).collect(),
            },
        },
        LanguageConfig {
            name: "rust".to_string(),
            parser_wasm_url: "./parsers/rust.wasm".to_string(),
            file_extensions: vec!["rs".to_string()],
            node_types: LanguageNodeTypes {
                function: vec!["function_item"].iter().map(|s| s.to_string()).collect(),
                class: vec!["struct_item", "impl_item"].iter().map(|s| s.to_string()).collect(),
                interface: vec!["trait_item"].iter().map(|s| s.to_string()).collect(),
                method: vec!["function_item"].iter().map(|s| s.to_string()).collect(),
                struct_type: vec!["struct_item"].iter().map(|s| s.to_string()).collect(),
                enum_type: vec!["enum_item"].iter().map(|s| s.to_string()).collect(),
                import: vec!["use_declaration"].iter().map(|s| s.to_string()).collect(),
                call: vec!["call_expression"].iter().map(|s| s.to_string()).collect(),
                property: vec!["field_expression"].iter().map(|s| s.to_string()).collect(),
                namespace: vec!["mod_item"].iter().map(|s| s.to_string()).collect(),
            },
        },
        LanguageConfig {
            name: "go".to_string(),
            parser_wasm_url: "./parsers/go.wasm".to_string(),
            file_extensions: vec!["go".to_string()],
            node_types: LanguageNodeTypes {
                function: vec!["function_declaration"].iter().map(|s| s.to_string()).collect(),
                class: vec!["type_declaration"].iter().map(|s| s.to_string()).collect(),
                interface: vec!["type_declaration"].iter().map(|s| s.to_string()).collect(),
                method: vec!["method_declaration"].iter().map(|s| s.to_string()).collect(),
                struct_type: vec!["type_declaration"].iter().map(|s| s.to_string()).collect(),
                enum_type: vec!["const_declaration"].iter().map(|s| s.to_string()).collect(),
                import: vec!["import_declaration"].iter().map(|s| s.to_string()).collect(),
                call: vec!["call_expression"].iter().map(|s| s.to_string()).collect(),
                property: vec!["selector_expression"].iter().map(|s| s.to_string()).collect(),
                namespace: vec![].iter().map(|s: &&str| s.to_string()).collect(),
            },
        },
        LanguageConfig {
            name: "java".to_string(),
            parser_wasm_url: "./parsers/java.wasm".to_string(),
            file_extensions: vec!["java".to_string()],
            node_types: LanguageNodeTypes {
                function: vec!["method_declaration"].iter().map(|s| s.to_string()).collect(),
                class: vec!["class_declaration"].iter().map(|s| s.to_string()).collect(),
                interface: vec!["interface_declaration"].iter().map(|s| s.to_string()).collect(),
                method: vec!["method_declaration"].iter().map(|s| s.to_string()).collect(),
                struct_type: vec!["record_declaration"].iter().map(|s| s.to_string()).collect(),
                enum_type: vec!["enum_declaration"].iter().map(|s| s.to_string()).collect(),
                import: vec!["import_declaration"].iter().map(|s| s.to_string()).collect(),
                call: vec!["method_invocation"].iter().map(|s| s.to_string()).collect(),
                property: vec!["field_access"].iter().map(|s| s.to_string()).collect(),
                namespace: vec!["package_declaration"].iter().map(|s| s.to_string()).collect(),
            },
        },
    ]
}

// ============================================================================
// Parser State
// ============================================================================

/// Loaded tree-sitter language instance
#[wasm_bindgen]
pub struct TreeSitterLanguage {
    name: String,
    #[wasm_bindgen(skip)]
    pub config: LanguageConfig,
    #[wasm_bindgen(skip)]
    pub parser: JsValue, // Tree-sitter Parser instance
    #[wasm_bindgen(skip)]
    pub language: JsValue, // Tree-sitter Language instance
}

#[wasm_bindgen]
impl TreeSitterLanguage {
    pub fn name(&self) -> String {
        self.name.clone()
    }
}

/// Parser registry - holds loaded languages
#[wasm_bindgen]
pub struct ParserRegistry {
    #[wasm_bindgen(skip)]
    pub languages: HashMap<String, TreeSitterLanguage>,
}

#[wasm_bindgen]
impl ParserRegistry {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        console_error_panic_hook::set_once();
        wasm_logger::init(wasm_logger::Config::new(log::Level::Info));
        Self {
            languages: HashMap::new(),
        }
    }

    /// Initialize tree-sitter and load a language parser
    pub async fn load_language(&mut self, lang_name: &str) -> Result<(), JsValue> {
        let config = built_in_languages()
            .into_iter()
            .find(|l| l.name == lang_name)
            .ok_or_else(|| JsValue::from_str(&format!("Unknown language: {}", lang_name)))?;

        info!("Loading tree-sitter parser for {}", lang_name);

        // Access global TreeSitter object from JS
        let window = web_sys::window().ok_or("No window")?;
        let tree_sitter = Reflect::get(&window, &"TreeSitter".into())?;

        // Create Parser instance
        let parser_ctor = Reflect::get(&tree_sitter, &"Parser".into())?;
        let parser: js_sys::Function = parser_ctor.dyn_into()?;
        let parser_instance = Reflect::construct(&parser, &Array::new())?;

        // Load language WASM
        let language_promise: Promise = {
            let lang_method: js_sys::Function = Reflect::get(&tree_sitter, &"Language".into())?.dyn_into()?;
            let load_method: js_sys::Function = Reflect::get(&lang_method, &"load".into())?.dyn_into()?;
            let url = JsValue::from_str(&config.parser_wasm_url);
            let promise: Promise = load_method.call1(&JsValue::NULL, &url)?.dyn_into()?;
            promise
        };

        let language = JsFuture::from(language_promise).await?;

        // Set language on parser
        let set_language: js_sys::Function = Reflect::get(&parser_instance, &"setLanguage".into())?.dyn_into()?;
        set_language.call1(&parser_instance, &language)?;

        let lang = TreeSitterLanguage {
            name: lang_name.to_string(),
            config: config.clone(),
            parser: parser_instance,
            language,
        };

        self.languages.insert(lang_name.to_string(), lang);
        info!("Successfully loaded parser for {}", lang_name);

        Ok(())
    }

    /// Check if a language is loaded
    pub fn is_loaded(&self, lang_name: &str) -> bool {
        self.languages.contains_key(lang_name)
    }

    /// Detect language from file extension
    pub fn detect_language(file_path: &str) -> Option<String> {
        let ext = file_path.split('.').last()?;
        built_in_languages()
            .into_iter()
            .find(|l| l.file_extensions.iter().any(|e| e.trim_start_matches('.') == ext))
            .map(|l| l.name)
    }

    /// Parse a file and extract symbols
    pub fn parse_file(&self, file_path: &str, content: &str) -> Result<JsValue, JsValue> {
        let lang_name = Self::detect_language(file_path)
            .ok_or_else(|| JsValue::from_str("Unsupported file type"))?;

        let lang = self.languages.get(&lang_name)
            .ok_or_else(|| JsValue::from_str(&format!("Language not loaded: {}", lang_name)))?;

        // Parse source code
        let parse_method: js_sys::Function = Reflect::get(&lang.parser, &"parse".into())?.dyn_into()?;
        let content_js = JsValue::from_str(content);
        let tree = parse_method.call1(&lang.parser, &content_js)?;

        // Extract root node
        let root_node = Reflect::get(&tree, &"rootNode".into())?;

        let mut symbols = Vec::new();
        let mut imports = Vec::new();
        let mut calls = Vec::new();

        // Walk the tree and extract nodes
        self.walk_tree(&root_node, content, &lang.config, &mut symbols, &mut imports, &mut calls, file_path)?;

        let parsed = ParsedFile {
            file_path: file_path.to_string(),
            language: lang_name,
            symbols,
            imports,
            calls,
        };
        
        serde_wasm_bindgen::to_value(&parsed).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    fn walk_tree(
        &self,
        node: &JsValue,
        source: &str,
        config: &LanguageConfig,
        symbols: &mut Vec<Symbol>,
        imports: &mut Vec<Import>,
        calls: &mut Vec<CallSite>,
        file_path: &str,
    ) -> Result<(), JsValue> {
        let type_method: js_sys::Function = Reflect::get(node, &"type".into())?.dyn_into()?;
        let node_type: String = type_method.call0(node)?.as_string().unwrap_or_default();

        // Extract position info
        let start_position = Reflect::get(node, &"startPosition".into())?;
        let end_position = Reflect::get(node, &"endPosition".into())?;
        let start_row: u32 = Reflect::get(&start_position, &"row".into())?.as_f64().unwrap_or(0.0) as u32;
        let end_row: u32 = Reflect::get(&end_position, &"row".into())?.as_f64().unwrap_or(0.0) as u32;

        // Extract text
        let text_method: js_sys::Function = Reflect::get(node, &"text".into())?.dyn_into()?;
        let text: String = text_method.call0(node)?.as_string().unwrap_or_default();

        // Check node type against language config
        if config.node_types.function.contains(&node_type) {
            let name = self.extract_name(node, "name")?;
            symbols.push(Symbol {
                id: format!("Function:{}", name),
                name,
                kind: SymbolKind::Function,
                file_path: file_path.to_string(),
                start_line: start_row + 1,
                end_line: end_row + 1,
                content: Some(text),
                ..Default::default()
            });
        } else if config.node_types.class.contains(&node_type) {
            let name = self.extract_name(node, "name")?;
            symbols.push(Symbol {
                id: format!("Class:{}", name),
                name,
                kind: SymbolKind::Class,
                file_path: file_path.to_string(),
                start_line: start_row + 1,
                end_line: end_row + 1,
                content: Some(text),
                ..Default::default()
            });
        } else if config.node_types.interface.contains(&node_type) {
            let name = self.extract_name(node, "name")?;
            symbols.push(Symbol {
                id: format!("Interface:{}", name),
                name,
                kind: SymbolKind::Interface,
                file_path: file_path.to_string(),
                start_line: start_row + 1,
                end_line: end_row + 1,
                content: Some(text),
                ..Default::default()
            });
        } else if config.node_types.method.contains(&node_type) {
            let name = self.extract_name(node, "name")?;
            symbols.push(Symbol {
                id: format!("Method:{}", name),
                name,
                kind: SymbolKind::Method,
                file_path: file_path.to_string(),
                start_line: start_row + 1,
                end_line: end_row + 1,
                content: Some(text),
                ..Default::default()
            });
        } else if config.node_types.import.contains(&node_type) {
            imports.push(Import {
                source: text,
                line: start_row + 1,
            });
        } else if config.node_types.call.contains(&node_type) {
            let func_name = self.extract_call_target(node)?;
            calls.push(CallSite {
                target: func_name,
                line: start_row + 1,
            });
        }

        // Recurse into children
        let children_method: js_sys::Function = Reflect::get(node, &"children".into())?.dyn_into()?;
        let children = children_method.call0(node)?;
        let children_array = js_sys::Array::from(&children);

        for i in 0..children_array.length() {
            let child = children_array.get(i);
            self.walk_tree(&child, source, config, symbols, imports, calls, file_path)?;
        }

        Ok(())
    }

    fn extract_name(&self, node: &JsValue, field: &str) -> Result<String, JsValue> {
        // Try childForFieldName first
        let child_for_field: js_sys::Function = Reflect::get(node, &"childForFieldName".into())?.dyn_into()?;
        let child = child_for_field.call1(node, &JsValue::from_str(field))?;

        if !child.is_null() && !child.is_undefined() {
            let text_method: js_sys::Function = Reflect::get(&child, &"text".into())?.dyn_into()?;
            return Ok(text_method.call0(&child)?.as_string().unwrap_or_default());
        }

        // Fallback: use node text
        let text_method: js_sys::Function = Reflect::get(node, &"text".into())?.dyn_into()?;
        Ok(text_method.call0(node)?.as_string().unwrap_or_default())
    }

    fn extract_call_target(&self, node: &JsValue) -> Result<String, JsValue> {
        // Try to get function name from call expression
        let child_for_field: js_sys::Function = Reflect::get(node, &"childForFieldName".into())?.dyn_into()?;
        let func = child_for_field.call1(node, &JsValue::from_str("function"))?;

        if !func.is_null() && !func.is_undefined() {
            let text_method: js_sys::Function = Reflect::get(&func, &"text".into())?.dyn_into()?;
            return Ok(text_method.call0(&func)?.as_string().unwrap_or_default());
        }

        Ok("unknown".to_string())
    }
}

// ============================================================================
// Parsed Output Types
// ============================================================================

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedFile {
    pub file_path: String,
    pub language: String,
    pub symbols: Vec<Symbol>,
    pub imports: Vec<Import>,
    pub calls: Vec<CallSite>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Symbol {
    pub id: String,
    pub name: String,
    pub kind: SymbolKind,
    pub file_path: String,
    pub start_line: u32,
    pub end_line: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_exported: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameter_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub return_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SymbolKind {
    #[default]
    Function,
    Class,
    Interface,
    Method,
    Struct,
    Enum,
    Trait,
    Module,
    Namespace,
    Property,
    Const,
    Static,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Import {
    pub source: String,
    pub line: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallSite {
    pub target: String,
    pub line: u32,
}

// ============================================================================
// WASM Exports
// ============================================================================

#[wasm_bindgen]
pub struct WasmParser {
    #[wasm_bindgen(skip)]
    pub registry: ParserRegistry,
}

#[wasm_bindgen]
impl WasmParser {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self { registry: ParserRegistry::new() }
    }

    pub async fn init(&mut self) -> Result<(), JsValue> {
        // Pre-load common languages
        for lang in ["typescript", "javascript", "python", "rust"] {
            let _ = self.registry.load_language(lang).await;
        }
        Ok(())
    }

    pub async fn load_language(&mut self, lang: &str) -> Result<(), JsValue> {
        self.registry.load_language(lang).await
    }

    pub fn parse(&self, file_path: &str, content: &str) -> Result<JsValue, JsValue> {
        self.registry.parse_file(file_path, content)
    }

    pub fn detect_language(file_path: &str) -> Option<String> {
        ParserRegistry::detect_language(file_path)
    }
}
