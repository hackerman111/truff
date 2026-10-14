use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use truff_core::commands::Action;
use truff_ui::{app::Application, shell::ShellAction};

const NOTES: usize = 32;
const NOTE_BYTES: usize = 512 * 1024;

struct TempVault(PathBuf);

impl TempVault {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let path = std::env::temp_dir().join(format!(
            "truff-buffer-residency-{}-{}",
            std::process::id(),
            SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
        ));
        std::fs::create_dir(&path)?;
        Ok(Self(path))
    }
}

impl Drop for TempVault {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn wait_until_idle(app: &mut Application) -> Result<(), Box<dyn std::error::Error>> {
    let deadline = Instant::now() + Duration::from_secs(15);
    while app.needs_poll() {
        app.poll();
        if Instant::now() >= deadline {
            return Err("application did not become idle within 15 seconds".into());
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    Ok(())
}

fn process_memory_bytes() -> (u64, u64) {
    #[cfg(target_os = "linux")]
    {
        let contents = std::fs::read_to_string("/proc/self/smaps_rollup").unwrap_or_default();
        let value = |name: &str| {
            contents
                .lines()
                .find_map(|line| {
                    let mut fields = line.split_whitespace();
                    if fields.next()? != name {
                        return None;
                    }
                    Some(fields.next()?.parse::<u64>().ok()?.saturating_mul(1024))
                })
                .unwrap_or(0)
        };
        (value("Rss:"), value("Pss:"))
    }
    #[cfg(not(target_os = "linux"))]
    {
        (0, 0)
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let vault = TempVault::new()?;
    let mut content = String::with_capacity(NOTE_BYTES);
    while content.len() + 32 <= NOTE_BYTES {
        content.push_str("# Section\nMarkdown paragraph with retained text.\n\n");
    }
    content.truncate(NOTE_BYTES);
    for index in 0..NOTES {
        std::fs::write(vault.0.join(format!("note-{index:02}.md")), &content)?;
    }

    let mut app = Application::new();
    app.open_vault(&vault.0)?;
    wait_until_idle(&mut app)?;
    for index in 0..NOTES {
        app.open_note(&format!("note-{index:02}.md"))?;
        wait_until_idle(&mut app)?;
        let view = app
            .shell
            .editor_view()
            .map(|view| view.id())
            .ok_or("opened note has no active view")?;
        app.handle_shell_action(ShellAction::Core(Action::CloseView(view)));
        wait_until_idle(&mut app)?;
    }

    let mut resident_buffers = 0usize;
    let mut resident_text_bytes = 0usize;
    let mut history_bytes = 0usize;
    for entry in app.shell.workspace().buffers().entries() {
        if let Some(buffer) = entry.buffer() {
            resident_buffers += 1;
            resident_text_bytes += buffer.byte_len();
            history_bytes += buffer.history().total_bytes();
        }
    }
    let (rss_bytes, pss_bytes) = process_memory_bytes();
    println!("scenario,resident_buffers,resident_text_bytes,history_bytes,rss_bytes,pss_bytes");
    println!(
        "closed_32x512k,{resident_buffers},{resident_text_bytes},{history_bytes},{rss_bytes},{pss_bytes}"
    );
    app.shutdown()?;
    Ok(())
}
