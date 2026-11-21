#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
mod app;
mod notice_view;
mod terminal_view;
use gpui::*;
use gpui_component::Root;
use std::borrow::Cow;
struct Assets;
impl AssetSource for Assets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        let asset: Option<&'static [u8]> = match path {
            "termviai/grip.svg" => Some(include_bytes!("../assets/lucide/grip-vertical.svg")),
            "termviai/clock.svg" => Some(include_bytes!("../assets/lucide/clock.svg")),
            "termviai/reconnect.svg" => Some(include_bytes!("../assets/lucide/refresh-cw.svg")),
            "termviai/key.svg" => Some(include_bytes!("../assets/lucide/key-round.svg")),
            "termviai/broadcast.svg" => Some(include_bytes!("../assets/lucide/radio.svg")),
            "termviai/list-filter.svg" => Some(include_bytes!("../assets/lucide/list-filter.svg")),
            "termviai/server.svg" => Some(include_bytes!("../assets/lucide/server.svg")),
            "termviai/link-off.svg" => Some(include_bytes!("../assets/lucide/link-2-off.svg")),
            "termviai/wifi-off.svg" => Some(include_bytes!("../assets/lucide/wifi-off.svg")),
            "termviai/layers.svg" => Some(include_bytes!("../assets/lucide/layers.svg")),
            "termviai/waypoints.svg" => Some(include_bytes!("../assets/lucide/waypoints.svg")),
            "termviai/pause.svg" => Some(include_bytes!("../assets/lucide/pause.svg")),
            "termviai/play.svg" => Some(include_bytes!("../assets/lucide/play.svg")),
            _ => None,
        };
        if let Some(bytes) = asset {
            return Ok(Some(Cow::Borrowed(bytes)));
        }
        gpui_component_assets::Assets.load(path)
    }
    fn list(&self, path: &str) -> anyhow::Result<Vec<SharedString>> {
        gpui_component_assets::Assets.list(path)
    }
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--version" || arg == "-V") {
        println!("TermViAI {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    if let Some(pair) = args.windows(2).find(|x| x[0] == "--data-dir") {
        unsafe {
            std::env::set_var("TERMVIAI_DATA_DIR", &pair[1]);
        }
    }
    let validation = args
        .windows(2)
        .find(|x| x[0] == "--validation-page")
        .map(|p| p[1].clone());
    if validation.is_some() {
        assert!(
            args.iter().any(|a| a == "--data-dir"),
            "Validation needs isolated --data-dir"
        );
    }
    let local = args.iter().any(|a| a == "--local");
    let _ = env_logger::try_init();
    Application::new().with_assets(Assets).run(move |cx| {
        gpui_component::init(cx);
        cx.bind_keys(termviai_gpui::terminal_keys::bindings());
        cx.bind_keys(termviai_gpui::tab_strip::bindings());
        cx.text_system()
            .add_fonts(vec![
                Cow::Borrowed(include_bytes!(
                    "../../assets/fonts/JetBrainsMono-Regular.ttf"
                )),
                Cow::Borrowed(include_bytes!("../../assets/fonts/JetBrainsMono-Bold.ttf")),
                Cow::Borrowed(include_bytes!(
                    "../../assets/fonts/JetBrainsMono-Italic.ttf"
                )),
                Cow::Borrowed(include_bytes!(
                    "../../assets/fonts/JetBrainsMono-BoldItalic.ttf"
                )),
                Cow::Borrowed(include_bytes!(
                    "../../assets/fonts/SymbolsNerdFontMono-Regular.ttf"
                )),
            ])
            .expect("Bundled terminal fonts");
        cx.on_window_closed(|cx| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        let bounds = Bounds::centered(None, size(px(1280.), px(820.)), cx);
        let result = cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("TermViAI".into()),
                    appears_transparent: true,
                    ..Default::default()
                }),
                window_min_size: Some(size(px(780.), px(520.))),
                ..Default::default()
            },
            move |w, cx| {
                let view = cx.new(|cx| app::AppView::new(w, cx));
                view.update(cx, |s, cx| {
                    s.initialize(local, w, cx);
                    if let Some(scene) = &validation {
                        s.validation_scene(scene, w, cx);
                    }
                });
                cx.new(|cx| Root::new(view, w, cx))
            },
        );
        if let Err(e) = result {
            eprintln!("{e:#}");
            cx.quit();
        } else {
            cx.activate(true);
        }
    });
}
