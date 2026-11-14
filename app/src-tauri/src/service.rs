//! Installed whole-device broker. Its IPC accepts data and fixed operations, never privileged paths.
use serde_json::Value;

const PROTOCOL: u32 = 1;
const MAX_MESSAGE: usize = 4 * 1024 * 1024;

fn allowed_api(method: &str, path: &str, body: &Option<Value>) -> bool {
    fn segment(value: &str, proxy_name: bool) -> bool {
        if value.is_empty()
            || !value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.~%".contains(&b))
        {
            return false;
        }
        let mut decoded = Vec::new();
        let mut bytes = value.bytes();
        while let Some(byte) = bytes.next() {
            if byte == b'%' {
                let Some(high) = bytes.next().and_then(|b| (b as char).to_digit(16)) else {
                    return false;
                };
                let Some(low) = bytes.next().and_then(|b| (b as char).to_digit(16)) else {
                    return false;
                };
                decoded.push((high * 16 + low) as u8);
            } else {
                decoded.push(byte);
            }
        }
        decoded != b"."
            && decoded != b".."
            && !decoded
                .iter()
                .any(|b| b.is_ascii_control() || *b == b'\\' || (!proxy_name && *b == b'/'))
    }
    if method == "GET" && body.is_none() {
        if ["/version", "/proxies", "/connections", "/traffic"].contains(&path) {
            return true;
        }
        if let Some((node, query)) = path
            .strip_prefix("/proxies/")
            .and_then(|p| p.split_once("/delay?"))
        {
            let fields: Vec<_> = url::form_urlencoded::parse(query.as_bytes()).collect();
            return segment(node, true)
                && query.len() <= 4096
                && fields.len() == 3
                && fields.iter().filter(|(key, _)| key == "url").count() == 1
                && fields.iter().any(|(key, value)| {
                    key == "url"
                        && url::Url::parse(value).is_ok_and(|url| {
                            ["http", "https"].contains(&url.scheme())
                                && url.host_str().is_some()
                                && url.username().is_empty()
                                && url.password().is_none()
                        })
                })
                && fields
                    .iter()
                    .any(|(key, value)| key == "timeout" && value == "2000")
                && fields
                    .iter()
                    .any(|(key, value)| key == "expected" && value == "204");
        }
    }
    if method == "DELETE" && body.is_none() {
        return path
            .strip_prefix("/connections/")
            .is_some_and(|name| segment(name, false));
    }
    if method == "PATCH" && path == "/configs" {
        return body.as_ref() == Some(&serde_json::json!({"tun":{"enable":false}}));
    }
    let Some(object) = body
        .as_ref()
        .and_then(Value::as_object)
        .filter(|o| o.len() == 1)
    else {
        return false;
    };
    if method == "PUT" && path == "/configs?force=false" {
        return object
            .get("payload")
            .and_then(Value::as_str)
            .is_some_and(|s| s.len() < MAX_MESSAGE);
    }
    method == "PUT"
        && path
            .strip_prefix("/proxies/")
            .is_some_and(|name| segment(name, true))
        && object
            .get("name")
            .and_then(Value::as_str)
            .is_some_and(|s| !s.is_empty() && s.len() <= 4096)
}

fn mixed_port(yaml: &str) -> Result<u16, String> {
    let config: serde_yaml::Value =
        serde_yaml::from_str(yaml).map_err(|_| "Invalid VPN configuration.")?;
    config["mixed-port"]
        .as_u64()
        .and_then(|p| u16::try_from(p).ok())
        .filter(|p| *p > 0)
        .ok_or_else(|| "Invalid VPN proxy port.".into())
}

#[cfg(windows)]
pub use windows::{alive, api, early_main, start, stop};

#[cfg(windows)]
mod windows {
    use super::*;
    use serde::{Deserialize, Serialize};
    use sha2::{Digest, Sha256};
    use std::collections::HashSet;
    use std::fs::{File, OpenOptions};
    use std::io::{BufRead, BufReader, Read, Write};
    use std::os::windows::fs::MetadataExt;
    use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
    use std::path::{Path, PathBuf};
    use std::ptr::{null, null_mut};
    use std::sync::{
        atomic::{AtomicPtr, AtomicUsize, Ordering},
        Arc, Mutex, OnceLock,
    };
    use std::time::{Duration, Instant};
    use windows_sys::Win32::{
        Foundation::*,
        Security::Authorization::*,
        Security::*,
        Storage::FileSystem::*,
        System::{Com::*, JobObjects::*, Pipes::*, Services::*, Threading::*},
        UI::Shell::*,
    };

    // shortcut: one installing Windows user owns this helper; namespace by SID if multi-user installs are needed.
    const NAME: &str = "akimov-vpn-helper";
    const PIPE: &str = "\\\\.\\pipe\\akimov-vpn-helper-v1";
    const EXE: &str = "akimov-vpn-helper.exe";
    const FILES: [&str; 4] = ["mihomo.exe", "wintun.dll", "geoip.metadb", "GeoSite.dat"];
    static CLIENTS: OnceLock<Mutex<HashSet<(u16, String)>>> = OnceLock::new();
    static STOP: AtomicPtr<std::ffi::c_void> = AtomicPtr::new(null_mut());

    #[derive(Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Request {
        version: u32,
        ctl: u16,
        token: String,
        op: Operation,
    }
    #[derive(Serialize, Deserialize)]
    enum Operation {
        Start {
            yaml: String,
        },
        Alive,
        Stop,
        Api {
            method: String,
            path: String,
            body: Option<Value>,
            wait: u64,
        },
    }
    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(Some(0)).collect()
    }
    fn error() -> String {
        std::io::Error::last_os_error().to_string()
    }
    fn handle(value: HANDLE) -> Result<OwnedHandle, String> {
        if value.is_null() || value == INVALID_HANDLE_VALUE {
            Err(error())
        } else {
            Ok(unsafe { OwnedHandle::from_raw_handle(value as _) })
        }
    }
    fn raw(value: &OwnedHandle) -> HANDLE {
        value.as_raw_handle() as _
    }
    struct Service(SC_HANDLE);
    impl Drop for Service {
        fn drop(&mut self) {
            unsafe {
                CloseServiceHandle(self.0);
            }
        }
    }
    fn scm(access: u32) -> Result<Service, String> {
        let result = unsafe { OpenSCManagerW(null(), null(), access) };
        if result.is_null() {
            Err(error())
        } else {
            Ok(Service(result))
        }
    }
    fn open_service(access: u32) -> Result<Service, String> {
        let manager = scm(SC_MANAGER_CONNECT)?;
        let value = unsafe { OpenServiceW(manager.0, wide(NAME).as_ptr(), access) };
        if value.is_null() {
            Err(error())
        } else {
            Ok(Service(value))
        }
    }
    fn known_folder(id: &windows_sys::core::GUID) -> Result<PathBuf, String> {
        let mut value = null_mut();
        if unsafe { SHGetKnownFolderPath(id, 0, null_mut(), &mut value) } < 0 {
            return Err("Windows folders are unavailable.".into());
        }
        let mut length = 0;
        unsafe {
            while *value.add(length) != 0 {
                length += 1;
            }
            let result = PathBuf::from(String::from_utf16_lossy(std::slice::from_raw_parts(
                value, length,
            )));
            CoTaskMemFree(value.cast());
            Ok(result)
        }
    }
    fn install_dir() -> Result<PathBuf, String> {
        Ok(known_folder(&FOLDERID_ProgramFiles)?.join(NAME))
    }
    fn runtime_dir() -> Result<PathBuf, String> {
        Ok(known_folder(&FOLDERID_ProgramData)?.join(NAME))
    }
    fn no_reparse(path: &Path) -> Result<(), String> {
        if std::fs::symlink_metadata(path)
            .map_err(|e| e.to_string())?
            .file_attributes()
            & FILE_ATTRIBUTE_REPARSE_POINT
            != 0
        {
            return Err("The helper directory must not be a link.".into());
        }
        Ok(())
    }
    struct Descriptor(*mut std::ffi::c_void);
    impl Drop for Descriptor {
        fn drop(&mut self) {
            unsafe {
                LocalFree(self.0);
            }
        }
    }
    fn descriptor(sddl: &str) -> Result<Descriptor, String> {
        let mut value = null_mut();
        if unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                wide(sddl).as_ptr(),
                SDDL_REVISION_1,
                &mut value,
                null_mut(),
            )
        } == 0
        {
            Err(error())
        } else {
            Ok(Descriptor(value))
        }
    }
    fn create_secure_dir(path: &Path, readable: bool) -> Result<(), String> {
        let descriptor = descriptor(if readable {
            "O:BAG:BAD:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;GRGX;;;BU)"
        } else {
            "O:BAG:BAD:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)"
        })?;
        let attributes = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor.0,
            bInheritHandle: 0,
        };
        if unsafe { CreateDirectoryW(wide(&path.to_string_lossy()).as_ptr(), &attributes) } == 0 {
            return Err(error());
        }
        Ok(())
    }
    fn token_sid(process: HANDLE) -> Result<String, String> {
        let mut token = null_mut();
        if unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) } == 0 {
            return Err(error());
        }
        let token = handle(token)?;
        let mut size = 0;
        unsafe {
            GetTokenInformation(raw(&token), TokenUser, null_mut(), 0, &mut size);
        }
        let mut buffer = vec![0usize; (size as usize).div_ceil(std::mem::size_of::<usize>())];
        if unsafe {
            GetTokenInformation(
                raw(&token),
                TokenUser,
                buffer.as_mut_ptr().cast(),
                size,
                &mut size,
            )
        } == 0
        {
            return Err(error());
        }
        let user = unsafe { &*buffer.as_ptr().cast::<TOKEN_USER>() };
        let mut text = null_mut();
        if unsafe { ConvertSidToStringSidW(user.User.Sid, &mut text) } == 0 {
            return Err(error());
        }
        let result = unsafe {
            let mut n = 0;
            while *text.add(n) != 0 {
                n += 1;
            }
            String::from_utf16_lossy(std::slice::from_raw_parts(text, n))
        };
        unsafe {
            LocalFree(text.cast());
        }
        Ok(result)
    }
    fn current_sid() -> Result<String, String> {
        token_sid(unsafe { GetCurrentProcess() })
    }
    fn valid_sid(value: &str) -> bool {
        if !value.starts_with("S-1-")
            || value.len() >= 200
            || !value[4..].bytes().all(|b| b.is_ascii_digit() || b == b'-')
        {
            return false;
        }
        let mut sid = null_mut();
        if unsafe { ConvertStringSidToSidW(wide(value).as_ptr(), &mut sid) } == 0 {
            return false;
        }
        let valid = unsafe { IsValidSid(sid) } != 0;
        unsafe {
            LocalFree(sid);
        }
        valid
    }
    fn owner() -> Result<String, String> {
        let dir = install_dir()?;
        no_reparse(&dir)?;
        let sid = std::fs::read_to_string(dir.join("owner.sid"))
            .map_err(|_| "The whole-device helper is not installed.")?;
        if !valid_sid(&sid) {
            return Err("Invalid helper owner.".into());
        }
        Ok(sid)
    }
    fn elevated(argument: &str) -> Result<(), String> {
        let app = wide(
            &std::env::current_exe()
                .map_err(|e| e.to_string())?
                .to_string_lossy(),
        );
        let verb = wide("runas");
        let argument = wide(argument);
        let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
        info.cbSize = std::mem::size_of_val(&info) as u32;
        info.fMask = SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC;
        info.lpVerb = verb.as_ptr();
        info.lpFile = app.as_ptr();
        info.lpParameters = argument.as_ptr();
        info.nShow = 0;
        let com = unsafe {
            CoInitializeEx(
                null(),
                (COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) as u32,
            )
        };
        let launched = unsafe { ShellExecuteExW(&mut info) };
        let code = unsafe { GetLastError() };
        if com >= 0 {
            unsafe {
                CoUninitialize();
            }
        }
        if launched == 0 {
            return Err(if code == ERROR_CANCELLED {
                "elevation_cancelled".into()
            } else {
                std::io::Error::from_raw_os_error(code as i32).to_string()
            });
        }
        let process = handle(info.hProcess)?;
        if unsafe { WaitForSingleObject(raw(&process), 120_000) } != WAIT_OBJECT_0 {
            return Err("Helper installation timed out.".into());
        }
        let mut code = 1;
        unsafe {
            GetExitCodeProcess(raw(&process), &mut code);
        }
        if code != 0 {
            Err("The whole-device helper could not be installed or removed. Reinstall the app and try again.".into())
        } else {
            Ok(())
        }
    }
    pub fn early_main() -> Option<Result<(), String>> {
        for argument in std::env::args().skip(1) {
            if argument == "--vpn-service" {
                return Some(run_service());
            }
            if let Some(sid) = argument.strip_prefix("--install-helper=") {
                return Some(if crate::core::is_admin() {
                    install(sid)
                } else {
                    Err("Administrator access is required.".into())
                });
            }
            if argument == "--remove-helper" {
                return Some((|| {
                    if !install_dir()?.exists() {
                        return Ok(());
                    }
                    if !crate::core::is_admin() {
                        if owner()? != current_sid()? {
                            return Err("The helper belongs to another Windows user.".into());
                        }
                        return elevated("--remove-helper");
                    }
                    remove()
                })());
            }
        }
        None
    }
    fn digest(path: &Path) -> Result<String, String> {
        let mut file = File::open(path).map_err(|e| e.to_string())?;
        let mut hash = Sha256::new();
        let mut buffer = [0; 65536];
        loop {
            let n = file.read(&mut buffer).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            hash.update(&buffer[..n]);
        }
        Ok(format!("{:x}", hash.finalize()))
    }
    fn copy_contents(source: &Path, target: &Path) -> Result<(), String> {
        let mut source = File::open(source).map_err(|e| e.to_string())?;
        // New files inherit the protected destination ACL, never source-file permissions.
        let mut target = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(target)
            .map_err(|e| e.to_string())?;
        std::io::copy(&mut source, &mut target).map_err(|e| e.to_string())?;
        Ok(())
    }
    fn install(sid: &str) -> Result<(), String> {
        if !valid_sid(sid) {
            return Err("Invalid helper owner.".into());
        }
        let target = install_dir()?;
        if target.exists() || open_service(SERVICE_QUERY_STATUS).is_ok() {
            return Err("A helper is already installed. Remove it before reinstalling.".into());
        }
        let source = std::env::current_exe().map_err(|e| e.to_string())?;
        let core = crate::core::core_exe()?
            .parent()
            .ok_or("The core is missing.")?
            .to_path_buf();
        let manifest: Value = serde_json::from_str(include_str!("../core/manifest.json"))
            .map_err(|e| e.to_string())?;
        let stage = target.with_file_name(format!("{NAME}-install-{}", crate::core::random_hex()));
        let security = descriptor(&format!("D:P(A;;GA;;;SY)(A;;GA;;;BA)(A;;LCRP;;;{sid})"))?;
        create_secure_dir(&stage, true)?;
        let mut published = false;
        let copied = (|| {
            copy_contents(&source, &stage.join(EXE))?;
            for name in FILES {
                let file = stage.join(name);
                copy_contents(&core.join(name), &file)
                    .map_err(|_| format!("The bundled {name} is missing. Reinstall the app."))?;
                let expected = match name {
                    "mihomo.exe" => manifest["binary_sha256"].as_str().unwrap_or("").to_string(),
                    "geoip.metadb" => manifest["geoip_sha256"].as_str().unwrap_or("").to_string(),
                    "GeoSite.dat" => manifest["geosite_sha256"]
                        .as_str()
                        .unwrap_or("")
                        .to_string(),
                    _ => format!("{:x}", Sha256::digest(include_bytes!("../core/wintun.dll"))),
                };
                if digest(&file)? != expected {
                    return Err(format!(
                        "The bundled {name} failed verification. Reinstall the app."
                    ));
                }
            }
            std::fs::write(stage.join("owner.sid"), sid).map_err(|e| e.to_string())?;
            std::fs::rename(&stage, &target).map_err(|e| e.to_string())?;
            published = true;
            let manager = scm(SC_MANAGER_CREATE_SERVICE)?;
            let image = wide(&format!("\"{}\" --vpn-service", target.join(EXE).display()));
            let service = unsafe {
                CreateServiceW(
                    manager.0,
                    wide(NAME).as_ptr(),
                    wide("akimov vpn whole-device helper").as_ptr(),
                    SERVICE_ALL_ACCESS,
                    SERVICE_WIN32_OWN_PROCESS,
                    SERVICE_DEMAND_START,
                    SERVICE_ERROR_NORMAL,
                    image.as_ptr(),
                    null(),
                    null_mut(),
                    null(),
                    null(),
                    null(),
                )
            };
            if service.is_null() {
                return Err(error());
            }
            let service = Service(service);
            if unsafe { SetServiceObjectSecurity(service.0, DACL_SECURITY_INFORMATION, security.0) }
                == 0
            {
                unsafe {
                    DeleteService(service.0);
                }
                return Err(error());
            }
            Ok(())
        })();
        if copied.is_err() {
            let _ = std::fs::remove_dir_all(&stage);
            if published {
                let _ = std::fs::remove_dir_all(&target);
            }
        }
        copied
    }
    fn service_pid(service: &Service) -> Result<u32, String> {
        let mut status: SERVICE_STATUS_PROCESS = unsafe { std::mem::zeroed() };
        let mut size = 0;
        if unsafe {
            QueryServiceStatusEx(
                service.0,
                SC_STATUS_PROCESS_INFO,
                std::ptr::from_mut(&mut status).cast(),
                std::mem::size_of_val(&status) as u32,
                &mut size,
            )
        } == 0
        {
            Err(error())
        } else {
            Ok(if status.dwCurrentState == SERVICE_STOPPED {
                0
            } else {
                status.dwProcessId
            })
        }
    }
    fn ensure_running() -> Result<(), String> {
        let sid = current_sid()?;
        match open_service(SERVICE_START | SERVICE_QUERY_STATUS) {
            Ok(_) => {}
            Err(_) => {
                // Only absence installs; access failures must never turn into another elevation prompt.
                if install_dir()?.exists() {
                    return Err(
                        "The helper is unavailable for this Windows user. Reinstall the app."
                            .into(),
                    );
                }
                elevated(&format!("--install-helper={sid}"))?;
            }
        }
        if owner()? != sid {
            return Err("The helper belongs to another Windows user.".into());
        }
        let service = open_service(SERVICE_START | SERVICE_QUERY_STATUS)?;
        if unsafe { StartServiceW(service.0, 0, null()) } == 0
            && unsafe { GetLastError() } != ERROR_SERVICE_ALREADY_RUNNING
        {
            return Err(error());
        }
        Ok(())
    }
    fn write_message(file: &mut File, value: &impl Serialize) -> Result<(), String> {
        let bytes = serde_json::to_vec(value).map_err(|e| e.to_string())?;
        if bytes.len() > MAX_MESSAGE {
            return Err("The helper message is too large.".into());
        }
        file.write_all(&(bytes.len() as u32).to_le_bytes())
            .and_then(|_| file.write_all(&bytes))
            .map_err(|e| e.to_string())
    }
    fn read_message<T: serde::de::DeserializeOwned>(file: &mut File) -> Result<T, String> {
        let mut length = [0; 4];
        file.read_exact(&mut length).map_err(|e| e.to_string())?;
        let length = u32::from_le_bytes(length) as usize;
        if length == 0 || length > MAX_MESSAGE {
            return Err("Invalid helper message length.".into());
        }
        let mut bytes = vec![0; length];
        file.read_exact(&mut bytes).map_err(|e| e.to_string())?;
        serde_json::from_slice(&bytes).map_err(|_| "Invalid helper message.".into())
    }
    fn call(ctl: u16, token: &str, op: Operation) -> Result<Value, String> {
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut file = loop {
            let value = unsafe {
                CreateFileW(
                    wide(PIPE).as_ptr(),
                    FILE_GENERIC_READ | FILE_WRITE_DATA,
                    FILE_SHARE_READ | FILE_SHARE_WRITE,
                    null(),
                    OPEN_EXISTING,
                    SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION,
                    null_mut(),
                )
            };
            if value != INVALID_HANDLE_VALUE {
                break unsafe { File::from_raw_handle(value as _) };
            }
            let code = unsafe { GetLastError() };
            if ![ERROR_PIPE_BUSY, ERROR_FILE_NOT_FOUND].contains(&code)
                || Instant::now() >= deadline
            {
                return Err(
                    "The whole-device helper did not answer. Restart the app and try again.".into(),
                );
            }
            std::thread::sleep(Duration::from_millis(25));
        };
        let mut pid = 0;
        let service = open_service(SERVICE_QUERY_STATUS)?;
        if unsafe { GetNamedPipeServerProcessId(file.as_raw_handle() as _, &mut pid) } == 0
            || pid == 0
            || service_pid(&service)? != pid
        {
            return Err("The helper connection could not be authenticated.".into());
        }
        write_message(
            &mut file,
            &Request {
                version: PROTOCOL,
                ctl,
                token: token.into(),
                op,
            },
        )?;
        read_message::<Result<Value, String>>(&mut file)?
    }
    fn clients() -> &'static Mutex<HashSet<(u16, String)>> {
        CLIENTS.get_or_init(|| Mutex::new(HashSet::new()))
    }
    pub fn start(yaml: &str) -> Result<(u16, String), String> {
        let (ctl, token) = public_identity(yaml)?;
        ensure_running()?;
        if let Err(error) = call(ctl, &token, Operation::Start { yaml: yaml.into() }) {
            // A lost response must not leave a running tunnel without its UI owner.
            let _ = call(ctl, &token, Operation::Stop);
            return Err(error);
        }
        clients().lock().unwrap().insert((ctl, token.clone()));
        Ok((ctl, token))
    }
    pub fn stop(ctl: u16, token: &str) -> Result<(), String> {
        let result = call(ctl, token, Operation::Stop).map(|_| ());
        clients().lock().unwrap().remove(&(ctl, token.into()));
        result
    }
    pub fn alive(ctl: u16, token: &str) -> Result<bool, String> {
        call(ctl, token, Operation::Alive).map(|v| v.as_bool().unwrap_or(false))
    }
    pub fn api(
        ctl: u16,
        token: &str,
        method: &str,
        path: &str,
        body: Option<&Value>,
        wait: u64,
    ) -> Option<Result<Value, String>> {
        if !clients().lock().unwrap().contains(&(ctl, token.into())) {
            return None;
        }
        Some(call(
            ctl,
            token,
            Operation::Api {
                method: method.into(),
                path: path.into(),
                body: body.cloned(),
                wait,
            },
        ))
    }
    fn public_identity(yaml: &str) -> Result<(u16, String), String> {
        if yaml.len() > MAX_MESSAGE {
            return Err("The VPN profile is too large.".into());
        }
        let config: serde_yaml::Value =
            serde_yaml::from_str(yaml).map_err(|_| "Invalid VPN configuration.")?;
        let address: std::net::SocketAddr = config["external-controller"]
            .as_str()
            .ok_or("Missing VPN controller.")?
            .parse()
            .map_err(|_| "Invalid VPN controller.")?;
        let token = config["secret"].as_str().ok_or("Missing VPN secret.")?;
        if address.ip() != std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)
            || address.port() == 0
            || token.len() != 32
            || !token.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err("Invalid VPN controller identity.".into());
        }
        Ok((address.port(), token.into()))
    }
    struct Core {
        process: OwnedHandle,
        _job: OwnedHandle,
    }
    impl Core {
        fn stop(&self) {
            unsafe {
                TerminateJobObject(raw(&self._job), 1);
                WaitForSingleObject(raw(&self.process), 5000);
            }
        }
        fn running(&self) -> bool {
            unsafe { WaitForSingleObject(raw(&self.process), 0) == WAIT_TIMEOUT }
        }
    }
    impl Drop for Core {
        fn drop(&mut self) {
            self.stop();
        }
    }
    fn spawn_core(dir: &Path) -> Result<Core, String> {
        let exe = install_dir()?.join("mihomo.exe");
        let job = handle(unsafe { CreateJobObjectW(null(), null()) })?;
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if unsafe {
            SetInformationJobObject(
                raw(&job),
                JobObjectExtendedLimitInformation,
                std::ptr::from_ref(&limits).cast(),
                std::mem::size_of_val(&limits) as u32,
            )
        } == 0
        {
            return Err(error());
        }
        let log = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(dir.join("core.log"))
            .map_err(|e| e.to_string())?;
        let input = File::open("NUL").map_err(|e| e.to_string())?;
        for file in [&log, &input] {
            if unsafe {
                SetHandleInformation(
                    file.as_raw_handle() as _,
                    HANDLE_FLAG_INHERIT,
                    HANDLE_FLAG_INHERIT,
                )
            } == 0
            {
                return Err(error());
            }
        }
        let windows = known_folder(&FOLDERID_Windows)?;
        let environment = format!(
            "PATH={}\\System32\0SystemRoot={}\0TEMP={}\0TMP={}\0WINDIR={}\0\0",
            windows.display(),
            windows.display(),
            dir.display(),
            dir.display(),
            windows.display()
        )
        .encode_utf16()
        .collect::<Vec<_>>();
        let mut command = wide(&format!(
            "\"{}\" -d \"{}\" -f \"{}\"",
            exe.display(),
            dir.display(),
            dir.join("config.yaml").display()
        ));
        let mut info: STARTUPINFOW = unsafe { std::mem::zeroed() };
        info.cb = std::mem::size_of_val(&info) as u32;
        info.dwFlags = STARTF_USESTDHANDLES;
        info.hStdInput = input.as_raw_handle() as _;
        info.hStdOutput = log.as_raw_handle() as _;
        info.hStdError = log.as_raw_handle() as _;
        let mut created: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };
        if unsafe {
            CreateProcessW(
                wide(&exe.to_string_lossy()).as_ptr(),
                command.as_mut_ptr(),
                null(),
                null(),
                1,
                CREATE_NO_WINDOW | CREATE_SUSPENDED | CREATE_UNICODE_ENVIRONMENT,
                environment.as_ptr().cast(),
                wide(&dir.to_string_lossy()).as_ptr(),
                &info,
                &mut created,
            )
        } == 0
        {
            return Err(error());
        }
        let process = handle(created.hProcess)?;
        let thread = handle(created.hThread)?;
        if unsafe { AssignProcessToJobObject(raw(&job), raw(&process)) } == 0
            || unsafe { ResumeThread(raw(&thread)) } == u32::MAX
        {
            unsafe {
                TerminateProcess(raw(&process), 1);
                WaitForSingleObject(raw(&process), 5000);
            }
            return Err(error());
        }
        Ok(Core { process, _job: job })
    }
    struct Session {
        ctl: u16,
        token: String,
        mixed: u16,
        owner: OwnedHandle,
        owner_pid: u32,
        core: Core,
        private_ctl: u16,
        private_token: String,
        dir: PathBuf,
        reload: Mutex<()>,
    }
    impl Session {
        fn active(&self) -> bool {
            self.core.running()
                && unsafe { WaitForSingleObject(raw(&self.owner), 0) == WAIT_TIMEOUT }
        }
        fn request(
            &self,
            method: &str,
            path: &str,
            body: Option<&Value>,
            wait: u64,
        ) -> Result<Value, String> {
            let agent = ureq::AgentBuilder::new().try_proxy_from_env(false).build();
            let request = agent
                .request(
                    method,
                    &format!("http://127.0.0.1:{}{path}", self.private_ctl),
                )
                .set("Authorization", &format!("Bearer {}", self.private_token))
                .timeout(Duration::from_secs(wait.clamp(1, 30)));
            let response = match body {
                Some(body) => request.send_json(body),
                None => request.call(),
            }
            .map_err(|_| "The VPN core did not answer.")?;
            if path == "/traffic" {
                let mut line = String::new();
                BufReader::new(response.into_reader())
                    .take(65536)
                    .read_line(&mut line)
                    .map_err(|e| e.to_string())?;
                return serde_json::from_str(&line)
                    .map_err(|_| "Invalid VPN traffic response.".into());
            }
            if response.status() == 204 {
                return Ok(serde_json::json!({}));
            }
            response
                .into_json()
                .map_err(|_| "Invalid VPN core response.".into())
        }
    }
    impl Drop for Session {
        fn drop(&mut self) {
            self.core.stop();
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }
    fn free_port() -> Result<u16, String> {
        std::net::TcpListener::bind("127.0.0.1:0")
            .and_then(|l| l.local_addr())
            .map(|a| a.port())
            .map_err(|e| e.to_string())
    }
    type State = Mutex<Option<Arc<Session>>>;
    fn process_request(request: Request, pid: u32, state: &State) -> Result<Value, String> {
        if request.version != PROTOCOL {
            return Err("The whole-device helper needs an update. Reinstall the app.".into());
        }
        let stale = {
            let mut state = state.lock().map_err(|_| "The helper failed.")?;
            if state.as_ref().is_some_and(|s| !s.active()) {
                state.take()
            } else {
                None
            }
        };
        drop(stale);
        if let Operation::Start { yaml } = request.op {
            let mut state = state.lock().map_err(|_| "The helper failed.")?;
            if state.is_some() {
                return Err("Whole-device mode is already connected.".into());
            }
            let (ctl, token) = public_identity(&yaml)?;
            let mixed = mixed_port(&yaml)?;
            if mixed == ctl {
                return Err("The VPN controller and proxy ports must differ.".into());
            }
            if ctl != request.ctl || token != request.token {
                return Err("Invalid VPN session.".into());
            }
            let parent = handle(unsafe {
                OpenProcess(
                    SYNCHRONIZATION_SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION,
                    0,
                    pid,
                )
            })?;
            let mut private_ctl = free_port()?;
            while private_ctl == ctl || private_ctl == mixed {
                private_ctl = free_port()?;
            }
            let private_token = crate::core::random_hex();
            let yaml = crate::config::privileged_config(&yaml, private_ctl, &private_token)?;
            let dir = runtime_dir()?.join(crate::core::random_hex());
            create_secure_dir(&dir, false)?;
            let install = install_dir()?;
            for name in ["geoip.metadb", "GeoSite.dat"] {
                copy_contents(&install.join(name), &dir.join(name))?;
            }
            std::fs::write(dir.join("config.yaml"), yaml).map_err(|e| e.to_string())?;
            let core = spawn_core(&dir)?;
            let core_pid = unsafe { GetProcessId(raw(&core.process)) };
            *state = Some(Arc::new(Session {
                ctl,
                token,
                mixed,
                owner: parent,
                owner_pid: pid,
                core,
                private_ctl,
                private_token,
                dir,
                reload: Mutex::new(()),
            }));
            return Ok(serde_json::json!({"pid":core_pid}));
        }
        let session = state.lock().map_err(|_| "The helper failed.")?.clone();
        let Some(session) = session else {
            return match request.op {
                Operation::Alive => Ok(Value::Bool(false)),
                Operation::Stop => Ok(serde_json::json!({})),
                _ => Err("The VPN session has stopped.".into()),
            };
        };
        if session.ctl != request.ctl || session.token != request.token || session.owner_pid != pid
        {
            return Err("Invalid VPN session.".into());
        }
        match request.op {
            Operation::Alive => Ok(Value::Bool(session.active())),
            Operation::Stop => {
                let stopped = {
                    let mut state = state.lock().map_err(|_| "The helper failed.")?;
                    if state
                        .as_ref()
                        .is_some_and(|current| Arc::ptr_eq(current, &session))
                    {
                        state.take()
                    } else {
                        None
                    }
                };
                session.core.stop();
                drop(stopped);
                Ok(serde_json::json!({}))
            }
            Operation::Api {
                method,
                path,
                body,
                wait,
            } => {
                if !allowed_api(&method, &path, &body) {
                    return Err("This helper operation is not allowed.".into());
                }
                if !session.active() {
                    return Err("The VPN session has stopped.".into());
                }
                if method == "PUT" && path == "/configs?force=false" {
                    let _reload = session
                        .reload
                        .lock()
                        .map_err(|_| "The VPN reload failed.")?;
                    let payload = body
                        .as_ref()
                        .and_then(|b| b["payload"].as_str())
                        .ok_or("Missing VPN configuration.")?;
                    if mixed_port(payload)? != session.mixed {
                        return Err("The VPN proxy port cannot change during a session.".into());
                    }
                    let yaml = crate::config::privileged_config(
                        payload,
                        session.private_ctl,
                        &session.private_token,
                    )?;
                    let cfg = session.dir.join("config.yaml");
                    let previous = std::fs::read(&cfg).map_err(|e| e.to_string())?;
                    std::fs::write(&cfg, &yaml).map_err(|e| e.to_string())?;
                    let result = session.request(
                        "PUT",
                        "/configs?force=false",
                        Some(&serde_json::json!({"payload":yaml})),
                        wait,
                    );
                    if result.is_err() {
                        std::fs::write(&cfg, previous).map_err(|e| e.to_string())?;
                    }
                    result
                } else {
                    session.request(&method, &path, body.as_ref(), wait)
                }
            }
            Operation::Start { .. } => unreachable!(),
        }
    }
    fn serve_client(pipe: OwnedHandle, sid: &str, state: &State) {
        let mut pid = 0;
        if unsafe { GetNamedPipeClientProcessId(raw(&pipe), &mut pid) } == 0 {
            return;
        }
        let parent = match handle(unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) })
        {
            Ok(p) => p,
            Err(_) => return,
        };
        if token_sid(raw(&parent)).ok().as_deref() != Some(sid) {
            return;
        }
        let mut file = File::from(pipe);
        let response =
            read_message::<Request>(&mut file).and_then(|r| process_request(r, pid, state));
        let _ = write_message(&mut file, &response);
        unsafe {
            FlushFileBuffers(file.as_raw_handle() as _);
            DisconnectNamedPipe(file.as_raw_handle() as _);
        }
    }
    fn pipe_loop(sid: String, state: Arc<State>) -> Result<(), String> {
        // FILE_WRITE_DATA allows requests without granting clients FILE_CREATE_PIPE_INSTANCE.
        let security = descriptor(&format!("D:P(A;;GA;;;SY)(A;;GA;;;BA)(A;;0x12008b;;;{sid})"))?;
        let attributes = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: security.0,
            bInheritHandle: 0,
        };
        let create = |first| {
            handle(unsafe {
                CreateNamedPipeW(
                    wide(PIPE).as_ptr(),
                    PIPE_ACCESS_DUPLEX
                        | if first {
                            FILE_FLAG_FIRST_PIPE_INSTANCE
                        } else {
                            0
                        },
                    PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                    64,
                    65536,
                    65536,
                    1000,
                    &attributes,
                )
            })
        };
        let mut listener = create(true)?;
        let workers = Arc::new(AtomicUsize::new(0));
        loop {
            if unsafe { ConnectNamedPipe(raw(&listener), null_mut()) } == 0
                && unsafe { GetLastError() } != ERROR_PIPE_CONNECTED
            {
                continue;
            }
            // Keep one server instance open continuously so another process cannot squat on the pipe name.
            let next = create(false)?;
            let accepted = std::mem::replace(&mut listener, next);
            if workers.load(Ordering::SeqCst) >= 32 {
                unsafe {
                    DisconnectNamedPipe(raw(&accepted));
                }
                continue;
            }
            workers.fetch_add(1, Ordering::SeqCst);
            let workers = workers.clone();
            let state = state.clone();
            let sid = sid.clone();
            std::thread::spawn(move || {
                serve_client(accepted, &sid, &state);
                workers.fetch_sub(1, Ordering::SeqCst);
            });
        }
    }
    unsafe extern "system" fn control(code: u32) {
        if code == SERVICE_CONTROL_STOP || code == SERVICE_CONTROL_SHUTDOWN {
            let event = STOP.load(Ordering::SeqCst);
            if !event.is_null() {
                SetEvent(event);
            }
        }
    }
    fn status(handle: SERVICE_STATUS_HANDLE, state: u32, error: u32) {
        let status = SERVICE_STATUS {
            dwServiceType: SERVICE_WIN32_OWN_PROCESS,
            dwCurrentState: state,
            dwControlsAccepted: if state == SERVICE_RUNNING {
                SERVICE_ACCEPT_STOP | SERVICE_ACCEPT_SHUTDOWN
            } else {
                0
            },
            dwWin32ExitCode: error,
            dwServiceSpecificExitCode: 0,
            dwCheckPoint: 0,
            dwWaitHint: if state == SERVICE_START_PENDING {
                10000
            } else {
                0
            },
        };
        unsafe {
            SetServiceStatus(handle, &status);
        }
    }
    unsafe extern "system" fn service_main(_: u32, _: *mut *mut u16) {
        let status_handle = RegisterServiceCtrlHandlerW(wide(NAME).as_ptr(), Some(control));
        if status_handle.is_null() {
            return;
        }
        status(status_handle, SERVICE_START_PENDING, 0);
        let result = (|| {
            let sid = owner()?;
            let stop = handle(CreateEventW(null(), 1, 0, null()))?;
            STOP.store(raw(&stop), Ordering::SeqCst);
            let runtime = runtime_dir()?;
            if runtime.exists() {
                no_reparse(&runtime)?;
                std::fs::remove_dir_all(&runtime).map_err(|e| e.to_string())?;
            }
            create_secure_dir(&runtime, false)?;
            let state = Arc::new(Mutex::new(None::<Arc<Session>>));
            let worker_state = state.clone();
            let stop_raw = raw(&stop) as usize;
            std::thread::spawn(move || {
                let _ = pipe_loop(sid, worker_state);
                SetEvent(stop_raw as HANDLE);
            });
            status(status_handle, SERVICE_RUNNING, 0);
            while WaitForSingleObject(raw(&stop), 100) == WAIT_TIMEOUT {
                let stale = if let Ok(mut session) = state.try_lock() {
                    if session.as_ref().is_some_and(|s| !s.active()) {
                        session.take()
                    } else {
                        None
                    }
                } else {
                    None
                };
                if let Some(session) = stale {
                    session.core.stop();
                }
            }
            let session = state.lock().ok().and_then(|mut s| s.take());
            if let Some(session) = session {
                session.core.stop();
            }
            STOP.store(null_mut(), Ordering::SeqCst);
            Ok::<(), String>(())
        })();
        status(
            status_handle,
            SERVICE_STOPPED,
            if result.is_ok() {
                0
            } else {
                ERROR_SERVICE_SPECIFIC_ERROR
            },
        );
    }
    fn run_service() -> Result<(), String> {
        let mut name = wide(NAME);
        let table = [
            SERVICE_TABLE_ENTRYW {
                lpServiceName: name.as_mut_ptr(),
                lpServiceProc: Some(service_main),
            },
            SERVICE_TABLE_ENTRYW {
                lpServiceName: null_mut(),
                lpServiceProc: None,
            },
        ];
        if unsafe { StartServiceCtrlDispatcherW(table.as_ptr()) } == 0 {
            Err(error())
        } else {
            Ok(())
        }
    }
    fn remove() -> Result<(), String> {
        let target = install_dir()?;
        no_reparse(&target)?;
        let manager = scm(SC_MANAGER_CONNECT)?;
        let service = unsafe {
            OpenServiceW(
                manager.0,
                wide(NAME).as_ptr(),
                SERVICE_STOP | SERVICE_QUERY_STATUS | DELETE,
            )
        };
        if service.is_null() && unsafe { GetLastError() } != ERROR_SERVICE_DOES_NOT_EXIST {
            return Err(error());
        }
        if !service.is_null() {
            let service = Service(service);
            let mut status: SERVICE_STATUS = unsafe { std::mem::zeroed() };
            unsafe {
                ControlService(service.0, SERVICE_CONTROL_STOP, &mut status);
            }
            let deadline = Instant::now() + Duration::from_secs(15);
            while service_pid(&service).unwrap_or(0) != 0 && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(50));
            }
            if service_pid(&service)? != 0 {
                return Err(
                    "The helper could not be stopped. Restart Windows and uninstall again.".into(),
                );
            }
            if unsafe { DeleteService(service.0) } == 0 {
                return Err(error());
            }
        }
        let runtime = runtime_dir()?;
        if runtime.exists() {
            no_reparse(&runtime)?;
            std::fs::remove_dir_all(runtime).map_err(|e| e.to_string())?;
        }
        for name in FILES.into_iter().chain(["owner.sid", EXE]) {
            let file = target.join(name);
            if std::fs::remove_file(&file).is_err()
                && file.exists()
                && unsafe {
                    MoveFileExW(
                        wide(&file.to_string_lossy()).as_ptr(),
                        null(),
                        MOVEFILE_DELAY_UNTIL_REBOOT,
                    )
                } == 0
            {
                return Err(error());
            }
        }
        if std::fs::remove_dir(&target).is_err() {
            unsafe {
                MoveFileExW(
                    wide(&target.to_string_lossy()).as_ptr(),
                    null(),
                    MOVEFILE_DELAY_UNTIL_REBOOT,
                );
            }
        }
        Ok(())
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::os::windows::process::CommandExt;
        use std::process::{Command, Stdio};

        fn fixture() -> (String, u16, String) {
            let mixed = free_port().unwrap();
            let mut ctl = free_port().unwrap();
            while ctl == mixed {
                ctl = free_port().unwrap();
            }
            let token = crate::core::random_hex();
            let sub = crate::config::parse("proxies: [{name: Fixture, type: socks5, server: 127.0.0.1, port: 9}]\nproxy-groups: [{name: Main, type: select, proxies: [Fixture]}]\nrules: ['MATCH,DIRECT']\n").unwrap();
            (
                crate::config::build(&sub, &[], mixed, ctl, &token, true)
                    .unwrap()
                    .yaml,
                ctl,
                token,
            )
        }
        fn ready(ctl: u16, token: &str) {
            let deadline = Instant::now() + Duration::from_secs(25);
            loop {
                match call(
                    ctl,
                    token,
                    Operation::Api {
                        method: "GET".into(),
                        path: "/version".into(),
                        body: None,
                        wait: 1,
                    },
                ) {
                    Ok(_) => break,
                    Err(error) => assert!(
                        Instant::now() < deadline,
                        "The service core never became ready: {error}"
                    ),
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        }
        struct Filtered(OwnedHandle);
        impl Filtered {
            fn enter() -> Self {
                let mut original = null_mut();
                assert_ne!(
                    unsafe {
                        OpenProcessToken(
                            GetCurrentProcess(),
                            TOKEN_DUPLICATE | TOKEN_QUERY,
                            &mut original,
                        )
                    },
                    0
                );
                let original = handle(original).unwrap();
                let mut admin = null_mut();
                assert_ne!(
                    unsafe { ConvertStringSidToSidW(wide("S-1-5-32-544").as_ptr(), &mut admin) },
                    0
                );
                let disabled = SID_AND_ATTRIBUTES {
                    Sid: admin,
                    Attributes: 0,
                };
                let mut filtered = null_mut();
                let created = unsafe {
                    CreateRestrictedToken(
                        raw(&original),
                        DISABLE_MAX_PRIVILEGE,
                        1,
                        &disabled,
                        0,
                        null(),
                        0,
                        null(),
                        &mut filtered,
                    )
                };
                unsafe {
                    LocalFree(admin);
                }
                assert_ne!(created, 0, "{}", error());
                let filtered = handle(filtered).unwrap();
                assert_ne!(
                    unsafe { ImpersonateLoggedOnUser(raw(&filtered)) },
                    0,
                    "{}",
                    error()
                );
                Self(filtered)
            }
        }
        impl Drop for Filtered {
            fn drop(&mut self) {
                let _ = &self.0;
                assert_ne!(unsafe { RevertToSelf() }, 0);
            }
        }
        struct Cleanup;
        impl Drop for Cleanup {
            fn drop(&mut self) {
                if let Err(error) = remove() {
                    eprintln!("Helper test cleanup failed: {error}");
                }
            }
        }
        struct ChildGuard(std::process::Child);
        impl Drop for ChildGuard {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }

        #[test]
        #[ignore = "Disposable administrator Windows runner only; installs and removes the helper and creates a real TUN adapter"]
        fn windows_service_smoke() {
            assert!(
                crate::core::is_admin(),
                "Run on a disposable administrator Windows runner"
            );
            assert!(
                !install_dir().unwrap().exists() && open_service(SERVICE_QUERY_STATUS).is_err(),
                "Refusing to touch an existing helper installation"
            );
            let executable = std::env::var_os("AKIMOV_SERVICE_TEST_EXE").expect("Build the app first and set AKIMOV_SERVICE_TEST_EXE to target/debug/akimov-vpn.exe");
            assert!(
                Command::new(executable)
                    .arg(format!("--install-helper={}", current_sid().unwrap()))
                    .creation_flags(CREATE_NO_WINDOW)
                    .status()
                    .unwrap()
                    .success(),
                "Helper installation failed"
            );
            let _cleanup = Cleanup;
            {
                let _filtered = Filtered::enter();
                assert!(
                    open_service(SERVICE_CHANGE_CONFIG).is_err(),
                    "A standard user can replace the service executable"
                );
                assert!(
                    OpenOptions::new()
                        .write(true)
                        .open(install_dir().unwrap().join("owner.sid"))
                        .is_err(),
                    "A standard user can rewrite the helper owner"
                );
                for _ in 0..2 {
                    let (yaml, ctl, token) = fixture();
                    assert_eq!(start(&yaml).unwrap(), (ctl, token.clone()));
                    assert!(alive(ctl, &token).unwrap());
                    ready(ctl, &token);
                    assert!(
                        std::fs::read_dir(runtime_dir().unwrap()).is_err(),
                        "The private session directory is readable by the user"
                    );
                    assert!(api(ctl, &token, "POST", "/upgrade", None, 1)
                        .unwrap()
                        .is_err());
                    let mut changed: serde_yaml::Value = serde_yaml::from_str(&yaml).unwrap();
                    changed["mixed-port"] = serde_yaml::Value::from(free_port().unwrap());
                    let bad_reload =
                        serde_json::json!({"payload": serde_yaml::to_string(&changed).unwrap()});
                    assert!(
                        api(
                            ctl,
                            &token,
                            "PUT",
                            "/configs?force=false",
                            Some(&bad_reload),
                            1
                        )
                        .unwrap()
                        .is_err(),
                        "Reload changed the public proxy port"
                    );
                    let reload = serde_json::json!({"payload":yaml});
                    api(ctl, &token, "PUT", "/configs?force=false", Some(&reload), 5)
                        .unwrap()
                        .unwrap();
                    stop(ctl, &token).unwrap();
                    assert!(!alive(ctl, &token).unwrap());
                }
            }
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let slow_port = listener.local_addr().unwrap().port();
            let (accepted, seen) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                for stream in listener.incoming().take(8) {
                    let stream = stream.unwrap();
                    let _ = accepted.send(());
                    std::thread::spawn(move || {
                        let _stream = stream;
                        std::thread::sleep(Duration::from_secs(5));
                    });
                }
            });
            let (yaml, ctl, token) = fixture();
            let mut config: serde_yaml::Value = serde_yaml::from_str(&yaml).unwrap();
            config["proxies"][0]["port"] = serde_yaml::Value::from(slow_port);
            start(&serde_yaml::to_string(&config).unwrap()).unwrap();
            ready(ctl, &token);
            let pings: Vec<_> = (0..8).map(|_| {
                let token = token.clone();
                std::thread::spawn(move || call(ctl, &token, Operation::Api { method: "GET".into(), path: "/proxies/Fixture/delay?url=https%3A%2F%2Fexample.com&timeout=2000&expected=204".into(), body: None, wait: 3 }))
            }).collect();
            seen.recv_timeout(Duration::from_secs(5))
                .expect("The slow proxy never received a latency request");
            let started = Instant::now();
            api(ctl, &token, "GET", "/version", None, 1)
                .unwrap()
                .unwrap();
            assert!(
                started.elapsed() < Duration::from_secs(1),
                "Queued latency requests blocked status"
            );
            let started = Instant::now();
            stop(ctl, &token).unwrap();
            assert!(
                started.elapsed() < Duration::from_secs(2),
                "Queued latency requests blocked disconnect"
            );
            for ping in pings {
                let _ = ping.join().unwrap();
            }
            let marker = std::env::temp_dir().join(format!(
                "akimov-service-child-{}",
                crate::core::random_hex()
            ));
            let mut child = ChildGuard(
                Command::new(std::env::current_exe().unwrap())
                    .args([
                        "--exact",
                        "service::windows::tests::windows_service_child",
                        "--ignored",
                        "--nocapture",
                    ])
                    .env("AKIMOV_SERVICE_CHILD_MARKER", &marker)
                    .stdin(Stdio::null())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .creation_flags(CREATE_NO_WINDOW)
                    .spawn()
                    .unwrap(),
            );
            let deadline = Instant::now() + Duration::from_secs(30);
            while !marker.exists() {
                assert!(
                    child.0.try_wait().unwrap().is_none(),
                    "The service client exited before connecting"
                );
                assert!(
                    Instant::now() < deadline,
                    "The service client never connected"
                );
                std::thread::sleep(Duration::from_millis(50));
            }
            let core_pid = std::fs::read_to_string(&marker)
                .unwrap()
                .parse::<u32>()
                .unwrap();
            let core =
                handle(unsafe { OpenProcess(SYNCHRONIZATION_SYNCHRONIZE, 0, core_pid) }).unwrap();
            child.0.kill().unwrap();
            child.0.wait().unwrap();
            assert_eq!(
                unsafe { WaitForSingleObject(raw(&core), 10000) },
                WAIT_OBJECT_0,
                "The core outlived its UI parent"
            );
            std::fs::remove_file(marker).unwrap();

            let (yaml, ctl, token) = fixture();
            ensure_running().unwrap();
            let pid = call(ctl, &token, Operation::Start { yaml }).unwrap()["pid"]
                .as_u64()
                .unwrap() as u32;
            ready(ctl, &token);
            let core = handle(unsafe { OpenProcess(SYNCHRONIZATION_SYNCHRONIZE, 0, pid) }).unwrap();
            let service = open_service(SERVICE_QUERY_STATUS).unwrap();
            let broker = handle(unsafe {
                OpenProcess(
                    PROCESS_TERMINATE | SYNCHRONIZATION_SYNCHRONIZE,
                    0,
                    service_pid(&service).unwrap(),
                )
            })
            .unwrap();
            assert_ne!(unsafe { TerminateProcess(raw(&broker), 1) }, 0);
            assert_eq!(
                unsafe { WaitForSingleObject(raw(&core), 10000) },
                WAIT_OBJECT_0,
                "The core outlived a crashed service"
            );
            println!("PASS: standard-user reconnect/reload, protected credentials, UI crash and service crash cleanup");
        }

        #[test]
        #[ignore = "Only a child process of windows_service_smoke"]
        fn windows_service_child() {
            let marker = std::env::var_os("AKIMOV_SERVICE_CHILD_MARKER")
                .expect("Only run from windows_service_smoke");
            let (yaml, ctl, token) = fixture();
            ensure_running().unwrap();
            let pid = call(ctl, &token, Operation::Start { yaml }).unwrap()["pid"]
                .as_u64()
                .unwrap();
            ready(ctl, &token);
            std::fs::write(marker, pid.to_string()).unwrap();
            loop {
                std::thread::sleep(Duration::from_secs(1));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn broker_allows_only_the_operations_used_by_the_app() {
        assert!(allowed_api("GET", "/version", &None));
        assert!(allowed_api("GET", "/proxies", &None));
        assert!(allowed_api("GET", "/connections", &None));
        assert!(allowed_api("GET", "/traffic", &None));
        assert!(allowed_api(
            "GET",
            "/proxies/Node%201/delay?url=https%3A%2F%2Fexample.com&timeout=2000&expected=204",
            &None
        ));
        assert!(allowed_api(
            "PUT",
            "/proxies/Node%201",
            &Some(json!({"name":"A"}))
        ));
        assert!(allowed_api("DELETE", "/connections/connection-id", &None));
        assert!(allowed_api(
            "PATCH",
            "/configs",
            &Some(json!({"tun":{"enable":false}}))
        ));
        assert!(allowed_api(
            "PUT",
            "/configs?force=false",
            &Some(json!({"payload":"proxies: []"}))
        ));
        for path in [
            "/upgrade",
            "/restart",
            "/configs/geo",
            "//upgrade",
            "/connections",
            "/proxies/../configs",
            "/proxies/x?z=/upgrade",
        ] {
            assert!(!allowed_api("POST", path, &None), "{path}");
        }
        assert!(!allowed_api(
            "PUT",
            "/configs?force=false",
            &Some(json!({"path":"C:\\evil.yaml"}))
        ));
        assert!(!allowed_api(
            "PATCH",
            "/configs",
            &Some(json!({"tun":{"enable":false},"allow-lan":true}))
        ));
        assert!(!allowed_api(
            "PUT",
            "/proxies/A",
            &Some(json!({"name":"B","extra":"x"}))
        ));
        assert!(!allowed_api("GET", "/version", &Some(json!({}))));
        for node in [
            "Germany%20%231",
            "Name%3F",
            "100%25",
            "%252e",
            "Auto%20WiFi%2FLTE%20%231%3F100%25",
        ] {
            assert!(
                allowed_api(
                    "PUT",
                    &format!("/proxies/{node}"),
                    &Some(json!({"name":"B"}))
                ),
                "{node}"
            );
        }
        for node in [".", "..", "%2e", "%2e%2e", "%2F", "%5C", "%", "%0a"] {
            assert!(
                !allowed_api("DELETE", &format!("/connections/{node}"), &None),
                "{node}"
            );
            if node != "%2F" {
                assert!(
                    !allowed_api(
                        "PUT",
                        &format!("/proxies/{node}"),
                        &Some(json!({"name":"B"}))
                    ),
                    "{node}"
                );
            }
        }
        assert!(!allowed_api(
            "GET",
            "/proxies/A/delay?url=file%3A%2F%2FC%3A%2Fsecret&timeout=2000&expected=204",
            &None
        ));
        assert!(!allowed_api(
            "GET",
            "/proxies/A/delay?url=https%3A%2F%2Fexample.com&timeout=2000&expected=204&extra=x",
            &None
        ));
    }

    #[test]
    fn broker_rejects_invalid_proxy_ports_before_start_or_reload() {
        assert_eq!(mixed_port("mixed-port: 12345").unwrap(), 12345);
        for yaml in [
            "mixed-port: 0",
            "mixed-port: 65536",
            "mixed-port: -1",
            "mixed-port: '12345'",
            "mixed-port: []",
            "port: 12345",
        ] {
            assert!(mixed_port(yaml).is_err(), "{yaml}");
        }
    }
}
