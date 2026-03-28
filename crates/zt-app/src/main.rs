use gpui::*;

fn main() {
    env_logger::init();

    zt_typst::world::warm_font_cache();

    Application::new()
        .with_assets(gpui_component_assets::Assets)
        .run(|cx: &mut App| {
        gpui_component::init(cx);

        // Force dark mode and apply Catppuccin Macchiato colors
        {
            use gpui_component::theme::{Theme, ThemeMode};
            Theme::change(ThemeMode::Dark, None, cx);
            let t = cx.global_mut::<Theme>();
            let c = &mut t.colors;

            // Catppuccin Macchiato palette
            let base: Hsla     = rgb(0x24273a).into();
            let mantle: Hsla   = rgb(0x1e2030).into();
            let crust: Hsla    = rgb(0x181926).into();
            let surface0: Hsla = rgb(0x363a4f).into();
            let surface1: Hsla = rgb(0x494d64).into();
            let surface2: Hsla = rgb(0x5b6078).into();
            let overlay0: Hsla = rgb(0x6e738d).into();
            let subtext0: Hsla = rgb(0xa5adcb).into();
            let text: Hsla     = rgb(0xcad3f5).into();
            let rosewater: Hsla = rgb(0xf4dbd6).into();
            let lavender: Hsla = rgb(0xb7bdf8).into();
            let red: Hsla      = rgb(0xed8796).into();
            let peach: Hsla    = rgb(0xf5a97f).into();
            let yellow: Hsla   = rgb(0xeed49f).into();
            let green: Hsla    = rgb(0xa6da95).into();
            let teal: Hsla     = rgb(0x8bd5ca).into();
            let blue: Hsla     = rgb(0x8aadf4).into();
            let mauve: Hsla    = rgb(0xc6a0f6).into();

            // Core
            c.background = base;
            c.foreground = text;
            c.border = surface0;
            c.input = surface0;
            c.caret = rosewater;
            c.selection = surface0;
            c.overlay = crust;

            // Accent
            c.accent = blue;
            c.accent_foreground = base;

            // Links
            c.link = blue;
            c.link_active = lavender;
            c.link_hover = lavender;

            // Muted
            c.muted = surface0;
            c.muted_foreground = subtext0;

            // Primary / Secondary
            c.primary = mauve;
            c.primary_foreground = base;
            c.secondary = surface0;
            c.secondary_foreground = text;

            // Danger / Warning / Info
            c.danger = red;
            c.danger_active = red;
            c.danger_foreground = base;
            c.danger_hover = red;
            c.warning = yellow;
            c.warning_active = yellow;
            c.warning_foreground = base;
            c.warning_hover = yellow;
            c.info = blue;
            c.info_active = blue;
            c.info_foreground = base;
            c.info_hover = blue;

            // Sidebar
            c.sidebar = mantle;
            c.sidebar_foreground = text;
            c.sidebar_border = surface0;
            c.sidebar_accent = blue;
            c.sidebar_accent_foreground = base;

            // Title bar
            c.title_bar = base;
            c.title_bar_border = surface0;

            // Tabs
            c.tab = mantle;
            c.tab_active = base;
            c.tab_active_foreground = text;
            c.tab_bar = mantle;
            c.tab_foreground = subtext0;

            // Popover — no shadow on context menus (flat dark theme)
            t.shadow = false;
            c.popover = surface0;
            c.popover_foreground = text;

            // List
            c.list = base;
            c.list_active = surface0;
            c.list_hover = surface0;
            c.list_even = mantle;
            c.list_head = mantle;

            // Table
            c.table = base;
            c.table_active = surface0;
            c.table_hover = surface0;
            c.table_even = mantle;
            c.table_head = mantle;
            c.table_head_foreground = text;
            c.table_row_border = surface0;

            // Scrollbar
            c.scrollbar = surface0;
            c.scrollbar_thumb = surface1;

            // Window
            c.window_border = surface0;

            // Named colors for syntax highlighting
            c.red = red;
            c.red_light = rgb(0xf5a8b8).into();
            c.green = green;
            c.green_light = rgb(0xb8e0aa).into();
            c.blue = blue;
            c.blue_light = lavender;
            c.yellow = yellow;
            c.yellow_light = rgb(0xf2ddb0).into();
            c.magenta = mauve;
            c.magenta_light = rgb(0xd4b8f9).into();
            c.cyan = teal;
            c.cyan_light = rgb(0xa4e0d6).into();

            // Accordion
            c.accordion = mantle;
            c.accordion_hover = surface0;

            // Drop target
            c.drop_target = surface0;
            c.drag_border = blue;

            // Editor highlight theme — gutter background and active line
            {
                use gpui_component::highlighter::{HighlightTheme, HighlightThemeStyle};
                let mut style = t.highlight_theme.style.clone();
                style.editor_background = Some(mantle);    // line-number gutter
                style.editor_foreground = Some(text);
                style.editor_active_line = Some(surface0); // active-line highlight
                t.highlight_theme = std::sync::Arc::new(HighlightTheme {
                    name: t.highlight_theme.name.clone(),
                    appearance: t.highlight_theme.appearance,
                    style,
                });
            }
        }

        // Register Typst's bundled fonts with GPUI
        let typst_fonts: Vec<std::borrow::Cow<'static, [u8]>> = typst_assets::fonts()
            .map(|data| std::borrow::Cow::Borrowed(data))
            .collect();
        if let Err(e) = cx.text_system().add_fonts(typst_fonts) {
            log::error!("Failed to register Typst fonts with GPUI: {e}");
        }

        let vault_root = std::env::args().nth(1).map(std::path::PathBuf::from);

        if let Some(ref root) = vault_root {
            log::info!("Vault root: {}", root.display());
        }

        let bounds = Bounds::centered(None, size(px(1400.0), px(900.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(TitlebarOptions {
                    title: Some("Zetteltypsten".into()),
                    appears_transparent: true,
                    traffic_light_position: Some(point(px(8.0), px(8.0))),
                }),
                ..Default::default()
            },
            |window, cx| {
                let workspace: Entity<zt_ui::Workspace> = cx.new(|cx| {
                    zt_ui::Workspace::new(vault_root.clone(), cx)
                });
                let view: AnyView = workspace.clone().into();
                // Store workspace handle for global init (done after window opens)
                window.on_next_frame(move |_window, cx| {
                    zt_ui::init(cx, workspace);
                });
                cx.new(|cx| gpui_component::Root::new(view, window, cx))
            },
        )
        .unwrap();

        cx.activate(true);
    });
}
