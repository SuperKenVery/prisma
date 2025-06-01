mod copy_to_screen;
mod post_processor;
mod render_context;
mod renderer;

pub use copy_to_screen::CopyToScreen;
pub use post_processor::PostProcessor;
pub use render_context::RenderContext;
pub use renderer::{BindGroupLayoutSet, BindGroupSet, Renderer};

use divrem::DivCeil;

/// Find minimum integer N where N % align_to == 0 and N >= num
fn align<T: DivCeil + Copy>(num: T, align_to: T) -> T
where
    <T as std::ops::Div>::Output: std::ops::Mul<T, Output = T>,
{
    num.div_ceil(align_to) * align_to
}
