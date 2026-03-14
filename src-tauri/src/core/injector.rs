/// 抽象的键盘注入器特征 (Trait)
/// 无论是真实的 WinAPI 还是模拟器，都必须实现这个接口
pub trait KeyboardInjector {
    fn send_key_down(&self, key: &str);
    fn send_key_up(&self, key: &str);
}

/// 终端模拟器 (专为脱机调试设计)
pub struct ConsoleSimulator;

impl ConsoleSimulator {
    pub fn new() -> Self {
        ConsoleSimulator {}
    }
}

impl KeyboardInjector for ConsoleSimulator {
    fn send_key_down(&self, key: &str) {
        // 使用特殊的终端控制符和 Emoji 让视觉反馈更明显
        println!("[终端模拟器] 按下: [{}]", key);
    }

    fn send_key_up(&self, key: &str) {
        println!("[终端模拟器] 抬起: [{}]", key);
    }
}


//临时的键位映射器
pub fn map_midi_note_to_key(note: u8) -> Option<&'static str> {
    match note {
        60 => Some("Q"), // 中音 C
        62 => Some("W"), // 中音 D
        64 => Some("E"), // 中音 E
        65 => Some("R"), // 中音 F
        67 => Some("T"), // 中音 G
        69 => Some("Y"), // 中音 A
        71 => Some("U"), // 中音 B
        _ => None,       // 超出映射范围的音符，暂时丢弃
    }
}