//! @file main.rs
//! @description Tauri 后端主入口，负责状态管理、前端通信、以及音高降维算法的实现

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use game_ensemble_lib::core::midi_parser::{EngineEventType, MidiParser};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, mpsc::Sender, Arc, Mutex};
use std::thread;
use tauri::{AppHandle, Emitter, State};
use std::fs;
use game_ensemble_lib::core::audio::AudioSimulator;
use game_ensemble_lib::core::engine::PlayerEngine;
// ==========================================
// 数据结构定义
// ==========================================

pub enum AudioCommand {
    NoteOn { channel: u8, note: u8, velocity: u8 },
    NoteOff { channel: u8, note: u8 },
    ProgramChange { channel: u8, program: u8 },
    Panic, // 全通道紧急消音
}

struct AppState {
    audio_tx: Sender<AudioCommand>,
    playback_strategy: Arc<Mutex<String>>,
    global_transpose: Arc<Mutex<i8>>,
    muted_channels: Arc<Mutex<HashSet<u8>>>,
    play_flag: Arc<AtomicBool>,
}

#[derive(serde::Serialize, Clone)]
struct NotePayload {
    note: u8,
    is_active: bool,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackInfo {
    pub channel: u8,
    pub instrument_name: String,
    pub is_muted: bool,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub tracks: Vec<TrackInfo>,
    pub suggested_transpose: i8,
    pub total_duration_ms: u64,
}

// ==========================================
// 核心算法：降维与映射
// ==========================================

/// 将全音域 MIDI 音符转换为适合 21 键游戏乐器的音高
fn apply_strategy(raw_note: u8, strategy: &str, manual_transpose: i8) -> Option<u8> {
    let valid_notes = [
        48, 50, 52, 53, 55, 57, 59,
        60, 62, 64, 65, 67, 69, 71,
        72, 74, 76, 77, 79, 81, 83
    ];

    let transposed = (raw_note as i16 + manual_transpose as i16).clamp(0, 127) as u8;

    match strategy {
        "Original" => Some(transposed),
        "Drop" => if valid_notes.contains(&transposed) { Some(transposed) } else { None },
        "PureFold" => {
            let mut f = transposed;
            while f > 83 { f -= 12; }
            while f < 48 { f += 12; }
            // 严格过滤：只放行白键，黑键直接丢弃不发声
            if valid_notes.contains(&f) { Some(f) } else { None }
        },
        "AutoMelody" | "Consonance" => {
            let mut f = transposed;
            while f > 83 { f -= 12; }
            while f < 48 { f += 12; }

            // 智能白键吸附 (牺牲音程准确度换取不断音)
            let diatonic_map = [0, 2, 2, 4, 4, 5, 7, 7, 9, 9, 11, 11];
            let octave = f / 12;
            let pitch_class = (f % 12) as usize;
            let final_note = octave * 12 + diatonic_map[pitch_class] as u8;

            if final_note > 83 { Some(final_note - 12) } else { Some(final_note) }
        },
        _ => Some(transposed)
    }
}

fn get_gm_instrument_name(program: u8) -> &'static str {
    match program {
        0..=7 => "钢琴", 8..=15 => "色彩打击乐", 16..=23 => "风琴", 24..=31 => "吉他",
        32..=39 => "贝斯", 40..=47 => "弦乐", 48..=55 => "合唱", 56..=63 => "铜管",
        64..=71 => "簧管", 72..=79 => "笛", 80..=87 => "合成主音", 88..=95 => "合成柔音",
        _ => "未知合成器",
    }
}

// ==========================================
// Tauri Commands
// ==========================================

/// 获取本地曲库列表
#[tauri::command]
fn get_local_midis() -> Result<Vec<String>, String> {
    let local_dir = std::env::current_dir().unwrap().join("local_midis");
    fs::create_dir_all(&local_dir).unwrap_or_default();

    let mut files = Vec::new();
    if let Ok(entries) = fs::read_dir(local_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|s| s == "mid" || s == "midi") {
                files.push(path.file_name().unwrap().to_string_lossy().into_owned());
            }
        }
    }
    Ok(files)
}

/// 导入 MIDI 文件 (附带同名文件防冲突处理)
#[tauri::command]
fn import_midi(source_path: String) -> Result<String, String> {
    // 1. 安全检测
    let _ = MidiParser::parse_file(&source_path).map_err(|e| format!("解析失败: {}", e))?;

    let local_dir = std::env::current_dir().unwrap().join("local_midis");
    fs::create_dir_all(&local_dir).unwrap_or_default();

    let original_name = std::path::Path::new(&source_path).file_name().unwrap().to_string_lossy();
    let mut final_name = original_name.to_string();
    let mut dest_path = local_dir.join(&final_name);

    // 2. 防冲突：如果已存在，自动添加 _1, _2 后缀
    let mut counter = 1;
    while dest_path.exists() {
        let stem = std::path::Path::new(&source_path).file_stem().unwrap().to_string_lossy();
        let ext = std::path::Path::new(&source_path).extension().unwrap().to_string_lossy();
        final_name = format!("{}_{}.{}", stem, counter, ext);
        dest_path = local_dir.join(&final_name);
        counter += 1;
    }

    // 3. 复制文件
    fs::copy(&source_path, &dest_path).map_err(|e| format!("保存失败，文件可能被占用: {}", e))?;
    Ok(final_name)
}

/// 扫描曲目并计算全自动移调
#[tauri::command]
fn scan_midi_tracks(file_name: String, state: State<'_, AppState>) -> Result<ScanResult, String> {
    let file_path = std::env::current_dir().unwrap().join("local_midis").join(&file_name);
    let events = MidiParser::parse_file(&file_path.to_string_lossy()).map_err(|e| e.to_string())?;

    let total_duration_ms = events.last().map(|e| e.absolute_time_ms).unwrap_or(0);
    let mut channel_program: HashMap<u8, u8> = HashMap::new();
    let mut active_channels: HashSet<u8> = HashSet::new();
    let mut pitch_counts = [0u32; 12];

    for event in &events {
        match event.event_type {
            EngineEventType::ProgramChange { program } => { channel_program.insert(event.channel, program); }
            EngineEventType::NoteOn { note, .. } => {
                active_channels.insert(event.channel);
                if event.channel != 9 { pitch_counts[(note % 12) as usize] += 1; }
            }
            _ => {}
        }
    }

    let mut muted = state.muted_channels.lock().unwrap();
    muted.clear();
    muted.insert(9); // 默认屏蔽架子鼓

    let mut tracks = Vec::new();
    let mut sorted_channels: Vec<_> = active_channels.into_iter().collect();
    sorted_channels.sort();

    for channel in sorted_channels {
        if channel == 9 {
            tracks.push(TrackInfo { channel, instrument_name: "🥁 打击乐 (默认静音)".into(), is_muted: true });
        } else {
            let prog = *channel_program.get(&channel).unwrap_or(&0);
            tracks.push(TrackInfo { channel, instrument_name: format!("🎹 {}", get_gm_instrument_name(prog)), is_muted: false });
        }
    }

    // 统计算法：寻找最佳移调
    let white_keys = [0, 2, 4, 5, 7, 9, 11];
    let mut best_offset = 0i8;
    let mut max_white_notes = 0u32;

    for offset in -5..=6 {
        let mut count = 0;
        for i in 0..12 {
            let t = (i as i32 + offset as i32).rem_euclid(12) as usize;
            if white_keys.contains(&t) { count += pitch_counts[i]; }
        }
        if count > max_white_notes { max_white_notes = count; best_offset = offset; }
    }

    Ok(ScanResult { tracks, suggested_transpose: best_offset, total_duration_ms })
}

/// 核心启动器：解析并开始播放
#[tauri::command]
fn start_auto_play(file_name: String, start_time_ms: u64, app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let file_path = std::env::current_dir().unwrap().join("local_midis").join(&file_name);

    let tx = state.audio_tx.clone();
    let strategy_arc = state.playback_strategy.clone();
    let transpose_arc = state.global_transpose.clone();
    let muted_arc = state.muted_channels.clone();
    let play_flag = state.play_flag.clone();

    play_flag.store(true, Ordering::Relaxed);

    thread::spawn(move || {
        let events = match MidiParser::parse_file(&file_path.to_string_lossy()) {
            Ok(e) => e,
            Err(_) => { let _ = app.emit("playback_stopped", ()); return; }
        };

        let app_clone = app.clone();

        // 用于 Consonance 和声净化的追踪表
        let mut active_notes_map: HashMap<(u8, u8), u8> = HashMap::new();

        PlayerEngine::play_blocking(events, play_flag, start_time_ms, move |event, _time| {
            let current_strategy = strategy_arc.lock().unwrap().clone();
            let current_transpose = *transpose_arc.lock().unwrap();
            let is_muted = muted_arc.lock().unwrap().contains(&event.channel);
            let is_pruning = current_strategy == "Consonance";

            match event.event_type {
                EngineEventType::NoteOn { note, velocity } => {
                    if !is_muted {
                        if let Some(final_note) = apply_strategy(note, &current_strategy, current_transpose) {
                            let mut should_play = true;

                            if is_pruning {
                                for &active_final in active_notes_map.values() {
                                    if final_note.abs_diff(active_final) <= 2 {
                                        should_play = false; break;
                                    }
                                }
                            }

                            if should_play {
                                active_notes_map.insert((note, event.channel), final_note);
                                let _ = tx.send(AudioCommand::NoteOn { channel: event.channel, note: final_note, velocity });
                                let _ = app_clone.emit("note_state_change", NotePayload { note: final_note, is_active: true });
                            }
                        }
                    }
                }
                EngineEventType::NoteOff { note } => {
                    // 仅停止当初被放行的音符
                    if let Some(final_note) = active_notes_map.remove(&(note, event.channel)) {
                        let _ = tx.send(AudioCommand::NoteOff { channel: event.channel, note: final_note });
                        let _ = app_clone.emit("note_state_change", NotePayload { note: final_note, is_active: false });
                    }
                }
                EngineEventType::ProgramChange { program } => {
                    let _ = tx.send(AudioCommand::ProgramChange { channel: event.channel, program });
                }
            }
        });

        let _ = app.emit("playback_stopped", ());
    });
    Ok(())
}

// 控制接口
#[tauri::command]
fn stop_playback(state: State<'_, AppState>) { state.play_flag.store(false, Ordering::Relaxed); let _ = state.audio_tx.send(AudioCommand::Panic); }
#[tauri::command]
fn toggle_mute(channel: u8, state: State<'_, AppState>) { let mut muted = state.muted_channels.lock().unwrap(); if muted.contains(&channel) { muted.remove(&channel); } else { muted.insert(channel); let _ = state.audio_tx.send(AudioCommand::Panic); } }
#[tauri::command]
fn set_playback_strategy(strategy: String, state: State<'_, AppState>) { *state.playback_strategy.lock().unwrap() = strategy; }
#[tauri::command]
fn set_global_transpose(offset: i8, state: State<'_, AppState>) { *state.global_transpose.lock().unwrap() = offset; }
#[tauri::command]
fn manual_note_on(note: u8, state: State<'_, AppState>) { let _ = state.audio_tx.send(AudioCommand::NoteOn { channel: 0, note, velocity: 100 }); }
#[tauri::command]
fn manual_note_off(note: u8, state: State<'_, AppState>) { let _ = state.audio_tx.send(AudioCommand::NoteOff { channel: 0, note }); }

#[tauri::command]
fn delete_midi(file_name: String) -> Result<(), String> {
    let file_path = std::env::current_dir().unwrap().join("local_midis").join(&file_name);

    if file_path.exists() {
        fs::remove_file(file_path).map_err(|e| format!("物理删除失败: {}", e))?;
    } else {
        return Err("找不到该文件，可能已被手动删除".to_string());
    }

    Ok(())
}

fn main() {
    let (tx, rx) = mpsc::channel::<AudioCommand>();

    thread::spawn(move || {
        let sf2_path = "test_assets/TimGM6mb.sf2";
        if let Ok(mut audio_sim) = AudioSimulator::new(sf2_path) {
            audio_sim.send_program_change(0, 4);
            for cmd in rx {
                match cmd {
                    AudioCommand::NoteOn { channel, note, velocity } => audio_sim.send_note_on(channel, note, velocity),
                    AudioCommand::NoteOff { channel, note } => audio_sim.send_note_off(channel, note),
                    AudioCommand::ProgramChange { channel, program } => audio_sim.send_program_change(channel, program),
                    AudioCommand::Panic => audio_sim.send_panic(),
                }
            }
        }
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState {
            audio_tx: tx,
            playback_strategy: Arc::new(Mutex::new("PureFold".to_string())),
            global_transpose: Arc::new(Mutex::new(0)),
            muted_channels: Arc::new(Mutex::new(HashSet::new())),
            play_flag: Arc::new(AtomicBool::new(false)),
        })
        .invoke_handler(tauri::generate_handler![
            get_local_midis, import_midi, delete_midi,scan_midi_tracks,
            start_auto_play, stop_playback, toggle_mute,
            set_playback_strategy, set_global_transpose,
            manual_note_on, manual_note_off
        ])
        .run(tauri::generate_context!())
        .expect("Tauri 运行失败");
}