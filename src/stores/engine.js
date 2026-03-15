/**
 * @file engine.js
 * @description 核心状态机，负责统筹前端 UI 与 Rust 底层音频引擎的通信与状态同步
 */
import { defineStore } from 'pinia'
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'

export const useEngineStore = defineStore('engine', () => {
    // ==========================================
    // [状态] 播放与曲库控制
    // ==========================================
    const isPlaying = ref(false)
    const isPaused = ref(false) // 新增：区分暂停与停止
    const currentFileName = ref(null)

    const localMidis = ref([])
    const totalDurationMs = ref(0)
    const currentProgressMs = ref(0)

    // ==========================================
    // [状态] 引擎与渲染数据
    // ==========================================
    const tracks = ref([])
    const activeNotes = ref(new Set())

    // 核心降维策略，默认使用最高级的无损纯净折叠
    const outOfBoundsStrategy = ref('PureFold')
    const globalTranspose = ref(0)
    const suggestedTranspose = ref(0)

    // ==========================================
    // [内部机制] 前端高频计时器 (用于平滑渲染进度条)
    // ==========================================
    let progressTimer = null;
    let playbackStartTime = 0;
    let startOffsetMs = 0;

    const startProgressTimer = () => {
        stopProgressTimer();
        playbackStartTime = Date.now();
        startOffsetMs = currentProgressMs.value;

        progressTimer = setInterval(() => {
            currentProgressMs.value = startOffsetMs + (Date.now() - playbackStartTime);
            if (currentProgressMs.value >= totalDurationMs.value) {
                currentProgressMs.value = totalDurationMs.value;
                stopProgressTimer();
                isPlaying.value = false;
            }
        }, 50); // 50ms 刷新率，保证视觉极度丝滑
    }

    const stopProgressTimer = () => {
        if (progressTimer) clearInterval(progressTimer);
    }

    // ==========================================
    // [动作] 核心交互 API
    // ==========================================

    /**
     * 响应来自 Rust 的高频音符脉冲，点亮/熄灭对应的虚拟按键
     * @param {number} note - MIDI 音符编号
     * @param {boolean} active - 是否按下
     */
    const setNoteActive = (note, active) => {
        // 使用克隆覆盖法，强制触发 Vue 的深层响应式渲染
        const newNotes = new Set(activeNotes.value)
        if (active) newNotes.add(note)
        else newNotes.delete(note)
        activeNotes.value = newNotes
    }

    /**
     * 同步静音状态到 Rust 物理底层
     * @param {number} channel - 通道号
     */
    const toggleMute = async (channel) => {
        const track = tracks.value.find(t => t.channel === channel)
        if (track) {
            track.isMuted = !track.isMuted
            try {
                await invoke('toggle_mute', { channel: channel })
            } catch (error) {
                console.error(`静音通道 ${channel} 失败:`, error)
            }
        }
    }

    return {
        // 状态导出
        isPlaying, isPaused, currentFileName, tracks, activeNotes,
        outOfBoundsStrategy, globalTranspose, suggestedTranspose,
        localMidis, totalDurationMs, currentProgressMs,
        // 方法导出
        startProgressTimer, stopProgressTimer,
        toggleMute, setNoteActive
    }
})