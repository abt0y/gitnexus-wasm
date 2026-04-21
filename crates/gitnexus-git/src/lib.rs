//! Git operations for GitNexus WASM (Browser)
//!
//! Uses isomorphic-git (pure JS) via wasm-bindgen since there's no native git binary.
//! Supports: clone, status, diff, log, branch detection

use wasm_bindgen::prelude::*;
use js_sys::{Promise, Reflect, Array, Object};

// ============================================================================
// isomorphic-git Bridge
// ============================================================================

/// Git repository handle
#[wasm_bindgen]
pub struct GitRepo {
    #[wasm_bindgen(skip)]
    pub dir: String,           // Directory handle or path
    #[wasm_bindgen(skip)]
    pub is_bare: bool,
    #[wasm_bindgen(skip)]
    pub git_instance: JsValue, // isomorphic-git instance
}

#[wasm_bindgen]
impl GitRepo {
    #[wasm_bindgen(constructor)]
    pub async fn new(dir: String) -> Result<GitRepo, JsValue> {
        console_error_panic_hook::set_once();

        let window = web_sys::window().ok_or("No window")?;
        let git = Reflect::get(&window, &"git".into())?;

        if git.is_undefined() {
            Self::load_git_lib().await?;
        }
        
        let git_fresh = Reflect::get(&window, &"git".into())?;

        Ok(GitRepo {
            dir,
            is_bare: false,
            git_instance: git_fresh,
        })
    }

    async fn load_git_lib() -> Result<(), JsValue> {
        let window = web_sys::window().ok_or("No window")?;
        let document = window.document().ok_or("No document")?;

        let scripts = [
            "https://cdn.jsdelivr.net/npm/isomorphic-git@1.25.0/index.umd.min.js",
            "https://cdn.jsdelivr.net/npm/@isomorphic-git/lightning-fs@4.6.0/dist/lightning-fs.min.js",
            "https://unpkg.com/isomorphic-git@1.25.0/http/web/index.umd.js" // http client
        ];

        for src in scripts {
            let script = document.create_element("script")?;
            script.set_attribute("src", src)?;
            script.set_attribute("crossorigin", "anonymous")?;

            let promise = Promise::new(&mut |resolve, _reject| {
                let closure = Closure::once_into_js(move || {
                    let _ = resolve.call1(&JsValue::NULL, &JsValue::NULL);
                });
                let _ = Reflect::set(&script, &"onload".into(), &closure);
            });

            // Fallback if head() is missing - use documentElement or find head by tag
            let head = document.get_elements_by_tag_name("head").item(0)
                .ok_or_else(|| JsValue::from_str("No <head> found"))?;
            head.append_child(&script)?;
            wasm_bindgen_futures::JsFuture::from(promise).await?;
        }

        Ok(())
    }

    /// Clone a remote repository (Task 6)
    pub async fn clone(
        &self, 
        url: &str, 
        token: Option<String>, 
        cors_proxy: Option<String>
    ) -> Result<(), JsValue> {
        let clone_method: js_sys::Function = Reflect::get(&self.git_instance, &"clone".into())?.dyn_into()?;
        let window = web_sys::window().unwrap();
        let http = Reflect::get(&window, &"GitHttp".into())?;

        let options = Object::new();
        Reflect::set(&options, &"dir".into(), &JsValue::from_str(&self.dir))?;
        Reflect::set(&options, &"url".into(), &JsValue::from_str(url))?;
        Reflect::set(&options, &"fs".into(), &self.get_fs()?)?;
        Reflect::set(&options, &"http".into(), &http)?;
        Reflect::set(&options, &"singleBranch".into(), &true.into())?;
        Reflect::set(&options, &"depth".into(), &1.0.into())?;

        if let Some(proxy) = cors_proxy {
            Reflect::set(&options, &"corsProxy".into(), &JsValue::from_str(&proxy))?;
        }

        if let Some(t) = token {
            let on_auth = Closure::wrap(Box::new(move |_: JsValue| {
                let auth = Object::new();
                let _ = Reflect::set(&auth, &"username".into(), &t.clone().into());
                auth.into()
            }) as Box<dyn Fn(JsValue) -> JsValue>);
            
            Reflect::set(&options, &"onAuth".into(), on_auth.as_ref())?;
            on_auth.forget(); // Keep alive for the duration of clone
        }

        let promise: Promise = clone_method.call1(&JsValue::NULL, &options)?.dyn_into()?;
        wasm_bindgen_futures::JsFuture::from(promise).await?;

        Ok(())
    }

    pub async fn pull(&self, token: Option<String>) -> Result<(), JsValue> {
        let pull_method: js_sys::Function = Reflect::get(&self.git_instance, &"pull".into())?.dyn_into()?;
        let window = web_sys::window().unwrap();
        let http = Reflect::get(&window, &"GitHttp".into())?;

        let options = Object::new();
        Reflect::set(&options, &"dir".into(), &JsValue::from_str(&self.dir))?;
        Reflect::set(&options, &"fs".into(), &self.get_fs()?)?;
        Reflect::set(&options, &"http".into(), &http)?;

        if let Some(t) = token {
            let on_auth = Closure::wrap(Box::new(move |_: JsValue| {
                let auth = Object::new();
                let _ = Reflect::set(&auth, &"username".into(), &t.clone().into());
                auth.into()
            }) as Box<dyn Fn(JsValue) -> JsValue>);
            Reflect::set(&options, &"onAuth".into(), on_auth.as_ref())?;
            on_auth.forget();
        }

        let promise: Promise = pull_method.call1(&JsValue::NULL, &options)?.dyn_into()?;
        wasm_bindgen_futures::JsFuture::from(promise).await?;
        Ok(())
    }

    fn get_fs(&self) -> Result<JsValue, JsValue> {
        let window = web_sys::window().ok_or("No window")?;
        let lightning_fs = Reflect::get(&window, &"LightningFS".into())?;
        let fs_class: js_sys::Function = lightning_fs.dyn_into()?;
        
        let args = Array::new();
        args.push(&JsValue::from_str("gitnexus-fs"));
        let fs_instance = Reflect::construct(&fs_class, &args)?;
        Ok(fs_instance)
    }

    pub async fn list_files(&self) -> Result<Vec<String>, JsValue> {
        let list_files_method: js_sys::Function = Reflect::get(&self.git_instance, &"listFiles".into())?.dyn_into()?;
        let options = Object::new();
        Reflect::set(&options, &"dir".into(), &JsValue::from_str(&self.dir))?;
        Reflect::set(&options, &"fs".into(), &self.get_fs()?)?;

        let promise: Promise = list_files_method.call1(&JsValue::NULL, &options)?.dyn_into()?;
        let files = wasm_bindgen_futures::JsFuture::from(promise).await?;
        Ok(js_sys::Array::from(&files).iter().map(|f| f.as_string().unwrap()).collect())
    }
}
