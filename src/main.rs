use anyhow::Result;
use clap::Parser;
use console::Emoji;
use log::info;
use prisma::{
    config::Config,
    render::{
        BindGroupLayoutSet, BindGroupSet, CopyToScreen, PostProcessor, RenderContext, Renderer,
    },
    scene::Scene,
    window,
};
use std::{cell::RefCell, error::Error, rc::Rc};

fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    window::show_window()?;

    Ok(())
}
