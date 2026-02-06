use anyhow::Context;
use gstreamer::glib::object::Cast;
use gstreamer::prelude::{ElementExt, GstBinExt};

type Result<T> = anyhow::Result<T>;

pub fn setup_gstreamer_pipeline(
    peer_addr: &str,
) -> Result<(gstreamer::Pipeline, gstreamer_app::AppSrc)> {
    println!("🎬 [{}] Setting up GStreamer pipeline...", peer_addr);

    // Create the pipeline string
    let pipeline_str = "appsrc name=ingest is-live=true format=time ! \
        flvdemux name=demux \
        demux.video ! h264parse ! queue ! tee name=videotee ! fakesink \
        demux.audio ! aacparse ! queue ! fakesink";

    // Parse and create the pipeline
    let pipeline = gstreamer::parse::launch(pipeline_str)?
        .downcast::<gstreamer::Pipeline>()
        .map_err(|_| anyhow::anyhow!("Failed to cast to Pipeline"))?;

    // Get the appsrc element
    let appsrc = pipeline
        .by_name("ingest")
        .context("Could not find 'ingest' element")?
        .downcast::<gstreamer_app::AppSrc>()
        .map_err(|_| anyhow::anyhow!("'ingest' is not an AppSrc"))?;

    // Start the pipeline
    pipeline.set_state(gstreamer::State::Playing)?;

    println!("✅ [{}] Pipeline ready and playing", peer_addr);

    Ok((pipeline, appsrc))
}
