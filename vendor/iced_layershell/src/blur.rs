use layershellev::WindowState;
use wayland_client::{Connection, Dispatch, QueueHandle};
pub use wayland_protocols::ext::background_effect::v1::client::{
    ext_background_effect_manager_v1::{self, ExtBackgroundEffectManagerV1},
    ext_background_effect_surface_v1::{self, ExtBackgroundEffectSurfaceV1},
};

// The compositor blurs what's under the region a surface asks for (ext-background-effect-v1).
#[derive(Debug, Clone, Copy)]
pub struct Blur;

impl<T> Dispatch<ExtBackgroundEffectManagerV1, Blur> for WindowState<T> {
    fn event(
        _: &mut Self,
        _: &ExtBackgroundEffectManagerV1,
        _: ext_background_effect_manager_v1::Event,
        _: &Blur,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl<T> Dispatch<ExtBackgroundEffectSurfaceV1, Blur> for WindowState<T> {
    fn event(
        _: &mut Self,
        _: &ExtBackgroundEffectSurfaceV1,
        _: ext_background_effect_surface_v1::Event,
        _: &Blur,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
