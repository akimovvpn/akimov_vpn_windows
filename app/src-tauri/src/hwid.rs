use sha2::{Digest, Sha256};

pub const UNAVAILABLE: &str =
    "Device identification is unavailable. Restart Windows and try again.";

pub fn valid(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit())
}

fn system_uuid(raw: &[u8]) -> Option<[u8; 16]> {
    let length = u32::from_le_bytes(raw.get(4..8)?.try_into().ok()?) as usize;
    let mut table = raw.get(8..8usize.checked_add(length)?)?;
    while table.len() >= 4 {
        let kind = table[0];
        let length = table[1] as usize;
        if length < 4 || length > table.len() {
            return None;
        }
        if kind == 1 && length >= 24 {
            let uuid: [u8; 16] = table.get(8..24)?.try_into().ok()?;
            if !uuid.iter().all(|b| *b == 0) && !uuid.iter().all(|b| *b == 255) {
                return Some(uuid);
            }
        }
        if kind == 127 {
            break;
        }
        let end = table[length..].windows(2).position(|s| s == [0, 0])?;
        table = &table[length + end + 2..];
    }
    None
}

fn fingerprint(uuid: &[u8; 16]) -> String {
    let mut hash = Sha256::new();
    hash.update(b"akimov-vpn/hwid/v1\0");
    hash.update(uuid);
    format!("{:x}", hash.finalize())
}

pub fn read() -> Result<String, String> {
    use windows_sys::Win32::System::SystemInformation::GetSystemFirmwareTable;
    let provider = u32::from_be_bytes(*b"RSMB");
    let size = unsafe { GetSystemFirmwareTable(provider, 0, std::ptr::null_mut(), 0) };
    if !(8..=4 * 1024 * 1024).contains(&size) {
        return Err(UNAVAILABLE.into());
    }
    let mut raw = vec![0; size as usize];
    let received = unsafe { GetSystemFirmwareTable(provider, 0, raw.as_mut_ptr(), size) };
    if received != size {
        return Err(UNAVAILABLE.into());
    }
    system_uuid(&raw)
        .map(|uuid| fingerprint(&uuid))
        .ok_or_else(|| UNAVAILABLE.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn table(uuid: [u8; 16]) -> Vec<u8> {
        let mut raw = vec![0, 3, 0, 0, 26, 0, 0, 0, 1, 24, 0, 0, 0, 0, 0, 0];
        raw.extend(uuid);
        raw.extend([0, 0]);
        raw
    }
    #[test]
    fn firmware_identity_is_stable_and_invalid_firmware_is_rejected() {
        let uuid = [7; 16];
        assert!(system_uuid(&table(uuid)) == Some(uuid));
        let id = fingerprint(&uuid);
        assert!(valid(&id) && id == fingerprint(&uuid));
        assert!(id != fingerprint(&[8; 16]));
        for raw in [
            vec![],
            table([0; 16]),
            table([255; 16]),
            vec![0, 3, 0, 0, 255, 255, 255, 255],
        ] {
            assert!(system_uuid(&raw).is_none());
        }
        assert!(!valid("") && !valid("disabled") && !valid(&"g".repeat(64)));
    }
    #[test]
    fn windows_firmware_can_be_read_without_exposing_the_identity() {
        let first = read().expect("Windows must expose a usable firmware UUID");
        assert!(valid(&first));
        assert!(read().is_ok_and(|second| first == second));
    }
}
