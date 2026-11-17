use axionomy_video::{AxionomyVideo, AxionomyVideoMedia, HEIGHT, WIDTH, facts::Facts};
use fframes::{EncoderOptions, RenderOptions, StaticMediaProvider, cli};
use fframes_skia_renderer::{
    SkiaFFramesRenderer, SkiaPipelineConcurrencyPolicy, SkiaPipelineConfig, metal::SkiaMetalCtx,
};
use std::process::ExitCode;

fn main() -> ExitCode {
    let media = AxionomyVideoMedia::prepare().expect("media");
    // Everything the video shows, read from the engine before a frame is drawn.
    let facts = Facts::read();
    let video = AxionomyVideo::new(&facts);
    let gpu = SkiaMetalCtx::new(WIDTH, HEIGHT).expect("GPU context");

    cli::new(
        &video,
        RenderOptions {
            media: Some(&media),
            default_font: "DM Sans",
            video_encoder_options: EncoderOptions {
                preferred_encoder: Some("libx264"),
                codec_params: Some(&[("crf", "18"), ("preset", "slow"), ("tune", "animation")]),
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .backend(
        SkiaFFramesRenderer::new_metal(
            &gpu,
            SkiaPipelineConfig {
                concurrency_policy: SkiaPipelineConcurrencyPolicy::MaxPerformance,
                ..Default::default()
            },
        )
        .expect("skia renderer"),
    )
    .preview(fframes_native_player::cli_preview)
    .run()
}
