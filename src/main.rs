//! OpenSnake: a snake game inspired by Google Snake.

use macroquad::prelude::*;
use opensnake::app::App;

fn conf() -> macroquad::conf::Conf {
    let window = Conf {
        window_title: "OpenSnake".into(),
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
    if app.settings.fullscreen {
        set_fullscreen(true);
    }
    loop {
        app.frame();
        if app.quit {
            break;
        }
        next_frame().await;
    }
}
