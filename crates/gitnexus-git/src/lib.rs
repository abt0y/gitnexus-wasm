//! Git operations for GitNexus WASM (Browser)
//!
//! Uses isomorphic-git (pure JS) via wasm-bindgen since there's no native git binary.
//! Supports: clone, status, diff, log, branch detection

use wasm_bindgen::prelude::*;
use js_sys::{Function, Promise, Reflect, Array, Object};
use web_sys::console;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use log::{info, warn, error};

use gitnexus_shared::*;

// ============================================================================
// isomorphic-git Bridge
// ============================================================================

/// Git repository handle
#[wasm_bindgen]
pub struct GitRepo {
    #[wasm_bindgen(skip)]
    dir: String,           // Directory handle or path
    #[wasm_bindgen(skip)]
    is_bare: bool,
    #[wasm_bindgen(skip)]
    git_instance: JsValue, // isomorphic-git instance
}

#[wasm_bindgen]
impl GitRepo {
    #[wasm_bindgen(constructor)]
    pub async fn new(dir: String) -> Result<GitRepo, JsValue> {
        console_error_panic_hook::set_once();

        let window = web_sys::window().ok_or("No window")?;
        let git = Reflect::get(&window, &"git".into())?;

        if git.is_undefined() {
            // Load isomorphic-git dynamically
            Self::load_git_lib().await?;
            let git = Reflect::get(&window, &"git".into())?;
            if git.is_undefined() {
                return Err(JsValue::from_str("Failed to load isomorphic-git"));
            }
        }

        Ok(GitRepo {
            dir,
            is_bare: false,
            git_instance: git,
        })
    }

    async fn load_git_lib() -> Result<(), JsValue> {
        let window = web_sys::window().ok_or("No window")?;
        let document = window.document().ok_or("No document")?;

        let script = document.create_element("script")?;
        script.set_attribute("src", "https://cdn.jsdelivr.net/npm/isomorphic-git@1.25.0/index.umd.min.js")?;
        script.set_attribute("crossorigin", "anonymous")?;

        let promise = Promise::new(&mut |resolve, _reject| {
            let closure = Closure::once_into_js(move || {
                resolve.call0(&JsValue::NULL).unwrap_or(JsValue::NULL);
            });
            let _ = Reflect::set(&script, &"onload".into(), &closure);
        });

        document.head().unwrap().append_child(&script)?;
        wasm_bindgen_futures::JsFuture::from(promise).await?;

        // Also load LightningFS for filesystem operations
        let fs_script = document.create_element("script")?;
        fs_script.set_attribute("src", "https://cdn.jsdelivr.net/npm/@isomorphic-git/lightning-fs@4.6.0/dist/lightning-fs.min.js")?;
        fs_script.set_attribute("crossorigin", "anonymous")?;

        let fs_promise = Promise::new(&mut |resolve, _reject| {
            let closure = Closure::once_into_js(move || {
                resolve.call0(&JsValue::NULL).unwrap_or(JsValue::NULL);
            });
            let _ = Reflect::set(&fs_script, &"onload".into(), &closure);
        });

        document.head().unwrap().append_child(&fs_script)?;
        wasm_bindgen_futures::JsFuture::from(fs_promise).await?;

        info!("isomorphic-git loaded");
        Ok(())
    }

    /// Initialize a new git repository
    pub async fn init(&self, default_branch: Option<String>) -> Result<(), JsValue> {
        let init_method: js_sys::Function = Reflect::get(&self.git_instance, &"init".into())?.dyn_into()?;

        let options = Object::new();
        Reflect::set(&options, &"dir".into(), &JsValue::from_str(&self.dir))?;
        Reflect::set(&options, &"fs".into(), &self.get_fs()?)?;

        if let Some(branch) = default_branch {
            Reflect::set(&options, &"defaultBranch".into(), &JsValue::from_str(&branch))?;
        }

        let promise: Promise = init_method.call1(&JsValue::NULL, &options)?.dyn_into()?;
        wasm_bindgen_futures::JsFuture::from(promise).await?;

        info!("Git repo initialized at {}", self.dir);
        Ok(())
    }

    /// Clone a remote repository
    pub async fn clone(&self, url: &str, cors_proxy: Option<String>) -> Result<(), JsValue> {
        let clone_method: js_sys::Function = Reflect::get(&self.git_instance, &"clone".into())?.dyn_into()?;

        let options = Object::new();
        Reflect::set(&options, &"dir".into(), &JsValue::from_str(&self.dir))?;
        Reflect::set(&options, &"url".into(), &JsValue::from_str(url))?;
        Reflect::set(&options, &"fs".into(), &self.get_fs()?)?;
        Reflect::set(&options, &"depth".into(), &JsValue::from_f64(1.0))?; // Shallow clone

        if let Some(proxy) = cors_proxy {
            Reflect::set(&options, &"corsProxy".into(), &JsValue::from_str(&proxy))?;
        }

        let promise: Promise = clone_method.call1(&JsValue::NULL, &options)?.dyn_into()?;
        wasm_bindgen_futures::JsFuture::from(promise).await?;

        info!("Cloned {} into {}", url, self.dir);
        Ok(())
    }

    /// Get repository status
    pub async fn status(&self) -> Result<JsValue, JsValue> {
        let status_matrix_method: js_sys::Function = Reflect::get(&self.git_instance, &"statusMatrix".into())?.dyn_into()?;

        let options = Object::new();
        Reflect::set(&options, &"dir".into(), &JsValue::from_str(&self.dir))?;
        Reflect::set(&options, &"fs".into(), &self.get_fs()?)?;

        let promise: Promise = status_matrix_method.call1(&JsValue::NULL, &options)?.dyn_into()?;
        let matrix = wasm_bindgen_futures::JsFuture::from(promise).await?;

        // Parse status matrix
        let matrix_array = js_sys::Array::from(&matrix);
        let mut statuses = Vec::new();

        for i in 0..matrix_array.length() {
            let row = js_sys::Array::from(&matrix_array.get(i));
            if row.length() >= 3 {
                let file_path = row.get(0).as_string().unwrap_or_default();
                let head_status = row.get(1).as_string().unwrap_or_default();
                let workdir_status = row.get(2).as_string().unwrap_or_default();

                let status = match (head_status.as_str(), workdir_status.as_str()) {
                    ("0", "1") => "added",
                    ("1", "1") => "modified",
                    ("1", "0") => "deleted",
                    ("0", "0") => "unmodified",
                    _ => "unknown",
                };

                statuses.push(GitFileStatus {
                    path: file_path,
                    status: status.to_string(),
                });
            }
        }

        serde_wasm_bindgen::to_value(&statuses)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get git diff
    pub async fn diff(&self, ref1: Option<String>, ref2: Option<String>) -> Result<String, JsValue> {
        // isomorphic-git doesn't have a direct diff command
        // We implement a simplified version using statusMatrix
        let statuses = self.status().await?;
        let statuses_vec: Vec<GitFileStatus> = serde_wasm_bindgen::from_value(statuses)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

        let mut diff_output = String::new();
        for status in statuses_vec {
            if status.status != "unmodified" {
                diff_output.push_str(&format!("{}: {}
", status.status, status.path));
            }
        }

        Ok(diff_output)
    }

    /// Get commit log
    pub async fn log(&self, limit: Option<u32>) -> Result<JsValue, JsValue> {
        let log_method: js_sys::Function = Reflect::get(&self.git_instance, &"log".into())?.dyn_into()?;

        let options = Object::new();
        Reflect::set(&options, &"dir".into(), &JsValue::from_str(&self.dir))?;
        Reflect::set(&options, &"fs".into(), &self.get_fs()?)?;

        if let Some(lim) = limit {
            Reflect::set(&options, &"depth".into(), &JsValue::from_f64(lim as f64))?;
        }

        let promise: Promise = log_method.call1(&JsValue::NULL, &options)?.dyn_into()?;
        let commits = wasm_bindgen_futures::JsFuture::from(promise).await?;

        let commits_array = js_sys::Array::from(&commits);
        let mut result = Vec::new();

        for i in 0..commits_array.length() {
            let commit = commits_array.get(i);
            let commit_obj = Object::from(commit);

            result.push(GitCommit {
                oid: Reflect::get(&commit_obj, &"oid".into())?.as_string().unwrap_or_default(),
                message: Reflect::get(&commit_obj, &"commit".into())?
                    .dyn_into::<Object>()
                    .ok()
                    .and_then(|c| Reflect::get(&c, &"message".into()).ok())
                    .and_then(|m| m.as_string()),
                author: Reflect::get(&commit_obj, &"commit".into())?
                    .dyn_into::<Object>()
                    .ok()
                    .and_then(|c| Reflect::get(&c, &"author".into()).ok())
                    .and_then(|a| a.dyn_into::<Object>().ok())
                    .and_then(|a| Reflect::get(&a, &"name".into()).ok())
                    .and_then(|n| n.as_string()),
                timestamp: Reflect::get(&commit_obj, &"commit".into())?
                    .dyn_into::<Object>()
                    .ok()
                    .and_then(|c| Reflect::get(&c, &"committer".into()).ok())
                    .and_then(|c| c.dyn_into::<Object>().ok())
                    .and_then(|c| Reflect::get(&c, &"timestamp".into()).ok())
                    .and_then(|t| t.as_f64().map(|f| f as u64)),
            });
        }

        serde_wasm_bindgen::to_value(&result)
            .map_err(|e| JsValue::from_str(&e.to_string()))
    }

    /// Get current branch
    pub async fn current_branch(&self) -> Result<Option<String>, JsValue> {
        let current_branch_method: js_sys::Function = Reflect::get(&self.git_instance, &"currentBranch".into())?.dyn_into()?;

        let options = Object::new();
        Reflect::set(&options, &"dir".into(), &JsValue::from_str(&self.dir))?;
        Reflect::set(&options, &"fs".into(), &self.get_fs()?)?;
        Reflect::set(&options, &"fullname".into(), &JsValue::from_bool(false))?;

        let promise: Promise = current_branch_method.call1(&JsValue::NULL, &options)?.dyn_into()?;
        let branch = wasm_bindgen_futures::JsFuture::from(promise).await?;

        if branch.is_null() || branch.is_undefined() {
            Ok(None)
        } else {
            Ok(branch.as_string())
        }
    }

    /// Check if directory is a git repository
    pub async fn is_git_repo(dir: &str) -> Result<bool, JsValue> {
        let window = web_sys::window().ok_or("No window")?;
        let git = Reflect::get(&window, &"git".into())?;

        if git.is_undefined() {
            return Ok(false);
        }

        let find_root_method: js_sys::Function = Reflect::get(&git, &"findRoot".into())?.dyn_into()?;
        let options = Object::new();
        Reflect::set(&options, &"filepath".into(), &JsValue::from_str(dir))?;

        let promise: Promise = find_root_method.call1(&JsValue::NULL, &options)?.dyn_into()?;
        match wasm_bindgen_futures::JsFuture::from(promise).await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    fn get_fs(&self) -> Result<JsValue, JsValue> {
        let window = web_sys::window().ok_or("No window")?;
        let lightning_fs = Reflect::get(&window, &"LightningFS".into())?;

        if lightning_fs.is_undefined() {
            return Err(JsValue::from_str("LightningFS not loaded"));
        }

        let fs_class: js_sys::Function = lightning_fs.dyn_into()?;
        let fs_instance = fs_class.new1(&JsValue::from_str("gitnexus-fs"))?;

        Ok(fs_instance)
    }
}

// ============================================================================
// Git Types
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitFileStatus {
    pub path: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitCommit {
    pub oid: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitDiff {
    pub file_path: String,
    pub change_type: String, // "added", "modified", "deleted"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hunks: Option<Vec<DiffHunk>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiffHunk {
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub lines: Vec<String>,
}
