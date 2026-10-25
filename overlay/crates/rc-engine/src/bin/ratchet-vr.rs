//! `RatchetVR.exe`: the one-click player launcher of the VR release folder (`scripts/vr/Package.ps1`).
//!
//! It plays the role of `scripts/vr/Run.ps1` and of the ReRAC launcher (docs/plan/launcher_contract.md) for players:
//! - the game data lives in `data\` next to the executable. Without a complete extraction (no `extract-info.json`), it
//!   asks for the disc image (a `.iso` dropped onto the executable, the only `.iso` in its folder, or a file dialog)
//!   and runs the sibling `rerac-extract.exe extract --ntsc-only` once;
//! - it then starts the sibling `rerac.exe --data-dir data` with the VR environment of Run.ps1's launch modes, saves
//!   in `saves\<mode>\` next to the executable, and waits for it;
//! - errors are shown in a message box in plain words (extractor and runtime exit codes from the launcher contract).
//!
//! Mode: `normal`, or `debug` (the sandbox) when the executable's name contains "sandbox" or `--sandbox` is passed;
//! `--gold` selects `debug-gold`. A variable already set in the environment (for instance `RC_VR_DESKTOP_PREVIEW=1`)
//! is left as it is.

#[cfg(windows)]
fn main() { std::process::exit(launcher::run()); }

#[cfg(not(windows))]
fn main() {
    eprintln!("RatchetVR is the Windows release launcher; use scripts/vr/Run.ps1 elsewhere.");
    std::process::exit(1);
}

#[cfg(windows)]
mod launcher {
    use std::ffi::OsString;
    use std::path::{Path, PathBuf};
    use std::process::Command;

    const TITLE: &str = "Ratchet & Clank VR";

    pub fn run() -> i32 {
        let Ok(exe) = std::env::current_exe() else { return fail("Cannot find the game folder.") };
        let dir = exe.parent().map(Path::to_path_buf).unwrap_or_default();
        let args: Vec<OsString> = std::env::args_os().skip(1).collect();
        let stem = exe.file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
        let mode = if args.iter().any(|a| a == "--gold") {
            "debug-gold"
        } else if stem.contains("sandbox") || args.iter().any(|a| a == "--sandbox") {
            "debug"
        } else {
            "normal"
        };
        let data = dir.join("data");
        if !data.join("extract-info.json").is_file() {
            let code = extract(&dir, &data, &args);
            if code != 0 { return code; }
        }
        play(&dir, &data, mode)
    }

    /// First launch: copies the game out of the player's disc image into `data\`.
    fn extract(dir: &Path, data: &Path, args: &[OsString]) -> i32 {
        let iso = match find_iso(dir, args) {
            Some(iso) => iso,
            None => {
                let ok = win::message(
                    "Welcome! The first time, the game needs your own Ratchet & Clank disc image (.iso file).\n\n\
                     Pick it in the next window. The game is copied from it once: it takes a few minutes and \
                     about 5 GB of disk space.",
                    win::MB_OKCANCEL | win::MB_ICONINFORMATION,
                );
                if !ok { return 1; }
                match win::pick_iso() {
                    Some(iso) => iso,
                    None => return 1,
                }
            }
        };
        println!("Copying the game from {} into {} ...", iso.display(), data.display());
        let status = Command::new(dir.join("rerac-extract.exe"))
            .arg("extract").arg("--iso").arg(&iso).arg("--out").arg(data).arg("--ntsc-only")
            .status();
        let code = match status {
            Ok(s) => s.code().unwrap_or(1),
            Err(_) => return fail("rerac-extract.exe is missing. Unzip the whole download again, then retry."),
        };
        if code == 0 { return 0; }
        fail(match code {
            10 | 11 => "This file can't be read as a disc image. Only .iso images of the disc work.",
            20 => "This disc image isn't Ratchet & Clank.",
            21 => "This version of the disc isn't supported. Only the original US release (SCUS-97199) works for now.",
            30 => "The game files couldn't be written. Move the game folder somewhere you can write to, like your \
                   Desktop or Documents, then try again.",
            31 => "Not enough disk space. About 5 GB of free space is needed next to the game.",
            40 => "This disc image is damaged. Make a new copy of your disc, then try again.",
            _ => "Copying the game from the disc image failed. The details are in the black window.",
        })
    }

    /// A `.iso` dropped onto the executable, else the only `.iso` in its folder.
    fn find_iso(dir: &Path, args: &[OsString]) -> Option<PathBuf> {
        let is_iso = |p: &Path| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("iso")) && p.is_file();
        if let Some(p) = args.iter().map(PathBuf::from).find(|p| is_iso(p)) { return Some(p); }
        let found: Vec<PathBuf> = std::fs::read_dir(dir).ok()?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| is_iso(p))
            .collect();
        (found.len() == 1).then(|| found.into_iter().next().unwrap())
    }

    /// Starts the game in VR (Run.ps1's environment for `mode`) and waits for it.
    fn play(dir: &Path, data: &Path, mode: &str) -> i32 {
        let mut game = Command::new(dir.join("rerac.exe"));
        game.arg("--data-dir").arg(data).current_dir(dir);
        let sandbox = mode != "normal";
        let vars: [(&str, String); 8] = [
            ("RC_VR", "1".into()),
            ("RC_VR_MODE", mode.into()),
            ("RC_VR_SCENE", "kerwan".into()),
            ("RC_VR_UNITS_PER_METRE", "1".into()),
            ("RC_VR_DESKTOP_PREVIEW", "0".into()),
            ("RC_SAVE_DIR", dir.join("saves").join(mode).to_string_lossy().into_owned()),
            ("RC_FRONTEND", if sandbox { "0" } else { "1" }.into()),
            ("RC_UNLOCK_PLANETS", (0..=18).map(|p: u32| p.to_string()).collect::<Vec<_>>().join(",")),
        ];
        for (key, value) in vars {
            if key == "RC_UNLOCK_PLANETS" && !sandbox { continue; }
            if std::env::var_os(key).is_none() { game.env(key, value); }
        }
        if sandbox && std::env::var_os("RC_LEVEL").is_none() { game.env("RC_LEVEL", "3"); }
        let status = match game.status() {
            Ok(s) => s,
            Err(_) => return fail("rerac.exe is missing. Unzip the whole download again, then retry."),
        };
        match status.code() {
            Some(0) => 0,
            Some(3) | Some(4) => {
                let _ = std::fs::remove_file(data.join("extract-info.json"));
                fail("The game files are incomplete or out of date. Start the game again to copy them from your \
                      disc image once more.")
            }
            _ => fail("The game stopped unexpectedly.\n\nMake sure your headset is connected and SteamVR is running, \
                       then try again. The details are in the black window."),
        }
    }

    fn fail(text: &str) -> i32 {
        eprintln!("{text}");
        win::message(text, win::MB_OK | win::MB_ICONERROR);
        1
    }

    /// The two Win32 dialogs used here (user32 / comdlg32), declared by hand so no dependency is added.
    mod win {
        use std::ffi::c_void;
        use std::os::windows::ffi::{OsStrExt, OsStringExt};
        use std::path::PathBuf;

        pub const MB_OK: u32 = 0x0;
        pub const MB_OKCANCEL: u32 = 0x1;
        pub const MB_ICONERROR: u32 = 0x10;
        pub const MB_ICONINFORMATION: u32 = 0x40;
        const MB_SETFOREGROUND: u32 = 0x1_0000;
        const IDOK: i32 = 1;
        const OFN_NOCHANGEDIR: u32 = 0x8;
        const OFN_PATHMUSTEXIST: u32 = 0x800;
        const OFN_FILEMUSTEXIST: u32 = 0x1000;

        #[repr(C)]
        struct OpenFileName {
            struct_size: u32,
            owner: *mut c_void,
            instance: *mut c_void,
            filter: *const u16,
            custom_filter: *mut u16,
            max_custom_filter: u32,
            filter_index: u32,
            file: *mut u16,
            max_file: u32,
            file_title: *mut u16,
            max_file_title: u32,
            initial_dir: *const u16,
            title: *const u16,
            flags: u32,
            file_offset: u16,
            file_extension: u16,
            default_ext: *const u16,
            custom_data: isize,
            hook: *mut c_void,
            template_name: *const u16,
            reserved_ptr: *mut c_void,
            reserved: u32,
            flags_ex: u32,
        }

        #[link(name = "user32")]
        unsafe extern "system" { fn MessageBoxW(owner: *mut c_void, text: *const u16, caption: *const u16, kind: u32) -> i32; }
        #[link(name = "comdlg32")]
        unsafe extern "system" { fn GetOpenFileNameW(ofn: *mut OpenFileName) -> i32; }

        fn wide(s: &str) -> Vec<u16> { std::ffi::OsStr::new(s).encode_wide().chain(Some(0)).collect() }

        /// Shows a message box; true when the player pressed OK.
        pub fn message(text: &str, kind: u32) -> bool {
            let (text, caption) = (wide(text), wide(super::TITLE));
            unsafe { MessageBoxW(std::ptr::null_mut(), text.as_ptr(), caption.as_ptr(), kind | MB_SETFOREGROUND) == IDOK }
        }

        /// The file dialog for the disc image; None when cancelled.
        pub fn pick_iso() -> Option<PathBuf> {
            let filter: Vec<u16> = "Disc images (*.iso)\0*.iso\0All files\0*.*\0\0".encode_utf16().collect();
            let title = wide("Choose your Ratchet & Clank disc image");
            let mut file = vec![0u16; 4096];
            let mut ofn = OpenFileName {
                struct_size: std::mem::size_of::<OpenFileName>() as u32,
                owner: std::ptr::null_mut(),
                instance: std::ptr::null_mut(),
                filter: filter.as_ptr(),
                custom_filter: std::ptr::null_mut(),
                max_custom_filter: 0,
                filter_index: 1,
                file: file.as_mut_ptr(),
                max_file: file.len() as u32,
                file_title: std::ptr::null_mut(),
                max_file_title: 0,
                initial_dir: std::ptr::null(),
                title: title.as_ptr(),
                flags: OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR,
                file_offset: 0,
                file_extension: 0,
                default_ext: std::ptr::null(),
                custom_data: 0,
                hook: std::ptr::null_mut(),
                template_name: std::ptr::null(),
                reserved_ptr: std::ptr::null_mut(),
                reserved: 0,
                flags_ex: 0,
            };
            if unsafe { GetOpenFileNameW(&mut ofn) } == 0 { return None; }
            let len = file.iter().position(|&c| c == 0).unwrap_or(file.len());
            Some(PathBuf::from(std::ffi::OsString::from_wide(&file[..len])))
        }
    }
}
