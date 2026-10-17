//! Protocole `cadence-plugin` (plan § 2.7) : sert à l'iframe d'un plugin les fichiers de son
//! dossier `ui/`, et rien d'autre.
//!
//! URL vue par le webview : `cadence-plugin://localhost/<id>/<version>/<chemin>` (macOS, Linux),
//! `http://cadence-plugin.localhost/<id>/<version>/<chemin>` (Windows, WebView2). Le chemin est
//! normalisé : `..`, racines et chemins hors de `ui/` répondent 404 ; un chemin vide sert
//! `ui.entry`. La version de l'URL doit être celle installée (elle change à chaque mise à jour :
//! pas de cache à invalider).
//!
//! En-têtes : type d'après l'extension, `Access-Control-Allow-Origin: *` (l'iframe a une origine
//! opaque et Vite produit des scripts `type=module` chargés en CORS), `Cache-Control: no-cache`,
//! et une CSP qui interdit tout appel réseau (`connect-src 'none'`).

use std::path::{Component, Path, PathBuf};

use crate::plugins::{manifest, registry::PluginRegistry};

pub const SCHEME: &str = "cadence-plugin";

/// Origine des iframes de plugin, telle que le webview la voit.
pub fn plugin_origin() -> &'static str {
    if cfg!(windows) { "http://cadence-plugin.localhost" } else { "cadence-plugin:" }
}

/// CSP d'une ressource de plugin.
pub fn content_security_policy() -> String {
    let o = plugin_origin();
    format!(
        "default-src 'none'; script-src {o} 'unsafe-inline'; style-src {o} 'unsafe-inline'; \
         img-src {o} data: blob: https:; font-src {o} data:; media-src {o} blob:; connect-src 'none'; \
         frame-src 'none'; form-action 'none'; base-uri 'none'"
    )
}

/// Type MIME d'après l'extension.
pub fn mime_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref() {
        Some("html" | "htm") => "text/html; charset=utf-8",
        Some("js" | "mjs") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json" | "map") => "application/json",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("ico") => "image/x-icon",
        Some("woff2") => "font/woff2",
        Some("woff") => "font/woff",
        Some("ttf") => "font/ttf",
        Some("wasm") => "application/wasm",
        Some("txt") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

/// Fichier à servir pour le chemin d'une requête (`/<id>/<version>/<chemin>`), ou `None` (404).
pub fn resolve(registry: &PluginRegistry, request_path: &str) -> Option<PathBuf> {
    let decoded = percent_decode(request_path.trim_start_matches('/'))?;
    let mut parts = decoded.splitn(3, '/');
    let (id, version, rest) = (parts.next()?, parts.next()?, parts.next().unwrap_or(""));
    if !manifest::is_plugin_id(id) {
        return None;
    }
    let entry = registry.entry(id).ok()?;
    let dir = registry.plugin_dir(id, &entry);
    let m = manifest::parse(&std::fs::read_to_string(dir.join(manifest::FILE_NAME)).ok()?, registry.app_version()).ok()?;
    if m.version != version || !entry.enabled {
        return None;
    }
    let relative = if rest.is_empty() || rest.ends_with('/') { m.ui_entry? } else { rest.to_string() };
    let rel = Path::new(&relative);
    if rel.is_absolute() || !rel.components().all(|c| matches!(c, Component::Normal(_))) {
        return None;
    }
    let path = dir.join("ui").join(rel);
    path.is_file().then_some(path)
}

/// Décodage `%XX` ; `None` si la séquence est invalide ou n'est pas de l'UTF-8.
fn percent_decode(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = std::str::from_utf8(bytes.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// Réponse HTTP pour une requête du protocole.
pub fn respond(registry: &PluginRegistry, request: &http::Request<Vec<u8>>) -> http::Response<Vec<u8>> {
    let builder = http::Response::builder()
        .header("Access-Control-Allow-Origin", "*")
        .header("Cache-Control", "no-cache");
    let found = resolve(registry, request.uri().path()).and_then(|path| std::fs::read(&path).ok().map(|b| (path, b)));
    match found {
        Some((path, body)) => builder
            .status(200)
            .header("Content-Type", mime_type(&path))
            .header("Content-Security-Policy", content_security_policy())
            .body(body),
        None => builder.status(404).header("Content-Type", "text/plain; charset=utf-8").body(b"Introuvable".to_vec()),
    }
    .unwrap_or_else(|_| http::Response::new(Vec::new()))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::plugins::registry::{InstalledEntry, PluginSource};

    fn setup(name: &str) -> (PathBuf, PluginRegistry) {
        let root = crate::app_settings::temp_dir(name);
        let dir = root.join("plugins").join("journal").join("0.1.0");
        fs::create_dir_all(dir.join("ui").join("assets")).unwrap();
        fs::write(
            dir.join("plugin.toml"),
            "api = 1\nid = \"journal\"\nname = \"Journal\"\nversion = \"0.1.0\"\nauthors = [\"a\"]\nmin_cadence = \"0.1.0\"\n[ui]\nentry = \"index.html\"\n",
        )
        .unwrap();
        fs::write(dir.join("ui").join("index.html"), "<html></html>").unwrap();
        fs::write(dir.join("ui").join("assets").join("app un.js"), "export {}").unwrap();
        fs::write(dir.join("secret.txt"), "hors de ui").unwrap();
        let registry = PluginRegistry::load(root.join("plugins"), "0.3.0");
        registry
            .put("journal", InstalledEntry { version: "0.1.0".into(), previous: None, source: PluginSource::File, dev_path: None, enabled: true })
            .unwrap();
        (root, registry)
    }

    #[test]
    fn serves_only_the_ui_folder_of_the_installed_version() {
        let (root, registry) = setup("plugin-protocol-resolve");
        let ok = |path: &str| resolve(&registry, path).map(|p| p.file_name().unwrap().to_string_lossy().into_owned());
        assert_eq!(ok("/journal/0.1.0/").as_deref(), Some("index.html"), "chemin vide : ui.entry");
        assert_eq!(ok("/journal/0.1.0").as_deref(), Some("index.html"));
        assert_eq!(ok("/journal/0.1.0/assets/app%20un.js").as_deref(), Some("app un.js"));
        for refused in [
            "/journal/0.1.0/../secret.txt",
            "/journal/0.1.0/%2e%2e/secret.txt",
            "/journal/0.1.0/assets/../../secret.txt",
            "/journal/0.2.0/index.html",
            "/autre/0.1.0/index.html",
            "/Journal/0.1.0/index.html",
            "/journal/0.1.0/absent.js",
            "/journal/0.1.0/%zz",
        ] {
            assert_eq!(ok(refused), None, "{refused}");
        }
        registry.set_enabled("journal", false).unwrap();
        assert_eq!(ok("/journal/0.1.0/"), None, "plugin désactivé");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn answers_with_cors_csp_and_types() {
        let (root, registry) = setup("plugin-protocol-respond");
        let request = |path: &str| http::Request::builder().uri(format!("cadence-plugin://localhost{path}")).body(Vec::new()).unwrap();
        let page = respond(&registry, &request("/journal/0.1.0/index.html"));
        assert_eq!(page.status(), 200);
        assert_eq!(page.headers()["Content-Type"], "text/html; charset=utf-8");
        assert_eq!(page.headers()["Access-Control-Allow-Origin"], "*");
        let csp = page.headers()["Content-Security-Policy"].to_str().unwrap();
        assert!(csp.contains("connect-src 'none'") && csp.contains(plugin_origin()));
        let missing = respond(&registry, &request("/journal/0.1.0/../secret.txt"));
        assert_eq!(missing.status(), 404);
        assert_eq!(mime_type(Path::new("a.MJS")), "text/javascript; charset=utf-8");
        let _ = fs::remove_dir_all(&root);
    }
}
