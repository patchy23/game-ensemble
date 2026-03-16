use midly::{Smf, TrackEventKind, MetaMessage, Timing};
use std::fs;

// 1. 将原先的结构体升级，使用枚举来区分不同事件
#[derive(Debug, Clone)]
pub enum EngineEventType {
    NoteOn { note: u8, velocity: u8 },
    NoteOff { note: u8 },
    ProgramChange { program: u8 }, // 乐器切换事件
}

#[derive(Debug, Clone)]
pub struct EngineEvent {
    pub absolute_time_ms: u64,
    pub channel: u8,
    pub event_type: EngineEventType,
}

pub struct MidiParser;

impl MidiParser {
    pub fn parse_file(file_path: &str) -> Result<Vec<EngineEvent>, String> {
        let data = fs::read(file_path).map_err(|e| format!("无法读取文件: {}", e))?;
        let smf = Smf::parse(&data).map_err(|e| format!("MIDI 解析失败: {}", e))?;

        let ticks_per_beat = match smf.header.timing {
            Timing::Metrical(ticks) => ticks.as_int() as f64,
            Timing::Timecode(_, _) => return Err("暂不支持 SMPTE".to_string()),
        };

        let mut all_events: Vec<EngineEvent> = Vec::new();
        let mut current_microseconds_per_beat: f64 = 500_000.0;

        for track in smf.tracks.iter() {
            //let mut absolute_ticks: u64 = 0;
            let mut absolute_ms: f64 = 0.0;

            for event in track.iter() {
                //absolute_ticks += event.delta.as_int() as u64;
                let delta_ms = (event.delta.as_int() as f64 / ticks_per_beat)
                    * (current_microseconds_per_beat / 1000.0);
                absolute_ms += delta_ms;

                match &event.kind {
                    TrackEventKind::Meta(MetaMessage::Tempo(tempo)) => {
                        current_microseconds_per_beat = tempo.as_int() as f64;
                    }
                    // 捕获乐器切换！(例如把 0号通道切成小提琴)
                    TrackEventKind::Midi { channel, message: midly::MidiMessage::ProgramChange { program } } => {
                        all_events.push(EngineEvent {
                            absolute_time_ms: absolute_ms.round() as u64,
                            channel: channel.as_int(),
                            event_type: EngineEventType::ProgramChange { program: program.as_int() },
                        });
                    }
                    TrackEventKind::Midi { channel, message: midly::MidiMessage::NoteOn { key, vel } } => {
                        let velocity = vel.as_int();
                        let event_type = if velocity > 0 {
                            EngineEventType::NoteOn { note: key.as_int(), velocity }
                        } else {
                            EngineEventType::NoteOff { note: key.as_int() } // 力度为0视为松开
                        };
                        all_events.push(EngineEvent {
                            absolute_time_ms: absolute_ms.round() as u64,
                            channel: channel.as_int(),
                            event_type,
                        });
                    }
                    TrackEventKind::Midi { channel, message: midly::MidiMessage::NoteOff { key, .. } } => {
                        all_events.push(EngineEvent {
                            absolute_time_ms: absolute_ms.round() as u64,
                            channel: channel.as_int(),
                            event_type: EngineEventType::NoteOff { note: key.as_int() },
                        });
                    }
                    _ => {}
                }
            }
        }
        all_events.sort_by_key(|e| e.absolute_time_ms);
        Ok(all_events)
    }
}