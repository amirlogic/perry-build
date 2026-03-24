//! Audio playback for Android using MediaPlayer via JNI.
//!
//! Architecture: Each player is a Java `android.media.MediaPlayer` instance created
//! via JNI, backed by an asset file descriptor from the app's bundled assets.
//! Player handles are stored in a global `Vec<Option<PlayerEntry>>` behind a `Mutex`
//! so that the fade background thread can access them safely.

use jni::objects::{JObject, JValue};
use std::sync::{Arc, Mutex, OnceLock};

use crate::jni_bridge;

// =============================================================================
// Data structures
// =============================================================================

struct PlayerEntry {
    /// Global reference to the Java MediaPlayer object.
    player: jni::objects::GlobalRef,
    /// Current volume (0.0 to 1.0).
    volume: f64,
    /// Whether the player is currently playing.
    is_playing: bool,
}

// =============================================================================
// Global state
// =============================================================================

static PLAYERS: OnceLock<Arc<Mutex<Vec<Option<PlayerEntry>>>>> = OnceLock::new();

fn players() -> &'static Arc<Mutex<Vec<Option<PlayerEntry>>>> {
    PLAYERS.get_or_init(|| Arc::new(Mutex::new(Vec::new())))
}

// =============================================================================
// Helper: extract string from StringHeader pointer
// =============================================================================

fn str_from_header(ptr: *const u8) -> &'static str {
    if ptr.is_null() {
        return "";
    }
    unsafe {
        let header = ptr as *const perry_runtime::string::StringHeader;
        let len = (*header).length as usize;
        let data = ptr.add(std::mem::size_of::<perry_runtime::string::StringHeader>());
        std::str::from_utf8_unchecked(std::slice::from_raw_parts(data, len))
    }
}

// =============================================================================
// Public API
// =============================================================================

/// Create a player for the given audio file. Returns a 1-based handle as f64,
/// or 0.0 on failure.
///
/// The file is loaded from the app's assets directory at `sounds/{filename}.m4a`.
pub fn player_create(filename_ptr: i64) -> f64 {
    let filename = str_from_header(filename_ptr as *const u8);
    if filename.is_empty() {
        crate::log_debug("[audio_playback] empty filename");
        return 0.0;
    }

    // Split filename into name and extension; default to .m4a
    let asset_path = if filename.contains('.') {
        format!("sounds/{}", filename)
    } else {
        format!("sounds/{}.m4a", filename)
    };

    let mut env = jni_bridge::get_env();
    let _ = env.push_local_frame(32);

    // Get the Activity (Context)
    let activity = crate::widgets::get_activity(&mut env);

    // Get AssetManager: context.getAssets()
    let assets = match env.call_method(&activity, "getAssets", "()Landroid/content/res/AssetManager;", &[]) {
        Ok(v) => match v.l() {
            Ok(obj) => obj,
            Err(_) => {
                crate::log_debug("[audio_playback] getAssets() did not return an object");
                unsafe { env.pop_local_frame(&JObject::null()); }
                return 0.0;
            }
        },
        Err(_) => {
            crate::log_debug("[audio_playback] getAssets() failed");
            unsafe { env.pop_local_frame(&JObject::null()); }
            return 0.0;
        }
    };

    // Open the asset file descriptor: assets.openFd(assetPath)
    let j_path = match env.new_string(&asset_path) {
        Ok(s) => s,
        Err(_) => {
            crate::log_debug("[audio_playback] failed to create Java string for path");
            unsafe { env.pop_local_frame(&JObject::null()); }
            return 0.0;
        }
    };

    let afd = match env.call_method(
        &assets,
        "openFd",
        "(Ljava/lang/String;)Landroid/content/res/AssetFileDescriptor;",
        &[JValue::Object(&j_path.into())],
    ) {
        Ok(v) => match v.l() {
            Ok(obj) if !obj.is_null() => obj,
            _ => {
                crate::log_debug(&format!("[audio_playback] openFd failed for: {}", asset_path));
                unsafe { env.pop_local_frame(&JObject::null()); }
                return 0.0;
            }
        },
        Err(_) => {
            crate::log_debug(&format!("[audio_playback] openFd exception for: {}", asset_path));
            let _ = env.exception_clear();
            unsafe { env.pop_local_frame(&JObject::null()); }
            return 0.0;
        }
    };

    // Create MediaPlayer: new MediaPlayer()
    let player = match env.new_object("android/media/MediaPlayer", "()V", &[]) {
        Ok(p) => p,
        Err(_) => {
            crate::log_debug("[audio_playback] failed to create MediaPlayer");
            unsafe { env.pop_local_frame(&JObject::null()); }
            return 0.0;
        }
    };

    // Get FileDescriptor, start offset, and length from AssetFileDescriptor
    let fd = match env.call_method(&afd, "getFileDescriptor", "()Ljava/io/FileDescriptor;", &[]) {
        Ok(v) => match v.l() {
            Ok(obj) => obj,
            Err(_) => {
                crate::log_debug("[audio_playback] getFileDescriptor() failed");
                unsafe { env.pop_local_frame(&JObject::null()); }
                return 0.0;
            }
        },
        Err(_) => {
            crate::log_debug("[audio_playback] getFileDescriptor() exception");
            unsafe { env.pop_local_frame(&JObject::null()); }
            return 0.0;
        }
    };

    let start_offset = env
        .call_method(&afd, "getStartOffset", "()J", &[])
        .map(|v| v.j().unwrap_or(0))
        .unwrap_or(0);

    let length = env
        .call_method(&afd, "getLength", "()J", &[])
        .map(|v| v.j().unwrap_or(0))
        .unwrap_or(0);

    // player.setDataSource(fd, offset, length)
    if env
        .call_method(
            &player,
            "setDataSource",
            "(Ljava/io/FileDescriptor;JJ)V",
            &[
                JValue::Object(&fd),
                JValue::Long(start_offset),
                JValue::Long(length),
            ],
        )
        .is_err()
    {
        crate::log_debug("[audio_playback] setDataSource failed");
        let _ = env.exception_clear();
        unsafe { env.pop_local_frame(&JObject::null()); }
        return 0.0;
    }

    // Close the AssetFileDescriptor
    let _ = env.call_method(&afd, "close", "()V", &[]);

    // player.prepare()
    if env.call_method(&player, "prepare", "()V", &[]).is_err() {
        crate::log_debug("[audio_playback] prepare() failed");
        let _ = env.exception_clear();
        unsafe { env.pop_local_frame(&JObject::null()); }
        return 0.0;
    }

    // player.setLooping(true)
    let _ = env.call_method(&player, "setLooping", "(Z)V", &[JValue::Bool(1)]);

    // Create a global reference to keep the MediaPlayer alive
    let global_ref = match env.new_global_ref(&player) {
        Ok(g) => g,
        Err(_) => {
            crate::log_debug("[audio_playback] failed to create global ref");
            unsafe { env.pop_local_frame(&JObject::null()); }
            return 0.0;
        }
    };

    unsafe { env.pop_local_frame(&JObject::null()); }

    // Store in the players vec
    let entry = PlayerEntry {
        player: global_ref,
        volume: 1.0,
        is_playing: false,
    };

    let handle = {
        let mut players = players().lock().unwrap();
        // Find an empty slot or push a new one
        for (i, slot) in players.iter_mut().enumerate() {
            if slot.is_none() {
                *slot = Some(entry);
                return (i + 1) as f64;
            }
        }
        players.push(Some(entry));
        players.len() as f64
    };

    crate::log_debug(&format!("[audio_playback] player created, handle={}", handle));
    handle
}

/// Start playback.
pub fn player_play(handle: f64) {
    let idx = handle as usize;
    if idx == 0 {
        return;
    }
    let idx = idx - 1;

    let mut players = players().lock().unwrap();
    if let Some(Some(entry)) = players.get_mut(idx) {
        let mut env = jni_bridge::get_env();
        let _ = env.push_local_frame(4);
        let _ = env.call_method(entry.player.as_obj(), "start", "()V", &[]);
        unsafe { env.pop_local_frame(&JObject::null()); }
        entry.is_playing = true;
        crate::log_debug(&format!("[audio_playback] player {} playing", handle));
    }
}

/// Stop playback and seek back to the beginning.
pub fn player_stop(handle: f64) {
    let idx = handle as usize;
    if idx == 0 {
        return;
    }
    let idx = idx - 1;

    let mut players = players().lock().unwrap();
    if let Some(Some(entry)) = players.get_mut(idx) {
        let mut env = jni_bridge::get_env();
        let _ = env.push_local_frame(4);
        let _ = env.call_method(entry.player.as_obj(), "pause", "()V", &[]);
        let _ = env.call_method(
            entry.player.as_obj(),
            "seekTo",
            "(I)V",
            &[JValue::Int(0)],
        );
        unsafe { env.pop_local_frame(&JObject::null()); }
        entry.is_playing = false;
        crate::log_debug(&format!("[audio_playback] player {} stopped", handle));
    }
}

/// Set volume immediately (0.0 to 1.0).
pub fn player_set_volume(handle: f64, volume: f64) {
    let idx = handle as usize;
    if idx == 0 {
        return;
    }
    let idx = idx - 1;

    let mut players = players().lock().unwrap();
    if let Some(Some(entry)) = players.get_mut(idx) {
        let vol = volume.max(0.0).min(1.0) as f32;
        let mut env = jni_bridge::get_env();
        let _ = env.push_local_frame(4);
        let _ = env.call_method(
            entry.player.as_obj(),
            "setVolume",
            "(FF)V",
            &[JValue::Float(vol), JValue::Float(vol)],
        );
        unsafe { env.pop_local_frame(&JObject::null()); }
        entry.volume = vol as f64;
    }
}

/// Fade volume to target over duration seconds using a background thread.
///
/// Android's MediaPlayer has no built-in fade API, so we spawn a thread that
/// steps the volume at ~30 Hz. The thread attaches to the JVM to make JNI calls.
pub fn player_fade_to(handle: f64, target: f64, duration: f64) {
    let idx = handle as usize;
    if idx == 0 {
        return;
    }
    let idx = idx - 1;

    let target = target.max(0.0).min(1.0);

    // If duration is zero or negative, set volume immediately.
    {
        let mut players = players().lock().unwrap();
        if let Some(Some(entry)) = players.get_mut(idx) {
            if duration <= 0.0 {
                let vol = target as f32;
                let mut env = jni_bridge::get_env();
                let _ = env.push_local_frame(4);
                let _ = env.call_method(
                    entry.player.as_obj(),
                    "setVolume",
                    "(FF)V",
                    &[JValue::Float(vol), JValue::Float(vol)],
                );
                unsafe { env.pop_local_frame(&JObject::null()); }
                entry.volume = target;
                if target == 0.0 {
                    let mut env = jni_bridge::get_env();
                    let _ = env.push_local_frame(4);
                    let _ = env.call_method(entry.player.as_obj(), "pause", "()V", &[]);
                    let _ = env.call_method(
                        entry.player.as_obj(),
                        "seekTo",
                        "(I)V",
                        &[JValue::Int(0)],
                    );
                    unsafe { env.pop_local_frame(&JObject::null()); }
                    entry.is_playing = false;
                }
                return;
            }
        } else {
            return;
        }
    }

    // Read the current volume under the lock
    let current_volume = {
        let players = players().lock().unwrap();
        match players.get(idx) {
            Some(Some(entry)) => entry.volume,
            _ => return,
        }
    };

    let vm = jni_bridge::get_vm().clone();
    let players_ref = Arc::clone(players());

    std::thread::spawn(move || {
        // Attach this thread to the JVM so we can make JNI calls
        let mut env = vm
            .attach_current_thread_permanently()
            .expect("Failed to attach fade thread to JVM");

        let steps = (duration * 30.0).max(1.0) as u32;
        let step_duration = std::time::Duration::from_millis(33);
        let volume_step = (target - current_volume) / steps as f64;
        let mut vol = current_volume;

        for i in 0..steps {
            vol += volume_step;
            let clamped = vol.max(0.0).min(1.0) as f32;

            let mut players = players_ref.lock().unwrap();
            if let Some(Some(entry)) = players.get_mut(idx) {
                let _ = env.push_local_frame(4);
                let _ = env.call_method(
                    entry.player.as_obj(),
                    "setVolume",
                    "(FF)V",
                    &[JValue::Float(clamped), JValue::Float(clamped)],
                );
                unsafe { env.pop_local_frame(&JObject::null()); }
                entry.volume = clamped as f64;

                // On the last step, snap to exact target
                if i == steps - 1 {
                    let final_vol = target as f32;
                    let _ = env.push_local_frame(4);
                    let _ = env.call_method(
                        entry.player.as_obj(),
                        "setVolume",
                        "(FF)V",
                        &[JValue::Float(final_vol), JValue::Float(final_vol)],
                    );
                    unsafe { env.pop_local_frame(&JObject::null()); }
                    entry.volume = target;

                    // If faded to silence, stop playback
                    if target == 0.0 {
                        let _ = env.push_local_frame(4);
                        let _ = env.call_method(entry.player.as_obj(), "pause", "()V", &[]);
                        let _ = env.call_method(
                            entry.player.as_obj(),
                            "seekTo",
                            "(I)V",
                            &[JValue::Int(0)],
                        );
                        unsafe { env.pop_local_frame(&JObject::null()); }
                        entry.is_playing = false;
                    }
                }
            } else {
                // Player was destroyed during fade; abort.
                break;
            }
            // Drop the lock before sleeping
            drop(players);
            std::thread::sleep(step_duration);
        }
    });
}

/// Returns 1.0 if playing, 0.0 if not playing, -1.0 for invalid handle.
pub fn player_is_playing(handle: f64) -> f64 {
    let idx = handle as usize;
    if idx == 0 {
        return -1.0;
    }
    let idx = idx - 1;

    let players = players().lock().unwrap();
    match players.get(idx) {
        Some(Some(entry)) => {
            if entry.is_playing {
                1.0
            } else {
                0.0
            }
        }
        _ => -1.0,
    }
}

/// Destroy a player, releasing the underlying MediaPlayer resources.
pub fn player_destroy(handle: f64) {
    let idx = handle as usize;
    if idx == 0 {
        return;
    }
    let idx = idx - 1;

    let mut players = players().lock().unwrap();
    if let Some(slot) = players.get_mut(idx) {
        if let Some(entry) = slot.take() {
            let mut env = jni_bridge::get_env();
            let _ = env.push_local_frame(4);
            // release() stops playback and frees native resources
            let _ = env.call_method(entry.player.as_obj(), "release", "()V", &[]);
            unsafe { env.pop_local_frame(&JObject::null()); }
            crate::log_debug(&format!("[audio_playback] player {} destroyed", handle));
            // GlobalRef is dropped here, releasing the Java-side reference
        }
    }
}

/// Set Now Playing info (lock screen metadata).
///
/// Stub/no-op on Android — implementing MediaSession is significantly more complex
/// and will be added in a future iteration.
pub fn player_set_now_playing(_title_ptr: i64) {
    // No-op: MediaSession integration is complex on Android.
}

/// Register a callback for audio interruption events.
///
/// Stub/no-op on Android — AudioFocus handling will be added in a future iteration.
pub fn player_set_on_interruption(_callback: f64) {
    // No-op: AudioFocus integration is complex on Android.
}
