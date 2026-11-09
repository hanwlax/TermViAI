#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
mod app;
mod terminal_view;
use gpui::*;
use gpui_component::Root;
use std::borrow::Cow;
struct Assets;
impl AssetSource for Assets {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>> {
        if path == "termviai/reconnect.svg" {
            return Ok(Some(Cow::Borrowed(br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"><path d="M20 7A8.5 8.5 0 1 0 20.5 16M20 3v5h-5"/></svg>"#)));
        }
        if path == "termviai/key.svg" {
            return Ok(Some(Cow::Borrowed(br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="8" cy="8" r="5"/><path d="m12 12 9 9m-5-5 3-3m0 6 3-3"/></svg>"#)));
        }
        if path == "termviai/broadcast.svg" {
            return Ok(Some(Cow::Borrowed(br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="black" stroke-width="2" stroke-linecap="round"><circle cx="12" cy="12" r="2"/><path d="M7.8 7.8a6 6 0 0 0 0 8.4m8.4-8.4a6 6 0 0 1 0 8.4M4.5 4.5a10.6 10.6 0 0 0 0 15m15-15a10.6 10.6 0 0 1 0 15"/></svg>"#)));
        }
        gpui_component_assets::Assets.load(path)
    }
    fn list(&self, path: &str) -> anyhow::Result<Vec<SharedString>> {
        gpui_component_assets::Assets.list(path)
    }
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
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
                    title: Some("TermViAI · Native GPUI Trial".into()),
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
