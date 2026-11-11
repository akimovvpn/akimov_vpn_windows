//! Owns the mihomo process. The core runs while connected or testing latency, so an idle app costs no core memory,
//! and it can never outlive the app: it is tied to us with a job object.
use crate::{config, sysproxy};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::BufRead;
#[cfg(test)]
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
#[cfg(test)]
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::SeqCst};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::Emitter;

#[derive(Default, Serialize, Deserialize)]
struct Settings {
    #[serde(default)]
    profile: crate::profile::Metadata,
    #[serde(default)]
    sub_id: Option<u64>,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default)]
    sub_url: String,
    #[serde(default)]
    rules: Vec<config::Rule>,
    #[serde(default)]
    node: String,
    #[serde(default)]
    last_proxy: String,
    #[serde(default)]
    hwid: String,
    #[serde(default)]
    profile_hwid: String,
    // stored inverted so a missing setting means "close to the tray", the default
    #[serde(default)]
    quit_on_close: bool,
}

struct Run {
    tun: bool,
    child: crate::process::Process,
    ctl: u16,
    mixed: u16,
    secret: String,
    main: String,
    since: Instant,
    prev: Option<sysproxy::Saved>,
}

struct ProbeDir {
    path: PathBuf,
    base: PathBuf,
}
impl Drop for ProbeDir {
    fn drop(&mut self) {
        if let Ok(path) = self.path.canonicalize() {
            if path.parent() == Some(self.base.as_path()) {
                let _ = std::fs::remove_dir_all(path);
            }
        }
    }
}
struct Probe {
    run: Run,
    _files: ProbeDir,
}
struct Testing<'a>(&'a AtomicBool);
impl Drop for Testing<'_> {
    fn drop(&mut self) {
        self.0.store(false, SeqCst);
    }
}

fn default_mode() -> String {
    "tun".into()
}

pub fn is_admin() -> bool {
    unsafe { windows_sys::Win32::UI::Shell::IsUserAnAdmin() != 0 }
}

fn tun_ready() -> bool {
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        ConvertInterfaceAliasToLuid, GetIfEntry2, MIB_IF_ROW2,
    };
    let alias: Vec<u16> = "akimov\0".encode_utf16().collect();
    unsafe {
        let mut row: MIB_IF_ROW2 = std::mem::zeroed();
        ConvertInterfaceAliasToLuid(alias.as_ptr(), &mut row.InterfaceLuid) == 0
            && GetIfEntry2(&mut row) == 0
            && row.OperStatus == 1
    }
}

impl Drop for Run {
    fn drop(&mut self) {
        if let Some(saved) = &self.prev {
            let _ = sysproxy::restore(saved, &data_dir().join("proxy-recovery.json"));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct St {
    settings: Settings,
    run: Option<Run>,
}

struct Latency {
    revision: u64,
    checked: Instant,
    results: HashMap<String, Value>,
}
impl Latency {
    fn fresh(&self, revision: u64) -> bool {
        self.revision == revision && self.checked.elapsed() < Duration::from_secs(60)
    }
}

pub struct Engine {
    st: Mutex<St>,
    gen: AtomicU64,
    watch: AtomicBool,
    local: ureq::Agent,
    closing: AtomicBool,
    testing: AtomicBool,
    latency: Mutex<Option<Latency>>,
    profile_revision: AtomicU64,
}

pub(crate) fn data_dir() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    let d = base.join("akimov-vpn");
    let _ = std::fs::create_dir_all(d.join("core"));
    d
}

fn save(s: &Settings) -> Result<(), String> {
    let text = serde_json::to_vec_pretty(s).map_err(|e| e.to_string())?;
    let path = data_dir().join("settings.json");
    let temp = path.with_extension("tmp");
    std::fs::write(&temp, text)
        .and_then(|_| std::fs::rename(temp, path))
        .map_err(|e| e.to_string())
}

fn clear_provider_cache() -> Result<(), String> {
    let base = data_dir().canonicalize().map_err(|e| e.to_string())?;
    let providers = base.join("core/providers");
    if providers.exists() {
        let resolved = providers.canonicalize().map_err(|e| e.to_string())?;
        if !resolved.starts_with(&base) {
            return Err("Provider cache is outside the app directory.".into());
        }
        std::fs::remove_dir_all(resolved).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub(crate) fn core_exe() -> Result<PathBuf, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let here = exe.parent().map(PathBuf::from).unwrap_or_default();
    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("core");
    [
        here.join("core").join("mihomo.exe"),
        here.join("mihomo.exe"),
        dev.join("mihomo-windows-amd64-compatible.exe"),
    ]
    .into_iter()
    .find(|p| p.exists())
    .ok_or_else(|| "The core is missing from this install.".to_string())
}

fn free_port() -> Result<u16, String> {
    std::net::TcpListener::bind("127.0.0.1:0")
        .and_then(|l| l.local_addr())
        .map(|a| a.port())
        .map_err(|e| e.to_string())
}

/// Cryptographic randomness from Windows, also used for the device id.
pub(crate) fn random_hex() -> String {
    use windows_sys::Win32::Security::Cryptography::{
        BCryptGenRandom, BCRYPT_USE_SYSTEM_PREFERRED_RNG,
    };
    let mut bytes = [0u8; 16];
    let status = unsafe {
        BCryptGenRandom(
            std::ptr::null_mut(),
            bytes.as_mut_ptr(),
            bytes.len() as u32,
            BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    };
    assert_eq!(status, 0, "Windows random generator failed");
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn enc(s: &str) -> String {
    s.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

fn node_health(name: &str, all: &Value, seen: &mut std::collections::HashSet<String>) -> Value {
    if !seen.insert(name.to_string()) {
        return json!({"delay":null,"status":"unknown"});
    }
    let p = &all[name];
    let kind = p["type"]
        .as_str()
        .unwrap_or("")
        .to_ascii_lowercase()
        .replace('-', "");
    let selected = p["now"].as_str();
    let mut result = if let Some(now) = selected {
        node_health(now, all, seen)
    } else {
        match p["history"]
            .as_array()
            .and_then(|h| h.last())
            .and_then(|h| h["delay"].as_u64())
        {
            Some(delay) if delay > 0 => json!({"delay":delay,"status":"ok"}),
            Some(_) => json!({"delay":null,"status":"timeout"}),
            None => json!({"delay":null,"status":"unknown"}),
        }
    };
    if ["urltest", "fallback", "loadbalance"].contains(&kind.as_str()) && result["status"] != "ok" {
        let children: Vec<_> = p["all"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(|n| (n, node_health(n, all, seen)))
            .collect();
        if let Some((now, health)) = children
            .iter()
            .filter(|(_, h)| h["status"] == "ok")
            .min_by_key(|(_, h)| h["delay"].as_u64())
        {
            result = health.clone();
            result["now"] = json!(now);
        } else if !children.is_empty() && children.iter().all(|(_, h)| h["status"] == "timeout") {
            result = json!({"delay":null,"status":"timeout"});
        } else {
            result = json!({"delay":null,"status":"unknown"});
        }
    }
    if result.get("now").is_none() {
        if let Some(now) = selected {
            result["now"] = json!(now);
        }
    }
    seen.remove(name);
    result
}

fn proxy_snapshot(v: &Value, main: &str) -> Value {
    let all = &v["proxies"];
    let group = &all[main];
    let nodes: Vec<_> = group["all"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(|name| {
            let mut health = node_health(name, all, &mut std::collections::HashSet::new());
            health["name"] = json!(name);
            health["type"] = all[name]["type"].clone();
            health
        })
        .collect();
    json!({"main":main,"now":group["now"],"nodes":nodes})
}

fn apply_url_measurements(snapshot: &mut Value, measurements: Option<&HashMap<String, Value>>) {
    if let Some(nodes) = snapshot["nodes"].as_array_mut() {
        for node in nodes {
            let measured =
                measurements.and_then(|values| values.get(node["name"].as_str().unwrap_or("")));
            node["delay"] = measured.map_or(Value::Null, |value| value["delay"].clone());
            node["status"] = measured.map_or(json!("unknown"), |value| value["status"].clone());
            node["url"] = measured.map_or(Value::Null, |value| value["url"].clone());
        }
    }
}

fn cached_proxy_snapshot(
    text: &str,
    rules: &[config::Rule],
    selected: &str,
) -> Result<Value, String> {
    let sub = config::parse(text)?;
    let built = config::build(&sub, rules, 1, 2, "", false)?;
    let profile = config::picker_profile(&built.yaml, &built.main)?;
    let group = profile["proxy-groups"]
        .as_sequence()
        .into_iter()
        .flatten()
        .find(|g| g["name"].as_str() == Some(&built.main))
        .ok_or("No servers are available in this subscription.")?;
    let names: Vec<_> = group["proxies"]
        .as_sequence()
        .into_iter()
        .flatten()
        .filter_map(serde_yaml::Value::as_str)
        .collect();
    let now = names
        .iter()
        .copied()
        .find(|n| *n == selected)
        .or_else(|| names.first().copied());
    let nodes: Vec<_> = names
        .iter()
        .map(|name| json!({"name":name,"delay":null,"status":"unknown"}))
        .collect();
    Ok(json!({"main":built.main,"now":now,"nodes":nodes}))
}

impl Engine {
    pub fn new() -> Self {
        let mut settings: Settings = std::fs::read_to_string(data_dir().join("settings.json"))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        let hardware_id = crate::hwid::read().unwrap_or_default();
        if settings.hwid != hardware_id {
            settings.hwid = hardware_id;
            let _ = save(&settings);
        }
        let local = ureq::AgentBuilder::new()
            .try_proxy_from_env(false)
            .timeout_connect(Duration::from_millis(200))
            .timeout(Duration::from_secs(4))
            .build();
        Engine {
            st: Mutex::new(St {
                settings,
                run: None,
            }),
            gen: AtomicU64::new(0),
            watch: AtomicBool::new(false),
            local,
            closing: AtomicBool::new(false),
            testing: AtomicBool::new(false),
            latency: Mutex::new(None),
            profile_revision: AtomicU64::new(0),
        }
    }

    fn api(
        &self,
        ctl: u16,
        secret: &str,
        method: &str,
        path: &str,
        body: Option<Value>,
        wait: u64,
    ) -> Result<Value, String> {
        let req = self
            .local
            .request(method, &format!("http://127.0.0.1:{ctl}{path}"))
            .set("Authorization", &format!("Bearer {secret}"))
            .timeout(Duration::from_secs(wait));
        let resp = match body {
            Some(b) => req.send_json(b),
            None => req.call(),
        }
        .map_err(|e| e.to_string())?;
        if resp.status() == 204 {
            return Ok(Value::Null);
        }
        Ok(resp.into_json().unwrap_or(Value::Null))
    }

    fn running(&self) -> Option<(u16, String, String, Instant)> {
        let st = self.st.lock().unwrap();
        st.run
            .as_ref()
            .map(|r| (r.ctl, r.secret.clone(), r.main.clone(), r.since))
    }

    pub fn boot(&self) -> Result<Value, String> {
        let st = self.st.lock().unwrap();
        if st.run.is_none() {
            sysproxy::recover(&data_dir().join("proxy-recovery.json"))
                .map_err(|e| format!("Could not recover the Windows proxy: {e}"))?;
        }
        Ok(json!({
            "has_sub": data_dir().join("sub.yaml").exists(),
            "sub_url": st.settings.sub_url,
            "connected": st.run.is_some(),
            "secs": st.run.as_ref().map(|r| r.since.elapsed().as_secs()).unwrap_or(0),
            "rules": st.settings.rules,
            "node": st.settings.node,
            "mode": if st.settings.mode.is_empty() { "tun" } else { &st.settings.mode },
            "close_to_tray": !st.settings.quit_on_close,
            "admin": is_admin(),
            "hwid": st.settings.hwid,
            "hwid_required": true,
            "hwid_available": crate::hwid::valid(&st.settings.hwid),
            "sub_id": st.settings.sub_id,
            "version": env!("CARGO_PKG_VERSION"),
            "profile": st.settings.profile,
        }))
    }

    pub fn set_subscription(&self, url: &str) -> Result<Value, String> {
        let url = url.trim();
        if !url.starts_with("https://") {
            return Err("The link must start with https://".into());
        }
        let hwid = {
            let st = self.st.lock().unwrap();
            if st.run.is_some() {
                return Err("Disconnect before updating the subscription.".into());
            }
            st.settings.hwid.clone()
        };
        let (text, metadata) = crate::profile::fetch(url, &hwid)?;
        let sub = config::parse(&text)?;
        let built = config::build(&sub, &[], 1, 1, "x", false)?; // proves the profile can be turned into a config before we keep it
        let mut st = self.st.lock().unwrap();
        if st.run.is_some() || self.closing.load(SeqCst) {
            return Err("Disconnect before updating the subscription.".into());
        }
        let temp = data_dir().join("sub.tmp");
        std::fs::write(&temp, &text)
            .and_then(|_| std::fs::rename(temp, data_dir().join("sub.yaml")))
            .map_err(|e| e.to_string())?;
        st.settings.sub_url = url.to_string();
        st.settings.profile = metadata;
        self.invalidate_latency();
        st.settings.profile_hwid = hwid;
        save(&st.settings)?;
        Ok(json!({ "ok": true, "nodes": built.nodes }))
    }

    pub fn refresh_subscription(&self) -> Result<Value, String> {
        let (url, hwid) = {
            let st = self.st.lock().unwrap();
            (st.settings.sub_url.clone(), st.settings.hwid.clone())
        };
        if url.is_empty() {
            return Err("Choose a plan before connecting.".into());
        }
        let (text, metadata) = crate::profile::fetch(&url, &hwid)?;
        let sub = config::parse(&text)?;
        let mut st = self.st.lock().unwrap();
        if st.settings.sub_url != url || st.settings.hwid != hwid {
            return Err("The subscription changed. Try again.".into());
        }
        if self.closing.load(SeqCst) {
            return Err("The app is closing.".into());
        }
        let path = data_dir().join("sub.yaml");
        let old = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let cfg = data_dir().join("core/config.yaml");
        let mut rollback = None;
        let mut main = None;
        if let Some(run) = st.run.as_ref() {
            let built = config::build(
                &sub,
                &st.settings.rules,
                run.mixed,
                run.ctl,
                &run.secret,
                run.tun,
            )?;
            let old_cfg = std::fs::read_to_string(&cfg).map_err(|e| e.to_string())?;
            std::fs::write(&cfg, &built.yaml).map_err(|e| e.to_string())?;
            if let Err(error) = self.api(
                run.ctl,
                &run.secret,
                "PUT",
                "/configs?force=false",
                Some(json!({"path":cfg.to_string_lossy()})),
                10,
            ) {
                std::fs::write(&cfg, &old_cfg).map_err(|e| e.to_string())?;
                return Err(error);
            }
            main = Some(built.main);
            rollback = Some((old_cfg, run.ctl, run.secret.clone()));
        } else {
            config::build(
                &sub,
                &st.settings.rules,
                1,
                2,
                "validation",
                st.settings.mode == "tun",
            )?;
        }
        self.invalidate_latency();
        let old_metadata = std::mem::replace(&mut st.settings.profile, metadata);
        let old_profile_hwid = std::mem::replace(&mut st.settings.profile_hwid, hwid);
        let temp = path.with_extension("tmp");
        let result = std::fs::write(&temp, &text)
            .and_then(|_| std::fs::rename(&temp, &path))
            .map_err(|e| e.to_string())
            .and_then(|_| save(&st.settings));
        if let Err(error) = result {
            st.settings.profile = old_metadata;
            st.settings.profile_hwid = old_profile_hwid;
            std::fs::write(&path, old).map_err(|e| e.to_string())?;
            if let Some((old_cfg, ctl, secret)) = rollback {
                std::fs::write(&cfg, old_cfg).map_err(|e| e.to_string())?;
                self.api(
                    ctl,
                    &secret,
                    "PUT",
                    "/configs?force=false",
                    Some(json!({"path":cfg.to_string_lossy()})),
                    10,
                )?;
            }
            return Err(error);
        }
        if let (Some(run), Some(main)) = (st.run.as_mut(), main) {
            run.main = main;
        }
        drop(st);
        self.boot()
    }

    fn ensure_profile_hwid(&self) -> Result<(), String> {
        let needs_refresh = {
            let st = self.st.lock().unwrap();
            if !crate::hwid::valid(&st.settings.hwid) {
                return Err(crate::hwid::UNAVAILABLE.into());
            }
            !st.settings.sub_url.is_empty() && st.settings.profile_hwid != st.settings.hwid
        };
        if needs_refresh {
            self.refresh_subscription()?;
        }
        Ok(())
    }

    pub fn connect(self: &Arc<Self>, app: tauri::AppHandle) -> Result<Value, String> {
        let attempt = self.gen.load(SeqCst);
        self.ensure_profile_hwid()?;
        let (result, generation) = self.start_attempt(attempt)?;
        if let Some(gen) = generation {
            if let Err(error) = self.verify_connection() {
                let _ = self.stop(Some(gen));
                return Err(error);
            }
            if self.gen.load(SeqCst) != gen {
                return Err("Connection cancelled.".into());
            }
            let me = self.clone();
            std::thread::spawn(move || me.ticker(app, gen));
        }
        Ok(result)
    }

    fn verify_connection(&self) -> Result<(), String> {
        let measured = self.test_latency()?;
        let selected = measured["now"].as_str().ok_or("vpn_unreachable")?;
        if !measured["nodes"].as_array().is_some_and(|nodes| {
            nodes
                .iter()
                .any(|node| node["name"] == selected && node["status"] == "ok")
        }) {
            return Err("vpn_unreachable".into());
        }
        let tun = self
            .st
            .lock()
            .unwrap()
            .run
            .as_ref()
            .is_some_and(|run| run.tun);
        if tun {
            let tls = native_tls::TlsConnector::new()
                .map_err(|_| "Whole-device connection could not be checked.")?;
            let agent = ureq::AgentBuilder::new()
                .tls_connector(Arc::new(tls))
                .try_proxy_from_env(false)
                .timeout(Duration::from_secs(8))
                .build();
            if ![
                "https://www.gstatic.com/generate_204",
                "https://cp.cloudflare.com/generate_204",
            ]
            .iter()
            .any(|url| {
                agent
                    .get(url)
                    .call()
                    .is_ok_and(|response| response.status() == 204)
            }) {
                return Err("tun_unreachable".into());
            }
        }
        Ok(())
    }

    #[cfg(test)]
    fn start(&self) -> Result<(Value, Option<u64>), String> {
        self.start_attempt(self.gen.load(SeqCst))
    }

    fn start_attempt(&self, attempt: u64) -> Result<(Value, Option<u64>), String> {
        let t0 = Instant::now();
        let mut st = self.st.lock().unwrap();
        if !crate::hwid::valid(&st.settings.hwid) {
            return Err(crate::hwid::UNAVAILABLE.into());
        }
        if self.closing.load(SeqCst) {
            return Err("The app is closing.".into());
        }
        if self.gen.load(SeqCst) != attempt {
            return Err("Connection cancelled.".into());
        }
        if st.run.is_some() {
            return Ok((json!({ "ok": true, "ms": 0 }), None));
        }
        let dir = data_dir();
        let tun = st.settings.mode == "tun" || st.settings.mode.is_empty();
        sysproxy::recover(&dir.join("proxy-recovery.json")).map_err(|e| e.to_string())?;
        let text = std::fs::read_to_string(dir.join("sub.yaml"))
            .map_err(|_| "no_subscription".to_string())?;
        let sub = config::parse(&text)?;
        let mixed = free_port()?;
        let mut ctl = free_port()?;
        while ctl == mixed {
            ctl = free_port()?;
        }
        let secret = random_hex();
        let built = config::build(&sub, &st.settings.rules, mixed, ctl, &secret, tun)?;
        let cfg = dir.join("core").join("config.yaml");
        std::fs::write(&cfg, &built.yaml).map_err(|e| e.to_string())?;
        let mut child = crate::process::Process::spawn(
            &core_exe()?,
            &dir.join("core"),
            &cfg,
            &dir.join("core/core.log"),
            tun && !is_admin(),
        )?;
        // connected means the controller answers; poll fast so a warm start takes a fraction of a second
        let deadline = Instant::now() + Duration::from_secs(25);
        loop {
            if self.closing.load(SeqCst) || self.gen.load(SeqCst) != attempt {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Connection cancelled.".into());
            }
            if self.api(ctl, &secret, "GET", "/version", None, 1).is_ok() && (!tun || tun_ready()) {
                break;
            }
            if let Ok(Some(code)) = child.try_wait() {
                return Err(format!(
                    "The core stopped while starting ({code}). The profile may be invalid."
                ));
            }
            if Instant::now() > deadline {
                let _ = child.kill();
                let _ = child.wait();
                let details =
                    std::fs::read_to_string(dir.join("core/core.log")).unwrap_or_default();
                let last = details
                    .lines()
                    .rev()
                    .find(|line| line.contains("error") || line.contains("Error"))
                    .unwrap_or("The adapter or controller did not become ready.");
                return Err(format!("The core did not start: {last}"));
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        if self.closing.load(SeqCst) || self.gen.load(SeqCst) != attempt {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Connection cancelled.".into());
        }
        if !st.settings.node.is_empty() {
            let _ = self.api(
                ctl,
                &secret,
                "PUT",
                &format!("/proxies/{}", enc(&built.main)),
                Some(json!({ "name": st.settings.node })),
                2,
            );
        }
        let prev = if tun {
            None
        } else {
            match sysproxy::set(mixed, &dir.join("proxy-recovery.json")) {
                Ok(saved) => Some(saved),
                Err(e) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!("Could not set the Windows proxy: {e}"));
                }
            }
        };
        st.settings.last_proxy = sysproxy::ours(mixed);
        let _ = save(&st.settings);
        st.run = Some(Run {
            tun,
            child,
            ctl,
            mixed,
            secret,
            main: built.main.clone(),
            since: Instant::now(),
            prev,
        });
        let gen = self.gen.fetch_add(1, SeqCst) + 1;
        drop(st);
        Ok((
            json!({ "ok": true, "ms": t0.elapsed().as_millis() as u64, "nodes": built.nodes, "main": built.main }),
            Some(gen),
        ))
    }

    /// Order matters: give Windows its proxy back, close every open connection, then stop the core.
    pub fn disconnect(&self) -> Result<Value, String> {
        self.gen.fetch_add(1, SeqCst);
        self.stop(None)
    }

    /// Local port of the running core, 0 when disconnected. Lets account requests fall back to the VPN route.
    pub fn proxy_port(&self) -> u16 {
        let st = self.st.lock().unwrap();
        st.run.as_ref().map(|r| r.mixed).unwrap_or(0)
    }

    pub fn shutdown(&self) {
        self.closing.store(true, SeqCst);
        let _ = self.disconnect();
    }

    fn stop(&self, expected: Option<u64>) -> Result<Value, String> {
        let mut st = self.st.lock().unwrap();
        if expected.is_some_and(|g| self.gen.load(SeqCst) != g) {
            return Ok(json!({"ok": true}));
        }
        self.gen.fetch_add(1, SeqCst);
        if let Some(r) = st.run.take() {
            let mut restored = Ok(());
            if let Some(p) = &r.prev {
                restored = sysproxy::restore(p, &data_dir().join("proxy-recovery.json"));
            }
            if r.tun {
                let _ = self.api(
                    r.ctl,
                    &r.secret,
                    "PATCH",
                    "/configs",
                    Some(json!({"tun":{"enable":false}})),
                    3,
                );
            }
            // Killing the owned process closes every socket without waiting for a controller round trip.
            drop(r);
            st.settings.last_proxy.clear();
            let _ = save(&st.settings);
            restored.map_err(|e| {
                format!("The core stopped, but Windows proxy recovery is needed: {e}")
            })?;
        }
        Ok(json!({ "ok": true }))
    }

    pub fn set_rules(&self, rules: Vec<config::Rule>) -> Result<Value, String> {
        let mut st = self.st.lock().unwrap();
        let text = std::fs::read_to_string(data_dir().join("sub.yaml"))
            .map_err(|_| "Add a subscription first.".to_string())?;
        let sub = config::parse(&text)?;
        let mut rollback = None;
        if let Some(run) = st.run.as_mut() {
            let built = config::build(&sub, &rules, run.mixed, run.ctl, &run.secret, run.tun)?;
            let cfg = data_dir().join("core/config.yaml");
            let old = std::fs::read_to_string(&cfg).map_err(|e| e.to_string())?;
            std::fs::write(&cfg, &built.yaml).map_err(|e| e.to_string())?;
            if let Err(e) = self.api(
                run.ctl,
                &run.secret,
                "PUT",
                "/configs?force=false",
                Some(json!({"path": cfg.to_string_lossy()})),
                10,
            ) {
                std::fs::write(&cfg, &old).map_err(|e| e.to_string())?;
                return Err(e);
            }
            rollback = Some((cfg, old, run.ctl, run.secret.clone()));
        } else {
            config::build(&sub, &rules, 1, 2, "validation", false)?;
        }
        let before = std::mem::replace(&mut st.settings.rules, rules);
        if let Err(e) = save(&st.settings) {
            st.settings.rules = before;
            if let Some((cfg, old, ctl, secret)) = rollback {
                std::fs::write(&cfg, old).map_err(|e| e.to_string())?;
                self.api(
                    ctl,
                    &secret,
                    "PUT",
                    "/configs?force=false",
                    Some(json!({"path": cfg.to_string_lossy()})),
                    10,
                )?;
            }
            return Err(e);
        }
        Ok(json!({"ok": true}))
    }

    pub fn close_to_tray(&self) -> bool {
        !self.st.lock().unwrap().settings.quit_on_close
    }

    pub fn set_close_to_tray(&self, on: bool) -> Result<Value, String> {
        let mut st = self.st.lock().unwrap();
        st.settings.quit_on_close = !on;
        save(&st.settings)?;
        Ok(json!({"ok": true, "close_to_tray": on}))
    }

    pub fn set_mode(&self, mode: String) -> Result<Value, String> {
        if !["tun", "proxy"].contains(&mode.as_str()) {
            return Err("Unknown connection mode.".into());
        }
        let mut st = self.st.lock().unwrap();
        if st.run.is_some() {
            return Err("Disconnect before changing mode.".into());
        }
        st.settings.mode = mode;
        save(&st.settings)?;
        Ok(json!({"ok":true, "mode":st.settings.mode}))
    }

    pub fn set_subscription_id(&self, id: u64) -> Result<Value, String> {
        let mut st = self.st.lock().unwrap();
        if st.run.is_some() {
            return Err("Disconnect before switching subscriptions.".into());
        }
        if st.settings.sub_id != Some(id) {
            clear_provider_cache()?;
        }
        st.settings.sub_id = Some(id);
        st.settings.node.clear();
        save(&st.settings)?;
        drop(st);
        self.boot()
    }

    pub fn forget_subscription(&self) -> Result<(), String> {
        let mut st = self.st.lock().unwrap();
        if st.run.is_some() {
            return Err("Disconnect before signing out.".into());
        }
        for path in [
            data_dir().join("sub.yaml"),
            data_dir().join("core/config.yaml"),
            data_dir().join("core/cache.db"),
            data_dir().join("core/core.log"),
        ] {
            match std::fs::remove_file(path) {
                Ok(()) => (),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
                Err(e) => return Err(e.to_string()),
            }
        }
        clear_provider_cache()?;
        st.settings.sub_id = None;
        st.settings.sub_url.clear();
        self.invalidate_latency();
        st.settings.profile = crate::profile::Metadata::default();
        st.settings.node.clear();
        save(&st.settings)
    }

    pub fn proxies(&self) -> Result<Value, String> {
        let Some((ctl, secret, main, _)) = self.running() else {
            let st = self.st.lock().unwrap();
            let text = std::fs::read_to_string(data_dir().join("sub.yaml"))
                .map_err(|_| "Choose a subscription to load its servers.")?;
            let mut snapshot = cached_proxy_snapshot(&text, &st.settings.rules, &st.settings.node)?;
            drop(st);
            self.measured_snapshot(&mut snapshot);
            return Ok(snapshot);
        };
        let v = self.api(ctl, &secret, "GET", "/proxies", None, 4)?;
        let mut snapshot = proxy_snapshot(&v, &main);
        self.order_nodes(&mut snapshot);
        self.measured_snapshot(&mut snapshot);
        Ok(snapshot)
    }

    fn order_nodes(&self, snapshot: &mut Value) {
        let st = self.st.lock().unwrap();
        let Ok(text) = std::fs::read_to_string(data_dir().join("sub.yaml")) else {
            return;
        };
        let Ok(cached) = cached_proxy_snapshot(&text, &st.settings.rules, &st.settings.node) else {
            return;
        };
        let positions: HashMap<_, _> = cached["nodes"]
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
            .filter_map(|(i, n)| Some((n["name"].as_str()?.to_string(), i)))
            .collect();
        if let Some(nodes) = snapshot["nodes"].as_array_mut() {
            nodes.sort_by_key(|n| {
                positions
                    .get(n["name"].as_str().unwrap_or(""))
                    .copied()
                    .unwrap_or(usize::MAX)
            });
        }
    }

    fn start_probe(&self) -> Result<Probe, String> {
        self.ensure_profile_hwid()?;
        if self.closing.load(SeqCst) {
            return Err("The app is closing.".into());
        }
        let (rules, selected) = {
            let st = self.st.lock().unwrap();
            (st.settings.rules.clone(), st.settings.node.clone())
        };
        let directory = data_dir();
        let base = directory.canonicalize().map_err(|e| e.to_string())?;
        let text = std::fs::read_to_string(directory.join("sub.yaml"))
            .map_err(|_| "Choose a subscription to load its servers.")?;
        let mixed = free_port()?;
        let mut ctl = free_port()?;
        while ctl == mixed {
            ctl = free_port()?;
        }
        let secret = random_hex();
        let built = config::build(&config::parse(&text)?, &rules, mixed, ctl, &secret, false)?;
        // Go's geodata loader cannot join filenames onto Windows verbatim (\\?\) paths.
        let path = directory.join(format!("latency-{}", random_hex()));
        std::fs::create_dir(&path).map_err(|e| e.to_string())?;
        let files = ProbeDir { path, base };
        for name in ["geoip.metadb", "Country.mmdb", "GeoIP.dat", "GeoSite.dat"] {
            let cached = directory.join("core").join(name);
            if cached.is_file() {
                std::fs::copy(cached, files.path.join(name))
                    .map_err(|_| "Could not prepare the server test.")?;
            }
        }
        let cfg = files.path.join("config.yaml");
        std::fs::write(&cfg, &built.yaml).map_err(|e| e.to_string())?;
        let child = crate::process::Process::spawn(
            &core_exe()?,
            &files.path,
            &cfg,
            &data_dir().join("core/latency.log"),
            false,
        )?;
        let mut probe = Probe {
            run: Run {
                tun: false,
                child,
                ctl,
                mixed,
                secret,
                main: built.main,
                since: Instant::now(),
                prev: None,
            },
            _files: files,
        };
        while probe.run.since.elapsed() < Duration::from_secs(25) {
            if self.closing.load(SeqCst) {
                return Err("The app is closing.".into());
            }
            if probe
                .run
                .child
                .try_wait()
                .map_err(|e| e.to_string())?
                .is_some()
            {
                break;
            }
            if self
                .api(ctl, &probe.run.secret, "GET", "/version", None, 1)
                .is_ok()
            {
                if !selected.is_empty() {
                    let _ = self.api(
                        ctl,
                        &probe.run.secret,
                        "PUT",
                        &format!("/proxies/{}", enc(&probe.run.main)),
                        Some(json!({"name":selected})),
                        2,
                    );
                }
                return Ok(probe);
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        Err("Could not test servers. Try again later.".into())
    }

    pub fn select_node(&self, name: String) -> Result<Value, String> {
        let mut st = self.st.lock().unwrap();
        if let Some(run) = &st.run {
            self.api(
                run.ctl,
                &run.secret,
                "PUT",
                &format!("/proxies/{}", enc(&run.main)),
                Some(json!({ "name": name })),
                4,
            )?;
        } else {
            let text = std::fs::read_to_string(data_dir().join("sub.yaml"))
                .map_err(|_| "Choose a subscription to load its servers.")?;
            let snapshot = cached_proxy_snapshot(&text, &st.settings.rules, &st.settings.node)?;
            if !snapshot["nodes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|n| n["name"] == name)
            {
                return Err(
                    "This server is no longer in your subscription. Update it and try again."
                        .into(),
                );
            }
        }
        let previous = st.settings.node.clone();
        st.settings.node = name;
        if let Err(error) = save(&st.settings) {
            st.settings.node = previous;
            return Err(error);
        }
        Ok(json!({ "ok": true }))
    }

    fn invalidate_latency(&self) {
        let mut saved = self.latency.lock().unwrap();
        self.profile_revision.fetch_add(1, SeqCst);
        *saved = None;
    }
    fn measured_snapshot(&self, snapshot: &mut Value) {
        let saved = self.latency.lock().unwrap();
        let fresh = saved
            .as_ref()
            .filter(|values| values.fresh(self.profile_revision.load(SeqCst)));
        apply_url_measurements(snapshot, fresh.map(|values| &values.results));
    }

    fn measure_proxy(&self, ctl: u16, secret: &str, name: &str) -> Value {
        for url in [
            "https://www.gstatic.com/generate_204",
            "https://cp.cloudflare.com/generate_204",
        ] {
            if let Ok(result) = self.api(
                ctl,
                secret,
                "GET",
                &format!(
                    "/proxies/{}/delay?url={}&timeout=8000&expected=204",
                    enc(name),
                    enc(url)
                ),
                None,
                10,
            ) {
                if result["delay"].as_u64().is_some_and(|delay| delay > 0) {
                    return json!({"delay":result["delay"],"status":"ok","url":url});
                }
            }
        }
        json!({"delay":null,"status":"failed","url":null})
    }

    pub fn test_latency(&self) -> Result<Value, String> {
        if self.testing.swap(true, SeqCst) {
            return Err("A latency test is already running.".into());
        }
        let _testing = Testing(&self.testing);
        self.ensure_profile_hwid()?;
        let generation = self.gen.load(SeqCst);
        let revision = self.profile_revision.load(SeqCst);
        let probe;
        let (ctl, secret, main) = if let Some((ctl, secret, main, _)) = self.running() {
            (ctl, secret, main)
        } else {
            probe = self.start_probe()?;
            (
                probe.run.ctl,
                probe.run.secret.clone(),
                probe.run.main.clone(),
            )
        };
        let snapshot = self.api(ctl, &secret, "GET", "/proxies", None, 4)?;
        let all = &snapshot["proxies"];
        let visible: Vec<String> = all[&main]["all"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(str::to_string)
            .collect();
        let mut pending = visible.clone();
        let mut seen = std::collections::HashSet::new();
        let mut leaves = Vec::new();
        while let Some(name) = pending.pop() {
            if !seen.insert(name.clone()) {
                continue;
            }
            if let Some(members) = all[&name]["all"].as_array() {
                pending.extend(members.iter().filter_map(Value::as_str).map(str::to_string));
            } else {
                leaves.push(name);
            }
        }
        let groups: Vec<_> = visible
            .into_iter()
            .filter(|name| all[name]["all"].is_array())
            .collect();
        let results = Mutex::new(HashMap::new());
        // Test each leaf once; slow hosts do not hold up the next available worker.
        for targets in [&leaves, &groups] {
            let next = std::sync::atomic::AtomicUsize::new(0);
            std::thread::scope(|scope| {
                for _ in 0..16.min(targets.len()) {
                    let (next, results, secret) = (&next, &results, &secret);
                    scope.spawn(move || loop {
                        if self.closing.load(SeqCst) || self.gen.load(SeqCst) != generation {
                            break;
                        }
                        let Some(name) = targets.get(next.fetch_add(1, SeqCst)) else {
                            break;
                        };
                        let measured = self.measure_proxy(ctl, secret, name);
                        results.lock().unwrap().insert(name.clone(), measured);
                    });
                }
            });
        }
        if self.closing.load(SeqCst) || self.gen.load(SeqCst) != generation {
            return Err("Test cancelled.".into());
        }
        let results = results.into_inner().unwrap();
        let mut result =
            proxy_snapshot(&self.api(ctl, &secret, "GET", "/proxies", None, 4)?, &main);
        apply_url_measurements(&mut result, Some(&results));
        self.order_nodes(&mut result);
        let mut saved = self.latency.lock().unwrap();
        if self.profile_revision.load(SeqCst) != revision {
            return Err("Test cancelled.".into());
        }
        *saved = Some(Latency {
            revision,
            checked: Instant::now(),
            results,
        });
        Ok(result)
    }

    pub fn watch(&self, on: bool) {
        self.watch.store(on, SeqCst);
    }

    /// Read one traffic sample per second; only materialize connection details on their screen.
    fn ticker(self: Arc<Self>, app: tauri::AppHandle, gen: u64) {
        let mut last: HashMap<String, (u64, u64)> = HashMap::new();
        let mut misses = 0;
        while self.gen.load(SeqCst) == gen {
            let Some((ctl, secret, _, since)) = self.running() else {
                break;
            };
            let tick_start = Instant::now();
            let sample = (|| -> Result<Value, String> {
                let response = self
                    .local
                    .get(&format!("http://127.0.0.1:{ctl}/traffic"))
                    .set("Authorization", &format!("Bearer {secret}"))
                    .timeout(Duration::from_secs(2))
                    .call()
                    .map_err(|e| e.to_string())?;
                let mut line = String::new();
                std::io::BufReader::new(response.into_reader())
                    .read_line(&mut line)
                    .map_err(|e| e.to_string())?;
                serde_json::from_str(&line).map_err(|e| e.to_string())
            })();
            match sample {
                Ok(v) => {
                    misses = 0;
                    let mut conns = Value::Null;
                    if self.watch.load(SeqCst) {
                        if let Ok(list) = self.api(ctl, &secret, "GET", "/connections", None, 2) {
                            let mut now = HashMap::new();
                            let mut rows = Vec::new();
                            for c in list["connections"]
                                .as_array()
                                .into_iter()
                                .flatten()
                                .take(300)
                            {
                                let id = c["id"].as_str().unwrap_or("").to_string();
                                let (d, u) = (
                                    c["download"].as_u64().unwrap_or(0),
                                    c["upload"].as_u64().unwrap_or(0),
                                );
                                let (od, ou) = last.get(&id).copied().unwrap_or((d, u));
                                let m = &c["metadata"];
                                let host = ["host", "sniffHost", "destinationIP"]
                                    .iter()
                                    .filter_map(|k| m[*k].as_str())
                                    .find(|s| !s.is_empty())
                                    .unwrap_or("");
                                rows.push(json!({ "id": id, "host": host, "port": m["destinationPort"], "proc": m["process"], "dn": d.saturating_sub(od), "up": u.saturating_sub(ou), "chains": c["chains"], "rule": c["rule"], "payload": c["rulePayload"] }));
                                now.insert(id, (d, u));
                            }
                            last = now;
                            conns = Value::Array(rows);
                        }
                    } else {
                        last.clear();
                    }
                    if self.gen.load(SeqCst) != gen {
                        break;
                    }
                    let _ = app.emit("tick", json!({ "dn": v["down"], "up": v["up"], "secs": since.elapsed().as_secs(), "conns": conns }));
                }
                Err(_) => {
                    misses += 1;
                    if misses >= 3 {
                        let _ = self.stop(Some(gen));
                        if self.gen.load(SeqCst) == gen + 1 {
                            let _ = app.emit("dropped", Value::Null);
                        }
                        break;
                    }
                }
            }
            std::thread::sleep(Duration::from_secs(1).saturating_sub(tick_start.elapsed()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn latency_from_a_previous_profile_is_never_reused() {
        let old = Latency {
            revision: 1,
            checked: Instant::now(),
            results: HashMap::new(),
        };
        assert!(old.fresh(1));
        assert!(
            !old.fresh(2),
            "a new profile must not inherit results for reused node names"
        );
    }
    #[test]
    fn fresh_url_results_replace_unrelated_background_timeouts() {
        let mut snapshot = json!({"nodes":[{"name":"Working","delay":null,"status":"timeout"},{"name":"Unmeasured","delay":12,"status":"ok"}]});
        let measured = HashMap::from([(
            "Working".to_string(),
            json!({"delay":29,"status":"ok","url":"https://example.invalid/check"}),
        )]);
        apply_url_measurements(&mut snapshot, Some(&measured));
        assert_eq!(snapshot["nodes"][0]["delay"], 29);
        assert_eq!(snapshot["nodes"][0]["status"], "ok");
        assert_eq!(snapshot["nodes"][1]["status"], "unknown");
        apply_url_measurements(&mut snapshot, None);
        assert_eq!(snapshot["nodes"][0]["status"], "unknown");
    }
    #[test]
    fn cached_picker_preserves_groups_order_and_saved_choice_without_starting_core() {
        let sub = "proxies:\n- {name: First, type: direct}\n- {name: Second, type: direct}\nproxy-groups:\n- {name: Main, type: select, proxies: [First, Second]}\nrules: ['MATCH,Main']";
        let v = cached_proxy_snapshot(sub, &[], "Second").unwrap();
        assert_eq!(v["now"], "Second");
        assert_eq!(v["nodes"][0]["name"], "First");
        assert_eq!(v["nodes"][0]["status"], "unknown");
        assert_eq!(
            cached_proxy_snapshot(sub, &[], "Removed").unwrap()["now"],
            "First"
        );
    }
    #[test]
    fn automatic_group_uses_working_member_and_unknown_is_not_timeout() {
        let v = json!({"proxies":{
            "Main":{"type":"Selector","now":"Finland","all":["Finland","Unknown","Dead"]},
            "Finland":{"type":"URLTest","now":"Dead","all":["Dead","Live"]},
            "Dead":{"type":"Vless","history":[{"delay":0}]},
            "Live":{"type":"Vless","history":[{"delay":42}]},
            "Unknown":{"type":"Vless","history":[]}
        }});
        let result = proxy_snapshot(&v, "Main");
        assert_eq!(result["nodes"][0]["status"], "ok");
        assert_eq!(result["nodes"][0]["delay"], 42);
        assert_eq!(result["nodes"][0]["now"], "Live");
        assert_eq!(result["nodes"][1]["status"], "unknown");
        assert_eq!(result["nodes"][2]["status"], "timeout");
    }
    #[test]
    #[ignore = "Uses the signed-in user's cached subscription for real latency requests; changes no routes"]
    fn windows_cached_subscription_latency() {
        let engine = Engine::new();
        let snapshot = engine
            .test_latency()
            .expect("Cached subscription latency startup failed");
        let nodes = snapshot["nodes"].as_array().unwrap();
        let working = nodes.iter().filter(|node| node["status"] == "ok").count();
        println!("Measured {} servers; {working} working routes", nodes.len());
        println!(
            "Selected route health: {}",
            nodes
                .iter()
                .find(|node| node["name"] == snapshot["now"])
                .map(|node| node["status"].clone())
                .unwrap_or(Value::Null)
        );
        assert!(working > 0, "No working route in the cached subscription");
        assert!(
            engine.st.lock().unwrap().run.is_none(),
            "Latency must leave the VPN disconnected"
        );
    }

    #[test]
    #[ignore = "Requires administrator consent; creates a real Windows TUN adapter"]
    fn windows_tun_smoke() {
        use std::io::{Read, Write};
        use std::net::{TcpListener, TcpStream, UdpSocket};
        assert!(is_admin(), "Run this check as administrator");
        let original_appdata = std::env::var_os("APPDATA");
        let dir = std::env::temp_dir().join(format!("akimov-tun-test-{}", random_hex()));
        std::env::set_var("APPDATA", &dir);
        let engine = Engine::new();
        let _cleanup = Cleanup(&engine, original_appdata, dir);
        engine.set_mode("tun".into()).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        listener.set_nonblocking(true).unwrap();
        let done = Arc::new(AtomicBool::new(false));
        struct Stop(Arc<AtomicBool>);
        impl Drop for Stop {
            fn drop(&mut self) {
                self.0.store(true, SeqCst);
            }
        }
        let _stop = Stop(done.clone());
        let fixture = std::thread::spawn(move || {
            while !done.load(SeqCst) {
                let (mut stream, _) = match listener.accept() {
                    Ok(s) => s,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(10));
                        continue;
                    }
                    Err(_) => break,
                };
                let done = done.clone();
                std::thread::spawn(move || -> std::io::Result<()> {
                    stream.set_nonblocking(false)?;
                    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
                    let mut greeting = [0; 2];
                    stream.read_exact(&mut greeting)?;
                    let mut methods = vec![0; greeting[1] as usize];
                    stream.read_exact(&mut methods)?;
                    stream.write_all(&[5, 0])?;
                    let mut header = [0; 4];
                    stream.read_exact(&mut header)?;
                    let length = match header[3] {
                        1 => 4,
                        4 => 16,
                        3 => {
                            let mut n = [0];
                            stream.read_exact(&mut n)?;
                            n[0] as usize
                        }
                        _ => return Ok(()),
                    };
                    let mut address = vec![0; length + 2];
                    stream.read_exact(&mut address)?;
                    println!("SOCKS fixture command={}, address={address:?}", header[1]);
                    if header[1] == 3 {
                        let udp = UdpSocket::bind("127.0.0.1:0")?;
                        udp.set_read_timeout(Some(Duration::from_millis(200)))?;
                        let p = udp.local_addr()?.port();
                        stream.write_all(&[5, 0, 0, 1, 127, 0, 0, 1, (p >> 8) as u8, p as u8])?;
                        let mut packet = [0; 2048];
                        while !done.load(SeqCst) {
                            match udp.recv_from(&mut packet) {
                                Ok((n, peer)) if n >= 8 && &packet[n - 8..n] == b"ping-udp" => {
                                    packet[n - 8..n].copy_from_slice(b"pong-udp");
                                    udp.send_to(&packet[..n], peer)?;
                                }
                                Ok(_) => (),
                                Err(e)
                                    if [
                                        std::io::ErrorKind::WouldBlock,
                                        std::io::ErrorKind::TimedOut,
                                    ]
                                    .contains(&e.kind()) => {}
                                Err(e) => return Err(e),
                            }
                        }
                    } else {
                        stream.write_all(&[5, 0, 0, 1, 127, 0, 0, 1, 0, 0])?;
                        let mut ping = [0; 4];
                        stream.read_exact(&mut ping)?;
                        println!("SOCKS fixture received {ping:?}");
                        if &ping == b"ping" {
                            stream.write_all(b"pong")?;
                        }
                        let _ = stream.read(&mut ping);
                    }
                    Ok(())
                });
            }
        });
        std::fs::write(data_dir().join("sub.yaml"), format!("proxies:\n  - {{name: Fixture, type: socks5, server: 127.0.0.1, port: {port}, udp: true}}\nproxy-groups:\n  - {{name: Test, type: select, proxies: [Fixture]}}\nrules: ['MATCH,Test']\n")).unwrap();
        let (result, _) = engine.start().unwrap();
        assert!(tun_ready());
        println!("TUN connected in {} ms", result["ms"]);
        let (ctl, secret, _, _) = engine.running().unwrap();
        // The other VPN's DNS-leak filter owns port 53. Exercise our DNS interception on an isolated test port.
        let cfg = data_dir().join("core/config.yaml");
        let mut test_config: serde_yaml::Value =
            serde_yaml::from_str(&std::fs::read_to_string(&cfg).unwrap()).unwrap();
        test_config["tun"]["dns-hijack"] =
            serde_yaml::from_str("['203.0.113.53:5353', 'any:53', 'tcp://any:53']").unwrap();
        std::fs::write(&cfg, serde_yaml::to_string(&test_config).unwrap()).unwrap();
        engine
            .api(
                ctl,
                &secret,
                "PATCH",
                "/configs",
                Some(json!({"tun":{"enable":false}})),
                3,
            )
            .unwrap();
        engine
            .api(
                ctl,
                &secret,
                "PUT",
                "/configs?force=true",
                Some(json!({"path":cfg.to_string_lossy()})),
                5,
            )
            .unwrap();
        // Pin only documentation addresses to our test adapter; the user's other VPN stays untouched.
        let route = Command::new("powershell.exe").args(["-NoProfile", "-NonInteractive", "-Command", "New-NetRoute -DestinationPrefix '203.0.113.0/24' -InterfaceAlias 'akimov' -NextHop '198.19.0.2' -RouteMetric 0 -PolicyStore ActiveStore -ErrorAction Stop | Out-Null"])
            .creation_flags(0x0800_0000).output().unwrap();
        assert!(
            route.status.success(),
            "Could not route the test fixture: {}",
            String::from_utf8_lossy(&route.stderr)
        );
        engine
            .api(
                ctl,
                &secret,
                "PATCH",
                "/configs",
                Some(json!({"log-level":"debug"})),
                2,
            )
            .unwrap();
        let mut tcp = TcpStream::connect_timeout(
            &"203.0.113.10:443".parse().unwrap(),
            Duration::from_secs(5),
        )
        .unwrap();
        tcp.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        tcp.write_all(b"ping").unwrap();
        let mut pong = [0; 4];
        tcp.read_exact(&mut pong).unwrap_or_else(|e| {
            panic!(
                "TCP failed: {e}; log: {}",
                std::fs::read_to_string(data_dir().join("core/core.log")).unwrap_or_default()
            )
        });
        assert_eq!(&pong, b"pong");
        println!("PASS: raw TCP traversed TUN and the local SOCKS fixture");
        let udp = UdpSocket::bind("0.0.0.0:0").unwrap();
        udp.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        udp.send_to(b"ping-udp", "203.0.113.11:8888").unwrap();
        let mut packet = [0; 2048];
        let n = udp.recv(&mut packet).unwrap();
        assert_eq!(&packet[..n], b"pong-udp");
        println!("PASS: raw UDP traversed TUN and the local SOCKS fixture");
        let mut query = vec![0x51, 0x23, 1, 0, 0, 1, 0, 0, 0, 0, 0, 0];
        for label in ["tun-check", "invalid"] {
            query.push(label.len() as u8);
            query.extend_from_slice(label.as_bytes());
        }
        query.extend_from_slice(&[0, 0, 1, 0, 1]);
        let dns = UdpSocket::bind("0.0.0.0:0").unwrap();
        dns.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        dns.send_to(&query, "203.0.113.53:5353").unwrap();
        let n = dns.recv(&mut packet).unwrap_or_else(|e| {
            panic!(
                "DNS failed: {e}; log: {}",
                std::fs::read_to_string(data_dir().join("core/core.log")).unwrap_or_default()
            )
        });
        assert_eq!(&packet[..2], &[0x51, 0x23]);
        assert_ne!(packet[7], 0);
        assert_eq!(&packet[n - 4..n - 2], &[198, 19]);
        println!("PASS: DNS intercepted and resolved to the TUN fake-IP range");
        engine.disconnect().unwrap();
        let deadline = Instant::now() + Duration::from_secs(3);
        while tun_ready() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(25));
        }
        assert!(!tun_ready(), "TUN remained up after disconnect");
        let mut byte = [0];
        assert!(
            matches!(tcp.read(&mut byte), Ok(0) | Err(_)),
            "Tunneled socket survived disconnect"
        );
        drop(_stop);
        fixture.join().unwrap();
        println!("PASS: TUN removed and tunneled socket closed on disconnect");
    }
    struct Cleanup<'a>(&'a Engine, Option<std::ffi::OsString>, PathBuf);
    impl Drop for Cleanup<'_> {
        fn drop(&mut self) {
            let _ = self.0.disconnect();
            if let Some(value) = &self.1 {
                std::env::set_var("APPDATA", value);
            } else {
                std::env::remove_var("APPDATA");
            }
            let _ = std::fs::remove_dir_all(&self.2);
        }
    }

    #[test]
    fn windows_core_lifecycle_smoke() {
        // One isolated real-core test; restore the user's exact proxy even when an assertion fails.
        let original_appdata = std::env::var_os("APPDATA");
        let dir = std::env::temp_dir().join(format!("akimov-test-{}", random_hex()));
        std::env::set_var("APPDATA", &dir);
        let engine = Engine::new();
        engine.set_mode("proxy".into()).unwrap();
        let _cleanup = Cleanup(&engine, original_appdata, dir);
        let key = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER)
            .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Internet Settings")
            .unwrap();
        let before: Vec<_> = ["ProxyEnable", "ProxyServer", "ProxyOverride"]
            .iter()
            .map(|name| key.get_raw_value(name).ok())
            .collect();
        assert!(engine.start().is_err());
        std::fs::write(data_dir().join("sub.yaml"), "proxies:\n  - {name: local-fixture, type: socks5, server: 127.0.0.1, port: 9}\nproxy-groups:\n  - {name: Test, type: select, proxies: [local-fixture]}\nrules: [MATCH,DIRECT]\n").unwrap();
        // A rule is a YAML scalar, not two sequence items.
        let sub = std::fs::read_to_string(data_dir().join("sub.yaml"))
            .unwrap()
            .replace("rules: [MATCH,DIRECT]", "rules: ['MATCH,DIRECT']");
        std::fs::write(data_dir().join("sub.yaml"), sub).unwrap();
        let offline = engine.proxies().unwrap();
        let choice = offline["nodes"][0]["name"].as_str().unwrap().to_string();
        engine.select_node(choice.clone()).unwrap();
        assert_eq!(engine.proxies().unwrap()["now"], choice);
        assert!(engine.select_node("not-in-profile".into()).is_err());
        assert!(
            engine.running().is_none(),
            "Choosing a server must not start any connections"
        );
        let probe = engine.start_probe().unwrap();
        let probe_port = probe.run.ctl;
        assert!(!probe.run.tun);
        assert!(engine.running().is_none());
        assert!(engine
            .api(probe_port, &probe.run.secret, "GET", "/proxies", None, 2)
            .is_ok());
        let probe_dir = probe._files.path.clone();
        drop(probe);
        assert!(
            !probe_dir.exists(),
            "Temporary profiles must be removed after testing"
        );
        assert!(std::net::TcpStream::connect(("127.0.0.1", probe_port)).is_err());
        let measured = engine.test_latency().unwrap();
        assert!(!measured["nodes"].as_array().unwrap().is_empty());
        assert!(!engine.testing.load(SeqCst));
        assert!(engine.running().is_none());
        let after_probe: Vec<_> = ["ProxyEnable", "ProxyServer", "ProxyOverride"]
            .iter()
            .map(|name| key.get_raw_value(name).ok())
            .collect();
        assert_eq!(
            before, after_probe,
            "Offline latency testing must leave the Windows proxy untouched"
        );
        for _ in 0..3 {
            let (result, generation) = engine.start().unwrap();
            assert!(generation.is_some());
            assert!(engine.boot().unwrap()["connected"].as_bool().unwrap());
            assert!(engine.start().unwrap().1.is_none());
            let (ctl, secret, _, _) = engine.running().unwrap();
            let core_pid = engine.st.lock().unwrap().run.as_ref().unwrap().child.id();
            assert!(engine
                .api(ctl, "wrong-secret", "GET", "/version", None, 1)
                .is_err());
            assert!(engine.api(ctl, &secret, "GET", "/version", None, 1).is_ok());
            engine
                .set_rules(vec![config::Rule {
                    id: 1,
                    kind: "DOMAIN".into(),
                    value: "example.com".into(),
                    target: "@direct".into(),
                    on: true,
                }])
                .unwrap();
            engine.disconnect().unwrap();
            engine.disconnect().unwrap();
            assert!(std::net::TcpStream::connect(("127.0.0.1", ctl)).is_err());
            let handle = unsafe {
                windows_sys::Win32::System::Threading::OpenProcess(
                    windows_sys::Win32::System::Threading::PROCESS_QUERY_LIMITED_INFORMATION,
                    0,
                    core_pid,
                )
            };
            if !handle.is_null() {
                let mut code = 0;
                unsafe {
                    windows_sys::Win32::System::Threading::GetExitCodeProcess(handle, &mut code);
                    windows_sys::Win32::Foundation::CloseHandle(handle);
                }
                assert_ne!(code, 259, "core must not remain alive");
            }
            let after: Vec<_> = ["ProxyEnable", "ProxyServer", "ProxyOverride"]
                .iter()
                .map(|name| key.get_raw_value(name).ok())
                .collect();
            assert_eq!(before, after);
            assert!(!data_dir().join("proxy-recovery.json").exists());
            println!(
                "connect={}ms; sockets closed, child reaped, proxy restored",
                result["ms"]
            );
        }
        let providers = data_dir().join("core/providers");
        std::fs::create_dir_all(&providers).unwrap();
        std::fs::write(providers.join("profile.yaml"), "credential: fixture-only").unwrap();
        engine.forget_subscription().unwrap();
        assert!(
            !providers.exists(),
            "Provider credentials survived forgetting the account"
        );
        assert_eq!(engine.boot().unwrap()["has_sub"], false);
        engine.shutdown();
        assert!(engine.start().is_err());
    }
}
