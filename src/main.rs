#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;
mod config;
mod monitor;
mod vlc;

const WINDOW_WIDTH: f32 = 700.0;
const WINDOW_HEIGHT: f32 = 450.0;

fn build_options(renderer: eframe::Renderer) -> eframe::NativeOptions {
    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_inner_size([WINDOW_WIDTH, WINDOW_HEIGHT])
        .with_min_inner_size([500.0, 350.0])
        .with_drag_and_drop(true);

    let mut centered = false;
    if let Some((x, y)) = monitor::center_on_cursor_monitor(WINDOW_WIDTH, WINDOW_HEIGHT) {
        viewport = viewport.with_position([x, y]);
    } else {
        centered = true;
    }

    let wgpu_options = eframe::egui_wgpu::WgpuConfiguration {
        wgpu_setup: eframe::egui_wgpu::WgpuSetup::CreateNew(
            eframe::egui_wgpu::WgpuSetupCreateNew {
                instance_descriptor: eframe::wgpu::InstanceDescriptor {
                    // Include ALL backends: DX12, DX11, Vulkan, GL, Metal
                    backends: eframe::wgpu::Backends::all(),
                    ..Default::default()
                },
                ..Default::default()
            },
        ),
        ..Default::default()
    };

    eframe::NativeOptions {
        viewport,
        centered,
        renderer,
        wgpu_options,
        ..Default::default()
    }
}

fn main() -> eframe::Result {
    // Try wgpu (DX12/Vulkan) first, fall back to glow (OpenGL) for max compatibility
    let result = eframe::run_native(
        "vlc2chromecast",
        build_options(eframe::Renderer::Wgpu),
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(eframe::egui::Visuals::dark());
            Ok(Box::new(app::VlcChromecastApp::new()))
        }),
    );

    match result {
        Ok(()) => Ok(()),
        Err(_) => eframe::run_native(
            "vlc2chromecast",
            build_options(eframe::Renderer::Glow),
            Box::new(|cc| {
                cc.egui_ctx.set_visuals(eframe::egui::Visuals::dark());
                Ok(Box::new(app::VlcChromecastApp::new()))
            }),
        ),
    }
}
