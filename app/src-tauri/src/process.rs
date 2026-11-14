//! Local probes own a job; whole-device connections use the installed service.
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use windows_sys::Win32::Foundation::*;
#[cfg(test)]
use windows_sys::Win32::System::Threading::*;

enum Owner {
    Local { child: Child, _job: OwnedHandle },
    Service { ctl: u16, token: String },
}
pub struct Process(Owner);

fn owned(handle: HANDLE) -> Result<OwnedHandle, String> {
    if handle.is_null() {
        Err(std::io::Error::last_os_error().to_string())
    } else {
        Ok(unsafe { OwnedHandle::from_raw_handle(handle as _) })
    }
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
        let yaml = std::fs::read_to_string(cfg).map_err(|e| e.to_string())?;
        let (ctl, token) = crate::service::start(&yaml)?;
        Ok(Self(Owner::Service { ctl, token }))
    }
    pub fn kill(&mut self) -> std::io::Result<()> {
        match &mut self.0 {
            Owner::Local { child, .. } => child.kill(),
            Owner::Service { ctl, token } => {
                crate::service::stop(*ctl, token).map_err(std::io::Error::other)
            }
        }
    }
    pub fn wait(&mut self) -> std::io::Result<()> {
        match &mut self.0 {
            Owner::Local { child, .. } => child.wait().map(|_| ()),
            Owner::Service { .. } => Ok(()),
        }
    }
    pub fn try_wait(&mut self) -> std::io::Result<Option<u32>> {
        match &mut self.0 {
            Owner::Local { child, .. } => child
                .try_wait()
                .map(|s| s.map(|s| s.code().unwrap_or(-1) as u32)),
            Owner::Service { ctl, token } => crate::service::alive(*ctl, token)
                .map(|alive| if alive { None } else { Some(0) })
                .map_err(std::io::Error::other),
        }
    }
    #[cfg(test)]
    pub fn id(&self) -> u32 {
        match &self.0 {
            Owner::Local { child, .. } => child.id(),
            Owner::Service { .. } => 0,
        }
    }
}
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.kill();
        let _ = self.wait();
    }
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
    fn closing_a_local_owner_job_stops_its_core() {
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
        drop(core);
        assert_eq!(
            unsafe { WaitForSingleObject(child.as_raw_handle() as _, 0) },
            WAIT_OBJECT_0
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}
