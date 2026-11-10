//! Per-user proxy, with a recovery record written before Windows is changed.
use serde::{Deserialize, Serialize};
use std::{io, path::Path};
use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
use winreg::RegKey;
const KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Internet Settings";
const BYPASS: &str = "localhost;127.*;10.*;172.16.*;172.17.*;172.18.*;172.19.*;172.20.*;172.21.*;172.22.*;172.23.*;172.24.*;172.25.*;172.26.*;172.27.*;172.28.*;172.29.*;172.30.*;172.31.*;192.168.*;<local>";
#[derive(Serialize, Deserialize)]
pub struct Saved {
    enable: Option<u32>,
    server: Option<String>,
    bypass: Option<String>,
    owner: String,
}
fn key() -> io::Result<RegKey> {
    RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(KEY, KEY_READ | KEY_WRITE)
}
fn refresh() {
    use windows_sys::Win32::Networking::WinInet::InternetSetOptionW;
    unsafe {
        InternetSetOptionW(std::ptr::null_mut(), 39, std::ptr::null_mut(), 0);
        InternetSetOptionW(std::ptr::null_mut(), 37, std::ptr::null_mut(), 0);
    }
}
pub fn ours(port: u16) -> String {
    format!("127.0.0.1:{port}")
}
fn put<T: winreg::types::ToRegValue>(k: &RegKey, name: &str, value: &Option<T>) -> io::Result<()> {
    match value {
        Some(v) => k.set_value(name, v),
        None => match k.delete_value(name) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            result => result,
        },
    }
}
pub fn set(port: u16, recovery: &Path) -> io::Result<Saved> {
    let k = key()?;
    let saved = Saved {
        enable: k.get_value("ProxyEnable").ok(),
        server: k.get_value("ProxyServer").ok(),
        bypass: k.get_value("ProxyOverride").ok(),
        owner: ours(port),
    };
    std::fs::write(recovery, serde_json::to_vec(&saved)?)?;
    let result = (|| {
        k.set_value("ProxyServer", &saved.owner)?;
        k.set_value("ProxyOverride", &BYPASS)?;
        k.set_value("ProxyEnable", &1u32)
    })();
    if let Err(e) = result {
        let _ = restore(&saved, recovery);
        return Err(e);
    }
    refresh();
    Ok(saved)
}
pub fn restore(saved: &Saved, recovery: &Path) -> io::Result<()> {
    let k = key()?;
    // Another app may have taken over the proxy while we were running.
    if k.get_value::<String, _>("ProxyServer").unwrap_or_default() == saved.owner {
        put(&k, "ProxyOverride", &saved.bypass)?;
        put(&k, "ProxyEnable", &saved.enable)?;
        // Keep ownership until every other value is restored, so a failed write remains recoverable.
        put(&k, "ProxyServer", &saved.server)?;
        refresh();
    }
    match std::fs::remove_file(recovery) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        result => result,
    }
}
pub fn recover(path: &Path) -> io::Result<()> {
    match std::fs::read(path) {
        Ok(bytes) => restore(&serde_json::from_slice(&bytes)?, path),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bypass_matches_only_private_172_addresses() {
        let matches = |ip: &str| {
            BYPASS.split(';').any(|pattern| {
                pattern
                    .strip_suffix('*')
                    .is_some_and(|prefix| ip.starts_with(prefix))
            })
        };
        for second in 16..=31 {
            assert!(matches(&format!("172.{second}.1.1")));
        }
        for ip in ["172.2.1.1", "172.200.1.1", "172.217.14.206", "172.32.1.1"] {
            assert!(!matches(ip), "public IP bypassed: {ip}");
        }
    }
}
