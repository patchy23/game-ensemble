use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 顶层 Profile 结构，严格对应我们推演的 JSON 格式
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Profile {
    pub profile_meta: ProfileMeta,
    pub engine_config: EngineConfig,
    pub keymap_dictionary: HashMap<String, KeymapEntry>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProfileMeta {
    pub id: String,
    pub name: String,
    pub author: Option<String>,
    pub target_process: TargetProcess,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TargetProcess {
    pub strategy: MatchStrategy,
    pub value: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum MatchStrategy {
    Exact,
    Contains,
    Regex,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct EngineConfig {
    pub injection_mode: InjectionMode,
    pub fallback_to_sendinput: bool,
    pub humanization: HumanizationConfig,
    pub chord_mode: ChordMode,
    pub out_of_bounds_strategy: OutOfBoundsStrategy,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum InjectionMode {
    PostMessage,
    SendInput,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HumanizationConfig {
    pub enabled: bool,
    pub base_hold_duration_ms: u64,
    pub random_jitter_ms: u64,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum ChordMode {
    Simultaneous, // 并发按下
    Arpeggiate,   // 强制微小间隔（排队）
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum OutOfBoundsStrategy {
    SmartFold, // 智能折叠到最近八度
    Drop,      // 丢弃不弹
}

/// 键位映射条目
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct KeymapEntry {
    pub vk_code: String, // 虚拟键码，如 "Q", "W", "SPACE"
    pub modifier: Option<String>, // 修饰键，如 "Ctrl", "Shift"
}