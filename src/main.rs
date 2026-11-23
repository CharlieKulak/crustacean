//! Crustacean: a snake game, and a client for OpenSnake servers.

use macroquad::prelude::*;
use crustacean::app::App;

fn conf() -> macroquad::conf::Conf {
    let window = Conf {
        window_title: "Crustacean".into(),
        window_width: 1280,
        window_height: 760,
        high_dpi: true,
        sample_count: 4,
        window_resizable: true,
        ..Default::default()
    };
    macroquad::conf::Conf {
        miniquad_conf: window,
        // (the UI draws lots of smooth shapes: give each draw call plenty of room)
        draw_call_vertex_capacity: 60_000,
        draw_call_index_capacity: 90_000,
        ..Default::default()
    }
}

#[macroquad::main(conf)]
async fn main() {
    // macroquad's own sound thread can't start where ALSA can't reach the sound card
    // (the game plays sound another way there): don't print its panic.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if info.location().is_some_and(|l| l.file().contains("quad-snd")) {
            return;
        }
        default_hook(info);
    }));
    let mut app = App::new().await;
    // A browser only goes full screen right after a click or a key press (asking before
    // one is refused, and on Chromebooks flickers): there, wait for the first one.
    let mut want_fullscreen = app.settings.fullscreen;
    if want_fullscreen && !cfg!(target_arch = "wasm32") {
        set_fullscreen(true);
        want_fullscreen = false;
    }
    loop {
        if want_fullscreen
            && (is_mouse_button_pressed(MouseButton::Left) || get_last_key_pressed().is_some() || !touches().is_empty())
        {
            want_fullscreen = false;
            if app.settings.fullscreen {
                set_fullscreen(true);
            }
        }
        app.frame();
        if app.quit {
            break;
        }
        next_frame().await;
    }
}
