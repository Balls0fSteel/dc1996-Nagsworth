//! Prints one platform sample per second: `cargo run -p an-platform --example probe [count]`.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn main() {
    let count: u32 = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(3);
    let platform = an_platform::create_platform();
    println!("caps: {:?}", platform.caps());
    println!("steam: {:?}", platform.steam_info());
    println!("exclusive fullscreen: {}", platform.exclusive_fullscreen());
    for i in 0..count {
        if i > 0 {
            std::thread::sleep(Duration::from_secs(1));
        }
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64;
        let s = platform.sample(ts);
        match &s.fg {
            Some(w) => println!(
                "idle {:>6} ms  {:?}  {}  pid {}  elevated {}  fullscreen {}  pkg {:?}  \"{}\"",
                s.idle_ms,
                s.session,
                w.process.exe_name,
                w.process.pid,
                w.process.is_elevated,
                w.is_fullscreen,
                w.process.package_id,
                w.title.chars().take(60).collect::<String>()
            ),
            None => println!(
                "idle {:>6} ms  {:?}  <no foreground window>",
                s.idle_ms, s.session
            ),
        }
    }
}
