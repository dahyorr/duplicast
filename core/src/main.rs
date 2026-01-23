use anyhow::Context;
use gstreamer::glib::object::Cast;
use gstreamer::prelude::{ElementExt, GstBinExt};
use rml_rtmp::handshake::{Handshake, HandshakeProcessResult, PeerType};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[tokio::main]
async fn main() -> Result<()> {
    gstreamer::init()?;

    let port = 1935;
    let addr = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(addr).await?;
    println!(
        "Duplicast engine listening on rtmp://localhost:{}/live",
        port
    );

    // handle connections
    while let Ok((socket, addr)) = listener.accept().await {
        println!("New Connection from {}", addr.ip());

        tokio::spawn(async move {
            if let Err(e) = handle_connection(socket).await {
                eprintln!("Error handling connection: {}", e);
            }
        });
    }

    Ok(())
}

async fn handle_connection(mut socket: tokio::net::TcpStream) -> Result<()> {
    peform_rtmp_handshake(&mut socket).await?;

    let (_pipeline, appsrc) = setup_gstreamer_pipeline()?;

    println!("Stream started, accepting video data...");
    process_stream_loop(&mut socket, &appsrc).await?;

    Ok(())
}

async fn peform_rtmp_handshake(socket: &mut TcpStream) -> Result<()> {
    println!("Peforming RTMP handshake");
    let mut handshake = Handshake::new(PeerType::Server);
    let mut buffer = [0u8; 4096];
    let mut received_data = Vec::new();

    loop {
        let n = socket.read(&mut buffer).await?;
        if n == 0 {
            return Err("🔌 Connection closed during handshake".into());
        }
        received_data.extend_from_slice(&buffer[..n]);

        match handshake.process_bytes(&received_data) {
            Ok(HandshakeProcessResult::InProgress { response_bytes }) => {
                socket.write_all(&response_bytes).await?;
                received_data.clear(); // Reset buffer until next chunk
            }

            Ok(HandshakeProcessResult::Completed {
                response_bytes,
                remaining_bytes: _,
            }) => {
                socket.write_all(&response_bytes).await?;
                println!("✅ RTMP handshake complete 🤝");
                return Ok(());
            }

            Err(e) => {
                return Err(format!("❌ Handshake error: {:?}", e).into());
            }
        };
    }
}

fn setup_gstreamer_pipeline() -> Result<(gstreamer::Pipeline, gstreamer_app::AppSrc)> {
    let pipeline_str = "appsrc name=ingest is-live=true format=time ! \
         flvdemux name=demux \
         demux.video ! h264parse ! queue ! tee name=videotee ! fakesink \
         demux.audio ! aacparse ! queue ! fakesink";
    let pipeline = gstreamer::parse::launch(pipeline_str)?.downcast::<gstreamer::Pipeline>()
        .map_err(|_| anyhow::anyhow!("Failed to cast to Pipeline"))?;

    let appsrc = pipeline.by_name("ingest").context("Could not find 'ingest' element")?
        .downcast::<gstreamer_app::AppSrc>()
        .map_err(|_| anyhow::anyhow!("ingest is not an AppSrc"))?;

    pipeline.set_state(gstreamer::State::Playing)?;

    Ok((pipeline, appsrc))
}

async fn process_stream_loop(socket: &mut TcpStream, appsrc: &gstreamer_app::AppSrc) -> Result<()> {
    let mut buf = [0u8; 4096];

    loop {
        let n = socket.read(&mut buf).await?;
        if n == 0 { break; } // Socket closed

        // Push data into GStreamer
        let buffer = gstreamer::Buffer::from_slice(buf[..n].to_vec());
        if appsrc.push_buffer(buffer).is_err() {
             break; // Pipeline died (e.g., error in GStreamer)
        }
    }
    Ok(())
}