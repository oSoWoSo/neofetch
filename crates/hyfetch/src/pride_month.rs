use std::io::{self, Write as _};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use anyhow::{Context as _, Result};
use palette::blend::Blend as _;
use palette::{LinSrgba, Srgb, WithAlpha as _};
use strum::VariantArray as _;
use terminal_size::{terminal_size, Height, Width};

use crate::color_util::{clear_screen, color, printc, ForegroundBackground, ToAnsiString as _};
use crate::presets::Preset;
use crate::types::AnsiMode;

const TEXT_ASCII: &str = r"
.======================================================.
| .  .              .__       .     .  .       , .   | |
| |__| _.._ ._   .  [__)._.* _| _   |\/| _ ._ -+-|_  | |
| |  |(_][_)[_)\_|  |   [  |(_](/,  |  |(_)[ ) | [ ) * |
|        |  |  ._|                                     |
'======================================================'
";

const NOTICE: &str = "Press enter to continue";

#[allow(clippy::too_many_lines)]
pub fn start_animation(color_mode: AnsiMode) -> Result<()> {
    let key_pressed = Arc::new(AtomicBool::new(false));
    let mut input: String = String::new();
    // TODO: use non-blocking I/O; no need for another thread
    let _handle = thread::spawn({
        let key_pressed = Arc::clone(&key_pressed);
        move || {
            loop {
                match io::stdin().read_line(&mut input) {
                    Ok(0) => {
                        // Ignore EOF
                    },
                    Ok(_) => {
                        key_pressed.store(true, Ordering::Release);
                        break;
                    },
                    Err(err) => {
                        eprintln!("failed to read line from standard input: {err}");
                    },
                }
            }
        }
    });

    let text = &TEXT_ASCII[1..TEXT_ASCII.len() - 1];
    let text_lines: Vec<&str> = text.split('\n').collect();
    let text_height: usize = text_lines.len();
    let text_width: usize = text_lines[0].len();

    let speed = 2;
    let frame_delay: Duration = Duration::from_secs_f32(1.0 / 25.0);

    let mut frame: usize = 0;

    let (w, h) = terminal_size()
        .map(|(Width(w), Height(h))| (usize::from(w), usize::from(h)))
        .context("failed to get terminal size")?;
    const BLOCKS: usize = 9;
    let block_width: usize = w / BLOCKS;

    let text_start_y = (h / 2) - (text_height / 2);
    let text_end_y = text_start_y + text_height;
    let text_start_x = (w / 2) - (text_width / 2);
    let text_end_x = text_start_x + text_width;

    let notice_start_x = w - NOTICE.len() - 1;
    let notice_end_x = w - 1;
    let notice_y = h - 1;

    // Add every preset to colors
    let colors: Vec<Srgb<u8>> = Preset::VARIANTS
        .iter()
        .flat_map(|p| p.color_profile().colors)
        .collect();

    let fg: Srgb<u8> = "#FFE09B"
        .parse()
        .expect("foreground color hex should be valid");
    let black = LinSrgba::new(0.0, 0.0, 0.0, 0.5);

    let draw_frame = |frame: usize| -> Result<()> {
        let mut buf = String::new();

        // Loop over the height
        for y in 0..h {
            // Print the starting color
            buf += &colors[((frame + y) / block_width) % colors.len()]
                .to_ansi_string(color_mode, ForegroundBackground::Background);
            buf += &fg.to_ansi_string(color_mode, ForegroundBackground::Foreground);

            // Loop over the width
            for x in 0..w {
                let idx = frame + x + y + (2.0 * (y as f64 + 0.5 * frame as f64).sin()) as usize;
                let y_text = text_start_y <= y && y < text_end_y;

                let border = 1 + usize::from(!(y == text_start_y || y == text_end_y - 1));

                // If it's a switching point
                if idx % block_width == 0
                    || x == text_start_x - border
                    || x == text_end_x + border
                    || x == notice_start_x - 1
                    || x == notice_end_x + 1
                {
                    // Print the color at the current frame
                    let c = colors[(idx / block_width) % colors.len()];
                    if (y_text && (text_start_x - border <= x) && (x < text_end_x + border))
                        || (y == notice_y && notice_start_x - 1 <= x && x < notice_end_x + 1)
                    {
                        let c: LinSrgba = c.with_alpha(1.0).into_linear();
                        let c = Srgb::<u8>::from_linear(c.overlay(black).without_alpha());
                        buf += &c.to_ansi_string(color_mode, ForegroundBackground::Background);
                    } else {
                        buf += &c
                            .into_format()
                            .to_ansi_string(color_mode, ForegroundBackground::Background);
                    }
                }

                // If text should be printed, print text
                if y_text && text_start_x <= x && x < text_end_x {
                    buf.push(
                        text_lines[y - text_start_y]
                            .chars()
                            .nth(x - text_start_x)
                            .unwrap(),
                    );
                } else if y == notice_y && notice_start_x <= x && x < notice_end_x {
                    buf.push(NOTICE.chars().nth(x - notice_start_x).unwrap());
                } else {
                    buf.push(' ');
                }
            }

            // New line if it isn't the last line
            if y != h - 1 {
                buf += &color("&r\n", color_mode)
                    .expect("line separator should not contain invalid color codes");
            }
        }

        write!(io::stdout(), "{buf}")
            .and_then(|_| io::stdout().flush())
            .context("failed to write `buf` to stdout")?;

        Ok(())
    };

    loop {
        // Clear the screen
        clear_screen(None, color_mode, false).context("failed to clear screen")?;

        draw_frame(frame)?;
        frame += speed;
        thread::sleep(frame_delay);

        // TODO: handle Ctrl+C so that we can clear the screen; but we don't have a nice
        // way to unregister the signal handler after that :'(
        // See https://github.com/Detegr/rust-ctrlc/issues/106
        if key_pressed.load(Ordering::Acquire) {
            break;
        }
    }

    // Clear the screen
    printc("&r", color_mode).context("failed to reset terminal style")?;
    clear_screen(None, color_mode, false).context("failed to clear screen")?;

    Ok(())
}
