use anyhow::Context;
use gstreamer::glib::object::Cast;
use gstreamer::prelude::{ElementExt, GstBinExt, PadExtManual};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

type Result<T> = anyhow::Result<T>;

pub struct PipelineSetup {
    pub pipeline: gstreamer::Pipeline,
    pub appsrc: gstreamer_app::AppSrc,
    pub videotee: gstreamer::Element,
    pub audiotee: gstreamer::Element,
    pub video_bytes: Arc<AtomicU64>,
    pub audio_bytes: Arc<AtomicU64>,
}

pub fn setup_gstreamer_pipeline(peer_addr: &str) -> Result<PipelineSetup> {
    tracing::info!("[{}] Setting up GStreamer pipeline", peer_addr);

    let pipeline_str = "appsrc name=ingest is-live=true format=time ! \
        flvdemux name=demux \
        demux.video ! h264parse ! queue ! tee name=videotee ! fakesink \
        demux.audio ! aacparse ! queue ! tee name=audiotee ! fakesink";

    let pipeline = gstreamer::parse::launch(pipeline_str)?
        .downcast::<gstreamer::Pipeline>()
        .map_err(|_| anyhow::anyhow!("Failed to cast to Pipeline"))?;

    let appsrc = pipeline
        .by_name("ingest")
        .context("Could not find 'ingest' element")?
        .downcast::<gstreamer_app::AppSrc>()
        .map_err(|_| anyhow::anyhow!("'ingest' is not an AppSrc"))?;

    let videotee = pipeline
        .by_name("videotee")
        .context("Could not find 'videotee' element")?;

    let audiotee = pipeline
        .by_name("audiotee")
        .context("Could not find 'audiotee' element")?;

    // Attach pad probes on the tee sink pads to count video and audio bytes separately.
    let video_bytes = Arc::new(AtomicU64::new(0));
    let audio_bytes = Arc::new(AtomicU64::new(0));

    if let Some(vsink) = videotee.static_pad("sink") {
        let vb = video_bytes.clone();
        vsink.add_probe(gstreamer::PadProbeType::BUFFER, move |_pad: &gstreamer::Pad, info: &mut gstreamer::PadProbeInfo<'_>| {
            if let Some(buf) = info.buffer() {
                vb.fetch_add(buf.size() as u64, Ordering::Relaxed);
            }
            gstreamer::PadProbeReturn::Ok
        });
    }

    if let Some(asink) = audiotee.static_pad("sink") {
        let ab = audio_bytes.clone();
        asink.add_probe(gstreamer::PadProbeType::BUFFER, move |_pad: &gstreamer::Pad, info: &mut gstreamer::PadProbeInfo<'_>| {
            if let Some(buf) = info.buffer() {
                ab.fetch_add(buf.size() as u64, Ordering::Relaxed);
            }
            gstreamer::PadProbeReturn::Ok
        });
    }

    pipeline.set_state(gstreamer::State::Playing)?;

    tracing::info!("[{}] Pipeline ready and playing", peer_addr);

    Ok(PipelineSetup {
        pipeline,
        appsrc,
        videotee,
        audiotee,
        video_bytes,
        audio_bytes,
    })
}
