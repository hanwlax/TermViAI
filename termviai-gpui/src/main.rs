#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
mod app;
mod terminal_view;
use gpui::*;
use gpui_component::Root;
use std::borrow::Cow;
struct Assets;
impl AssetSource for Assets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        if path == "termviai/clock.svg" {
            return Ok(Some(Cow::Borrowed(br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><path d="M12 6v6l4 2"/></svg>"#)));
        }
        if path == "termviai/reconnect.svg" {
            return Ok(Some(Cow::Borrowed(br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M20 7A8.5 8.5 0 1 0 20.5 16M20 3v5h-5"/></svg>"#)));
        }
        if path == "termviai/key.svg" {
            return Ok(Some(Cow::Borrowed(br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="8" cy="8" r="5"/><path d="m12 12 9 9m-5-5 3-3m0 6 3-3"/></svg>"#)));
        }
        if path == "termviai/broadcast.svg" {
            return Ok(Some(Cow::Borrowed(br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="2" stroke-linecap="round"><circle cx="12" cy="12" r="2"/><path d="M7.8 7.8a6 6 0 0 0 0 8.4m8.4-8.4a6 6 0 0 1 0 8.4M4.5 4.5a10.6 10.6 0 0 0 0 15m15-15a10.6 10.6 0 0 1 0 15"/></svg>"#)));
        }
        let state_shape = match path {
            "termviai/list-filter.svg" => Some(r#"<path d="M4 6h16M7 12h10M10 18h4"/>"#),
            "termviai/server.svg" => Some(
                r#"<rect x="3" y="3" width="18" height="7" rx="2"/><rect x="3" y="14" width="18" height="7" rx="2"/><path d="M7 6.5h.01M7 17.5h.01"/>"#,
            ),
            "termviai/link-off.svg" => Some(
                r#"<path d="m3 3 18 18M10 13a5 5 0 0 0 7 .5l3-3a5 5 0 0 0-7-7l-1 1M14 11a5 5 0 0 0-7-.5l-3 3a5 5 0 0 0 7 7l1-1"/>"#,
            ),
            "termviai/wifi-off.svg" => Some(
                r#"<path d="m2 2 20 20M2 8.8a16 16 0 0 1 4.6-2.7M9.5 5.2a16 16 0 0 1 12.5 3.6M5 12a11 11 0 0 1 4-2M13 10a11 11 0 0 1 6 2M8.5 15.5a6 6 0 0 1 7 0M12 19.5h.01"/>"#,
            ),
            "termviai/layers.svg" => {
                Some(r#"<path d="m12 3 10 5-10 5L2 8Zm-10 9 10 5 10-5M2 16l10 5 10-5"/>"#)
            }
            "termviai/waypoints.svg" => Some(
                r#"<circle cx="12" cy="4" r="2"/><circle cx="4" cy="18" r="2"/><circle cx="20" cy="18" r="2"/><path d="m11 6-6 10m8-10 6 10M6 18h12"/>"#,
            ),
            _ => None,
        };
        if let Some(shape) = state_shape {
            return Ok(Some(Cow::Owned(format!(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">{shape}</svg>"#).into_bytes())));
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
