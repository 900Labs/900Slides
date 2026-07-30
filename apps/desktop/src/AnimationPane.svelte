<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import MotionPathEditor from './MotionPathEditor.svelte'
  import type {
    BuildEffectDto,
    BuildStepDto,
    DeckSnapshot,
    RectDto,
    SlideSnapshot,
    TriggerDto,
  } from './lib/types'

  interface Props {
    /** The slide whose animation is being edited. */
    slide: SlideSnapshot
    /** Adopts the deck snapshot returned by each animation command. */
    onApplied: (deck: DeckSnapshot) => void
  }

  let { slide, onApplied }: Props = $props()

  /** Build-in effects offered by the step picker. */
  const BUILD_EFFECTS: BuildEffectDto[] = [
    'fade',
    'slide_in_left',
    'slide_in_right',
    'slide_in_top',
    'slide_in_bottom',
    'appear',
    'disappear',
    'motion_path',
  ]

  /** Trigger kinds in display order. */
  const TRIGGERS: { value: TriggerDto; label: string }[] = [
    { value: 'on_click', label: 'On Click' },
    { value: 'with_previous', label: 'With Previous' },
    { value: 'after_previous', label: 'After Previous' },
  ]

  /** Selected shape index for adding a build step. */
  let selectedShapeIndex = $state<number>(0)
  /** Selected build effect for adding a build step. */
  let selectedBuildEffect = $state<BuildEffectDto>('fade')
  /** Duration (ms) for a new build step. */
  let selectedBuildDuration = $state(500)

  /** Step index whose motion-path editor overlay is open, or null. */
  let editingPath = $state<number | null>(null)

  const steps = $derived<BuildStepDto[]>(slide.animation?.steps ?? [])
  const reduceMotion = $derived<boolean>(slide.reduceMotion === true)

  /** A short, human-readable label for a shape at a given index. */
  function shapeLabel(index: number): string {
    const shape = slide.shapes[index]
    return shape ? `${index}: ${shape.kind}` : `Shape ${index}`
  }

  /** Replaces underscores with spaces and title-cases the first letter. */
  function effectLabel(effect: BuildEffectDto): string {
    return effect.replace(/_/g, ' ')
  }

  /** Sets the trigger of a build step. */
  async function onTrigger(stepIndex: number, trigger: TriggerDto): Promise<void> {
    const deck = await invoke<DeckSnapshot>('set_build_step_trigger', {
      slide_id: slide.id,
      step_index: stepIndex,
      trigger,
    })
    onApplied(deck)
  }

  /** Sets the delay of a build step. */
  async function onDelay(stepIndex: number, delayMs: number): Promise<void> {
    const deck = await invoke<DeckSnapshot>('set_build_step_delay', {
      slide_id: slide.id,
      step_index: stepIndex,
      delay_ms: delayMs,
    })
    onApplied(deck)
  }

  /** Sets the full duration sequence by replacing all steps (duration lives on
   *  the step). Since there is no single-field duration command, the step is
   *  removed and re-added is not ideal; instead the deck's set_slide_animation
   *  replaces the whole sequence with the new duration. */
  async function onDuration(stepIndex: number, durationMs: number): Promise<void> {
    const next = steps.map((step, i) =>
      i === stepIndex ? { ...step, durationMs } : step,
    )
    const deck = await invoke<DeckSnapshot>('set_slide_animation', {
      slide_id: slide.id,
      steps: next,
    })
    onApplied(deck)
  }

  /** Removes a build step by index. */
  async function onRemove(stepIndex: number): Promise<void> {
    const deck = await invoke<DeckSnapshot>('remove_build_step', {
      slide_id: slide.id,
      step_index: stepIndex,
    })
    onApplied(deck)
  }

  /** Moves a build step up or down. */
  async function onMove(from: number, to: number): Promise<void> {
    if (to < 0 || to >= steps.length) return
    const deck = await invoke<DeckSnapshot>('move_build_step', {
      slide_id: slide.id,
      from,
      to,
    })
    onApplied(deck)
  }

  /** Appends a build step to the slide's animation sequence. */
  async function onAdd(): Promise<void> {
    const deck = await invoke<DeckSnapshot>('add_build_step', {
      slide_id: slide.id,
      shape_index: selectedShapeIndex,
      effect: selectedBuildEffect,
      duration_ms: selectedBuildDuration,
    })
    onApplied(deck)
  }

  /** Toggles the per-slide reduce-motion override. */
  async function onToggleReduceMotion(next: boolean): Promise<void> {
    const deck = await invoke<DeckSnapshot>('set_slide_reduce_motion', {
      slide_id: slide.id,
      reduce_motion: next ? true : null,
    })
    onApplied(deck)
  }

  /** Saves a motion path for a step. */
  async function onMotionPath(stepIndex: number, path: RectDto[] | null): Promise<void> {
    const deck = await invoke<DeckSnapshot>('set_build_step_motion_path', {
      slide_id: slide.id,
      step_index: stepIndex,
      path,
    })
    onApplied(deck)
    editingPath = null
  }
</script>

<div class="animation-pane">
  <div class="section">
    <label class="reduce-motion">
      <input
        type="checkbox"
        checked={reduceMotion}
        onchange={(e) => onToggleReduceMotion((e.target as HTMLInputElement).checked)}
      />
      Reduce motion (instant build-ins)
    </label>
  </div>

  <div class="section">
    <h3>Build Steps</h3>
    {#if steps.length > 0}
      <ol class="build-list">
        {#each steps as step, stepIndex (stepIndex)}
          <li class="build-item">
            <div class="build-head">
              <span class="build-info">
                {shapeLabel(step.shapeIndex)} — {effectLabel(step.effect)}
              </span>
              <span class="build-actions">
                <button
                  onclick={() => onMove(stepIndex, stepIndex - 1)}
                  disabled={stepIndex === 0}
                  type="button"
                  title="Move up"
                >
                  ↑
                </button>
                <button
                  onclick={() => onMove(stepIndex, stepIndex + 1)}
                  disabled={stepIndex === steps.length - 1}
                  type="button"
                  title="Move down"
                >
                  ↓
                </button>
                <button
                  onclick={() => onRemove(stepIndex)}
                  type="button"
                  title="Remove"
                >
                  −
                </button>
              </span>
            </div>

            <label class="field">
              Trigger
              <select
                value={step.trigger}
                onchange={(e) => onTrigger(stepIndex, (e.target as HTMLSelectElement).value as TriggerDto)}
              >
                {#each TRIGGERS as t}
                  <option value={t.value}>{t.label}</option>
                {/each}
              </select>
            </label>

            <label class="field">
              Duration: {step.durationMs}ms
              <input
                type="range"
                min={0}
                max={3000}
                step={100}
                value={step.durationMs}
                onchange={(e) => onDuration(stepIndex, Number.parseInt((e.target as HTMLInputElement).value, 10))}
              />
            </label>

            <label class="field">
              Delay: {step.delayMs}ms
              <input
                type="range"
                min={0}
                max={2000}
                step={50}
                value={step.delayMs}
                onchange={(e) => onDelay(stepIndex, Number.parseInt((e.target as HTMLInputElement).value, 10))}
              />
            </label>

            {#if step.effect === 'motion_path'}
              <div class="motion-row">
                <span class="motion-count">
                  {step.motionPath ? step.motionPath.length : 0} waypoints
                </span>
                <button type="button" onclick={() => (editingPath = stepIndex)}>
                  {step.motionPath && step.motionPath.length > 0 ? 'Edit path' : 'Add path'}
                </button>
              </div>
            {/if}
          </li>
        {/each}
      </ol>
    {:else}
      <p class="placeholder">No build steps yet.</p>
    {/if}
  </div>

  <div class="section">
    <h3>Add Build Step</h3>
    <label class="field">
      Shape
      <select bind:value={selectedShapeIndex}>
        {#each slide.shapes as shape, index}
          <option value={index}>{index}: {shape.kind}</option>
        {/each}
      </select>
    </label>
    <label class="field">
      Effect
      <select bind:value={selectedBuildEffect}>
        {#each BUILD_EFFECTS as effect}
          <option value={effect}>{effectLabel(effect)}</option>
        {/each}
      </select>
    </label>
    <label class="field">
      Duration: {selectedBuildDuration}ms
      <input type="range" min={0} max={3000} step={100} bind:value={selectedBuildDuration} />
    </label>
    <button onclick={onAdd} type="button">Add Step</button>
  </div>

  {#if editingPath !== null}
    {@const idx = editingPath}
    {@const activeStep = steps[idx]}
    {#if activeStep}
      <MotionPathEditor
        waypoints={activeStep.motionPath ?? []}
        onCancel={() => (editingPath = null)}
        onSave={(path) => onMotionPath(idx, path)}
      />
    {/if}
  {/if}
</div>

<style>
  .animation-pane {
    display: flex;
    flex-direction: column;
    gap: 1rem;
  }
  .section {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }
  .reduce-motion {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    cursor: pointer;
  }
  .build-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }
  .build-item {
    border: 1px solid var(--border, #ddd);
    border-radius: 6px;
    padding: 0.5rem;
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }
  .build-head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 0.5rem;
  }
  .build-info {
    font-weight: 600;
  }
  .build-actions {
    display: flex;
    gap: 0.25rem;
  }
  .build-actions button {
    width: 1.75rem;
    height: 1.75rem;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    font-size: 0.85rem;
  }
  .motion-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 0.5rem;
  }
  .motion-count {
    font-size: 0.85rem;
    opacity: 0.85;
  }
  .placeholder {
    opacity: 0.7;
    margin: 0;
  }
</style>
