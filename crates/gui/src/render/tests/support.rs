use super::*;

pub(super) fn census_app() -> feral_processes_app_core::App {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    // Keyed by a counter as well as the pid: six tests call this, the
    // harness runs them on parallel threads, and a directory shared
    // across them raced — one thread's `remove_dir_all` landing between
    // another's failed `mkdir` and the `is_dir` check that would have
    // forgiven it, which surfaces as `AlreadyExists` from
    // `create_dir_all` and reads as anything but a fixture fault.
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let nth = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let tmp = std::env::temp_dir().join(format!("fp_gui_census_{}_{nth}", std::process::id()));
    std::fs::create_dir_all(&tmp).unwrap();
    let mut app = feral_processes_app_core::App::new(
        root.join("assets"),
        tmp.join("saves"),
        tmp.join("history.log"),
        tmp.join("profile.ron"),
        root.join("dev-arenas"),
        tmp.join("telemetry.jsonl"),
    );
    // [N] opens the wizard, and then every step is answered the
    // cheapest legal way — there is no key that skips it, and the two
    // steps that hand out an allowance will not be left until it is
    // spent.
    app.handle_key(feral_processes_app_core::GameKey::Char('n'));
    for step in feral_processes_app_core::CreationStep::ALL {
        match step {
            feral_processes_app_core::CreationStep::Difficulty => {
                app.handle_key(feral_processes_app_core::GameKey::Char('f'))
            }
            feral_processes_app_core::CreationStep::Profile => {
                app.handle_key(feral_processes_app_core::GameKey::Enter)
            }
            feral_processes_app_core::CreationStep::Class => {
                app.handle_key(feral_processes_app_core::GameKey::Enter)
            }
            feral_processes_app_core::CreationStep::Kit
            | feral_processes_app_core::CreationStep::Points => {
                for i in 0..app.creation_rows().len() {
                    app.menu_selected = i;
                    app.handle_key(feral_processes_app_core::GameKey::ShiftRight);
                }
                app.menu_selected = 0;
                app.handle_key(feral_processes_app_core::GameKey::Enter);
            }
            feral_processes_app_core::CreationStep::Icon
            | feral_processes_app_core::CreationStep::Colour => {
                app.handle_key(feral_processes_app_core::GameKey::Char('n'))
            }
            feral_processes_app_core::CreationStep::Perks => {
                app.handle_key(feral_processes_app_core::GameKey::Enter)
            }

            feral_processes_app_core::CreationStep::Summary
            | feral_processes_app_core::CreationStep::Name => {
                app.handle_key(feral_processes_app_core::GameKey::Enter)
            }
        }
    }
    // The game menu remembers whether it was opened from a run, and only
    // then has a Save & quit page; open and close it once from the map so
    // that page exists for the censuses that set the mode directly.
    // A fresh run can hold something an Esc dismisses first.
    for _ in 0..4 {
        if app.mode == Mode::GameplayOptions {
            break;
        }
        app.handle_key(feral_processes_app_core::GameKey::Esc);
    }
    assert_eq!(
        app.mode,
        Mode::GameplayOptions,
        "Esc never reached the game menu"
    );
    app.handle_key(feral_processes_app_core::GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing, "census app must start on the map");
    app
}
