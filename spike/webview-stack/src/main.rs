//! Does a stack of webviews behave like a workbench?
//!
//! One window, two child webviews. The lower one holds a real web page; the
//! upper one is transparent and holds the chrome that has to draw over it —
//! context menus, modals, tooltips. The page's own script is replaced by
//! nothing: a shim is injected into it that takes back the keys and the
//! right-click the workbench claims and hands them to the host.
//!
//! Five questions, and the program answers four of them out loud:
//!
//!   1. does a page that refuses to be framed still render      (automatic)
//!   2. does a chord pressed inside that page reach the host    (automatic)
//!   3. does a right-click inside that page reach the host      (automatic)
//!   4. does a click reach the page with the overlay idle       (automatic)
//!   5. does the overlay draw above the page, transparently     (look at it)
//!
//! The fifth is the one that decides the architecture, and it is the one no
//! program can answer for you.

use std::sync::mpsc;

use serde::Deserialize;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
use winit::window::{Window, WindowId};
use wry::dpi::{LogicalPosition, LogicalSize};
use wry::{Rect, WebView, WebViewBuilder};

/// A page that sets `X-Frame-Options: DENY`. If it renders, this is not an
/// iframe — which is the premise the whole idea rests on, so it is the first
/// thing checked and it is checked by loading the hardest case rather than an
/// agreeable one.
const CONTENT_URL: &str = "https://github.com/PowderworksCode/immersion";

const GUEST_JS: &str = include_str!("guest.js");
const OVERLAY_HTML: &str = include_str!("overlay.html");

/// What either webview can tell the host. One shape for both, because the
/// host does not care which webview a message came from — only what happened.
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Msg {
    Hello { url: String },
    Chord { chord: String, action: String },
    Menu { x: f64, y: f64 },
    Click,
    OverlayClick,
    Picked { label: String },
    Cleared,
}

/// What the run has proved so far. Printed as it happens and again at the end,
/// so a run that is closed early still says what it learned.
#[derive(Default)]
struct Findings {
    guest_loaded: bool,
    chord_forwarded: bool,
    menu_forwarded: bool,
    click_reached_page: bool,
    overlay_ate_a_click: bool,
}

impl Findings {
    fn report(&self) {
        println!("\n--- what this run proved ---");
        line(
            self.guest_loaded,
            "a page that refuses to be framed rendered, and our script ran inside it",
        );
        line(
            self.chord_forwarded,
            "a chord pressed inside that page reached the host",
        );
        line(
            self.menu_forwarded,
            "a right-click inside that page reached the host",
        );
        line(
            self.click_reached_page,
            "a click reached the page with the overlay idle",
        );
        if self.overlay_ate_a_click {
            println!(
                "  NOTE  the overlay swallowed a click while it was meant to be idle.\n\
                 \x20       Hiding it between uses is not enough for chrome that has to\n\
                 \x20       stay live — a tooltip that follows the pointer would need\n\
                 \x20       hit-testing (an NSView hitTest: override on macOS)."
            );
        }
        println!(
            "\n  BY EYE  did the magenta frame and the menu draw ABOVE the page,\n\
             \x20        and could you read the page through the dimmed backdrop?\n\
             \x20        That is z-order and transparency, and nothing here can\n\
             \x20        check it for you.\n"
        );
    }
}

fn line(ok: bool, what: &str) {
    println!("  {}  {what}", if ok { "PASS" } else { "----" });
}

struct App {
    proxy: EventLoopProxy<Msg>,
    window: Option<Window>,
    content: Option<WebView>,
    overlay: Option<WebView>,
    /// The overlay is hidden between uses rather than made click-through.
    /// Hiding needs no platform code; click-through needs an NSView subclass
    /// on macOS and a window style on Windows. If the spike shows that hiding
    /// is enough, that is a real simplification — and if it is not, the
    /// `p` key is here to find out.
    overlay_visible: bool,
    /// Keep the overlay up and empty, to see whether clicks reach the page
    /// through a transparent webview. This is the question that decides
    /// whether persistent chrome (tooltips) is possible without interop.
    passthrough_test: bool,
    findings: Findings,
}

impl App {
    fn new(proxy: EventLoopProxy<Msg>) -> Self {
        Self {
            proxy,
            window: None,
            content: None,
            overlay: None,
            overlay_visible: false,
            passthrough_test: false,
            findings: Findings::default(),
        }
    }

    fn show_overlay(&mut self, show: bool) {
        self.overlay_visible = show;
        if let Some(overlay) = &self.overlay {
            let _ = overlay.set_visible(show);
        }
    }

    /// Both webviews fill the window, so a resize is the same rect twice.
    fn fit(&self) {
        let Some(window) = &self.window else { return };
        let size = window.inner_size();
        let rect = Rect {
            position: LogicalPosition::new(0.0, 0.0).into(),
            size: LogicalSize::new(size.width as f64, size.height as f64).into(),
        };
        for view in [self.content.as_ref(), self.overlay.as_ref()].into_iter().flatten() {
            let _ = view.set_bounds(rect);
        }
    }
}

impl ApplicationHandler<Msg> for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("webview stack — spike")
            .with_inner_size(LogicalSize::new(1200.0, 800.0));
        let window = event_loop.create_window(attrs).expect("window");

        let full = Rect {
            position: LogicalPosition::new(0.0, 0.0).into(),
            size: LogicalSize::new(1200.0, 800.0).into(),
        };

        // Order matters: the one built second is expected to stack above. If
        // that turns out to be false, this is the line to argue with.
        let to_host = self.proxy.clone();
        let content = WebViewBuilder::new()
            .with_url(CONTENT_URL)
            .with_bounds(full)
            .with_initialization_script(GUEST_JS)
            .with_ipc_handler(move |req| forward(&to_host, req.body()))
            .build_as_child(&window)
            .expect("content webview");

        let to_host = self.proxy.clone();
        let overlay = WebViewBuilder::new()
            .with_html(OVERLAY_HTML)
            .with_bounds(full)
            .with_transparent(true)
            .with_ipc_handler(move |req| forward(&to_host, req.body()))
            .build_as_child(&window)
            .expect("overlay webview");
        let _ = overlay.set_visible(false);

        println!("loading {CONTENT_URL}\n");
        println!("  inside the page:  F3  a modal over the web pane");
        println!("                right-click  a context menu at the pointer");
        println!("                     click  proves the page is reachable");
        println!("                         p  keep the overlay up and empty");
        println!("                    Escape  dismiss\n");

        self.window = Some(window);
        self.content = Some(content);
        self.overlay = Some(overlay);
    }

    fn user_event(&mut self, _event_loop: &ActiveEventLoop, msg: Msg) {
        match msg {
            Msg::Hello { url } => {
                self.findings.guest_loaded = true;
                line(true, &format!("guest script ran in {url}"));
            }
            Msg::Chord { chord, action } => {
                self.findings.chord_forwarded = true;
                line(true, &format!("chord {chord} -> {action} (from inside the page)"));
                match action.as_str() {
                    "palette" => {
                        self.show_overlay(true);
                        if let Some(o) = &self.overlay {
                            let _ = o.evaluate_script("window.imOverlay.card('Command palette')");
                        }
                    }
                    "dismiss" => {
                        self.passthrough_test = false;
                        self.show_overlay(false);
                    }
                    "passthrough" => {
                        self.passthrough_test = !self.passthrough_test;
                        self.show_overlay(self.passthrough_test);
                        if let Some(o) = &self.overlay {
                            let _ = o.evaluate_script("window.imOverlay.clear()");
                        }
                        println!(
                            "  TEST  overlay is {} and empty — now click a link on the page.\n\
                             \x20       a click that lands is passthrough; one that does not\n\
                             \x20       means persistent chrome needs hit-testing.",
                            if self.passthrough_test { "UP" } else { "down" }
                        );
                    }
                    _ => {}
                }
            }
            Msg::Menu { x, y } => {
                self.findings.menu_forwarded = true;
                line(true, &format!("right-click at ({x:.0}, {y:.0}) (from inside the page)"));
                self.show_overlay(true);
                if let Some(o) = &self.overlay {
                    let _ = o.evaluate_script(&format!("window.imOverlay.menu({x}, {y})"));
                }
            }
            Msg::Click => {
                if !self.findings.click_reached_page {
                    self.findings.click_reached_page = true;
                    line(true, "a click reached the page");
                }
                if self.passthrough_test {
                    println!("  PASS  ...and it reached the page THROUGH the idle overlay");
                }
            }
            Msg::OverlayClick => {
                if self.passthrough_test {
                    self.findings.overlay_ate_a_click = true;
                    println!("  FAIL  the idle overlay swallowed that click");
                }
            }
            Msg::Picked { label } => println!("  ---   menu pick: {label}"),
            Msg::Cleared => {
                if !self.passthrough_test {
                    self.show_overlay(false);
                }
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                self.findings.report();
                event_loop.exit();
            }
            WindowEvent::Resized(_) => self.fit(),
            _ => {}
        }
    }
}

/// A webview said something. Anything unparseable is printed rather than
/// swallowed — a spike that hides its own confusion is not worth running.
fn forward(proxy: &EventLoopProxy<Msg>, body: &str) {
    match serde_json::from_str::<Msg>(body) {
        Ok(msg) => {
            let _ = proxy.send_event(msg);
        }
        Err(e) => println!("  ???   unparsed ipc: {body} ({e})"),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::<Msg>::with_user_event().build()?;
    let mut app = App::new(event_loop.create_proxy());
    event_loop.run_app(&mut app)?;
    Ok(())
}

#[cfg(test)]
mod protocol {
    use super::*;

    /// The guest script and this enum are a wire protocol split across a .js
    /// file and a Rust type, with a JSON string in the middle and nothing
    /// checking that they agree. Every kind the scripts can send has to parse
    /// here, or the message is dropped at runtime with a line in a console
    /// nobody is reading.
    #[test]
    fn every_kind_the_scripts_send_is_one_this_understands() {
        let samples = [
            r#"{"kind":"hello","url":"https://example.com","title":"x"}"#,
            r#"{"kind":"chord","chord":"F3","action":"palette"}"#,
            r#"{"kind":"menu","x":120.0,"y":340.0}"#,
            r#"{"kind":"click"}"#,
            r#"{"kind":"overlay_click"}"#,
            r#"{"kind":"picked","label":"Split H"}"#,
            r#"{"kind":"cleared"}"#,
        ];
        for s in samples {
            serde_json::from_str::<Msg>(s).unwrap_or_else(|e| panic!("{s} does not parse: {e}"));
        }

        // And the other direction: a kind emitted by a script with no sample
        // above is one this test is not actually covering.
        let sources = format!("{GUEST_JS}{OVERLAY_HTML}");
        let mut emitted: Vec<&str> = sources
            .match_indices("kind: \"")
            .map(|(i, _)| {
                let rest = &sources[i + "kind: \"".len()..];
                &rest[..rest.find('"').expect("a closing quote")]
            })
            .collect();
        emitted.sort_unstable();
        emitted.dedup();
        for kind in emitted {
            assert!(
                samples.iter().any(|s| s.contains(&format!("\"{kind}\""))),
                "the scripts send {kind:?}, which nothing here parses"
            );
        }
    }

    /// The one the overlay calls itself. Rust builds this string by hand, so
    /// a rename on either side is a menu that silently never opens.
    #[test]
    fn the_host_calls_functions_the_overlay_defines() {
        for call in ["window.imOverlay.menu(", "window.imOverlay.card(", "window.imOverlay.clear("] {
            let name = call
                .trim_start_matches("window.imOverlay.")
                .trim_end_matches('(');
            assert!(
                OVERLAY_HTML.contains(&format!("{name}(")),
                "the host calls imOverlay.{name}, which the overlay does not define"
            );
        }
    }
}
