//! Shows what the platform layer sees.
//!
//! ```text
//! cargo run -p an-platform --example probe [count]       one sample per second
//! cargo run -p an-platform --example probe -- --session 30   print session events for 30 s
//! cargo run -p an-platform --example probe -- --autostart    turn autostart on, check, turn off
//! cargo run -p an-platform --example probe -- --lock         lock the workstation
//! ```

use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let platform = an_platform::create_platform();
    println!("caps: {:?}", platform.caps());

    match args.first().map(String::as_str) {
        Some("--session") => {
            let secs: u64 = args.get(1).and_then(|a| a.parse().ok()).unwrap_or(30);
            match platform.session_events(Box::new(|e| println!("session event: {e:?}"))) {
                Ok(()) => println!("watching session events for {secs} s"),
                Err(e) => println!("session events unavailable: {e}"),
            }
            for _ in 0..secs {
                std::thread::sleep(Duration::from_secs(1));
            }
            println!("session state: {:?}", platform.session_state());
            return;
        }
        Some("--autostart") => {
            println!("autostart before: {:?}", platform.autostart());
            println!("enable: {:?}", platform.set_autostart(true));
            println!("autostart now: {:?}", platform.autostart());
            println!("disable: {:?}", platform.set_autostart(false));
            println!("autostart after: {:?}", platform.autostart());
            return;
        }
        Some("--lock") => {
            println!("lock: {:?}", platform.lock_workstation());
            return;
        }
        _ => {}
    }

    let count: u32 = args.first().and_then(|a| a.parse().ok()).unwrap_or(3);
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
