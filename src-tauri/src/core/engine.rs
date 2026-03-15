//! @file engine.rs
//! @description 核心播放引擎，负责 MIDI 指令的精确时间调度、进度跳转与多线程发声控制

use std::time::{Duration, Instant};
use std::thread;
use std::hint;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use crate::core::midi_parser::{EngineEvent, EngineEventType};

/// 高精度微秒级睡眠函数
/// 解决 Windows 默认 thread::sleep 精度不足导致播放卡顿的问题
pub fn spin_sleep_until(target: Instant) {
    loop {
        let now = Instant::now();
        if now >= target { break; }

        // 距离目标时间大于 2ms 时，让出 CPU 避免过度空转
        if target - now > Duration::from_millis(2) {
            thread::sleep(Duration::from_millis(1));
        } else {
            // 最后 2ms 采用自旋锁，保证零延迟
            hint::spin_loop();
        }
    }
}

pub struct PlayerEngine;

impl PlayerEngine {
    /// 阻塞式播放器主循环
    ///
    /// # 参数
    /// * `events` - 解析好的 MIDI 事件流
    /// * `cancel_flag` - 控制播放/停止的原子开关
    /// * `start_offset_ms` - 进度条跳转时间（毫秒），用于实现静默快进
    /// * `on_event_fired` - 当事件到达触发时间时的回调闭包
    pub fn play_blocking<F>(
        events: Vec<EngineEvent>,
        cancel_flag: Arc<AtomicBool>,
        start_offset_ms: u64,
        mut on_event_fired: F
    ) where
        F: FnMut(&EngineEvent, Instant),
    {
        let start_time_real = Instant::now();

        for event in events {
            // 每一帧检查是否被用户按下了停止/暂停键
            if !cancel_flag.load(Ordering::Relaxed) { break; }

            // --------------------------------------------------------
            // [核心算法] 静默快进 (Silent Fast-Forward)
            // --------------------------------------------------------
            if event.absolute_time_ms < start_offset_ms {
                // 如果是乐器切换指令，必须立即执行以保证快进后的音色正确
                if let EngineEventType::ProgramChange { .. } = event.event_type {
                    on_event_fired(&event, Instant::now());
                }
                // 跳过睡眠和发声
                continue;
            }

            // 计算相对等待时间
            let wait_ms = event.absolute_time_ms - start_offset_ms;
            let target_time = start_time_real + Duration::from_millis(wait_ms);

            // 阻塞直到该音符的物理触发时间
            spin_sleep_until(target_time);

            // 睡醒后再查一次旗帜，防止挂起音
            if !cancel_flag.load(Ordering::Relaxed) { break; }

            // 触发事件
            on_event_fired(&event, Instant::now());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spin_sleep_accuracy() {
        // 测试目标：验证 Spin-Sleep 是否能打破 Windows 默认的 15ms 误差。
        let wait_time = Duration::from_millis(43); // 挑一个不规则的毫秒数
        let start = Instant::now();
        let target = start + wait_time;

        spin_sleep_until(target);

        let elapsed = start.elapsed();
        let expected_secs = wait_time.as_secs_f64();
        let actual_secs = elapsed.as_secs_f64();

        let diff_ms = (actual_secs - expected_secs).abs() * 1000.0;

        // 断言：误差必须小于 1.5 毫秒（在绝大多数机器上，通常误差在 0.1ms 以内）
        println!("目标休眠: 43ms, 实际流逝: {:.3}ms, 误差: {:.3}ms", actual_secs * 1000.0, diff_ms);
        assert!(diff_ms < 1.5, "时钟调度器精度不达标，误差过大: {}ms", diff_ms);
    }
}

#[test]
fn test_engine_event_timing() {
    use crate::core::midi_parser::{EngineEvent, EngineEventType};

    // 使用新的 Enum 结构构造测试用例
    let mock_events = vec![
        EngineEvent {
            absolute_time_ms: 0,
            channel: 0,
            event_type: EngineEventType::NoteOn { note: 60, velocity: 100 }
        },
        EngineEvent {
            absolute_time_ms: 37,
            channel: 0,
            event_type: EngineEventType::NoteOff { note: 60 }
        },
        EngineEvent {
            absolute_time_ms: 105,
            channel: 0,
            event_type: EngineEventType::NoteOn { note: 62, velocity: 100 }
        },
    ];

    let start_time = Instant::now();
    let mut recorded_times = Vec::new();

    PlayerEngine::play_blocking(mock_events, Arc::new(AtomicBool::new(true)),0,|event, actual_fire_time| {
        let actual_ms = actual_fire_time.duration_since(start_time).as_millis();
        recorded_times.push((event.absolute_time_ms, actual_ms));
    });

    for (expected_ms, actual_ms) in recorded_times {
        let diff = (expected_ms as i128 - actual_ms as i128).abs();
        assert!(diff <= 2, "事件在错误的时间被触发！");
    }
}


///根据mid文件和音源文件，播放音乐
#[test]
fn test_real_midi_playback_integration() {
    use std::path::Path;
    use crate::core::midi_parser::{MidiParser, EngineEventType};
    use crate::core::audio::AudioSimulator;

    let is_covert = false;
    const TARGET_CHANNEL: u8 = 0; //要替换乐器的目标音轨号
    const CUSTOM_PROGRAM: u8 = 0; //自定义乐器编号

    let midi_path = "test_assets/dont-say-lazy(k-on).mid";
    //let sf2_path = "test_assets/TimGM6mb.sf2";
    let sf2_path = "test_assets/TimGM6mb.sf2";
    let mut custom_instrument_loaded = false;

    if !Path::new(midi_path).exists() || !Path::new(sf2_path).exists() {
        println!("文件未找到!");
        return;
    }

    let events = MidiParser::parse_file(midi_path).unwrap();

    let mut audio_sim = AudioSimulator::new(sf2_path).expect("音库加载失败");

    println!("开始...");

    PlayerEngine::play_blocking(events, Arc::new(AtomicBool::new(true)),0,|event, _time| {

        if event.channel == TARGET_CHANNEL && is_covert{
            if !custom_instrument_loaded {
                audio_sim.send_program_change(event.channel, CUSTOM_PROGRAM);
                println!("通道 {} 强制切换为自定义乐器: {}", TARGET_CHANNEL, CUSTOM_PROGRAM);
                custom_instrument_loaded = true;
            }
            match event.event_type {
                EngineEventType::NoteOn { note, velocity } => {
                    audio_sim.send_note_on(event.channel, note, velocity);
                }
                EngineEventType::NoteOff { note } => {
                    audio_sim.send_note_off(event.channel, note);
                }
                _ => {} // 忽略原生ProgramChange
            }
        }else {
            match event.event_type {
                EngineEventType::ProgramChange { program } => {
                    audio_sim.send_program_change(event.channel, program);
                    println!("🎸 通道 {} 切换乐器为: {}", event.channel, program);
                }
                EngineEventType::NoteOn { note, velocity } => {
                    audio_sim.send_note_on(event.channel, note, velocity);
                }
                EngineEventType::NoteOff { note } => {
                    audio_sim.send_note_off(event.channel, note);
                }
            }
        }

    });
}

///测试音源中各乐器的声音
#[test]
fn test_explore_instruments() {
    use crate::core::audio::AudioSimulator;
    use std::path::Path;
    use std::time::Duration;
    use std::thread;

    //let sf2_path = "test_assets/TimGM6mb.sf2";
    let sf2_path = "test_assets/TimGM6mb.sf2";
    if !Path::new(sf2_path).exists() {
        println!("找不到音库文件!!");
        return;
    }

    let mut audio_sim = AudioSimulator::new(sf2_path).expect("音库加载失败");

    let target_instrument: u8 = 27; // 默认 27：清音电吉他

    println!("正在加载并切换到乐器编号: {}", target_instrument);
    // 发送 0xC0 指令，将 0 号通道的乐器切成选择的乐器
    audio_sim.send_program_change(0, target_instrument);

    // 准备一个C大调组合，用来测试音色
    let notes = [60, 64, 67, 72]; // C4, E4, G4, C5

    println!("1. 播放琶音 (测试单音连贯性)...");
    for &note in &notes {
        audio_sim.send_note_on(0, note, 100);
        thread::sleep(Duration::from_millis(300));
        audio_sim.send_note_off(0, note);
    }

    thread::sleep(Duration::from_millis(300));

    println!("2. 播放和弦 (测试多音并发与共鸣)...");
    for &note in &notes {
        audio_sim.send_note_on(0, note, 100);
    }

    // 让和弦持续响 2.5 秒
    thread::sleep(Duration::from_millis(2500));

    // 释放按键
    for &note in &notes {
        audio_sim.send_note_off(0, note);
    }

    // 等待余音 (Release 阶段) 结束
    println!("余音...");
    thread::sleep(Duration::from_secs(2));
    println!("试听结束！");
}


fn get_gm_instrument_name(program: u8) -> &'static str {
    let gm_instruments = [
        // 0-7 钢琴族 (Piano)
        "大钢琴 (Acoustic Grand)", "亮音钢琴 (Bright Acoustic)", "电大钢琴 (Electric Grand)", "酒吧钢琴 (Honky-tonk)",
        "电钢琴 1 (Electric Piano 1)", "电钢琴 2 (Electric Piano 2)", "大键琴 (Harpsichord)", "古钢琴 (Clavinet)",
        // 8-15 色彩打击乐 (Chromatic Percussion)
        "钢片琴 (Celesta)", "钟琴 (Glockenspiel)", "八音盒 (Music Box)", "颤音琴 (Vibraphone)",
        "马林巴琴 (Marimba)", "木琴 (Xylophone)", "管钟 (Tubular Bells)", "扬琴 (Dulcimer)",
        // 16-23 风琴族 (Organ)
        "击弦风琴 (Drawbar Organ)", "敲击风琴 (Percussive Organ)", "摇滚风琴 (Rock Organ)", "教堂管风琴 (Church Organ)",
        "簧风琴 (Reed Organ)", "手风琴 (Accordion)", "口琴 (Harmonica)", "探戈手风琴 (Tango Accordion)",
        // 24-31 吉他族 (Guitar)
        "尼龙弦吉他 (Acoustic Guitar - Nylon)", "钢弦吉他 (Acoustic Guitar - Steel)", "爵士电吉他 (Electric Guitar - Jazz)", "清音电吉他 (Electric Guitar - Clean)",
        "闷音电吉他 (Electric Guitar - Muted)", "过载电吉他 (Overdriven Guitar)", "失真电吉他 (Distortion Guitar)", "吉他泛音 (Guitar harmonics)",
        // 32-39 贝斯族 (Bass)
        "声学贝斯 (Acoustic Bass)", "指拨电贝斯 (Electric Bass - finger)", "拨片电贝斯 (Electric Bass - pick)", "无品贝斯 (Fretless Bass)",
        "击弦贝斯 1 (Slap Bass 1)", "击弦贝斯 2 (Slap Bass 2)", "合成贝斯 1 (Synth Bass 1)", "合成贝斯 2 (Synth Bass 2)",
        // 40-47 弦乐族 (Strings)
        "小提琴 (Violin)", "中提琴 (Viola)", "大提琴 (Cello)", "低音提琴 (Contrabass)",
        "颤音弦乐 (Tremolo Strings)", "弹拨弦乐 (Pizzicato Strings)", "竖琴 (Orchestral Harp)", "定音鼓 (Timpani)",
        // 48-55 合奏族 (Ensemble)
        "弦乐群 1 (String Ensemble 1)", "弦乐群 2 (String Ensemble 2)", "合成弦乐 1 (SynthStrings 1)", "合成弦乐 2 (SynthStrings 2)",
        "合唱“啊”音 (Choir Aahs)", "人声“嘟”音 (Voice Oohs)", "合成人声 (Synth Voice)", "管弦乐敲击 (Orchestral Hit)",
        // 56-63 铜管族 (Brass)
        "小号 (Trumpet)", "长号 (Trombone)", "大号 (Tuba)", "弱音小号 (Muted Trumpet)",
        "圆号 (French Horn)", "铜管群 (Brass Section)", "合成铜管 1 (SynthBrass 1)", "合成铜管 2 (SynthBrass 2)",
        // 64-71 簧管族 (Reed)
        "高音萨克斯 (Soprano Sax)", "中音萨克斯 (Alto Sax)", "次中音萨克斯 (Tenor Sax)", "上低音萨克斯 (Baritone Sax)",
        "双簧管 (Oboe)", "英国管 (English Horn)", "巴松管 (Bassoon)", "单簧管 (Clarinet)",
        // 72-79 笛管族 (Pipe)
        "短笛 (Piccolo)", "长笛 (Flute)", "竖笛 (Recorder)", "排箫 (Pan Flute)",
        "吹瓶声 (Blown Bottle)", "尺八 (Shakuhachi)", "哨子 (Whistle)", "陶笛 (Ocarina)",
        // 80-87 合成主音 (Synth Lead)
        "方波 (Lead 1 - square)", "锯齿波 (Lead 2 - sawtooth)", "汽笛风琴 (Lead 3 - calliope)", "合成吹管 (Lead 4 - chiff)",
        "失真电吉他 (Lead 5 - charang)", "人声键盘 (Lead 6 - voice)", "第五度 (Lead 7 - fifths)", "贝斯加主音 (Lead 8 - bass + lead)",
        // 88-95 合成音色 (Synth Pad)
        "新世纪 (Pad 1 - new age)", "温暖 (Pad 2 - warm)", "多重合唱 (Pad 3 - polysynth)", "人声合唱 (Pad 4 - choir)",
        "弓弦 (Pad 5 - bowed)", "金属 (Pad 6 - metallic)", "光环 (Pad 7 - halo)", "宽阔 (Pad 8 - sweep)",
        // 96-103 合成特效 (Synth Effects)
        "雨滴 (FX 1 - rain)", "音轨 (FX 2 - soundtrack)", "水晶 (FX 3 - crystal)", "大气 (FX 4 - atmosphere)",
        "明亮 (FX 5 - brightness)", "哥布林 (FX 6 - goblins)", "回声 (FX 7 - echoes)", "科幻 (FX 8 - sci-fi)",
        // 104-111 民族乐器 (Ethnic)
        "西塔尔琴 (Sitar)", "班卓琴 (Banjo)", "三味线 (Shamisen)", "十三弦古筝 (Koto)",
        "卡林巴琴 (Kalimba)", "风笛 (Bag pipe)", "提琴 (Fiddle)", "唢呐 (Shanai)",
        // 112-119 打击乐 (Percussive)
        "叮当铃 (Tinkle Bell)", "阿哥哥鼓 (Agogo)", "钢鼓 (Steel Drums)", "木鱼 (Woodblock)",
        "太鼓 (Taiko Drum)", "通通鼓 (Melodic Tom)", "合成鼓 (Synth Drum)", "反镲 (Reverse Cymbal)",
        // 120-127 声音特效 (Sound Effects)
        "吉他品格杂音 (Guitar Fret Noise)", "呼吸声 (Breath Noise)", "海浪声 (Seashore)", "鸟鸣声 (Bird Tweet)",
        "电话铃声 (Telephone Ring)", "直升机声 (Helicopter)", "鼓掌声 (Applause)", "枪声 (Gunshot)",
    ];

    if program < 128 {
        gm_instruments[program as usize]
    } else {
        "未知乐器 (Unknown)"
    }
}

///从mid中扒出乐器
#[test]
fn test_scan_midi_instruments() {
    use crate::core::midi_parser::{MidiParser, EngineEventType};
    use std::collections::{HashMap, HashSet};
    use std::path::Path;

    let midi_path = "test_assets/dont-say-lazy(k-on).mid";
    if !Path::new(midi_path).exists() {
        println!("找不到测试文件，跳过乐器扫描。");
        return;
    }

    let events = MidiParser::parse_file(midi_path).unwrap();

    let mut channel_current_program: HashMap<u8, u8> = HashMap::new();

    // 记录某个通道是什么乐器 (通道编号 -> 乐器编号)
    let mut channel_current_program: HashMap<u8, u8> = HashMap::new();

    // 记录整首曲子中【实际发声过】的乐器清单 (通道编号, 乐器名称)
    let mut active_instruments: HashSet<(u8,u8, String)> = HashSet::new();

    for event in events {
        match event.event_type {
            EngineEventType::ProgramChange { program } => {
                // 换了乐器，更新记录
                channel_current_program.insert(event.channel, program);
            }
            EngineEventType::NoteOn { .. } => {
                // 发出了声音
                if event.channel == 9 {
                    // 通道 9 在 MIDI 协议中被锁死为打击乐组，不受 Program Change 影响
                    active_instruments.insert((9,99, "架子鼓 / 标准打击乐组 (Standard Drum Kit)".to_string()));
                } else {
                    // 查一下当前是什么乐器？如果没有收到过换乐器指令，默认是 0号大钢琴
                    let program = *channel_current_program.get(&event.channel).unwrap_or(&0);
                    let instrument_name = get_gm_instrument_name(program);
                    active_instruments.insert((event.channel, program,format!("{}", instrument_name)));
                }
            }
            _ => {} // 忽略抬起指令等
        }
    }

    // --- 扫描报告 ---
    println!("\n扫描完毕！《{}》中包含以下音轨：", midi_path);
    println!("--------------------------------------------------");

    // 按通道号升序排序
    let mut sorted_instruments: Vec<_> = active_instruments.into_iter().collect();
    sorted_instruments.sort_by_key(|k| k.0);

    if sorted_instruments.is_empty() {
        println!("这是一首空曲子，没有任何声音！");
    } else {
        for (channel, program,name) in sorted_instruments {
            println!("  通道 {:02} | 乐器编号 {:02} : {}", channel, program, name);
        }
    }
}