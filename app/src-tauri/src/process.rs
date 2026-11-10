//! The elevated TUN owner has no window and exits when its UI parent or stop event signals.
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use windows_sys::Win32::{Foundation::*, System::Threading::*, UI::Shell::*};

enum Owner {
    Local {
        child: Child,
        _job: OwnedHandle,
    },
    Elevated {
        process: OwnedHandle,
        stop: OwnedHandle,
    },
}
pub struct Process(Owner);

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}
fn owned(handle: HANDLE) -> Result<OwnedHandle, String> {
    if handle.is_null() {
        Err(std::io::Error::last_os_error().to_string())
    } else {
        Ok(unsafe { OwnedHandle::from_raw_handle(handle as _) })
    }
}
fn event_name(pid: u32, nonce: &str) -> String {
    format!("Local\\akimov-core-{pid}-{nonce}")
}

fn spawn_local(exe: &Path, dir: &Path, cfg: &Path, log: &Path) -> Result<Process, String> {
    let log = std::fs::File::create(log).map_err(|e| e.to_string())?;
    let mut child = Command::new(exe)
        .arg("-d")
        .arg(dir)
        .arg("-f")
        .arg(cfg)
        .stdin(Stdio::null())
        .stdout(log.try_clone().map_err(|e| e.to_string())?)
        .stderr(log)
        .creation_flags(0x0800_0000)
        .spawn()
        .map_err(|_| "The VPN core could not be started. Reinstall the app and try again.")?;
    let job = match tie_to_us(&child) {
        Ok(job) => job,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
    };
    Ok(Process(Owner::Local { child, _job: job }))
}

impl Process {
    pub fn spawn(
        exe: &Path,
        dir: &Path,
        cfg: &Path,
        log: &Path,
        elevate: bool,
    ) -> Result<Self, String> {
        if !elevate {
            return spawn_local(exe, dir, cfg, log);
        }
        let nonce = crate::core::random_hex();
        let stop = owned(unsafe {
            CreateEventW(
                std::ptr::null(),
                1,
                0,
                wide(&event_name(std::process::id(), &nonce)).as_ptr(),
            )
        })?;
        let app = wide(
            &std::env::current_exe()
                .map_err(|e| e.to_string())?
                .to_string_lossy(),
        );
        let args = wide(&format!(
            "--tun-helper={}:{} --core-dir=\"{}\"",
            std::process::id(),
            nonce,
            dir.display()
        ));
        let verb = wide("runas");
        let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
        info.cbSize = std::mem::size_of_val(&info) as u32;
        info.fMask = SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC;
        info.lpVerb = verb.as_ptr();
        info.lpFile = app.as_ptr();
        info.lpParameters = args.as_ptr();
        info.nShow = 0;
        use windows_sys::Win32::System::Com::*;
        let com = unsafe {
            CoInitializeEx(
                std::ptr::null(),
                (COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) as u32,
            )
        };
        let launched = unsafe { ShellExecuteExW(&mut info) };
        let error = unsafe { GetLastError() };
        if com >= 0 {
            unsafe {
                CoUninitialize();
            }
        }
        if launched == 0 {
            return Err(if error == ERROR_CANCELLED {
                "elevation_cancelled"
            } else {
                "Windows could not start whole-device mode. Try again."
            }
            .into());
        }
        let process = owned(info.hProcess)?;
        Ok(Self(Owner::Elevated { process, stop }))
    }
    pub fn kill(&mut self) -> std::io::Result<()> {
        match &mut self.0 {
            Owner::Local { child, .. } => child.kill(),
            Owner::Elevated { stop, .. } => {
                if unsafe { SetEvent(stop.as_raw_handle() as _) } == 0 {
                    Err(std::io::Error::last_os_error())
                } else {
                    Ok(())
                }
            }
        }
    }
    pub fn wait(&mut self) -> std::io::Result<()> {
        match &mut self.0 {
            Owner::Local { child, .. } => child.wait().map(|_| ()),
            Owner::Elevated { process, .. } => {
                if unsafe { WaitForSingleObject(process.as_raw_handle() as _, 5000) }
                    == WAIT_OBJECT_0
                {
                    Ok(())
                } else {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::TimedOut,
                        "VPN shutdown timed out",
                    ))
                }
            }
        }
    }
    pub fn try_wait(&mut self) -> std::io::Result<Option<u32>> {
        match &mut self.0 {
            Owner::Local { child, .. } => child
                .try_wait()
                .map(|s| s.map(|s| s.code().unwrap_or(-1) as u32)),
            Owner::Elevated { process, .. } => {
                let handle = process.as_raw_handle() as _;
                match unsafe { WaitForSingleObject(handle, 0) } {
                    WAIT_TIMEOUT => Ok(None),
                    WAIT_OBJECT_0 => {
                        let mut code = 0;
                        if unsafe { GetExitCodeProcess(handle, &mut code) } == 0 {
                            Err(std::io::Error::last_os_error())
                        } else {
                            Ok(Some(code))
                        }
                    }
                    _ => Err(std::io::Error::last_os_error()),
                }
            }
        }
    }
    #[cfg(test)]
    pub fn id(&self) -> u32 {
        match &self.0 {
            Owner::Local { child, .. } => child.id(),
            Owner::Elevated { process, .. } => unsafe {
                GetProcessId(process.as_raw_handle() as _)
            },
        }
    }
}
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.kill();
        let _ = self.wait();
    }
}

pub fn helper() -> Option<Result<(), String>> {
    let arg = std::env::args().find(|a| a.starts_with("--tun-helper="))?;
    Some((|| {
        let (pid, nonce) = parse_helper(&arg).ok_or("Invalid TUN owner.")?;
        if !crate::core::is_admin() {
            return Err("Administrator access is required.".into());
        }
        let dir = std::env::args()
            .find_map(|a| a.strip_prefix("--core-dir=").map(PathBuf::from))
            .ok_or("Missing core directory.")?;
        if !dir.is_absolute() || dir.file_name().is_none_or(|name| name != "core") {
            return Err("Invalid core directory.".into());
        }
        let parent = owned(unsafe {
            OpenProcess(
                SYNCHRONIZATION_SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION,
                0,
                pid,
            )
        })?;
        let mut image = vec![0u16; 32768];
        let mut length = image.len() as u32;
        if unsafe {
            QueryFullProcessImageNameW(
                parent.as_raw_handle() as _,
                0,
                image.as_mut_ptr(),
                &mut length,
            )
        } == 0
        {
            return Err("The app is no longer running.".into());
        }
        if !String::from_utf16_lossy(&image[..length as usize]).eq_ignore_ascii_case(
            &std::env::current_exe()
                .map_err(|e| e.to_string())?
                .to_string_lossy(),
        ) {
            return Err("Invalid TUN owner.".into());
        }
        let stop = owned(unsafe {
            OpenEventW(
                SYNCHRONIZATION_SYNCHRONIZE,
                0,
                wide(&event_name(pid, nonce)).as_ptr(),
            )
        })?;
        let handles = [
            parent.as_raw_handle() as HANDLE,
            stop.as_raw_handle() as HANDLE,
        ];
        if unsafe { WaitForMultipleObjects(2, handles.as_ptr(), 0, 0) } != WAIT_TIMEOUT {
            return Ok(());
        }
        let core = spawn_local(
            &crate::core::core_exe()?,
            &dir,
            &dir.join("config.yaml"),
            &dir.join("core.log"),
        )?;
        supervise(parent, stop, core);
        Ok(())
    })())
}
fn supervise(parent: OwnedHandle, stop: OwnedHandle, mut core: Process) {
    let Owner::Local { child, .. } = &mut core.0 else {
        unreachable!()
    };
    let handles = [
        parent.as_raw_handle() as HANDLE,
        stop.as_raw_handle() as HANDLE,
        child.as_raw_handle() as HANDLE,
    ];
    unsafe {
        WaitForMultipleObjects(3, handles.as_ptr(), 0, INFINITE);
    }
    // Dropping the owned job kills the core even if the UI crashed or was force-closed.
    drop(core);
}
fn parse_helper(arg: &str) -> Option<(u32, &str)> {
    let (pid, nonce) = arg.strip_prefix("--tun-helper=")?.split_once(':')?;
    let pid = pid.parse::<u32>().ok().filter(|pid| *pid > 0)?;
    (nonce.len() == 32 && nonce.bytes().all(|b| b.is_ascii_hexdigit())).then_some((pid, nonce))
}

fn tie_to_us(child: &Child) -> Result<OwnedHandle, String> {
    use windows_sys::Win32::System::JobObjects::*;
    unsafe {
        let job = owned(CreateJobObjectW(std::ptr::null(), std::ptr::null()))?;
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if SetInformationJobObject(
            job.as_raw_handle() as _,
            JobObjectExtendedLimitInformation,
            std::ptr::from_ref(&info).cast(),
            std::mem::size_of_val(&info) as u32,
        ) == 0
            || AssignProcessToJobObject(job.as_raw_handle() as _, child.as_raw_handle() as _) == 0
        {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(job)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn headless_owner_stops_its_core_without_exiting_the_ui() {
        let dir = std::env::temp_dir().join(format!("akimov-owner-{}", crate::core::random_hex()));
        std::fs::create_dir(&dir).unwrap();
        let cfg = dir.join("config.yaml");
        std::fs::write(
            &cfg,
            "mixed-port: 0\ndns: {enable: false}\ntun: {enable: false}\nrules: ['MATCH,DIRECT']\n",
        )
        .unwrap();
        let core = spawn_local(
            &crate::core::core_exe().unwrap(),
            &dir,
            &cfg,
            &dir.join("core.log"),
        )
        .unwrap();
        let child =
            owned(unsafe { OpenProcess(SYNCHRONIZATION_SYNCHRONIZE, 0, core.id()) }).unwrap();
        let parent =
            owned(unsafe { OpenProcess(SYNCHRONIZATION_SYNCHRONIZE, 0, std::process::id()) })
                .unwrap();
        let name = wide(&event_name(std::process::id(), &crate::core::random_hex()));
        let stop = owned(unsafe { CreateEventW(std::ptr::null(), 1, 0, name.as_ptr()) }).unwrap();
        let worker_stop =
            owned(unsafe { OpenEventW(SYNCHRONIZATION_SYNCHRONIZE, 0, name.as_ptr()) }).unwrap();
        let (done, ended) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            supervise(parent, worker_stop, core);
            done.send(()).unwrap();
        });
        assert_ne!(unsafe { SetEvent(stop.as_raw_handle() as _) }, 0);
        ended
            .recv_timeout(std::time::Duration::from_secs(3))
            .expect("The helper ignored the disconnect signal");
        worker.join().unwrap();
        assert_eq!(
            unsafe { WaitForSingleObject(child.as_raw_handle() as _, 0) },
            WAIT_OBJECT_0,
            "The core outlived the helper"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn helper_rejects_missing_or_untrusted_owner_arguments() {
        assert!(parse_helper("--tun-helper=0:00000000000000000000000000000000").is_none());
        assert!(parse_helper("--tun-helper=3:../../../config").is_none());
        assert!(parse_helper("--tun-helper=3:00000000000000000000000000000000:4").is_none());
        assert_eq!(
            parse_helper("--tun-helper=3:0123456789abcdef0123456789abcdef"),
            Some((3, "0123456789abcdef0123456789abcdef"))
        );
    }
}
