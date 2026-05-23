Multi-turn action execution with two viewing modes: progress bar and time-skip.

## Why
Actions like pitching a tent (300 game-seconds), brewing tea (180 sec), or sleeping (jumps 8 hours) span many turns. The player must be able to (a) watch the action, (b) skip to the end, and (c) abort if something goes wrong.

## ActionQueue
```rust
struct ActionQueue {
    steps: Vec<ActionStep>,   // e.g. [PitchTent, UnrollBedroll] for setup-camp
    current: usize,
    seconds_remaining_in_step: u64,
    view_mode: ViewMode,      // ProgressBar | TimeSkip
}
```

## Progress-bar mode
- Each game-tick advances the action by N seconds (chunked to amortize render cost; e.g. 5-sec chunks).
- HUD shows a progress bar + remaining time.
- Any non-Select-hold input cancels the queue.

## Time-skip mode
- The executor advances the queue's full remaining time **but must still tick interrupt conditions every game-second**:
  - Any need entering the <10 zone (death threshold imminent)
  - In slice 2+: hostile entering FOV; weather change
- On interrupt: queue pauses at the interrupting turn, control returns to the player, HUD shows the reason.
- On clean completion: queue resolves; player regains control at the final game-time.

## Toggle UX
- Holding Select swaps the view mode mid-action.
- Switching to time-skip mid-action immediately fast-forwards the remaining duration (subject to interrupts).
- Switching back to progress-bar from time-skip does nothing if the action already completed; otherwise resumes per-tick advancement.

## Restoration on load
- `RunSave.current_action` stores the queue mid-execution. Loading mid-pitch restores the remaining seconds + step index + view mode.

Reference: `[[Survival - Command menu]]`, `[[Survival - Game time clock and needs]]`.
