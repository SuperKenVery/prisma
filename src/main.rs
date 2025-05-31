use anyhow::Result;
use clap::Parser;
use console::Emoji;
use log::info;
use prisma::{
    config::Config,
    render::{BindGroupLayoutSet, BindGroupSet, PostProcessor, RenderContext, Renderer},
    scene::Scene,
    window,
};
use std::{error::Error, rc::Rc};

fn build_scene(context: Rc<RenderContext>, config: &Config) -> Result<Scene> {
    let (document, buffers, images) = gltf::import(&config.scene)?;

    let scene = Scene::new(
        context.clone(),
        config,
        &document.scenes().next().unwrap(),
        &buffers,
        &images,
    )?;

    Ok(scene)
}

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let config = Config::parse();

    let context = Rc::new(pollster::block_on(RenderContext::new())?);
    info!(
        "{} {} Parsing and loading the scene...",
        console::style("[1/4]").bold().dim(),
        Emoji("📜 ", "")
    );
    let scene = build_scene(context.clone(), &config)?;
    let (bind_group_layout_set, bind_group_set) =
        (scene.bind_group_layout.clone(), scene.bind_group.clone());

    let renderer = Renderer::new(context.clone(), &config, bind_group_layout_set);
    info!(
        "{} {} Taking samples of path-traced rays...",
        console::style("[2/4]").bold().dim(),
        Emoji("📷 ", "")
    );
    // renderer.render(bind_group_set)?;

    // info!(
    //     "{} {} Applying post-processing effects...",
    //     console::style("[3/4]").bold().dim(),
    //     Emoji("🌟 ", "")
    // );
    // let post_processor = PostProcessor::new(context.clone(), &config);
    // post_processor.post_process(renderer.render_target());

    window::show_window(renderer, scene)?;

    // let image = pollster::block_on(post_processor.retrieve_result())?.unwrap();
    // info!(
    //     "{} {} Exporting the image...",
    //     console::style("[4/4]").bold().dim(),
    //     Emoji("🎞️ ", "")
    // );
    // image.save(config.output)?;
    Ok(())
}
