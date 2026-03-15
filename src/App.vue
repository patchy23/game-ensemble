<template>
  <div class="daw-layout">
    <header class="app-header">
      <h1>Game Ensemble 🎼 <span class="version">v1.0-RC</span></h1>
      <button class="action-btn upload-btn" @click="uploadMidiFile">☁️ 导入 MIDI</button>
    </header>

    <main class="main-workspace">
      <aside class="library-panel">
        <h2 class="panel-title">📂 本地曲库</h2>
        <div class="midi-list">
          <div
              v-for="file in engineStore.localMidis"
              :key="file"
              class="midi-item"
              :class="{ active: engineStore.currentFileName === file }"
              @click="selectAndLoadMidi(file)"
          >
            <span class="midi-name" :title="file">🎵 {{ file }}</span>
            <button class="delete-btn" @click.stop="deleteMidiFile(file)" title="删除曲目">🗑️</button>
          </div>

          <div v-if="engineStore.localMidis.length === 0" class="empty-tip">
            暂无曲目，请点击右上角导入
          </div>
        </div>
      </aside>

      <section class="mixer-panel">
        <TrackMixer v-if="engineStore.currentFileName" />
        <div v-else class="empty-tip center">👈 请先从左侧选择一首曲目</div>
      </section>
    </main>

    <footer class="bottom-dock">
      <div class="transport-bar" :class="{ disabled: !engineStore.currentFileName }">
        <div class="playback-controls">
          <button class="ctrl-btn play" v-if="!engineStore.isPlaying" @click="handlePlay" title="播放">▶</button>
          <button class="ctrl-btn pause" v-else @click="handlePause" title="暂停">⏸</button>
          <button class="ctrl-btn stop" @click="handleStop" title="停止">⏹</button>
        </div>

        <div class="progress-section">
          <span class="time-text">{{ formatTime(engineStore.currentProgressMs) }}</span>
          <div class="slider-wrapper">
            <input
                type="range" min="0" :max="engineStore.totalDurationMs"
                v-model.number="engineStore.currentProgressMs"
                @input="handleSeekPreview" @change="handleSeekConfirm"
                class="custom-progress-slider"
                :disabled="!engineStore.currentFileName"
            >
            <div class="slider-fill" :style="{ width: progressPercentage + '%' }"></div>
          </div>
          <span class="time-text">{{ formatTime(engineStore.totalDurationMs) }}</span>
        </div>

        <button class="toggle-kbd-btn" @click="isKeyboardVisible = !isKeyboardVisible">
          🎹 {{ isKeyboardVisible ? '收起键盘' : '展开键盘' }}
        </button>
      </div>

      <div class="keyboard-wrapper" v-show="isKeyboardVisible">
        <VirtualKeyboard />
      </div>
    </footer>
  </div>
</template>

<script setup>
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { open } from '@tauri-apps/plugin-dialog'
import { useEngineStore } from './stores/engine'
import VirtualKeyboard from './components/VirtualKeyboard.vue'
import TrackMixer from './components/TrackMixer.vue'

const engineStore = useEngineStore()
let noteUnlistenFn = null
let stopUnlistenFn = null

// 控制键盘显隐
const isKeyboardVisible = ref(true)

// 计算进度条百分比（用于炫酷的填充层）
const progressPercentage = computed(() => {
  if (!engineStore.totalDurationMs) return 0;
  return (engineStore.currentProgressMs / engineStore.totalDurationMs) * 100;
})

onMounted(async () => {
  noteUnlistenFn = await listen('note_state_change', (event) => {
    const payload = event.payload
    const isActive = payload.is_active !== undefined ? payload.is_active : payload.isActive
    engineStore.setNoteActive(payload.note, isActive)
  })

  stopUnlistenFn = await listen('playback_stopped', () => {
    engineStore.isPlaying = false
    engineStore.activeNotes.clear()
    engineStore.stopProgressTimer()
  })

  await fetchLocalMidis()
})

onUnmounted(() => {
  if (noteUnlistenFn) noteUnlistenFn()
  if (stopUnlistenFn) stopUnlistenFn()
})

const fetchLocalMidis = async () => {
  try { engineStore.localMidis = await invoke('get_local_midis') }
  catch (e) { console.error("加载曲库失败", e) }
}

const uploadMidiFile = async () => {
  try {
    const filePath = await open({ multiple: false, filters: [{ name: 'MIDI', extensions: ['mid', 'midi'] }] });
    if (!filePath) return;
    const fileName = await invoke('import_midi', { sourcePath: filePath });
    await fetchLocalMidis();
    await selectAndLoadMidi(fileName);
  } catch (error) { alert(`❌ 导入失败: ${error}`); }
}

const selectAndLoadMidi = async (fileName) => {
  if (engineStore.isPlaying) await handleStop();

  engineStore.$patch({ currentFileName: fileName, currentProgressMs: 0, totalDurationMs: 0, isPaused: false });

  try {
    const scanResult = await invoke('scan_midi_tracks', { fileName });
    engineStore.$patch({
      tracks: scanResult.tracks,
      suggestedTranspose: scanResult.suggestedTranspose,
      globalTranspose: scanResult.suggestedTranspose,
      totalDurationMs: scanResult.totalDurationMs
    });
    await invoke('set_global_transpose', { offset: scanResult.suggestedTranspose });
  } catch (error) { alert("扫描文件失败，文件可能已损坏。"); }
}

// ==========================================
// [曲目删除逻辑]
// ==========================================
const deleteMidiFile = async (fileName) => {
  // 1. 二次确认防止手滑
  if (!confirm(`确定要从本地曲库中永久删除「${fileName}」吗？`)) return;

  try {
    // 2. 呼叫 Rust 物理删除
    await invoke('delete_midi', { fileName });

    // 3. 边界处理：如果删除的正好是当前正在播放或选中的歌
    if (engineStore.currentFileName === fileName) {
      if (engineStore.isPlaying) {
        await handleStop(); // 紧急刹车
      }
      // 清空工作台
      engineStore.$patch({
        currentFileName: null,
        tracks: [],
        totalDurationMs: 0,
        currentProgressMs: 0
      });
    }

    // 4. 重新拉取曲库列表更新 UI
    await fetchLocalMidis();
  } catch (error) {
    alert(`❌ 删除失败: ${error}`);
  }
}

const handlePlay = async () => {
  if (!engineStore.currentFileName) return;
  engineStore.isPlaying = true;
  engineStore.isPaused = false;
  engineStore.startProgressTimer();
  await invoke('start_auto_play', { fileName: engineStore.currentFileName, startTimeMs: engineStore.currentProgressMs });
}

const handlePause = async () => {
  await invoke('stop_playback');
  engineStore.isPlaying = false;
  engineStore.isPaused = true;
  engineStore.stopProgressTimer();
}

const handleStop = async () => {
  await invoke('stop_playback');
  engineStore.isPlaying = false;
  engineStore.isPaused = false;
  engineStore.stopProgressTimer();
  engineStore.currentProgressMs = 0;
}

let wasPlayingBeforeSeek = false;
const handleSeekPreview = () => {
  if (engineStore.isPlaying) {
    wasPlayingBeforeSeek = true;
    handlePause();
  }
}
const handleSeekConfirm = async () => {
  if (wasPlayingBeforeSeek) {
    await handlePlay();
    wasPlayingBeforeSeek = false;
  }
}

const formatTime = (ms) => {
  if (!ms || isNaN(ms)) return "0:00";
  const secs = Math.floor(ms / 1000);
  return `${Math.floor(secs / 60)}:${(secs % 60).toString().padStart(2, '0')}`;
}
</script>

<style>
/* 全局充斥视口，禁止原生滚动 */
body { margin: 0; background: #050505; color: #fff; font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif; overflow: hidden; height: 100vh; }
.daw-layout { display: flex; flex-direction: column; height: 100vh; }

.app-header { display: flex; justify-content: space-between; align-items: center; padding: 15px 25px; background: #111; border-bottom: 1px solid #222; flex-shrink: 0; z-index: 10;}
.app-header h1 { margin: 0; font-size: 20px; color: #4ade80; }
.version { font-size: 12px; color: #666; margin-left: 8px; }
.upload-btn { background: #3b82f6; color: white; border: none; padding: 8px 16px; border-radius: 4px; cursor: pointer; font-weight: bold; }
.upload-btn:hover { background: #2563eb; }

/* 中间主工作区 */
.main-workspace { display: flex; flex: 1; overflow: hidden; }

/* 左侧曲库 */
.library-panel { width: 280px; background: #0a0a0a; border-right: 1px solid #222; display: flex; flex-direction: column; }
.panel-title { font-size: 14px; color: #888; padding: 15px; margin: 0; border-bottom: 1px solid #1a1a1a; }
.midi-list { flex: 1; overflow-y: auto; padding: 10px; }
.midi-list::-webkit-scrollbar { width: 6px; }
.midi-list::-webkit-scrollbar-thumb { background: #333; border-radius: 3px; }
.midi-item { padding: 12px; margin-bottom: 8px; background: #151515; border-radius: 6px; cursor: pointer; font-size: 13px; color: #ccc; border: 1px solid transparent; transition: 0.2s; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.midi-item:hover { background: #1f1f1f; }
.midi-item.active { background: #166534; color: #4ade80; border-color: #22c55e; }

/* 右侧混音台：设置为 flex-column 且 overflow: hidden，让 TrackMixer 内部接管滚动 */
.mixer-panel { flex: 1; background: #0a0a0a; display: flex; flex-direction: column; overflow: hidden; padding: 20px; }
.empty-tip { color: #555; font-size: 14px; padding: 20px; text-align: center; }
.empty-tip.center { margin-top: 20vh; font-size: 16px; }

/* 底部 Dock 栏 */
.bottom-dock { background: #111; border-top: 1px solid #222; flex-shrink: 0; display: flex; flex-direction: column; padding: 15px; gap: 15px; z-index: 20; box-shadow: 0 -10px 30px rgba(0,0,0,0.5);}

.transport-bar { display: flex; align-items: center; gap: 20px; background: #1a1a1a; padding: 10px 20px; border-radius: 8px; border: 1px solid #2a2a2a; }
.transport-bar.disabled { opacity: 0.5; pointer-events: none; }

.playback-controls { display: flex; gap: 10px; }
.ctrl-btn { width: 36px; height: 36px; border-radius: 50%; border: none; display: flex; justify-content: center; align-items: center; cursor: pointer; font-size: 14px; color: white; transition: 0.2s; }
.ctrl-btn.play { background: #4ade80; color: #000; }
.ctrl-btn.play:hover { background: #22c55e; transform: scale(1.05); }
.ctrl-btn.pause { background: #f59e0b; }
.ctrl-btn.stop { background: #ef4444; border-radius: 8px; width: 36px; }
.ctrl-btn:hover { filter: brightness(1.2); }

/* 全新定制进度条 UI */
.progress-section { flex: 1; display: flex; align-items: center; gap: 15px; }
.time-text { font-family: monospace; font-size: 12px; color: #aaa; width: 40px; text-align: center; }

.slider-wrapper { position: relative; flex: 1; height: 6px; display: flex; align-items: center; }
.custom-progress-slider {
  -webkit-appearance: none;
  width: 100%;
  background: transparent;
  position: absolute;
  z-index: 2;
  margin: 0;
}
.custom-progress-slider:focus { outline: none; }
.custom-progress-slider::-webkit-slider-runnable-track { width: 100%; height: 6px; background: #333; border-radius: 3px; cursor: pointer; }
.custom-progress-slider::-webkit-slider-thumb {
  -webkit-appearance: none;
  height: 14px; width: 14px;
  border-radius: 50%;
  background: #fff;
  cursor: pointer;
  margin-top: -4px; /* (Track Height / 2) - (Thumb Height / 2) */
  box-shadow: 0 0 10px rgba(74, 222, 128, 0.8);
  border: 2px solid #4ade80;
}
.slider-fill { position: absolute; height: 6px; background: #4ade80; border-radius: 3px; z-index: 1; pointer-events: none; }

.toggle-kbd-btn { background: #222; border: 1px solid #444; color: #ccc; padding: 6px 12px; border-radius: 6px; font-size: 12px; cursor: pointer; transition: 0.2s; }
.toggle-kbd-btn:hover { background: #333; color: white; }

.keyboard-wrapper { transition: all 0.3s ease; }

.midi-item {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 10px 12px;
  margin-bottom: 8px;
  background: #151515;
  border-radius: 6px;
  cursor: pointer;
  border: 1px solid transparent;
  transition: 0.2s;
}
/* 限制文字溢出变省略号 */
.midi-name {
  font-size: 13px;
  color: #ccc;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  flex: 1;
  margin-right: 10px;
}

/* 垃圾桶按钮默认半透明，悬浮变亮 */
.delete-btn {
  background: transparent;
  border: none;
  cursor: pointer;
  font-size: 14px;
  opacity: 0; /* 默认隐藏，悬浮显示更高级 */
  transition: all 0.2s;
  padding: 2px;
}

.midi-item:hover .delete-btn {
  opacity: 0.6;
}

.delete-btn:hover {
  opacity: 1 !important;
  transform: scale(1.2);
}
</style>