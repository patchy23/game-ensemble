use rodio::{OutputStream, OutputStreamHandle, Source, Sink};
use rustysynth::{SoundFont, Synthesizer, SynthesizerSettings};
use std::fs::File;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// 自定义的音频流源，桥接 rustysynth 和 rodio
struct SynthSource {
    synth: Arc<Mutex<Synthesizer>>,
    buffer: Vec<f32>,
    index: usize,
}

impl SynthSource {
    fn new(synth: Arc<Mutex<Synthesizer>>) -> Self {
        Self { synth, buffer: Vec::with_capacity(1024), index: 0 }
    }
}

// 核心渲染循环：每次 rodio 饿了来要声音数据，我们就让 synth 渲染一批
impl Iterator for SynthSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.buffer.len() {
            let mut synth = self.synth.lock().unwrap();
            let mut left = vec![0.0; 512];
            let mut right = vec![0.0; 512];

            // 渲染 512 个采样点的立体声
            synth.render(&mut left, &mut right);

            self.buffer.clear();
            // 交错混合立体声 (L, R, L, R...)
            for (l, r) in left.into_iter().zip(right.into_iter()) {
                self.buffer.push(l);
                self.buffer.push(r);
            }
            self.index = 0;
        }

        let sample = self.buffer[self.index];
        self.index += 1;
        Some(sample)
    }
}

impl Source for SynthSource {
    fn current_frame_len(&self) -> Option<usize> { None }
    fn channels(&self) -> u16 { 2 } // 立体声
    fn sample_rate(&self) -> u32 { 44100 } // CD 级音质
    fn total_duration(&self) -> Option<Duration> { None }
}

pub struct AudioSimulator {
    _stream: OutputStream,
    _sink: Sink,
    synth: Arc<Mutex<Synthesizer>>,
}

impl AudioSimulator {
    pub fn new(soundfont_path: &str) -> Result<Self, String> {
        let (stream, stream_handle) = OutputStream::try_default()
            .map_err(|_| "找不到系统音频设备")?;

        // 1. 加载几个百兆的真实音源库
        let mut sf2_file = File::open(soundfont_path)
            .map_err(|_| format!("找不到音库文件: {}", soundfont_path))?;
        let sound_font = Arc::new(SoundFont::new(&mut sf2_file).unwrap());

        // 2. 配置合成器
        let settings = SynthesizerSettings::new(44100);
        let synth = Arc::new(Mutex::new(Synthesizer::new(&sound_font, &settings).unwrap()));

        // 3. 将合成器挂载到声卡
        let sink = Sink::try_new(&stream_handle).unwrap();
        let synth_source = SynthSource::new(synth.clone());
        sink.append(synth_source); // 声音流开始在后台默默流淌

        Ok(Self { _stream: stream, _sink: sink, synth })
    }

    // 暴露给引擎的控制接口
    pub fn send_note_on(&mut self, channel: u8, note: u8, velocity: u8) {
        self.synth.lock().unwrap().note_on(channel as i32, note as i32, velocity as i32);
    }

    pub fn send_note_off(&mut self, channel: u8, note: u8) {
        self.synth.lock().unwrap().note_off(channel as i32, note as i32);
    }

    pub fn send_program_change(&mut self, channel: u8, program: u8) {
        // rustysynth 没有直接的 program_change 方法，必须通过底层 MIDI 消息接口发送。
        // 0xC0 是 MIDI 协议中代表 "Program Change" (乐器切换) 的状态码。
        self.synth.lock().unwrap().process_midi_message(
            channel as i32,
            0xC0,           // Command: Program Change
            program as i32, // Data 1: 目标乐器编号 (0-127)
            0               // Data 2: 切换乐器不需要第二个参数，填 0 即可
        );
    }

    pub fn send_panic(&mut self) {
        if let Ok(mut synth) = self.synth.lock() {
            // 遍历 16 个通道，发送 0xB0 (控制变更) -> 123 (All Notes Off)
            for ch in 0..16 {
                synth.process_midi_message(ch as i32, 0xB0, 123, 0);
            }
        }
    }
}

