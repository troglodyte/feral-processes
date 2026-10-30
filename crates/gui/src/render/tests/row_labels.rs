use super::*;

#[test]
fn emulation_rows_are_labelled_on_the_battle_screen_only() {
    let option = EmulationOption {
        species: "x".into(),
        name: "Kestrel".to_string(),
        glyph: 'k',
        atk: 8,
        mitigation: 5,
    };
    assert_eq!(
        emulation_row_label(0, &option, true),
        "[1] Kestrel — ATK 8 MIT 5"
    );
    assert_eq!(
        emulation_row_label(0, &option, false),
        "Kestrel — ATK 8 MIT 5"
    );
}

/// Whether `src` calls `emulation_row_label` with `labelled` = `true`:
/// the text between the call's parentheses ends in a literal `true`.
fn labelled_emulation_call(src: &str) -> bool {
    src.match_indices("emulation_row_label(").any(|(at, m)| {
        let args = &src[at + m.len()..];
        let mut depth = 1;
        let end = args
            .find(|c| {
                match c {
                    '(' => depth += 1,
                    ')' => depth -= 1,
                    _ => {}
                }
                depth == 0
            })
            .unwrap_or(args.len());
        args[..end]
            .trim_end()
            .trim_end_matches(',')
            .ends_with("true")
    })
}

/// Only the classic battle screen labels its rows. Skipped by full path
/// (`render/battle.rs`, `render/mod.rs`, which defines the helper, and this
/// file, which names it), so a `battle.rs` or `mod.rs` that grows in a
/// subdirectory is still checked.
#[test]
fn no_list_renderer_outside_battle_names_menu_shortcut() {
    fn walk(dir: &std::path::Path, skip: &[std::path::PathBuf], hits: &mut Vec<String>) {
        for entry in std::fs::read_dir(dir).expect("render dir reads") {
            let path = entry.expect("dir entry").path();
            if path.is_dir() {
                walk(&path, skip, hits);
                continue;
            }
            if skip.contains(&path) || path.extension().is_none_or(|e| e != "rs") {
                continue;
            }
            let src = std::fs::read_to_string(&path).expect("source reads");
            if src.contains("menu_shortcut(")
                || src.contains("map(menu_shortcut)")
                || labelled_emulation_call(&src)
            {
                hits.push(path.display().to_string());
            }
        }
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/render");
    let skip = [
        root.join("battle.rs"),
        root.join("mod.rs"),
        root.join("tests/row_labels.rs"),
    ];
    let mut hits = Vec::new();
    walk(&root, &skip, &mut hits);
    assert!(hits.is_empty(), "labelled rows outside battle: {hits:?}");
}
